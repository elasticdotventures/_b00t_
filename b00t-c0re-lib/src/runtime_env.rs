use std::env;

fn codex_managed() -> bool {
    env::var_os("CODEX_MANAGED_BY_BUN").is_some() || env::var_os("CODEX_THREAD_ID").is_some()
}

fn codex_network_disabled() -> bool {
    env::var_os("CODEX_SANDBOX_NETWORK_DISABLED").is_some()
}

pub fn sandbox_root_cause_hint(resource: &str) -> Option<String> {
    if !codex_managed() {
        return None;
    }

    let mut hint = format!(
        "Codex-managed runtime detected; {} may be blocked by sandbox policy or missing unsandboxed permissions.",
        resource
    );

    if codex_network_disabled() {
        hint.push_str(" Network access is explicitly disabled in this session.");
    } else {
        hint.push_str(" Re-run unsandboxed or allow local socket/TCP access before retrying.");
    }

    Some(hint)
}

#[cfg(test)]
mod tests {
    use super::sandbox_root_cause_hint;
    use std::sync::{LazyLock, Mutex};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn returns_none_outside_codex() {
        let _guard = ENV_LOCK.lock().expect("lock");
        unsafe {
            std::env::remove_var("CODEX_MANAGED_BY_BUN");
            std::env::remove_var("CODEX_THREAD_ID");
            std::env::remove_var("CODEX_SANDBOX_NETWORK_DISABLED");
        }

        assert!(sandbox_root_cause_hint("Redis IPC").is_none());
    }

    #[test]
    fn mentions_disabled_network_when_present() {
        let _guard = ENV_LOCK.lock().expect("lock");
        unsafe {
            std::env::set_var("CODEX_MANAGED_BY_BUN", "1");
            std::env::set_var("CODEX_SANDBOX_NETWORK_DISABLED", "1");
            std::env::remove_var("CODEX_THREAD_ID");
        }

        let hint = sandbox_root_cause_hint("Redis IPC").expect("hint");
        assert!(hint.contains("Codex-managed runtime detected"));
        assert!(hint.contains("Network access is explicitly disabled"));

        unsafe {
            std::env::remove_var("CODEX_MANAGED_BY_BUN");
            std::env::remove_var("CODEX_SANDBOX_NETWORK_DISABLED");
        }
    }
}
