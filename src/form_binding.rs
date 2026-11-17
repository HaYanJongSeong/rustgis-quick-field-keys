use crate::model::{Error, Session};
use std::hash::{DefaultHasher, Hash, Hasher};

pub fn token(session: &Session, names: &[String]) -> Result<String, Error> {
    let current = session
        .captured
        .get(session.cursor)
        .ok_or_else(|| Error::Invalid("Invalid form target".into()))?;
    let revision = serde_json::to_string(&(
        &session.context,
        &session.fields,
        &session.identity,
        current,
        names,
    ))?;
    let mut hash = DefaultHasher::new();
    revision.hash(&mut hash);
    Ok(format!(
        "l{}r{}h{:x}",
        session.layer,
        current.row,
        hash.finish()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Context, Feature};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn session() -> Session {
        let captured = (0..2)
            .map(|row| Feature {
                row,
                properties: BTreeMap::from([("n".into(), json!(10))]),
                geometry: json!(null),
                id: json!(null),
            })
            .collect();
        Session {
            context: Context {
                name: "test".into(),
                path: json!(null),
                map: "map".into(),
                layer_name: "layer".into(),
                source: json!(null),
            },
            layer: 1,
            identity: "uid".into(),
            fields: vec![],
            count: 2,
            captured,
            cursor: 0,
            previous_view: None,
        }
    }

    #[test]
    fn changes_token_when_form_target_moves() -> Result<(), Error> {
        // Given
        let mut session = session();
        let names = vec!["n".into()];
        let original = token(&session, &names)?;
        session.cursor = 1;
        // When
        let moved = token(&session, &names)?;
        // Then
        assert_ne!(original, moved);
        Ok(())
    }

    #[test]
    fn changes_token_when_form_revision_changes() -> Result<(), Error> {
        // Given
        let mut session = session();
        let names = vec!["n".into()];
        let original = token(&session, &names)?;
        session
            .captured
            .first_mut()
            .ok_or_else(|| Error::Invalid("Empty fixture".into()))?
            .properties
            .insert("n".into(), json!(11));
        // When
        let changed = token(&session, &names)?;
        // Then
        assert_ne!(original, changed);
        Ok(())
    }
}
