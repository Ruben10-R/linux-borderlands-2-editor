use bl2_save::SaveFile;
use eframe::egui;

use crate::theme;

/// The read-only viewer application state.
#[derive(Default)]
pub struct App {
    loaded: Option<Loaded>,
    error: Option<String>,
    theme: theme::Theme,
}

/// Everything we display for one loaded save (all read-only).
struct Loaded {
    file: String,
    class: String,
    level: i64,
    xp: i64,
    money: i64,
    eridium: i64,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let app = Self::default();
        theme::apply(&cc.egui_ctx, app.theme);
        app
    }

    /// Parse dropped bytes into the display model, or record the error.
    fn load(&mut self, name: String, bytes: &[u8]) {
        match SaveFile::from_bytes(bytes) {
            Ok(s) => {
                self.loaded = Some(Loaded {
                    file: name,
                    class: s.class_name().unwrap_or_else(|| "Unknown".into()),
                    level: s.level().unwrap_or(0),
                    xp: s.xp().unwrap_or(0),
                    money: s.money(),
                    eridium: s.eridium(),
                });
                self.error = None;
            }
            Err(e) => {
                self.error = Some(e.to_string());
                self.loaded = None;
            }
        }
    }

    /// Handle any file dropped onto the window this frame. On web the bytes are
    /// delivered directly; on native we get a path and read it ourselves.
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

        if let Some(bytes) = &f.bytes {
            self.load(name, bytes);
        } else if let Some(path) = &f.path {
            match std::fs::read(path) {
                Ok(b) => self.load(name, &b),
                Err(e) => {
                    self.error = Some(format!("could not read {}: {e}", path.display()));
                    self.loaded = None;
                }
            }
        }
    }
}

impl eframe::App for App {
    // eframe 0.34 hands us a root `Ui`; panels go in via `show_inside`.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_dropped(&ctx);

        // Bottom hint (added before the central panel so layout is correct).
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
                    ui.label(
                        egui::RichText::new("BL2 SAVE EDITOR")
                            .color(accent)
                            .size(24.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("read-only save viewer")
                            .color(self.theme.text())
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
                        theme::apply(&ctx, t);
                    }
                }
            });

            ui.add_space(4.0);
            ui.separator();
            ui.label("Drag a .sav file onto this window to inspect it.");
            ui.add_space(6.0);

            if let Some(err) = &self.error {
                ui.colored_label(theme::DANGER, format!("⚠ {err}"));
            }

            let accent = self.theme.accent();
            match &self.loaded {
                Some(s) => {
                    egui::Grid::new("general")
                        .num_columns(2)
                        .spacing([24.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            field(ui, "File", &s.file, accent);
                            field(ui, "Class", &s.class, accent);
                            field(ui, "Level", &s.level.to_string(), accent);
                            field(ui, "XP", &s.xp.to_string(), accent);

                            key(ui, "Money", accent);
                            ui.horizontal(|ui| {
                                theme::coin(ui, 16.0);
                                ui.monospace(s.money.to_string());
                            });
                            ui.end_row();

                            key(ui, "Eridium", accent);
                            ui.horizontal(|ui| {
                                theme::eridium(ui, 16.0);
                                ui.monospace(s.eridium.to_string());
                            });
                            ui.end_row();
                        });
                }
                None if self.error.is_none() => {
                    ui.add_space(8.0);
                    ui.weak("No save loaded yet.");
                }
                None => {}
            }
        });
    }
}

/// A bold accent-coloured key label (left column of the grid).
fn key(ui: &mut egui::Ui, text: &str, accent: egui::Color32) {
    ui.label(egui::RichText::new(text).color(accent).strong());
}

/// A full "key : value" grid row with a monospace value.
fn field(ui: &mut egui::Ui, k: &str, value: &str, accent: egui::Color32) {
    key(ui, k, accent);
    ui.monospace(value);
    ui.end_row();
}
