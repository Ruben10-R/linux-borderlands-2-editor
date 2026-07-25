use std::path::PathBuf;

use bl2_save::{Location, ProfileFile, SaveError, SaveFile};
use eframe::egui;

use crate::theme;

const MAX: i64 = i32::MAX as i64; // currency/level/xp are int32 in the save

/// The editable viewer application state.
#[derive(Default)]
pub struct App {
    doc: Option<Doc>,
    /// A loaded account profile.bin (mutually exclusive with `doc`).
    profile: Option<ProfileDoc>,
    /// (is_error, message) shown under the fields.
    status: Option<(bool, String)>,
    theme: theme::Theme,
    /// Classic (original flat) vs Modern (rounded, elevated) styling.
    look: theme::Look,
    show_help: bool,
    tab: Tab,
    /// Web-only: file bytes picked by the browser Open dialog (async), consumed
    /// next frame. `Rc<RefCell<..>>` because wasm is single-threaded.
    #[cfg(target_arch = "wasm32")]
    #[allow(clippy::type_complexity)]
    pending_open: std::rc::Rc<std::cell::RefCell<Option<(String, Vec<u8>)>>>,
}

/// A loaded `profile.bin` plus its editable scratch values.
struct ProfileDoc {
    profile: ProfileFile,
    name: String,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    path: Option<PathBuf>,
    /// Editable SHiFT Golden Keys (0–255).
    golden_keys: i64,
    badass_rank: i64,
    badass_tokens: i64,
    /// Pending customization change to apply on save: Some(true)=unlock all,
    /// Some(false)=lock all, None=leave as-is.
    pending_customizations: Option<bool>,
}

/// Which editor tab is shown.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Character,
    General,
    Currency,
    Items,
    FastTravel,
    Vehicle,
    Raw,
    About,
}

impl Tab {
    const ALL: [Tab; 8] = [
        Tab::Character,
        Tab::General,
        Tab::Currency,
        Tab::Items,
        Tab::FastTravel,
        Tab::Vehicle,
        Tab::Raw,
        Tab::About,
    ];
    fn label(self) -> &'static str {
        match self {
            Tab::Character => "Character",
            Tab::General => "General",
            Tab::Currency => "Currency",
            Tab::Items => "Items",
            Tab::FastTravel => "Fast Travel",
            Tab::Vehicle => "Vehicle",
            Tab::Raw => "Raw",
            Tab::About => "About",
        }
    }
}

/// One loaded save: the parsed file plus editable scratch values.
struct Doc {
    save: SaveFile,
    name: String,
    /// Present on native (where we can write back); always `None` on web,
    /// where it is unused (the web build downloads instead of writing).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    path: Option<PathBuf>,
    char_name: String,
    class_def: String,
    /// Equipped head/skin paths (field 35 "wearing" indices 0 and 4).
    head: String,
    skin: String,
    /// Equipped vehicle skins: (slot1, slot2) per family, in VEHICLE_FAMILIES order.
    vehicle: Vec<(String, String)>,
    skill_points: i64,
    specialist_skill_points: i64,
    playthroughs_completed: i64,
    active_playthrough: i64,
    op_level: i64,
    backpack_size: i64,
    bank_size: i64,
    money: i64,
    eridium: i64,
    seraph: i64,
    torgue: i64,
    level: i64,
    xp: i64,
    items: Vec<ItemView>,
    /// Target for the "set all levelable items" convenience button.
    item_level: i64,
    /// Item id whose parts editor is open (None = closed).
    editing_parts: Option<usize>,
    /// Which part slot's picker is expanded (None = none).
    editing_part_slot: Option<usize>,
    part_filter: String,
    part_catalog: Vec<bl2_save::PartOption>,
    /// (is_weapon, set) the cached catalog was built for.
    part_catalog_key: Option<(bool, u32)>,
    /// Scratch input for the "add item from code" (BL2(...)) field.
    import_code: String,
    /// Code-library browser state.
    show_library: bool,
    lib_filter: String,
    lib_category: usize, // 0 = All, else index into library_categories()
    lib_to_bank: bool,
    /// Items sub-tab: false = Backpack, true = Bank.
    show_bank: bool,
    /// Search filter for the Raw inspector.
    raw_filter: String,
    /// Scratch set of unlocked fast-travel station resource_names (field 16).
    unlocked: std::collections::HashSet<String>,
}

/// One inventory row: `level` is an editable scratch value applied on save.
struct ItemView {
    id: usize,
    is_bank: bool,
    is_weapon: bool,
    /// False for grade-≤1 "no-level" items — locked (leveling can break them).
    levelable: bool,
    level: i64,
    name: String,
    /// Balance + parts breakdown, shown on hover.
    details: String,
    /// Resolved header fields, for the item detail form.
    type_name: String,
    balance: String,
    manufacturer: String,
    is_weapon_set: (bool, u32),
    /// Present part slots (editable).
    parts: Vec<PartSlotView>,
}

/// One present part slot of an item, for the parts editor.
struct PartSlotView {
    slot: usize,
    lib: u32,
    asset: u32,
    name: String,
}

fn build_item_views(s: &SaveFile) -> Vec<ItemView> {
    s.items()
        .unwrap_or_default()
        .into_iter()
        .filter(|it| !it.serial.is_placeholder())
        .map(|it| {
            let ser = &it.serial;
            let balance = ser
                .balance_name()
                .unwrap_or_else(|| format!("bal {}:{}", ser.balance.lib, ser.balance.asset));
            let part_names = ser.part_names();
            let mut details = format!("Balance: {balance}\nParts:");
            for (i, part) in part_names.iter().enumerate() {
                if let Some(n) = part {
                    details.push_str(&format!("\n  {i:>2}: {n}"));
                }
            }
            let parts = ser
                .parts
                .iter()
                .enumerate()
                .filter_map(|(slot, p)| {
                    p.map(|r| PartSlotView {
                        slot,
                        lib: r.lib,
                        asset: r.asset,
                        name: part_names[slot]
                            .clone()
                            .unwrap_or_else(|| format!("{}:{}", r.lib, r.asset)),
                    })
                })
                .collect();
            let manufacturer = ser.manufacturer_name().unwrap_or_default();
            let type_name = ser
                .type_name()
                .unwrap_or_else(|| format!("type {}:{}", ser.item_type.lib, ser.item_type.asset));
            ItemView {
                id: it.id,
                is_bank: it.location == Location::Bank,
                is_weapon: ser.is_weapon,
                levelable: ser.is_levelable(),
                level: ser.stage.unwrap_or(0),
                name: format!("{manufacturer} {type_name}").trim().to_string(),
                details,
                type_name,
                balance,
                manufacturer,
                is_weapon_set: (ser.is_weapon, ser.set),
                parts,
            }
        })
        .collect()
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        // Restore the last-used theme + look (native: RON file; web: localStorage).
        if let Some(storage) = cc.storage {
            if let Some(i) = eframe::get_value::<u8>(storage, "theme") {
                app.theme = theme::Theme::from_index(i);
            }
            if let Some(i) = eframe::get_value::<u8>(storage, "look") {
                app.look = theme::Look::from_index(i);
            }
        }
        theme::apply(&cc.egui_ctx, app.theme, app.look);
        app
    }

    fn load(&mut self, name: String, path: Option<PathBuf>, bytes: &[u8]) {
        // A character save is the common case (WSG container); a profile.bin has
        // no WSG, so it fails SaveFile and we try ProfileFile next.
        if SaveFile::from_bytes(bytes).is_err() {
            match ProfileFile::from_bytes(bytes) {
                Ok(p) => {
                    self.doc = None;
                    self.profile = Some(ProfileDoc {
                        golden_keys: p.golden_keys().unwrap_or(0) as i64,
                        badass_rank: p.badass_rank().unwrap_or(0) as i64,
                        badass_tokens: p.badass_tokens().unwrap_or(0) as i64,
                        pending_customizations: None,
                        name,
                        path,
                        profile: p,
                    });
                    self.status = Some((false, "Loaded profile.".to_string()));
                    return;
                }
                Err(_) => {
                    // fall through: report the save error below (the likely intent)
                }
            }
        }
        match SaveFile::from_bytes(bytes) {
            Ok(s) => {
                self.profile = None;
                self.doc = Some(Doc {
                    char_name: s.name().unwrap_or_default(),
                    class_def: s.class_def().unwrap_or_default(),
                    head: s.wearing().first().cloned().unwrap_or_else(|| "0".into()),
                    skin: s.wearing().get(4).cloned().unwrap_or_else(|| "0".into()),
                    vehicle: bl2_save::VEHICLE_FAMILIES
                        .iter()
                        .map(|f| {
                            let sk = s.vehicle_family_skins(f.path);
                            (
                                sk.first().cloned().unwrap_or_default(),
                                sk.get(1).cloned().unwrap_or_default(),
                            )
                        })
                        .collect(),
                    skill_points: s.skill_points().unwrap_or(0),
                    specialist_skill_points: s.specialist_skill_points().unwrap_or(0),
                    playthroughs_completed: s.playthroughs_completed().unwrap_or(0),
                    active_playthrough: s.active_playthrough(),
                    op_level: s.op_level().unwrap_or(0),
                    backpack_size: s.backpack_size().unwrap_or(12),
                    bank_size: s.bank_size(),
                    level: s.level().unwrap_or(0),
                    xp: s.xp().unwrap_or(0),
                    money: s.money(),
                    eridium: s.eridium(),
                    seraph: s.seraph(),
                    torgue: s.torgue(),
                    items: build_item_views(&s),
                    item_level: s.level().unwrap_or(50).clamp(1, 127),
                    editing_parts: None,
                    editing_part_slot: None,
                    part_filter: String::new(),
                    part_catalog: Vec::new(),
                    part_catalog_key: None,
                    import_code: String::new(),
                    show_library: false,
                    lib_filter: String::new(),
                    lib_category: 0,
                    lib_to_bank: false,
                    show_bank: false,
                    raw_filter: String::new(),
                    unlocked: s.visited_stations().into_iter().collect(),
                    name,
                    path,
                    save: s,
                });
                self.status = Some((false, "Loaded.".to_string()));
            }
            Err(e) => {
                self.doc = None;
                self.status = Some((true, e.to_string()));
            }
        }
    }

    fn handle_dropped(&mut self, ctx: &egui::Context) {
        let Some(f) = ctx.input(|i| i.raw.dropped_files.first().cloned()) else {
            return;
        };
        let name = if !f.name.is_empty() {
            f.name.clone()
        } else if let Some(p) = &f.path {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            "(dropped file)".to_string()
        };
        let path = f.path.clone();

        if let Some(bytes) = &f.bytes {
            self.load(name, path, bytes);
        } else if let Some(p) = &f.path {
            match std::fs::read(p) {
                Ok(b) => self.load(name, path, &b),
                Err(e) => {
                    self.status = Some((true, format!("could not read {}: {e}", p.display())))
                }
            }
        }
    }

    /// Push all scratch edits into the loaded save/profile (the shared pre-write
    /// step for Save and Save As). Does not touch the disk.
    fn apply_all(&mut self) -> Result<(), String> {
        if let Some(pdoc) = self.profile.as_mut() {
            if pdoc.golden_keys != pdoc.profile.golden_keys().unwrap_or(0) as i64 {
                pdoc.profile
                    .set_golden_keys(pdoc.golden_keys.clamp(0, 255) as u8)
                    .map_err(|e| e.to_string())?;
            }
            if pdoc.badass_rank != pdoc.profile.badass_rank().unwrap_or(0) as i64 {
                pdoc.profile
                    .set_badass_rank(pdoc.badass_rank.clamp(0, MAX) as i32)
                    .map_err(|e| e.to_string())?;
            }
            if let Some(unlock) = pdoc.pending_customizations.take() {
                pdoc.profile
                    .set_all_customizations(unlock)
                    .map_err(|e| e.to_string())?;
            }
            // Refresh scratch: rank snaps to the LUT and tokens/available change.
            pdoc.golden_keys = pdoc.profile.golden_keys().unwrap_or(0) as i64;
            pdoc.badass_rank = pdoc.profile.badass_rank().unwrap_or(0) as i64;
            pdoc.badass_tokens = pdoc.profile.badass_tokens().unwrap_or(0) as i64;
            return Ok(());
        }
        if let Some(doc) = self.doc.as_mut() {
            apply_edits(doc).map_err(|e| e.to_string())?;
            let edits: Vec<(usize, i64)> = doc
                .items
                .iter()
                .filter(|v| v.levelable)
                .map(|v| (v.id, v.level))
                .collect();
            for (id, lvl) in edits {
                let _ = doc.save.set_item_level(id, lvl);
            }
            return Ok(());
        }
        Err("nothing loaded".into())
    }

    /// The current file's suggested name + freshly-encoded bytes (after apply).
    /// Used by the native "Save As…" dialog.
    #[cfg(not(target_arch = "wasm32"))]
    fn current_name(&self) -> String {
        self.profile
            .as_ref()
            .map(|p| p.name.clone())
            .or_else(|| self.doc.as_ref().map(|d| d.name.clone()))
            .unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn encoded_bytes(&self) -> Result<Vec<u8>, String> {
        if let Some(pdoc) = self.profile.as_ref() {
            pdoc.profile.to_bytes().map_err(|e| e.to_string())
        } else if let Some(doc) = self.doc.as_ref() {
            doc.save.to_bytes().map_err(|e| e.to_string())
        } else {
            Err("nothing loaded".into())
        }
    }

    /// Apply edits and write them out (disk on native, download on web).
    fn save_current(&mut self) {
        if let Err(e) = self.apply_all() {
            self.status = Some((true, format!("edit failed: {e}")));
            return;
        }
        if let Some(pdoc) = self.profile.as_mut() {
            self.status = Some(persist_profile(pdoc));
        } else if let Some(doc) = self.doc.as_mut() {
            self.status = Some(persist(doc));
        }
    }
}

// Native file dialogs (a real Open / Save As, backed by the OS picker).
#[cfg(not(target_arch = "wasm32"))]
impl App {
    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("BL2 save / profile", &["sav", "bin"])
            .pick_file()
        {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.load(name, Some(path), &bytes);
                }
                Err(e) => self.status = Some((true, format!("could not read: {e}"))),
            }
        }
    }

    fn save_as_dialog(&mut self) {
        if let Err(e) = self.apply_all() {
            self.status = Some((true, format!("edit failed: {e}")));
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(self.current_name())
            .save_file()
        else {
            return;
        };
        match self.encoded_bytes() {
            Ok(bytes) => match std::fs::write(&path, &bytes) {
                Ok(()) => self.status = Some((false, format!("Saved to {}", path.display()))),
                Err(e) => self.status = Some((true, format!("write failed: {e}"))),
            },
            Err(e) => self.status = Some((true, format!("encode failed: {e}"))),
        }
    }
}

// Web file open (browser picker → async read into a shared slot).
#[cfg(target_arch = "wasm32")]
impl App {
    fn open_dialog(&mut self) {
        let slot = self.pending_open.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(fh) = rfd::AsyncFileDialog::new()
                .add_filter("BL2 save / profile", &["sav", "bin"])
                .pick_file()
                .await
            {
                let name = fh.file_name();
                let bytes = fh.read().await;
                *slot.borrow_mut() = Some((name, bytes));
            }
        });
    }
}

/// Push the scratch values into the save via the guarded setters.
fn apply_edits(doc: &mut Doc) -> Result<(), SaveError> {
    doc.save.set_money(doc.money.clamp(0, MAX))?;
    doc.save.set_eridium(doc.eridium.clamp(0, MAX))?;
    doc.save.set_seraph(doc.seraph.clamp(0, MAX))?;
    doc.save.set_torgue(doc.torgue.clamp(0, MAX))?;
    doc.save.set_level(doc.level.clamp(0, MAX))?;
    doc.save.set_xp(doc.xp.clamp(0, MAX))?;
    doc.save.set_skill_points(doc.skill_points.clamp(0, MAX))?;
    doc.save
        .set_specialist_skill_points(doc.specialist_skill_points.clamp(0, MAX))?;
    doc.save
        .set_playthroughs_completed(doc.playthroughs_completed.clamp(0, 3))?;
    doc.save
        .set_active_playthrough(doc.active_playthrough.clamp(0, 2))?;
    // OP level — only rewrite the virtual item if it actually changed.
    if doc.op_level != doc.save.op_level().unwrap_or(0) {
        doc.save.set_op_level(doc.op_level.clamp(0, 80))?;
    }
    // Backpack / bank capacity — only rewrite if changed.
    if doc.backpack_size != doc.save.backpack_size().unwrap_or(12) {
        doc.save
            .set_backpack_size(doc.backpack_size.clamp(12, 39))?;
    }
    if doc.bank_size != doc.save.bank_size() {
        doc.save.set_bank_size(doc.bank_size.clamp(6, 200))?;
    }
    // Class + name are best-effort (a stray save might lack those fields).
    if !doc.class_def.is_empty() {
        let _ = doc.save.set_class(&doc.class_def);
    }
    if !doc.char_name.is_empty() {
        let _ = doc.save.set_name(&doc.char_name);
    }
    // Head/skin (field 35) — only rewrite if changed from what's on the save.
    let wearing = doc.save.wearing();
    let cur_head = wearing.first().cloned().unwrap_or_default();
    let cur_skin = wearing.get(4).cloned().unwrap_or_default();
    if (doc.head != cur_head || doc.skin != cur_skin)
        && !doc.head.is_empty()
        && !doc.skin.is_empty()
    {
        let _ = doc.save.set_wearing(&doc.head, &doc.skin);
    }
    // Vehicle skins (field 57) — rewrite a family only if its selection changed.
    for (i, fam) in bl2_save::VEHICLE_FAMILIES.iter().enumerate() {
        let (s1, s2) = &doc.vehicle[i];
        let want: Vec<String> = [s1, s2]
            .iter()
            .filter(|s| !s.is_empty() && **s != "None")
            .map(|s| s.to_string())
            .collect();
        if doc.save.vehicle_family_skins(fam.path) != want {
            let _ = doc
                .save
                .set_vehicle_skins(fam.path, &[s1.clone(), s2.clone()]);
        }
    }
    // Fast-travel stations — only rewrite field 16 if the set actually changed.
    let current: std::collections::HashSet<String> =
        doc.save.visited_stations().into_iter().collect();
    if current != doc.unlocked {
        let catalog = bl2_save::stations_catalog();
        // Deterministic: catalog order for known stations, then preserve any
        // unknown ones already on the save.
        let mut list: Vec<String> = catalog
            .iter()
            .map(|s| s.rn.clone())
            .filter(|rn| doc.unlocked.contains(rn))
            .collect();
        for extra in &doc.unlocked {
            if !catalog.iter().any(|s| &s.rn == extra) {
                list.push(extra.clone());
            }
        }
        doc.save.set_visited_stations(&list)?;
    }
    Ok(())
}

/// Native: write back to the loaded file with an automatic `.bak` backup.
#[cfg(not(target_arch = "wasm32"))]
fn persist(doc: &mut Doc) -> (bool, String) {
    match &doc.path {
        Some(path) => match doc.save.save(path, true) {
            Ok(()) => (
                false,
                format!("Saved {} (backup written alongside).", path.display()),
            ),
            Err(e) => (true, format!("save failed: {e}")),
        },
        None => (true, "no file path to write to".to_string()),
    }
}

/// Web: browsers can't write to disk, so download the edited bytes.
#[cfg(target_arch = "wasm32")]
fn persist(doc: &mut Doc) -> (bool, String) {
    match doc.save.to_bytes() {
        Ok(bytes) => {
            crate::io::download(&doc.name, &bytes);
            (false, format!("Downloaded edited {}.", doc.name))
        }
        Err(e) => (true, format!("encode failed: {e}")),
    }
}

/// Native: write the edited profile back to disk with a `.bak` backup.
#[cfg(not(target_arch = "wasm32"))]
fn persist_profile(pdoc: &mut ProfileDoc) -> (bool, String) {
    match &pdoc.path {
        Some(path) => match pdoc.profile.save(path, true) {
            Ok(()) => (
                false,
                format!("Saved {} (backup written alongside).", path.display()),
            ),
            Err(e) => (true, format!("save failed: {e}")),
        },
        None => (true, "no file path to write to".to_string()),
    }
}

/// Web: download the edited profile.
#[cfg(target_arch = "wasm32")]
fn persist_profile(pdoc: &mut ProfileDoc) -> (bool, String) {
    match pdoc.profile.to_bytes() {
        Ok(bytes) => {
            crate::io::download(&pdoc.name, &bytes);
            (false, format!("Downloaded edited {}.", pdoc.name))
        }
        Err(e) => (true, format!("encode failed: {e}")),
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Re-apply every frame so our theme wins over eframe's system-theme
        // following (otherwise the web build reverts to the browser's light mode).
        theme::apply(&ctx, self.theme, self.look);
        self.handle_dropped(&ctx);

        // Floating look-switcher (bottom-right), above everything.
        if theme::fab(&ctx, self.look, self.theme.accent()) {
            self.look = self.look.other();
            theme::apply(&ctx, self.theme, self.look);
        }

        // Web: consume a file picked asynchronously by the browser Open dialog.
        #[cfg(target_arch = "wasm32")]
        {
            let picked = self.pending_open.borrow_mut().take();
            if let Some((name, bytes)) = picked {
                self.load(name, None, &bytes);
            }
        }

        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            egui::Panel::bottom("drop_hint").show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| ui.label("⤵ Drop to load"));
            });
        }

        let look = self.look;
        // Modern floats the content as an elevated card over a darker backdrop.
        let panel_frame = if look == theme::Look::Modern {
            egui::Frame::central_panel(&ctx.global_style())
                .fill(self.theme.backdrop())
                .inner_margin(egui::Margin::same(14))
        } else {
            egui::Frame::central_panel(&ctx.global_style())
        };
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show_inside(ui, |ui| {
                theme::card(ui, look, |ui| {
                    let accent = self.theme.accent();
                    let text = self.theme.text();

                    // Header: original emblem + wordmark.
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        theme::emblem(ui, 44.0, accent);
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("BL2 SAVE EDITOR")
                                    .color(accent)
                                    .size(24.0)
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new("Borderlands 2 save editor")
                                    .color(text)
                                    .italics(),
                            );
                        });
                    });

                    // Theme switcher.
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Theme:");
                        for t in theme::Theme::ALL {
                            if ui.selectable_label(self.theme == t, t.label()).clicked() {
                                self.theme = t;
                                theme::apply(&ctx, t, self.look);
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.separator();

                    // Open (OS file dialog on native, browser picker on web).
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("📂  Open file…").clicked() {
                            self.open_dialog();
                        }
                        ui.weak("· or drag a .sav / profile.bin onto the window");
                    });
                    ui.add_space(4.0);
                    ui.separator();

                    if self.doc.is_none() && self.profile.is_none() {
                        ui.add_space(8.0);
                        ui.label("Open or drag a character .sav or your account profile.bin.");
                        ui.add_space(2.0);
                        ui.weak("profile.bin holds Golden Keys and Badass Rank (account-wide).");
                        if let Some((true, msg)) = &self.status {
                            ui.add_space(8.0);
                            ui.colored_label(theme::DANGER, msg);
                        }
                        return;
                    }

                    // A profile.bin is loaded — show the Profile view (no save tabs).
                    if self.profile.is_some() {
                        ui.add_space(6.0);
                        let label = if cfg!(target_arch = "wasm32") {
                            "⬇  Download edited profile"
                        } else {
                            "💾  Save profile (with backup)"
                        };
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new(label).strong()).clicked() {
                                self.save_current();
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.button("Save As…").clicked() {
                                self.save_as_dialog();
                            }
                            ui.weak("· account profile");
                        });
                        if let Some((is_err, msg)) = &self.status {
                            let col = if *is_err { theme::DANGER } else { accent };
                            ui.colored_label(col, msg);
                        }
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);
                        profile_view(self.profile.as_mut().unwrap(), ui, accent);
                        return;
                    }

                    // Global actions (apply to the whole save, whatever tab is open).
                    ui.add_space(6.0);
                    let label = if cfg!(target_arch = "wasm32") {
                        "⬇  Download edited save"
                    } else {
                        "💾  Save (with backup)"
                    };
                    ui.horizontal(|ui| {
                        if ui.button(egui::RichText::new(label).strong()).clicked() {
                            self.save_current();
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        if ui.button("Save As…").clicked() {
                            self.save_as_dialog();
                        }
                        if cfg!(target_arch = "wasm32")
                            && ui.button("ⓘ  How to install this save").clicked()
                        {
                            self.show_help = true;
                        }
                    });
                    if let Some((is_err, msg)) = &self.status {
                        let col = if *is_err { theme::DANGER } else { accent };
                        ui.colored_label(col, msg);
                    }

                    // Tab bar (each tab gets an original glyph in its state colour).
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for t in Tab::ALL {
                            let col = if self.tab == t { accent } else { text };
                            match t {
                                Tab::Character => theme::head(ui, 16.0, col),
                                Tab::General => theme::flag(ui, 16.0, col),
                                Tab::Currency => theme::coin(ui, 16.0),
                                Tab::Items => theme::crate_icon(ui, 16.0, col),
                                Tab::FastTravel => theme::signpost(ui, 16.0, col),
                                Tab::Vehicle => theme::wheel(ui, 16.0, col),
                                Tab::Raw => theme::page(ui, 16.0, col),
                                Tab::About => theme::info(ui, 16.0, col),
                            }
                            let label = egui::RichText::new(t.label()).color(col).strong();
                            if ui.selectable_label(self.tab == t, label).clicked() {
                                self.tab = t;
                            }
                            ui.add_space(10.0);
                        }
                    });
                    ui.separator();
                    ui.add_space(6.0);

                    // Tab content.
                    let tab = self.tab;
                    let doc = self.doc.as_mut().unwrap();
                    let tab_status = match tab {
                        Tab::Character => {
                            character_tab(doc, ui, accent);
                            None
                        }
                        Tab::General => {
                            general_tab(doc, ui, accent);
                            None
                        }
                        Tab::Currency => {
                            currency_tab(doc, ui, accent);
                            None
                        }
                        Tab::Items => items_tab(doc, ui, accent, text),
                        Tab::FastTravel => {
                            fast_travel_tab(doc, ui, accent);
                            None
                        }
                        Tab::Vehicle => {
                            vehicle_tab(doc, ui, accent);
                            None
                        }
                        Tab::Raw => {
                            raw_tab(doc, ui, accent);
                            None
                        }
                        Tab::About => {
                            about_tab(ui, accent);
                            None
                        }
                    };
                    if let Some(s) = tab_status {
                        self.status = Some(s);
                    }
                });
            });

        // Parts picker modal — floats with full space; search keeps focus.
        self.parts_picker_modal(&ctx);

        // Code library browser modal.
        self.library_modal(&ctx);

        // "How to install" modal — rendered above everything else.
        if self.show_help {
            let accent = self.theme.accent();
            let mut close = false;
            let resp = egui::Modal::new(egui::Id::new("install_help")).show(&ctx, |ui| {
                install_help_ui(ui, accent, &mut close);
            });
            if close || resp.should_close() {
                self.show_help = false;
            }
        }
    }

    /// Persist the chosen theme + look (native: RON file; web: localStorage).
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "theme", &self.theme.index());
        eframe::set_value(storage, "look", &self.look.index());
    }
}

impl App {
    /// The floating part picker for the open (item, slot) — a search field + a
    /// tall scrollable list. On pick, applies the swap and refreshes the list.
    fn parts_picker_modal(&mut self, ctx: &egui::Context) {
        let Some(doc) = self.doc.as_mut() else { return };
        let (Some(id), Some(slot)) = (doc.editing_parts, doc.editing_part_slot) else {
            return;
        };
        let Some(idx) = doc.items.iter().position(|v| v.id == id) else {
            doc.editing_part_slot = None;
            return;
        };
        let key = doc.items[idx].is_weapon_set;
        let slot_name = bl2_save::slot_label(key.0, slot);
        if doc.part_catalog_key != Some(key) {
            doc.part_catalog = bl2_save::parts_catalog(key.0, key.1);
            doc.part_catalog_key = Some(key);
        }
        let cur = doc.items[idx]
            .parts
            .iter()
            .find(|p| p.slot == slot)
            .map(|p| (p.lib, p.asset));
        let accent = self.theme.accent();

        let mut pending: Option<(u32, u32)> = None;
        let mut close = false;
        let resp =
            egui::Modal::new(egui::Id::new("part_picker")).show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(
                    egui::RichText::new(format!("{slot_name} — choose a part"))
                        .color(accent)
                        .strong(),
                );
                let te = ui.add(
                    egui::TextEdit::singleline(&mut doc.part_filter)
                        .hint_text("type to search parts…")
                        .desired_width(f32::INFINITY),
                );
                te.request_focus();
                let needle = doc.part_filter.to_lowercase();
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .max_height(380.0)
                    .show(ui, |ui| {
                        for opt in doc.part_catalog.iter().filter(|o| {
                            needle.is_empty() || o.name.to_lowercase().contains(&needle)
                        }) {
                            let selected = Some((opt.lib, opt.asset)) == cur;
                            // full-width row so the highlight/layout doesn't jump per item
                            if ui
                                .add_sized(
                                    [ui.available_width(), 20.0],
                                    egui::Button::selectable(selected, &opt.name),
                                )
                                .clicked()
                            {
                                pending = Some((opt.lib, opt.asset));
                            }
                        }
                    });
                ui.separator();
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });

        if let Some((lib, asset)) = pending {
            let _ = doc.save.set_item_part(id, slot, lib, asset);
            doc.items = build_item_views(&doc.save);
            doc.editing_part_slot = None;
        } else if close || resp.should_close() {
            doc.editing_part_slot = None;
        }
    }

    /// The code-library browser: filter by category, search, add codes with one
    /// click (virtualized list handles the thousands of entries).
    fn library_modal(&mut self, ctx: &egui::Context) {
        let Some(doc) = self.doc.as_mut() else { return };
        if !doc.show_library {
            return;
        }
        let accent = self.theme.accent();
        let cats = bl2_save::library_categories();
        let lib = bl2_save::code_library();
        let needle = doc.lib_filter.to_lowercase();
        let want_cat = if doc.lib_category == 0 {
            None
        } else {
            cats.get(doc.lib_category - 1).copied()
        };

        // Indices of matching entries (recomputed each frame; a few thousand — cheap).
        let matches: Vec<usize> = lib
            .iter()
            .enumerate()
            .filter(|(_, e)| want_cat.is_none_or(|c| e.category == c))
            .filter(|(_, e)| {
                needle.is_empty()
                    || e.name.to_lowercase().contains(&needle)
                    || e.family.to_lowercase().contains(&needle)
            })
            .map(|(i, _)| i)
            .collect();

        let mut close = false;
        let mut add: Option<usize> = None;
        let resp = egui::Modal::new(egui::Id::new("code_library")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.horizontal(|ui| {
                theme::crate_icon(ui, 18.0, accent);
                ui.label(egui::RichText::new("Item code library").color(accent).size(18.0).strong());
                ui.weak(format!("{} codes", lib.len()));
            });
            // Category filter.
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(doc.lib_category == 0, "All").clicked() {
                    doc.lib_category = 0;
                }
                for (i, c) in cats.iter().enumerate() {
                    if ui.selectable_label(doc.lib_category == i + 1, *c).clicked() {
                        doc.lib_category = i + 1;
                    }
                }
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.add(
                    egui::TextEdit::singleline(&mut doc.lib_filter)
                        .hint_text("name or family…")
                        .desired_width(300.0),
                );
                ui.checkbox(&mut doc.lib_to_bank, "add to bank")
                    .on_hover_text("When ticked, \u{201c}Add\u{201d} puts the item in your Bank instead of your Backpack.");
                ui.weak(format!("{} shown", matches.len()));
            });
            ui.separator();

            let row_h = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
            egui::ScrollArea::vertical()
                .auto_shrink(false) // keep full width so rows don't jump while scrolling
                .max_height(420.0)
                .show_rows(ui, row_h, matches.len(), |ui, range| {
                    for &mi in &matches[range] {
                        let e = &lib[mi];
                        ui.horizontal(|ui| {
                            if ui.add_sized([44.0, row_h - 3.0], egui::Button::new("Add")).clicked() {
                                add = Some(mi);
                            }
                            ui.add_sized(
                                [300.0, row_h - 3.0],
                                egui::Label::new(egui::RichText::new(&e.name).monospace())
                                    .truncate(),
                            )
                            .on_hover_text(format!("{}\nLv {}\n{}", e.family, e.level, e.code));
                            ui.add(
                                egui::Label::new(egui::RichText::new(&e.family).weak()).truncate(),
                            );
                        });
                    }
                });
            ui.separator();
            if ui.button("Close").clicked() {
                close = true;
            }
        });

        let mut new_status = None;
        if let Some(mi) = add {
            let code = lib[mi].code.clone();
            match doc.save.add_item_from_code(&code, doc.lib_to_bank) {
                Ok(()) => {
                    rebuild_items_preserving_levels(doc);
                    let where_ = if doc.lib_to_bank { "bank" } else { "backpack" };
                    new_status = Some((false, format!("Added {} to {where_}.", lib[mi].name)));
                }
                Err(e) => new_status = Some((true, format!("Add failed: {e}"))),
            }
        } else if close || resp.should_close() {
            doc.show_library = false;
        }
        if let Some(s) = new_status {
            self.status = Some(s);
        }
    }
}

mod tabs;
use tabs::*;

/// Contents of the install-instructions modal.
fn install_help_ui(ui: &mut egui::Ui, accent: egui::Color32, close: &mut bool) {
    ui.set_max_width(480.0);
    ui.label(
        egui::RichText::new("Installing your edited save")
            .color(accent)
            .size(20.0)
            .strong(),
    );
    ui.add_space(6.0);
    ui.label("Your browser downloaded a .sav file. To use it in-game:");
    ui.add_space(6.0);

    let step = |ui: &mut egui::Ui, n: &str, s: &str| {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(n).color(accent).strong());
            ui.label(s);
        });
    };
    step(ui, "1.", "Fully close Borderlands 2.");
    step(ui, "2.", "Open your save folder:");
    ui.indent("paths", |ui| {
        ui.monospace("Linux (Steam):  ~/.local/share/aspyr-media/borderlands 2/");
        ui.monospace("                willowgame/savedata/<SteamID>/");
        ui.monospace(
            "Windows:  …\\Documents\\My Games\\Borderlands 2\\WillowGame\\SaveData\\<SteamID>\\",
        );
    });
    step(
        ui,
        "3.",
        "Back up the existing save there (copy it somewhere safe).",
    );
    step(
        ui,
        "4.",
        "Replace it with your download, keeping the same name (e.g. save0001.sav).",
    );
    step(ui, "5.", "Steam Cloud can overwrite it at launch. To be safe: quit Steam fully, replace the file, then start Steam again before launching — or turn off Steam Cloud for BL2 / go Offline.");
    step(ui, "6.", "Launch BL2 and check your stats.");

    ui.add_space(10.0);
    if ui.button(egui::RichText::new("Got it").strong()).clicked() {
        *close = true;
    }
}

/// A clamped integer editor (typed entry + drag), kept in `0..=i32::MAX`.
fn edit_number(ui: &mut egui::Ui, value: &mut i64, speed: f64) {
    ui.add(egui::DragValue::new(value).speed(speed));
    *value = (*value).clamp(0, MAX);
}

/// A bold accent-coloured key label (left column of the grid).
fn key(ui: &mut egui::Ui, text: &str, accent: egui::Color32) {
    ui.label(egui::RichText::new(text).color(accent).strong());
}

/// A read-only "key : value" grid row with a monospace value.
fn field(ui: &mut egui::Ui, k: &str, value: &str, accent: egui::Color32) {
    key(ui, k, accent);
    ui.monospace(value);
    ui.end_row();
}
