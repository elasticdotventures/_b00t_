# MinIO — ARCHIVED, do not use for new work

**MinIO is archived and no longer maintained as a consumable project.**

Confirmed via GitHub API 2026-07-25: `minio/minio` repo has `archived: true`. The
project has also moved to **source-only distribution** — no pre-built container
images or binaries are published anymore; consumers must build from source
themselves (see the minio/minio README's "Source-Only Distribution" section,
and https://github.com/minio/minio/issues/21647 for the community reaction).

## Do not

- Do not add new MinIO usage to any project.
- Do not treat an existing MinIO container/service as "just works" — pulling
  `minio/minio:latest` (or any tag) may silently stop receiving updates, and
  a fresh pull may eventually fail entirely once old tags are pruned.

## Recommendation: RustFS as default replacement, behind a compat facade

Based on an external writeup (not app4dog-specific — spot-checked, not fully
verified; treat as a strong starting point, not settled) plus direct
verification of the checkable claims:

- **[RustFS](https://github.com/rustfs/rustfs)** — Rust, **Apache-2.0**
  (verified via GitHub API — vs. MinIO's AGPL-3.0), not archived, actively
  pushed. Its own S3 compatibility matrix (`docs/architecture/s3-compatibility-matrix.md`,
  verified real, not the writeup's invention) reports **452 implemented**,
  **17 unimplemented**, **273 intentionally-excluded** Ceph s3tests as of this
  check. Same default ports (9000 API / 9001 console), same `server /data
  --console-address :9001` invocation shape, many `MINIO_*` env var aliases,
  MinIO-compatible `/minio/health/live`/`/minio/health/ready`/`/minio/admin`
  paths. **Still beta** — distributed mode, lifecycle management, and KMS are
  explicitly called out as under testing in their own docs.
- **[Garage](https://garagehq.deuxfleurs.fr/)** — mature in its own niche
  (geo-distributed small/edge clusters, production since 2020 at Deuxfleurs)
  but NOT a broad MinIO equivalent — lacks/limits bucket policies, object
  versioning, replication endpoints, object lock, bucket notifications,
  tagging, several lifecycle ops. Use it specifically for geo/edge-replication
  needs, not as a general default.
- **SeaweedFS** (Go, not Rust) — the more mature fallback if RustFS's
  distributed-mode beta status becomes a blocker. S3 + POSIX/FUSE + WebDAV,
  more operational history, but a heavier multi-component architecture
  (masters/volume servers/filers/S3 gateways) and not MinIO-operationally-compatible.
- **Ceph RGW** — only sensible if Ceph is already deployed for other reasons;
  deploying Ceph just to replace MinIO is a category error (control-plane/ops
  overhead far exceeds the problem being solved).
- **VersityGW** (Apache-2.0, Go) — S3-over-POSIX-filesystem gateway; good fit
  for single-host dev or exposing an existing ZFS/Btrfs/XFS archive over S3,
  not a distributed/erasure-coded replacement.
- **[s3s](https://github.com/Nugine/s3s)** (Rust crate) — not a store itself;
  useful for building a custom S3-protocol gateway/compat shim if none of the
  above fit exactly.

## Critical migration rule

**Never swap the binary while keeping MinIO's on-disk data directory.** The
migration boundary is S3 objects + config, not disk blocks — migrate via
actual S3-level copy (`rclone`/`mc mirror`/AWS CLI) into fresh buckets on the
new service, verify object count/bytes/checksums, then cut over. Versioning
history, legal holds, and retention state are not guaranteed to survive a
generic S3-to-S3 copy — treat those as separate migration concerns if in use.

## Compatibility levels worth defining before committing to a replacement

L1 (S3 client endpoint compat) through L5 (on-disk data format compat — never
claim this one). RustFS is strong on L1/L2 (ports, env vars, health probes),
partial on L3 (`mc`/admin ops — validate each command you actually use), and
explicitly incomplete on L4 (IAM/notifications/lifecycle/replication/KMS
semantics) per its own beta-status docs.

## Known existing MinIO usage to migrate (found while writing this note)

- `app4dog` workspace: `s3.dev.app4.dog` -> MinIO on `:9000`, referenced in
  `nginxProxy/nginx.conf`, PM2 `ecosystem.config.js`, and
  `middleware/artifacts/media-storage-config.json`'s `dev` region provider.
  Not yet migrated — this note exists so the migration isn't done blind to
  the fact that MinIO itself is going away, not just "currently down."

## Not yet verified from the external writeup

The Dockerfile/entrypoint-wrapper packaging pattern (`objectstore-minio-compat`
image, `minio` compat executable shimming to RustFS's real entrypoint), the
provider-independent `object_store:` config contract, and the four-layer test
gate (protocol/application/failure/performance) are useful *patterns* but
untested here — don't treat them as already-built or already-working.
