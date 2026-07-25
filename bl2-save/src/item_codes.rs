//! A browsable library of shareable `BL2(...)` item codes.
//!
//! Codes were collected from a community guide and each was **decoded by our own
//! engine** (see [`crate::describe_code`]) to derive its category, family and
//! level — so the categorisation is ours, not scraped. Data is factual code
//! strings + identifiers, not game art (see ASSETS.md). Currently weapons only
//! (that's what the source covered); the schema supports every category.

use std::sync::OnceLock;

use serde_json::Value;

const DATA: &str = include_str!("item_codes_data.json");

/// One entry in the code library.
#[derive(Clone, Debug)]
pub struct LibraryItem {
    pub code: String,
    /// "Weapon" | "Shield" | "Grenade" | "Class Mod" | "Relic" | "Item".
    pub category: String,
    /// The item/family the code belongs to (variants share it).
    pub family: String,
    /// A readable label for this specific variant.
    pub name: String,
    pub level: i64,
}

/// The full embedded code library (parsed once).
pub fn code_library() -> &'static [LibraryItem] {
    static CACHE: OnceLock<Vec<LibraryItem>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let v: Value = serde_json::from_str(DATA).unwrap_or(Value::Null);
        v.as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        Some(LibraryItem {
                            code: e.get("code")?.as_str()?.to_string(),
                            category: e.get("category")?.as_str()?.to_string(),
                            family: e.get("family")?.as_str().unwrap_or("").to_string(),
                            name: e.get("name")?.as_str().unwrap_or("").to_string(),
                            level: e.get("level").and_then(Value::as_i64).unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// The distinct categories present in the library, in a sensible order.
pub fn library_categories() -> Vec<&'static str> {
    const ORDER: [&str; 6] = ["Weapon", "Shield", "Grenade", "Class Mod", "Relic", "Item"];
    ORDER.iter().copied().filter(|c| code_library().iter().any(|e| e.category == *c)).collect()
}
