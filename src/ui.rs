use crate::{
    model::{Config, Layer, Session},
    settings,
};
use rustgis_plugin::{Request, Value, host, json};
use std::collections::BTreeSet;

pub fn filter_fields(layer: Option<&Layer>) -> Vec<Value> {
    let mut sorts = vec![json!("row")];
    if let Some(layer) = layer {
        sorts.extend(
            layer
                .fields
                .iter()
                .filter(|f| f.name != "row")
                .take(255)
                .map(|f| json!(f.name)),
        );
    }
    vec![
        json!({"id":"sort","kind":"select","label":"Sort by","choices":sorts,"default":"row"}),
        json!({"id":"ascending","kind":"checkbox","label":"Ascending","default":true}),
        json!({"id":"mode","kind":"select","label":"Filter mode","choices":["Text contains","RustGIS SQL"],"default":"Text contains","help":"RustGIS SQL uses the host expression language, not full QGIS syntax. It matches only features drawn by the host, including its definition, time and display filters."}),
        json!({"id":"query","kind":"text","label":"Filter","help":"Text contains: substring search. RustGIS SQL: a WHERE condition, for example uid >= 2 AND status IS NOT NULL. Apply filter / reload to use it; source data and provider filters stay unchanged.","default":""}),
        json!({"id":"selected-only","kind":"checkbox","label":"Selected only","help":"Rebuild navigation from the selection instead of the whole layer","default":false}),
    ]
}

pub fn option<'a>(req: &'a Request, name: &str) -> Option<&'a Value> {
    req.panel.get("filter").and_then(|form| form.get(name))
}

pub fn is_control(id: &str) -> bool {
    matches!(
        id,
        "choose-layer" | "clear-filter" | "set-locks" | "rebuild-navigation"
    )
}

pub fn require_capture(req: &Request) -> Result<(), String> {
    if req
        .panel
        .get("session")
        .and_then(|s| s.get("payload"))
        .and_then(Value::as_str)
        .is_none_or(|s| s.trim().is_empty())
    {
        return Err("Choose a layer before navigating/editing.".into());
    }
    Ok(())
}

pub fn capture_controls(result: &mut Value, enabled: bool) {
    for id in [
        "first",
        "previous",
        "next",
        "last",
        "previous-view",
        "zoom-current",
        "attributes",
        "locks",
        "save",
        "visible-fields",
    ] {
        result["panel"]["items"][id]["enabled"] = json!(enabled);
    }
    for id in ["attributes", "locks"] {
        result["panel"]["items"][id]["visible"] = json!(enabled);
    }
    if !enabled {
        result["panel"]["items"]["status"]["text"] = json!("Choose a layer to prepare editing.");
        result["panel"]["items"]["undo-edit"]["enabled"] = json!(false);
    }
}

pub fn state(req: &Request) -> Result<Value, String> {
    let layers = settings::layers()?;
    let config = settings::config(req)?;
    let value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
    let fields: Vec<_> = value
        .as_object()
        .ok_or("Invalid configuration")?
        .iter()
        .map(|(name, v)| json!({"id":name,"kind":"text","label":name,"default":v}))
        .collect();
    let mut options = vec![json!({"id":"__choose__","label":"Choose editing layer…"})];
    options.extend(
        layers
            .iter()
            .filter(|l| !l.raster)
            .take(999)
            .map(|l| json!({"id":l.id.to_string(),"label":l.name})),
    );
    let selected = settings::layer(&layers, &config.layer).ok();
    let mut result = json!({"host":host::call("host.info",json!({}))?,"layers":host::layers()?,"capture":req.panel.get("session"),
    "report":"Choose the editing layer, configure Values, then use assigned shortcuts. Save publishes all pending layer edits to its source.",
    "panel":{"values":{"layer":if config.layer.is_empty() {"__choose__"}else{&config.layer}},"items":{
        "columns-open":{"visible":false},
        "edit-history":{"visible":false,"fields":[{"id":"payload","kind":"text","label":"Internal edit history","default":""}]},
        "layer":{"options":options},
        "filter":{"fields":filter_fields(selected)},
        "view":{"fields":[{"id":"zoom-mode","kind":"select","label":"Zoom mode","choices":["feature","keep-scale","fixed-scale","off"],"default":"off","help":"feature: fit object; keep-scale: center without changing scale; fixed-scale: center at the scale below; off: leave the view unchanged"},{"id":"scale","kind":"number","label":"Fixed scale 1:","integer":true,"default":1000,"help":"Used only in fixed-scale mode. Scale denominator from 1 to 100000000."}]},
        "config":{"visible":false,"fields":fields},
        "session":{"visible":false,"fields":[{"id":"payload","kind":"text","label":"Internal capture","default":""}]}
    }}});
    crate::columns::visibility(&mut result, req.panel_bool("columns-open").unwrap_or(false));
    for id in [
        "filter",
        "view",
        "rebuild-navigation",
        "clear-filter",
        "zoom-current",
    ] {
        result["panel"]["items"][id]["visible"] = json!(true);
    }
    capture_controls(&mut result, require_capture(req).is_ok());
    if req
        .panel
        .get("config")
        .and_then(|v| v.get("layer"))
        .is_none()
    {
        result["panel"]["values"]["config"] = value;
    }
    if let Some(payload) = req
        .panel
        .get("session")
        .and_then(|s| s.get("payload"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        let session: Session = serde_json::from_str(payload).map_err(|e| e.to_string())?;
        decorate(&mut result, &session, &config)?;
        crate::history_ui::decorate(&mut result, req, &session)?;
    }
    Ok(result)
}

pub fn decorate(result: &mut Value, session: &Session, config: &Config) -> Result<(), String> {
    result["panel"]["items"]["save-note"] =
        json!({"text":"Press Save to apply changes to the original file."});
    let current = session
        .captured
        .get(session.cursor)
        .ok_or("Invalid cursor")?;
    let locks: BTreeSet<String> = serde_json::from_str(&config.locks).map_err(|e| e.to_string())?;
    let mut fields = Vec::new();
    let mut lock_fields = Vec::new();
    let mut values = serde_json::Map::new();
    let mut lock_values = serde_json::Map::new();
    let names = settings::chosen(config)?;
    let token = crate::form_binding::token(session, &names).map_err(|e| e.to_string())?;
    for (index, name) in names.iter().enumerate() {
        let field = session
            .fields
            .iter()
            .find(|f| &f.name == name)
            .ok_or("Configured field missing")?;
        let value = current
            .properties
            .get(name)
            .ok_or("Current field missing")?;
        let (kind, default) = match field.kind.as_str() {
            "Text" => ("text", json!("")),
            "Integer" | "Long"
                if value
                    .as_i64()
                    .is_some_and(|n| n.unsigned_abs() > 9_007_199_254_740_992) =>
            {
                ("text", json!(""))
            }
            "Integer" | "Long" | "Double" => ("number", json!(0)),
            "Boolean" => ("checkbox", json!(false)),
            _ => return Err(format!("Unsupported input field {name}")),
        };
        let shown = if value.is_null() {
            default
        } else if kind == "text" && value.is_number() {
            json!(value.to_string())
        } else {
            value.clone()
        };
        fields.push(json!({"id":format!("{token}-v{index}"),"label":name,"kind":kind,"integer":matches!(field.kind.as_str(),"Integer"|"Long"),"default":shown}));
        fields.push(json!({"id":format!("{token}-n{index}"),"label":format!("{name}: NULL"),"kind":"checkbox","default":value.is_null()}));
        values.insert(format!("{token}-v{index}"), shown);
        values.insert(format!("{token}-n{index}"), json!(value.is_null()));
        lock_fields.push(json!({"id":format!("lock{index}"),"label":format!("Lock {name}"),"kind":"checkbox","default":false}));
        lock_values.insert(
            format!("lock{index}"),
            json!(locks.contains(name) || name == &config.identity),
        );
    }
    result["panel"]["items"]["attributes"] = json!({"fields":fields});
    result["panel"]["items"]["locks"] = json!({"fields":lock_fields});
    if !names.is_empty() {
        result["panel"]["values"]["attributes"] = Value::Object(values);
        result["panel"]["values"]["locks"] = Value::Object(lock_values);
    }
    capture_controls(result, true);
    result["panel"]["items"]["previous-view"]["enabled"] = json!(session.previous_view.is_some());
    if names.is_empty() {
        for id in ["attributes", "locks"] {
            result["panel"]["items"][id]["visible"] = json!(false);
        }
    }
    crate::columns::render(result, session, config)?;
    Ok(())
}

pub fn controls(req: &Request) -> Result<Value, String> {
    match req.id.as_str() {
        "clear-filter" => {
            let mut filter = req.panel.get("filter").cloned().unwrap_or(json!({}));
            filter["query"] = json!("");
            Ok(
                json!({"panel":{"values":{"filter":filter}},"report":"Capture filter cleared. Capture again to change membership."}),
            )
        }
        "choose-layer" => crate::target::choose(req),
        "rebuild-navigation" => crate::target::prepare(req, settings::config(req)?),
        "set-locks" => {
            let mut config = settings::config(req)?;
            let names = settings::chosen(&config)?;
            let input = req.panel.get("locks").ok_or("No lock controls")?;
            let mut locks: BTreeSet<String> =
                serde_json::from_str(&config.locks).map_err(|e| e.to_string())?;
            for (i, name) in names.iter().enumerate() {
                match input.get(format!("lock{i}")).and_then(Value::as_bool) {
                    Some(true) => {
                        locks.insert(name.clone());
                    }
                    Some(false) => {
                        locks.remove(name);
                    }
                    None => {
                        return Err(
                            "Field lock controls changed; refresh the current columns".into()
                        );
                    }
                }
            }
            config.locks = serde_json::to_string(&locks).map_err(|e| e.to_string())?;
            Ok(
                json!({"report":"Plugin field locks updated; no data saved.","panel":{"values":{"config":config}}}),
            )
        }
        _ => Err("Unknown control".into()),
    }
}
