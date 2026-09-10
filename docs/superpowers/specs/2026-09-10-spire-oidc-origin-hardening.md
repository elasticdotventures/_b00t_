# Hardening `spire-oidc.promptexecution.com` as a trust root

**Date:** 2026-09-10 · b00t task #200 (goal F of the build-plane trust pass).
**Scope of change:** `PromptExecution/infrastructure` (`terraform/b00t/`) +
vultr1 pingap config. **Authored, phased, not applied** — steps are ordered so
the live JWKS endpoint never breaks. Breaking it breaks *every* cloud's
JWT-SVID validation at once.

## What this endpoint is

`https://spire-oidc.promptexecution.com` serves the OIDC discovery document +
JWKS (public signing keys) for the `promptexecution.com` SPIFFE trust domain.
Every relying party validates every JWT-SVID against it:

- Entra federated credentials (`b00t-agent-sm3lly`/`-fung1`)
- AWS IAM OIDC provider (`aws-oidc-spire.tf`)
- GCP WIF (`spire-pool` / `spire-provider`)
- Alicloud IMS OIDC provider
- Tailscale (console-managed OIDC trust)

Forging one SPIFFE identity fleet-wide = serving a forged JWKS to one of those
relying parties. So the integrity of this HTTP response is the integrity of the
whole identity plane.

## Current path (as built)

```
RP (AWS/GCP/Entra/Alicloud/Tailscale backend)
  │  GET /.well-known/openid-configuration , /keys
  ▼
Cloudflare edge   proxied=true, CF Universal cert           ── TLS #1 (CF fully trusted)
  │  origin pull over the public internet
  ▼
vultr1 reserved public IPv4 : 443   (cloudflare_dns_record.spire_oidc)
  │  pingap, Host-header vhost match
  ▼
k0s  oidc-discovery-provider : 8443
     insecure_addr = "0.0.0.0:8443"
     allow_insecure_scheme = true          ── plaintext inside the node
```

`spire-oidc` shares the reserved IP + pingap 80/443 with `k0s-oidc.promptexecution.com`;
pingap tells them apart by Host header (`terraform/b00t/vultr_node.tf`).

## Gaps

| ID | Gap | Consequence |
|----|-----|-------------|
| **G1** | No **Authenticated Origin Pull**. CF does not present a client cert; the origin does not require one. | Anyone who can route packets to `vultr1:443` (BGP hijack, on-path, brief reserved-IP reassignment) can answer *as the origin* and serve a forged JWKS to CF, which relays it to RPs under a valid CF cert. |
| **G2** | Zone **SSL mode not codified**. No `cloudflare_zone_setting "ssl"` in TF. | If "Flexible": CF→origin is plaintext HTTP across the internet. If "Full" (not strict): CF accepts *any* origin cert → trivial MITM. Only **Full (strict)** is safe, and its state is currently clickops. |
| **G3** | `vultr1:443` is open to the whole internet, not just Cloudflare IP ranges. | The CF proxy (and any AOP added in G1) can be bypassed by connecting to the IP directly with `Host: spire-oidc.promptexecution.com`. |
| **G4** | `allow_insecure_scheme = true` + `insecure_addr` on the discovery provider. | No defence-in-depth: if any hop degrades to HTTP the provider still answers. |
| **G5** | Single origin (vultr1), single hostname. | Availability SPOF for *all* federation — vultr1 down ⇒ every RP's JWKS fetch/refresh fails. Ties to task #191 / goal A. |
| **G6** | Cloudflare zone admin for `promptexecution.com` = fleet-wide identity forgery, undocumented. | Whoever holds CF zone access can repoint `spire-oidc` and serve any JWKS. Inherent to `proxied=true` for a trust anchor; must at least be *written down*. |

## Plan — phased, non-breaking if ordered

Each step is safe to apply on its own once its predecessor holds.

### 1. Codify the edge-side TLS floor (safe now, no origin impact)

New `terraform/b00t/cloudflare-tls-hardening.tf`, CF provider v5.18:

```hcl
resource "cloudflare_zone_setting" "min_tls_version" {
  zone_id    = local.CF_ROOT_ZONE_ID
  setting_id = "min_tls_version"
  value      = "1.2"
}
resource "cloudflare_zone_setting" "always_use_https" {
  zone_id    = local.CF_ROOT_ZONE_ID
  setting_id = "always_use_https"
  value      = "on"
}
resource "cloudflare_zone_setting" "tls_1_3" {
  zone_id    = local.CF_ROOT_ZONE_ID
  setting_id = "tls_1_3"
  value      = "on"
}
```

These are client↔edge only; they cannot affect the CF→origin pull or JWKS
availability. `terraform plan` first to see whether they already match (import,
don't recreate).

### 2. Real origin cert on pingap, then SSL mode = strict

`ssl = "strict"` requires pingap to present a cert Cloudflare validates. Use a
**Cloudflare Origin CA cert** (free, 15-year, trusted only by CF) for
`spire-oidc.promptexecution.com` + `k0s-oidc.promptexecution.com`:

- `cloudflare_origin_ca_certificate` (TF) → deploy the cert+key to pingap's
  vhost for those hosts on vultr1.
- Verify `curl --resolve spire-oidc.promptexecution.com:443:<vultr1-ip> https://spire-oidc.promptexecution.com/keys` succeeds with the Origin CA cert.
- Then `cloudflare_zone_setting "ssl" { value = "strict" }`.

### 3. Authenticated Origin Pulls (closes G1)

```hcl
resource "cloudflare_authenticated_origin_pulls_certificate" "cf_aop" {
  zone_id     = local.CF_ROOT_ZONE_ID
  certificate = <CF's AOP CA cert>   # or a custom mTLS cert pair
  private_key = <...>
  type        = "per-zone"
}
resource "cloudflare_authenticated_origin_pulls" "spire_oidc" {
  zone_id  = local.CF_ROOT_ZONE_ID
  hostname = "spire-oidc.promptexecution.com"
  authenticated_origin_pulls_certificate = cloudflare_authenticated_origin_pulls_certificate.cf_aop.id
  enabled  = true
}
# repeat for k0s-oidc.promptexecution.com
```

pingap on vultr1 must then **require** the CF client cert on those vhosts:
`ssl_verify_client on; ssl_client_certificate <CF AOP CA>;` (or pingap's
equivalent). After this, a direct-to-origin request without CF's client cert is
refused at TLS.

### 4. Firewall `vultr1:443` to Cloudflare ranges (defence-in-depth for G3)

Replace the open `vultr_firewall_rule` for tcp/443 with one rule per CF CIDR
from `https://www.cloudflare.com/ips-v4` / `ips-v6` (a `for_each` over a
data source or a checked-in list refreshed by a small job). Keep tcp/80 open
only for the ACME/redirect path if still needed.

### 5. Turn off the insecure scheme (closes G4)

Once 2–4 hold end-to-end: set the k0s `oidc-discovery-provider`
`allow_insecure_scheme = false` and confirm pingap→provider stays on-node
(loopback / cluster IP, never leaving vultr1). If pingap terminates TLS and
speaks HTTP to `:8443` purely on `localhost`, that hop is acceptable; verify it
is not routed off-box.

### 6. Write the trust dependency down (closes G6)

Add to the goal-D ownership doc and a repo `SECURITY.md`: *"Cloudflare zone
admin for `promptexecution.com` is a root of trust for the
`spiffe://promptexecution.com` identity domain — it can repoint `spire-oidc`
and serve an arbitrary JWKS. Treat CF zone access accordingly (hardware-key
2FA, minimal admin set, audit)."*

### 7. Availability (defer to #191 / goal A)

A second JWKS origin removes G5. Options: a static JWKS mirror on Cloudflare R2
or a Worker (the JWKS is public and changes only on key rotation), or a second
`oidc-discovery-provider` on a second node behind a CF load-balanced hostname.
Whatever HA path #191 picks for the SPIRE server should carry the discovery
provider with it.

## Ship now vs. later

- **Now:** step 1 (edge TLS floor) + this doc. Committed as
  `terraform/b00t/cloudflare-tls-hardening.tf` (steps 2–4 stubbed as commented
  resources with the ordering note).
- **Coordinated change (own PR + a vultr1 maintenance window):** steps 2→3→4→5.
- **#191:** step 7.
