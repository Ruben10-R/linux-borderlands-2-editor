//! Equipped vehicle skins (save field 57 = `ChosenVehicleCustomizations`).
//!
//! Field 57 is a repeated message, one per vehicle family: sub-field 1 = the
//! family path, sub-field 2 = a repeated list of chosen skin paths (up to two
//! slots). Skin lists come from GameInfo (identifier data, not art). Structure
//! and field number understood from Gibbed's proto; our own implementation.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::error::Result;
use crate::proto;

const DATA: &str = include_str!("vehicles_data.json");
const FIELD_VEHICLE: u64 = 57;

/// One selectable vehicle skin.
#[derive(Clone, Debug)]
pub struct VehicleSkin {
    pub path: String,
    pub name: String,
}

/// A vehicle family: a stable token, a display name, and the save's family path.
#[derive(Clone, Copy, Debug)]
pub struct VehicleFamily {
    pub token: &'static str,
    pub name: &'static str,
    pub path: &'static str,
}

/// The four vehicle families, in a sensible order.
pub const FAMILIES: [VehicleFamily; 4] = [
    VehicleFamily {
        token: "Runner",
        name: "Runner",
        path: "GD_Globals.VehicleSpawnStation.VehicleFamily_Runner",
    },
    VehicleFamily {
        token: "BanditTech",
        name: "Bandit Technical",
        path: "GD_Globals.VehicleSpawnStation.VehicleFamily_BanditTechnical",
    },
    VehicleFamily {
        token: "Hovercraft",
        name: "Hovercraft (Pirate's Booty)",
        path: "GD_OrchidPackageDef.Vehicles.VehicleFamily_Hovercraft",
    },
    VehicleFamily {
        token: "FanBoat",
        name: "Fan Boat (Hammerlock's Hunt)",
        path: "GD_SagePackageDef.Vehicles.VehicleFamily_FanBoat",
    },
];

fn db() -> &'static HashMap<String, Vec<VehicleSkin>> {
    static CACHE: OnceLock<HashMap<String, Vec<VehicleSkin>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let v: Value = serde_json::from_str(DATA).unwrap_or(Value::Null);
        let mut out = HashMap::new();
        if let Some(obj) = v.as_object() {
            for (token, arr) in obj {
                let mut skins: Vec<VehicleSkin> = arr
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| {
                                Some(VehicleSkin {
                                    path: s.get("path")?.as_str()?.to_string(),
                                    name: s.get("name")?.as_str()?.to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                skins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                out.insert(token.clone(), skins);
            }
        }
        out
    })
}

/// Every skin usable by a vehicle family (by its `token`).
pub fn skins_for(token: &str) -> &'static [VehicleSkin] {
    db().get(token).map(Vec::as_slice).unwrap_or(&[])
}

/// Display name for an equipped skin path, if known.
pub fn skin_name(path: &str) -> Option<&'static str> {
    db().values()
        .flatten()
        .find(|s| s.path == path)
        .map(|s| s.name.as_str())
}

/// The chosen skins for a family (from field 57's matching entry).
pub fn family_skins(protobuf: &[u8], family_path: &str) -> Vec<String> {
    let Ok(fields) = proto::parse_fields(protobuf) else {
        return Vec::new();
    };
    for f in fields
        .iter()
        .filter(|f| f.number == FIELD_VEHICLE && f.wire_type == 2)
    {
        let Ok(entry) = proto::wire2_content(protobuf, f) else {
            continue;
        };
        let Ok(ifields) = proto::parse_fields(entry) else {
            continue;
        };
        if proto::read_string_field(entry, &ifields, 1).as_deref() != Some(family_path) {
            continue;
        }
        return ifields
            .iter()
            .filter(|x| x.number == 2 && x.wire_type == 2)
            .filter_map(|x| {
                proto::wire2_content(entry, x)
                    .ok()
                    .and_then(|c| std::str::from_utf8(c).ok())
                    .map(str::to_string)
            })
            .collect();
    }
    Vec::new()
}

/// Rewrite a family's chosen skins (drops empty/"None"). Returns the new proto;
/// only field 57 changes.
pub fn set_family_skins(protobuf: &[u8], family_path: &str, skins: &[String]) -> Result<Vec<u8>> {
    let keep: Vec<&String> = skins
        .iter()
        .filter(|s| !s.is_empty() && *s != "None")
        .collect();
    let mut new_entry = Vec::new();
    if !keep.is_empty() {
        proto::emit_wire2_field(&mut new_entry, 1, family_path.as_bytes());
        for s in &keep {
            proto::emit_wire2_field(&mut new_entry, 2, s.as_bytes());
        }
    }

    let fields = proto::parse_fields(protobuf)?;
    let mut out = Vec::with_capacity(protobuf.len() + new_entry.len());
    let mut replaced = false;
    for f in &fields {
        if f.number == FIELD_VEHICLE && f.wire_type == 2 {
            let entry = proto::wire2_content(protobuf, f)?;
            let ifields = proto::parse_fields(entry)?;
            if proto::read_string_field(entry, &ifields, 1).as_deref() == Some(family_path) {
                // replace this family's entry (or drop it if no skins left)
                if !new_entry.is_empty() {
                    proto::emit_wire2_field(&mut out, FIELD_VEHICLE, &new_entry);
                }
                replaced = true;
                continue;
            }
        }
        out.extend_from_slice(&protobuf[f.tag_start..f.end]);
    }
    if !replaced && !new_entry.is_empty() {
        proto::emit_wire2_field(&mut out, FIELD_VEHICLE, &new_entry);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_has_skins_and_names_resolve() {
        for f in FAMILIES {
            let skins = skins_for(f.token);
            assert!(!skins.is_empty(), "{} has skins", f.token);
        }
        let first = &skins_for("Runner")[0];
        assert_eq!(skin_name(&first.path), Some(first.name.as_str()));
        assert!(skin_name("not_a_skin").is_none());
    }
}
