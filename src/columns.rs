use crate::{
    form_binding,
    model::{Config, Session},
    settings, ui,
};
use rustgis_plugin::{Request, Value, json};
use std::collections::BTreeSet;

pub fn is_command(id: &str) -> bool {
    matches!(
        id,
        "visible-fields"
            | "set-visible-fields"
            | "columns-all"
            | "columns-none"
            | "apply-visible-fields"
            | "cancel-visible-fields"
    )
}

fn names(session: &Session) -> Result<Vec<String>, String> {
    let fields: Vec<_> = session
        .fields
        .iter()
        .filter(|f| {
            matches!(
                f.kind.as_str(),
                "Text" | "Integer" | "Long" | "Double" | "Boolean"
            )
        })
        .map(|f| f.name.clone())
        .collect();
    if fields.len() > 64 {
        return Err("The host column chooser supports at most 64 scalar fields".into());
    }
    Ok(fields)
}

pub fn render(result: &mut Value, session: &Session, config: &Config) -> Result<(), String> {
    let names = match names(session) {
        Ok(names) => names,
        Err(error) => {
            result["panel"]["items"]["visible-fields"] = json!({"enabled":false,"help":error});
            result["panel"]["items"]["columns"] = json!({"visible":false});
            return Ok(());
        }
    };
    let token = form_binding::token(session, &names).map_err(|e| e.to_string())?;
    let chosen: BTreeSet<_> = settings::chosen(config)?.into_iter().collect();
    let fields: Vec<_> = names.iter().enumerate().map(|(i, name)|
        json!({"id":format!("{token}-c{i}"),"kind":"checkbox","label":name,
            "default":chosen.contains(name),"help":"Show or hide this column in the editor. Does not change its data or preset assignment."})
    ).collect();
    let values: serde_json::Map<String, Value> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (format!("{token}-c{i}"), json!(chosen.contains(name))))
        .collect();
    result["panel"]["items"]["columns"]["fields"] = json!(fields);
    result["panel"]["values"]["columns"] = json!(values);
    Ok(())
}

pub fn visibility(result: &mut Value, open: bool) {
    for id in [
        "columns",
        "columns-help",
        "columns-all",
        "columns-none",
        "apply-visible-fields",
        "cancel-visible-fields",
    ] {
        result["panel"]["items"][id]["visible"] = json!(open);
    }
    result["panel"]["values"]["columns-open"] = json!(open);
}

pub fn handle(req: &Request) -> Result<Value, String> {
    ui::require_capture(req)?;
    let mut config = settings::config(req)?;
    let session: Session = serde_json::from_str(
        req.panel
            .get("session")
            .and_then(|v| v.get("payload"))
            .and_then(Value::as_str)
            .ok_or("Choose the editing layer first")?,
    )
    .map_err(|e| e.to_string())?;
    if req.panel_str("layer") != Some(session.layer.to_string().as_str())
        || config.layer != session.layer.to_string()
    {
        return Err(
            "Column choices belong to another layer; select the editing layer first".into(),
        );
    }
    if matches!(req.id.as_str(), "visible-fields" | "cancel-visible-fields") {
        let open = req.id == "visible-fields" && !req.panel_bool("columns-open").unwrap_or(false);
        let mut result = json!({});
        render(&mut result, &session, &config)?;
        visibility(&mut result, open);
        return Ok(result);
    }
    let names = names(&session)?;
    let token = form_binding::token(&session, &names).map_err(|e| e.to_string())?;
    if matches!(req.id.as_str(), "columns-all" | "columns-none") {
        let checked = req.id == "columns-all";
        let values: serde_json::Map<String, Value> = names
            .iter()
            .enumerate()
            .map(|(i, _)| (format!("{token}-c{i}"), json!(checked)))
            .collect();
        return Ok(json!({"panel":{"values":{"columns":values}}}));
    }
    let input = req.panel.get("columns").ok_or("No column choices")?;
    let mut selected = Vec::new();
    for (i, name) in names.iter().enumerate() {
        match input.get(format!("{token}-c{i}")).and_then(Value::as_bool) {
            Some(true) => selected.push(name.clone()),
            Some(false) => {}
            None => {
                return Err(
                    "Column choices are stale; reopen the chooser for the current layer".into(),
                );
            }
        }
    }
    if req.id == "set-visible-fields" {
        return Ok(
            json!({"report":"Column choices pending: Apply to use them, or Cancel to keep the previous display"}),
        );
    }
    config.visible_fields = serde_json::to_string(&selected).map_err(|e| e.to_string())?;
    if selected.len() > 32 {
        return Err("Show at most 32 columns at once: each uses a value and NULL control".into());
    }
    if config.visible_fields.len() > 4000 {
        return Err("Column selection exceeds the host's 4000-byte field limit".into());
    }
    let mut result = json!({"panel":{"values":{"config":config}},
        "report":"Visible columns updated. Values, presets and source files unchanged."});
    ui::decorate(&mut result, &session, &config)?;
    visibility(&mut result, false);
    crate::column_preferences::remember(&session, selected)?;
    Ok(result)
}
