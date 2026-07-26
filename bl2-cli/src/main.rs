//! `bl2edit` — command-line Borderlands 2 save editor (frontend over `bl2-save`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bl2_save::{ImportGroup, Location, ProfileFile, SaveError, SaveFile};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bl2edit", version, about = "Edit Borderlands 2 save files")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show a character summary (class, level, xp, money, eridium).
    Info { sav: PathBuf },
    /// List backpack + bank items and weapons (read-only).
    Items {
        sav: PathBuf,
        /// Also list each item's resolved parts.
        #[arg(long)]
        parts: bool,
    },
    /// Set money (dollars).
    SetMoney {
        sav: PathBuf,
        amount: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set eridium.
    SetEridium {
        sav: PathBuf,
        amount: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set seraph crystals.
    SetSeraph {
        sav: PathBuf,
        amount: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set torgue tokens.
    SetTorgue {
        sav: PathBuf,
        amount: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set experience level (does not adjust xp or skill points).
    SetLevel {
        sav: PathBuf,
        level: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set experience points.
    SetXp {
        sav: PathBuf,
        xp: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// List available parts for an item (to find values for set-part).
    PartCatalog {
        sav: PathBuf,
        /// Item id (from `items`).
        id: usize,
        /// Only show parts whose name contains this (case-insensitive).
        #[arg(default_value = "")]
        filter: String,
    },
    /// Swap one part slot of an item. Find id via `items`, values via `part-catalog`.
    SetPart {
        sav: PathBuf,
        id: usize,
        slot: usize,
        lib: u32,
        asset: u32,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// List head/skin options for the save's class (name → asset path).
    Customizations { sav: PathBuf },
    /// Set the equipped head and skin by asset path (see `customizations`).
    SetHeadSkin {
        sav: PathBuf,
        head: String,
        skin: String,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Show account profile.bin info (Golden Keys, Badass Rank, tokens).
    ProfileInfo { profile: PathBuf },
    /// Set SHiFT Golden Keys in a profile.bin (0–255).
    SetGoldenKeys {
        profile: PathBuf,
        count: u8,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set Badass Rank in a profile.bin (grants the tokens to spend).
    SetBadassRank {
        profile: PathBuf,
        rank: i32,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Unlock (or --lock) every head/skin/vehicle-skin customization.
    UnlockCustomizations {
        profile: PathBuf,
        /// Lock all instead of unlocking.
        #[arg(long)]
        lock: bool,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Decode a file of BL2(...) codes into a JSON library (category/name/level).
    CodeIndex {
        /// Text file containing BL2(...) codes (any separators).
        file: PathBuf,
    },
    /// Dump every top-level protobuf field (read-only inspector).
    Raw { sav: PathBuf },
    /// Set the save-slot id (field 20). Keep it equal to the NNNN in
    /// saveNNNN.sav, or the game may renumber the slot and overwrite another
    /// save. The GUI does this for you when saving a new character.
    SetSaveId {
        sav: PathBuf,
        id: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set playthroughs completed (0-3): 1 unlocks TVHM, 2 unlocks UVHM.
    SetPlaythroughs {
        sav: PathBuf,
        count: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set the current playthrough (0=Normal, 1=TVHM, 2=UVHM).
    SetPlaythrough {
        sav: PathBuf,
        index: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set backpack slot count (12–39, snaps to +3 per SDU).
    SetBackpack {
        sav: PathBuf,
        slots: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set bank slot count (snaps to +2 per SDU).
    SetBank {
        sav: PathBuf,
        slots: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set unspent skill points for the main skill trees.
    SetSkillPoints {
        sav: PathBuf,
        points: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set specialist skill points.
    SetSpecialistPoints {
        sav: PathBuf,
        points: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set the unlocked Overpower level (0 clears it; needs lvl 72 + UVHM).
    SetOpLevel {
        sav: PathBuf,
        level: i64,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Unlock all fast-travel stations (base game + DLC).
    UnlockStations {
        sav: PathBuf,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Print shareable BL2(...) codes for every item (Gibbed-compatible).
    ExportCodes { sav: PathBuf },
    /// Import one or more BL2(...) item codes into the backpack (or bank).
    ImportCode {
        sav: PathBuf,
        /// One or many BL2(...) codes (any separators — quote the whole string).
        code: String,
        /// Add to the bank instead of the backpack.
        #[arg(long)]
        bank: bool,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Set every backpack + bank item and weapon to a level.
    SetItemLevels {
        sav: PathBuf,
        level: i64,
        /// Also level "no-level" items (grade ≤ 1). WARNING: can invalidate
        /// special/starter items (some grenades) so the game drops them.
        #[arg(long)]
        force: bool,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Level every backpack + bank item/weapon to the character's own level.
    SyncItemLevels {
        sav: PathBuf,
        /// Also level "no-level" items (grade ≤ 1).
        #[arg(long)]
        force: bool,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Create a new level-1 character (like Gibbed's "New").
    New {
        /// Class keyword: axton/commando, maya/siren, salvador/gunzerker,
        /// zer0/assassin, gaige/mechromancer, krieg/psycho.
        class: String,
        /// The character's in-game name.
        name: String,
        /// Where to write the new .sav.
        out: PathBuf,
        /// Overwrite the output file if it already exists (backs it up first).
        #[arg(long)]
        force: bool,
    },
    /// Copy groups (skills/missions/world/stats) from another save into this one.
    Import {
        /// The character save to import INTO (edited in place, with backup).
        sav: PathBuf,
        /// The source save to copy FROM.
        source: PathBuf,
        /// Skill tree + unspent skill points.
        #[arg(long)]
        skills: bool,
        /// Mission progress (all playthroughs).
        #[arg(long)]
        missions: bool,
        /// Fast-travel unlocks, map discovery, playthrough.
        #[arg(long)]
        world: bool,
        /// Stats, challenges, and total play time.
        #[arg(long)]
        stats: bool,
        /// Copy all four groups.
        #[arg(long)]
        all: bool,
        #[command(flatten)]
        w: WriteOpts,
    },
}

#[derive(Args)]
struct WriteOpts {
    /// Write to this path instead of editing the input in place.
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Do not create a `.bak` backup before overwriting.
    #[arg(long)]
    no_backup: bool,
    /// Print the change but write nothing.
    #[arg(long)]
    dry_run: bool,
}

fn main() -> ExitCode {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Dispatch a parsed command. Takes the `Cli` rather than parsing it here so
/// tests can drive real commands via `Cli::try_parse_from`.
fn run(cli: Cli) -> Result<(), SaveError> {
    match cli.cmd {
        Cmd::Info { sav } => cmd_info(&sav),
        Cmd::Items { sav, parts } => cmd_items(&sav, parts),
        Cmd::SetMoney { sav, amount, w } => {
            edit(&sav, w, "money", |s| s.money(), |s| s.set_money(amount))
        }
        Cmd::SetEridium { sav, amount, w } => edit(
            &sav,
            w,
            "eridium",
            |s| s.eridium(),
            |s| s.set_eridium(amount),
        ),
        Cmd::SetSeraph { sav, amount, w } => {
            edit(&sav, w, "seraph", |s| s.seraph(), |s| s.set_seraph(amount))
        }
        Cmd::SetTorgue { sav, amount, w } => {
            edit(&sav, w, "torgue", |s| s.torgue(), |s| s.set_torgue(amount))
        }
        Cmd::SetLevel { sav, level, w } => edit(
            &sav,
            w,
            "level",
            |s| s.level().unwrap_or(0),
            |s| s.set_level(level),
        ),
        Cmd::SetXp { sav, xp, w } => edit(&sav, w, "xp", |s| s.xp().unwrap_or(0), |s| s.set_xp(xp)),
        Cmd::SetItemLevels {
            sav,
            level,
            force,
            w,
        } => cmd_set_item_levels(&sav, level, force, w),
        Cmd::PartCatalog { sav, id, filter } => cmd_part_catalog(&sav, id, &filter),
        Cmd::SetPart {
            sav,
            id,
            slot,
            lib,
            asset,
            w,
        } => cmd_set_part(&sav, id, slot, lib, asset, w),
        Cmd::SetPlaythroughs { sav, count, w } => edit(
            &sav,
            w,
            "playthroughs completed",
            |s| s.playthroughs_completed().unwrap_or(0),
            |s| s.set_playthroughs_completed(count.clamp(0, 3)),
        ),
        Cmd::SetPlaythrough { sav, index, w } => edit(
            &sav,
            w,
            "current playthrough",
            |s| s.active_playthrough(),
            |s| s.set_active_playthrough(index.clamp(0, 2)),
        ),
        Cmd::SetBackpack { sav, slots, w } => edit(
            &sav,
            w,
            "backpack slots",
            |s| s.backpack_size().unwrap_or(12),
            |s| s.set_backpack_size(slots),
        ),
        Cmd::SetBank { sav, slots, w } => edit(
            &sav,
            w,
            "bank slots",
            |s| s.bank_size(),
            |s| s.set_bank_size(slots),
        ),
        Cmd::SetSkillPoints { sav, points, w } => edit(
            &sav,
            w,
            "skill points",
            |s| s.skill_points().unwrap_or(0),
            |s| s.set_skill_points(points.max(0)),
        ),
        Cmd::SetSpecialistPoints { sav, points, w } => edit(
            &sav,
            w,
            "specialist points",
            |s| s.specialist_skill_points().unwrap_or(0),
            |s| s.set_specialist_skill_points(points.max(0)),
        ),
        Cmd::SetOpLevel { sav, level, w } => edit(
            &sav,
            w,
            "op level",
            |s| s.op_level().unwrap_or(0),
            |s| s.set_op_level(level.clamp(0, 80)),
        ),
        Cmd::Customizations { sav } => cmd_customizations(&sav),
        Cmd::SetHeadSkin { sav, head, skin, w } => cmd_set_head_skin(&sav, &head, &skin, w),
        Cmd::ProfileInfo { profile } => cmd_profile_info(&profile),
        Cmd::SetGoldenKeys { profile, count, w } => cmd_set_golden_keys(&profile, count, w),
        Cmd::SetBadassRank { profile, rank, w } => cmd_set_badass_rank(&profile, rank, w),
        Cmd::UnlockCustomizations { profile, lock, w } => {
            cmd_unlock_customizations(&profile, !lock, w)
        }
        Cmd::CodeIndex { file } => cmd_code_index(&file),
        Cmd::Raw { sav } => cmd_raw(&sav),
        Cmd::SetSaveId { sav, id, w } => edit(
            &sav,
            w,
            "save id",
            |s| s.save_game_id().unwrap_or(0),
            |s| s.set_raw_varint(20, id.max(1)),
        ),
        Cmd::UnlockStations { sav, w } => cmd_unlock_stations(&sav, w),
        Cmd::ExportCodes { sav } => cmd_export_codes(&sav),
        Cmd::ImportCode { sav, code, bank, w } => cmd_import_code(&sav, &code, bank, w),
        Cmd::SyncItemLevels { sav, force, w } => cmd_sync_item_levels(&sav, force, w),
        Cmd::New {
            class,
            name,
            out,
            force,
        } => cmd_new(&class, &name, &out, force),
        Cmd::Import {
            sav,
            source,
            skills,
            missions,
            world,
            stats,
            all,
            w,
        } => cmd_import(
            &sav,
            &source,
            [skills || all, missions || all, world || all, stats || all],
            w,
        ),
    }
}

/// Match a class keyword (name or class-def fragment) to one of the six classes.
fn resolve_class(input: &str) -> Option<(&'static str, &'static str)> {
    let n = input.to_lowercase();
    bl2_save::CLASSES.iter().copied().find(|(display, def)| {
        display.to_lowercase().contains(&n) || def.to_lowercase().contains(&n)
    })
}

fn cmd_new(class: &str, name: &str, out: &Path, force: bool) -> Result<(), SaveError> {
    let Some((display, class_def)) = resolve_class(class) else {
        eprintln!("unknown class '{class}'. Try: axton, maya, salvador, zer0, gaige, krieg.");
        return Ok(());
    };
    if out.exists() && !force {
        eprintln!(
            "{} already exists (use --force to overwrite)",
            out.display()
        );
        return Ok(());
    }
    let s = SaveFile::new_character(class_def, name);
    let backup = out.exists();
    s.save(out, backup)?;
    println!("== new character ==");
    println!("  class   : {display}");
    println!("  name    : {name}");
    println!("  wrote   : {}", out.display());
    if backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_import(sav: &Path, source: &Path, groups: [bool; 4], w: WriteOpts) -> Result<(), SaveError> {
    if !groups.iter().any(|g| *g) {
        eprintln!("pick a group: --skills --missions --world --stats (or --all)");
        return Ok(());
    }
    let mut s = SaveFile::load(sav)?;
    let src = SaveFile::load(source)?;
    let all = [
        ImportGroup::Skills,
        ImportGroup::Missions,
        ImportGroup::World,
        ImportGroup::Stats,
    ];
    println!("== import ==");
    println!("  into    : {}", sav.display());
    println!("  from    : {}", source.display());
    for (g, on) in all.iter().zip(groups) {
        if on {
            s.import_group(&src, *g)?;
            println!("  imported: {}", g.label());
        }
    }
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let out = w.out.as_deref().unwrap_or(sav);
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    s.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_sync_item_levels(sav: &Path, force: bool, w: WriteOpts) -> Result<(), SaveError> {
    let mut s = SaveFile::load(sav)?;
    let level = s.level().unwrap_or(0);
    let n = s.set_all_item_levels(level, force)?;
    let out = w.out.as_deref().unwrap_or(sav);
    println!("== sync item levels ==");
    println!("  input   : {}", sav.display());
    println!("  leveled : {n} items/weapons -> character Lv {level}");
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    s.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_part_catalog(sav: &Path, id: usize, filter: &str) -> Result<(), SaveError> {
    let s = SaveFile::load(sav)?;
    let Some(item) = s.items()?.into_iter().find(|it| it.id == id) else {
        eprintln!("no item with id {id} (see `items`)");
        return Ok(());
    };
    let needle = filter.to_lowercase();
    let cat = bl2_save::parts_catalog(item.serial.is_weapon, item.serial.set);
    println!("== parts for item {id} ({} matches) ==", cat.len());
    for p in cat
        .iter()
        .filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle))
    {
        println!("  {}:{}  {}", p.lib, p.asset, p.name);
    }
    Ok(())
}

fn cmd_set_part(
    sav: &Path,
    id: usize,
    slot: usize,
    lib: u32,
    asset: u32,
    w: WriteOpts,
) -> Result<(), SaveError> {
    let mut s = SaveFile::load(sav)?;
    let changed = s.set_item_part(id, slot, lib, asset)?;
    println!("== set part ==");
    println!(
        "  item {id} slot {slot} -> {lib}:{asset}  ({})",
        if changed {
            "changed"
        } else {
            "no change — empty slot or bad id"
        }
    );
    if !changed || w.dry_run {
        if w.dry_run {
            println!("  dry-run : nothing written");
        }
        return Ok(());
    }
    let out = w.out.as_deref().unwrap_or(sav);
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    s.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_unlock_stations(sav: &Path, w: WriteOpts) -> Result<(), SaveError> {
    let mut s = SaveFile::load(sav)?;
    let before = s.visited_stations().len();
    let all: Vec<String> = bl2_save::stations_catalog()
        .iter()
        .map(|st| st.rn.clone())
        .collect();
    s.set_visited_stations(&all)?;
    let out = w.out.as_deref().unwrap_or(sav);
    println!("== unlock stations ==");
    println!("  input   : {}", sav.display());
    println!("  stations: {before} -> {} unlocked", all.len());
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    s.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_set_head_skin(sav: &Path, head: &str, skin: &str, w: WriteOpts) -> Result<(), SaveError> {
    let mut s = SaveFile::load(sav)?;
    s.set_wearing(head, skin)?;
    let out = w.out.as_deref().unwrap_or(sav);
    println!("== set head/skin ==");
    println!("  head    : {head}");
    println!("  skin    : {skin}");
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    s.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_customizations(sav: &Path) -> Result<(), SaveError> {
    let s = SaveFile::load(sav)?;
    let class = s.class_def().unwrap_or_default();
    for (label, is_head) in [("Heads", true), ("Skins", false)] {
        println!("== {label} for {} ==", s.class_name().unwrap_or_default());
        for c in bl2_save::customizations(&class, is_head) {
            println!("  {:<28}  {}", c.name, c.path);
        }
    }
    Ok(())
}

fn cmd_profile_info(profile: &Path) -> Result<(), SaveError> {
    let p = ProfileFile::load(profile)?;
    println!("== profile {} ==", profile.display());
    println!(
        "  golden keys : {}",
        p.golden_keys()
            .map(|k| k.to_string())
            .unwrap_or_else(|| "0 (none)".into())
    );
    println!(
        "  badass rank : {}",
        p.badass_rank()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "?".into())
    );
    println!(
        "  badass tokens (unspent) : {}",
        p.badass_tokens()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "?".into())
    );
    Ok(())
}

fn cmd_set_golden_keys(profile: &Path, count: u8, w: WriteOpts) -> Result<(), SaveError> {
    let mut p = ProfileFile::load(profile)?;
    let before = p.golden_keys().unwrap_or(0);
    p.set_golden_keys(count)?;
    let out = w.out.as_deref().unwrap_or(profile);
    println!("== set golden keys ==");
    println!("  input   : {}", profile.display());
    println!("  keys    : {before} -> {count}");
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    p.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_unlock_customizations(profile: &Path, unlock: bool, w: WriteOpts) -> Result<(), SaveError> {
    let mut p = ProfileFile::load(profile)?;
    p.set_all_customizations(unlock)?;
    let out = w.out.as_deref().unwrap_or(profile);
    let (u, total) = p.customization_stats().unwrap_or((0, 0));
    println!(
        "== {} customizations ==",
        if unlock { "unlock" } else { "lock" }
    );
    println!("  input   : {}", profile.display());
    println!("  result  : {u} / {total} unlocked");
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    p.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_set_badass_rank(profile: &Path, rank: i32, w: WriteOpts) -> Result<(), SaveError> {
    let mut p = ProfileFile::load(profile)?;
    let before = p.badass_rank().unwrap_or(0);
    p.set_badass_rank(rank)?;
    let after = p.badass_rank().unwrap_or(0);
    let out = w.out.as_deref().unwrap_or(profile);
    println!("== set badass rank ==");
    println!("  input   : {}", profile.display());
    println!(
        "  rank    : {before} -> {after}  (tokens available: {})",
        p.badass_tokens().unwrap_or(0)
    );
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    p.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

/// Escape a string for embedding in a JSON string literal.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn cmd_code_index(file: &Path) -> Result<(), SaveError> {
    let text = std::fs::read_to_string(file)?;
    let codes = bl2_save::extract_codes(&text);
    let esc = json_escape;
    let mut ok = 0usize;
    println!("[");
    let mut first = true;
    for code in &codes {
        if let Some(info) = bl2_save::describe_code(code) {
            if !first {
                println!(",");
            }
            first = false;
            print!(
                "  {{\"code\":\"{}\",\"category\":\"{}\",\"name\":\"{}\",\"family\":\"{}\",\"level\":{}}}",
                esc(code),
                info.category,
                esc(&info.name),
                esc(&info.family),
                info.level
            );
            ok += 1;
        }
    }
    println!("\n]");
    eprintln!("indexed {ok}/{} codes", codes.len());
    Ok(())
}

fn cmd_raw(sav: &Path) -> Result<(), SaveError> {
    let s = SaveFile::load(sav)?;
    println!("== raw protobuf fields of {} ==", sav.display());
    for f in s.raw_fields()? {
        let name = if f.name.is_empty() {
            format!("#{}", f.number)
        } else {
            f.name.to_string()
        };
        println!("  {name:<24} {:<11} {}", f.kind, f.preview);
    }
    Ok(())
}

fn cmd_export_codes(sav: &Path) -> Result<(), SaveError> {
    let s = SaveFile::load(sav)?;
    let items = s.items()?;
    eprintln!("== item codes in {} ==", sav.display());
    let mut n = 0;
    for it in &items {
        if let Some(code) = s.item_code(it.id)? {
            println!("{code}");
            n += 1;
        }
    }
    eprintln!("  {n} codes exported");
    Ok(())
}

fn cmd_import_code(sav: &Path, code: &str, bank: bool, w: WriteOpts) -> Result<(), SaveError> {
    let mut s = SaveFile::load(sav)?;
    // Accepts one or many BL2(...) codes in `code` (any separators).
    let (added, failed) = s.add_items_from_codes(code, bank);
    if added == 0 {
        eprintln!("no valid BL2(...) codes found");
        return Ok(());
    }
    let out = w.out.as_deref().unwrap_or(sav);
    println!("== import code ==");
    println!("  input   : {}", sav.display());
    println!(
        "  added   : {added} item(s) into {}{}",
        if bank { "bank" } else { "backpack" },
        if failed > 0 {
            format!(" ({failed} failed)")
        } else {
            String::new()
        }
    );
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    s.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_set_item_levels(sav: &Path, level: i64, force: bool, w: WriteOpts) -> Result<(), SaveError> {
    let mut s = SaveFile::load(sav)?;
    let n = s.set_all_item_levels(level, force)?;
    let out = w.out.as_deref().unwrap_or(sav);
    println!("== set item levels ==");
    println!("  input   : {}", sav.display());
    println!("  leveled : {n} items/weapons -> Lv {level}");
    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    s.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

fn cmd_info(sav: &Path) -> Result<(), SaveError> {
    let s = SaveFile::load(sav)?;
    println!("== {} ==", sav.display());
    if let Some(c) = s.class_name() {
        println!("  class   : {c}");
    }
    if let Some(l) = s.level() {
        println!("  level   : {l}");
    }
    if let Some(x) = s.xp() {
        println!("  xp      : {x}");
    }
    println!("  money   : {}", s.money());
    println!("  eridium : {}", s.eridium());
    Ok(())
}

fn cmd_items(sav: &Path, show_parts: bool) -> Result<(), SaveError> {
    let s = SaveFile::load(sav)?;
    let items = s.items()?;
    println!("== items in {} ==", sav.display());
    let (mut weapons, mut gear, mut placeholders) = (0u32, 0u32, 0u32);
    for it in &items {
        let ser = &it.serial;
        if ser.is_placeholder() {
            placeholders += 1;
            continue;
        }
        let loc = match it.location {
            Location::Backpack => "backpack",
            Location::Bank => "bank",
        };
        let kind = if ser.is_weapon {
            weapons += 1;
            "weapon"
        } else {
            gear += 1;
            "item"
        };
        let manu = ser
            .manufacturer_name()
            .unwrap_or_else(|| format!("manu {}:{}", ser.manufacturer.lib, ser.manufacturer.asset));
        let ty = ser
            .type_name()
            .unwrap_or_else(|| format!("type {}:{}", ser.item_type.lib, ser.item_type.asset));
        let balance = ser
            .balance_name()
            .unwrap_or_else(|| format!("bal {}:{}", ser.balance.lib, ser.balance.asset));
        let name = ser.display_name().unwrap_or_else(|| format!("{manu} {ty}"));
        println!(
            "  [{:>2}] {loc:<8}  {kind:<6}  Lv {:<3}  {name:<26}  ({manu} {ty}, {balance})",
            it.id,
            ser.stage.unwrap_or(0),
        );
        if show_parts {
            for (i, part) in ser.part_names().iter().enumerate() {
                if let Some(n) = part {
                    println!("      part {i:>2}: {n}");
                }
            }
        }
    }
    println!("  ---");
    print!("  {weapons} weapons, {gear} items");
    if placeholders > 0 {
        print!(", {placeholders} placeholder(s) skipped");
    }
    println!();
    Ok(())
}

/// Shared edit flow: load, show old→new, then write (unless `--dry-run`).
fn edit(
    sav: &Path,
    w: WriteOpts,
    label: &str,
    read: impl Fn(&SaveFile) -> i64,
    apply: impl FnOnce(&mut SaveFile) -> Result<(), SaveError>,
) -> Result<(), SaveError> {
    let mut s = SaveFile::load(sav)?;
    let old = read(&s);
    apply(&mut s)?;
    let new = read(&s);

    let out = w.out.as_deref().unwrap_or(sav);
    println!("== set {label} ==");
    println!("  input   : {}", sav.display());
    println!("  {label:<7} : {old} -> {new}");

    if w.dry_run {
        println!("  dry-run : nothing written");
        return Ok(());
    }

    // A backup is only made when overwriting a file that already exists.
    let backup = !w.no_backup;
    let did_backup = backup && out.exists();
    s.save(out, backup)?;
    println!("  wrote   : {}", out.display());
    if did_backup {
        println!("  backup  : {}.bak", out.display());
    }
    warn_if_steam_cloud(out);
    Ok(())
}

/// BL2's Aspyr port uses Steam Auto-Cloud, which can revert on-disk edits at
/// launch. Warn when writing into a savedata directory.
fn warn_if_steam_cloud(path: &Path) {
    let p = path.to_string_lossy().to_lowercase();
    if p.contains("savedata") {
        eprintln!(
            "  note    : this looks like a live savedata dir. Steam Auto-Cloud may\n\
             \x20           revert this edit at launch — quit Steam fully, then edit,\n\
             \x20           then let Steam upload on startup; and verify in-game."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch dir unique to the calling test, cleaned up by the caller.
    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bl2cli_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A fresh level-1 save on disk to run commands against.
    fn fixture(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        SaveFile::new_character("GD_Mercenary.Character.CharClass_Mercenary", "Sal")
            .save(&p, false)
            .unwrap();
        p
    }

    /// Parse an argv the way the real binary does, then dispatch it.
    fn cli(args: &[&str]) -> Result<(), SaveError> {
        let parsed = Cli::try_parse_from(args).expect("argv should parse");
        run(parsed)
    }

    #[test]
    fn set_save_id_writes_field_20() {
        let dir = tmpdir("saveid");
        let sav = fixture(&dir, "save0007.sav");
        // A fresh character is always slot 1; the file name says 7.
        assert_eq!(SaveFile::load(&sav).unwrap().save_game_id(), Some(1));

        cli(&["bl2edit", "set-save-id", sav.to_str().unwrap(), "7"]).unwrap();

        let after = SaveFile::load(&sav).unwrap();
        assert_eq!(after.save_game_id(), Some(7), "slot id written");
        // The guarded edit must not disturb anything else.
        assert_eq!(after.level(), Some(1));
        assert_eq!(after.name().as_deref(), Some("Sal"));
        assert_eq!(after.money(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_save_id_clamps_to_a_real_slot() {
        // Slot 0 and negatives aren't valid save slots; the command clamps to 1
        // rather than writing a value the game would choke on.
        let dir = tmpdir("saveid_clamp");
        let sav = fixture(&dir, "save0001.sav");
        let p = sav.to_str().unwrap();

        cli(&["bl2edit", "set-save-id", p, "0"]).unwrap();
        assert_eq!(
            SaveFile::load(&sav).unwrap().save_game_id(),
            Some(1),
            "slot 0 clamps to 1"
        );

        // A bare "-5" is rejected by clap as an unknown flag, so a negative can
        // only arrive after "--" — clamped the same way.
        assert!(Cli::try_parse_from(["bl2edit", "set-save-id", p, "-5"]).is_err());
        cli(&["bl2edit", "set-save-id", p, "--", "-5"]).unwrap();
        assert_eq!(
            SaveFile::load(&sav).unwrap().save_game_id(),
            Some(1),
            "negative slot clamps to 1"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_skill_points_writes_and_clamps() {
        let dir = tmpdir("skill");
        let sav = fixture(&dir, "save0001.sav");
        assert_eq!(SaveFile::load(&sav).unwrap().skill_points(), Some(0));

        cli(&["bl2edit", "set-skill-points", sav.to_str().unwrap(), "26"]).unwrap();
        let after = SaveFile::load(&sav).unwrap();
        assert_eq!(after.skill_points(), Some(26));
        assert_eq!(after.level(), Some(1), "nothing else moved");
        assert_eq!(
            after.specialist_skill_points(),
            Some(0),
            "other pool intact"
        );

        // Negative points are meaningless — clamp to 0. clap rejects a bare
        // "-3" as an unknown flag, so it can only arrive after "--".
        assert!(Cli::try_parse_from(["bl2edit", "set-skill-points", "x.sav", "-3"]).is_err());
        cli(&[
            "bl2edit",
            "set-skill-points",
            sav.to_str().unwrap(),
            "--",
            "-3",
        ])
        .unwrap();
        assert_eq!(SaveFile::load(&sav).unwrap().skill_points(), Some(0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_commands_honour_dry_run_out_and_backup() {
        let dir = tmpdir("writeopts");
        let sav = fixture(&dir, "save0001.sav");

        // --dry-run reports the change but leaves the file alone.
        cli(&[
            "bl2edit",
            "set-skill-points",
            sav.to_str().unwrap(),
            "15",
            "--dry-run",
        ])
        .unwrap();
        assert_eq!(
            SaveFile::load(&sav).unwrap().skill_points(),
            Some(0),
            "dry-run wrote nothing"
        );
        assert!(
            !dir.join("save0001.sav.bak").exists(),
            "dry-run made no .bak"
        );

        // --out redirects, leaving the input untouched.
        let out = dir.join("copy.sav");
        cli(&[
            "bl2edit",
            "set-save-id",
            sav.to_str().unwrap(),
            "9",
            "--out",
            out.to_str().unwrap(),
        ])
        .unwrap();
        assert_eq!(SaveFile::load(&out).unwrap().save_game_id(), Some(9));
        assert_eq!(
            SaveFile::load(&sav).unwrap().save_game_id(),
            Some(1),
            "input untouched by --out"
        );

        // An in-place edit backs the old file up by default...
        cli(&["bl2edit", "set-skill-points", sav.to_str().unwrap(), "5"]).unwrap();
        let bak = dir.join("save0001.sav.bak");
        assert!(bak.exists(), "backup written by default");
        assert_eq!(
            SaveFile::load(&bak).unwrap().skill_points(),
            Some(0),
            "backup holds the pre-edit value"
        );

        // ...and --no-backup skips refreshing it.
        std::fs::remove_file(&bak).unwrap();
        cli(&[
            "bl2edit",
            "set-skill-points",
            sav.to_str().unwrap(),
            "7",
            "--no-backup",
        ])
        .unwrap();
        assert!(!bak.exists(), "--no-backup wrote no .bak");
        assert_eq!(SaveFile::load(&sav).unwrap().skill_points(), Some(7));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_commands_report_a_missing_file_as_an_error() {
        let missing = std::env::temp_dir().join("bl2cli_does_not_exist.sav");
        assert!(cli(&["bl2edit", "set-save-id", missing.to_str().unwrap(), "3"]).is_err());
        assert!(cli(&[
            "bl2edit",
            "set-skill-points",
            missing.to_str().unwrap(),
            "3"
        ])
        .is_err());
    }

    #[test]
    fn new_commands_reject_bad_argv() {
        // Both take exactly <SAV> <VALUE>; a non-numeric value or a missing
        // argument must fail parsing rather than be silently coerced.
        for args in [
            vec!["bl2edit", "set-save-id"],
            vec!["bl2edit", "set-save-id", "x.sav"],
            vec!["bl2edit", "set-save-id", "x.sav", "notanumber"],
            vec!["bl2edit", "set-skill-points"],
            vec!["bl2edit", "set-skill-points", "x.sav"],
            vec!["bl2edit", "set-skill-points", "x.sav", "1.5"],
        ] {
            assert!(
                Cli::try_parse_from(&args).is_err(),
                "argv {args:?} should not parse"
            );
        }
        // The documented forms do parse.
        assert!(Cli::try_parse_from(["bl2edit", "set-save-id", "x.sav", "3"]).is_ok());
        assert!(
            Cli::try_parse_from(["bl2edit", "set-skill-points", "x.sav", "26", "--dry-run"])
                .is_ok()
        );
    }

    /// clap's own invariants: every subcommand and flag stays wired up.
    #[test]
    fn command_definition_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn resolve_class_accepts_names_and_paths() {
        // Vault hunter names, class names, and raw def fragments all resolve.
        for (input, expect) in [
            ("axton", "GD_Soldier.Character.CharClass_Soldier"),
            ("commando", "GD_Soldier.Character.CharClass_Soldier"),
            ("MAYA", "GD_Siren.Character.CharClass_Siren"),
            ("siren", "GD_Siren.Character.CharClass_Siren"),
            ("salvador", "GD_Mercenary.Character.CharClass_Mercenary"),
            ("gunzerker", "GD_Mercenary.Character.CharClass_Mercenary"),
            ("zer0", "GD_Assassin.Character.CharClass_Assassin"),
            ("assassin", "GD_Assassin.Character.CharClass_Assassin"),
            (
                "gaige",
                "GD_Tulip_Mechromancer.Character.CharClass_Mechromancer",
            ),
            (
                "krieg",
                "GD_Lilac_PlayerClass.Character.CharClass_LilacPlayerClass",
            ),
            (
                "psycho",
                "GD_Lilac_PlayerClass.Character.CharClass_LilacPlayerClass",
            ),
        ] {
            let got = resolve_class(input)
                .unwrap_or_else(|| panic!("'{input}' should resolve"))
                .1;
            assert_eq!(got, expect, "input '{input}'");
        }
        assert!(resolve_class("bandit").is_none());
        assert!(resolve_class("").is_some(), "empty matches the first class");
    }

    #[test]
    fn resolved_classes_build_a_valid_save() {
        // Every keyword must produce a class the core actually accepts.
        for kw in ["axton", "maya", "salvador", "zer0", "gaige", "krieg"] {
            let (_, def) = resolve_class(kw).unwrap();
            let save = SaveFile::new_character(def, "Test");
            assert!(save.to_bytes().is_ok(), "{kw} produces a valid save");
            assert_eq!(save.class_def().as_deref(), Some(def));
        }
    }

    #[test]
    fn json_escape_produces_valid_json_strings() {
        assert_eq!(json_escape("plain"), "plain");
        assert_eq!(json_escape(r#"a"b"#), r#"a\"b"#);
        assert_eq!(json_escape(r"a\b"), r"a\\b");
        assert_eq!(json_escape("a\nb\tc\rd"), "a\\nb\\tc\\rd");
        assert_eq!(json_escape("a\u{0}b"), "a\\u0000b");
        // A BL2 code's characters (including / and +) pass through untouched.
        assert_eq!(json_escape("BL2(aa/bb+cc)"), "BL2(aa/bb+cc)");
    }

    #[test]
    fn write_opts_default_to_in_place_with_backup() {
        // The flag combination that decides where bytes land, exercised via the
        // same expressions the commands use.
        let w = WriteOpts {
            out: None,
            no_backup: false,
            dry_run: false,
        };
        let sav = Path::new("save0001.sav");
        assert_eq!(w.out.as_deref().unwrap_or(sav), sav, "defaults to in place");
        assert!(!w.no_backup, "backups on by default");

        let w = WriteOpts {
            out: Some(PathBuf::from("other.sav")),
            no_backup: true,
            dry_run: true,
        };
        assert_eq!(
            w.out.as_deref().unwrap_or(sav),
            Path::new("other.sav"),
            "--out redirects"
        );
    }
}
