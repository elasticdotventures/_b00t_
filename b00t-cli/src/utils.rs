use duct::cmd;

pub fn is_git_repo() -> bool {
    cmd!("git", "rev-parse", "--show-toplevel")
        .stderr_to_stdout()
        .read()
        .is_ok()
}

// 🦨: for jinja templates
pub fn get_workspace_root() -> String {
    if let Ok(test_root) = std::env::var("_B00T_TEST_ROOT") {
        return test_root;
    }

    cmd!("git", "rev-parse", "--show-toplevel")
        .read()
        .unwrap_or_else(|_| "b00t".to_string())
        .trim()
        .to_string()
}
