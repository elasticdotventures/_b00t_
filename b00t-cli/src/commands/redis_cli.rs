//! Redis CLI commands with skill-based feature flags
//!
//! 🤓 Redis commands are ONLY available when:
//! 1. Role datum has #redis or #agent-coordination skill tag
//! 2. OR --force flag is provided
//!
//! This ensures AI agents only see tools they're skilled to use

use anyhow::{Context, Result};
use b00t_c0re_lib::aaiii::SkillFeatureFlags;
use b00t_c0re_lib::kv_store::{KvConfig, KvStore};
use b00t_c0re_lib::redis::{RedisComms, RedisConfig};
use clap::Parser;

#[derive(Parser, Clone)]
pub enum RedisCommands {
    #[clap(about = "Check Redis/Valkey server status")]
    Status,

    #[clap(about = "Test connection with PING")]
    Ping,

    #[clap(about = "Get a value")]
    Get {
        #[arg(help = "Key to get")]
        key: String,
    },

    #[clap(about = "Set a value")]
    Set {
        #[arg(help = "Key to set")]
        key: String,
        #[arg(help = "Value to set")]
        value: String,
        #[arg(long, help = "Expiration in seconds")]
        expire: Option<u64>,
    },

    #[clap(about = "Delete a key")]
    Del {
        #[arg(help = "Key to delete")]
        key: String,
    },
}

/// Check if redis commands should be enabled based on role skills
pub fn redis_enabled(role: Option<&str>, force: bool) -> bool {
    if force {
        return true;
    }

    // Check if role has redis-related skills
    if let Some(role_name) = role {
        let flags = SkillFeatureFlags::for_role(role_name);
        return flags.is_enabled("redis-coordination")
            || flags.is_enabled("agent-delegation")
            || flags.is_enabled("hive-cmdb");
    }

    false
}

pub async fn handle_redis_command(cmd: RedisCommands, _force: bool) -> Result<()> {
    // Auto-detect KV backend (Valkey > Redis > ForgeKV > File)
    let config = KvConfig::detect();
    let backend = config.backend; // Save backend before move
    let store = KvStore::new(config);

    if !store.ping().unwrap_or(false) {
        eprintln!("⚠️  KV backend {} not responding", backend);
    }

    match cmd {
        RedisCommands::Status => {
            println!("🔍 KV Store Status");
            println!("Backend: {}", backend);
            println!("Host: {}:{}", store.config().host, store.config().port);
            match store.ping() {
                Ok(true) => println!("Status: ✅ Connected"),
                Ok(false) => println!("Status: ❌ Not responding"),
                Err(e) => println!("Status: ❌ Error: {}", e),
            }
        }
        RedisCommands::Ping => match store.ping() {
            Ok(true) => println!("✅ PONG"),
            Ok(false) => println!("❌ No response"),
            Err(e) => println!("❌ Error: {}", e),
        },
        RedisCommands::Get { key } => match store.get(&key) {
            Ok(Some(value)) => println!("{} = {}", key, value),
            Ok(None) => println!("{} = (nil)", key),
            Err(e) => eprintln!("Error: {}", e),
        },
        RedisCommands::Set { key, value, expire } => match store.set(&key, &value, expire) {
            Ok(_) => println!("✅ OK"),
            Err(e) => eprintln!("Error: {}", e),
        },
        RedisCommands::Del { key } => match store.del(&key) {
            Ok(count) => println!("Deleted {} keys", count),
            Err(e) => eprintln!("Error: {}", e),
        },
    }

    Ok(())
}
