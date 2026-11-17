use crate::model::{Error, Feature};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Center {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Camera {
    pub center: Center,
    pub scale: f64,
    pub bearing: f64,
    pub pitch: f64,
    pub projection: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Selection {
    pub row: usize,
    pub id: Value,
    pub identity: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreviousView {
    pub camera: Camera,
    pub selected: Vec<Selection>,
    pub cursor: usize,
}

impl PreviousView {
    pub fn rows(&self, features: &[Feature], identity: &str) -> Result<Vec<usize>, Error> {
        self.selected
            .iter()
            .map(|saved| {
                features
                    .iter()
                    .find(|feature| {
                        feature.row == saved.row
                            && feature.id == saved.id
                            && feature.properties.get(identity) == Some(&saved.identity)
                    })
                    .map(|feature| feature.row)
                    .ok_or_else(|| {
                        Error::Invalid(
                            "Previous selection changed or moved; rebuild navigation".into(),
                        )
                    })
            })
            .collect()
    }
}

#[cfg(target_arch = "wasm32")]
pub fn capture(session: &crate::model::Session) -> Result<PreviousView, String> {
    use rustgis_plugin::{host, json};
    let camera = serde_json::from_value(host::call("info.project", json!({}))?)
        .map_err(|e| e.to_string())?;
    let collection: crate::model::Collection = serde_json::from_value(host::call(
        "host.features",
        json!({"layer":session.layer,"selected":true,"limit":10001}),
    )?)
    .map_err(|e| e.to_string())?;
    let selected = collection
        .features
        .iter()
        .map(|feature| {
            Ok(Selection {
                row: feature.row,
                id: feature.id.clone(),
                identity: feature
                    .properties
                    .get(&session.identity)
                    .ok_or("Selected feature has no identity")?
                    .clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(PreviousView {
        camera,
        selected,
        cursor: session.cursor,
    })
}

#[cfg(target_arch = "wasm32")]
pub fn restore(
    session: &mut crate::model::Session,
    config: &crate::model::Config,
    all: &[Feature],
) -> Result<Value, String> {
    use rustgis_plugin::{host, json};
    let previous = session
        .previous_view
        .take()
        .ok_or("No previous selection and view")?;
    let rows = previous
        .rows(all, &session.identity)
        .map_err(|e| e.to_string())?;
    let current: Camera = serde_json::from_value(host::call("info.project", json!({}))?)
        .map_err(|e| e.to_string())?;
    if current.projection != previous.camera.projection {
        return Err("Map projection changed; previous view is unavailable".into());
    }
    session.cursor = previous.cursor;
    let mut result = crate::runtime::answer(session, config)?;
    let camera = &previous.camera;
    host::call(
        "select.rows",
        json!({"layer":session.layer,"rows":all.iter().map(|f| f.row).collect::<Vec<_>>(),"mode":"remove"}),
    )?;
    host::call(
        "select.rows",
        json!({"layer":session.layer,"rows":rows,"mode":"add"}),
    )?;
    host::call(
        "map.goToXY",
        json!({"lon":camera.center.lon,"lat":camera.center.lat,
        "scale":camera.scale,"bearing":camera.bearing,"pitch":camera.pitch}),
    )?;
    result["report"] = json!(
        "Restored previous selection and view; feature edits and navigation filter unchanged"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_same_count_selection_when_feature_identity_changes() {
        // Given
        let mut feature = Feature {
            row: 2,
            id: json!("fid-42"),
            properties: [("uid".into(), json!(42))].into(),
            geometry: Value::Null,
        };
        let saved = PreviousView {
            camera: Camera {
                center: Center {
                    lon: 127.0,
                    lat: 37.0,
                },
                scale: 25000.0,
                bearing: 31.0,
                pitch: 45.0,
                projection: "mercator".into(),
            },
            selected: vec![Selection {
                row: 2,
                id: json!("fid-42"),
                identity: json!(42),
            }],
            cursor: 0,
        };
        assert!(matches!(saved.rows(&[feature.clone()], "uid"), Ok(rows) if rows == vec![2]));
        // When
        feature.id = json!("replacement");
        // Then
        assert!(saved.rows(&[feature], "uid").is_err());
    }
}
