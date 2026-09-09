# deploy/k0s/spire — the SPIRE trust domain (identity-plane step 3)

The single OIDC issuer every cloud (GCP · Azure · AWS) federates against.
Trust domain: `spiffe://b00t.promptexecution.com`. Requirements + rationale:
`docs/superpowers/specs/2026-09-09-identity-plane-minimum-requirements.md`.

**Authored, not applied.** Needs staffed platform work to production-harden
(datastore, CA, ingress cert, monitoring — see the TODOs in `values.yaml`).

## Install

```sh
helm repo add spiffe https://spiffe.github.io/helm-charts-hardened/
helm repo update
helm upgrade --install spire-crds spiffe/spire-crds -n spire-mgmt --create-namespace
helm upgrade --install spire spiffe/spire -n spire-mgmt \
  -f deploy/k0s/spire/values.yaml
# workload -> SPIFFE ID mapping (after the CRD exists):
kubectl kustomize deploy/k0s/spire | kubectl apply -f -
```

## Wire the clouds to SPIRE

Once `https://oidc.b00t.promptexecution.com/.well-known/openid-configuration`
is live:

| Cloud | Set |
|---|---|
| **GCP** | `modules/gcp-build-plane` var `spire_oidc_issuer_uri = "https://oidc.b00t.promptexecution.com"` → adds the `spire-oidc` provider to the `external-idp` pool. Then drop `k0s_oidc_issuer_uri`. |
| **Azure** | `modules/azure-control-plane` var `gcp_federation_issuer` / a new `azurerm_federated_identity_credential` with `issuer = https://oidc.b00t.promptexecution.com`, `subject = <spiffe id>`. |
| **AWS** | `modules/identity-aws` vars `spire_oidc_issuer_uri` (JWT-SVID → IAM OIDC) and/or `spire_ca_bundle_pem` (X.509-SVID → IAM Roles Anywhere). `spiffe_id` pins the workload. |

## Workload SVID delivery

Replaces the bootstrap projected-token + pfnet-webhook path. Pods mount the
SPIFFE CSI driver volume:

```yaml
volumes:
  - name: spiffe
    csi:
      driver: csi.spiffe.io
      readOnly: true
volumeMounts:
  - name: spiffe
    mountPath: /spiffe-workload-api
    readOnly: true
```

`gcs-obj.sh` then reads an `external_account` config whose
`credential_source.file` points at a JWT-SVID fetched from
`/spiffe-workload-api/spire-agent.sock` (via `spiffe-helper` writing it to a
path, or a tiny init step calling the Workload API). The STS exchange in
`gcs-obj.sh` is unchanged.

## Federation growth path

Add a second trust domain (per region / BU) and `ClusterFederatedTrustDomain`
CRs so they cross-trust — additive, no rebuild.
