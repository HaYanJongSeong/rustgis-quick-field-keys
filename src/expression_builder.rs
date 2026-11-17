use crate::{
    settings,
    sql_builder::{Condition, Join},
    ui,
};
use rustgis_plugin::{Request, Value, json};

pub fn build(req: &Request) -> Result<Value, String> {
    ui::require_capture(req)?;
    let config = settings::config(req)?;
    let layers = settings::layers()?;
    let target = settings::layer(&layers, &config.layer)?;
    let selected = settings::layer(&layers, req.str("layer").ok_or("Choose the editing layer")?)?;
    if target.id != selected.id || req.panel_str("layer") != Some(target.id.to_string().as_str()) {
        return Err("Expression layer must match the editing layer at the top of the panel".into());
    }
    let field = target
        .fields
        .iter()
        .find(|f| Some(f.name.as_str()) == req.str("field"))
        .ok_or("Choose a field of the editing layer")?;
    let condition: Condition = serde_json::from_value(json!({
        "operator":req.str("operator").ok_or("Choose an operator")?,
        "value":req.str("value").unwrap_or_default(),
        "combine":req.str("combine").unwrap_or("Replace")
    }))
    .map_err(|e| e.to_string())?;
    let mut filter = req
        .panel
        .get("filter")
        .cloned()
        .ok_or("Open the panel first")?;
    let old = filter
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(condition.combine, Join::Replace)
        && !old.trim().is_empty()
        && filter.get("mode").and_then(Value::as_str) != Some("RustGIS SQL")
    {
        return Err("Use Replace to start SQL instead of combining a text-contains filter".into());
    }
    let query = condition
        .expression(field, old)
        .map_err(|e| e.to_string())?;
    filter["query"] = json!(query);
    filter["mode"] = json!("RustGIS SQL");
    Ok(
        json!({"expression":query,"report":format!("Filter input updated: {query}\nPress ↻ to apply the filter and rebuild navigation. Values and Undo history are unchanged until reload."),
        "panel":{"values":{"filter":filter}}}),
    )
}
