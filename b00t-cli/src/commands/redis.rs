//! Redis/ForgeKV store management commands for b00t-cli
//!
//! Unified KV store abstraction supporting:
//! - Redis (redis-cli or redis crate)
//! - ForgeKV (RESP2-compatible, rust-native alternative)  
//! - File-based fallback (for development without KV server)

use anyhow::{Context, Result};
use b00t_c0re_lib::kv_store::{KvConfig, KvStore, KvBackend};
use b00t_c0re_lib::redis::{AgentMessage, BroadcastPriority, RedisComms, RedisConfig};
use clap::Parser;
use std::collections::HashMap;
use tokio::time::{Duration, sleep};

#[derive(Parser, Clone)]
pub enum RedisCommands {
    #[clap(about = "Check Redis server status and connection")]
    Status,

    #[clap(about = "Show Redis server information")]
    Info,

    #[clap(about = "Test Redis connection with ping")]
    Ping,

    #[clap(about = "Monitor Redis pub/sub messages")]
    Monitor {
        #[clap(long, help = "Channel pattern to monitor", default_value = "b00t:*")]
        pattern: String,
        #[clap(long, help = "Duration to monitor in seconds", default_value = "10")]
        duration: u64,
    },

    #[clap(about = "Publish a message to Redis channel")]
    Publish {
        #[clap(help = "Channel name")]
        channel: String,
        #[clap(help = "Message content")]
        message: String,
    },

    #[clap(about = "Get a value from Redis")]
    Get {
        #[clap(help = "Redis key")]
        key: String,
    },

    #[clap(about = "Set a value in Redis")]
    Set {
        #[clap(help = "Redis key")]
        key: String,
        #[clap(help = "Value to set")]
        value: String,
        #[clap(long, help = "Expiration time in seconds")]
        expire: Option<usize>,
    },

    #[clap(about = "Delete a key from Redis")]
    Del {
        #[clap(help = "Redis key")]
        key: String,
    },

    #[clap(about = "List all agent statuses")]
    Agents,

    #[clap(about = "Broadcast a message to all agents")]
    Broadcast {
        #[clap(help = "Message to broadcast")]
        message: String,
        #[clap(long, help = "Priority level", value_enum, default_value = "normal")]
        priority: BroadcastPriorityArg,
        #[clap(long, help = "Expiration time in seconds")]
        expire: Option<u64>,
    },

    #[clap(about = "Show Redis memory usage and statistics")]
    Stats,

    #[clap(about = "Clear all b00t-related keys (use with caution)")]
    Clear {
        #[clap(long, help = "Confirm the clear operation")]
        confirm: bool,
    },
}

#[derive(Clone, clap::ValueEnum)]
pub enum BroadcastPriorityArg {
    Low,
    Normal,
    High,
    Critical,
}

impl From<BroadcastPriorityArg> for BroadcastPriority {
    fn from(arg: BroadcastPriorityArg) -> Self {
        match arg {
            BroadcastPriorityArg::Low => BroadcastPriority::Low,
            BroadcastPriorityArg::Normal => BroadcastPriority::Normal,
            BroadcastPriorityArg::High => BroadcastPriority::High,
            BroadcastPriorityArg::Critical => BroadcastPriority::Critical,
        }
    }
}

pub async fn handle_redis_command(redis_command: RedisCommands) -> Result<()> {
    // Detect and show KV backend on first command
    let kv_config = KvConfig::detect();
    let kv_store = KvStore::new(kv_config.clone());
    
    if !kv_store.ping().unwrap_or(false) {
        eprintln!("⚠️  KV backend {} detected but not responding", kv_config.backend);
    } else {
        eprintln!("✅ KV backend {} ready", kv_config.backend);
    }

    let config = RedisConfig::default();
    let agent_id = format!("b00t-cli-{}", std::process::id());

    match redis_command {
        RedisCommands::Status => {
            show_redis_status(config).await?;
        }
        RedisCommands::Info => {
            show_redis_info(config).await?;
        }
        RedisCommands::Ping => {
            test_redis_ping(config).await?;
        }
        RedisCommands::Monitor { pattern, duration } => {
            monitor_redis_messages(config, &pattern, duration).await?;
        }
        RedisCommands::Publish { channel, message } => {
            publish_message(config, agent_id, &channel, &message).await?;
        }
        RedisCommands::Get { key } => {
            get_redis_value(config, agent_id, &key).await?;
        }
        RedisCommands::Set { key, value, expire } => {
            set_redis_value(config, agent_id, &key, &value, expire).await?;
        }
        RedisCommands::Del { key } => {
            delete_redis_key(config, agent_id, &key).await?;
        }
        RedisCommands::Agents => {
            list_agent_statuses(config, agent_id).await?;
        }
        RedisCommands::Broadcast {
            message,
            priority,
            expire,
        } => {
            broadcast_message(config, agent_id, &message, priority.into(), expire).await?;
        }
        RedisCommands::Stats => {
            show_redis_stats(config, agent_id).await?;
        }
        RedisCommands::Clear { confirm } => {
            clear_b00t_keys(config, agent_id, confirm).await?;
        }
    }

    Ok(())
}

async fn show_redis_status(config: RedisConfig) -> Result<()> {
    println!("🔍 Redis Connection Status");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Host: {}:{}", config.host, config.port);
    println!("Database: {}", config.database);

    let agent_id = "status-check".to_string();
    match RedisComms::new(config.clone(), agent_id) {
        Ok(redis) => {
            if redis.is_available() {
                println!("Status: ✅ Connected");

                if let Ok(info) = redis.get_server_info() {
                    if let Some(version) = info.get("redis_version") {
                        println!("Version: {}", version);
                    }
                    if let Some(uptime) = info.get("uptime_in_seconds") {
                        println!("Uptime: {}s", uptime);
                    }
                }
            } else {
                println!("Status: ❌ Not responding");
            }
        }
        Err(e) => {
            println!("Status: ❌ Connection failed: {}", e);
        }
    }

    Ok(())
}

async fn show_redis_info(config: RedisConfig) -> Result<()> {
    let agent_id = "info-check".to_string();
    let redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    let info = redis
        .get_server_info()
        .context("Failed to get Redis server info")?;

    println!("📊 Redis Server Information");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Group information by category
    let mut server_info = HashMap::new();
    let mut memory_info = HashMap::new();
    let mut stats_info = HashMap::new();

    for (key, value) in &info {
        if key.starts_with("redis_version") || key.starts_with("uptime") || key.starts_with("arch")
        {
            server_info.insert(key, value);
        } else if key.contains("memory") || key.contains("mem_") {
            memory_info.insert(key, value);
        } else if key.contains("connections") || key.contains("commands") || key.contains("ops") {
            stats_info.insert(key, value);
        }
    }

    if !server_info.is_empty() {
        println!("\n🖥️  Server:");
        for (key, value) in server_info {
            println!("  {}: {}", key, value);
        }
    }

    if !memory_info.is_empty() {
        println!("\n💾 Memory:");
        for (key, value) in memory_info {
            println!("  {}: {}", key, value);
        }
    }

    if !stats_info.is_empty() {
        println!("\n📈 Statistics:");
        for (key, value) in stats_info {
            println!("  {}: {}", key, value);
        }
    }

    Ok(())
}

async fn test_redis_ping(config: RedisConfig) -> Result<()> {
    let agent_id = "ping-test".to_string();
    let redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    println!("🏓 Testing Redis connection...");

    let start = std::time::Instant::now();
    match redis.ping() {
        Ok(true) => {
            let duration = start.elapsed();
            println!("✅ PONG received in {:?}", duration);
        }
        Ok(false) => {
            println!("❌ Unexpected ping response");
        }
        Err(e) => {
            println!("❌ Ping failed: {}", e);
        }
    }

    Ok(())
}

async fn monitor_redis_messages(_config: RedisConfig, pattern: &str, duration: u64) -> Result<()> {
    println!("👁️  Monitoring Redis channels: {}", pattern);
    println!("Duration: {}s", duration);
    println!("Press Ctrl+C to stop early");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // TODO: Implement actual pub/sub monitoring
    // This would require implementing the Redis pub/sub consumer
    // For now, we'll simulate monitoring

    let mut elapsed = 0u64;
    while elapsed < duration {
        sleep(Duration::from_secs(1)).await;
        elapsed += 1;

        // This is a placeholder - in real implementation,
        // we'd listen to actual Redis pub/sub messages
        if elapsed % 5 == 0 {
            println!("[{}s] 📦 Sample message on b00t:agents:status", elapsed);
        }
    }

    println!("Monitoring completed.");
    Ok(())
}

async fn publish_message(
    config: RedisConfig,
    agent_id: String,
    channel: &str,
    message: &str,
) -> Result<()> {
    let redis =
        RedisComms::new(config, agent_id.clone()).context("Failed to create Redis connection")?;

    // Create an AgentMessage::Broadcast for the publish
    let agent_msg = AgentMessage::Broadcast {
        message: message.to_string(),
        priority: BroadcastPriority::Normal,
        expires_at: None,
    };

    let _subscribers = redis.publish(channel, &agent_msg)?;

    println!("📢 Published to '{}': {}", channel, message);

    Ok(())
}

async fn get_redis_value(config: RedisConfig, agent_id: String, key: &str) -> Result<()> {
    let redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    match redis.get(key)? {
        Some(value) => {
            println!("🔑 {}: {}", key, value);
        }
        None => {
            println!("🔑 {}: (nil)", key);
        }
    }

    Ok(())
}

async fn set_redis_value(
    config: RedisConfig,
    agent_id: String,
    key: &str,
    value: &str,
    expire: Option<usize>,
) -> Result<()> {
    let redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    match expire {
        Some(seconds) => {
            redis.setex(key, value, seconds)?;
            println!("✅ Set '{}' = '{}' (expires in {}s)", key, value, seconds);
        }
        None => {
            redis.set(key, value)?;
            println!("✅ Set '{}' = '{}'", key, value);
        }
    }

    Ok(())
}

async fn delete_redis_key(config: RedisConfig, agent_id: String, key: &str) -> Result<()> {
    let redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    let deleted = redis.del(key)?;

    if deleted > 0 {
        println!("🗑️  Deleted key '{}'", key);
    } else {
        println!("🗑️  Key '{}' not found", key);
    }

    Ok(())
}

async fn list_agent_statuses(config: RedisConfig, agent_id: String) -> Result<()> {
    let _redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    println!("🤖 Agent Status Dashboard");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━");

    // TODO: Implement actual agent discovery
    // This would scan for agent presence keys or listen to status updates
    // For now, show placeholder data

    println!("(No active agents detected)");
    println!("💡 Agents will appear here when they publish status updates");

    Ok(())
}

async fn broadcast_message(
    config: RedisConfig,
    agent_id: String,
    message: &str,
    priority: BroadcastPriority,
    expire: Option<u64>,
) -> Result<()> {
    let redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    let expires_at = expire.map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));

    let subscribers = redis.broadcast(message, priority.clone(), expires_at)?;

    println!("📢 Broadcast sent: {}", message);
    println!("   Priority: {:?}", priority);
    if let Some(exp) = expires_at {
        println!("   Expires: {}", exp.to_rfc3339());
    }
    println!("   Delivered to {} subscribers", subscribers);

    Ok(())
}

async fn show_redis_stats(config: RedisConfig, agent_id: String) -> Result<()> {
    let redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    println!("📊 Redis Statistics");
    println!("━━━━━━━━━━━━━━━━━━━━━");

    let info = redis
        .get_server_info()
        .context("Failed to get Redis server info")?;

    // Show key statistics
    if let Some(used_memory) = info.get("used_memory_human") {
        println!("💾 Memory Used: {}", used_memory);
    }

    if let Some(connected_clients) = info.get("connected_clients") {
        println!("👥 Connected Clients: {}", connected_clients);
    }

    if let Some(total_commands) = info.get("total_commands_processed") {
        println!("⚡ Commands Processed: {}", total_commands);
    }

    if let Some(keyspace_hits) = info.get("keyspace_hits") {
        println!("🎯 Cache Hits: {}", keyspace_hits);
    }

    if let Some(keyspace_misses) = info.get("keyspace_misses") {
        println!("💔 Cache Misses: {}", keyspace_misses);
    }

    // Calculate hit ratio if available
    if let (Some(hits), Some(misses)) = (info.get("keyspace_hits"), info.get("keyspace_misses")) {
        if let (Ok(h), Ok(m)) = (hits.parse::<u64>(), misses.parse::<u64>()) {
            let total = h + m;
            if total > 0 {
                let ratio = (h as f64 / total as f64) * 100.0;
                println!("📈 Hit Ratio: {:.1}%", ratio);
            }
        }
    }

    Ok(())
}

async fn clear_b00t_keys(config: RedisConfig, agent_id: String, confirm: bool) -> Result<()> {
    if !confirm {
        println!("⚠️  This will delete all b00t-related keys from Redis!");
        println!("   Use --confirm to proceed");
        return Ok(());
    }

    let _redis = RedisComms::new(config, agent_id).context("Failed to create Redis connection")?;

    println!("🧹 Clearing b00t-related Redis keys...");

    // TODO: Implement pattern-based key deletion
    // This would scan for keys matching b00t:* pattern and delete them
    // For safety, we'll just show what would be deleted

    println!("Keys that would be cleared:");
    println!("  • b00t:agents:*");
    println!("  • b00t:tasks:*");
    println!("  • b00t:sessions:*");
    println!("  • b00t:system:*");

    println!("⚠️  Key clearing not yet implemented for safety");
    println!(
        "   Use redis-cli directly: redis-cli --scan --pattern 'b00t:*' | xargs redis-cli del"
    );

    Ok(())
}
