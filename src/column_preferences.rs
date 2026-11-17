use crate::model::{Config, Session};
use rustgis_plugin::{Value, host, json};
use std::collections::BTreeMap;

fn key(session: &Session) -> Result<String, String> {
    serde_json::to_string(&(
        &session.context.path,
        &session.context.map,
        &session.context.source,
        &session.context.layer_name,
        &session.fields,
    ))
    .map_err(|e| e.to_string())
}

fn saved() -> Result<BTreeMap<String, Vec<String>>, String> {
    let settings = host::call("host.settings", json!({}))?;
    match settings.get("column-visibility") {
        Some(value) => serde_json::from_value(value.clone()).map_err(|e| e.to_string()),
        None => Ok(BTreeMap::new()),
    }
}

pub fn restore(session: &Session, config: &mut Config) -> Result<(), String> {
    if !config.visible_fields.is_empty() {
        return Ok(());
    }
    if let Some(names) = saved()?.get(&key(session)?) {
        if names.len() > 32
            || names
                .iter()
                .any(|name| !session.fields.iter().any(|f| &f.name == name))
        {
            return Err("Stored column choices do not match this layer".into());
        }
        let text = serde_json::to_string(names).map_err(|e| e.to_string())?;
        if text.len() > 4000 {
            return Err("Stored column choices exceed the host field limit".into());
        }
        config.visible_fields = text;
    }
    Ok(())
}

pub fn remember(session: &Session, names: Vec<String>) -> Result<Value, String> {
    let mut columns = saved()?;
    columns.insert(key(session)?, names);
    host::call(
        "host.setSettings",
        json!({"values":{"column-visibility":columns}}),
    )
}
