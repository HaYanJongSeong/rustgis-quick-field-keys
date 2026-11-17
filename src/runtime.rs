use crate::model;
use crate::{settings, ui};
use model::{Collection, Config, Context, Feature, Layer, Project, Session};
use rustgis_plugin::{Kind, Request, Value, host, json};

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn features(layer: u64, selected: bool) -> Result<Vec<Feature>, String> {
    let collection: Collection = parse(host::call(
        "host.features",
        json!({"layer":layer,"selected":selected,"limit":10001}),
    )?)?;
    if collection.features.len() > 10000 {
        return Err("At most 10000 features supported".into());
    }
    Ok(collection.features)
}

pub(crate) fn answer(session: &Session, config: &Config) -> Result<Value, String> {
    let payload = serde_json::to_string(session).map_err(|e| e.to_string())?;
    model::validate_capture_size(&payload).map_err(|e| e.to_string())?;
    let row = session
        .captured
        .get(session.cursor)
        .ok_or("Invalid cursor")?
        .row;
    let mut result = json!({"current":session.captured.get(session.cursor),"report":format!("Ready: {} features; current {} of {}, row {}. Nothing saved.", session.captured.len(),session.cursor.saturating_add(1),session.captured.len(),row),
        "panel":{"values":{"config":config,"session":{"payload":payload}},"items":{"status":{"text":format!("{} / {} | {}: {}",session.cursor.saturating_add(1),session.captured.len(),session.identity,session.captured.get(session.cursor).and_then(|f|f.properties.get(&session.identity)).unwrap_or(&Value::Null))}}}});
    ui::decorate(&mut result, session, config)?;
    Ok(result)
}

pub fn handle(mut req: Request) -> Result<Value, String> {
    match req.kind {
        Kind::Command => {}
        Kind::Tool => return Err("Commands only".into()),
    }
    if req.id == "state" {
        return ui::state(&req);
    }
    if req.id == "configure" {
        return settings::configure(&req);
    }
    if req.id == "options" {
        return settings::options(&req);
    }
    if req.id == "save" {
        let mut result = crate::publish::save(&req)?;
        crate::history_ui::clear(&mut result);
        return Ok(result);
    }
    if crate::columns::is_command(&req.id) {
        return crate::columns::handle(&req);
    }
    if ui::is_control(&req.id) {
        return ui::controls(&req);
    }
    if req.id != "capture" {
        ui::require_capture(&req)?;
    }
    let mut config: Config = parse(
        req.panel
            .get("config")
            .cloned()
            .ok_or("Open the panel first")?,
    )?;
    let layers: Vec<Layer> = parse(host::layers()?)?;
    let layer = settings::layer(&layers, &config.layer)?;
    if layer.raster || layer.features > 10000 {
        return Err("Choose a vector layer with at most 10000 features".into());
    }
    let all = features(layer.id, false)?;
    if req.id == "capture" && config.identity.is_empty() {
        config.identity = layer
            .fields
            .iter()
            .find(|field| model::validate_identity(&all, &field.name).is_ok())
            .ok_or(
                "No unique non-null text or numeric identity field; configure one before editing",
            )?
            .name
            .clone();
    }
    if req
        .panel_str("layer")
        .is_some_and(|id| id != layer.id.to_string() && id != layer.name)
    {
        return Err(
            "Selected layer differs from the prepared target; select the layer again".into(),
        );
    }
    let project: Project = parse(host::call("info.project", json!({}))?)?;
    let context = Context {
        name: project.name,
        path: project.path,
        map: project.active_map,
        layer_name: layer.name.clone(),
        source: project
            .layers
            .into_iter()
            .find(|l| l.id == layer.id)
            .ok_or("Layer source missing")?
            .source,
    };
    if all.len() != layer.features {
        return Err("Incomplete feature response; refusing edits".into());
    }
    let mut session = if req.id == "capture" {
        crate::target::snapshot(
            &req,
            Session {
                context: context.clone(),
                layer: layer.id,
                identity: config.identity.clone(),
                fields: layer.fields.clone(),
                count: all.len(),
                captured: all.clone(),
                cursor: 0,
                previous_view: None,
            },
        )?
    } else {
        let payload = req
            .panel
            .get("session")
            .and_then(|v| v.get("payload"))
            .and_then(Value::as_str)
            .ok_or("Choose a layer before navigating/editing.")?;
        serde_json::from_str::<Session>(payload).map_err(|e| e.to_string())?
    };
    if session.identity != config.identity {
        return Err("Identity configuration changed; capture again".into());
    }
    if session.context != context {
        return Err("Project/map/source changed; capture again".into());
    }
    model::check_session(&session, layer, &all).map_err(|e| e.to_string())?;
    if req.id == "previous-view" {
        let mut result = crate::previous_view::restore(&mut session, &config, &all)?;
        crate::history_ui::decorate(&mut result, &req, &session)?;
        return Ok(result);
    }
    if matches!(
        req.id.as_str(),
        "first" | "previous" | "next" | "last" | "zoom-current"
    ) {
        session.previous_view = Some(crate::previous_view::capture(&session)?);
    }
    if req.id == "capture" {
        crate::column_preferences::restore(&session, &mut config)?;
    }
    match req.id.as_str() {
        "capture" => {}
        "first" => {
            session.cursor = 0;
        }
        "last" => {
            session.cursor = session
                .captured
                .len()
                .checked_sub(1)
                .ok_or("Empty capture")?;
        }
        "zoom-current" => {}
        "previous" => {
            session.cursor = session
                .cursor
                .checked_sub(1)
                .ok_or("Already at first captured feature")?;
        }
        "next" => {
            let next = session.cursor.saturating_add(1);
            if next >= session.captured.len() {
                return Err("Already at last captured feature".into());
            }
            session.cursor = next;
        }
        _ => {
            let values = crate::attribute_values::prepare(&mut req, &mut session, &config)?;
            if values.is_empty() {
                let mut result = answer(&session, &config)?;
                crate::history_ui::decorate(&mut result, &req, &session)?;
                return Ok(result);
            }
            let current = session
                .captured
                .get(session.cursor)
                .ok_or("Invalid cursor")?;
            for (field, value) in &values {
                host::call(
                    "edit.setValue",
                    json!({"layer":layer.id,"row":current.row,"field":field,"value":value}),
                )?;
            }
            let updated = features(layer.id, false)?;
            let refreshed = updated
                .iter()
                .find(|f| f.row == current.row)
                .ok_or("Edited row disappeared")?;
            for (field, value) in &values {
                if refreshed.properties.get(field) != Some(value) {
                    return Err(format!("Host did not retain requested value for {field}"));
                }
            }
            *session
                .captured
                .get_mut(session.cursor)
                .ok_or("Invalid cursor")? = refreshed.clone();
        }
    }
    let mut result = answer(&session, &config)?;
    crate::history_ui::decorate(&mut result, &req, &session)?;
    let current = session
        .captured
        .get(session.cursor)
        .ok_or("Invalid cursor")?;
    host::call(
        "select.rows",
        json!({"layer":layer.id,"rows":all.iter().map(|f| f.row).collect::<Vec<_>>(),"mode":"remove"}),
    )?;
    host::call(
        "select.rows",
        json!({"layer":layer.id,"rows":[current.row],"mode":"add"}),
    )?;
    crate::view::apply(&req, current)?;
    Ok(result)
}

#[cfg(target_arch = "wasm32")]
rustgis_plugin::export!(handle);
