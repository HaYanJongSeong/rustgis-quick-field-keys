use crate::model::{Config, Error};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct PresetUpdate {
    pub layer: String,
    pub identity: String,
    pub slot: usize,
    pub values: BTreeMap<String, Value>,
}

pub fn configure_slot(old: Config, update: PresetUpdate) -> Result<Config, Error> {
    if update.values.contains_key(&update.identity) {
        return Err(Error::Invalid("Identity is always locked".into()));
    }
    let mut config = if old.layer == update.layer && old.identity == update.identity {
        old
    } else {
        Config::empty(update.layer)
    };
    config.identity = update.identity;
    let preset = match update.slot {
        1 => &mut config.preset1,
        2 => &mut config.preset2,
        3 => &mut config.preset3,
        4 => &mut config.preset4,
        5 => &mut config.preset5,
        6 => &mut config.preset6,
        7 => &mut config.preset7,
        8 => &mut config.preset8,
        9 => &mut config.preset9,
        _ => return Err(Error::Invalid("Choose preset slot 1 through 9".into())),
    };
    *preset = serde_json::to_string(&update.values)?;
    if preset.len() > 4000 {
        return Err(Error::Invalid(
            "Preset settings exceed 4000 UTF-8 bytes in a host text field".into(),
        ));
    }
    Ok(config)
}
