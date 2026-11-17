use super::model::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn preserves_other_slots_when_one_preset_is_configured() {
    // Given
    let mut old = Config::empty("1".into());
    old.identity = "uid".into();
    old.preset1 = "{\"label\":\"kept\"}".into();
    let update = crate::presets::PresetUpdate {
        layer: "1".into(),
        identity: "uid".into(),
        slot: 2,
        values: BTreeMap::from([("rank".into(), json!(null))]),
    };
    // When
    let result = crate::presets::configure_slot(old, update);
    // Then
    assert!(
        matches!(result,Ok(c) if c.preset1=="{\"label\":\"kept\"}" && c.preset(1).is_ok_and(|v|v.get("rank")==Some(&json!(null))))
    );
}

#[test]
fn clears_foreign_slots_when_identity_changes() {
    // Given
    let mut old = Config::empty("1".into());
    old.identity = "uid".into();
    old.preset1 = "{\"label\":\"old\"}".into();
    let update = crate::presets::PresetUpdate {
        layer: "1".into(),
        identity: "other".into(),
        slot: 2,
        values: BTreeMap::new(),
    };
    // When
    let result = crate::presets::configure_slot(old, update);
    // Then
    assert!(matches!(result,Ok(c) if c.preset1=="{}" && c.preset2=="{}"));
}

#[test]
fn clears_foreign_slots_when_target_layer_changes() {
    // Given
    let mut old = Config::empty("1".into());
    old.identity = "uid".into();
    old.preset1 = "{\"label\":\"old\"}".into();
    let update = crate::presets::PresetUpdate {
        layer: "2".into(),
        identity: "uid".into(),
        slot: 2,
        values: BTreeMap::new(),
    };
    // When
    let result = crate::presets::configure_slot(old, update);
    // Then
    assert!(matches!(result,Ok(c) if c.layer=="2" && c.preset1=="{}" && c.preset2=="{}"));
}

#[test]
fn rejects_oversized_slot_when_settings_would_be_truncated() {
    // Given
    let old = Config::empty("1".into());
    let update = crate::presets::PresetUpdate {
        layer: "1".into(),
        identity: "uid".into(),
        slot: 1,
        values: BTreeMap::from([("label".into(), json!("x".repeat(4000)))]),
    };
    // When
    let result = crate::presets::configure_slot(old, update);
    // Then
    assert!(result.is_err());
}

#[test]
fn parses_integer_when_native_settings_supplies_text() {
    // Given
    let field = Field {
        name: "rank".into(),
        kind: "Integer".into(),
    };
    // When
    let result = literal(&field, "42", false);
    // Then
    assert!(matches!(result,Ok(value) if value==json!(42)));
}

#[test]
fn rejects_nonfinite_when_native_settings_supplies_double() {
    // Given
    let field = Field {
        name: "rank".into(),
        kind: "Double".into(),
    };
    // When
    let result = literal(&field, "NaN", false);
    // Then
    assert!(result.is_err());
}

#[test]
fn freezes_filtered_descending_order_when_capturing() {
    // Given
    let rows = (0..3)
        .map(|row| Feature {
            row,
            properties: BTreeMap::from([
                ("name".into(), json!(if row == 0 { "skip" } else { "keep" })),
                ("rank".into(), json!(row)),
            ]),
            geometry: json!(null),
            id: json!(null),
        })
        .collect();
    // When
    let captured = crate::navigation::capture_order(rows, "keep", "rank", false);
    // Then
    assert_eq!(
        captured.iter().map(|f| f.row).collect::<Vec<_>>(),
        vec![2, 1]
    );
}

#[test]
fn accepts_capture_when_exactly_at_host_safe_byte_limit() {
    // Given
    let payload = "a".repeat(4000);
    // When
    let result = validate_capture_size(&payload);
    // Then
    assert!(result.is_ok());
}

#[test]
fn rejects_capture_when_one_byte_above_host_safe_limit() {
    // Given
    let payload = "a".repeat(4001);
    // When
    let result = validate_capture_size(&payload);
    // Then
    assert!(result.is_err());
}

#[test]
fn rejects_capture_when_multibyte_text_exceeds_byte_limit() {
    // Given
    let payload = "한".repeat(1334);
    // When
    let result = validate_capture_size(&payload);
    // Then
    assert!(result.is_err());
}

fn config() -> Config {
    Config {
        layer: "a".into(),
        identity: "id".into(),
        visible_fields: String::new(),
        locks: "[\"locked\"]".into(),
        preset1: "{}".into(),
        preset2: "{}".into(),
        preset3: "{}".into(),
        preset4: "{}".into(),
        preset5: "{}".into(),
        preset6: "{}".into(),
        preset7: "{}".into(),
        preset8: "{}".into(),
        preset9: "{}".into(),
    }
}

#[test]
fn rejects_wrong_types_when_values_cross_boundary() {
    // Given
    let fields = vec![Field {
        name: "n".into(),
        kind: "Long".into(),
    }];
    let values = BTreeMap::from([("n".into(), json!(1.5))]);
    // When
    let result = validate_values(&config(), &fields, &values);
    // Then
    assert!(result.is_err());
}

#[test]
fn accepts_null_when_supported_scalar_field() {
    // Given
    let fields = vec![Field {
        name: "n".into(),
        kind: "Long".into(),
    }];
    let values = BTreeMap::from([("n".into(), json!(null))]);
    // When
    let result = validate_values(&config(), &fields, &values);
    // Then
    assert!(result.is_ok());
}

#[test]
fn rejects_locked_field_when_preset_names_it() {
    // Given
    let fields = vec![Field {
        name: "locked".into(),
        kind: "Text".into(),
    }];
    let values = BTreeMap::from([("locked".into(), json!("changed"))]);
    // When
    let result = validate_values(&config(), &fields, &values);
    // Then
    assert!(result.is_err());
}

#[test]
fn rejects_duplicate_identity_when_capturing() {
    // Given
    let feature = Feature {
        row: 0,
        properties: BTreeMap::from([("id".into(), json!(7))]),
        geometry: json!(null),
        id: json!(null),
    };
    let mut second = feature.clone();
    second.row = 1;
    // When
    let result = validate_identity(&[feature, second], "id");
    // Then
    assert!(result.is_err());
}

#[test]
fn rejects_changed_row_when_capture_is_stale() {
    // Given
    let feature = Feature {
        row: 0,
        properties: BTreeMap::from([("id".into(), json!(7))]),
        geometry: json!(null),
        id: json!(null),
    };
    let layer = Layer {
        id: 1,
        name: "A".into(),
        raster: false,
        features: 1,
        fields: vec![],
    };
    let session = Session {
        context: Context {
            name: "test".into(),
            path: json!(null),
            map: "map".into(),
            layer_name: "A".into(),
            source: json!(null),
        },
        layer: 1,
        identity: "id".into(),
        fields: vec![],
        count: 1,
        captured: vec![feature.clone()],
        cursor: 0,
        previous_view: None,
    };
    let mut changed = feature;
    changed.properties.insert("id".into(), json!(8));
    // When
    let result = check_session(&session, &layer, &[changed]);
    // Then
    assert!(result.is_err());
}
