//! Resolve item-serial database refs to human names.
//!
//! An item serial stores `(sublibrary, asset)` indices into Gibbed's GameInfo
//! "asset library". We embed a compact slice of that database — manufacturers +
//! weapon/item types across all sets — extracted from the zlib-licensed
//! Gibbed.Borderlands2.GameInfo. These are factual identifier strings (not art)
//! used for interoperability. Balance/part names are a larger future slice
//! (§15). Lookup: `sets[set].libraries[category].sublibraries[lib].assets[asset]`.

use std::sync::OnceLock;

use serde_json::Value;

const DATA: &str = include_str!("gameinfo_data.json");

fn db() -> &'static Value {
    static DB: OnceLock<Value> = OnceLock::new();
    DB.get_or_init(|| serde_json::from_str(DATA).unwrap_or(Value::Null))
}

/// The raw asset path (`package.asset`) for a ref, or None if out of range.
fn asset_path(category: &str, set: u32, lib: u32, asset: u32) -> Option<String> {
    let sub = db().get(category)?.get(set as usize)?.get(lib as usize)?;
    let pkg = sub.get("p")?.as_str().unwrap_or("");
    let a = sub.get("a")?.as_array()?.get(asset as usize)?.as_str()?;
    Some(if pkg.is_empty() { a.to_string() } else { format!("{pkg}.{a}") })
}

/// A short, human-friendly name for a ref (e.g. "Jakobs", "Jakobs Pistol"),
/// or None if the ref isn't in our embedded slice.
pub(crate) fn name(category: &str, set: u32, lib: u32, asset: u32) -> Option<String> {
    let path = asset_path(category, set, lib, asset)?;
    let seg = path.rsplit('.').next().unwrap_or(&path);
    let seg = seg
        .strip_prefix("WeaponType_")
        .or_else(|| seg.strip_prefix("WT_"))
        .unwrap_or(seg);
    Some(seg.replace('_', " "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_known_manufacturers() {
        assert_eq!(name("Manufacturers", 0, 1, 3).as_deref(), Some("Jakobs"));
        assert_eq!(name("Manufacturers", 0, 1, 7).as_deref(), Some("Tediore"));
        assert_eq!(name("Manufacturers", 0, 1, 8).as_deref(), Some("Maliwan"));
    }

    #[test]
    fn unknown_refs_are_none() {
        assert!(name("Manufacturers", 0, 99, 99).is_none());
        assert!(name("Nonsense", 0, 0, 0).is_none());
    }
}
