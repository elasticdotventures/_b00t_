//! Redis pub/sub agent communication and coordination
//!
//! Provides Redis-based communication channels for agent coordination,
//! session sharing, and distributed b00t operations using the redis crate.

use crate::B00tResult;
use anyhow::Context;
use redis::{Client, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Redis connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub database: u8,
    pub timeout_ms: u64,
    pub max_retries: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            database: 0,
            timeout_ms: 5000,
            max_retries: 3,
        }
    }
}

impl RedisConfig {
    /// Build Redis connection URL
    pub fn connection_url(&self) -> String {
        match &self.password {
            Some(password) => format!("redis://:{}@{}:{}/{}", password, self.host, self.port, self.database),
            None => format!("redis://{}:{}/{}", self.host, self.port, self.database),
        }
    }
}

/// Agent message types for pub/sub communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentMessage {
    /// Agent status update
    Status {
        agent_id: String,
        status: AgentStatus,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Task coordination
    Task {
        task_id: String,
        action: TaskAction,
        agent_id: String,
        payload: serde_json::Value,
    },
    /// Session state change
    Session {
        session_id: String,
        event: SessionEvent,
        data: HashMap<String, serde_json::Value>,
    },
    /// System broadcast
    Broadcast {
        message: String,
        priority: BroadcastPriority,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Online,
    Busy,
    Idle,
    Offline,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskAction {
    Claim,
    Complete,
    Failed,
    Progress,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEvent {
    Created,
    Updated,
    Destroyed,
    Checkpoint,
    Lock,
    Unlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BroadcastPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Redis pub/sub communication hub for b00t agents
pub struct RedisComms {
    client: Client,
    #[allow(dead_code)]
    config: RedisConfig,
    agent_id: String,
}

impl RedisComms {
    /// Create new Redis communication hub
    pub fn new(config: RedisConfig, agent_id: String) -> B00tResult<Self> {
        let client = Client::open(config.connection_url())
            .context("Failed to create Redis client")?;

        Ok(Self {
            client,
            config,
            agent_id,
        })
    }

    /// Get Redis connection
    fn get_connection(&self) -> B00tResult<Connection> {
        let conn = self.client.get_connection()
            .context("Failed to get Redis connection")?;

        // Note: Redis crate handles timeouts internally via connection configuration
        Ok(conn)
    }

    /// Check if Redis is available and responsive
    pub fn is_available(&self) -> bool {
        self.ping().unwrap_or(false)
    }

    /// Ping Redis server
    pub fn ping(&self) -> B00tResult<bool> {
        let mut conn = self.get_connection()?;
        let pong: String = redis::cmd("PING")
            .query(&mut conn)
            .context("Failed to ping Redis server")?;
        Ok(pong == "PONG")
    }

    /// Publish message to a channel
    pub fn publish(&self, channel: &str, message: &AgentMessage) -> B00tResult<i32> {
        let mut conn = self.get_connection()?;
        let json_message = serde_json::to_string(message)
            .context("Failed to serialize agent message")?;

        let subscribers: i32 = redis::cmd("PUBLISH")
            .arg(channel)
            .arg(json_message)
            .query(&mut conn)
            .context("Failed to publish message to Redis")?;

        Ok(subscribers)
    }

    /// Create a new PubSub connection for subscribing to channels
    /// Note: PubSub in Redis crate requires special handling - see examples in tests
    pub fn get_pubsub_client(&self) -> B00tResult<Client> {
        // Return client for PubSub operations - the caller will need to handle
        // the connection lifecycle for subscription operations
        Ok(self.client.clone())
    }

    /// Publish agent status update
    pub fn publish_status(&self, status: AgentStatus) -> B00tResult<i32> {
        let message = AgentMessage::Status {
            agent_id: self.agent_id.clone(),
            status,
            timestamp: chrono::Utc::now(),
        };

        self.publish("b00t:agents:status", &message)
    }

    /// Publish task coordination message
    pub fn publish_task(&self, task_id: &str, action: TaskAction, payload: serde_json::Value) -> B00tResult<i32> {
        let message = AgentMessage::Task {
            task_id: task_id.to_string(),
            action,
            agent_id: self.agent_id.clone(),
            payload,
        };

        self.publish("b00t:tasks:coordination", &message)
    }

    /// Publish session event
    pub fn publish_session(&self, session_id: &str, event: SessionEvent, data: HashMap<String, serde_json::Value>) -> B00tResult<i32> {
        let message = AgentMessage::Session {
            session_id: session_id.to_string(),
            event,
            data,
        };

        self.publish("b00t:sessions:events", &message)
    }

    /// Publish system broadcast
    pub fn broadcast(&self, message: &str, priority: BroadcastPriority, expires_at: Option<chrono::DateTime<chrono::Utc>>) -> B00tResult<i32> {
        let msg = AgentMessage::Broadcast {
            message: message.to_string(),
            priority,
            expires_at,
        };

        self.publish("b00t:system:broadcast", &msg)
    }

    /// Get Redis server info
    pub fn get_server_info(&self) -> B00tResult<HashMap<String, String>> {
        let mut conn = self.get_connection()?;
        let info: String = redis::cmd("INFO")
            .arg("server")
            .query(&mut conn)
            .context("Failed to get Redis server info")?;

        let mut result = HashMap::new();
        for line in info.lines() {
            if line.contains(':') && !line.starts_with('#') {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    result.insert(parts[0].to_string(), parts[1].trim().to_string());
                }
            }
        }

        Ok(result)
    }

    /// Set a key-value pair in Redis
    pub fn set(&self, key: &str, value: &str) -> B00tResult<()> {
        let mut conn = self.get_connection()?;
        let _: () = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .query(&mut conn)
            .context("Failed to SET key in Redis")?;
        Ok(())
    }

    /// Set a key-value pair with expiration
    pub fn setex(&self, key: &str, value: &str, seconds: usize) -> B00tResult<()> {
        let mut conn = self.get_connection()?;
        let _: () = redis::cmd("SETEX")
            .arg(key)
            .arg(seconds)
            .arg(value)
            .query(&mut conn)
            .context("Failed to SETEX key in Redis")?;
        Ok(())
    }

    /// Get a value from Redis
    pub fn get(&self, key: &str) -> B00tResult<Option<String>> {
        let mut conn = self.get_connection()?;
        let result: Option<String> = redis::cmd("GET")
            .arg(key)
            .query(&mut conn)
            .context("Failed to GET key from Redis")?;
        Ok(result)
    }

    /// Increment a numeric value in Redis
    pub fn incr(&self, key: &str) -> B00tResult<i64> {
        let mut conn = self.get_connection()?;
        let result: i64 = redis::cmd("INCR")
            .arg(key)
            .query(&mut conn)
            .context("Failed to INCR key in Redis")?;
        Ok(result)
    }

    /// Increment by a specific amount
    pub fn incrby(&self, key: &str, increment: i64) -> B00tResult<i64> {
        let mut conn = self.get_connection()?;
        let result: i64 = redis::cmd("INCRBY")
            .arg(key)
            .arg(increment)
            .query(&mut conn)
            .context("Failed to INCRBY key in Redis")?;
        Ok(result)
    }

    /// Decrement a numeric value in Redis
    pub fn decr(&self, key: &str) -> B00tResult<i64> {
        let mut conn = self.get_connection()?;
        let result: i64 = redis::cmd("DECR")
            .arg(key)
            .query(&mut conn)
            .context("Failed to DECR key in Redis")?;
        Ok(result)
    }

    /// Delete a key from Redis
    pub fn del(&self, key: &str) -> B00tResult<i32> {
        let mut conn = self.get_connection()?;
        let result: i32 = redis::cmd("DEL")
            .arg(key)
            .query(&mut conn)
            .context("Failed to DEL key from Redis")?;
        Ok(result)
    }

    /// Check if key exists
    pub fn exists(&self, key: &str) -> B00tResult<bool> {
        let mut conn = self.get_connection()?;
        let result: i32 = redis::cmd("EXISTS")
            .arg(key)
            .query(&mut conn)
            .context("Failed to check if key EXISTS in Redis")?;
        Ok(result > 0)
    }

    /// Set expiration on a key
    pub fn expire(&self, key: &str, seconds: usize) -> B00tResult<bool> {
        let mut conn = self.get_connection()?;
        let result: i32 = redis::cmd("EXPIRE")
            .arg(key)
            .arg(seconds)
            .query(&mut conn)
            .context("Failed to set EXPIRE on key in Redis")?;
        Ok(result == 1)
    }

    /// Get hash field
    pub fn hget(&self, key: &str, field: &str) -> B00tResult<Option<String>> {
        let mut conn = self.get_connection()?;
        let result: Option<String> = redis::cmd("HGET")
            .arg(key)
            .arg(field)
            .query(&mut conn)
            .context("Failed to HGET from Redis hash")?;
        Ok(result)
    }

    /// Set hash field
    pub fn hset(&self, key: &str, field: &str, value: &str) -> B00tResult<i32> {
        let mut conn = self.get_connection()?;
        let result: i32 = redis::cmd("HSET")
            .arg(key)
            .arg(field)
            .arg(value)
            .query(&mut conn)
            .context("Failed to HSET in Redis hash")?;
        Ok(result)
    }

    /// Get all hash fields and values
    pub fn hgetall(&self, key: &str) -> B00tResult<HashMap<String, String>> {
        let mut conn = self.get_connection()?;
        let result: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(key)
            .query(&mut conn)
            .context("Failed to HGETALL from Redis hash")?;
        Ok(result)
    }
}

/// Redis-based session storage backend
pub struct RedisSessionStorage {
    redis: RedisComms,
    session_prefix: String,
}

impl RedisSessionStorage {
    /// Create new Redis session storage
    pub fn new(config: RedisConfig, session_prefix: Option<String>) -> B00tResult<Self> {
        let agent_id = format!("session-{}", uuid::Uuid::new_v4());
        let redis = RedisComms::new(config, agent_id)?;

        Ok(Self {
            redis,
            session_prefix: session_prefix.unwrap_or_else(|| "b00t:session".to_string()),
        })
    }

    /// Check if Redis backend is available
    pub fn is_available(&self) -> bool {
        self.redis.is_available()
    }

    /// Get session key
    fn session_key(&self, session_id: &str, key: &str) -> String {
        format!("{}:{}:{}", self.session_prefix, session_id, key)
    }

    /// Set session value
    pub fn set_session_value(&self, session_id: &str, key: &str, value: &str) -> B00tResult<()> {
        let redis_key = self.session_key(session_id, key);
        self.redis.set(&redis_key, value)
    }

    /// Set session value with expiration
    pub fn set_session_value_ex(&self, session_id: &str, key: &str, value: &str, seconds: usize) -> B00tResult<()> {
        let redis_key = self.session_key(session_id, key);
        self.redis.setex(&redis_key, value, seconds)
    }

    /// Get session value
    pub fn get_session_value(&self, session_id: &str, key: &str) -> B00tResult<Option<String>> {
        let redis_key = self.session_key(session_id, key);
        self.redis.get(&redis_key)
    }

    /// Increment session counter
    pub fn incr_session_counter(&self, session_id: &str, key: &str) -> B00tResult<i64> {
        let redis_key = self.session_key(session_id, key);
        self.redis.incr(&redis_key)
    }

    /// Delete session key
    pub fn delete_session_key(&self, session_id: &str, key: &str) -> B00tResult<bool> {
        let redis_key = self.session_key(session_id, key);
        Ok(self.redis.del(&redis_key)? > 0)
    }

    /// Store entire session as hash
    pub fn store_session_hash(&self, session_id: &str, data: &HashMap<String, String>) -> B00tResult<()> {
        let hash_key = format!("{}:{}", self.session_prefix, session_id);
        for (field, value) in data {
            self.redis.hset(&hash_key, field, value)?;
        }
        Ok(())
    }

    /// Get entire session as hash
    pub fn get_session_hash(&self, session_id: &str) -> B00tResult<HashMap<String, String>> {
        let hash_key = format!("{}:{}", self.session_prefix, session_id);
        self.redis.hgetall(&hash_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_config_default() {
        let config = RedisConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 6379);
        assert_eq!(config.database, 0);
    }

    #[test]
    fn test_redis_config_connection_url() {
        let config = RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: Some("secret".to_string()),
            database: 1,
            timeout_ms: 5000,
            max_retries: 3,
        };

        assert_eq!(config.connection_url(), "redis://:secret@localhost:6379/1");

        let config_no_pass = RedisConfig {
            password: None,
            ..config
        };
        assert_eq!(config_no_pass.connection_url(), "redis://localhost:6379/1");
    }

    #[test]
    fn test_agent_message_serialization() {
        let message = AgentMessage::Status {
            agent_id: "test-agent".to_string(),
            status: AgentStatus::Online,
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&message).unwrap();
        let deserialized: AgentMessage = serde_json::from_str(&json).unwrap();

        match deserialized {
            AgentMessage::Status { agent_id, status, .. } => {
                assert_eq!(agent_id, "test-agent");
                assert!(matches!(status, AgentStatus::Online));
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_session_storage_key_generation() {
        let config = RedisConfig::default();
        let storage = RedisSessionStorage::new(config, Some("test".to_string())).unwrap();
        let key = storage.session_key("session123", "counter");
        assert_eq!(key, "test:session123:counter");
    }

    // Integration tests require Redis server running
    #[test]
    #[ignore]
    fn test_redis_connection() {
        let config = RedisConfig::default();
        let comms = RedisComms::new(config, "test-agent".to_string()).unwrap();
        assert!(comms.ping().unwrap());
    }
}