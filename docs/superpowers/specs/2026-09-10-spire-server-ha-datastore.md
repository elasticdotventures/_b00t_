# SPIRE server HA + durable datastore

**Date:** 2026-09-10 · b00t task **#191** (goal A of the build-plane trust
pass). **Authored, not applied.** Answers the operator question *"postgres with
duckdb could read from a durable S3 bucket — how does that work?"* and turns it
into a concrete HA plan.

## Today (the SPOF)

`k0s/spire/` in `PromptExecution/infrastructure`:

| Piece | State | Failure mode |
|---|---|---|
| `spire-server` | `replicas: 1`, `strategy: Recreate` (the manifest comment: *"sqlite datastore + disk keymanager can't run >1 replica"*) | node loss ⇒ no SVID issuance/renewal |
| DataStore | `sqlite3` at `/var/lib/spire/server/datastore.sqlite3` on local disk | disk loss ⇒ all registration entries gone |
| KeyManager | `disk` — `keys.json` on local disk | key loss ⇒ **new CA** ⇒ re-bootstrap trust at every RP (Entra FIC, AWS OIDC thumbprint, Tailscale console; GCP/Azure JWKS auto-heal) |
| UpstreamAuthority | none (self-signed root) | a full rebuild rotates the trust-domain root itself |
| OIDC discovery | 1 sidecar on the same pod | JWKS unavailable when vultr1 is down ⇒ every cloud's token validation fails |

JWT-SVID TTL is **900s** (verified on the live `ns:b00t-ci` entry), so the
blast-radius clock after a server outage is ≤15 min, not hours.

## The operator's question: postgres + DuckDB + a durable S3 bucket

### How that pattern works (it's `pods/pgduck`)

`pgduck` (branch `feat/pgduck-postgres-replacement-clean`, crate
`b00t-pgduck` in `_b00t_`) is a **Postgres-wire-protocol server whose
executor is DuckDB** (`sunng87/pgwire` + `duckdb-rs`, `NoopQueryParser` —
raw passthrough). "Durable on S3" means one of:

1. **DuckDB file on a FUSE mount** — `DUCKDB_PATH` points at a
   `mountpoint-s3` / `s3fs` mount of an R2/S3 bucket. The single `.duckdb`
   file *is* the database; the compute pod is stateless, remount elsewhere on
   loss.
2. **No file at all** — DuckDB's `httpfs` reads `s3://…/*.parquet`
   directly (`read_parquet`, Iceberg/Delta scans). Storage is a pile of
   Parquet objects; DuckDB is pure compute.
3. **Federation** — DuckDB `ATTACH`es a real Postgres over the wire *and*
   reads S3 Parquet in the same query: hot transactional data in PG, cold
   bulk data as Parquet, one SQL surface.

DuckDB deliberately matches pgvector's `<=>` / `<#>` operators, so
pgvector-style similarity queries pass through unmodified — that's what
`pgduck` is *for*: analytical / vector / BI workloads that were paying for a
full Postgres binary they didn't need.

### Why it is the wrong tool for SPIRE's DataStore

SPIRE's DataStore is **OLTP**, and pgduck/DuckDB-on-S3 is **OLAP**:

| SPIRE DataStore needs | DuckDB-on-S3(FS) gives |
|---|---|
| Many small transactional writes (register/renew/prune entries, attested-node heartbeats) | Optimized for bulk columnar scans; row-at-a-time writes are slow |
| Row locks on the CA journal (`SELECT … FOR UPDATE` during signing-key rotation) | DuckDB MVCC is single-writer, single-process; no cross-connection row locking model SPIRE relies on |
| Concurrent access from N server replicas (the entire point of HA) | One writer. A `.duckdb` file on `s3fs` has no multi-process write coordination — and `s3fs` write-back caching means no real `fsync` durability |
| Low, predictable write latency | Every cold page = an S3 GET |
| Zero tolerance for datastore corruption (corrupt CA journal = broken trust domain) | A half-flushed `s3fs` write on pod kill can corrupt the file |

SPIRE's `sql` DataStore plugin supports **`sqlite3`, `postgres`, `mysql`**
only. There is no DuckDB driver, and writing one would inherit all of the
above. So: **do not put SPIRE's DataStore on pgduck.**

### Where "durable S3" *does* belong in this design

- **Postgres WAL archiving to R2** (`wal-g` or `pgBackRest`): continuous WAL
  + periodic base backups to an S3/R2 bucket. The DB *compute* stays
  ephemeral; on node loss you restore to a point-in-time from object
  storage. This is the real "durable on S3" pattern for an OLTP store — and
  it's orthogonal to DuckDB.
- **Audit / history retention**: ship SPIRE's audit log (or a CDC stream of
  DataStore mutations) to Parquet on R2 and query it with `pgduck`. Cheap,
  queryable, and off the hot path.

## Recommended HA plan for #191

Ordered; each step is independently shippable.

### 1. DataStore: sqlite3 → Postgres

`server.conf`:

```hcl
DataStore "sql" {
  plugin_data {
    database_type     = "postgres"
    connection_string = "postgres://spire@<pg-host>:5432/spire?sslmode=verify-full"
    # + read_only_connection_string for replicas once >1
  }
}
```

Postgres options, cheapest-safe first:

- **Managed HA Postgres** — Cloud SQL (GCP, same project as the build plane)
  or Neon (separates compute/storage, storage already S3-backed, scale-to-zero
  friendly). Least operational load; pick this unless there's a reason not to.
- **Self-hosted on k0s** — `zalando/postgres-operator` or CloudNativePG for
  Patroni-style leader election + streaming replicas, **`wal-g` archiving to
  R2** for PITR. More control, more to run. CloudNativePG has first-class
  `barmanObjectStore` → S3.

Migrate: stand up PG, `spire-server` with the new DataStore against a **fresh**
DB, then re-create the handful of registration entries (5 today —
sm3lly/fung1/vultr1-k8s agents + the `ns:b00t-ci` workload) from
`fleet/spire-agents.json` + the buildplane entry. No need to dump/convert
sqlite for a fleet this small.

### 2. KeyManager: disk → cloud KMS

```hcl
KeyManager "aws_kms" {           # or "gcp_kms"
  plugin_data {
    region        = "..."
    key_metadata_file = "/var/lib/spire/server/kms_key_metadata"
    key_policy_file   = "..."
  }
}
```

Now the CA + JWT signing keys live in KMS: survive node loss, can't be copied
off as a file, rotation is a KMS operation. **This is the higher-value change**
— losing `keys.json` is worse than losing the DataStore (entries can be
re-created from Terraform; a new CA can't).

### 3. UpstreamAuthority: add an offline root

Today `spire-server` is its own root. Add `UpstreamAuthority "disk"` (or
`"aws_pca"` / a KMS-held intermediate) so SPIRE issues from an **intermediate**
under a root that lives offline. A full SPIRE rebuild then re-issues an
intermediate under the *same* root — RPs that pinned the root (AWS OIDC
thumbprint, Alicloud's pinned CA — note: Alicloud pins the *Cloudflare edge*
cert today, see `alicloud-oidc-feasibility.md`, which is a separate fragility)
don't have to be touched.

### 4. Scale out

Once 1+2 hold: `replicas: 2` (or 3), `strategy: RollingUpdate`, a
`PodDisruptionBudget`, and spread across nodes — which requires **a second k0s
node** (vultr1 is currently the only one). The OIDC discovery provider scales
with the server.

### 5. JWKS availability (overlaps goal F step 7)

- Front `spire-oidc.promptexecution.com` with a Cloudflare **load-balanced**
  hostname across ≥2 origins, **or**
- Publish a **static JWKS mirror** to R2 / a Cloudflare Worker (the JWKS is
  public and changes only on key rotation — a Worker can serve it with a
  short TTL and refresh from the live provider). Removes "JWKS down when
  vultr1 is down" without a second SPIRE server.

### 6. Backups regardless of path

Even with managed PG: a nightly `spire-server` logical export of entries +
the KMS key metadata file, to R2. Small, and it's the "rebuild from nothing"
artifact.

## Sequencing vs. the other goals

- Goal **F** step 7 (JWKS mirror) == step 5 here — do once, cite both.
- Goal **E** (dedicated `b00t-buildplane-ci` SA, no roles on `spire-agent`)
  is unaffected by this.
- The Entra-hub cutover (`feat/pgduck-postgres-replacement-clean` also
  carries `google-oidc-entra.tf`) is independent; if it lands, this HA work
  makes the *one* SPIRE→Entra link it depends on non-fragile.

## Minimum viable (if only one thing gets done)

**Step 2 (KMS KeyManager) + step 6 (export to R2).** That removes the
unrecoverable failure (lost CA key) for the least work, without needing a
second node or a Postgres. Full HA (steps 1, 3, 4, 5) is the follow-on.
