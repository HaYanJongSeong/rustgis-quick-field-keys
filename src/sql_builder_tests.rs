use super::*;

fn field(name: &str, kind: &str) -> Field {
    Field {
        name: name.into(),
        kind: kind.into(),
    }
}

#[test]
fn quotes_identifiers_and_text_when_building_comparisons() -> Result<(), Error> {
    // Given
    let condition = Condition {
        operator: Operator::Equal,
        value: "O'Brien' OR 1=1".into(),
        combine: Join::Replace,
    };
    // When
    let query = condition.expression(&field("a\"b", "Text"), "ignored")?;
    // Then
    assert_eq!(query, "\"a\"\"b\" = 'O''Brien'' OR 1=1'");
    Ok(())
}

#[test]
fn formats_typed_values_when_building_comparisons() -> Result<(), Error> {
    // Given
    for (kind, value, expected) in [
        ("Integer", " 12 ", "12"),
        ("Double", "1.5", "1.5"),
        ("Boolean", "true", "TRUE"),
    ] {
        let condition = Condition {
            operator: Operator::GreaterEqual,
            value: value.into(),
            combine: Join::Replace,
        };
        // When
        let query = condition.expression(&field("value", kind), "")?;
        // Then
        assert_eq!(query, format!("\"value\" >= {expected}"));
    }
    Ok(())
}

#[test]
fn preserves_precedence_when_appending_conditions() -> Result<(), Error> {
    // Given
    let condition = Condition {
        operator: Operator::IsNull,
        value: "ignored".into(),
        combine: Join::And,
    };
    // When
    let query = condition.expression(&field("status", "Text"), "a = 1 OR b = 2")?;
    // Then
    assert_eq!(query, "(a = 1 OR b = 2) AND (\"status\" IS NULL)");
    Ok(())
}

#[test]
fn rejects_invalid_literals_and_pattern_types() {
    // Given
    for (operator, kind, value) in [
        (Operator::Equal, "Integer", "1 OR 1=1"),
        (Operator::Equal, "Double", "NaN"),
        (Operator::Like, "Integer", "%"),
    ] {
        let condition = Condition {
            operator,
            value: value.into(),
            combine: Join::Replace,
        };
        // When
        let query = condition.expression(&field("value", kind), "");
        // Then
        assert!(query.is_err());
    }
}

#[test]
fn rejects_expression_over_byte_budget() {
    // Given
    let condition = Condition {
        operator: Operator::Equal,
        value: "가".repeat(1400),
        combine: Join::Replace,
    };
    // When
    let query = condition.expression(&field("value", "Text"), "");
    // Then
    assert!(query.is_err());
}
