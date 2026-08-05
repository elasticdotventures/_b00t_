//! Credential guard for ScopeStore writes (#899, reframed).
//!
//! The original #899 framing was "reject credential writes to repo-scope
//! only" (git-trackable, risk of a slipped `.gitignore`). Reframed after
//! checking what actually exists: `_b00t_/learn/managing-secrets.md`
//! already prescribes a reference/delivery model ("`_b00t_` should never
//! need to persist the password itself" — secrets come from Infisical/
//! Teller/SOPS/vals at delivery time, not from a stored value) that is
//! orthogonal to and stronger than a repo-vs-node-vs-global distinction.
//! `datum_credential.rs` already has its own narrow, dedicated,
//! home-dir-only, OS-keyring-wrapped storage path — outside ScopeStore
//! entirely.
//!
//! So: ScopeStore rejects credential-shaped writes at **every** scope by
//! default, not just repo-scope, pushing callers toward the reference
//! model the docs already prescribe. `datum_credential.rs`'s storage
//! stays the one explicit, narrow, already-audited exception — it doesn't
//! go through this guard because it doesn't go through ScopeStore at all.

use crate::errors::ScopeError;

/// True when `key` looks like a credential-shaped datum key.
///
/// Matches the `.credential`/`.credentials` suffix convention from
/// b00t-cli's `DatumType::Credential` (`b00t-cli/src/datum_types.rs`,
/// `Credential => ["credential", "credentials"] => ".credential"`) — as a
/// string pattern, not the enum itself. b00t-cli depends on
/// b00t-c0re-gov, not the reverse, so importing `DatumType` here would
/// invert the crate dependency graph. If that suffix convention changes,
/// this needs updating too; this comment is the other end of that link.
pub fn looks_like_credential_key(key: &str) -> bool {
    key.contains(".credential")
}

/// Returns `Err(ScopeError::WriteRejected)` when `key` looks
/// credential-shaped; `Ok(())` otherwise. Scope-independent by design —
/// see this module's doc comment for why "reject everywhere" replaced the
/// original "reject repo-scope only" framing.
pub fn guard_write(key: &str) -> Result<(), ScopeError> {
    if looks_like_credential_key(key) {
        return Err(ScopeError::WriteRejected(format!(
            "key {key:?} looks like a credential (matches the .credential/.credentials \
             datum-type suffix) -- ScopeStore never stores raw secret values, at any \
             scope. Use the reference/delivery pattern from \
             _b00t_/learn/managing-secrets.md instead (Infisical/Teller/SOPS/vals at \
             delivery time), or datum_credential.rs's dedicated OS-keyring-wrapped \
             storage if a local encrypted copy is genuinely required."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_credential_suffix_detected() {
        assert!(looks_like_credential_key("openai.credential"));
        assert!(looks_like_credential_key("aws.credentials"));
    }

    #[test]
    fn credential_shaped_filename_detected() {
        assert!(looks_like_credential_key("openai.credential.toml"));
    }

    #[test]
    fn ordinary_keys_not_flagged() {
        assert!(!looks_like_credential_key("greeting"));
        assert!(!looks_like_credential_key("openai.cli"));
        assert!(!looks_like_credential_key("rust.skill"));
    }

    #[test]
    fn guard_write_rejects_credential_shaped_keys() {
        let err = guard_write("openai.credential").unwrap_err();
        assert!(matches!(err, ScopeError::WriteRejected(_)));
    }

    #[test]
    fn guard_write_allows_ordinary_keys() {
        assert!(guard_write("greeting").is_ok());
    }

    #[test]
    fn error_message_points_to_managing_secrets_doc() {
        let err = guard_write("x.credential").unwrap_err();
        assert!(err.to_string().contains("managing-secrets.md"));
    }
}
