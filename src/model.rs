use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn validate_capture_size(payload: &str) -> Result<(), Error> {
    if payload.len() > 4000 {
        return Err(Error::Invalid(
            "Capture exceeds 4000 UTF-8 bytes; select fewer/smaller features".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Feature {
    pub row: usize,
    pub properties: BTreeMap<String, Value>,
    pub geometry: Value,
    #[serde(default)]
    pub id: Value,
}

#[derive(Debug, Deserialize)]
pub struct Collection {
    pub features: Vec<Feature>,
}

#[derive(Debug, Deserialize)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub raster: bool,
    pub features: usize,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Session {
    pub context: Context,
    pub layer: u64,
    pub identity: String,
    pub fields: Vec<Field>,
    pub count: usize,
    pub captured: Vec<Feature>,
    pub cursor: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_view: Option<crate::previous_view::PreviousView>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Context {
    pub name: String,
    pub path: Value,
    pub map: String,
    pub layer_name: String,
    pub source: Value,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: Value,
    #[serde(rename = "activeMap")]
    pub active_map: String,
    pub layers: Vec<Source>,
}

#[derive(Debug, Deserialize)]
pub struct Source {
    pub id: u64,
    pub source: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub layer: String,
    pub identity: String,
    #[serde(default)]
    pub visible_fields: String,
    pub locks: String,
    pub preset1: String,
    pub preset2: String,
    pub preset3: String,
    pub preset4: String,
    pub preset5: String,
    pub preset6: String,
    pub preset7: String,
    pub preset8: String,
    pub preset9: String,
}

impl Config {
    pub fn empty(layer: String) -> Self {
        Self {
            layer,
            identity: String::new(),
            visible_fields: String::new(),
            locks: "[]".into(),
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
    pub fn preset(&self, slot: usize) -> Result<BTreeMap<String, Value>, Error> {
        let presets = [
            &self.preset1,
            &self.preset2,
            &self.preset3,
            &self.preset4,
            &self.preset5,
            &self.preset6,
            &self.preset7,
            &self.preset8,
            &self.preset9,
        ];
        let text = presets
            .get(slot)
            .ok_or_else(|| Error::Invalid("Invalid preset".into()))?;
        Ok(serde_json::from_str(text)?)
    }
}

pub fn literal(field: &Field, text: &str, null: bool) -> Result<Value, Error> {
    if null
        && matches!(
            field.kind.as_str(),
            "Text" | "Integer" | "Long" | "Double" | "Boolean"
        )
    {
        return Ok(Value::Null);
    }
    let invalid = || Error::Invalid(format!("Invalid {} value for {}", field.kind, field.name));
    match field.kind.as_str() {
        "Text" => Ok(Value::String(text.into())),
        "Integer" | "Long" => text
            .trim()
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| invalid()),
        "Double" => text
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(invalid),
        "Boolean" => text
            .trim()
            .parse::<bool>()
            .map(Value::Bool)
            .map_err(|_| invalid()),
        _ => Err(invalid()),
    }
}

pub fn validate_values(
    config: &Config,
    fields: &[Field],
    values: &BTreeMap<String, Value>,
) -> Result<(), Error> {
    let locks: BTreeSet<String> = serde_json::from_str(&config.locks)?;
    if values.is_empty() {
        return Err(Error::Invalid("Preset is empty".into()));
    }
    for (name, value) in values {
        if locks.contains(name) || name == &config.identity {
            return Err(Error::Invalid(format!("Locked field: {name}")));
        }
        let field = fields
            .iter()
            .find(|f| &f.name == name)
            .ok_or_else(|| Error::Invalid(format!("Unknown field: {name}")))?;
        let valid = match (field.kind.as_str(), value) {
            ("Text", Value::String(_)) | ("Boolean", Value::Bool(_)) => true,
            ("Long" | "Integer", Value::Number(n)) => n.as_i64().is_some(),
            ("Double", Value::Number(n)) => n.as_f64().is_some_and(f64::is_finite),
            ("Text" | "Boolean" | "Long" | "Integer" | "Double", Value::Null) => true,
            _ => false,
        };
        if !valid {
            return Err(Error::Invalid(format!(
                "Unsupported value/type for {name}: {}",
                field.kind
            )));
        }
    }
    Ok(())
}

pub fn validate_identity(features: &[Feature], field: &str) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for feature in features {
        let value = feature
            .properties
            .get(field)
            .ok_or_else(|| Error::Invalid("Identity field missing".into()))?;
        if value.is_null()
            || !matches!(value, Value::String(_) | Value::Number(_))
            || !seen.insert(value.to_string())
        {
            return Err(Error::Invalid(
                "Identity must be non-null, unique text or number".into(),
            ));
        }
    }
    Ok(())
}

pub fn check_session(session: &Session, layer: &Layer, features: &[Feature]) -> Result<(), Error> {
    if session.layer != layer.id
        || session.fields != layer.fields
        || session.count != features.len()
        || session.captured.is_empty()
        || session.cursor >= session.captured.len()
    {
        return Err(Error::Invalid(
            "Layer/schema changed or invalid capture; capture again".into(),
        ));
    }
    validate_identity(features, &session.identity)?;
    for original in &session.captured {
        if features.iter().find(|f| f.row == original.row) != Some(original) {
            return Err(Error::Invalid(
                "Captured row changed or moved; capture again".into(),
            ));
        }
    }
    Ok(())
}
