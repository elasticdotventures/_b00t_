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

## Host Mapping Options

Pingap is a reverse proxy/edge. It is not a browser forward proxy. With a
basic Kubernetes Ingress or `Service type=LoadBalancer`, the cluster can route
by `Host`/SNI after traffic reaches the load balancer, but the operator still
needs a way for the browser to resolve the desired hostnames to that load
balancer.

Preferred options:

1. Real DNS or wildcard DNS to the load balancer IP.
   Use this for cloud or shared demos. Point `*.dev.example.com` at the
   Ingress/LB address, then configure Ingress hosts such as
   `app.dev.example.com` and `api.dev.example.com`.

2. Encoded-IP wildcard DNS such as `sslip.io` or `nip.io`.
   Use this when you have an IP but no DNS zone. For LB IP `203.0.113.10`,
   hosts like `app.203.0.113.10.sslip.io` resolve back to that IP. This is the
   fastest way to prove basic Ingress/LB routing without editing `/etc/hosts`.

3. Local `/etc/hosts`.
   Use this for one developer machine. Map each required host to the local or
   remote LB IP:

   ```text
   127.0.0.1 demo.pingap.test
   203.0.113.10 app.dev.example.test api.dev.example.test
   ```

4. Chromium host resolver rules.
   Use this when you do not want to edit `/etc/hosts` and the browser is local
   to the operator:

   ```bash
   chromium \
     --host-resolver-rules='MAP demo.pingap.test 127.0.0.1' \
     https://demo.pingap.test:19443/
   ```

5. PAC file, only with a real forward proxy.
   PAC chooses a proxy for browser requests; it does not make Pingap a forward
   proxy. Use PAC with `ssh -D`, mitmproxy, or another SOCKS/HTTP forward proxy:

   ```js
   function FindProxyForURL(url, host) {
     if (dnsDomainIs(host, ".dev.example.test")) {
       return "SOCKS5 127.0.0.1:1080; DIRECT";
     }
     return "DIRECT";
   }
   ```

6. SSH local forward to the Ingress/LB.
   Use this when the LB is reachable only from a jumpbox. Keep the browser
   mapping local, then forward the LB listener:

   ```bash
   ssh -N -L 127.0.0.1:19443:ingress.internal:443 jumpbox
   chromium --host-resolver-rules='MAP app.dev.example.test 127.0.0.1' \
     https://app.dev.example.test:19443/
   ```

7. SOCKS or `sshuttle` for broad private-network access.
   Use `ssh -D` plus PAC when only browser traffic needs the private network.
   Use `sshuttle` when the whole WSL node needs to reach many private pod or
   service IPs. This is heavier than a single host mapping and should remain an
   explicit operator choice.

For the target shape "hosts running inside Kubernetes pods with only a basic
Ingress/LoadBalancer", choose option 1 or 2 whenever possible. The Ingress/LB
owns fan-out to Services and pods; host mapping only ensures the browser sends
the right hostname to the LB.

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
