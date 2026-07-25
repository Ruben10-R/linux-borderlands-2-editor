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
    level: i64,
    xp: i64,
    items: Vec<ItemView>,
}

/// A read-only display row for one inventory item.
struct ItemView {
    location: &'static str,
    is_weapon: bool,
    level: i64,
    refs: String,
}

fn build_item_views(s: &SaveFile) -> Vec<ItemView> {
    s.items()
        .unwrap_or_default()
        .into_iter()
        .filter(|it| !it.serial.is_placeholder())
        .map(|it| {
            let ser = &it.serial;
            ItemView {
                location: match it.location {
                    Location::Backpack => "backpack",
                    Location::Bank => "bank",
                },
                is_weapon: ser.is_weapon,
                level: ser.stage.unwrap_or(0),
                refs: {
                    let manu = ser.manufacturer_name().unwrap_or_default();
                    let ty = ser.type_name().unwrap_or_else(|| {
                        format!("type {}:{}", ser.item_type.lib, ser.item_type.asset)
                    });
                    format!("{manu} {ty}").trim().to_string()
                },
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
                    items: build_item_views(&s),
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
        self.status = Some(persist(doc));
    }
}

/// Push the scratch values into the save via the guarded setters.
fn apply_edits(doc: &mut Doc) -> Result<(), SaveError> {
    doc.save.set_money(doc.money.clamp(0, MAX))?;
    doc.save.set_eridium(doc.eridium.clamp(0, MAX))?;
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
        self.handle_dropped(&ctx);

        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            egui::Panel::bottom("drop_hint").show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| ui.label("⤵ Drop to load"));
            });
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let accent = self.theme.accent();

            // Header: original emblem + wordmark.
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                theme::emblem(ui, 44.0, accent);
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("BL2 SAVE EDITOR").color(accent).size(24.0).strong());
                    ui.label(
                        egui::RichText::new("edit your General stats").color(self.theme.text()).italics(),
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
                        theme::apply(&ctx, t);
                    }
                }
            });

            ui.add_space(4.0);
            ui.separator();
            ui.label("Drag a .sav file onto this window to load it.");
            ui.add_space(6.0);

            if self.doc.is_some() {
                let accent = self.theme.accent();
                {
                    let doc = self.doc.as_mut().unwrap();
                    egui::Grid::new("general")
                        .num_columns(2)
                        .spacing([24.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            field(ui, "File", &doc.name, accent);
                            field(ui, "Class", &doc.class, accent);

                            key(ui, "Level", accent);
                            edit_number(ui, &mut doc.level, 1.0);
                            ui.end_row();

                            key(ui, "XP", accent);
                            edit_number(ui, &mut doc.xp, 1000.0);
                            ui.end_row();

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
                        });
                } // doc borrow ends

                ui.add_space(12.0);
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
                });
                if cfg!(target_arch = "wasm32") {
                    ui.add_space(2.0);
                    ui.weak("Downloads a .sav — click \u{201c}How to install\u{201d} to put it in your game.");
                }
            } else {
                ui.add_space(8.0);
                ui.weak("No save loaded yet.");
            }

            if let Some((is_err, msg)) = &self.status {
                ui.add_space(8.0);
                let col = if *is_err { theme::DANGER } else { self.theme.accent() };
                ui.colored_label(col, msg);
            }

            // Read-only item/weapon list.
            if let Some(doc) = &self.doc {
                if !doc.items.is_empty() {
                    let accent = self.theme.accent();
                    let text = self.theme.text();
                    ui.add_space(10.0);
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("Items ({})", doc.items.len())).color(accent).strong(),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                            egui::Grid::new("items_grid")
                                .num_columns(4)
                                .spacing([16.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    for v in &doc.items {
                                        ui.label(v.location);
                                        let (kcol, kind) =
                                            if v.is_weapon { (accent, "weapon") } else { (text, "item") };
                                        ui.label(egui::RichText::new(kind).color(kcol).strong());
                                        ui.monospace(format!("Lv {}", v.level));
                                        ui.monospace(&v.refs);
                                        ui.end_row();
                                    }
                                });
                        });
                        ui.add_space(2.0);
                        ui.weak("manufacturer + type from GameInfo; balance/part names coming later.");
                    });
                }
            }
        });

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
