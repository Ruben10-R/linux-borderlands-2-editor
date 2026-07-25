//! Free-function catalogs and value types layered over the modules: item-code
//! decoding, parts/customization/station/vehicle lists, and the classes table.

use crate::{gameinfo, serial, stations, vehicles};
use crate::{Customization, Station, VehicleSkin};

/// What a `BL2(...)` code decodes to, for building an item-code library.
#[derive(Clone, Debug)]
pub struct CodeInfo {
    /// One of: "Weapon", "Shield", "Grenade", "Class Mod", "Relic", "Item".
    pub category: &'static str,
    /// Readable name (manufacturer + type, e.g. "Jakobs Sniper").
    pub name: String,
    /// The balance/family id — variants of the same item share it.
    pub family: String,
    /// Item level (game stage), 0 if none.
    pub level: i64,
}

/// Decode a `BL2(...)` code into a [`CodeInfo`] (category + name), or None if it
/// isn't a valid code. Uses the item serial + GameInfo — no save needed.
pub fn describe_code(code: &str) -> Option<CodeInfo> {
    let (serial, _) = serial::from_code(code).ok()?;
    let s = serial::unwrap(&serial).ok()?;
    let type_name = s.type_name().unwrap_or_default();
    let category = if s.is_weapon {
        "Weapon"
    } else {
        let hay = format!("{} {}", type_name, s.balance_name().unwrap_or_default()).to_lowercase();
        if hay.contains("shield") {
            "Shield"
        } else if hay.contains("grenade") {
            "Grenade"
        } else if hay.contains("class") && hay.contains("mod") || hay.contains("classmod") {
            "Class Mod"
        } else if hay.contains("artifact") || hay.contains("relic") {
            "Relic"
        } else {
            "Item"
        }
    };
    let manu = s.manufacturer_name().unwrap_or_default();
    let name = format!("{manu} {type_name}").trim().to_string();
    let family = s
        .balance_name()
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| {
            if name.is_empty() {
                "Unknown".into()
            } else {
                name.clone()
            }
        });
    Some(CodeInfo {
        category,
        name,
        family,
        level: s.stage.unwrap_or(0),
    })
}

/// Pull every `BL2(...)` token out of free text. A code's base64 body can
/// contain `/` and `+`, but never `)`, so scanning `BL2(` up to the next `)`
/// robustly separates codes regardless of the separators between them.
pub fn extract_codes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("BL2(") {
        let after = &rest[start..];
        if let Some(end) = after.find(')') {
            out.push(after[..=end].to_string());
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

/// The six playable classes: (display name, class-definition asset path).
/// Paths extracted from Gibbed's GameInfo; pass the path to [`SaveFile::set_class`].
pub const CLASSES: [(&str, &str); 6] = [
    ("Axton (Commando)", "GD_Soldier.Character.CharClass_Soldier"),
    ("Maya (Siren)", "GD_Siren.Character.CharClass_Siren"),
    (
        "Salvador (Gunzerker)",
        "GD_Mercenary.Character.CharClass_Mercenary",
    ),
    (
        "Zer0 (Assassin)",
        "GD_Assassin.Character.CharClass_Assassin",
    ),
    (
        "Gaige (Mechromancer)",
        "GD_Tulip_Mechromancer.Character.CharClass_Mechromancer",
    ),
    (
        "Krieg (Psycho)",
        "GD_Lilac_PlayerClass.Character.CharClass_LilacPlayerClass",
    ),
];

/// A grouped view of one top-level protobuf field number, for the Raw inspector.
/// Single-occurrence scalar fields expose an editable `value`/`text`; repeated or
/// nested fields are summarised in `preview`.
#[derive(Clone, Debug)]
pub struct RawField {
    pub number: u64,
    /// Human field name (from the save schema), or "" if unknown.
    pub name: &'static str,
    /// Plain-language explanation of the field, or "" if undocumented.
    pub help: &'static str,
    /// "varint", "string", "message", "collection", "bytes", "fixed32/64".
    pub kind: &'static str,
    /// Number of occurrences of this field number.
    pub count: usize,
    /// Editable value for a single-occurrence varint field.
    pub value: Option<i64>,
    /// Editable value for a single-occurrence UTF-8 string field.
    pub text: Option<String>,
    /// Human summary (value, quoted string, "(message, N bytes)", "N entries").
    pub preview: String,
}

/// One selectable part in a parts picker.
#[derive(Clone, Debug)]
pub struct PartOption {
    pub lib: u32,
    pub asset: u32,
    pub name: String,
}

/// Every known fast-travel station (base game + DLC), for building the unlock
/// list. Each `Station`'s `rn` is what [`SaveFile::set_visited_stations`] expects.
pub fn stations_catalog() -> &'static [Station] {
    stations::catalog()
}

/// Display name for a stored station `resource_name`, if known.
pub fn station_display_name(resource_name: &str) -> Option<&'static str> {
    stations::display_name(resource_name)
}

/// Every head (`is_head=true`) or skin usable by the given class-definition path,
/// for building a customization picker. See [`SaveFile::set_wearing`].
pub fn customizations(class_def: &str, is_head: bool) -> Vec<Customization> {
    crate::customizations::for_class(class_def, is_head)
}

/// Every skin usable by a vehicle family (by its token, e.g. "Runner").
pub fn vehicle_skins(family_token: &str) -> &'static [VehicleSkin] {
    vehicles::skins_for(family_token)
}

/// Display name for an equipped vehicle skin path, if known.
pub fn vehicle_skin_name(path: &str) -> Option<&'static str> {
    vehicles::skin_name(path)
}

/// Display name for an equipped head/skin path, if known.
pub fn customization_name(path: &str) -> Option<&'static str> {
    crate::customizations::name(path)
}

/// The stock "Default" head/skin path for a class (a safe reset).
pub fn default_customization(class_def: &str, is_head: bool) -> Option<String> {
    crate::customizations::default_path(class_def, is_head)
}

/// Human name for part slot `slot` of a weapon or item.
///
/// Weapons have a fixed 11-slot layout, so every slot gets its real name.
/// Items are heterogeneous (a shield, grenade mod and relic use slots 0–7 for
/// different things), so only the tail — Material/Prefix/Title — is named; the
/// rest are generic "Part N". Verified against real save0001 items.
pub fn slot_label(is_weapon: bool, slot: usize) -> &'static str {
    const WEAPON: [&str; 11] = [
        "Body",
        "Grip",
        "Barrel",
        "Sight",
        "Stock",
        "Element",
        "Accessory 1",
        "Accessory 2",
        "Material",
        "Prefix",
        "Title",
    ];
    const ITEM: [&str; 11] = [
        "Part 1", "Part 2", "Part 3", "Part 4", "Part 5", "Part 6", "Part 7", "Part 8", "Material",
        "Prefix", "Title",
    ];
    let table = if is_weapon { &WEAPON } else { &ITEM };
    table.get(slot).copied().unwrap_or("Part")
}

/// Every available part (with a readable name) for weapons vs items in a given
/// set — the choices for a parts picker. NOTE: not filtered by item compatibility,
/// so arbitrary combinations can produce items the game rejects.
pub fn parts_catalog(is_weapon: bool, set: u32) -> Vec<PartOption> {
    let category = if is_weapon {
        "WeaponParts"
    } else {
        "ItemParts"
    };
    gameinfo::catalog(category, set)
        .into_iter()
        .map(|(lib, asset, name)| PartOption { lib, asset, name })
        .collect()
}
