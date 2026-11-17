use crate::model::{Error, Feature, Session};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Group {
    pub row: usize,
    pub id: Value,
    pub identity: Value,
    pub before: BTreeMap<String, Value>,
    pub after: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct History {
    pub target: String,
    pub groups: Vec<Group>,
}

impl History {
    pub fn binding(session: &Session) -> Result<String, Error> {
        Ok(serde_json::to_string(&(
            &session.context,
            session.layer,
            &session.identity,
            &session.fields,
            session.count,
        ))?)
    }

    pub fn decode(text: &str, session: &Session) -> Result<Self, Error> {
        let target = Self::binding(session)?;
        if text.is_empty() {
            return Ok(Self {
                target,
                groups: Vec::new(),
            });
        }
        let history: Self = serde_json::from_str(text)?;
        if history.target != target {
            return Err(Error::Invalid(
                "Undo history belongs to another target or schema; rebuild navigation".into(),
            ));
        }
        Ok(history)
    }

    pub fn encode(&self) -> Result<String, Error> {
        let text = serde_json::to_string(self)?;
        if text.len() > 4000 {
            return Err(Error::Invalid("Undo history exceeds 4000 bytes. Save or rebuild navigation to start a new history; rebuilding keeps edits but discards Undo history".into()));
        }
        Ok(text)
    }

    pub fn remember(
        &mut self,
        feature: &Feature,
        values: &BTreeMap<String, Value>,
        identity: &str,
    ) -> Result<(), Error> {
        let mut before = BTreeMap::new();
        let mut after = BTreeMap::new();
        for (name, value) in values {
            let old = feature
                .properties
                .get(name)
                .ok_or_else(|| Error::Invalid("Edited field disappeared".into()))?;
            if old != value {
                before.insert(name.clone(), old.clone());
                after.insert(name.clone(), value.clone());
            }
        }
        if after.is_empty() {
            return Ok(());
        }
        let identity = feature
            .properties
            .get(identity)
            .ok_or_else(|| Error::Invalid("Undo target has no identity".into()))?
            .clone();
        if let Some(last) = self.groups.last_mut().filter(|last| {
            last.row == feature.row && last.id == feature.id && last.identity == identity
        }) {
            if !last
                .after
                .iter()
                .all(|(key, value)| feature.properties.get(key) == Some(value))
            {
                return Err(Error::Invalid(
                    "An edited value changed outside this history; nothing was overwritten".into(),
                ));
            }
            for (key, value) in before {
                last.before.entry(key).or_insert(value);
            }
            last.after.extend(after);
        } else {
            self.groups.push(Group {
                row: feature.row,
                id: feature.id.clone(),
                identity,
                before,
                after,
            });
        }
        Ok(())
    }

    pub fn last_target<'a>(&self, session: &'a Session) -> Result<(usize, &'a Feature), Error> {
        let last = self
            .groups
            .last()
            .ok_or_else(|| Error::Invalid("No Quick Field Keys edits to undo".into()))?;
        session
            .captured
            .iter()
            .enumerate()
            .find(|(_, feature)| {
                feature.row == last.row
                    && feature.id == last.id
                    && feature.properties.get(&session.identity) == Some(&last.identity)
            })
            .ok_or_else(|| {
                Error::Invalid("Undo target disappeared or moved; nothing was reverted".into())
            })
    }

    pub fn undo_values(&self, feature: &Feature) -> Result<BTreeMap<String, Value>, Error> {
        let last = self
            .groups
            .last()
            .ok_or_else(|| Error::Invalid("No Quick Field Keys edits to undo".into()))?;
        if feature.row != last.row
            || feature.id != last.id
            || !last
                .after
                .iter()
                .all(|(key, value)| feature.properties.get(key) == Some(value))
        {
            return Err(Error::Invalid(
                "An edited value changed outside this history; nothing was reverted".into(),
            ));
        }
        Ok(last.before.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn feature() -> Feature {
        Feature {
            row: 2,
            id: json!(42),
            properties: [
                ("uid".into(), json!(42)),
                ("status".into(), json!("original")),
            ]
            .into(),
            geometry: Value::Null,
        }
    }

    #[test]
    fn merges_consecutive_edits_to_the_same_feature() -> Result<(), Error> {
        // Given
        let mut feature = feature();
        let mut history = History::default();
        history.remember(&feature, &[("status".into(), json!("first"))].into(), "uid")?;
        feature.properties.insert("status".into(), json!("first"));
        // When
        history.remember(
            &feature,
            &[("status".into(), json!("second"))].into(),
            "uid",
        )?;
        feature.properties.insert("status".into(), json!("second"));
        // Then
        assert_eq!(history.groups.len(), 1);
        assert_eq!(
            history.undo_values(&feature)?.get("status"),
            Some(&json!("original"))
        );
        Ok(())
    }

    #[test]
    fn refuses_to_revert_external_changes() -> Result<(), Error> {
        // Given
        let mut feature = feature();
        let mut history = History::default();
        history.remember(
            &feature,
            &[("status".into(), json!("plugin"))].into(),
            "uid",
        )?;
        // When
        feature
            .properties
            .insert("status".into(), json!("external"));
        // Then
        assert!(history.undo_values(&feature).is_err());
        assert_eq!(history.groups.len(), 1);
        Ok(())
    }

    #[test]
    fn refuses_history_over_the_host_byte_limit() {
        // Given
        let history = History {
            target: "가".repeat(1400),
            groups: Vec::new(),
        };
        // When
        let result = history.encode();
        // Then
        assert!(result.is_err());
    }
}
