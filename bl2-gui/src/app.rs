use std::path::PathBuf;

use bl2_save::{Location, SaveError, SaveFile};
use eframe::egui;

use crate::theme;

const MAX: i64 = i32::MAX as i64; // currency/level/xp are int32 in the save

/// The editable viewer application state.
#[derive(Default)]
pub struct App {
    doc: Option<Doc>,
    /// (is_error, message) shown under the fields.
    status: Option<(bool, String)>,
    theme: theme::Theme,
    show_help: bool,
    tab: Tab,
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
    Raw,
    About,
}

impl Tab {
    const ALL: [Tab; 7] = [
        Tab::Character,
        Tab::General,
        Tab::Currency,
        Tab::Items,
        Tab::FastTravel,
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
    skill_points: i64,
    specialist_skill_points: i64,
    playthroughs_completed: i64,
    active_playthrough: i64,
    op_level: i64,
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
                        name: part_names[slot].clone().unwrap_or_else(|| format!("{}:{}", r.lib, r.asset)),
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
        let app = Self::default();
        theme::apply(&cc.egui_ctx, app.theme);
        app
    }

    fn load(&mut self, name: String, path: Option<PathBuf>, bytes: &[u8]) {
        match SaveFile::from_bytes(bytes) {
            Ok(s) => {
                self.doc = Some(Doc {
                    char_name: s.name().unwrap_or_default(),
                    class_def: s.class_def().unwrap_or_default(),
                    head: s.wearing().first().cloned().unwrap_or_else(|| "0".into()),
                    skin: s.wearing().get(4).cloned().unwrap_or_else(|| "0".into()),
                    skill_points: s.skill_points().unwrap_or(0),
                    specialist_skill_points: s.specialist_skill_points().unwrap_or(0),
                    playthroughs_completed: s.playthroughs_completed().unwrap_or(0),
                    active_playthrough: s.active_playthrough(),
                    op_level: s.op_level().unwrap_or(0),
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
            p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            "(dropped file)".to_string()
        };
        let path = f.path.clone();

        if let Some(bytes) = &f.bytes {
            self.load(name, path, bytes);
        } else if let Some(p) = &f.path {
            match std::fs::read(p) {
                Ok(b) => self.load(name, path, &b),
                Err(e) => self.status = Some((true, format!("could not read {}: {e}", p.display()))),
            }
        }
    }

    /// Apply the edited scratch values and write them out (disk on native,
    /// download on web), updating `status`.
    fn save_current(&mut self) {
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        if let Err(e) = apply_edits(doc) {
            self.status = Some((true, format!("edit failed: {e}")));
            return;
        }
        // Apply per-item level edits (levelable rows only).
        let edits: Vec<(usize, i64)> = doc
            .items
            .iter()
            .filter(|v| v.levelable)
            .map(|v| (v.id, v.level))
            .collect();
        for (id, lvl) in edits {
            let _ = doc.save.set_item_level(id, lvl);
        }
        self.status = Some(persist(doc));
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
    doc.save.set_specialist_skill_points(doc.specialist_skill_points.clamp(0, MAX))?;
    doc.save.set_playthroughs_completed(doc.playthroughs_completed.clamp(0, 3))?;
    doc.save.set_active_playthrough(doc.active_playthrough.clamp(0, 2))?;
    // OP level — only rewrite the virtual item if it actually changed.
    if doc.op_level != doc.save.op_level().unwrap_or(0) {
        doc.save.set_op_level(doc.op_level.clamp(0, 80))?;
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
    if (doc.head != cur_head || doc.skin != cur_skin) && !doc.head.is_empty() && !doc.skin.is_empty()
    {
        let _ = doc.save.set_wearing(&doc.head, &doc.skin);
    }
    // Fast-travel stations — only rewrite field 16 if the set actually changed.
    let current: std::collections::HashSet<String> =
        doc.save.visited_stations().into_iter().collect();
    if current != doc.unlocked {
        let catalog = bl2_save::stations_catalog();
        // Deterministic: catalog order for known stations, then preserve any
        // unknown ones already on the save.
        let mut list: Vec<String> =
            catalog.iter().map(|s| s.rn.clone()).filter(|rn| doc.unlocked.contains(rn)).collect();
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
            Ok(()) => (false, format!("Saved {} (backup written alongside).", path.display())),
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Re-apply every frame so our theme wins over eframe's system-theme
        // following (otherwise the web build reverts to the browser's light mode).
        theme::apply(&ctx, self.theme);
        self.handle_dropped(&ctx);

        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            egui::Panel::bottom("drop_hint").show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| ui.label("⤵ Drop to load"));
            });
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let accent = self.theme.accent();
            let text = self.theme.text();

            // Header: original emblem + wordmark.
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                theme::emblem(ui, 44.0, accent);
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("BL2 SAVE EDITOR").color(accent).size(24.0).strong());
                    ui.label(egui::RichText::new("Borderlands 2 save editor").color(text).italics());
                });
            });

            // Theme switcher.
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Theme:");
                for t in theme::Theme::ALL {
                    if ui.selectable_label(self.theme == t, t.label()).clicked() {
                        self.theme = t;
                        theme::apply(&ctx, t);
                    }
                }
            });
            ui.add_space(4.0);
            ui.separator();

            if self.doc.is_none() {
                ui.add_space(8.0);
                ui.label("Drag a .sav file onto this window to load it.");
                ui.add_space(4.0);
                ui.weak("No save loaded yet.");
                if let Some((true, msg)) = &self.status {
                    ui.add_space(8.0);
                    ui.colored_label(theme::DANGER, msg);
                }
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
                if cfg!(target_arch = "wasm32")
                    && ui.button("ⓘ  How to install this save").clicked()
                {
                    self.show_help = true;
                }
                ui.weak("· drag another .sav to load it");
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

        // Parts picker modal — floats with full space; search keeps focus.
        self.parts_picker_modal(&ctx);

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
        let resp = egui::Modal::new(egui::Id::new("part_picker")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label(egui::RichText::new(format!("{slot_name} — choose a part")).color(accent).strong());
            let te = ui.add(
                egui::TextEdit::singleline(&mut doc.part_filter)
                    .hint_text("type to search parts…")
                    .desired_width(f32::INFINITY),
            );
            te.request_focus();
            let needle = doc.part_filter.to_lowercase();
            ui.separator();
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                for opt in doc
                    .part_catalog
                    .iter()
                    .filter(|o| needle.is_empty() || o.name.to_lowercase().contains(&needle))
                {
                    let selected = Some((opt.lib, opt.asset)) == cur;
                    if ui.selectable_label(selected, &opt.name).clicked() {
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
}

/// Character tab: file, name, class, level, XP, skill points.
fn character_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    egui::Grid::new("character").num_columns(2).spacing([24.0, 8.0]).striped(true).show(ui, |ui| {
        field(ui, "File", &doc.name, accent);

        key(ui, "Name", accent);
        ui.text_edit_singleline(&mut doc.char_name);
        ui.end_row();

        key(ui, "Class", accent);
        let current = bl2_save::CLASSES
            .iter()
            .find(|(_, path)| *path == doc.class_def)
            .map(|(disp, _)| *disp)
            .unwrap_or("(unknown)");
        egui::ComboBox::from_id_salt("class_combo").selected_text(current).show_ui(ui, |ui| {
            for (disp, path) in bl2_save::CLASSES {
                if ui.selectable_label(doc.class_def == path, disp).clicked() {
                    doc.class_def = path.to_string();
                }
            }
        });
        ui.end_row();

        key(ui, "Head", accent);
        customization_combo(ui, "head_combo", &doc.class_def, true, &mut doc.head);
        ui.end_row();
        key(ui, "Skin", accent);
        customization_combo(ui, "skin_combo", &doc.class_def, false, &mut doc.skin);
        ui.end_row();

        key(ui, "Level", accent);
        ui.horizontal(|ui| {
            edit_number(ui, &mut doc.level, 1.0);
            if ui.button("Sync").on_hover_text("Set XP to the minimum for this level").clicked() {
                doc.xp = bl2_save::xp_for_level(doc.level);
            }
        });
        ui.end_row();
        key(ui, "XP", accent);
        ui.horizontal(|ui| {
            edit_number(ui, &mut doc.xp, 1000.0);
            if ui.button("Sync").on_hover_text("Set level to match this XP").clicked() {
                doc.level = bl2_save::level_for_xp(doc.xp);
            }
        });
        ui.end_row();
        key(ui, "Skill Points", accent);
        edit_number(ui, &mut doc.skill_points, 1.0);
        ui.end_row();
        key(ui, "Specialist Points", accent);
        edit_number(ui, &mut doc.specialist_skill_points, 1.0);
        ui.end_row();
    });
    ui.add_space(4.0);
    ui.weak("Changing class does not reset skills — level/skill mismatch may look odd in-game. \u{201c}Sync\u{201d} keeps Level and XP consistent.");
}

/// A head/skin picker for the character's class. Writes the chosen asset path
/// (or the stock "Default") into `current`. Head/skin apply on Save/Download.
fn customization_combo(
    ui: &mut egui::Ui,
    id_salt: &str,
    class_def: &str,
    is_head: bool,
    current: &mut String,
) {
    let display = if current == "0" || current.is_empty() || current.contains("_Default") {
        "Default".to_string()
    } else {
        bl2_save::customization_name(current)
            .map(str::to_string)
            .unwrap_or_else(|| current.rsplit('.').next().unwrap_or(current).to_string())
    };
    egui::ComboBox::from_id_salt(id_salt).width(320.0).selected_text(display).show_ui(ui, |ui| {
        if let Some(def) = bl2_save::default_customization(class_def, is_head) {
            if ui.selectable_label(*current == def, "Default").clicked() {
                *current = def;
            }
        }
        for c in bl2_save::customizations(class_def, is_head) {
            if ui.selectable_label(*current == c.path, &c.name).clicked() {
                *current = c.path;
            }
        }
    });
}

/// General tab: playthrough progression + read-only save info.
fn general_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    const PT: [&str; 3] = ["Normal (NVHM)", "True Vault Hunter (TVHM)", "Ultimate Vault Hunter (UVHM)"];

    ui.horizontal(|ui| {
        theme::flag(ui, 20.0, accent);
        ui.label(egui::RichText::new("Playthrough").color(accent).size(18.0).strong());
    });
    ui.add_space(4.0);

    egui::Grid::new("general").num_columns(2).spacing([24.0, 8.0]).striped(true).show(ui, |ui| {
        key(ui, "Playthroughs completed", accent);
        ui.horizontal(|ui| {
            edit_number(ui, &mut doc.playthroughs_completed, 1.0);
            doc.playthroughs_completed = doc.playthroughs_completed.clamp(0, 3);
            let hint = match doc.playthroughs_completed {
                0 => "TVHM & UVHM locked",
                1 => "TVHM unlocked",
                _ => "TVHM & UVHM unlocked",
            };
            ui.weak(hint);
        });
        ui.end_row();

        key(ui, "Current playthrough", accent);
        let cur = *PT.get(doc.active_playthrough.clamp(0, 2) as usize).unwrap_or(&PT[0]);
        egui::ComboBox::from_id_salt("playthrough_combo").selected_text(cur).show_ui(ui, |ui| {
            for (i, name) in PT.iter().enumerate() {
                if ui.selectable_label(doc.active_playthrough == i as i64, *name).clicked() {
                    doc.active_playthrough = i as i64;
                }
            }
        });
        ui.end_row();

        key(ui, "Overpower level", accent);
        ui.horizontal(|ui| {
            edit_number(ui, &mut doc.op_level, 1.0);
            doc.op_level = doc.op_level.clamp(0, 80);
            ui.weak(if doc.op_level == 0 { "off" } else { "OP levels unlocked" });
        });
        ui.end_row();

        if let Some(id) = doc.save.save_game_id() {
            field(ui, "Save ID", &id.to_string(), accent);
        }
        if let Some(secs) = doc.save.time_played() {
            let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
            field(ui, "Time played", &format!("{h}h {m:02}m {s:02}s"), accent);
        }
    });

    ui.add_space(6.0);
    ui.colored_label(
        theme::DANGER,
        "⚠ Set \u{201c}playthroughs completed\u{201d} to unlock TVHM/UVHM. Changing the current \
         playthrough switches the mode you load into — only do so if that mode is unlocked.",
    );
    ui.weak(
        "Overpower level only matters at level 72 with UVHM completed; after saving, pick the OP \
         level at the character-select screen.",
    );
}

/// Currency tab: money, eridium (seraph/torgue coming next slice).
fn currency_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    egui::Grid::new("currency").num_columns(2).spacing([24.0, 8.0]).striped(true).show(ui, |ui| {
        key(ui, "Money", accent);
        ui.horizontal(|ui| {
            theme::coin(ui, 16.0);
            edit_number(ui, &mut doc.money, 5000.0);
        });
        ui.end_row();
        key(ui, "Eridium", accent);
        ui.horizontal(|ui| {
            theme::eridium(ui, 16.0);
            edit_number(ui, &mut doc.eridium, 1.0);
        });
        ui.end_row();
        key(ui, "Seraph Crystals", accent);
        ui.horizontal(|ui| {
            theme::seraph(ui, 16.0);
            edit_number(ui, &mut doc.seraph, 1.0);
        });
        ui.end_row();
        key(ui, "Torgue Tokens", accent);
        ui.horizontal(|ui| {
            theme::torgue(ui, 16.0);
            edit_number(ui, &mut doc.torgue, 1.0);
        });
        ui.end_row();
    });
}

/// Items tab: backpack + bank list with per-item level + parts editing.
fn items_tab(
    doc: &mut Doc,
    ui: &mut egui::Ui,
    accent: egui::Color32,
    text: egui::Color32,
) -> Option<(bool, String)> {
    let mut status = None;

    // Import: paste a BL2(...) code and drop a fresh copy into backpack or bank.
    ui.horizontal(|ui| {
        theme::crate_icon(ui, 14.0, accent);
        ui.label("Add item from code:");
        ui.add(
            egui::TextEdit::singleline(&mut doc.import_code)
                .hint_text("BL2(...)")
                .desired_width(260.0),
        );
        let code = doc.import_code.trim().to_string();
        let valid = code.starts_with("BL2(") && code.ends_with(')');
        let mut do_import = None;
        if ui.add_enabled(valid, egui::Button::new("→ Backpack")).clicked() {
            do_import = Some(false);
        }
        if ui.add_enabled(valid, egui::Button::new("→ Bank")).clicked() {
            do_import = Some(true);
        }
        if let Some(to_bank) = do_import {
            status = Some(match doc.save.add_item_from_code(&code, to_bank) {
                Ok(()) => {
                    rebuild_items_preserving_levels(doc);
                    doc.import_code.clear();
                    (false, format!("Imported item into {}.", if to_bank { "bank" } else { "backpack" }))
                }
                Err(e) => (true, format!("Import failed: {e}")),
            });
        }
    });
    ui.add_space(4.0);

    if doc.items.is_empty() {
        ui.weak("No items in backpack or bank. Paste a BL2(...) code above to add one.");
        return status;
    }
    // Convenience: set every levelable item to one level.
    ui.horizontal(|ui| {
        ui.label("Set all to level");
        ui.add(egui::DragValue::new(&mut doc.item_level).speed(1.0));
        doc.item_level = doc.item_level.clamp(1, 127);
        if ui
            .button("Apply to all")
            .on_hover_text("Sets every levelable item (both bags) to this level. Locked ⚠ items are left alone.")
            .clicked()
        {
            let lvl = doc.item_level;
            for v in doc.items.iter_mut().filter(|v| v.levelable) {
                v.level = lvl;
            }
        }
    });
    ui.add_space(4.0);

    // Backpack / Bank split — pick which bag to show.
    let backpack_n = doc.items.iter().filter(|v| !v.is_bank).count();
    let bank_n = doc.items.iter().filter(|v| v.is_bank).count();
    ui.horizontal(|ui| {
        theme::crate_icon(ui, 14.0, if doc.show_bank { text } else { accent });
        if ui.selectable_label(!doc.show_bank, format!("Backpack ({backpack_n})")).clicked() {
            doc.show_bank = false;
        }
        ui.add_space(8.0);
        theme::crate_icon(ui, 14.0, if doc.show_bank { accent } else { text });
        if ui.selectable_label(doc.show_bank, format!("Bank ({bank_n})")).clicked() {
            doc.show_bank = true;
        }
    });
    ui.add_space(2.0);

    let show_bank = doc.show_bank;
    if doc.items.iter().filter(|v| v.is_bank == show_bank).count() == 0 {
        ui.weak(if show_bank { "Bank is empty." } else { "Backpack is empty." });
    }

    let mut open_parts = None;
    let mut copy_code = None;
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        egui::Grid::new("items_grid")
            .num_columns(5)
            .spacing([16.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for v in doc.items.iter_mut().filter(|v| v.is_bank == show_bank) {
                    let (kcol, kind) = if v.is_weapon { (accent, "weapon") } else { (text, "item") };
                    ui.label(egui::RichText::new(kind).color(kcol).strong());
                    if v.levelable {
                        ui.add(egui::DragValue::new(&mut v.level).speed(1.0));
                        v.level = v.level.clamp(1, 127);
                    } else {
                        ui.horizontal(|ui| {
                            ui.monospace(format!("Lv {}", v.level));
                            ui.label(egui::RichText::new("⚠").color(theme::DANGER)).on_hover_text(
                                "No-level item (starter/special gear — some grenades, relics, class mods). \
                                 Leveling it can make the game drop it, so it's locked.",
                            );
                        });
                    }
                    ui.monospace(&v.name).on_hover_text(&v.details);
                    if ui.small_button("Parts").clicked() {
                        open_parts = Some(v.id);
                    }
                    if ui
                        .small_button("Code")
                        .on_hover_text("Copy this item's shareable BL2(...) code to the clipboard.")
                        .clicked()
                    {
                        copy_code = Some(v.id);
                    }
                    ui.end_row();
                }
            });
    });
    if let Some(oid) = open_parts {
        doc.editing_parts = if doc.editing_parts == Some(oid) { None } else { Some(oid) };
        doc.editing_part_slot = None;
        doc.part_filter.clear();
    }
    if let Some(id) = copy_code {
        status = Some(match doc.save.item_code(id) {
            Ok(Some(code)) => {
                ui.ctx().copy_text(code.clone());
                (false, format!("Copied code to clipboard: {code}"))
            }
            Ok(None) => (true, "Could not build a code for that item.".to_string()),
            Err(e) => (true, format!("Code failed: {e}")),
        });
    }
    ui.add_space(2.0);
    ui.weak(
        "Edit a level or use \u{201c}Apply to all\u{201d}, then Save/Download. Locked \u{26a0} items can't be leveled. \
         \u{201c}Parts\u{201d} swaps parts; \u{201c}Code\u{201d} copies a shareable BL2(...) code. Edited items unequip in-game — re-equip.",
    );
    parts_editor(doc, ui, accent);
    status
}

/// Fast Travel tab: tick which stations are unlocked (base game + DLC), grouped
/// by pack. Changes are scratch and written to field 16 on Save/Download.
fn fast_travel_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    let catalog = bl2_save::stations_catalog();
    let last = doc.save.last_station();

    ui.horizontal(|ui| {
        theme::signpost(ui, 20.0, accent);
        ui.label(egui::RichText::new("Fast Travel").color(accent).size(18.0).strong());
    });
    if let Some(l) = &last {
        let shown = bl2_save::station_display_name(l).unwrap_or(l);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Current station:").color(accent));
            ui.label(shown);
        });
    }
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        if ui.button("Unlock all").clicked() {
            for s in catalog {
                doc.unlocked.insert(s.rn.clone());
            }
        }
        if ui.button("Lock all").clicked() {
            for s in catalog {
                doc.unlocked.remove(&s.rn);
            }
        }
        let on = catalog.iter().filter(|s| doc.unlocked.contains(&s.rn)).count();
        ui.label(egui::RichText::new(format!("{on} / {} unlocked", catalog.len())).color(accent));
    });
    ui.add_space(2.0);

    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
        let mut pack = "";
        for s in catalog {
            if s.pack != pack {
                pack = &s.pack;
                ui.add_space(4.0);
                ui.label(egui::RichText::new(pack).color(accent).strong());
            }
            let mut on = doc.unlocked.contains(&s.rn);
            let here = last.as_deref() == Some(s.rn.as_str());
            let label = if here { format!("★ {}", s.name) } else { s.name.clone() };
            if ui.checkbox(&mut on, label).changed() {
                if on {
                    doc.unlocked.insert(s.rn.clone());
                } else {
                    doc.unlocked.remove(&s.rn);
                }
            }
        }
    });
    ui.add_space(6.0);
    ui.weak("Tick stations to unlock them, then Save/Download.");
}

/// Raw tab: a read-only dump of every top-level protobuf field.
fn raw_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    ui.horizontal(|ui| {
        theme::page(ui, 20.0, accent);
        ui.label(egui::RichText::new("Raw fields").color(accent).size(18.0).strong());
    });
    ui.weak(
        "Named top-level save fields. Single scalar fields (varint/string) are editable inline — \
         changes apply immediately. Advanced: back up first.",
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.add(egui::TextEdit::singleline(&mut doc.raw_filter).hint_text("field name or number…"));
        if ui.button("✕").clicked() {
            doc.raw_filter.clear();
        }
    });
    ui.add_space(2.0);

    let needle = doc.raw_filter.to_lowercase();
    let fields = doc.save.raw_fields().unwrap_or_default();
    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
        egui::Grid::new("raw_grid").num_columns(3).spacing([16.0, 4.0]).striped(true).show(ui, |ui| {
            ui.label(egui::RichText::new("field").color(accent).strong());
            ui.label(egui::RichText::new("type").color(accent).strong());
            ui.label(egui::RichText::new("value").color(accent).strong());
            ui.end_row();
            for f in &fields {
                let label = if f.name.is_empty() { format!("field {}", f.number) } else { f.name.to_string() };
                if !needle.is_empty()
                    && !label.to_lowercase().contains(&needle)
                    && !f.number.to_string().contains(&needle)
                {
                    continue;
                }
                ui.monospace(label).on_hover_text(format!("field #{}", f.number));
                ui.weak(f.kind);
                if let Some(v) = f.value {
                    let mut v2 = v;
                    if ui.add(egui::DragValue::new(&mut v2).speed(1.0)).changed() {
                        let _ = doc.save.set_raw_varint(f.number, v2);
                    }
                } else if let Some(t) = &f.text {
                    let mut t2 = t.clone();
                    if ui.text_edit_singleline(&mut t2).changed() {
                        let _ = doc.save.set_raw_string(f.number, &t2);
                    }
                } else {
                    ui.monospace(&f.preview);
                }
                ui.end_row();
            }
        });
    });
}

/// About tab: what this is, what it edits, credits, and the IP/asset policy.
fn about_tab(ui: &mut egui::Ui, accent: egui::Color32) {
    ui.horizontal(|ui| {
        theme::emblem(ui, 28.0, accent);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("BL2 Save Editor").color(accent).size(20.0).strong());
    });
    ui.add_space(6.0);
    ui.label(
        "A Linux-native, dependency-light Borderlands 2 save editor written in Rust. It runs as a \
         web app (this page) and as a native desktop app from the same code — no Wine, Mono, or \
         installed game files required.",
    );
    ui.add_space(8.0);

    let head = |ui: &mut egui::Ui, s: &str| {
        ui.label(egui::RichText::new(s).color(accent).strong());
    };
    head(ui, "What it edits");
    for s in [
        "Character — name, class, head/skin, level, XP, skill points",
        "Currency — money, eridium, seraph crystals, torgue tokens",
        "Items — per-item level, parts, shareable BL2(…) codes, backpack ↔ bank",
        "Fast Travel — unlock stations (base game + DLC)",
    ] {
        ui.label(format!("   •  {s}"));
    }
    ui.add_space(8.0);
    head(ui, "Art & data");
    ui.label(
        "Every icon and theme is original art drawn in code — no Gearbox/2K assets are bundled. \
         Item and part names come from open identifier data (zlib-licensed).",
    );
    ui.add_space(10.0);
    ui.colored_label(theme::DANGER, "⚠ Always back up your save before editing.");
}

/// Rebuild the item views from the save after a structural change (e.g. import),
/// preserving any unsaved per-item level scratch edits by id.
fn rebuild_items_preserving_levels(doc: &mut Doc) {
    let old: std::collections::HashMap<usize, i64> =
        doc.items.iter().map(|v| (v.id, v.level)).collect();
    doc.items = build_item_views(&doc.save);
    for v in &mut doc.items {
        if let Some(&lv) = old.get(&v.id) {
            v.level = lv;
        }
    }
}

/// Render the open item's slot list. Each "change" opens the picker modal
/// (rendered separately) for that slot. The actual swap happens in the modal.
/// The item detail form: header (type/balance/manufacturer/level) plus a list of
/// this item's part slots, each with its human name (see [`bl2_save::slot_label`])
/// and a "change" button that opens the picker modal.
fn parts_editor(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    let Some(id) = doc.editing_parts else { return };
    let Some(idx) = doc.items.iter().position(|v| v.id == id) else {
        doc.editing_parts = None;
        return;
    };
    let v = &doc.items[idx];
    let item_name = v.name.clone();
    let is_weapon = v.is_weapon;
    let (kind, type_name, balance, manufacturer, level) = (
        if is_weapon { "Weapon" } else { "Item" },
        v.type_name.clone(),
        v.balance.clone(),
        v.manufacturer.clone(),
        v.level,
    );
    let slots: Vec<(usize, String)> = v.parts.iter().map(|p| (p.slot, p.name.clone())).collect();

    ui.add_space(6.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("Details — {item_name}")).color(accent).strong());
        if ui.button("Close").clicked() {
            doc.editing_parts = None;
            doc.editing_part_slot = None;
        }
    });
    if doc.editing_parts.is_none() {
        return;
    }

    // Read-only header: what this item is.
    egui::Grid::new("item_header_grid").num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
        let field = |ui: &mut egui::Ui, k: &str, val: &str| {
            ui.label(egui::RichText::new(k).color(accent));
            ui.monospace(if val.is_empty() { "—" } else { val });
            ui.end_row();
        };
        field(ui, "Kind", kind);
        field(ui, "Type", &type_name);
        field(ui, "Balance", &balance);
        if !manufacturer.is_empty() {
            field(ui, "Manufacturer", &manufacturer);
        }
        field(ui, "Level", &format!("{level}"));
    });

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Parts").color(accent).strong());
    ui.colored_label(
        theme::DANGER,
        "⚠ Changing parts can create items the game rejects — back up first and verify in-game.",
    );
    egui::Grid::new("parts_grid").num_columns(3).spacing([10.0, 4.0]).show(ui, |ui| {
        for (slot, name) in &slots {
            ui.label(egui::RichText::new(bl2_save::slot_label(is_weapon, *slot)).strong());
            ui.monospace(name);
            if ui.small_button("change").clicked() {
                doc.editing_part_slot = Some(*slot);
                doc.part_filter.clear();
            }
            ui.end_row();
        }
    });
}

/// Contents of the install-instructions modal.
fn install_help_ui(ui: &mut egui::Ui, accent: egui::Color32, close: &mut bool) {
    ui.set_max_width(480.0);
    ui.label(egui::RichText::new("Installing your edited save").color(accent).size(20.0).strong());
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
        ui.monospace("Windows:  …\\Documents\\My Games\\Borderlands 2\\WillowGame\\SaveData\\<SteamID>\\");
    });
    step(ui, "3.", "Back up the existing save there (copy it somewhere safe).");
    step(ui, "4.", "Replace it with your download, keeping the same name (e.g. save0001.sav).");
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
