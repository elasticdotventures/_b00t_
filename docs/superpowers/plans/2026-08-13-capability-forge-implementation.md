# capability-forge Implementation Plan (Phase 1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the phase-1 capability-forge authorization service (NATS request/mint/revoke path, no HTTP dashboard) and prove it end-to-end against a real, ephemeral local `nats-server` — an agent enrolls, requests scoped access, connects to NATS with the minted JWT, is genuinely allowed on granted subjects and refused on non-granted ones, then loses access after revocation.

**Architecture:** A new Rust crate `capability-forge` in the `_b00t_` workspace, built directly on `b00t-c0re-gov::scope_store` (no IPC hop) for all persistent state. Agents hold an `nkeys`-based Ed25519 keypair that doubles as both their request-signing identity and their NATS user identity — one key, two uses, so the JWT capability-forge mints is immediately usable to connect. Requests arrive over NATS request-reply; a small policy engine partitions requested skills into base/escalatable/restricted tiers, calls an LLM judge for the escalatable ones (fails closed), persists a grant record before ever minting, and mints a real NATS user JWT via `nkeys`/`nats-jwt`. Revocation rewrites the account JWT's revocation map and reloads the local `nats-server`.

**Tech Stack:** Rust 2024, `async-nats` 0.45, `nkeys`, `nats-jwt`, `tokio`, `b00t-c0re-gov` (`ScopeStore`/`TransactionalScopeStore`/`RedbScopeStore`), `async-openai` 0.30 (real LLM judge only), `nats-server` binary (test-only, spawned as a child process).

## Global Constraints

- Design of record: `docs/superpowers/specs/2026-08-13-capability-forge-design.md` in the `infrastructure` repo. Every task below implements a specific section of it; do not add behavior the spec doesn't call for.
- Phase 1 only (per the spec's "Phasing" section): no HTTP/pingap dashboard, no deployment to the live Vultr node, no live production NATS operator/account claims. All NATS interaction in this plan is against an ephemeral local `nats-server` spawned per test run.
- LLM judge **fails closed**: any timeout/error/malformed response from the judge is a deny, never a grant. This applies in the real `OpenAiJudge` impl; the default test suite never exercises it directly (uses `FakeJudge`).
- A JWT is never returned to a caller without a durable grant record persisted first (spec's "Request flow" step 5-before-6).
- Escalatable-tier grants are grant-scoped only — never written to an agent's allowlist. Restricted-tier skills are never LLM-escalatable under any circumstance.
- No comments explaining *what* code does; only comments for non-obvious *why* (matches this repo's existing style, e.g. `scope_store.rs`'s module doc).
- `_b00t_` is a workspace of 30+ members with many git submodules — this worktree (`~/.b00t/.worktrees/task-capability-forge`, branch `task/capability-forge`) already has submodules initialized. Every `cargo` command in this plan should be run with `/home/brianh/.cargo/bin/cargo` from inside that worktree directory (the toolchain was freshly repaired via `rustup-init` and is not yet guaranteed to be on every shell's `PATH`).

---

## File Structure

```
capability-forge/
  Cargo.toml
  src/
    lib.rs           # pub mod declarations only
    identity.rs       # AgentKeyPair: nkeys-based generate/sign/verify
    request.rs         # CapabilityRequest, CapabilityReply, SignedRequest (sign/verify envelope)
    tiers.rs             # SkillTier, per-agent pubkey/allowlist/suspended, skill-tier registry (ScopeStore-backed)
    enroll.rs              # enroll_agent, suspend_agent, set_skill_tier (built on tiers.rs)
    grant.rs                 # Grant record, jti, persistence + revoked-set (ScopeStore-backed, transactional)
    judge.rs                   # EscalationJudge trait, FakeJudge, OpenAiJudge
    jwt_mint.rs                  # NATS operator/account/user JWT construction (nkeys + nats-jwt)
    service.rs                     # CapabilityForge::handle_request — the policy engine
    bin/
      main.rs                       # NATS subscriber binary wiring service.rs to async-nats
  tests/
    e2e_local_nats.rs                # the end-to-end integration test (ephemeral nats-server)
```

- `capability-forge/Cargo.toml` — new crate manifest.
- `Cargo.toml` (workspace root) — add `"capability-forge"` to `members`.
- `b00t-cli/src/commands/capability_forge.rs` — new thin admin subcommand (Task 10).
- `b00t-cli/src/main.rs` — wire the new subcommand in (Task 10).

---

### Task 1: Crate scaffold + identity module

**Files:**
- Modify: `/home/brianh/.b00t/.worktrees/task-capability-forge/Cargo.toml` (workspace `members`)
- Create: `capability-forge/Cargo.toml`
- Create: `capability-forge/src/lib.rs`
- Create: `capability-forge/src/identity.rs`
- Test: inline `#[cfg(test)]` in `capability-forge/src/identity.rs`

**Interfaces:**
- Produces: `capability_forge::identity::AgentKeyPair` with `AgentKeyPair::generate() -> Self`, `AgentKeyPair::from_seed(seed: &str) -> anyhow::Result<Self>`, `.public_key(&self) -> String`, `.seed(&self) -> anyhow::Result<String>`, `.sign(&self, data: &[u8]) -> anyhow::Result<Vec<u8>>`, `verify(public_key: &str, data: &[u8], sig: &[u8]) -> anyhow::Result<()>` (free function).

- [ ] **Step 1: Add the workspace member**

Edit `/home/brianh/.b00t/.worktrees/task-capability-forge/Cargo.toml`, in the `members = [...]` array, add a new line:

```toml
    "capability-forge",
```

(Anywhere in the list; alphabetical-ish placement near `"b00t-c0re-gov"` is consistent with the existing ordering but not required.)

- [ ] **Step 2: Create the crate manifest**

Create `capability-forge/Cargo.toml`:

```toml
[package]
name = "capability-forge"
version.workspace = true
edition = "2024"
description = "Skill-scoped NATS JWT authorization service — mint/revoke, three-tier policy, LLM-judged escalation"
license.workspace = true
authors.workspace = true
repository.workspace = true

[dependencies]
b00t-c0re-gov = { path = "../b00t-c0re-gov" }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["full"] }
chrono = { workspace = true, features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "2"
anyhow = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
async-nats = "0.45"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Add the new nkeys/nats-jwt/async-openai/base64 dependencies with real resolved versions**

Run from the worktree root:

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo add nkeys nats-jwt base64 --manifest-path capability-forge/Cargo.toml
/home/brianh/.cargo/bin/cargo add async-openai@0.30.1 --manifest-path capability-forge/Cargo.toml
```

This resolves and pins real crates.io versions rather than guessing them by hand.

- [ ] **Step 4: Create `src/lib.rs`**

```rust
pub mod identity;
```

(Later tasks append one `pub mod` line each — do not pre-declare modules that don't exist yet, `cargo check` will fail on them.)

- [ ] **Step 5: Write the failing test**

Create `capability-forge/src/identity.rs`:

```rust
use anyhow::{Context, Result};
use nkeys::KeyPair;

pub struct AgentKeyPair(KeyPair);

impl AgentKeyPair {
    pub fn generate() -> Self {
        Self(KeyPair::new_user())
    }

    pub fn from_seed(seed: &str) -> Result<Self> {
        Ok(Self(KeyPair::from_seed(seed).context("invalid nkeys seed")?))
    }

    pub fn public_key(&self) -> String {
        self.0.public_key()
    }

    pub fn seed(&self) -> Result<String> {
        self.0.seed().context("keypair has no seed (public-key-only instance)")
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.0.sign(data).context("signing failed")
    }
}

pub fn verify(public_key: &str, data: &[u8], sig: &[u8]) -> Result<()> {
    let kp = KeyPair::from_public_key(public_key).context("invalid nkeys public key")?;
    kp.verify(data, sig).context("signature verification failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trips() {
        let kp = AgentKeyPair::generate();
        let sig = kp.sign(b"hello capability-forge").unwrap();
        verify(&kp.public_key(), b"hello capability-forge", &sig).unwrap();
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let kp = AgentKeyPair::generate();
        let sig = kp.sign(b"hello").unwrap();
        assert!(verify(&kp.public_key(), b"goodbye", &sig).is_err());
    }

    #[test]
    fn from_seed_reconstructs_same_public_key() {
        let kp = AgentKeyPair::generate();
        let seed = kp.seed().unwrap();
        let restored = AgentKeyPair::from_seed(&seed).unwrap();
        assert_eq!(kp.public_key(), restored.public_key());
    }
}
```

- [ ] **Step 6: Run the tests**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge identity:: -- --nocapture
```

Expected: 3 tests pass. If `nkeys::KeyPair`'s method names differ from above (check with `/home/brianh/.cargo/bin/cargo doc -p nkeys --no-deps --open` or docs.rs/nkeys if compile errors point at a mismatch), adjust `identity.rs` to the real API — the round-trip behavior (generate → sign → verify; tamper → reject; seed → reconstruct same pubkey) is the actual requirement, not the exact method spelling.

- [ ] **Step 7: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add Cargo.toml capability-forge/Cargo.toml capability-forge/src/lib.rs capability-forge/src/identity.rs
git commit -m "feat(capability-forge): scaffold crate + nkeys-based agent identity"
```

---

### Task 2: Signed request/reply wire types

**Files:**
- Create: `capability-forge/src/request.rs`
- Modify: `capability-forge/src/lib.rs` (add `pub mod request;`)
- Test: inline `#[cfg(test)]` in `request.rs`

**Interfaces:**
- Consumes: `identity::AgentKeyPair` (`.sign`, `.public_key`), `identity::verify`.
- Produces: `CapabilityRequest { agent_id: String, requested_skills: Vec<String>, justification: String }`, `SignedRequest { body: CapabilityRequest, agent_pubkey: String, signature: Vec<u8> }` with `SignedRequest::sign(kp: &AgentKeyPair, body: CapabilityRequest) -> Self` and `.verify(&self) -> anyhow::Result<()>`, `CapabilityReply { granted: Vec<String>, denied: Vec<(String, String)>, jwt: Option<String>, expires_at: Option<chrono::DateTime<chrono::Utc>> }`.

- [ ] **Step 1: Write the failing test**

Create `capability-forge/src/request.rs`:

```rust
use crate::identity::{self, AgentKeyPair};
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub agent_id: String,
    pub requested_skills: Vec<String>,
    pub justification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRequest {
    pub body: CapabilityRequest,
    pub agent_pubkey: String,
    pub signature: Vec<u8>,
}

impl SignedRequest {
    pub fn sign(kp: &AgentKeyPair, body: CapabilityRequest) -> Result<Self> {
        let bytes = serde_json::to_vec(&body)?;
        let signature = kp.sign(&bytes)?;
        Ok(Self { body, agent_pubkey: kp.public_key(), signature })
    }

    pub fn verify(&self) -> Result<()> {
        let bytes = serde_json::to_vec(&self.body)?;
        identity::verify(&self.agent_pubkey, &bytes, &self.signature)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityReply {
    pub granted: Vec<String>,
    pub denied: Vec<(String, String)>,
    pub jwt: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> CapabilityRequest {
        CapabilityRequest {
            agent_id: "agent-1".into(),
            requested_skills: vec!["skill.read".into()],
            justification: "testing".into(),
        }
    }

    #[test]
    fn signed_request_round_trips_through_json_and_verifies() {
        let kp = AgentKeyPair::generate();
        let signed = SignedRequest::sign(&kp, body()).unwrap();
        let wire = serde_json::to_vec(&signed).unwrap();
        let back: SignedRequest = serde_json::from_slice(&wire).unwrap();
        back.verify().unwrap();
    }

    #[test]
    fn tampered_body_fails_verification() {
        let kp = AgentKeyPair::generate();
        let mut signed = SignedRequest::sign(&kp, body()).unwrap();
        signed.body.requested_skills.push("skill.admin".into());
        assert!(signed.verify().is_err());
    }

    #[test]
    fn wrong_signer_pubkey_fails_verification() {
        let kp = AgentKeyPair::generate();
        let other = AgentKeyPair::generate();
        let mut signed = SignedRequest::sign(&kp, body()).unwrap();
        signed.agent_pubkey = other.public_key();
        assert!(signed.verify().is_err());
    }
}
```

- [ ] **Step 2: Wire the module in**

In `capability-forge/src/lib.rs`, add:

```rust
pub mod request;
```

- [ ] **Step 3: Run the tests**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge request:: -- --nocapture
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/src/request.rs capability-forge/src/lib.rs
git commit -m "feat(capability-forge): signed request/reply wire types"
```

---

### Task 3: ScopeStore-backed tier registry and agent records

**Files:**
- Create: `capability-forge/src/tiers.rs`
- Modify: `capability-forge/src/lib.rs` (add `pub mod tiers;`)
- Test: inline `#[cfg(test)]` in `tiers.rs`, using `tempfile::tempdir()` + `RedbScopeStore`

**Interfaces:**
- Consumes: `b00t_c0re_gov::scope_store::{ScopeStore, ScopeId}`, `b00t_c0re_gov::redb_scope_store::RedbScopeStore`.
- Produces: `SkillTier { Base, Escalatable, Restricted }` (serde, `Default = Escalatable` per spec — "defaulting to escalatable for any skill not explicitly classified"), and on a `dyn ScopeStore`: `get_skill_tier`, `set_skill_tier`, `get_agent_pubkey`, `set_agent_pubkey`, `get_agent_allowlist`, `add_to_allowlist`, `is_agent_suspended`, `set_agent_suspended`, `get_skill_description`, `set_skill_description` — all free functions taking `&mut dyn ScopeStore` (or `&dyn ScopeStore` for reads) so `service.rs` doesn't need a generic parameter everywhere.

- [ ] **Step 1: Write the failing test**

Create `capability-forge/src/tiers.rs`:

```rust
use anyhow::Result;
use b00t_c0re_gov::scope_store::ScopeStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SkillTier {
    Base,
    #[default]
    Escalatable,
    Restricted,
}

fn skill_tier_key(skill: &str) -> String {
    format!("capforge:skill:{skill}:tier")
}

fn skill_description_key(skill: &str) -> String {
    format!("capforge:skill:{skill}:description")
}

fn agent_pubkey_key(agent_id: &str) -> String {
    format!("capforge:agent:{agent_id}:pubkey")
}

fn agent_allowlist_key(agent_id: &str) -> String {
    format!("capforge:agent:{agent_id}:allowlist")
}

fn agent_suspended_key(agent_id: &str) -> String {
    format!("capforge:agent:{agent_id}:suspended")
}

pub fn get_skill_tier(store: &dyn ScopeStore, skill: &str) -> Result<SkillTier> {
    match store.get_raw(&skill_tier_key(skill))? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(SkillTier::default()),
    }
}

pub fn set_skill_tier(store: &mut dyn ScopeStore, skill: &str, tier: SkillTier) -> Result<()> {
    Ok(store.set_raw(&skill_tier_key(skill), serde_json::to_value(tier)?)?)
}

pub fn get_skill_description(store: &dyn ScopeStore, skill: &str) -> Result<String> {
    match store.get_raw(&skill_description_key(skill))? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(String::new()),
    }
}

pub fn set_skill_description(store: &mut dyn ScopeStore, skill: &str, description: &str) -> Result<()> {
    Ok(store.set_raw(&skill_description_key(skill), serde_json::to_value(description)?)?)
}

pub fn get_agent_pubkey(store: &dyn ScopeStore, agent_id: &str) -> Result<Option<String>> {
    match store.get_raw(&agent_pubkey_key(agent_id))? {
        Some(v) => Ok(Some(serde_json::from_value(v)?)),
        None => Ok(None),
    }
}

pub fn set_agent_pubkey(store: &mut dyn ScopeStore, agent_id: &str, pubkey: &str) -> Result<()> {
    Ok(store.set_raw(&agent_pubkey_key(agent_id), serde_json::to_value(pubkey)?)?)
}

pub fn get_agent_allowlist(store: &dyn ScopeStore, agent_id: &str) -> Result<Vec<String>> {
    match store.get_raw(&agent_allowlist_key(agent_id))? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(Vec::new()),
    }
}

pub fn add_to_allowlist(store: &mut dyn ScopeStore, agent_id: &str, skills: &[String]) -> Result<()> {
    let mut current = get_agent_allowlist(store, agent_id)?;
    for skill in skills {
        if !current.contains(skill) {
            current.push(skill.clone());
        }
    }
    Ok(store.set_raw(&agent_allowlist_key(agent_id), serde_json::to_value(current)?)?)
}

pub fn is_agent_suspended(store: &dyn ScopeStore, agent_id: &str) -> Result<bool> {
    match store.get_raw(&agent_suspended_key(agent_id))? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(false),
    }
}

pub fn set_agent_suspended(store: &mut dyn ScopeStore, agent_id: &str, suspended: bool) -> Result<()> {
    Ok(store.set_raw(&agent_suspended_key(agent_id), serde_json::to_value(suspended)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;

    fn store() -> RedbScopeStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        RedbScopeStore::open(path, ScopeId::Global, None).unwrap()
    }

    #[test]
    fn unknown_skill_defaults_to_escalatable() {
        let s = store();
        assert_eq!(get_skill_tier(&s, "skill.nonexistent").unwrap(), SkillTier::Escalatable);
    }

    #[test]
    fn skill_tier_round_trips() {
        let mut s = store();
        set_skill_tier(&mut s, "skill.delete-infra", SkillTier::Restricted).unwrap();
        assert_eq!(get_skill_tier(&s, "skill.delete-infra").unwrap(), SkillTier::Restricted);
    }

    #[test]
    fn allowlist_accumulates_without_duplicates() {
        let mut s = store();
        add_to_allowlist(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        add_to_allowlist(&mut s, "agent-1", &["skill.read".into(), "skill.write".into()]).unwrap();
        let list = get_agent_allowlist(&s, "agent-1").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"skill.read".to_string()));
        assert!(list.contains(&"skill.write".to_string()));
    }

    #[test]
    fn suspended_defaults_false_and_round_trips() {
        let mut s = store();
        assert!(!is_agent_suspended(&s, "agent-1").unwrap());
        set_agent_suspended(&mut s, "agent-1", true).unwrap();
        assert!(is_agent_suspended(&s, "agent-1").unwrap());
    }

    #[test]
    fn agent_pubkey_round_trips() {
        let mut s = store();
        assert!(get_agent_pubkey(&s, "agent-1").unwrap().is_none());
        set_agent_pubkey(&mut s, "agent-1", "UABC123").unwrap();
        assert_eq!(get_agent_pubkey(&s, "agent-1").unwrap().unwrap(), "UABC123");
    }
}
```

`std::mem::forget(dir)` deliberately leaks the `TempDir` past the test's scope: `RedbScopeStore::open` keeps the file open for the store's lifetime, and dropping the `TempDir` guard would delete the directory out from under it mid-test. This is test-only; `/tmp` cleanup on the CI/dev box handles the leak.

- [ ] **Step 2: Wire the module in**

In `capability-forge/src/lib.rs`, add:

```rust
pub mod tiers;
```

- [ ] **Step 3: Run the tests**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge tiers:: -- --nocapture
```

Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/src/tiers.rs capability-forge/src/lib.rs
git commit -m "feat(capability-forge): ScopeStore-backed tier registry and agent records"
```

---

### Task 4: Enrollment functions

**Files:**
- Create: `capability-forge/src/enroll.rs`
- Modify: `capability-forge/src/lib.rs` (add `pub mod enroll;`)
- Test: inline `#[cfg(test)]` in `enroll.rs`

**Interfaces:**
- Consumes: `tiers::{set_agent_pubkey, add_to_allowlist, set_agent_suspended, get_agent_allowlist}`, `identity::AgentKeyPair`.
- Produces: `enroll_agent(store: &mut dyn ScopeStore, agent_id: &str, base_skills: &[String]) -> Result<AgentKeyPair>` (generates the keypair, registers pubkey + base allowlist, returns the keypair so the caller can hand the seed to the agent), `suspend_agent(store: &mut dyn ScopeStore, agent_id: &str) -> Result<()>`, `grant_base_skill(store: &mut dyn ScopeStore, agent_id: &str, skill: &str) -> Result<()>` (the "admin adds a skill to the allowlist later" path, including moving a restricted-tier skill into an agent's reach per the spec).

- [ ] **Step 1: Write the failing test**

Create `capability-forge/src/enroll.rs`:

```rust
use crate::identity::AgentKeyPair;
use crate::tiers::{add_to_allowlist, get_agent_allowlist, set_agent_pubkey, set_agent_suspended};
use anyhow::Result;
use b00t_c0re_gov::scope_store::ScopeStore;

pub fn enroll_agent(
    store: &mut dyn ScopeStore,
    agent_id: &str,
    base_skills: &[String],
) -> Result<AgentKeyPair> {
    let kp = AgentKeyPair::generate();
    set_agent_pubkey(store, agent_id, &kp.public_key())?;
    add_to_allowlist(store, agent_id, base_skills)?;
    Ok(kp)
}

pub fn suspend_agent(store: &mut dyn ScopeStore, agent_id: &str) -> Result<()> {
    set_agent_suspended(store, agent_id, true)
}

pub fn grant_base_skill(store: &mut dyn ScopeStore, agent_id: &str, skill: &str) -> Result<()> {
    add_to_allowlist(store, agent_id, std::slice::from_ref(&skill.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity;
    use crate::tiers::is_agent_suspended;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;

    fn store() -> RedbScopeStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        RedbScopeStore::open(path, ScopeId::Global, None).unwrap()
    }

    #[test]
    fn enroll_registers_pubkey_and_allowlist_and_key_is_usable() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        assert_eq!(get_agent_allowlist(&s, "agent-1").unwrap(), vec!["skill.read".to_string()]);
        let sig = kp.sign(b"proof").unwrap();
        identity::verify(&kp.public_key(), b"proof", &sig).unwrap();
    }

    #[test]
    fn suspend_sets_flag() {
        let mut s = store();
        enroll_agent(&mut s, "agent-1", &[]).unwrap();
        assert!(!is_agent_suspended(&s, "agent-1").unwrap());
        suspend_agent(&mut s, "agent-1").unwrap();
        assert!(is_agent_suspended(&s, "agent-1").unwrap());
    }

    #[test]
    fn grant_base_skill_appends_to_existing_allowlist() {
        let mut s = store();
        enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        grant_base_skill(&mut s, "agent-1", "skill.delete-infra").unwrap();
        let list = get_agent_allowlist(&s, "agent-1").unwrap();
        assert!(list.contains(&"skill.read".to_string()));
        assert!(list.contains(&"skill.delete-infra".to_string()));
    }
}
```

- [ ] **Step 2: Wire the module in**

In `capability-forge/src/lib.rs`, add:

```rust
pub mod enroll;
```

- [ ] **Step 3: Run the tests**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge enroll:: -- --nocapture
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/src/enroll.rs capability-forge/src/lib.rs
git commit -m "feat(capability-forge): enrollment, suspension, and allowlist-grant functions"
```

---

### Task 5: Grant records and revocation

**Files:**
- Create: `capability-forge/src/grant.rs`
- Modify: `capability-forge/src/lib.rs` (add `pub mod grant;`)
- Test: inline `#[cfg(test)]` in `grant.rs`, using `b00t_c0re_gov::scope_store::{TransactionalScopeStore, ScopeOp, ScopeOpResult}`

**Interfaces:**
- Consumes: `TransactionalScopeStore::transaction`, `tiers::SkillTier`.
- Produces: `Grant { jti: String, agent_id: String, skills: Vec<String>, tier_source: std::collections::HashMap<String, SkillTier>, issued_at: DateTime<Utc>, expires_at: DateTime<Utc> }`, `persist_grant(store: &mut dyn TransactionalScopeStore, grant: &Grant) -> Result<()>` (atomically writes the grant record — a single key today, but done via `transaction()` so it's ready if a second key joins it later), `revoke_grant(store: &mut dyn TransactionalScopeStore, jti: &str) -> Result<()>`, `is_revoked(store: &dyn ScopeStore, jti: &str) -> Result<bool>`.

- [ ] **Step 1: Write the failing test**

Create `capability-forge/src/grant.rs`:

```rust
use crate::tiers::SkillTier;
use anyhow::{bail, Result};
use b00t_c0re_gov::scope_store::{ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub jti: String,
    pub agent_id: String,
    pub skills: Vec<String>,
    pub tier_source: HashMap<String, SkillTier>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl Grant {
    pub fn new(
        agent_id: &str,
        skills: Vec<String>,
        tier_source: HashMap<String, SkillTier>,
        ttl: chrono::Duration,
    ) -> Self {
        let now = Utc::now();
        Self {
            jti: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            skills,
            tier_source,
            issued_at: now,
            expires_at: now + ttl,
        }
    }
}

fn grant_key(jti: &str) -> String {
    format!("capforge:grant:{jti}")
}

const REVOKED_KEY: &str = "capforge:revoked";

pub fn persist_grant(store: &mut dyn TransactionalScopeStore, grant: &Grant) -> Result<()> {
    let results = store.transaction(vec![ScopeOp::Put {
        key: grant_key(&grant.jti),
        value: serde_json::to_value(grant)?,
        expect_gen: None,
        expires_at: Some(grant.expires_at),
    }])?;
    match results.as_slice() {
        [ScopeOpResult::Written { .. }] => Ok(()),
        other => bail!("unexpected transaction result persisting grant: {other:?}"),
    }
}

fn get_revoked_set(store: &dyn ScopeStore) -> Result<Vec<String>> {
    match store.get_raw(REVOKED_KEY)? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(Vec::new()),
    }
}

pub fn revoke_grant(store: &mut dyn TransactionalScopeStore, jti: &str) -> Result<()> {
    let mut revoked = get_revoked_set(store)?;
    if !revoked.contains(&jti.to_string()) {
        revoked.push(jti.to_string());
    }
    let results = store.transaction(vec![ScopeOp::Put {
        key: REVOKED_KEY.to_string(),
        value: serde_json::to_value(revoked)?,
        expect_gen: None,
        expires_at: None,
    }])?;
    match results.as_slice() {
        [ScopeOpResult::Written { .. }] => Ok(()),
        other => bail!("unexpected transaction result revoking grant: {other:?}"),
    }
}

pub fn is_revoked(store: &dyn ScopeStore, jti: &str) -> Result<bool> {
    Ok(get_revoked_set(store)?.contains(&jti.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;

    fn store() -> RedbScopeStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        RedbScopeStore::open(path, ScopeId::Global, None).unwrap()
    }

    #[test]
    fn persisted_grant_is_not_revoked_by_default() {
        let mut s = store();
        let grant = Grant::new("agent-1", vec!["skill.read".into()], HashMap::new(), chrono::Duration::minutes(30));
        persist_grant(&mut s, &grant).unwrap();
        assert!(!is_revoked(&s, &grant.jti).unwrap());
    }

    #[test]
    fn revoke_marks_jti_revoked_without_disturbing_others() {
        let mut s = store();
        let a = Grant::new("agent-1", vec!["skill.read".into()], HashMap::new(), chrono::Duration::minutes(30));
        let b = Grant::new("agent-1", vec!["skill.write".into()], HashMap::new(), chrono::Duration::minutes(30));
        persist_grant(&mut s, &a).unwrap();
        persist_grant(&mut s, &b).unwrap();
        revoke_grant(&mut s, &a.jti).unwrap();
        assert!(is_revoked(&s, &a.jti).unwrap());
        assert!(!is_revoked(&s, &b.jti).unwrap());
    }
}
```

- [ ] **Step 2: Wire the module in**

In `capability-forge/src/lib.rs`, add:

```rust
pub mod grant;
```

- [ ] **Step 3: Run the tests**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge grant:: -- --nocapture
```

Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/src/grant.rs capability-forge/src/lib.rs
git commit -m "feat(capability-forge): grant records and explicit revocation via TransactionalScopeStore"
```

---

### Task 6: Escalation judge (fake + real, fail-closed)

**Files:**
- Create: `capability-forge/src/judge.rs`
- Modify: `capability-forge/src/lib.rs` (add `pub mod judge;`)
- Test: inline `#[cfg(test)]` in `judge.rs`

**Interfaces:**
- Produces: `#[async_trait] pub trait EscalationJudge: Send + Sync { async fn judge(&self, agent_id: &str, skill: &str, skill_description: &str, justification: &str) -> JudgeOutcome; }`, `JudgeOutcome { Granted, Denied { reason: String } }` (note: no `Err` variant reaches callers — timeouts/API errors are converted to `Denied` *inside* the judge implementation, so `service.rs` in Task 8 has no fail-open code path to accidentally write), `FakeJudge` (constructed with a fixed decision or a closure, for tests), `OpenAiJudge` (real, `async-openai`-backed, reads `OPENAI_API_KEY` from env via the client's default config).

- [ ] **Step 1: Write the failing test**

Create `capability-forge/src/judge.rs`:

```rust
use async_openai::{types::CreateChatCompletionRequestArgs, Client};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgeOutcome {
    Granted,
    Denied { reason: String },
}

#[async_trait]
pub trait EscalationJudge: Send + Sync {
    async fn judge(
        &self,
        agent_id: &str,
        skill: &str,
        skill_description: &str,
        justification: &str,
    ) -> JudgeOutcome;
}

pub struct FakeJudge {
    pub decision: JudgeOutcome,
}

impl FakeJudge {
    pub fn always_grant() -> Self {
        Self { decision: JudgeOutcome::Granted }
    }

    pub fn always_deny(reason: &str) -> Self {
        Self { decision: JudgeOutcome::Denied { reason: reason.to_string() } }
    }
}

#[async_trait]
impl EscalationJudge for FakeJudge {
    async fn judge(&self, _agent_id: &str, _skill: &str, _skill_description: &str, _justification: &str) -> JudgeOutcome {
        self.decision.clone()
    }
}

#[derive(Deserialize)]
struct JudgeResponse {
    granted: bool,
    reason: String,
}

pub struct OpenAiJudge {
    client: Client<async_openai::config::OpenAIConfig>,
    model: String,
    timeout: Duration,
}

impl OpenAiJudge {
    pub fn new(model: impl Into<String>) -> Self {
        Self { client: Client::new(), model: model.into(), timeout: Duration::from_secs(15) }
    }
}

#[async_trait]
impl EscalationJudge for OpenAiJudge {
    async fn judge(&self, agent_id: &str, skill: &str, skill_description: &str, justification: &str) -> JudgeOutcome {
        let prompt = format!(
            "Agent '{agent_id}' requests skill '{skill}' ({skill_description}). \
             Justification: {justification}. \
             Reply with ONLY a JSON object: {{\"granted\": bool, \"reason\": string}}."
        );

        let request = match CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([async_openai::types::ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()
                .expect("static message construction cannot fail")
                .into()])
            .build()
        {
            Ok(r) => r,
            Err(e) => return JudgeOutcome::Denied { reason: format!("request build failed: {e}") },
        };

        let call = self.client.chat().create(request);
        let response = match tokio::time::timeout(self.timeout, call).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return JudgeOutcome::Denied { reason: format!("llm call failed: {e}") },
            Err(_) => return JudgeOutcome::Denied { reason: "llm call timed out".into() },
        };

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        match serde_json::from_str::<JudgeResponse>(&content) {
            Ok(parsed) if parsed.granted => JudgeOutcome::Granted,
            Ok(parsed) => JudgeOutcome::Denied { reason: parsed.reason },
            Err(e) => JudgeOutcome::Denied { reason: format!("malformed llm response: {e}") },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_judge_always_grant_grants() {
        let j = FakeJudge::always_grant();
        assert_eq!(j.judge("a", "s", "d", "j").await, JudgeOutcome::Granted);
    }

    #[tokio::test]
    async fn fake_judge_always_deny_denies_with_reason() {
        let j = FakeJudge::always_deny("no");
        assert_eq!(j.judge("a", "s", "d", "j").await, JudgeOutcome::Denied { reason: "no".into() });
    }
}
```

- [ ] **Step 2: Wire the module in**

In `capability-forge/src/lib.rs`, add:

```rust
pub mod judge;
```

- [ ] **Step 3: Run the tests**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge judge:: -- --nocapture
```

Expected: 2 tests pass. `OpenAiJudge` is not exercised by any test in this task (per the spec: real LLM calls are a separate manual-run suite, not the default one) — if `async-openai`'s exact type names in the code above (`CreateChatCompletionRequestArgs`, `ChatCompletionRequestUserMessageArgs`, `response.choices[0].message.content`) don't match what `cargo` resolves for `0.30.1`, fix against the compiler's actual error; the shape (build request → call with timeout → parse strict JSON → fail closed on any error) is the real requirement.

- [ ] **Step 4: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/src/judge.rs capability-forge/src/lib.rs
git commit -m "feat(capability-forge): escalation judge trait, fake test double, fail-closed OpenAI impl"
```

---

### Task 7: NATS user JWT minting

**Files:**
- Create: `capability-forge/src/jwt_mint.rs`
- Modify: `capability-forge/src/lib.rs` (add `pub mod jwt_mint;`)
- Test: inline `#[cfg(test)]` in `jwt_mint.rs`

**Interfaces:**
- Produces: `mint_user_jwt(account_signing_key: &nkeys::KeyPair, account_pubkey: &str, agent_pubkey: &str, granted_skills: &[String], ttl: chrono::Duration) -> anyhow::Result<String>` — the granted skill names become NATS publish/subscribe subject permissions of the form `capforge.{agent_pubkey}.{skill}` (a per-agent, per-skill subject namespace — this is the concrete "subjects" the spec's tier table refers to). Also produces `pub fn skill_subject(agent_pubkey: &str, skill: &str) -> String` (used identically by the minter and by anything checking what subject a skill maps to, e.g. the end-to-end test).

- [ ] **Step 1: Write the failing test**

Create `capability-forge/src/jwt_mint.rs`:

```rust
use anyhow::{Context, Result};
use nats_jwt::Token;
use nkeys::KeyPair;

pub fn skill_subject(agent_pubkey: &str, skill: &str) -> String {
    format!("capforge.{agent_pubkey}.{skill}")
}

pub fn mint_user_jwt(
    account_signing_key: &KeyPair,
    account_pubkey: &str,
    agent_pubkey: &str,
    granted_skills: &[String],
    ttl: chrono::Duration,
) -> Result<String> {
    let subjects: Vec<String> = granted_skills
        .iter()
        .map(|skill| skill_subject(agent_pubkey, skill))
        .collect();

    let mut token = Token::new_user(account_pubkey, agent_pubkey);
    for subject in &subjects {
        token.add_pub_permission(subject);
        token.add_sub_permission(subject);
    }
    token.set_expires_in(ttl.to_std().context("negative ttl")?);

    token.sign(account_signing_key).context("signing user jwt failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_subject_is_namespaced_per_agent() {
        assert_eq!(skill_subject("UABC", "skill.read"), "capforge.UABC.skill.read");
    }

    #[test]
    fn mint_produces_a_jwt_with_three_segments() {
        let account = KeyPair::new_account();
        let agent = KeyPair::new_user();
        let jwt = mint_user_jwt(
            &account,
            &account.public_key(),
            &agent.public_key(),
            &["skill.read".to_string()],
            chrono::Duration::minutes(30),
        )
        .unwrap();
        assert_eq!(jwt.split('.').count(), 3);
    }
}
```

- [ ] **Step 2: Wire the module in**

In `capability-forge/src/lib.rs`, add:

```rust
pub mod jwt_mint;
```

- [ ] **Step 3: Run the tests**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge jwt_mint:: -- --nocapture
```

Expected: 2 tests pass. `nats-jwt`'s `Token` builder method names (`new_user`, `add_pub_permission`, `add_sub_permission`, `set_expires_in`, `sign`) are the plan's best-effort match to its documented API (docs.rs/nats-jwt) — check `/home/brianh/.cargo/bin/cargo doc -p nats-jwt --no-deps --open` against any compile error and adjust to the real builder surface; a JWT that decodes to three base64url segments with the requested subjects permissioned is the actual requirement.

- [ ] **Step 4: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/src/jwt_mint.rs capability-forge/src/lib.rs
git commit -m "feat(capability-forge): mint NATS user JWTs scoped to per-agent skill subjects"
```

---

### Task 8: Policy engine (`service.rs`)

**Files:**
- Create: `capability-forge/src/service.rs`
- Modify: `capability-forge/src/lib.rs` (add `pub mod service;`)
- Test: inline `#[cfg(test)]` in `service.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–7 (`identity::verify`, `request::{SignedRequest, CapabilityReply}`, `tiers::*`, `grant::{Grant, persist_grant, revoke_grant, is_revoked}`, `judge::{EscalationJudge, JudgeOutcome}`, `jwt_mint::mint_user_jwt`).
- Produces: `pub struct CapabilityForge<'a> { pub store: &'a mut dyn TransactionalScopeStore, pub judge: &'a dyn EscalationJudge, pub account_signing_key: &'a nkeys::KeyPair, pub account_pubkey: &'a str, pub grant_ttl: chrono::Duration }` with `async fn handle_request(&mut self, signed: SignedRequest) -> CapabilityReply` — the exact orchestration the spec's "Request flow" section describes, steps 1–7.

- [ ] **Step 1: Write the failing test**

Create `capability-forge/src/service.rs`:

```rust
use crate::grant::{persist_grant, Grant};
use crate::judge::{EscalationJudge, JudgeOutcome};
use crate::request::{CapabilityReply, SignedRequest};
use crate::tiers::{get_agent_allowlist, get_agent_pubkey, get_skill_description, get_skill_tier, is_agent_suspended, SkillTier};
use crate::{identity, jwt_mint};
use b00t_c0re_gov::scope_store::TransactionalScopeStore;
use chrono::Duration;
use nkeys::KeyPair;
use std::collections::HashMap;

pub struct CapabilityForge<'a> {
    pub store: &'a mut dyn TransactionalScopeStore,
    pub judge: &'a dyn EscalationJudge,
    pub account_signing_key: &'a KeyPair,
    pub account_pubkey: &'a str,
    pub grant_ttl: Duration,
}

impl<'a> CapabilityForge<'a> {
    pub async fn handle_request(&mut self, signed: SignedRequest) -> CapabilityReply {
        let deny_all = |reason: &str, skills: &[String]| CapabilityReply {
            granted: vec![],
            denied: skills.iter().map(|s| (s.clone(), reason.to_string())).collect(),
            jwt: None,
            expires_at: None,
        };

        if signed.verify().is_err() {
            return deny_all("invalid signature", &signed.body.requested_skills);
        }

        let agent_id = &signed.body.agent_id;

        let Ok(Some(registered_pubkey)) = get_agent_pubkey(self.store, agent_id) else {
            return deny_all("unknown agent_id", &signed.body.requested_skills);
        };
        if registered_pubkey != signed.agent_pubkey {
            return deny_all("pubkey does not match enrollment", &signed.body.requested_skills);
        }

        if is_agent_suspended(self.store, agent_id).unwrap_or(true) {
            return deny_all("agent suspended", &signed.body.requested_skills);
        }

        let allowlist = get_agent_allowlist(self.store, agent_id).unwrap_or_default();

        let mut granted: Vec<String> = Vec::new();
        let mut denied: Vec<(String, String)> = Vec::new();
        let mut tier_source: HashMap<String, SkillTier> = HashMap::new();

        for skill in &signed.body.requested_skills {
            if allowlist.contains(skill) {
                granted.push(skill.clone());
                tier_source.insert(skill.clone(), SkillTier::Base);
                continue;
            }

            let tier = get_skill_tier(self.store, skill).unwrap_or_default();
            match tier {
                SkillTier::Restricted => {
                    denied.push((skill.clone(), "restricted tier — requires admin allowlist grant".into()));
                }
                SkillTier::Base => {
                    // In the registry as base-tier but not on THIS agent's
                    // allowlist yet — same denial as an unenrolled skill,
                    // no LLM call: base tier is never escalatable either.
                    denied.push((skill.clone(), "base-tier skill not in agent's allowlist".into()));
                }
                SkillTier::Escalatable => {
                    let description = get_skill_description(self.store, skill).unwrap_or_default();
                    match self
                        .judge
                        .judge(agent_id, skill, &description, &signed.body.justification)
                        .await
                    {
                        JudgeOutcome::Granted => {
                            granted.push(skill.clone());
                            tier_source.insert(skill.clone(), SkillTier::Escalatable);
                        }
                        JudgeOutcome::Denied { reason } => denied.push((skill.clone(), reason)),
                    }
                }
            }
        }

        if granted.is_empty() {
            return CapabilityReply { granted, denied, jwt: None, expires_at: None };
        }

        let grant = Grant::new(agent_id, granted.clone(), tier_source, self.grant_ttl);
        if persist_grant(self.store, &grant).is_err() {
            return deny_all("failed to persist grant record", &signed.body.requested_skills);
        }

        let jwt = match jwt_mint::mint_user_jwt(
            self.account_signing_key,
            self.account_pubkey,
            &signed.agent_pubkey,
            &granted,
            self.grant_ttl,
        ) {
            Ok(j) => j,
            Err(_) => return deny_all("failed to mint jwt after persisting grant", &signed.body.requested_skills),
        };

        CapabilityReply { granted, denied, jwt: Some(jwt), expires_at: Some(grant.expires_at) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enroll::enroll_agent;
    use crate::judge::FakeJudge;
    use crate::request::CapabilityRequest;
    use crate::tiers::set_skill_tier;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;

    fn store() -> RedbScopeStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        RedbScopeStore::open(path, ScopeId::Global, None).unwrap()
    }

    #[tokio::test]
    async fn base_skill_grants_without_llm_call() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        let judge = FakeJudge::always_deny("should not be called");
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.read".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert_eq!(reply.granted, vec!["skill.read".to_string()]);
        assert!(reply.jwt.is_some());
    }

    #[tokio::test]
    async fn restricted_skill_never_reaches_the_judge() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &[]).unwrap();
        set_skill_tier(&mut s, "skill.delete-infra", SkillTier::Restricted).unwrap();
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.delete-infra".into()], justification: "trust me".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert_eq!(reply.denied[0].0, "skill.delete-infra");
        assert!(reply.jwt.is_none());
    }

    #[tokio::test]
    async fn escalatable_skill_denied_by_judge_yields_no_grant() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &[]).unwrap();
        let judge = FakeJudge::always_deny("insufficient justification");
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.write".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert!(reply.jwt.is_none());
    }

    #[tokio::test]
    async fn unknown_agent_is_denied() {
        let mut s = store();
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let stray_kp = identity::AgentKeyPair::generate();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &stray_kp,
            CapabilityRequest { agent_id: "ghost".into(), requested_skills: vec!["skill.read".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert!(reply.jwt.is_none());
    }

    #[tokio::test]
    async fn suspended_agent_is_denied_even_for_base_skills() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        crate::enroll::suspend_agent(&mut s, "agent-1").unwrap();
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.read".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
    }
}
```

- [ ] **Step 2: Wire the module in**

In `capability-forge/src/lib.rs`, add:

```rust
pub mod service;
```

- [ ] **Step 3: Run the tests**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge service:: -- --nocapture
```

Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/src/service.rs capability-forge/src/lib.rs
git commit -m "feat(capability-forge): policy engine wiring identity/tiers/judge/grant/mint"
```

---

### Task 9: NATS service binary

**Files:**
- Create: `capability-forge/src/bin/main.rs`
- Modify: `capability-forge/Cargo.toml` (nothing new needed — `tokio`/`async-nats` are already deps; `[[bin]]` is auto-detected from `src/bin/`)

**Interfaces:**
- Consumes: `service::CapabilityForge`, `request::SignedRequest`, `judge::OpenAiJudge`, `b00t_c0re_gov::redb_scope_store::RedbScopeStore`.
- Produces: the `capability-forge` binary — subscribes to `capability.request.*` on a NATS server (URL from `NATS_URL` env, default `nats://127.0.0.1:4222`), replies on each message's `reply` subject. Reads `CAPFORGE_DB_PATH` (redb file path), `CAPFORGE_ACCOUNT_SEED` (nkeys account seed), `CAPFORGE_ACCOUNT_PUBKEY`, `CAPFORGE_JUDGE_MODEL` (passed to `OpenAiJudge::new`) from env — no config file parsing in phase 1, matches YAGNI.

This task has no unit test of its own (it is glue over already-tested pieces); its correctness is proven by Task 11's end-to-end test, which spawns this exact binary.

- [ ] **Step 1: Write the binary**

Create `capability-forge/src/bin/main.rs`:

```rust
use anyhow::{Context, Result};
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::scope_store::ScopeId;
use capability_forge::judge::OpenAiJudge;
use capability_forge::request::SignedRequest;
use capability_forge::service::CapabilityForge;
use futures::StreamExt;
use nkeys::KeyPair;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber_init();

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let db_path = env::var("CAPFORGE_DB_PATH").context("CAPFORGE_DB_PATH not set")?;
    let account_seed = env::var("CAPFORGE_ACCOUNT_SEED").context("CAPFORGE_ACCOUNT_SEED not set")?;
    let account_pubkey = env::var("CAPFORGE_ACCOUNT_PUBKEY").context("CAPFORGE_ACCOUNT_PUBKEY not set")?;
    let judge_model = env::var("CAPFORGE_JUDGE_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let mut store = RedbScopeStore::open(&db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    let account_signing_key = KeyPair::from_seed(&account_seed).context("invalid account seed")?;
    let judge = OpenAiJudge::new(judge_model);

    let client = async_nats::connect(&nats_url).await.context("connecting to NATS")?;
    let mut sub = client.subscribe("capability.request.*").await.context("subscribing")?;

    tracing::info!("capability-forge listening on capability.request.*");

    while let Some(msg) = sub.next().await {
        let Some(reply_subject) = msg.reply.clone() else {
            tracing::warn!("request with no reply subject, dropping");
            continue;
        };

        let signed: SignedRequest = match serde_json::from_slice(&msg.payload) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("malformed request: {e}");
                continue;
            }
        };

        let mut forge = CapabilityForge {
            store: &mut store,
            judge: &judge,
            account_signing_key: &account_signing_key,
            account_pubkey: &account_pubkey,
            grant_ttl: chrono::Duration::minutes(30),
        };
        let reply = forge.handle_request(signed).await;

        let payload = serde_json::to_vec(&reply).context("serializing reply")?;
        client.publish(reply_subject, payload.into()).await.context("publishing reply")?;
    }

    Ok(())
}

fn tracing_subscriber_init() {
    let _ = tracing_subscriber::fmt::try_init();
}
```

- [ ] **Step 2: Add the two new dependencies this binary needs**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo add futures tracing-subscriber --manifest-path capability-forge/Cargo.toml
```

- [ ] **Step 3: Verify it builds**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo build -p capability-forge --bin capability-forge
```

Expected: builds successfully (no test to run — this binary has no standalone logic, only wiring; `RedbScopeStore` is used directly, not behind `dyn TransactionalScopeStore`, so no trait-object coercion issue is expected, but if `CapabilityForge.store` expects `&mut dyn TransactionalScopeStore` specifically, pass `&mut store as &mut dyn TransactionalScopeStore` or adjust the struct field type — resolve whichever way the compiler actually requires).

- [ ] **Step 4: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/src/bin/main.rs capability-forge/Cargo.toml
git commit -m "feat(capability-forge): NATS request-reply service binary"
```

---

### Task 10: b00t-cli admin subcommand

**Files:**
- Create: `b00t-cli/src/commands/capability_forge.rs`
- Modify: `b00t-cli/src/main.rs` (register the subcommand — find the existing `mod commands::...` / clap subcommand enum registration pattern already used for e.g. `mod commands::agent;` and mirror it)
- Modify: `b00t-cli/Cargo.toml` (add `capability-forge = { path = "../capability-forge" }`)

**Interfaces:**
- Consumes: `capability_forge::enroll::{enroll_agent, suspend_agent, grant_base_skill}`.
- Produces: `b00t-cli capability-forge enroll --agent-id <id> --skill <skill> [--skill <skill>...] --db-path <path>` (prints the generated seed to stdout — the plan does not specify a secrets-transport mechanism beyond "hands it to the agent out of band," per the spec), `b00t-cli capability-forge suspend --agent-id <id> --db-path <path>`, `b00t-cli capability-forge grant --agent-id <id> --skill <skill> --db-path <path>`.

This task has no automated test — it is a thin CLI wrapper over already-tested `enroll.rs` functions. Verification is manual (Step 3 below) since `b00t-cli`'s existing subcommand-registration pattern must be discovered and matched, not invented.

- [ ] **Step 1: Find the existing subcommand registration pattern**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
grep -n "mod agent;\|Agent(" b00t-cli/src/main.rs
grep -n "^pub fn \|^pub struct \|clap::Subcommand\|clap::Args" b00t-cli/src/commands/agent.rs | head -20
```

Read enough of `b00t-cli/src/commands/agent.rs` and its registration in `main.rs` to see the exact `clap` enum-variant-plus-handler-function pattern this codebase uses (it will not match any hypothetical shown here — this repo's actual convention is the source of truth).

- [ ] **Step 2: Add the dependency**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo add capability-forge --path capability-forge --manifest-path b00t-cli/Cargo.toml
```

- [ ] **Step 3: Write `b00t-cli/src/commands/capability_forge.rs`**

Following the exact pattern found in Step 1, implement three handlers:

```rust
use anyhow::{Context, Result};
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::scope_store::ScopeId;
use capability_forge::enroll::{enroll_agent, grant_base_skill, suspend_agent};

pub fn enroll(db_path: &str, agent_id: &str, skills: &[String]) -> Result<()> {
    let mut store = RedbScopeStore::open(db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    let kp = enroll_agent(&mut store, agent_id, skills)?;
    println!("agent_id: {agent_id}");
    println!("pubkey: {}", kp.public_key());
    println!("seed (hand to agent, do not store here): {}", kp.seed()?);
    Ok(())
}

pub fn suspend(db_path: &str, agent_id: &str) -> Result<()> {
    let mut store = RedbScopeStore::open(db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    suspend_agent(&mut store, agent_id)?;
    println!("agent {agent_id} suspended");
    Ok(())
}

pub fn grant(db_path: &str, agent_id: &str, skill: &str) -> Result<()> {
    let mut store = RedbScopeStore::open(db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    grant_base_skill(&mut store, agent_id, skill)?;
    println!("agent {agent_id} granted base-tier skill {skill}");
    Ok(())
}
```

Wire the clap arg-parsing layer and `main.rs` dispatch arm using the exact enum/derive pattern read in Step 1 — write the real `clap::Args`/`clap::Subcommand` variants and their fields (`--agent-id`, `--skill` as `Vec<String>` for enroll via `#[arg(long)]` repeated, `--db-path`) matching that pattern's field-attribute style precisely, not a generic guess.

- [ ] **Step 4: Verify it builds and runs**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo build -p b00t-cli
/home/brianh/.cargo/bin/cargo run -p b00t-cli -- capability-forge enroll --agent-id smoke-test --skill skill.read --db-path /tmp/capforge-smoke.redb
```

Expected: prints an `agent_id`, `pubkey`, and `seed` line. Delete `/tmp/capforge-smoke.redb` afterward.

- [ ] **Step 5: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add b00t-cli/src/commands/capability_forge.rs b00t-cli/src/main.rs b00t-cli/Cargo.toml
git commit -m "feat(b00t-cli): capability-forge enroll/suspend/grant admin subcommand"
```

---

### Task 11: End-to-end integration test against ephemeral local `nats-server`

**Files:**
- Create: `capability-forge/tests/e2e_local_nats.rs`
- Modify: `capability-forge/Cargo.toml` (`[dev-dependencies]`: add `futures` if not already promoted from Task 9's main-dependency add — check first)

**Interfaces:**
- Consumes: everything. This is the capstone test the whole plan builds toward — the concrete definition of "test this end to end" from the design spec's Testing section.

This is the one task in the plan that is deliberately NOT written as a single tight TDD red/green step, because its "test" IS the deliverable, not a unit alongside one. The steps below build it incrementally so each piece is verified before the next depends on it.

- [ ] **Step 1: Write the NATS server fixture (operator + one account, MEMORY resolver)**

Create `capability-forge/tests/e2e_local_nats.rs`:

```rust
use capability_forge::enroll::enroll_agent;
use capability_forge::grant::{revoke_grant, Grant};
use capability_forge::judge::FakeJudge;
use capability_forge::jwt_mint::skill_subject;
use capability_forge::request::{CapabilityRequest, SignedRequest};
use capability_forge::service::CapabilityForge;
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::scope_store::ScopeId;
use nats_jwt::Token;
use nkeys::KeyPair;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct NatsFixture {
    child: Child,
    port: u16,
    account_pubkey: String,
    account_signing_key: KeyPair,
    config_path: std::path::PathBuf,
    _tempdir: tempfile::TempDir,
}

impl NatsFixture {
    fn start() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let port = pick_free_port();

        let operator = KeyPair::new_operator();
        let account_signing_key = KeyPair::new_account();
        let account_pubkey = account_signing_key.public_key();

        let mut account_token = Token::new_account(&account_pubkey);
        account_token.set_name("capforge-test");
        let account_jwt = account_token.sign(&operator).expect("sign account jwt");

        let config_path = tempdir.path().join("nats.conf");
        let config = format!(
            r#"
port: {port}
operator: {operator_jwt}
resolver: MEMORY
resolver_preload: {{
  {account_pubkey}: {account_jwt:?}
}}
"#,
            operator_jwt = operator_pseudo_jwt(&operator),
        );
        std::fs::write(&config_path, config).unwrap();

        let child = Command::new("/home/brianh/.local/bin/nats-server")
            .arg("-c")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn nats-server — is it installed at /home/brianh/.local/bin/nats-server?");

        std::thread::sleep(Duration::from_millis(500));

        Self { child, port, account_pubkey, account_signing_key, config_path, _tempdir: tempdir }
    }

    fn url(&self) -> String {
        format!("nats://127.0.0.1:{}", self.port)
    }

    fn reload(&self, new_account_jwt: &str) {
        let config = format!(
            r#"
port: {port}
operator: {operator_jwt}
resolver: MEMORY
resolver_preload: {{
  {account_pubkey}: {account_jwt:?}
}}
"#,
            port = self.port,
            operator_jwt = "REPLACE_WITH_SAME_OPERATOR_JWT_USED_AT_START",
            account_pubkey = self.account_pubkey,
            account_jwt = new_account_jwt,
        );
        std::fs::write(&self.config_path, config).unwrap();
        Command::new("/home/brianh/.local/bin/nats-server")
            .arg("--signal")
            .arg(format!("reload={}", self.child.id()))
            .status()
            .expect("send reload signal");
        std::thread::sleep(Duration::from_millis(300));
    }
}

impl Drop for NatsFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pick_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn operator_pseudo_jwt(_operator: &KeyPair) -> String {
    // Placeholder for Step 1 compile-check only — Step 2 replaces this with
    // a real signed operator self-JWT once `nats-jwt::Token::new_operator`'s
    // exact builder API is confirmed against the installed crate version.
    String::new()
}

#[test]
fn nats_server_starts_and_stops_cleanly() {
    let fixture = NatsFixture::start();
    assert!(fixture.url().starts_with("nats://127.0.0.1:"));
    drop(fixture);
}
```

- [ ] **Step 2: Run the smoke test, then replace the operator-JWT placeholder with the real signed token**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge --test e2e_local_nats nats_server_starts_and_stops_cleanly -- --nocapture
```

This will fail (the empty `operator` config value is invalid) — that failure is expected and is the point: it proves the fixture is actually exercising a real `nats-server` process, not a mock. Fix `operator_pseudo_jwt` to build and sign a real self-signed operator JWT (`Token::new_operator(&operator.public_key())`, set its own account-signing/system fields as `nats-jwt`'s API requires — consult `cargo doc -p nats-jwt --no-deps --open`), pass the resulting JWT string into the config's `operator:` field (nats-server accepts an inline JWT string there, not only a file path — confirm against `nats-server --help` / its config docs; if it only accepts a path, write the operator JWT to its own file in `tempdir` and reference that path instead). Re-run until `nats_server_starts_and_stops_cleanly` passes — check `fixture.child`'s stderr (captured via the piped `Stdio`) for the server's own error message if it still won't start; that message names exactly which config key is wrong.

- [ ] **Step 3: Add the full end-to-end flow test**

Append to `capability-forge/tests/e2e_local_nats.rs`:

```rust
#[tokio::test]
async fn full_flow_enroll_request_connect_scope_enforced_then_revoked() {
    let fixture = NatsFixture::start();

    let db_dir = tempfile::tempdir().unwrap();
    let mut store = RedbScopeStore::open(db_dir.path().join("capforge.redb"), ScopeId::Global, None).unwrap();

    let agent_kp = enroll_agent(&mut store, "agent-e2e", &["skill.read".to_string()]).unwrap();

    let judge = FakeJudge::always_grant();
    let mut forge = CapabilityForge {
        store: &mut store,
        judge: &judge,
        account_signing_key: &fixture.account_signing_key,
        account_pubkey: &fixture.account_pubkey,
        grant_ttl: chrono::Duration::minutes(30),
    };

    let signed = SignedRequest::sign(
        &agent_kp,
        CapabilityRequest {
            agent_id: "agent-e2e".into(),
            requested_skills: vec!["skill.read".into()],
            justification: "".into(),
        },
    )
    .unwrap();
    let reply = forge.handle_request(signed).await;
    assert!(reply.jwt.is_some(), "expected a minted jwt, got denials: {:?}", reply.denied);
    let jwt = reply.jwt.unwrap();

    let allowed_subject = skill_subject(&agent_kp.public_key(), "skill.read");
    let disallowed_subject = skill_subject(&agent_kp.public_key(), "skill.write");

    let creds = write_creds_file(&jwt, &agent_kp.seed().unwrap());
    let client = async_nats::ConnectOptions::new()
        .credentials_file(&creds)
        .await
        .unwrap()
        .connect(fixture.url())
        .await
        .expect("agent should connect with minted jwt");

    client.publish(allowed_subject.clone(), "hi".into()).await.expect("publish on granted subject should succeed");
    client.flush().await.unwrap();

    let publish_result = client.publish(disallowed_subject.clone(), "hi".into()).await;
    assert!(publish_result.is_err(), "publish on non-granted subject should be rejected by NATS itself");

    drop(client);
}

fn write_creds_file(jwt: &str, seed: &str) -> std::path::PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.creds");
    let mut f = std::fs::File::create(&path).unwrap();
    write!(
        f,
        "-----BEGIN NATS USER JWT-----\n{jwt}\n------END NATS USER JWT------\n\n\
         -----BEGIN USER NKEY SEED-----\n{seed}\n------END USER NKEY SEED------\n"
    )
    .unwrap();
    std::mem::forget(dir);
    path
}
```

- [ ] **Step 4: Run and fix against real behavior**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge --test e2e_local_nats full_flow -- --nocapture
```

Two likely real-world adjustments, both expected and both to be resolved by reading the actual error, not guessed in advance:
1. `async-nats`'s exact JWT+seed connection API (`ConnectOptions::credentials_file` vs. `jwt`+`nkey`/signature-callback methods) — check `cargo doc -p async-nats --no-deps --open` under `ConnectOptions` and use whichever the installed `0.45` version actually exposes; a `.creds` file (the format written by `write_creds_file`) is the standard NATS CLI format either way and is a reasonable target regardless of which connection method reads it.
2. Whether an in-process publish to a permission-denied subject surfaces as an `Err` from `.publish()` or is instead accepted client-side and silently dropped/disconnected server-side (some NATS clients don't hard-fail a permission violation synchronously). If the latter, assert on the *connection's* error/disconnect handler firing, or on `client.flush().await` returning an error after the disallowed publish, instead of the `.publish()` call itself — whichever the real client behavior turns out to be. The requirement is "the disallowed publish demonstrably did not succeed as a normal message would," not a specific error site.

- [ ] **Step 5: Add revocation to the same test**

Extend `full_flow_enroll_request_connect_scope_enforced_then_revoked` (after the disallowed-publish assertion, before the final `drop(client)`) with:

```rust
    revoke_grant(&mut store, &current_jti /* captured from `reply` before it was consumed above — restructure the test to keep `reply.expires_at`/jti accessible, e.g. by not shadowing `reply` or by extracting the jti from the minted jwt's claims via `nats_jwt`'s decode function before this point */).unwrap();

    let mut updated_account_token = Token::new_account(&fixture.account_pubkey);
    // populate the same account fields as NatsFixture::start(), plus:
    updated_account_token.add_revocation(&agent_kp.public_key(), chrono::Utc::now());
    let updated_account_jwt = updated_account_token.sign(&fixture.account_signing_key).unwrap();
    fixture.reload(&updated_account_jwt);

    let reconnect = async_nats::ConnectOptions::new()
        .credentials_file(&creds)
        .await
        .unwrap()
        .connect(fixture.url())
        .await;
    assert!(reconnect.is_err(), "revoked agent should be refused on reconnect");
```

This step names a real gap to close, not a placeholder to leave: `Grant`'s `jti` needs to be readable by the test after `handle_request` returns (either don't let `reply` go out of scope before this point, or decode the minted JWT's `jti` claim directly), and `Token::add_revocation`'s exact signature (subject key vs. all-users wildcard, timestamp type) needs the same "check `cargo doc -p nats-jwt`" treatment as Step 2. Resolve both against the compiler and the crate's actual API before considering this task done — do not comment this section out or stub it to make the suite green.

- [ ] **Step 6: Run the full suite one more time clean**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
/home/brianh/.cargo/bin/cargo test -p capability-forge -- --nocapture
```

Expected: every test in the crate passes, including `full_flow_enroll_request_connect_scope_enforced_then_revoked`. This is the plan's actual finish line.

- [ ] **Step 7: Commit**

```bash
cd /home/brianh/.b00t/.worktrees/task-capability-forge
git add capability-forge/tests/e2e_local_nats.rs capability-forge/Cargo.toml
git commit -m "test(capability-forge): end-to-end flow against ephemeral local nats-server, including revocation-on-reconnect"
```

---

## Notes for whoever executes this plan

- Tasks 1–8 are pure-Rust, no external process, and should go smoothly with normal TDD iteration.
- Task 9 (binary) and Task 10 (CLI) are glue; low risk, no new logic.
- Task 11 is where real-world friction lives: two third-party crates (`nkeys`, `nats-jwt`) whose exact builder APIs this plan matched to their documented shape but not to a byte-verified compile, and `nats-server`'s own config/reload semantics. Budget the most iteration time there. If `nats-jwt` proves unworkable against the installed version, the fallback is building the JWT claims JSON by hand (NATS JWT v2 is a documented, stable wire format: base64url(header) + "." + base64url(claims) + "." + base64url(nkeys-Ed25519-signature-over-the-first-two-segments)) signed via `nkeys::KeyPair::sign` directly — more code, zero new dependency risk.

## Status: implemented, reviewed, merge-ready (2026-08-14)

All 11 tasks complete via subagent-driven development (fresh implementer + independent
reviewer per task, fix loops for every finding, final whole-branch review on the most capable
available model). Full crate suite: 44/44 tests passing, including a true end-to-end test
against an ephemeral local `nats-server` proving server-side NATS scope enforcement and a
genuine `AuthorizationViolation`-specific rejection on revoked-JWT reconnect.

**Known follow-ups, deliberately not fixed here** (real, disclosed, non-blocking for phase-1
merge):

1. **Revocation has no production wiring.** `grant.rs`'s `revoke_grant`/`is_revoked` are
   real, tested, and correct as bookkeeping — but nothing in production code (the NATS
   service binary, the CLI) actually pushes an updated NATS account JWT or triggers a server
   reload. `tests/e2e_local_nats.rs` proves the underlying NATS mechanism works by performing
   that push/reload by hand. The design spec's Revocation section already flagged this as
   "real machinery, not an afterthought" (capability-forge holding operator-level authority
   over live account claims); it needs its own design pass before phase 2, not a bolt-on fix.
   Until then, "revoke" is a persisted intention with no automatic enforcement path.
2. Minor hardening items parked by the final review and its fix-wave re-review: `mint_user_jwt`
   should be `pub(crate)` not `pub`; `tiers.rs`/`grant.rs` use two different persistence
   conventions on the same store (`add_to_allowlist` is an unguarded read-modify-write, unlike
   `revoke_grant`'s CAS-retry loop); `b00t-cli`'s `capability-forge` dependency isn't
   feature-gated and pulls a heavy transitive stack (reqwest et al.) into every `b00t-cli`
   build; the oversized-request denial path in `service.rs` echoes the full attacker-controlled
   skill list back (response-amplification shape); `is_valid_skill_name` bounds the charset but
   not the length; the LLM judge's prompt-injection defense delimiters aren't escaped against a
   crafted `justification` forging a closing tag; `bin/main.rs` never cross-checks
   `CAPFORGE_ACCOUNT_SEED`'s derived pubkey against `CAPFORGE_ACCOUNT_PUBKEY`.
