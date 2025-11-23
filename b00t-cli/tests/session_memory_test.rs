use std::env;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;
use b00t_cli::session_memory::SessionMemory;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn test_session_memory_basic_operations() {
    let _guard = env_lock().lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("_B00T_TEST_ROOT", temp_dir.path()); }
    
    // Initialize git directory marker
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
    assert!(keys.iter().any(|(key, type_)| key == "test_key" && type_ == "string"));
    assert!(keys.iter().any(|(key, type_)| key == "counter" && type_ == "number"));
    assert!(keys.iter().any(|(key, type_)| key == "enabled" && type_ == "flag"));
    
    // Verify TOML file was created in .git directory
    assert!(temp_dir.path().join(".git/_b00t_.toml").exists());
    
    // Test clear operation
    memory.clear().unwrap();
    assert!(memory.list_keys().is_empty());
    
    unsafe { env::remove_var("_B00T_TEST_ROOT"); }
}

#[test]
fn test_session_memory_persistence() {
    let _guard = env_lock().lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("_B00T_TEST_ROOT", temp_dir.path()); }
    
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
        assert_eq!(memory.get("persistent_key"), Some(&"persistent_value".to_string()));
        assert_eq!(memory.get_num("persistent_counter"), 1);
    }
    
    unsafe { env::remove_var("_B00T_TEST_ROOT"); }
}

#[test]
fn test_readme_tracking() {
    let _guard = env_lock().lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("_B00T_TEST_ROOT", temp_dir.path()); }
    
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
    
    unsafe { env::remove_var("_B00T_TEST_ROOT"); }
}

#[test]
fn test_metadata_tracking() {
    let _guard = env_lock().lock().unwrap();
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("_B00T_TEST_ROOT", temp_dir.path()); }
    
    std::fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    std::fs::write(temp_dir.path().join(".git/HEAD"), "ref: refs/heads/test-branch").ok();
    
    let memory = SessionMemory::load().unwrap();
    
    // Verify metadata is populated
    assert!(!memory.metadata.session_id.is_empty());
    assert!(memory.metadata.created_at <= chrono::Utc::now());
    
    unsafe { env::remove_var("_B00T_TEST_ROOT"); }
}
