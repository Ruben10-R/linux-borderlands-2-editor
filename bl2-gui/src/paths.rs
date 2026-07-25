//! Locate the Borderlands 2 save folder on this machine, so a freshly-created
//! character can be written straight into the game's `SaveData` directory.
//!
//! We probe where the game actually stores saves:
//!   - Linux (Proton): `~/.local/share/Steam/steamapps/compatdata/<appid>/pfx/`
//!     `drive_c/users/steamuser/Documents/My Games/Borderlands 2/WillowGame/SaveData/<id>/`
//!   - Linux (native Aspyr port): `~/.local/share/aspyr-media/borderlands 2/WillowGame/SaveData/<id>/`
//!   - Windows: `%USERPROFILE%\Documents\My Games\Borderlands 2\WillowGame\SaveData\<id>\`
//!     (and the OneDrive-redirected Documents folder)
//!
//! `<id>` is the per-Steam-account subfolder; we pick whichever holds the most
//! existing saves.

use std::fs;
use std::path::{Path, PathBuf};

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn is_save(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("save") && n.ends_with(".sav")
}

fn count_saves(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter(|e| is_save(&e.file_name().to_string_lossy()))
                .count()
        })
        .unwrap_or(0)
}

/// Resolve `rel` under `base`, matching each component case-insensitively. The
/// native Linux (Aspyr) port lowercases `willowgame/savedata`, while Proton
/// keeps the Windows `WillowGame/SaveData` casing — this finds either.
fn resolve_ci(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut cur = base.to_path_buf();
    for want in rel.split('/').filter(|c| !c.is_empty()) {
        let hit = fs::read_dir(&cur)
            .ok()?
            .flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(want))?;
        cur = hit.path();
    }
    Some(cur)
}

/// The `SaveData` folders worth probing (each holds per-account subfolders).
fn savedata_roots() -> Vec<PathBuf> {
    let Some(home) = home() else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    let mut add = |base: PathBuf, rel: &str| {
        if let Some(p) = resolve_ci(&base, rel) {
            roots.push(p);
        }
    };

    const WILLOW: &str = "My Games/Borderlands 2/WillowGame/SaveData";
    // Windows: Documents, plus the common OneDrive redirect.
    add(home.join("Documents"), WILLOW);
    add(home.join("OneDrive").join("Documents"), WILLOW);
    // Linux native port (Aspyr) — case varies, so resolve_ci handles it.
    add(
        home.join(".local/share/aspyr-media"),
        "Borderlands 2/WillowGame/SaveData",
    );
    // Linux Proton: any compatdata prefix (the appid differs across editions).
    let compat = home.join(".local/share/Steam/steamapps/compatdata");
    if let Ok(entries) = fs::read_dir(&compat) {
        for e in entries.flatten() {
            add(
                e.path().join("pfx/drive_c/users/steamuser/Documents"),
                WILLOW,
            );
        }
    }
    roots
}

/// The account `SaveData` folder holding the most saves, if one is found.
pub fn detect_save_dir() -> Option<PathBuf> {
    let mut best: Option<(usize, PathBuf)> = None;
    let mut consider = |n: usize, p: PathBuf| {
        if n > 0 && best.as_ref().is_none_or(|(bn, _)| n > *bn) {
            best = Some((n, p));
        }
    };
    for root in savedata_roots() {
        if !root.is_dir() {
            continue;
        }
        // Per-account subfolders (named by SteamID64).
        if let Ok(entries) = fs::read_dir(&root) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let n = count_saves(&p);
                    consider(n, p);
                }
            }
        }
        // Or saves directly under the root (rare).
        let n = count_saves(&root);
        consider(n, root);
    }
    best.map(|(_, p)| p)
}

/// The lowest slot number `1..=9999` with no `saveNNNN.sav` in `dir`.
pub fn next_slot(dir: &Path) -> u32 {
    (1..=9999u32)
        .find(|n| !dir.join(slot_filename(*n)).exists())
        .unwrap_or(1)
}

/// The BL2 save filename for a slot number, e.g. `5` → `"save0005.sav"`.
pub fn slot_filename(slot: u32) -> String {
    format!("save{slot:04}.sav")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_names_are_zero_padded() {
        assert_eq!(slot_filename(1), "save0001.sav");
        assert_eq!(slot_filename(42), "save0042.sav");
    }

    #[test]
    fn resolve_ci_matches_lowercase_aspyr_layout() {
        // The native Linux port lowercases these; we ask with Windows casing.
        let base = std::env::temp_dir().join(format!("bl2ci_{}", std::process::id()));
        let deep = base.join("borderlands 2/willowgame/savedata");
        fs::create_dir_all(&deep).unwrap();
        let got = resolve_ci(&base, "Borderlands 2/WillowGame/SaveData");
        assert_eq!(got.as_deref(), Some(deep.as_path()));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn next_slot_skips_existing_saves() {
        let dir = std::env::temp_dir().join(format!("bl2slot_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("save0001.sav"), b"x").unwrap();
        fs::write(dir.join("save0002.sav"), b"x").unwrap();
        assert_eq!(next_slot(&dir), 3);
        let _ = fs::remove_dir_all(&dir);
    }
}
