use crate::model::{self, Config, Layer, Session};
use crate::presets::{PresetUpdate, configure_slot};
use rustgis_plugin::{Request, Value, host, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn config(req: &Request) -> Result<Config, String> {
    match req.panel.get("config").filter(|v| v.get("layer").is_some()) {
        Some(value) => serde_json::from_value(value.clone()).map_err(|e| e.to_string()),
        None => Ok(Config::empty(String::new())),
    }
}

pub fn layers() -> Result<Vec<Layer>, String> {
    serde_json::from_value(host::layers()?).map_err(|e| e.to_string())
}

pub fn layer<'a>(layers: &'a [Layer], name: &str) -> Result<&'a Layer, String> {
    let matches: Vec<_> = layers
        .iter()
        .filter(|l| !l.raster && (l.name == name || Some(l.id) == name.parse::<u64>().ok()))
        .collect();
    match matches.as_slice() {
        [layer] => Ok(layer),
        _ => Err("Choose an unambiguous vector layer".into()),
    }
}

pub fn chosen(config: &Config) -> Result<Vec<String>, String> {
    if !config.visible_fields.is_empty() {
        return serde_json::from_str(&config.visible_fields).map_err(|e| e.to_string());
    }
    let mut names = BTreeSet::new();
    for slot in 0..9 {
        names.extend(config.preset(slot).map_err(|e| e.to_string())?.into_keys());
    }
    Ok(names.into_iter().collect())
}

pub fn configure(req: &Request) -> Result<Value, String> {
    let name = req
        .str("layer")
        .ok_or("Choose a layer in Preset Settings")?;
    let layers = layers()?;
    let layer = layer(&layers, name)?;
    if req
        .panel_str("layer")
        .is_some_and(|target| target != layer.id.to_string() && target != layer.name)
    {
        return Err("Choose the editing layer in the top layer selector first".into());
    }
    let identity = req
        .str("identity")
        .filter(|s| !s.is_empty())
        .ok_or("Choose a unique identity field")?;
    if !layer.fields.iter().any(|f| f.name == identity) {
        return Err("Identity field missing".into());
    }
    let mut old = config(req)?;
    if old.layer == name {
        old.layer = layer.id.to_string();
    }
    let slot = req
        .str("slot")
        .ok_or("Choose a preset slot")?
        .parse::<usize>()
        .map_err(|_| "Invalid preset slot")?;
    let mut values = BTreeMap::new();
    if !req.bool("unassign").unwrap_or(false) {
        let field_name = req
            .str("field")
            .filter(|s| !s.is_empty())
            .ok_or("Choose a field, or enable Unassign this slot")?;
        let field = layer
            .fields
            .iter()
            .find(|f| f.name == field_name)
            .ok_or("Preset field missing")?;
        let text = req.str("value").unwrap_or_default();
        let null = req.bool("null").unwrap_or(false);
        let literal = model::literal(field, text, null).map_err(|e| e.to_string())?;
        values.insert(field_name.to_string(), literal);
    }
    let cfg = configure_slot(
        old,
        PresetUpdate {
            layer: layer.id.to_string(),
            identity: identity.into(),
            slot,
            values,
        },
    )
    .map_err(|e| e.to_string())?;
    let mut value = serde_json::to_value(&cfg).map_err(|e| e.to_string())?;
    let locks: Vec<String> = serde_json::from_str(&cfg.locks).map_err(|e| e.to_string())?;
    if locks
        .iter()
        .any(|f| !layer.fields.iter().any(|l| &l.name == f))
    {
        value["locks"] = json!("[]");
    }
    let config = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let mut result = crate::target::prepare(req, config)?;
    result["report"] = json!(format!(
        "Preset {slot} updated. Editing target ready; nothing saved."
    ));
    Ok(result)
}

pub fn options(req: &Request) -> Result<Value, String> {
    let config = config(req)?;
    let identity = req
        .str("identity")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&config.identity);
    let report = "Keyboard shortcuts: RustGIS Settings > Keyboard Shortcuts. The plugin cannot open that editor with the current SDK; no preferences are changed.";
    if identity == config.identity {
        return Ok(json!({"report":report,"identity":config.identity}));
    }
    if req.bool("reset-presets") != Some(true) {
        return Err(
            "Changing identity clears presets and field locks; confirm the reset first".into(),
        );
    }
    let layers = layers()?;
    let layer = layer(&layers, &config.layer)?;
    if req.panel_str("layer") != Some(layer.id.to_string().as_str()) {
        return Err("Choose the editing layer in the top selector first".into());
    }
    if !layer.fields.iter().any(|f| f.name == identity) {
        return Err("Identity field missing".into());
    }
    let mut updated = Config::empty(config.layer.clone());
    updated.identity = identity.into();
    let mut result = crate::target::prepare(req, updated)?;
    result["report"] = json!(format!(
        "Identity updated; presets and locks reset. {report}"
    ));
    Ok(result)
}

pub fn edit_values(
    req: &Request,
    session: &Session,
    config: &Config,
) -> Result<BTreeMap<String, Value>, String> {
    let input = req.panel.get("attributes").ok_or("No attribute inputs")?;
    let mut values = BTreeMap::new();
    let names = chosen(config)?;
    let token = crate::form_binding::token(session, &names).map_err(|e| e.to_string())?;
    for (index, name) in names.iter().enumerate() {
        let field = session
            .fields
            .iter()
            .find(|f| &f.name == name)
            .ok_or("Configured field missing")?;
        let value = input
            .get(format!("{token}-v{index}"))
            .ok_or("Attribute form target changed; confirm the current row before editing")?;
        let text = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) if matches!(field.kind.as_str(), "Integer" | "Long") => {
                match n.as_i64() {
                    Some(n) => n.to_string(),
                    None => {
                        let n = n
                            .as_f64()
                            .filter(|n| n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15)
                            .ok_or("Invalid whole number")?;
                        format!("{n:.0}")
                    }
                }
            }
            other => other.to_string(),
        };
        let null = input
            .get(format!("{token}-n{index}"))
            .and_then(Value::as_bool)
            .ok_or("Attribute form target changed")?;
        values.insert(
            name.clone(),
            model::literal(field, &text, null).map_err(|e| e.to_string())?,
        );
    }
    Ok(values)
}
