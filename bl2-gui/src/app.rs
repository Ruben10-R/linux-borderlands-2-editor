use bl2_save::SaveFile;
use eframe::egui;

/// The read-only viewer application state.
#[derive(Default)]
pub struct App {
    loaded: Option<Loaded>,
    error: Option<String>,
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
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
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
            ui.heading("Borderlands 2 — Save Viewer");
            ui.label("Drag a .sav file onto this window to inspect it. (Read-only.)");
            ui.separator();

            if let Some(err) = &self.error {
                ui.colored_label(egui::Color32::from_rgb(0xD0, 0x40, 0x40), format!("⚠ {err}"));
            }

            match &self.loaded {
                Some(s) => {
                    egui::Grid::new("general")
                        .num_columns(2)
                        .spacing([24.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            field(ui, "File", &s.file);
                            field(ui, "Class", &s.class);
                            field(ui, "Level", &s.level.to_string());
                            field(ui, "XP", &s.xp.to_string());
                            field(ui, "Money", &s.money.to_string());
                            field(ui, "Eridium", &s.eridium.to_string());
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

fn field(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.strong(key);
    ui.monospace(value);
    ui.end_row();
}
