//! Pipeline secret injection — securely resolve and inject secrets into stage
//! environments without exposing them in pipeline definitions.
//!
//! # Sources
//! - `File` — reads secret from a file path
//! - `EnvVar` — reads secret from an environment variable
//! - `Keyring` — reads secret from the OS keyring (via `keyring-rs`)
//! - `Prompt` — prompts the user for the secret via stdin (no echo)
//!
//! # Security
//! - `SecretStore::Debug` prints key names but **redacts** values
//! - `SecretRef` never stores raw values — only metadata (key, env_var, source)
//! - Secrets are never printed in logs or Debug output

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ── SecretSource ──────────────────────────────────────────────────────────

/// Where a secret originates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SecretSource {
    /// Read from a file on disk (whitespace-trimmed).
    File {
        path: String,
    },
    /// Read from a process environment variable.
    EnvVar {
        name: String,
    },
    /// Read from the OS keyring / credential store.
    Keyring {
        service: String,
        account: String,
    },
    /// Read interactively from stdin (no echo).
    Prompt {
        /// Human-readable prompt shown to the user.
        description: String,
    },
}

// ── SecretRef ─────────────────────────────────────────────────────────────

/// A reference to a single secret: how to resolve it and where to inject it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecretRef {
    /// Logical key used to retrieve the resolved value from `SecretStore::get()`.
    pub key: String,
    /// Environment variable name into which the resolved value is injected.
    pub env_var: String,
    /// Where to resolve the secret from.
    pub source: SecretSource,
}

// ── SecretStore ───────────────────────────────────────────────────────────

/// In-memory store of resolved secrets, keyed by logical name.
///
/// # Debug redaction
/// The `Debug` impl prints only the resolved key names — **not** the values.
#[derive(Clone)]
pub struct SecretStore {
    /// logical_key -> (env_var_name, resolved_value)
    secrets: HashMap<String, (String, String)>,
}

impl fmt::Debug for SecretStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<&str> = self.secrets.keys().map(|k| k.as_str()).collect();
        f.debug_struct("SecretStore")
            .field("resolved_keys", &keys)
            .field("values", &"<redacted>")
            .finish()
    }
}

impl SecretStore {
    /// Resolve every `SecretRef` in the slice, collecting results into a new
    /// `SecretStore`.  Resolution order is not guaranteed — all refs are
    /// independent and errors are surfaced for the first failure.
    pub fn resolve(secret_refs: &[SecretRef]) -> Result<Self> {
        let mut secrets = HashMap::with_capacity(secret_refs.len());
        for ref_ in secret_refs {
            let value =
                load_secret(ref_).with_context(|| format!("failed to resolve secret '{}'", ref_.key))?;
            secrets.insert(ref_.key.clone(), (ref_.env_var.clone(), value));
        }
        Ok(SecretStore { secrets })
    }

    /// Inject all resolved secrets into the provided environment map, using
    /// each secret's `env_var` field as the destination key.
    ///
    /// This **overwrites** any existing keys with the same name.
    pub fn inject_to_env(&self, env: &mut HashMap<String, String>) {
        for (env_var, value) in self.secrets.values() {
            env.insert(env_var.clone(), value.clone());
        }
    }

    /// Retrieve a resolved secret by its logical `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(|(_, value)| value.as_str())
    }

    /// Number of resolved secrets in this store.
    pub fn len(&self) -> usize {
        self.secrets.len()
    }

    /// Returns `true` when no secrets are stored.
    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }
}

// ── load_secret ───────────────────────────────────────────────────────────

/// Resolve a single `SecretRef` to its string value by reading from the
/// configured source.
///
/// | Source    | Behaviour                                                |
/// |-----------|----------------------------------------------------------|
/// | `File`    | Reads file, trims trailing whitespace                    |
/// | `EnvVar`  | Reads `std::env::var(name)`                              |
/// | `Keyring` | Retrieves credential via `keyring::Entry::get_secret()`  |
/// | `Prompt`  | Prompts on stderr, reads hidden input from stdin          |
pub fn load_secret(ref_: &SecretRef) -> Result<String> {
    match &ref_.source {
        SecretSource::File { path } => {
            let expanded = shellexpand::tilde(path);
            std::fs::read_to_string(expanded.as_ref())
                .map(|s| s.trim().to_string())
                .with_context(|| format!("failed to read secret file at '{}'", path))
        }
        SecretSource::EnvVar { name } => {
            std::env::var(name).map_err(|e| match e {
                std::env::VarError::NotPresent => {
                    anyhow!("environment variable '{}' is not set", name)
                }
                std::env::VarError::NotUnicode(_) => {
                    anyhow!("environment variable '{}' contains invalid unicode", name)
                }
            })
        }
        SecretSource::Keyring { service, account } => {
            #[cfg(feature = "keyring")]
            {
                let entry = keyring::Entry::new(service, account)
                    .with_context(|| format!("failed to create keyring entry for '{}'/'{}'", service, account))?;
                let secret = entry
                    .get_secret()
                    .with_context(|| format!("failed to get secret from keyring for '{}'/'{}'", service, account))?;
                String::from_utf8(secret).map_err(|_| {
                    anyhow!(
                        "keyring secret for '{}'/'{}' is not valid UTF-8",
                        service,
                        account
                    )
                })
            }
            #[cfg(not(feature = "keyring"))]
            {
                anyhow::bail!("keyring secret source requires the 'keyring' feature")
            }
        }
        SecretSource::Prompt { description } => {
            let prompt = format!("{}: ", description);
            rpassword::prompt_password(&prompt)
                .map_err(|e| anyhow!("failed to read secret from prompt: {}", e))
        }
    }
}

// ── SecureStageEnv ────────────────────────────────────────────────────────

/// A pipeline stage with its associated secret refs and an optional resolved
/// store.  The `store` starts as `None` and is populated via `resolve()`.
#[derive(Debug, Clone)]
pub struct SecureStageEnv {
    /// Name of the stage that owns these secrets.
    pub stage_name: String,
    /// Secret references (metadata only — never raw values).
    pub secret_refs: Vec<SecretRef>,
    /// Resolved secrets, populated by calling `resolve()`.
    pub store: Option<SecretStore>,
}

impl SecureStageEnv {
    /// Create a new `SecureStageEnv` with no resolved secrets yet.
    pub fn new(stage_name: impl Into<String>, secret_refs: Vec<SecretRef>) -> Self {
        SecureStageEnv {
            stage_name: stage_name.into(),
            secret_refs,
            store: None,
        }
    }

    /// Resolve all secret refs and store them in `self.store`.
    ///
    /// Returns an error if **any** ref fails to resolve.  On success the
    /// previous store (if any) is replaced.
    pub fn resolve(&mut self) -> Result<()> {
        let store = SecretStore::resolve(&self.secret_refs)?;
        self.store = Some(store);
        Ok(())
    }

    /// Inject resolved secrets into the given environment map.
    ///
    /// # Panics
    /// Panics if `resolve()` has not been called yet (store is `None`).
    pub fn inject_to_env(&self, env: &mut HashMap<String, String>) {
        let store = self
            .store
            .as_ref()
            .expect("SecureStageEnv::inject_to_env called before resolve()");
        store.inject_to_env(env);
    }

    /// Retrieve a resolved secret by its logical key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.store.as_ref().and_then(|s| s.get(key))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::ENV_LOCK;

    // ── Helpers ────────────────────────────────────────────────────────

    fn make_file_secret(path_str: &str) -> SecretRef {
        SecretRef {
            key: "api_key".into(),
            env_var: "API_KEY".into(),
            source: SecretSource::File {
                path: path_str.into(),
            },
        }
    }

    fn make_env_secret(env_name: &str) -> SecretRef {
        SecretRef {
            key: "db_password".into(),
            env_var: "DB_PASSWORD".into(),
            source: SecretSource::EnvVar {
                name: env_name.into(),
            },
        }
    }

    // ── File source reads contents ─────────────────────────────────────

    #[test]
    fn file_source_reads_contents() {
        let dir = tempfile::tempdir().unwrap();
        let secret_path = dir.path().join("my_secret.txt");
        std::fs::write(&secret_path, "s3cr3t-value\n").unwrap();

        let ref_ = make_file_secret(secret_path.to_str().unwrap());
        let store = SecretStore::resolve(&[ref_]).unwrap();
        assert_eq!(store.get("api_key"), Some("s3cr3t-value"));
    }

    #[test]
    fn file_source_trims_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        let secret_path = dir.path().join("secret2.txt");
        std::fs::write(&secret_path, "  val-with-spaces  \n").unwrap();

        let ref_ = make_file_secret(secret_path.to_str().unwrap());
        let store = SecretStore::resolve(&[ref_]).unwrap();
        assert_eq!(store.get("api_key"), Some("val-with-spaces"));
    }

    #[test]
    fn file_source_missing_file_errors() {
        let ref_ = make_file_secret("/tmp/__nonexistent_secret_file_739__");
        let result = SecretStore::resolve(&[ref_]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("failed to resolve secret 'api_key'"),
            "error should mention the secret key: {err}"
        );
    }

    // ── EnvVar source reads environment ────────────────────────────────

    #[test]
    fn envvar_source_reads_environment() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        unsafe {
            std::env::set_var("__B00T_TEST_SECRET_739__", "env-value-739");
        }

        let ref_ = make_env_secret("__B00T_TEST_SECRET_739__");
        let store = SecretStore::resolve(&[ref_]).unwrap();
        assert_eq!(store.get("db_password"), Some("env-value-739"));

        unsafe {
            std::env::remove_var("__B00T_TEST_SECRET_739__");
        }
    }

    #[test]
    fn envvar_source_missing_var_errors() {
        let ref_ = make_env_secret("__B00T_NONEXISTENT_VAR_739__");
        let result = SecretStore::resolve(&[ref_]);
        assert!(result.is_err());
    }

    // ── Inject to env merges correctly ────────────────────────────────

    #[test]
    fn inject_to_env_merges_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let path_a = dir.path().join("a.txt");
        let path_b = dir.path().join("b.txt");
        std::fs::write(&path_a, "val-a").unwrap();
        std::fs::write(&path_b, "val-b").unwrap();

        let refs = vec![
            SecretRef {
                key: "secret_a".into(),
                env_var: "SECRET_A".into(),
                source: SecretSource::File {
                    path: path_a.to_str().unwrap().into(),
                },
            },
            SecretRef {
                key: "secret_b".into(),
                env_var: "SECRET_B".into(),
                source: SecretSource::File {
                    path: path_b.to_str().unwrap().into(),
                },
            },
        ];

        let store = SecretStore::resolve(&refs).unwrap();
        let mut env = HashMap::new();
        env.insert("EXISTING".into(), "keep-me".into());
        store.inject_to_env(&mut env);

        assert_eq!(env.get("SECRET_A"), Some(&"val-a".to_string()));
        assert_eq!(env.get("SECRET_B"), Some(&"val-b".to_string()));
        assert_eq!(env.get("EXISTING"), Some(&"keep-me".to_string()));
    }

    #[test]
    fn inject_to_env_overwrites_existing_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overwrite.txt");
        std::fs::write(&path, "new-value").unwrap();

        let refs = vec![SecretRef {
            key: "dup".into(),
            env_var: "DUPLICATE".into(),
            source: SecretSource::File {
                path: path.to_str().unwrap().into(),
            },
        }];

        let store = SecretStore::resolve(&refs).unwrap();
        let mut env = HashMap::new();
        env.insert("DUPLICATE".into(), "old-value".into());
        store.inject_to_env(&mut env);

        assert_eq!(env.get("DUPLICATE"), Some(&"new-value".to_string()));
    }

    // ── Debug does not print secret values ──────────────────────────────

    #[test]
    fn debug_does_not_print_secret_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("debug_test.txt");
        std::fs::write(&path, "super-secret-value").unwrap();

        let refs = vec![SecretRef {
            key: "my_key".into(),
            env_var: "MY_VAR".into(),
            source: SecretSource::File {
                path: path.to_str().unwrap().into(),
            },
        }];

        let store = SecretStore::resolve(&refs).unwrap();
        let debug_str = format!("{:?}", store);

        assert!(
            debug_str.contains("my_key"),
            "Debug should show key names: {debug_str}"
        );
        assert!(
            !debug_str.contains("super-secret-value"),
            "Debug must NOT contain secret values: {debug_str}"
        );
        assert!(debug_str.contains("<redacted>"), "Debug should have <redacted>: {debug_str}");
    }

    #[test]
    fn secret_ref_debug_does_not_contain_values() {
        let ref_ = SecretRef {
            key: "test".into(),
            env_var: "TEST".into(),
            source: SecretSource::EnvVar {
                name: "TEST_ENV".into(),
            },
        };
        let debug_str = format!("{:?}", ref_);
        // SecretRef never stores values, but confirm it only has metadata fields
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("TEST_ENV"));
    }

    // ── Empty refs resolves to empty store ───────────────────────────────

    #[test]
    fn empty_refs_resolves_to_empty_store() {
        let store = SecretStore::resolve(&[]).unwrap();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.get("anything").is_none());

        let mut env = HashMap::new();
        env.insert("EXISTING".into(), "keep".into());
        store.inject_to_env(&mut env);
        assert_eq!(env.len(), 1);
        assert_eq!(env.get("EXISTING"), Some(&"keep".to_string()));
    }

    // ── SecureStageEnv integration ──────────────────────────────────────

    #[test]
    fn secure_stage_env_resolve_and_inject() {
        let dir = tempfile::tempdir().unwrap();
        let secret_path = dir.path().join("stage_secret.txt");
        std::fs::write(&secret_path, "stage-val").unwrap();

        let mut stage_env = SecureStageEnv::new(
            "encode",
            vec![SecretRef {
                key: "encoder_key".into(),
                env_var: "ENCODER_KEY".into(),
                source: SecretSource::File {
                    path: secret_path.to_str().unwrap().into(),
                },
            }],
        );

        // Initially store is None
        assert!(stage_env.store.is_none());
        assert_eq!(stage_env.stage_name, "encode");

        // Resolve succeeds
        stage_env.resolve().unwrap();
        assert!(stage_env.store.is_some());

        // Get works
        assert_eq!(stage_env.get("encoder_key"), Some("stage-val"));

        // Inject works
        let mut env = HashMap::new();
        stage_env.inject_to_env(&mut env);
        assert_eq!(env.get("ENCODER_KEY"), Some(&"stage-val".to_string()));
    }

    #[test]
    fn secure_stage_env_debug_redacts() {
        let stage_env = SecureStageEnv::new("test", vec![]);
        let debug_str = format!("{:?}", stage_env);
        assert!(debug_str.contains("test"));
        // No values to leak, but confirm the store field is shown
        assert!(debug_str.contains("store"));
    }

    #[test]
    fn secret_store_get_returns_none_for_missing_key() {
        let store = SecretStore::resolve(&[]).unwrap();
        assert_eq!(store.get("nonexistent"), None);
    }

    // ── Serialization round-trip ────────────────────────────────────────

    #[test]
    fn secret_ref_serialize_round_trip() {
        let ref_ = SecretRef {
            key: "api".into(),
            env_var: "API_KEY".into(),
            source: SecretSource::File {
                path: "/etc/secrets/api".into(),
            },
        };
        let json = serde_json::to_string(&ref_).unwrap();
        let back: SecretRef = serde_json::from_str(&json).unwrap();
        assert_eq!(ref_, back);
    }

    #[test]
    fn secret_source_serialize_round_trip_all_variants() {
        let variants = vec![
            SecretSource::File { path: "/path".into() },
            SecretSource::EnvVar { name: "VAR".into() },
            SecretSource::Keyring {
                service: "svc".into(),
                account: "usr".into(),
            },
            SecretSource::Prompt {
                description: "Enter token".into(),
            },
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let back: SecretSource = serde_json::from_str(&json).unwrap();
            assert_eq!(v, back);
        }
    }
}
