//! Fast-travel station catalog.
//!
//! Extracted from the zlib-licensed Gibbed.Borderlands2.GameInfo
//! (`FastTravelStationDefinition` entries): the `resource_name` stored in the
//! save's field 16 mapped to a human `station_display_name`, grouped by DLC pack.
//! Factual identifier data, not art — see ASSETS.md.

use std::sync::OnceLock;

use serde_json::Value;

const DATA: &str = include_str!("stations_data.json");

/// One fast-travel station: `rn` is stored in the save; `name` is for display.
#[derive(Clone, Debug)]
pub struct Station {
    pub rn: String,
    pub name: String,
    pub pack: String,
}

/// Every known fast-travel station, ordered base-game-first then by DLC pack.
pub fn catalog() -> &'static [Station] {
    static CACHE: OnceLock<Vec<Station>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let v: Value = serde_json::from_str(DATA).unwrap_or(Value::Null);
        v.as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| {
                        Some(Station {
                            rn: s.get("rn")?.as_str()?.to_string(),
                            name: s.get("name")?.as_str()?.to_string(),
                            pack: s.get("pack").and_then(Value::as_str).unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Display name for a stored `resource_name`, if known.
pub fn display_name(resource_name: &str) -> Option<&'static str> {
    catalog().iter().find(|s| s.rn == resource_name).map(|s| s.name.as_str())
}
