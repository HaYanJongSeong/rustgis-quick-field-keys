use crate::{edit_history::History, model::Session};
use rustgis_plugin::{Request, Value, json};

pub fn clear(result: &mut Value) {
    result["panel"]["values"]["edit-history"] = json!({"payload":""});
    result["panel"]["items"]["undo-edit"]["enabled"] = json!(false);
}

pub fn decorate(result: &mut Value, req: &Request, session: &Session) -> Result<(), String> {
    if req.id == "capture" {
        clear(result);
        return Ok(());
    }
    let text = req
        .panel
        .get("edit-history")
        .and_then(|v| v.get("payload"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let history = History::decode(text, session).map_err(|e| e.to_string())?;
    result["panel"]["values"]["edit-history"] = json!({"payload":text});
    result["panel"]["items"]["undo-edit"]["enabled"] = json!(!history.groups.is_empty());
    Ok(())
}
