use crate::model::Feature;
use serde_json::Value;

pub fn capture_order(
    features: Vec<Feature>,
    query: &str,
    field: &str,
    ascending: bool,
) -> Vec<Feature> {
    let query = query.to_lowercase();
    let mut rows: Vec<_> = features
        .into_iter()
        .filter(|f| {
            query.is_empty()
                || f.properties.values().any(|v| match v {
                    Value::String(s) => s.to_lowercase().contains(&query),
                    _ => v.to_string().to_lowercase().contains(&query),
                })
        })
        .collect();
    rows.sort_by(|a, b| {
        let order = match (a.properties.get(field), b.properties.get(field)) {
            (Some(Value::Number(a)), Some(Value::Number(b))) => match (a.as_i64(), b.as_i64()) {
                (Some(a), Some(b)) => a.cmp(&b),
                _ => match (a.as_f64(), b.as_f64()) {
                    (Some(a), Some(b)) => a.total_cmp(&b),
                    _ => a.to_string().cmp(&b.to_string()),
                },
            },
            (Some(Value::String(a)), Some(Value::String(b))) => a.cmp(b),
            _ => a.row.cmp(&b.row),
        }
        .then(a.row.cmp(&b.row));
        if ascending { order } else { order.reverse() }
    });
    rows
}
