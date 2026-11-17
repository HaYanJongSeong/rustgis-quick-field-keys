use crate::{
    model::{Project, Session},
    settings, target, ui,
};
use rustgis_plugin::{Request, Value, host, json};
use serde::Deserialize;

#[derive(Deserialize)]
enum SyncState {
    #[serde(rename = "inSync")]
    InSync,
    #[serde(rename = "edited")]
    Edited,
    #[serde(rename = "sourceChanged")]
    SourceChanged,
    #[serde(rename = "both")]
    Both,
}

#[derive(Deserialize)]
struct SourceState {
    layer: u64,
    state: SyncState,
}

pub fn save(req: &Request) -> Result<Value, String> {
    ui::require_capture(req)?;
    let config = settings::config(req)?;
    let layers = settings::layers()?;
    let layer = settings::layer(&layers, &config.layer)?;
    if req.panel_str("layer") != Some(layer.id.to_string().as_str()) {
        return Err("Select the designated layer again before saving".into());
    }
    let session: Session = serde_json::from_str(
        req.panel
            .get("session")
            .and_then(|v| v.get("payload"))
            .and_then(Value::as_str)
            .ok_or("Prepare the layer first")?,
    )
    .map_err(|e| e.to_string())?;
    let project: Project = serde_json::from_value(host::call("info.project", json!({}))?)
        .map_err(|e| e.to_string())?;
    let source = project
        .layers
        .iter()
        .find(|l| l.id == layer.id)
        .ok_or("Layer source missing")?;
    if session.layer != layer.id
        || session.context.name != project.name
        || session.context.path != project.path
        || session.context.map != project.active_map
        || session.context.layer_name != layer.name
        || session.context.source != source.source
    {
        return Err(
            "Project or layer source changed; prepare the designated layer again before saving"
                .into(),
        );
    }
    let state: SourceState =
        serde_json::from_value(host::call("source.syncState", json!({"layer":layer.id}))?)
            .map_err(|e| e.to_string())?;
    if state.layer != layer.id {
        return Err("Source state belongs to another layer; nothing saved".into());
    }
    match state.state {
        SyncState::SourceChanged | SyncState::Both => {
            return Err(
                "Source file changed outside RustGIS; nothing saved. In-memory edits retained"
                    .into(),
            );
        }
        SyncState::InSync | SyncState::Edited => {}
    }
    let published = host::call("source.publish", json!({"layer":layer.id}))?;
    // An external write cannot be rolled back. No fallible operation may escape after it.
    let mut result = match target::prepare(req, config) {
        Ok(value) => value,
        Err(error) => {
            let mut value = json!({"panel":{"values":{"session":{"payload":""}}},
                "preparationError":error});
            ui::capture_controls(&mut value, false);
            value
        }
    };
    let message = match published.get("published").and_then(Value::as_bool) {
        Some(true) => "Saved all pending edits of the designated layer to its source",
        Some(false) => "Source already up to date; no file written",
        None => {
            "Source publish returned without a write receipt; check Source File State before saving again"
        }
    };
    let message = match result.get("preparationError").and_then(Value::as_str) {
        Some(error) => format!(
            "{message}. Editor not ready: {error}. Adjust Navigation and rebuild before editing"
        ),
        None => message.into(),
    };
    result["report"] = json!(message);
    result["sourcePublish"] = published;
    result["panel"]["items"]["save-note"]["text"] = json!(message);
    Ok(result)
}
