# pingap-devproxy-b00t

Reusable b00t wrapper for Pingap/Pingora development reverse proxies.

This component does not fork Pingap. It standardizes the project-local wrapper
around Pingap:

- `services.mjs` remains the project-owned route and certificate source of truth.
- `bin/gen-pingap-config.mjs` generates Pingap TOML from that source.
- `templates/pingap.pod.yml.tmpl` renders a portable `podman kube play` Pod.
- `bin/pingap-kube-play.sh` owns shadow/cutover/status lifecycle behavior.
- `Containerfile` creates an auditable, pinned wrapper image for GHCR/GAR.

## Project Contract

A consuming repo provides:

```text
_b00t_/k8s/pingap/services.mjs
certs/
```

The service module must export:

```js
export const services = [
  {
    name: "app",
    host: "app.example.test",
    upstreamName: "appUpstream",
    target: "host.containers.internal:5173",
    certGroup: "example",
  },
];

export const certificates = {
  example: {
    tls_cert: "/certs/example.pem",
    tls_key: "/certs/example-key.pem",
    domains: "app.example.test",
    is_default: true,
  },
};
```

## Local Use

```bash
export PINGAP_PROJECT_ROOT="$PWD"
export PINGAP_POD_NAME="my-project-pingap"
~/.b00t/vendor/pingap-devproxy-b00t/bin/gen-pingap-config.mjs
~/.b00t/vendor/pingap-devproxy-b00t/bin/pingap-kube-play.sh --shadow
```

## Image Publication

The wrapper image is intentionally thin. It exists to pin and label the Pingap
runtime, then mirror that artifact to public/private registries.

```bash
just build
just push-ghcr
just push-gar GAR_IMAGE=australia-southeast2-docker.pkg.dev/PROJECT/REPO/pingap-devproxy-b00t
```

Use immutable digests in production deployment manifests. Keep local dev
defaults overrideable through `PINGAP_IMAGE`.
