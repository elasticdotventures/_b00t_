use duct::cmd;

pub fn is_git_repo() -> bool {
    cmd!("git", "rev-parse", "--show-toplevel")
        .stderr_to_stdout()
        .read()
        .is_ok()
}

pub fn get_workspace_root() -> String {
    cmd!("git", "rev-parse", "--show-toplevel")
        .read()
        .unwrap_or_else(|_| "b00t".to_string())
        .trim()
        .to_string()
}
