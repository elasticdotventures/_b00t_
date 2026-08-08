# Managing Secrets

Secrets commonly **start in a `.env` file** during development:

```dotenv
DATABASE_PASSWORD=correct-horse-battery-staple
HF_TOKEN=hf_xxx
```

This is acceptable as a bootstrap state, but `.env` must be treated as **secret debt to migrate**, not the permanent storage model.

```text
.env
  │
  │ migrate
  ▼
Secret Manager
  │
  ▼
Secret Reference
  │
  ▼
Runtime Delivery
```

The desired invariant is:

> Configuration records where a secret comes from and how it is delivered. It does not contain the secret value.

---

## Secret lifecycle

### 1. Bootstrap: `.env`

Initial development may use:

```dotenv
DSTACK_PASSWORD=secret
```

Keep it out of Git:

```gitignore
.env
.env.*
!.env.example
```

Document the requirement:

```dotenv
# .env.example
DSTACK_PASSWORD=
```

Then migrate the value to a real provider.

---

## 2. Record the secret requirement in the datum

A `_b00t_` datum requiring credentials should explicitly describe:

* whether the secret is required;
* where it is stored;
* how it is sourced;
* how it should be delivered.

For example:

```yaml
secrets:
  dstack_password:
    required: true

    source:
      provider: infisical
      project: infrastructure
      environment: production
      path: /dstack
      key: DSTACK_PASSWORD

    delivery:
      type: env
      name: DSTACK_PASSWORD
```

Or with 1Password:

```yaml
secrets:
  dstack_password:
    required: true

    source:
      provider: onepassword
      ref: op://Infrastructure/DStack/password

    delivery:
      type: env
      name: DSTACK_PASSWORD
```

Or SOPS:

```yaml
secrets:
  dstack_password:
    required: true

    source:
      provider: sops
      file: secrets.enc.yaml
      key: dstack_password

    delivery:
      type: fifo
```

The datum describes the **secret dependency**, not its value.

---

# Recommended patterns

## Infisical

Migrate:

```text
.env
  │
  ▼
Infisical
```

Then launch the application with secrets scoped to the child process:

```bash
infisical run --env=prod -- dstack apply
```

The application receives:

```text
DSTACK_PASSWORD=<resolved at runtime>
```

without `_b00t_` generating a secret-bearing YAML file.

Example datum:

```yaml
secrets:
  dstack_password:
    required: true

    source:
      provider: infisical
      path: /dstack
      key: DSTACK_PASSWORD

    delivery:
      type: env
      name: DSTACK_PASSWORD
```

Use Infisical when an organisation wants a central secret manager with access control, environments and rotation.

---

## Teller

Teller maps secrets from external providers into the environment of a command.

For example:

```bash
teller run -- dstack apply
```

A Teller configuration can map:

```text
DSTACK_PASSWORD
        │
        ▼
Vault / AWS / GCP / other provider
```

The datum can record Teller as the resolver:

```yaml
secrets:
  dstack_password:
    required: true

    source:
      provider: teller
      variable: DSTACK_PASSWORD

    delivery:
      type: env
      name: DSTACK_PASSWORD
```

Then:

```bash
teller run -- b00t run dstack
```

Teller is useful when the secret store may vary but applications expect conventional environment variables.

---

## `vals`

Use reference URIs when the configuration itself needs to identify the secret source:

```yaml
password: ref+vault://secret/data/dstack#/password
```

A datum could contain:

```yaml
secrets:
  dstack_password:
    required: true

    source:
      provider: vals
      ref: ref+vault://secret/data/dstack#/password

    delivery:
      type: env
      name: DSTACK_PASSWORD
```

This is a strong generic model because the value remains a reference until runtime.

---

## SOPS

Use SOPS when encrypted secret material must live alongside source-controlled configuration.

```bash
sops exec-env secrets.enc.yaml \
  'dstack apply'
```

When software insists on receiving a filename:

```bash
sops exec-file secrets.enc.yaml \
  'application --config {}'
```

On Unix, FIFO delivery can avoid creating a persistent decrypted file.

Example:

```yaml
secrets:
  database_credentials:
    required: true

    source:
      provider: sops
      file: secrets.enc.yaml

    delivery:
      type: fifo
```

---

## Native runtime secrets

Prefer native secret transport when available.

### DSTACK

```yaml
env:
  HF_TOKEN: ${{ secrets.hf_token }}
```

Datum:

```yaml
secrets:
  hf_token:
    required: true

    source:
      provider: dstack
      name: hf_token

    delivery:
      type: native
```

`_b00t_` should preserve the reference and let DSTACK resolve it.

### Podman

Prefer a secret mount:

```bash
podman run \
  --secret database_password \
  my-image
```

The application reads:

```text
/run/secrets/database_password
```

Datum:

```yaml
secrets:
  database_password:
    required: true

    source:
      provider: podman
      name: database_password

    delivery:
      type: file
      path: /run/secrets/database_password
```

### systemd

For host services:

```ini
LoadCredential=dstack_password:/secure/source
```

The service reads the credential from:

```text
$CREDENTIALS_DIRECTORY/dstack_password
```

This is preferable to putting secrets into unit-file environment variables.

---

# YAML rendering is a fallback

When an application requires the actual secret inside generated YAML, render only in a pipeline.

Prefer YAML-aware `yq`:

```bash
DSTACK_PASSWORD="$DSTACK_PASSWORD" \
yq '
  .credentials.password = strenv(DSTACK_PASSWORD)
' dstack.template.yaml |
consumer -
```

Do not do:

```bash
envsubst < template.yaml > rendered.yaml
```

because `rendered.yaml` now becomes another secret that must be protected and deleted.

Preferred order:

```text
native secret reference
    >
runtime secret mount
    >
scoped child environment
    >
FD / FIFO
    >
streamed YAML rendering
    >
temporary plaintext file
```

---

# `_b00t_` datum contract

A datum requiring authentication should declare secrets explicitly:

```yaml
secrets:
  api_token:
    required: true

    source:
      provider: infisical
      key: API_TOKEN

    delivery:
      type: env
      name: API_TOKEN
```

The generic model is:

```rust
struct SecretRequirement {
    required: bool,
    source: SecretSource,
    delivery: SecretDelivery,
}
```

Conceptually:

```rust
enum SecretSource {
    Env { name: String },          // migration/bootstrap only
    Infisical { key: String },
    Teller { variable: String },
    Sops { file: PathBuf, key: String },
    Ref { uri: String },
    Native { name: String },
}
```

```rust
enum SecretDelivery {
    Native,
    Env { name: String },
    File { path: PathBuf },
    Fd,
    Fifo,
}
```

This allows `_b00t_` to detect immature secret configuration:

```text
source: env
```

and recommend migration:

```text
⚠ DSTACK_PASSWORD is sourced from .env

Suggested migration:
  b00t secret migrate dstack_password --to infisical
```

The target state is:

```text
.env                → bootstrap only
datum               → secret requirement + source metadata
secret manager      → actual value
runtime adapter     → narrowest possible delivery mechanism
```

`_b00t_` should know **that a password is required, where it is stored, how to obtain it, and how to deliver it**.

It should never need to persist the password itself.

