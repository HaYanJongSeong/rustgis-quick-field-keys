use crate::{
    edit_history::History,
    model::{self, Config, Session},
    settings,
};
use rustgis_plugin::{Request, Value, json};
use std::collections::BTreeMap;

pub fn prepare(
    req: &mut Request,
    session: &mut Session,
    config: &Config,
) -> Result<BTreeMap<String, Value>, String> {
    let text = req
        .panel
        .get("edit-history")
        .and_then(|v| v.get("payload"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut history = History::decode(text, session).map_err(|e| e.to_string())?;
    let mut values = match req.id.as_str() {
        "undo-edit" => {
            let (cursor, target) = history.last_target(session).map_err(|e| e.to_string())?;
            let values = history.undo_values(target).map_err(|e| e.to_string())?;
            session.cursor = cursor;
            values
        }
        "edit-current" => settings::edit_values(req, session, config)?,
        command => {
            let slot = command
                .strip_prefix("apply-")
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|n| n.checked_sub(1))
                .filter(|n| *n < 9)
                .ok_or("Unknown command")?;
            config.preset(slot).map_err(|e| e.to_string())?
        }
    };
    let current = session
        .captured
        .get(session.cursor)
        .ok_or("Invalid cursor")?;
    if req.id == "edit-current" {
        values.retain(|name, value| current.properties.get(name) != Some(value));
        if values.is_empty() {
            return Ok(values);
        }
    }
    model::validate_values(config, &session.fields, &values).map_err(|e| e.to_string())?;
    if req.id == "undo-edit" {
        history
            .groups
            .pop()
            .ok_or("No Quick Field Keys edits to undo")?;
    } else {
        history
            .remember(current, &values, &session.identity)
            .map_err(|e| e.to_string())?;
    }
    let payload = history.encode().map_err(|e| e.to_string())?;
    req.panel
        .insert("edit-history".into(), json!({"payload":payload}));
    Ok(values)
}
