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

/// Which editor tab is shown (more will be added — Fast Travel, Vehicle, …).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Character,
    Currency,
    Items,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Character, Tab::Currency, Tab::Items];
    fn label(self) -> &'static str {
        match self {
            Tab::Character => "Character",
            Tab::Currency => "Currency",
            Tab::Items => "Items",
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
    class: String,
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
}

/// One inventory row: `level` is an editable scratch value applied on save.
struct ItemView {
    id: usize,
    location: &'static str,
    is_weapon: bool,
    /// False for grade-≤1 "no-level" items — locked (leveling can break them).
    levelable: bool,
    level: i64,
    name: String,
    /// Balance + parts breakdown, shown on hover.
    details: String,
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
            ItemView {
                id: it.id,
                location: match it.location {
                    Location::Backpack => "backpack",
                    Location::Bank => "bank",
                },
                is_weapon: ser.is_weapon,
                levelable: ser.is_levelable(),
                level: ser.stage.unwrap_or(0),
                name: {
                    let manu = ser.manufacturer_name().unwrap_or_default();
                    let ty = ser.type_name().unwrap_or_else(|| {
                        format!("type {}:{}", ser.item_type.lib, ser.item_type.asset)
                    });
                    format!("{manu} {ty}").trim().to_string()
                },
                details,
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
                    class: s.class_name().unwrap_or_else(|| "Unknown".into()),
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

            // Tab bar.
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for t in Tab::ALL {
                    let label = egui::RichText::new(t.label())
                        .color(if self.tab == t { accent } else { text })
                        .strong();
                    if ui.selectable_label(self.tab == t, label).clicked() {
                        self.tab = t;
                    }
                }
            });
            ui.separator();
            ui.add_space(6.0);

            // Tab content.
            let tab = self.tab;
            let doc = self.doc.as_mut().unwrap();
            match tab {
                Tab::Character => character_tab(doc, ui, accent),
                Tab::Currency => currency_tab(doc, ui, accent),
                Tab::Items => items_tab(doc, ui, accent, text),
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
            ui.label(egui::RichText::new(format!("Slot {slot} — choose a part")).color(accent).strong());
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

/// Character tab: file, class, level, XP.
fn character_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    egui::Grid::new("character").num_columns(2).spacing([24.0, 8.0]).striped(true).show(ui, |ui| {
        field(ui, "File", &doc.name, accent);
        field(ui, "Class", &doc.class, accent);
        key(ui, "Level", accent);
        edit_number(ui, &mut doc.level, 1.0);
        ui.end_row();
        key(ui, "XP", accent);
        edit_number(ui, &mut doc.xp, 1000.0);
        ui.end_row();
    });
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
        edit_number(ui, &mut doc.seraph, 1.0);
        ui.end_row();
        key(ui, "Torgue Tokens", accent);
        edit_number(ui, &mut doc.torgue, 1.0);
        ui.end_row();
    });
}

/// Items tab: backpack + bank list with per-item level + parts editing.
fn items_tab(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32, text: egui::Color32) {
    if doc.items.is_empty() {
        ui.weak("No items in backpack or bank.");
        return;
    }
    // Convenience: set every levelable item to one level.
    ui.horizontal(|ui| {
        ui.label("Set all to level");
        ui.add(egui::DragValue::new(&mut doc.item_level).speed(1.0));
        doc.item_level = doc.item_level.clamp(1, 127);
        if ui
            .button("Apply to all")
            .on_hover_text("Sets every levelable item to this level. Locked ⚠ items are left alone.")
            .clicked()
        {
            let lvl = doc.item_level;
            for v in doc.items.iter_mut().filter(|v| v.levelable) {
                v.level = lvl;
            }
        }
    });
    ui.add_space(4.0);

    let mut open_parts = None;
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        egui::Grid::new("items_grid")
            .num_columns(5)
            .spacing([16.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for v in &mut doc.items {
                    ui.label(v.location);
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
                    ui.end_row();
                }
            });
    });
    if let Some(oid) = open_parts {
        doc.editing_parts = if doc.editing_parts == Some(oid) { None } else { Some(oid) };
        doc.editing_part_slot = None;
        doc.part_filter.clear();
    }
    ui.add_space(2.0);
    ui.weak(
        "Edit a level or use \u{201c}Apply to all\u{201d}, then Save/Download. Locked \u{26a0} items can't be leveled. \
         Click \u{201c}Parts\u{201d} to swap parts. Edited items unequip in-game — re-equip.",
    );
    parts_editor(doc, ui, accent);
}

/// Render the open item's slot list. Each "change" opens the picker modal
/// (rendered separately) for that slot. The actual swap happens in the modal.
fn parts_editor(doc: &mut Doc, ui: &mut egui::Ui, accent: egui::Color32) {
    let Some(id) = doc.editing_parts else { return };
    let Some(idx) = doc.items.iter().position(|v| v.id == id) else {
        doc.editing_parts = None;
        return;
    };
    let item_name = doc.items[idx].name.clone();
    let slots: Vec<(usize, String)> =
        doc.items[idx].parts.iter().map(|p| (p.slot, p.name.clone())).collect();

    ui.add_space(6.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("Parts — {item_name}")).color(accent).strong());
        if ui.button("Close").clicked() {
            doc.editing_parts = None;
            doc.editing_part_slot = None;
        }
    });
    if doc.editing_parts.is_none() {
        return;
    }
    ui.colored_label(
        theme::DANGER,
        "⚠ Changing parts can create items the game rejects — back up first and verify in-game.",
    );
    egui::Grid::new("parts_grid").num_columns(3).spacing([10.0, 4.0]).show(ui, |ui| {
        for (slot, name) in &slots {
            ui.monospace(format!("slot {slot:>2}"));
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
