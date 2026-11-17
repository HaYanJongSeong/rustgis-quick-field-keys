use crate::{
    model::{self, Collection, Config, Session},
    navigation, runtime, settings, ui,
};
use rustgis_plugin::{Request, Value, host, json};

pub fn snapshot(req: &Request, mut session: Session) -> Result<Session, String> {
    model::validate_identity(&session.captured, &session.identity).map_err(|e| e.to_string())?;
    let selected = match ui::option(req, "selected-only").and_then(Value::as_bool) {
        Some(true) => {
            let collection: Collection = serde_json::from_value(host::call(
                "host.features",
                json!({"layer":session.layer,"selected":true,"limit":10001}),
            )?)
            .map_err(|e| e.to_string())?;
            collection.features
        }
        _ => std::mem::take(&mut session.captured),
    };
    let matched = crate::expression_filter::matches(req, session.layer, &selected)?;
    let query = match ui::option(req, "mode").and_then(Value::as_str) {
        Some("RustGIS SQL") => "",
        _ => ui::option(req, "query")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    };
    session.captured = navigation::capture_order(
        selected
            .into_iter()
            .filter(|f| matched.as_ref().is_none_or(|rows| rows.contains(&f.row)))
            .collect(),
        query,
        ui::option(req, "sort")
            .and_then(Value::as_str)
            .unwrap_or("row"),
        ui::option(req, "ascending")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    );
    if session.captured.is_empty() {
        return Err("No matching features; change the navigation filter or selection".into());
    }
    Ok(session)
}

pub fn prepare(req: &Request, config: Config) -> Result<Value, String> {
    let mut request = req.clone();
    request.id = "capture".into();
    request.panel.insert("config".into(), json!(config));
    let mut result = runtime::handle(request)?;
    let layers = settings::layers()?;
    let selected = settings::layer(&layers, &config.layer)?;
    result["panel"]["values"]["layer"] = json!(selected.id.to_string());
    result["panel"]["items"]["filter"]["fields"] = json!(ui::filter_fields(Some(selected)));
    Ok(result)
}

pub fn choose(req: &Request) -> Result<Value, String> {
    let layer = req.panel_str("layer").ok_or("Choose a layer")?;
    let layers = settings::layers()?;
    let selected = settings::layer(&layers, layer)?;
    let old = settings::config(req)?;
    if old.layer == selected.id.to_string() && ui::require_capture(req).is_ok() {
        // Reselecting the same target must not replace a pending field draft.
        return Ok(json!({"panel":{"values":{"layer":old.layer}}}));
    }
    let config = if old.layer == selected.id.to_string() {
        old
    } else {
        Config::empty(selected.id.to_string())
    };
    let mut request = req.clone();
    let filter = json!({"sort":"row","ascending":true,"selected-only":false,"mode":"Text contains","query":""});
    request.panel.insert("filter".into(), filter.clone());
    let mut result = prepare(&request, config)?;
    result["panel"]["values"]["filter"] = filter;
    Ok(result)
}
