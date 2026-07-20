//! Integration tests for agent travel between hives.
//!
//! Tests cover:
//! - TravelManifest creation
//! - TravelState lifecycle (Departing → Abroad → Returning → Completed)
//! - Overdue detection

use b00t_c0re_a2a::agent_card::AgentCard;
use b00t_c0re_a2a::travel::{TravelAgent, TravelManifest, TravelState};
use url::Url;

// ---------------------------------------------------------------------------
// TravelManifest creation
// ---------------------------------------------------------------------------

#[test]
fn test_travel_manifest_creation() {
    let url = Url::parse("stdio://traveller").unwrap();
    let agent = AgentCard::new("traveller", "A travelling agent", url);

    let manifest = TravelManifest {
        agent,
        source_hive: "hive-alpha".to_string(),
        destination_hive: "hive-beta".to_string(),
        departure_time: chrono::Utc::now(),
        return_by: Some(chrono::Utc::now() + chrono::Duration::hours(2)),
        frozen_balance: 42.0,
        skill_ids: vec!["ping".to_string(), "pong".to_string()],
        state: TravelState::Departing,
    };

    assert_eq!(manifest.source_hive, "hive-alpha");
    assert_eq!(manifest.destination_hive, "hive-beta");
    assert_eq!(manifest.state, TravelState::Departing);
    assert_eq!(manifest.frozen_balance, 42.0);
    assert_eq!(manifest.skill_ids.len(), 2);
    assert!(manifest.return_by.is_some());
}

#[test]
fn test_travel_manifest_no_return_by() {
    let url = Url::parse("stdio://one-way").unwrap();
    let agent = AgentCard::new("one-way", "One-way traveller", url);

    let manifest = TravelManifest {
        agent,
        source_hive: "alpha".to_string(),
        destination_hive: "beta".to_string(),
        departure_time: chrono::Utc::now(),
        return_by: None,
        frozen_balance: 100.0,
        skill_ids: vec![],
        state: TravelState::Departing,
    };

    assert!(manifest.return_by.is_none());
}

#[test]
fn test_travel_manifest_serialization_roundtrip() {
    let url = Url::parse("stdio://serial-traveller").unwrap();
    let agent = AgentCard::new("serial-traveller", "Round-trip test", url);

    let manifest = TravelManifest {
        agent,
        source_hive: "src".to_string(),
        destination_hive: "dst".to_string(),
        departure_time: chrono::Utc::now(),
        return_by: None,
        frozen_balance: 75.0,
        skill_ids: vec!["code-gen".to_string()],
        state: TravelState::Abroad,
    };

    let json = serde_json::to_string_pretty(&manifest).expect("serialize");
    let deserialized: TravelManifest = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.source_hive, "src");
    assert_eq!(deserialized.destination_hive, "dst");
    assert_eq!(deserialized.state, TravelState::Abroad);
    assert!((deserialized.frozen_balance - 75.0).abs() < 1e-6);
    assert_eq!(deserialized.skill_ids, vec!["code-gen".to_string()]);
}

// ---------------------------------------------------------------------------
// TravelState lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_travel_state_lifecycle() {
    // The canonical lifecycle: Departing → Abroad → Returning → Completed
    let mut state = TravelState::Departing;
    assert_eq!(state, TravelState::Departing);

    state = TravelState::Abroad;
    assert_eq!(state, TravelState::Abroad);

    state = TravelState::Returning;
    assert_eq!(state, TravelState::Returning);

    state = TravelState::Completed;
    assert_eq!(state, TravelState::Completed);
}

#[test]
fn test_travel_state_lost() {
    // Lost is a terminal state reachable from Abroad when overdue
    let state = TravelState::Lost;
    assert_eq!(state, TravelState::Lost);
}

#[test]
fn test_travel_state_partial_eq() {
    assert_eq!(TravelState::Departing, TravelState::Departing);
    assert_ne!(TravelState::Departing, TravelState::Abroad);
    assert_ne!(TravelState::Abroad, TravelState::Completed);
    assert_ne!(TravelState::Returning, TravelState::Lost);
}

// ---------------------------------------------------------------------------
// Overdue detection
// ---------------------------------------------------------------------------

#[test]
fn test_overdue_detection() {
    let url = Url::parse("stdio://overdue-agent").unwrap();
    let agent = AgentCard::new("overdue", "Overdue agent", url);

    let past_deadline = chrono::Utc::now() - chrono::Duration::hours(1);

    // Agent that is abroad and past return deadline
    let overdue = TravelManifest {
        agent: agent.clone(),
        source_hive: "hive-a".to_string(),
        destination_hive: "hive-b".to_string(),
        departure_time: chrono::Utc::now() - chrono::Duration::hours(4),
        return_by: Some(past_deadline),
        frozen_balance: 50.0,
        skill_ids: vec![],
        state: TravelState::Abroad,
    };

    // Agent that is abroad but within return deadline
    let on_time = TravelManifest {
        agent: agent.clone(),
        source_hive: "hive-a".to_string(),
        destination_hive: "hive-b".to_string(),
        departure_time: chrono::Utc::now(),
        return_by: Some(chrono::Utc::now() + chrono::Duration::hours(4)),
        frozen_balance: 50.0,
        skill_ids: vec![],
        state: TravelState::Abroad,
    };

    // Agent that already returned (completed) but past deadline — should NOT be flagged
    let returned = TravelManifest {
        agent: agent.clone(),
        source_hive: "hive-a".to_string(),
        destination_hive: "hive-b".to_string(),
        departure_time: chrono::Utc::now() - chrono::Duration::hours(4),
        return_by: Some(chrono::Utc::now() - chrono::Duration::hours(2)),
        frozen_balance: 50.0,
        skill_ids: vec![],
        state: TravelState::Completed,
    };

    // Agent with no return deadline (indefinite stay)
    let indefinite = TravelManifest {
        agent: agent.clone(),
        source_hive: "hive-a".to_string(),
        destination_hive: "hive-b".to_string(),
        departure_time: chrono::Utc::now(),
        return_by: None,
        frozen_balance: 50.0,
        skill_ids: vec![],
        state: TravelState::Abroad,
    };

    let manifests = vec![overdue, on_time, returned, indefinite];
    let overdue_manifests = TravelAgent::check_overdue(&manifests);

    assert_eq!(
        overdue_manifests.len(),
        1,
        "Only one manifest should be overdue"
    );
    assert_eq!(
        overdue_manifests[0].state,
        TravelState::Abroad,
        "Overdue agent should be Abroad"
    );
    // The overdue one should have had the early return_by
    assert!(
        overdue_manifests[0]
            .return_by
            .map(|t| t < chrono::Utc::now())
            .unwrap_or(false),
        "Overdue agent's return_by should be in the past"
    );
}

#[test]
fn test_no_overdue_when_all_returned() {
    let manifests: Vec<TravelManifest> = vec![];
    let overdue = TravelAgent::check_overdue(&manifests);
    assert!(overdue.is_empty());
}

#[test]
fn test_overdue_requires_abroad_state() {
    let url = Url::parse("stdio://test-agent").unwrap();
    let agent = AgentCard::new("test", "Test agent", url);
    let past = chrono::Utc::now() - chrono::Duration::hours(1);

    // Departing state — even past deadline, not overdue (not abroad yet)
    let departed = TravelManifest {
        agent: agent.clone(),
        source_hive: "a".to_string(),
        destination_hive: "b".to_string(),
        departure_time: chrono::Utc::now() - chrono::Duration::hours(4),
        return_by: Some(past),
        frozen_balance: 0.0,
        skill_ids: vec![],
        state: TravelState::Departing,
    };

    // Lost state — already flagged, not "overdue"
    let lost = TravelManifest {
        agent: agent.clone(),
        source_hive: "a".to_string(),
        destination_hive: "b".to_string(),
        departure_time: chrono::Utc::now() - chrono::Duration::hours(4),
        return_by: Some(past),
        frozen_balance: 0.0,
        skill_ids: vec![],
        state: TravelState::Lost,
    };

    let manifests = vec![departed, lost];
    let overdue = TravelAgent::check_overdue(&manifests);
    assert_eq!(
        overdue.len(),
        0,
        "Neither Departing nor Lost should be overdue"
    );
}
