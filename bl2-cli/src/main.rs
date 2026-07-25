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
    Items { sav: PathBuf },
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
        Cmd::Items { sav } => cmd_items(&sav),
        Cmd::SetMoney { sav, amount, w } => {
            edit(&sav, w, "money", |s| s.money(), |s| s.set_money(amount))
        }
        Cmd::SetEridium { sav, amount, w } => {
            edit(&sav, w, "eridium", |s| s.eridium(), |s| s.set_eridium(amount))
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
    }
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

fn cmd_items(sav: &Path) -> Result<(), SaveError> {
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
        let parts = ser.parts.iter().filter(|p| p.is_some()).count();
        println!(
            "  {loc:<8}  {kind:<6}  Lv {:<3}  type {}:{}  bal {}:{}  manu {}:{}  parts {parts}",
            ser.stage.unwrap_or(0),
            ser.item_type.lib, ser.item_type.asset,
            ser.balance.lib, ser.balance.asset,
            ser.manufacturer.lib, ser.manufacturer.asset,
        );
    }
    println!("  ---");
    print!("  {weapons} weapons, {gear} items");
    if placeholders > 0 {
        print!(", {placeholders} placeholder(s) skipped");
    }
    println!();
    println!("  note: type/bal/manu are GameInfo db refs (lib:asset); human names need the GameInfo DB (future).");
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
