//! RedisScopeStore v2 (stub) — distributed ScopeStore backend (#893 checklist).
//!
//! Deliberately built on `b00t_c0re_lib::redis::RedisComms`'s existing
//! `get`/`set` rather than a new redis client -- that crate already wraps
//! connection setup, TCP-connectivity error hints, and a
//! `{prefix}:{id}:{key}` namespacing convention
//! (`RedisSessionStorage::session_key`). Reinventing any of that here is
//! exactly the duplication flagged in scope_store.rs's own module doc.
//!
//! "Stub" because: no retry/backoff policy, no connection pooling, and the
//! single-writer-vs-distributed-concurrency question (#902) is explicitly
//! unresolved -- this proves the reuse shape and the ScopeStore contract,
//! not a production-hardened backend. Real hardening is #902's territory,
//! not this commit's.

use crate::errors::{ScopeError, ScopeResult};
use crate::scope_store::{ScopeId, ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use b00t_c0re_lib::redis::{RedisComms, RedisConfig};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A ScopeStore backed by RedisComms, namespaced per scope.
pub struct RedisScopeStore {
    comms: RedisComms,
    id: ScopeId,
    parent: Option<ScopeId>,
}

impl RedisScopeStore {
    pub fn open(config: RedisConfig, id: ScopeId, parent: Option<ScopeId>) -> ScopeResult<Self> {
        let comms = RedisComms::new(config, format!("scope-store:{id}"))
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        Ok(Self { comms, id, parent })
    }

    /// True when the underlying Redis server actually answers PING.
    /// `RedisComms::new` never fails on its own (redis::Client::open is a
    /// lazy URL parse, not a connection attempt) -- this is the real
    /// "is the backend up" check, matching RedisComms::is_available.
    pub fn is_available(&self) -> bool {
        self.comms.is_available()
    }

    /// Namespaced key: `scope:{scope_id}:{key}`, mirroring
    /// `RedisSessionStorage::session_key`'s `{prefix}:{id}:{key}` shape
    /// rather than inventing a new convention.
    fn scoped_key(&self, key: &str) -> String {
        format!("scope:{}:{key}", self.id)
    }
}

impl ScopeStore for RedisScopeStore {
    fn get_raw(&self, key: &str) -> ScopeResult<Option<Value>> {
        let raw = self
            .comms
            .get(&self.scoped_key(key))
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        match raw {
            None => Ok(None),
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        }
    }

    fn set_raw(&mut self, key: &str, val: Value) -> ScopeResult<()> {
        let serialized = serde_json::to_string(&val)?;
        self.comms
            .set(&self.scoped_key(key), &serialized)
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        Ok(())
    }

    fn scope_id(&self) -> &ScopeId {
        &self.id
    }

    fn parent(&self) -> Option<&ScopeId> {
        self.parent.as_ref()
    }
}

/// Applies a `ScopeOp` batch atomically: CAS pre-check pass (aborts the
/// whole call on any mismatch, before any write), then an apply pass.
/// KEYS[i] pairs positionally with the i-th entry of the ARGV[1] JSON op
/// array; ARGV[2] is the caller's `now` as Unix seconds (avoids relying on
/// Lua-internal TIME for determinism across Redis/Valkey configurations).
const TRANSACTION_SCRIPT: &str = r#"
local ops = cjson.decode(ARGV[1])
local now = tonumber(ARGV[2])

for i, op in ipairs(ops) do
  if op.expect_gen ~= nil then
    local raw = redis.call('GET', KEYS[i])
    local current_gen = 0
    if raw then
      current_gen = cjson.decode(raw).gen
    end
    if current_gen ~= op.expect_gen then
      return cjson.encode({error = 'cas_mismatch', key = KEYS[i], expected = op.expect_gen, found = current_gen})
    end
  end
end

local results = {}
for i, op in ipairs(ops) do
  if op.op == 'get' then
    local raw = redis.call('GET', KEYS[i])
    if raw then
      local env = cjson.decode(raw)
      if env.expires_at ~= nil and env.expires_at <= now then
        results[i] = {type = 'value', gen = 0}
      else
        results[i] = {type = 'value', value = env.v, gen = env.gen}
      end
    else
      results[i] = {type = 'value', gen = 0}
    end
  elseif op.op == 'put' then
    local raw = redis.call('GET', KEYS[i])
    local current_gen = 0
    if raw then
      current_gen = cjson.decode(raw).gen
    end
    local new_gen = current_gen + 1
    local env = {v = op.value, gen = new_gen, expires_at = op.expires_at}
    redis.call('SET', KEYS[i], cjson.encode(env))
    if op.expires_at ~= nil then
      local ttl = math.floor(op.expires_at - now)
      if ttl > 0 then
        redis.call('EXPIRE', KEYS[i], ttl)
      end
    end
    results[i] = {type = 'written', gen = new_gen}
  elseif op.op == 'delete' then
    redis.call('DEL', KEYS[i])
    results[i] = {type = 'deleted'}
  end
end

return cjson.encode({ok = results})
"#;

#[derive(Serialize)]
struct LuaOp {
    op: &'static str,
    expect_gen: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
}

#[derive(Deserialize)]
struct LuaOpResult {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    value: Option<Value>,
    #[serde(rename = "gen", default)]
    generation: u64,
}

#[derive(Deserialize)]
struct LuaScriptOutput {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    expected: Option<u64>,
    #[serde(default)]
    found: Option<u64>,
    #[serde(default)]
    ok: Option<Vec<LuaOpResult>>,
}

impl TransactionalScopeStore for RedisScopeStore {
    fn transaction(&mut self, ops: Vec<ScopeOp>) -> ScopeResult<Vec<ScopeOpResult>> {
        let now = Utc::now();
        let mut keys = Vec::with_capacity(ops.len());
        let mut lua_ops = Vec::with_capacity(ops.len());

        for op in &ops {
            match op {
                ScopeOp::Get { key } => {
                    keys.push(self.scoped_key(key));
                    lua_ops.push(LuaOp { op: "get", expect_gen: None, value: None, expires_at: None });
                }
                ScopeOp::Put { key, value, expect_gen, expires_at } => {
                    keys.push(self.scoped_key(key));
                    lua_ops.push(LuaOp {
                        op: "put",
                        expect_gen: *expect_gen,
                        value: Some(value.clone()),
                        expires_at: expires_at.map(|e| e.timestamp()),
                    });
                }
                ScopeOp::Delete { key, expect_gen } => {
                    keys.push(self.scoped_key(key));
                    lua_ops.push(LuaOp { op: "delete", expect_gen: *expect_gen, value: None, expires_at: None });
                }
            }
        }

        let ops_json = serde_json::to_string(&lua_ops)?;
        let raw = self
            .comms
            .eval_script(TRANSACTION_SCRIPT, &keys, &[ops_json, now.timestamp().to_string()])
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        let output: LuaScriptOutput = serde_json::from_str(&raw)?;

        if let Some(err) = output.error {
            if err == "cas_mismatch" {
                return Err(ScopeError::WriteRejected(format!(
                    "CAS mismatch on {}: expected generation {:?}, found {:?}",
                    output.key.unwrap_or_default(),
                    output.expected,
                    output.found
                )));
            }
            return Err(ScopeError::BackendUnavailable(format!("transaction script error: {err}")));
        }

        let raw_results = output.ok.unwrap_or_default();
        let mut results = Vec::with_capacity(raw_results.len());
        for (i, r) in raw_results.into_iter().enumerate() {
            let result = match r.kind.as_str() {
                "value" => ScopeOpResult::Value { value: r.value, generation: r.generation },
                "written" => ScopeOpResult::Written { generation: r.generation },
                "deleted" => ScopeOpResult::Deleted,
                other => {
                    return Err(ScopeError::BackendUnavailable(format!(
                        "unrecognized transaction result kind '{other}' at index {i}"
                    )));
                }
            };
            results.push(result);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope_store::{ScopeOp, ScopeOpResult, TransactionalScopeStore};

    fn store() -> RedisScopeStore {
        RedisScopeStore::open(
            RedisConfig::default(),
            ScopeId::Repo("myrepo".into()),
            Some(ScopeId::Node("myhost".into())),
        )
        .unwrap()
    }

    #[test]
    fn open_never_fails_without_a_live_connection() {
        // RedisComms::new is a lazy client handle, not a connection
        // attempt -- open() must succeed even with no Redis reachable.
        let _store = store();
    }

    #[test]
    fn scoped_key_uses_session_storage_style_prefixing() {
        let s = store();
        assert_eq!(s.scoped_key("greeting"), "scope:repo:myrepo:greeting");
    }

    #[test]
    fn scope_identity_preserved() {
        let s = store();
        assert_eq!(s.scope_id(), &ScopeId::Repo("myrepo".into()));
        assert_eq!(s.parent(), Some(&ScopeId::Node("myhost".into())));
    }

    /// Live round-trip against a real Redis server -- gated on
    /// is_available() rather than assumed, since this sandbox has none
    /// installed. Runs for real wherever Redis is actually reachable
    /// (dev machines, CI with a redis service); documents the contract
    /// either way rather than silently omitting it.
    #[test]
    fn get_set_round_trips_when_redis_is_actually_available() {
        let mut s = store();
        if !s.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        s.set_raw("k", Value::String("v".into())).unwrap();
        assert_eq!(s.get_raw("k").unwrap(), Some(Value::String("v".into())));
    }

    #[test]
    fn transaction_put_then_get_round_trips_when_redis_is_actually_available() {
        let mut s = store();
        if !s.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        let results = s
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();
        assert_eq!(results, vec![ScopeOpResult::Written { generation: 1 }]);

        let results = s.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(
            results,
            vec![ScopeOpResult::Value { value: Some(Value::String("v1".into())), generation: 1 }]
        );
    }

    #[test]
    fn transaction_cas_mismatch_rejects_whole_batch_when_redis_is_actually_available() {
        let mut s = store();
        if !s.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        s.transaction(vec![ScopeOp::Put {
            key: "cas-test".into(),
            value: Value::String("v1".into()),
            expect_gen: None,
            expires_at: None,
        }])
        .unwrap();

        let err = s
            .transaction(vec![
                ScopeOp::Put {
                    key: "cas-test".into(),
                    value: Value::String("v2".into()),
                    expect_gen: Some(99),
                    expires_at: None,
                },
                ScopeOp::Put {
                    key: "cas-test-other".into(),
                    value: Value::String("should-not-land".into()),
                    expect_gen: None,
                    expires_at: None,
                },
            ])
            .unwrap_err();
        assert!(matches!(err, ScopeError::WriteRejected(_)));
        assert_eq!(s.get_raw("cas-test-other").unwrap(), None);
    }
}
