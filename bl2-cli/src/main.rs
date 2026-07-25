//! `bl2edit` — command-line Borderlands 2 save editor (frontend over `bl2-save`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bl2_save::{Location, SaveError, SaveFile};
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
    Customizations {
        sav: PathBuf,
    },
    /// Set the equipped head and skin by asset path (see `customizations`).
    SetHeadSkin {
        sav: PathBuf,
        head: String,
        skin: String,
        #[command(flatten)]
        w: WriteOpts,
    },
    /// Dump every top-level protobuf field (read-only inspector).
    Raw {
        sav: PathBuf,
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
    ExportCodes {
        sav: PathBuf,
    },
    /// Import a BL2(...) item code as a new backpack (or bank) item.
    ImportCode {
        sav: PathBuf,
        /// The BL2(...) code to import.
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
    if let Err(e) = run() {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run() -> Result<(), SaveError> {
    match Cli::parse().cmd {
        Cmd::Info { sav } => cmd_info(&sav),
        Cmd::Items { sav, parts } => cmd_items(&sav, parts),
        Cmd::SetMoney { sav, amount, w } => {
            edit(&sav, w, "money", |s| s.money(), |s| s.set_money(amount))
        }
        Cmd::SetEridium { sav, amount, w } => {
            edit(&sav, w, "eridium", |s| s.eridium(), |s| s.set_eridium(amount))
        }
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
        Cmd::SetXp { sav, xp, w } => {
            edit(&sav, w, "xp", |s| s.xp().unwrap_or(0), |s| s.set_xp(xp))
        }
        Cmd::SetItemLevels { sav, level, force, w } => cmd_set_item_levels(&sav, level, force, w),
        Cmd::PartCatalog { sav, id, filter } => cmd_part_catalog(&sav, id, &filter),
        Cmd::SetPart { sav, id, slot, lib, asset, w } => cmd_set_part(&sav, id, slot, lib, asset, w),
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
        Cmd::Raw { sav } => cmd_raw(&sav),
        Cmd::UnlockStations { sav, w } => cmd_unlock_stations(&sav, w),
        Cmd::ExportCodes { sav } => cmd_export_codes(&sav),
        Cmd::ImportCode { sav, code, bank, w } => cmd_import_code(&sav, &code, bank, w),
    }
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
    for p in cat.iter().filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle)) {
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
    println!("  item {id} slot {slot} -> {lib}:{asset}  ({})", if changed { "changed" } else { "no change — empty slot or bad id" });
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
    let all: Vec<String> = bl2_save::stations_catalog().iter().map(|st| st.rn.clone()).collect();
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

fn cmd_raw(sav: &Path) -> Result<(), SaveError> {
    let s = SaveFile::load(sav)?;
    println!("== raw protobuf fields of {} ==", sav.display());
    for f in s.raw_fields()? {
        let name = if f.name.is_empty() { format!("#{}", f.number) } else { f.name.to_string() };
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
    let before = s.items()?.len();
    s.add_item_from_code(code, bank)?;
    let out = w.out.as_deref().unwrap_or(sav);
    println!("== import code ==");
    println!("  input   : {}", sav.display());
    println!("  added   : 1 item into {} ({} -> {} entries)", if bank { "bank" } else { "backpack" }, before, before + 1);
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
        println!(
            "  [{:>2}] {loc:<8}  {kind:<6}  Lv {:<3}  {manu:<9} {ty:<22}  {balance}",
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
/// launch. Warn when writing into a savedata directory. (See PLAN.md §4.1.)
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
