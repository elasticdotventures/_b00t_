use b00t_cli::session_memory::SessionMemory;
use std::env;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

fn session_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct DirGuard {
    original: std::path::PathBuf,
    cleared_agent: bool,
}

impl DirGuard {
    fn change_to(path: &Path) -> Self {
        let original = env::current_dir().unwrap();
        env::set_current_dir(path).unwrap();
        // Clear agent env markers to avoid contaminating other tests
        std::env::remove_var("_B00T_Agent");
        std::env::remove_var("CLAUDECODE");
        Self { original }
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original);
        // Ensure agent env vars remain cleared after test
        std::env::remove_var("_B00T_Agent");
        std::env::remove_var("CLAUDECODE");
    }
}

#[test]
fn test_session_memory_basic_operations() {
    let _lock = session_lock().lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let _dir_guard = DirGuard::change_to(temp_dir.path());

    // Initialize git repo to create valid git root with .git directory
    std::process::Command::new("git")
        .args(&["init"])
        .output()
        .unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    // Ensure .git directory exists
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();

    // Test creating and loading session memory
    let mut memory = SessionMemory::load().unwrap();

    // Test string operations
    memory.set("test_key", "test_value").unwrap();
    assert_eq!(memory.get("test_key"), Some(&"test_value".to_string()));

    // Test numeric operations
    assert_eq!(memory.incr("counter").unwrap(), 1);
    assert_eq!(memory.incr("counter").unwrap(), 2);
    assert_eq!(memory.decr("counter").unwrap(), 1);
    assert_eq!(memory.get_num("counter"), 1);

    // Test flag operations
    memory.set_flag("enabled", true).unwrap();
    assert!(memory.get_flag("enabled"));
    assert!(!memory.get_flag("disabled"));

    // Test keys listing
    let keys = memory.list_keys();
    assert!(
        keys.iter()
            .any(|(key, type_)| key == "test_key" && type_ == "string")
    );
    assert!(
        keys.iter()
            .any(|(key, type_)| key == "counter" && type_ == "number")
    );
    assert!(
        keys.iter()
            .any(|(key, type_)| key == "enabled" && type_ == "flag")
    );

    // Verify TOML file was created in .git directory
    assert!(temp_dir.path().join(".git/_b00t_.toml").exists());

    // Test clear operation
    memory.clear().unwrap();
    assert!(memory.list_keys().is_empty());
}

#[test]
fn test_session_memory_persistence() {
    let _lock = session_lock().lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let _dir_guard = DirGuard::change_to(temp_dir.path());

    std::process::Command::new("git")
        .args(&["init"])
        .output()
        .unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();

    // Create and populate session memory
    {
        let mut memory = SessionMemory::load().unwrap();
        memory.set("persistent_key", "persistent_value").unwrap();
        memory.incr("persistent_counter").unwrap();
    }

    // Load in a new instance and verify persistence
    {
        let memory = SessionMemory::load().unwrap();
        assert_eq!(
            memory.get("persistent_key"),
            Some(&"persistent_value".to_string())
        );
        assert_eq!(memory.get_num("persistent_counter"), 1);
    }
}

#[test]
fn test_readme_tracking() {
    let _lock = session_lock().lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let _dir_guard = DirGuard::change_to(temp_dir.path());

    std::process::Command::new("git")
        .args(&["init"])
        .output()
        .unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();

    // Test README tracking functionality
    let mut memory = SessionMemory::load().unwrap();

    // Initially README should not be marked as read
    assert!(!memory.is_readme_read());

    // Mark as read and verify
    memory.mark_readme_read().unwrap();
    assert!(memory.is_readme_read());

    // Verify persistence
    let memory2 = SessionMemory::load().unwrap();
    assert!(memory2.is_readme_read());
}

#[test]
fn test_metadata_tracking() {
    let _lock = session_lock().lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let _dir_guard = DirGuard::change_to(temp_dir.path());

    std::process::Command::new("git")
        .args(&["init"])
        .output()
        .unwrap();
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    std::process::Command::new("git")
        .args(&["checkout", "-b", "test-branch"])
        .output()
        .ok();

    let memory = SessionMemory::load().unwrap();

    // Verify metadata is populated
    assert!(!memory.metadata.session_id.is_empty());
    assert!(memory.metadata.created_at <= chrono::Utc::now());
}
