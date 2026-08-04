use ufo_stereotype_spike::{Kind, Mode, Relator, Role, SubKind, UfoStereotypeOrSubtype};

#[test]
fn kind_survives_json_roundtrip() {
    let original = UfoStereotypeOrSubtype::Kind(Kind {
        name: "Company".into(),
    });
    let json = serde_json::to_string(&original).unwrap();
    assert_eq!(json, r#"{"stereotype_kind":"Kind","name":"Company"}"#);
    let back: UfoStereotypeOrSubtype = serde_json::from_str(&json).unwrap();
    assert_eq!(original, back);
}

#[test]
fn role_is_not_confused_with_kind() {
    let role = UfoStereotypeOrSubtype::Role(Role {
        name: "TaxCreditClaimant".into(),
    });
    let json = serde_json::to_string(&role).unwrap();
    println!("Role serialized as: {json}");
    let back: UfoStereotypeOrSubtype = serde_json::from_str(&json).unwrap();
    match back {
        UfoStereotypeOrSubtype::Role(_) => {}
        other => panic!("Role round-tripped as {other:?} instead of Role — tag not respected"),
    }
}

#[test]
fn mode_is_not_confused_with_relator() {
    let mode = UfoStereotypeOrSubtype::Mode(Mode {
        name: "Eligibility".into(),
    });
    let json = serde_json::to_string(&mode).unwrap();
    println!("Mode serialized as: {json}");
    let back: UfoStereotypeOrSubtype = serde_json::from_str(&json).unwrap();
    match back {
        UfoStereotypeOrSubtype::Mode(_) => {}
        other => panic!("Mode round-tripped as {other:?} instead of Mode — tag not respected"),
    }
}

#[test]
fn subkind_carries_parent_and_tag() {
    let sub_kind = UfoStereotypeOrSubtype::SubKind(SubKind {
        parent: "Company".into(),
        name: "PtyLtd".into(),
    });
    let json = serde_json::to_string(&sub_kind).unwrap();
    let back: UfoStereotypeOrSubtype = serde_json::from_str(&json).unwrap();
    assert_eq!(sub_kind, back);
}
