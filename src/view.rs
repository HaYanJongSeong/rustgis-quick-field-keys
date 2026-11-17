use crate::model::Feature;
use rustgis_plugin::{Request, Value, host, json};

pub fn apply(req: &Request, feature: &Feature) -> Result<(), String> {
    let view = req.panel.get("view");
    let mode = view
        .and_then(|v| v.get("zoom-mode"))
        .and_then(Value::as_str)
        .unwrap_or("off");
    let scale = match mode {
        "off" if req.id != "zoom-current" => return Ok(()),
        "off" | "feature" => None,
        "keep-scale" => Some(
            host::call("info.project", json!({}))?
                .get("scale")
                .and_then(Value::as_f64)
                .filter(|n| n.is_finite() && *n > 0.0)
                .ok_or("Current map scale is unavailable")?,
        ),
        "fixed-scale" => Some(
            view.and_then(|v| v.get("scale"))
                .and_then(Value::as_f64)
                .filter(|n| n.is_finite() && (1.0..=100_000_000.0).contains(n))
                .ok_or("Fixed scale must be between 1 and 100000000")?,
        ),
        _ => return Err("Unknown zoom mode".into()),
    };
    let positions = rustgis_plugin::geojson::positions(&feature.geometry);
    let mut bbox = rustgis_plugin::geojson::bbox(&positions)
        .ok_or("Current feature has no zoomable geometry")?;
    let [west, south, east, north] = &mut bbox;
    if (*east - *west).abs() < 0.0001 {
        *west -= 0.0001;
        *east += 0.0001;
    }
    if (*north - *south).abs() < 0.0001 {
        *south -= 0.0001;
        *north += 0.0001;
    }
    host::call("map.zoomToExtent", json!({"extent":bbox}))?;
    if let Some(scale) = scale {
        host::call("map.setScale", json!({"scale":scale}))?;
    }
    Ok(())
}
