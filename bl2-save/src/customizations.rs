//! Character head/skin customization catalog.
//!
//! Extracted from the zlib-licensed Gibbed.Borderlands2.GameInfo
//! (`CustomizationDefinition` entries): asset path → display `name`, `type`
//! (Head/Skin) and the classes that can `use` it. The save stores the equipped
//! head/skin *paths* in field 35 ("wearing"); this maps them to names and lets a
//! picker offer the valid options per class. Identifier data, not art — ASSETS.md.

use std::sync::OnceLock;

use serde_json::Value;

const DATA: &str = include_str!("customizations_data.json");

/// One head or skin option.
#[derive(Clone, Debug)]
pub struct Customization {
    pub path: String,
    pub name: String,
    pub is_head: bool,
    classes: Vec<String>,
}

fn all() -> &'static [Customization] {
    static CACHE: OnceLock<Vec<Customization>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let v: Value = serde_json::from_str(DATA).unwrap_or(Value::Null);
        v.as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        Some(Customization {
                            path: e.get("path")?.as_str()?.to_string(),
                            name: e.get("name")?.as_str()?.to_string(),
                            is_head: e.get("type")?.as_str()? == "Head",
                            classes: e
                                .get("cls")?
                                .as_array()?
                                .iter()
                                .filter_map(|c| c.as_str().map(str::to_string))
                                .collect(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// The usage token (e.g. "Assassin", "Psycho") for a class-definition path.
pub fn class_token(class_def: &str) -> Option<&'static str> {
    // (substring in the class_def, usage token in the customization data)
    const MAP: [(&str, &str); 7] = [
        ("GD_Assassin", "Assassin"),
        ("GD_Siren", "Siren"),
        ("GD_Mercenary", "Mercenary"),
        ("GD_Soldier", "Soldier"),
        ("Mechromancer", "Mechromancer"),
        ("Lilac", "Psycho"),
        ("Psycho", "Psycho"),
    ];
    MAP.iter()
        .find(|(needle, _)| class_def.contains(needle))
        .map(|(_, tok)| *tok)
}

/// Every head (or skin) usable by the given class, sorted by display name.
pub fn for_class(class_def: &str, is_head: bool) -> Vec<Customization> {
    let Some(token) = class_token(class_def) else {
        return Vec::new();
    };
    let mut out: Vec<Customization> = all()
        .iter()
        .filter(|c| c.is_head == is_head && c.classes.iter().any(|t| t == token))
        .cloned()
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Display name for an equipped head/skin path, if known.
pub fn name(path: &str) -> Option<&'static str> {
    all()
        .iter()
        .find(|c| c.path == path)
        .map(|c| c.name.as_str())
}

/// The stock "Default" head/skin path for a class (a safe reset target).
pub fn default_path(class_def: &str, is_head: bool) -> Option<String> {
    let token = class_token(class_def)?;
    let kind = if is_head {
        "Head_Default"
    } else {
        "Skin_Default"
    };
    Some(format!("GD_DefaultCustoms_MainGame.{token}.{kind}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_tokens_map_correctly() {
        assert_eq!(
            class_token("GD_Assassin.Character.CharClass_Assassin"),
            Some("Assassin")
        );
        assert_eq!(
            class_token("GD_Siren.Character.CharClass_Siren"),
            Some("Siren")
        );
        assert_eq!(
            class_token("GD_Lilac_PlayerClass.Character.CharClass_LilacPlayerClass"),
            Some("Psycho")
        );
        assert_eq!(
            class_token("GD_Tulip_Mechromancer.Character.X"),
            Some("Mechromancer")
        );
        assert!(class_token("nonsense").is_none());
    }

    #[test]
    fn for_class_and_default() {
        let heads = for_class("GD_Siren.Character.CharClass_Siren", true);
        let skins = for_class("GD_Siren.Character.CharClass_Siren", false);
        assert!(!heads.is_empty() && !skins.is_empty());
        assert!(heads.iter().all(|c| c.is_head));
        assert!(skins.iter().all(|c| !c.is_head));
        assert!(name(&heads[0].path).is_some());
        assert!(default_path("GD_Siren.Character.CharClass_Siren", true)
            .unwrap()
            .contains("Head_Default"));
    }
}
