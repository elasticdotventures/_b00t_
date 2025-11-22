# b00t Integration Test Plan

## Test Coverage Matrix

| Feature | Unit Tests | Integration Tests | E2E Tests | Status |
|---------|-----------|-------------------|-----------|--------|
| Message Bus | ✅ 4/4 | ❌ | ❌ | Partial |
| Voting Protocol | ✅ | ❌ | ❌ | Partial |
| Handshake | ✅ | ❌ | ❌ | Partial |
| Ahoy Protocol | ❌ | ❌ | ❌ | **BROKEN** |
| Skill Sharing | ❌ | ❌ | ❌ | Not Implemented |
| Datum Display | ❌ | ❌ | N/A | Not Implemented |
| Cross-Process IPC | ❌ | ❌ | ❌ | Not Implemented |
| State Persistence | ❌ | ❌ | ❌ | Not Implemented |

---

## Priority 1: Critical Path Tests (Blockers)

### Test 1: Ahoy Complete Workflow
**File**: `b00t-ipc/tests/integration_ahoy.rs`
**Duration**: ~15 min to write, 2 hours to implement supporting code
**Blocks**: /award command functionality

```rust
#[tokio::test]
async fn test_ahoy_full_lifecycle() {
    // Setup
    let bus = MessageBus::new().await.unwrap();
    let captain = Agent::new("captain", vec!["leadership"]);
    let dev1 = Agent::new("dev1", vec!["rust", "docker"]);
    let dev2 = Agent::new("dev2", vec!["python"]);

    bus.register(captain.clone()).await.unwrap();
    bus.register(dev1.clone()).await.unwrap();
    bus.register(dev2.clone()).await.unwrap();

    // STEP 1: Captain posts ahoy
    let ahoy_id = Uuid::new_v4().to_string();
    bus.send(Message::Ahoy {
        from: captain.id.clone(),
        ahoy_id: ahoy_id.clone(),
        role: "Backend Dev".to_string(),
        description: "Build REST API".to_string(),
        required_skills: vec!["rust".to_string()],
        budget: 100,
    }).await.unwrap();

    // Verify ahoy is tracked
    let announcement = bus.get_ahoy(&ahoy_id).await.expect("Ahoy not found");
    assert_eq!(announcement.budget, 100);
    assert_eq!(announcement.applications.len(), 0);

    // STEP 2: dev1 applies (matches skills)
    bus.send(Message::Apply {
        from: dev1.id.clone(),
        ahoy_id: ahoy_id.clone(),
        pitch: "Expert Rustacean with Docker exp".to_string(),
        relevant_skills: vec!["rust".to_string(), "docker".to_string()],
    }).await.unwrap();

    // STEP 3: dev2 applies (doesn't match but tries)
    bus.send(Message::Apply {
        from: dev2.id.clone(),
        ahoy_id: ahoy_id.clone(),
        pitch: "Fast learner, will learn Rust".to_string(),
        relevant_skills: vec!["python".to_string()],
    }).await.unwrap();

    // Verify applications tracked
    let announcement = bus.get_ahoy(&ahoy_id).await.unwrap();
    assert_eq!(announcement.applications.len(), 2);

    // STEP 4: Captain reviews and awards to dev1
    bus.send(Message::Award {
        from: captain.id.clone(),
        ahoy_id: ahoy_id.clone(),
        winner: dev1.id.clone(),
        budget: 100, // PROBLEM: Captain shouldn't need to know budget!
    }).await.unwrap();

    // Verify award state
    let final_announcement = bus.get_ahoy(&ahoy_id).await.unwrap();
    assert_eq!(final_announcement.awarded_to, Some(dev1.id.clone()));

    // STEP 5: dev1 should receive cake budget
    // TODO: Implement budget transfer tracking
}
```

**Required Implementation**:
1. Add `ahoys: HashMap<String, AhoyAnnouncement>` to MessageBus
2. Implement `get_ahoy()`, `post_ahoy()`, `apply_to_ahoy()` methods
3. Handle Message::Ahoy in message processing
4. Track applications per announcement
5. Implement award logic that retrieves budget

---

### Test 2: Datum Display Rendering
**File**: `b00t-c0re-lib/tests/datum_display.rs`
**Duration**: ~30 min
**Blocks**: Self-documenting datums

```rust
#[test]
fn test_boot_datum_display() {
    let datum = BootDatum {
        name: "just".to_string(),
        datum_type: Some(DatumType::Tool),
        hint: "Command runner and task automation".to_string(),
        version: Some("1.40.0".to_string()),
        install: Some("cargo install just".to_string()),
        usage: Some(vec![
            UsageExample {
                description: "List recipes".to_string(),
                command: "just -l".to_string(),
                output: None,
            },
        ]),
        ..Default::default()
    };

    let output = format!("{}", datum);

    // Verify markdown structure
    assert!(output.contains("# just"));
    assert!(output.contains("**Type**: Tool"));
    assert!(output.contains("**Hint**: Command runner"));
    assert!(output.contains("## Installation"));
    assert!(output.contains("```bash"));
    assert!(output.contains("cargo install just"));
    assert!(output.contains("## Usage"));
    assert!(output.contains("List recipes"));
}

#[test]
fn test_agent_datum_display() {
    let agent_config = r#"
[b00t]
name = "alpha"
type = "agent"

[b00t.agent]
skills = ["rust", "testing", "tdd"]
personality = "curious"
role = "specialist"
    "#;

    let datum: BootDatum = toml::from_str(agent_config).unwrap();
    let output = format!("{}", datum);

    assert!(output.contains("🤖"));
    assert!(output.contains("alpha"));
    assert!(output.contains("rust"));
    assert!(output.contains("specialist"));
}
```

**Required Implementation**:
1. Add `impl Display for BootDatum` in `b00t-c0re-lib/src/datum_types.rs`
2. Add `impl Display for AgentDatum` if separate type exists
3. Format as markdown with proper sections
4. Include emoji for different datum types

---

### Test 3: Multi-Agent Crew Formation
**File**: `b00t-ipc/tests/integration_crew.rs`
**Duration**: ~20 min
**Blocks**: Nothing, but validates core functionality

```rust
#[tokio::test]
async fn test_two_agents_form_crew_and_vote() {
    let bus = Arc::new(MessageBus::new().await.unwrap());

    // Spawn two agent tasks
    let alpha = Agent::new("alpha", vec!["rust", "testing"]);
    let beta = Agent::new("beta", vec!["docker", "ci-cd"]);

    bus.register(alpha.clone()).await.unwrap();
    bus.register(beta.clone()).await.unwrap();

    // Alpha initiates handshake with beta
    bus.send(Message::Handshake {
        from: alpha.id.clone(),
        to: beta.id.clone(),
        proposal: "Build microservice together".to_string(),
    }).await.unwrap();

    // Beta accepts (would need message handler)
    bus.send(Message::HandshakeReply {
        from: beta.id.clone(),
        to: alpha.id.clone(),
        accept: true,
        role: AgentRole::Specialist,
    }).await.unwrap();

    // Form crew
    bus.send(Message::CrewForm {
        initiator: alpha.id.clone(),
        members: vec![alpha.id.clone(), beta.id.clone()],
        purpose: "Microservice development".to_string(),
    }).await.unwrap();

    // Create proposal
    let proposal_id = bus.create_proposal(
        "Use Rust for backend, Python for ML pipeline",
        alpha.id.clone()
    ).await.unwrap();

    // Both vote yes
    bus.vote(&proposal_id, alpha.id.clone(), VoteChoice::Yes).await.unwrap();
    bus.vote(&proposal_id, beta.id.clone(), VoteChoice::Yes).await.unwrap();

    // Verify proposal passed (quorum = 2)
    assert!(bus.is_proposal_passed(&proposal_id).await.unwrap());
}
```

---

## Priority 2: Skill-Sharing Tests

### Test 4: Skill Teaching & Learning
**File**: `b00t-ipc/tests/integration_skills.rs`
**Duration**: ~1 hour (needs skill system implementation)

```rust
#[tokio::test]
async fn test_expert_teaches_novice() {
    let bus = MessageBus::new().await.unwrap();

    let mut expert = Agent::new("expert", vec!["rust"])
        .with_skill_proficiency(hashmap! {
            "rust".to_string() => 0.95,
            "tokio".to_string() => 0.88,
        });

    let mut novice = Agent::new("novice", vec![])
        .with_skill_proficiency(hashmap! {
            "rust".to_string() => 0.15,
        });

    bus.register(expert.clone()).await.unwrap();
    bus.register(novice.clone()).await.unwrap();

    // Expert offers to teach
    bus.send(Message::TeachSkill {
        from: expert.id.clone(),
        to: novice.id.clone(),
        skill: "tokio".to_string(),
        proficiency: 0.88,
        method: TeachingMethod::PairProgramming,
    }).await.unwrap();

    // Simulate learning session (6 hours of pairing)
    let sessions = 3; // 3 x 2-hour sessions
    for _ in 0..sessions {
        novice.learn_skill_from(&expert, "tokio", TeachingMethod::PairProgramming);
    }

    // Novice should now have decent tokio proficiency
    let novice_tokio = novice.get_skill_proficiency("tokio");
    assert!(novice_tokio > 0.5 && novice_tokio < 0.7);

    // Novice acknowledges learning
    bus.send(Message::SkillLearned {
        from: novice.id.clone(),
        skill: "tokio".to_string(),
        proficiency: novice_tokio,
        learned_from: expert.id.clone(),
    }).await.unwrap();
}

#[tokio::test]
async fn test_crew_knowledge_sharing() {
    let bus = MessageBus::new().await.unwrap();

    let agents = vec![
        Agent::new("alpha", vec!["rust"]),
        Agent::new("beta", vec!["docker"]),
        Agent::new("gamma", vec!["kubernetes"]),
    ];

    for agent in &agents {
        bus.register(agent.clone()).await.unwrap();
    }

    // Alpha shares async patterns knowledge
    bus.send(Message::ShareKnowledge {
        from: "alpha".to_string(),
        crew: "backend-team".to_string(),
        topic: "Tokio select! patterns".to_string(),
        content: "Use select! to race multiple futures...".to_string(),
        skill_impact: vec![
            ("tokio".to_string(), 0.15),  // Boost tokio skill
        ],
    }).await.unwrap();

    // All crew members should gain tokio knowledge
    // This requires crew membership tracking
}
```

**Required Implementation**:
1. Add `skill_proficiency: HashMap<String, f32>` to Agent struct
2. Implement `learn_skill_from()` method with teaching method multipliers
3. Add `TeachSkill`, `SkillLearned`, `ShareKnowledge` message types
4. Implement crew membership tracking in MessageBus
5. Add /teach, /learn k0mmand3r commands

---

## Priority 3: State Persistence Tests

### Test 5: MessageBus State Persistence
**File**: `b00t-ipc/tests/integration_persistence.rs`
**Duration**: ~45 min

```rust
#[tokio::test]
async fn test_bus_state_save_and_restore() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state_path = temp_dir.path().join("bus_state.json");

    let agent_id: String;
    let proposal_id: String;

    // Create bus, register agents, create proposals
    {
        let bus = MessageBus::new().await.unwrap();

        let agent = Agent::new("alpha", vec!["rust", "testing"]);
        agent_id = agent.id.clone();

        bus.register(agent).await.unwrap();
        proposal_id = bus.create_proposal("Test proposal", "alpha").await.unwrap();

        // Save state
        bus.save_state(&state_path).await.unwrap();
    }

    // Restore from state
    {
        let bus = MessageBus::load_from(&state_path).await.unwrap();

        // Verify agents restored
        let agents = bus.list_agents().await.unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, agent_id);

        // Verify proposals restored
        let proposal = bus.get_proposal(&proposal_id).await.unwrap();
        assert_eq!(proposal.description, "Test proposal");
    }
}
```

**Required Implementation**:
1. Implement `save_state()` method (serialize to JSON)
2. Implement `load_from()` static method (deserialize)
3. Add `list_agents()` and `get_proposal()` methods
4. Handle state versioning for backwards compatibility

---

## Priority 4: Cross-Process IPC Tests

### Test 6: Unix Socket Communication
**File**: `b00t-agent/tests/integration_ipc.rs`
**Duration**: ~2 hours (complex)

```rust
#[tokio::test]
async fn test_agents_communicate_via_unix_socket() {
    // Create socket directory
    let socket_dir = tempfile::tempdir().unwrap();
    std::env::set_var("B00T_SOCKET_DIR", socket_dir.path());

    // Spawn alpha agent process
    let mut alpha_proc = Command::new("b00t-agent")
        .args(&[
            "--id", "alpha",
            "--skills", "rust,testing",
            "--socket", socket_dir.path().join("alpha.sock").to_str().unwrap(),
        ])
        .spawn()
        .unwrap();

    // Wait for socket to be created
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Connect to alpha via socket
    let socket_path = socket_dir.path().join("alpha.sock");
    let stream = UnixStream::connect(&socket_path).await.unwrap();

    // Send handshake message
    let msg = Message::Handshake {
        from: "external-client".to_string(),
        to: "alpha".to_string(),
        proposal: "Test connection".to_string(),
    };

    write_message(&stream, &msg).await.unwrap();

    // Read response
    let response = read_message(&stream).await.unwrap();

    match response {
        Message::HandshakeReply { accept, .. } => {
            assert!(accept, "Alpha should accept handshake");
        }
        _ => panic!("Expected HandshakeReply"),
    }

    // Cleanup
    alpha_proc.kill().unwrap();
}

async fn write_message(stream: &UnixStream, msg: &Message) -> Result<()> {
    let json = serde_json::to_vec(msg)?;
    let len = json.len() as u32;

    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(&json).await?;

    Ok(())
}

async fn read_message(stream: &UnixStream) -> Result<Message> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut msg_buf = vec![0u8; len];
    stream.read_exact(&mut msg_buf).await?;

    Ok(serde_json::from_slice(&msg_buf)?)
}
```

**Required Implementation**:
1. Add Unix socket server to b00t-agent binary
2. Implement message framing protocol (length-prefixed JSON)
3. Add connection handling loop
4. Implement graceful shutdown on SIGTERM
5. Add socket cleanup on exit

---

## Test Execution Plan

### Week 1: Foundation
- Day 1-2: Implement ahoy state tracking + Test 1
- Day 3-4: Implement Display trait + Test 2
- Day 5: Multi-agent crew test (Test 3)

### Week 2: Skill System
- Day 1-2: Design skill proficiency model
- Day 3-4: Implement TeachSkill protocol
- Day 5: Skill-sharing tests (Test 4)

### Week 3: Persistence & IPC
- Day 1-2: State persistence (Test 5)
- Day 3-5: Unix socket IPC (Test 6)

---

## Continuous Integration

Add to `.github/workflows/test.yml`:

```yaml
- name: Run integration tests
  run: |
    cargo nextest run --package b00t-ipc --test integration_*
    cargo nextest run --package b00t-agent --test integration_*

- name: Run E2E tests
  run: |
    cargo test --package b00t-ipc --test e2e_*
```

---

## Test Coverage Goals

- **Unit tests**: 80% coverage minimum
- **Integration tests**: All cross-module workflows
- **E2E tests**: Complete user workflows (ahoy, crew formation, skill teaching)
- **Property tests**: Message serialization, state transitions

**Current Coverage**: ~30% (unit tests only)
**Target Coverage**: 75% by end of Phase 3
