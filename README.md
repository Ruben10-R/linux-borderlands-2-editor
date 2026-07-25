# BL2 Save Editor  ·  rust-borderlands-2-editor

[![CI](https://github.com/Ruben10-R/rust-borderlands-2-editor/actions/workflows/ci.yml/badge.svg)](https://github.com/Ruben10-R/rust-borderlands-2-editor/actions/workflows/ci.yml)

A **Borderlands 2 save editor written in Rust** — not a port, no Wine / Mono /
.NET. It runs natively on **Linux and Windows** and in the browser. One
`bl2-save` core library powers three frontends from the same code:

- **Desktop app** (`bl2-editor`) — a native window; drag a file in, edit, Save.
- **Web app** — the same editor in your browser via `docker compose up`.
- **CLI** (`bl2edit`) — scriptable edits from the terminal.

It edits both **character saves** (`SaveNNNN.sav`) and your **account profile**
(`profile.bin` — Golden Keys, Badass Rank, customization unlocks), which is
**more than Gibbed's editor can do**.

> ⚠️ **Always back up first.** Editing can corrupt a save if misused. The tools
> write a `.bak` next to the file, but keep your own copy too. Test one edit
> in-game before relying on it.

---

## Quick start

Everything runs through Docker, so **your machine stays toolchain-free** (no Rust
install needed).

### Desktop app (native, edits files in place)
```bash
./build-native.sh        # compiles ./dist/bl2-editor and ./dist/bl2edit
./dist/bl2-editor        # launch the window
./install-desktop.sh     # (optional) add it to your applications menu
```
In the app: **Open…** a `.sav` or `profile.bin` (or drag it onto the window) →
edit → **Save** (writes back to the same file, keeping a `.bak`) or **Save As…**
to choose a new location. No download/rewrite dance — it saves in place.

> The native file dialogs use GTK; any Linux desktop already has the GTK 3
> runtime (`libgtk-3-0`). The web build uses the browser's own file picker.

### Windows app (`.exe`, cross-compiled from Linux)
```bash
./build-windows.sh       # → dist/bl2-editor.exe and dist/bl2edit.exe
```
Copy `dist/bl2-editor.exe` to a Windows PC and **double-click** it — it's a
normal windowed app (native file dialogs, embedded icon, no console popup).
`dist/bl2edit.exe` is the command-line tool. No Windows machine is needed to
*build* them; the cross-compile runs in Docker on your Linux box.

### Web app (browser)
```bash
docker compose up        # first run compiles; then open http://localhost:8080
```
Drag a file in → edit → **Download edited save/profile**, then copy the download
over your original (browsers can't write files directly).

### Command line
```bash
./dist/bl2edit --help                          # after ./build-native.sh
# or without building a binary, straight from source:
./run.sh run -p bl2-cli -- --help
```

---

## What it edits

**Character save (`SaveNNNN.sav`)**
- **Character** — name, class, head & skin, level (+ XP *Sync*), XP, skill points, specialist points
- **General** — playthroughs completed (unlock TVHM/UVHM), current playthrough, Overpower level, backpack/bank slots, save info
- **Currency** — money, eridium, seraph crystals, torgue tokens
- **Items** — per-item level, part swapping, shareable **`BL2(…)` item codes** (export, and **batch import** — paste many codes separated by any of `, | \ /`), a built-in **code library** (browse thousands of weapon codes, filter by category, one-click add), backpack ↔ bank
- **Fast Travel** — unlock stations (base game + DLC)
- **Vehicle** — equip vehicle skins (Runner / Bandit Technical / Hovercraft / Fan Boat, two slots each)
- **Raw** — a named, searchable inspector (**all 58 fields labelled**); edit scalar fields directly (with hover help)

**Account profile (`profile.bin`)** — drag it into the app just like a save
- **Golden Keys** (0–255)
- **Badass Rank** (grants the tokens to spend)
- **Unlock all customizations** (every head, skin, vehicle skin)

---

## Where your files live (Linux / Steam)

```
Saves:   ~/.local/share/aspyr-media/borderlands 2/willowgame/savedata/<SteamID>/SaveNNNN.sav
Profile: ~/.local/share/aspyr-media/borderlands 2/willowgame/savedata/<SteamID>/profile.bin
```

**Steam Cloud:** if a `steam_autocloud.vdf` sits next to your saves, Steam Cloud
is on. It usually syncs your edited file up fine; if you ever see edits revert,
launch offline (or disable Cloud for BL2) once.

---

## CLI reference (`bl2edit`)

Every write command takes optional `--out <file>` (write elsewhere),
`--no-backup`, and `--dry-run`. Read commands are read-only.

### Character save
| Command | What it does |
|---|---|
| `info <sav>` | Summary: class, level, xp, money, eridium |
| `items <sav> [--parts]` | List backpack + bank items/weapons |
| `set-money <sav> <n>` · `set-eridium` · `set-seraph` · `set-torgue` | Currencies |
| `set-level <sav> <n>` · `set-xp <sav> <n>` | Level / XP |
| `set-playthroughs <sav> <0-3>` | Playthroughs completed (1=TVHM, 2=UVHM) |
| `set-playthrough <sav> <0-2>` | Current playthrough (0=NVHM,1=TVHM,2=UVHM) |
| `set-op-level <sav> <n>` | Overpower level |
| `set-specialist-points <sav> <n>` | Specialist skill points |
| `set-backpack <sav> <12-39>` · `set-bank <sav> <n>` | Inventory slots (SDU) |
| `part-catalog <sav> <id> [filter]` · `set-part <sav> <id> <slot> <lib> <asset>` | Item parts |
| `set-item-levels <sav> <lvl> [--force]` | Level every item |
| `export-codes <sav>` · `import-code <sav> "BL2(...)" [--bank]` | Shareable item codes |
| `customizations <sav>` · `set-head-skin <sav> <head> <skin>` | Equipped head/skin |
| `unlock-stations <sav>` | Unlock all fast-travel stations |
| `raw <sav>` | Dump every top-level field |

### Account profile
| Command | What it does |
|---|---|
| `profile-info <profile.bin>` | Golden Keys, Badass Rank, tokens |
| `set-golden-keys <profile.bin> <0-255>` | SHiFT Golden Keys |
| `set-badass-rank <profile.bin> <rank>` | Badass Rank (+ tokens to spend) |
| `unlock-customizations <profile.bin> [--lock]` | Unlock (or lock) all customizations |

Example:
```bash
cp profile.bin profile.bin.backup
./dist/bl2edit set-golden-keys profile.bin 110
```

---

## Building from source

- `./run.sh <cargo args>` — run cargo in the `rust:1` container (CLI + core lib + tests).
- `./run.sh test` — run the test suite.
- `./build-native.sh` — release binaries for the desktop app + CLI into `./dist/`.
- `docker compose up` — serve the web build at `localhost:8080`.

The native GUI is excluded from the default cargo build (it needs GL/X11 libs);
`build-native.sh` provides those via `docker/native.Dockerfile`.

### Layout
- `bl2-save/` — core library: save/profile codec, item serials, GameInfo data, all edits.
- `bl2-cli/` — the `bl2edit` command-line tool.
- `bl2-gui/` — the egui/eframe app (native window + web/WASM canvas).
- `samples/` — copies of real saves/profiles for testing (git-ignored; never the live files).
- `PLAN.md` — the full implementation history and format notes.

---

## Credits & IP

All icons and themes are **original art drawn in code** — no Gearbox/2K assets
are bundled (see [`ASSETS.md`](./ASSETS.md)). The save/profile **file formats** and
item/customization **identifier data** were understood from open-source projects —
[Gibbed's tools](https://github.com/gibbed/Gibbed.Borderlands2) (zlib),
apocalyptech's Python editor, and B2Profile — and **re-implemented cleanly** in
Rust; no third-party code is copied. Borderlands 2 is a trademark of Gearbox/2K;
this project is unaffiliated.

The built-in **code library** contains community-shared `BL2(…)` codes (from a
public Steam guide); each was **decoded by our own engine** to derive its
category/level (the categorisation is ours, not scraped). Codes are shareable
data, not game assets. Currently weapons only — the browser supports every
category, so a broader code set drops straight in.
