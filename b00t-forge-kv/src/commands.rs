//! Command dispatch — the exact ~16-command subset `RedisComms`
//! (b00t-c0re-lib/src/redis.rs) actually issues, not a full Redis clone.
//! Deliberately scoped this way: every command implemented here has a real
//! caller in the b00t codebase, so there's nothing untested/unused to trust.

use std::sync::Arc;
use std::time::Duration;

use crate::resp::RespValue;
use crate::store::Store;

fn err(msg: impl Into<String>) -> RespValue {
    RespValue::Error(msg.into())
}

fn wrong_args(cmd: &str) -> RespValue {
    err(format!("ERR wrong number of arguments for '{cmd}' command"))
}

fn as_str(bytes: &[u8]) -> Result<&str, RespValue> {
    std::str::from_utf8(bytes).map_err(|_| err("ERR invalid UTF-8 in argument"))
}

/// Non-PUBSUB commands, dispatched per-request. SUBSCRIBE is handled
/// separately by the connection loop since it changes the connection's
/// mode (blocks on incoming published messages rather than request/reply).
pub async fn dispatch(store: &Arc<Store>, args: &[Vec<u8>]) -> RespValue {
    if args.is_empty() {
        return err("ERR empty command");
    }
    let Ok(cmd_str) = as_str(&args[0]) else {
        return err("ERR invalid UTF-8 in command name");
    };
    let cmd = cmd_str.to_ascii_uppercase();
    let rest = &args[1..];

    match cmd.as_str() {
        "PING" => match rest {
            [] => RespValue::SimpleString("PONG".to_string()),
            [msg] => match as_str(msg) {
                Ok(s) => RespValue::bulk(s),
                Err(e) => e,
            },
            _ => wrong_args("PING"),
        },

        "SET" => {
            if rest.len() < 2 {
                return wrong_args("SET");
            }
            let Ok(key) = as_str(&rest[0]) else {
                return err("ERR invalid UTF-8 in key");
            };
            store.set(key, rest[1].clone(), None).await;
            RespValue::ok()
        }

        "SETEX" => {
            if rest.len() != 3 {
                return wrong_args("SETEX");
            }
            let (Ok(key), Ok(seconds_str)) = (as_str(&rest[0]), as_str(&rest[1])) else {
                return err("ERR invalid UTF-8 in argument");
            };
            let Ok(seconds) = seconds_str.parse::<u64>() else {
                return err("ERR value is not an integer or out of range");
            };
            store
                .set(key, rest[2].clone(), Some(Duration::from_secs(seconds)))
                .await;
            RespValue::ok()
        }

        "GET" => {
            if rest.len() != 1 {
                return wrong_args("GET");
            }
            let Ok(key) = as_str(&rest[0]) else {
                return err("ERR invalid UTF-8 in key");
            };
            match store.get(key).await {
                Some(v) => RespValue::BulkString(Some(v)),
                None => RespValue::nil(),
            }
        }

        "DEL" => {
            if rest.is_empty() {
                return wrong_args("DEL");
            }
            match rest.iter().map(|k| as_str(k).map(str::to_string)).collect::<Result<Vec<_>, _>>() {
                Ok(keys) => RespValue::Integer(store.del(&keys).await),
                Err(e) => e,
            }
        }

        "EXISTS" => {
            if rest.is_empty() {
                return wrong_args("EXISTS");
            }
            match rest.iter().map(|k| as_str(k).map(str::to_string)).collect::<Result<Vec<_>, _>>() {
                Ok(keys) => RespValue::Integer(store.exists(&keys).await),
                Err(e) => e,
            }
        }

        "EXPIRE" => {
            if rest.len() != 2 {
                return wrong_args("EXPIRE");
            }
            let (Ok(key), Ok(seconds_str)) = (as_str(&rest[0]), as_str(&rest[1])) else {
                return err("ERR invalid UTF-8 in argument");
            };
            let Ok(seconds) = seconds_str.parse::<i64>() else {
                return err("ERR value is not an integer or out of range");
            };
            if seconds < 0 {
                return RespValue::Integer(store.del(&[key.to_string()]).await.min(1));
            }
            let ok = store.expire(key, Duration::from_secs(seconds as u64)).await;
            RespValue::Integer(if ok { 1 } else { 0 })
        }

        "INCR" => incrby(store, rest, 1).await,
        "DECR" => incrby(store, rest, -1).await,
        "INCRBY" => incrby_with_arg(store, rest, 1).await,
        "DECRBY" => incrby_with_arg(store, rest, -1).await,

        "HSET" => {
            if rest.len() != 3 {
                return wrong_args("HSET");
            }
            let (Ok(key), Ok(field), Ok(value)) =
                (as_str(&rest[0]), as_str(&rest[1]), as_str(&rest[2]))
            else {
                return err("ERR invalid UTF-8 in argument");
            };
            let is_new = store.hset(key, field, value).await;
            RespValue::Integer(if is_new { 1 } else { 0 })
        }

        "HGET" => {
            if rest.len() != 2 {
                return wrong_args("HGET");
            }
            let (Ok(key), Ok(field)) = (as_str(&rest[0]), as_str(&rest[1])) else {
                return err("ERR invalid UTF-8 in argument");
            };
            match store.hget(key, field).await {
                Some(v) => RespValue::bulk(v),
                None => RespValue::nil(),
            }
        }

        "HGETALL" => {
            if rest.len() != 1 {
                return wrong_args("HGETALL");
            }
            let Ok(key) = as_str(&rest[0]) else {
                return err("ERR invalid UTF-8 in key");
            };
            let pairs = store.hgetall(key).await;
            let mut items = Vec::with_capacity(pairs.len() * 2);
            for (f, v) in pairs {
                items.push(RespValue::bulk(f));
                items.push(RespValue::bulk(v));
            }
            RespValue::Array(Some(items))
        }

        "PUBLISH" => {
            if rest.len() != 2 {
                return wrong_args("PUBLISH");
            }
            let Ok(channel) = as_str(&rest[0]) else {
                return err("ERR invalid UTF-8 in channel");
            };
            let n = store.publish(channel, rest[1].clone()).await;
            RespValue::Integer(n)
        }

        "INFO" => RespValue::bulk("# Server\r\nb00t_forge_kv_version:0.1\r\nredis_version:7.0.0-forgekv-compat\r\n"),

        "SUBSCRIBE" => err("ERR SUBSCRIBE must be the connection's first command in a dedicated pub/sub connection (matches real Redis client usage via a separate PubSub connection)"),

        "COMMAND" | "HELLO" | "CLIENT" => RespValue::Array(Some(vec![])),

        other => err(format!("ERR unknown command '{other}', ForgeKV implements the RedisComms subset only")),
    }
}

async fn incrby(store: &Arc<Store>, rest: &[Vec<u8>], delta: i64) -> RespValue {
    if rest.len() != 1 {
        return wrong_args("INCR/DECR");
    }
    let Ok(key) = as_str(&rest[0]) else {
        return err("ERR invalid UTF-8 in key");
    };
    match store.incrby(key, delta).await {
        Ok(v) => RespValue::Integer(v),
        Err(e) => err(format!("ERR {e}")),
    }
}

/// INCRBY/DECRBY: `sign` flips the direction for DECRBY (Redis's DECRBY
/// subtracts its argument; sharing incrby's math keeps that one code path).
async fn incrby_with_arg(store: &Arc<Store>, rest: &[Vec<u8>], sign: i64) -> RespValue {
    if rest.len() != 2 {
        return wrong_args("INCRBY/DECRBY");
    }
    let Ok(amount_str) = as_str(&rest[1]) else {
        return err("ERR invalid UTF-8 in argument");
    };
    let Ok(amount) = amount_str.parse::<i64>() else {
        return err("ERR value is not an integer or out of range");
    };
    incrby(store, &rest[..1], sign * amount).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn run(store: &Arc<Store>, args: &[&str]) -> RespValue {
        let args: Vec<Vec<u8>> = args.iter().map(|s| s.as_bytes().to_vec()).collect();
        dispatch(store, &args).await
    }

    #[tokio::test]
    async fn ping_pong() {
        let store = Arc::new(Store::new());
        assert_eq!(run(&store, &["PING"]).await, RespValue::SimpleString("PONG".into()));
    }

    #[tokio::test]
    async fn set_then_get() {
        let store = Arc::new(Store::new());
        assert_eq!(run(&store, &["SET", "k", "v"]).await, RespValue::ok());
        assert_eq!(run(&store, &["GET", "k"]).await, RespValue::bulk("v"));
    }

    #[tokio::test]
    async fn get_missing_key_is_nil() {
        let store = Arc::new(Store::new());
        assert_eq!(run(&store, &["GET", "missing"]).await, RespValue::nil());
    }

    #[tokio::test]
    async fn hset_hget_hgetall() {
        let store = Arc::new(Store::new());
        assert_eq!(run(&store, &["HSET", "h", "f", "v"]).await, RespValue::Integer(1));
        assert_eq!(run(&store, &["HGET", "h", "f"]).await, RespValue::bulk("v"));
        assert_eq!(
            run(&store, &["HGETALL", "h"]).await,
            RespValue::Array(Some(vec![RespValue::bulk("f"), RespValue::bulk("v")]))
        );
    }

    #[tokio::test]
    async fn publish_with_no_subscribers_returns_zero() {
        let store = Arc::new(Store::new());
        assert_eq!(run(&store, &["PUBLISH", "ch", "hi"]).await, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn incr_and_decr() {
        let store = Arc::new(Store::new());
        assert_eq!(run(&store, &["INCR", "c"]).await, RespValue::Integer(1));
        assert_eq!(run(&store, &["INCRBY", "c", "5"]).await, RespValue::Integer(6));
        assert_eq!(run(&store, &["DECR", "c"]).await, RespValue::Integer(5));
        assert_eq!(run(&store, &["DECRBY", "c", "2"]).await, RespValue::Integer(3));
    }

    #[tokio::test]
    async fn unknown_command_is_a_clean_error_not_a_panic() {
        let store = Arc::new(Store::new());
        assert_eq!(
            run(&store, &["BITCOUNT", "k"]).await,
            err("ERR unknown command 'BITCOUNT', ForgeKV implements the RedisComms subset only")
        );
    }

    #[tokio::test]
    async fn wrong_arity_is_a_clean_error() {
        let store = Arc::new(Store::new());
        assert_eq!(run(&store, &["SET", "onlykey"]).await, wrong_args("SET"));
    }
}
