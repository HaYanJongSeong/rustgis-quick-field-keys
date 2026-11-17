use crate::model::{Collection, Feature};
use rustgis_plugin::{Request, Value, host, json};
use std::collections::BTreeSet;

pub fn matches(
    req: &Request,
    layer: u64,
    candidates: &[Feature],
) -> Result<Option<BTreeSet<usize>>, String> {
    let filter = req.panel.get("filter");
    match filter
        .and_then(|f| f.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or("Text contains")
    {
        "Text contains" => return Ok(None),
        "RustGIS SQL" => {}
        _ => return Err("Unknown navigation filter mode".into()),
    }
    let query = filter
        .and_then(|f| f.get("query"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if query.is_empty() {
        return Ok(None);
    }
    let original: Collection = serde_json::from_value(host::call(
        "host.features",
        json!({"layer":layer,"selected":true,"limit":10001}),
    )?)
    .map_err(|e| e.to_string())?;
    let original_rows: Vec<_> = original.features.iter().map(|f| f.row).collect();
    let candidate_rows: Vec<_> = candidates.iter().map(|f| f.row).collect();
    host::call(
        "select.rows",
        json!({"layer":layer,"rows":candidate_rows,"mode":"add"}),
    )?;
    host::call(
        "select.byAttributes",
        json!({"layer":layer,"where":query,"mode":"subset"}),
    )?;
    let matched: Collection = serde_json::from_value(host::call(
        "host.features",
        json!({"layer":layer,"selected":true,"limit":10001}),
    )?)
    .map_err(|e| e.to_string())?;
    let changed_rows: BTreeSet<_> = original_rows
        .iter()
        .chain(candidate_rows.iter())
        .copied()
        .collect();
    host::call(
        "select.rows",
        json!({"layer":layer,"rows":changed_rows,"mode":"remove"}),
    )?;
    host::call(
        "select.rows",
        json!({"layer":layer,"rows":original_rows,"mode":"add"}),
    )?;
    Ok(Some(matched.features.iter().map(|f| f.row).collect()))
}
