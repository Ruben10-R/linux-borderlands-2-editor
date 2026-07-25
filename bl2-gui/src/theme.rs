//! Original Borderlands-*flavored* themes — selectable palettes + hand-drawn art.
//!
//! IMPORTANT: everything here is our own art, drawn in code. We deliberately do
//! NOT use Gearbox/2K's copyrighted icons, logo, or the vault symbol. We evoke the
//! cel-shaded look (deep background + a bold accent, heavy black outlines) with
//! original shapes only. See ASSETS.md.

use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke, Vec2};

// Fixed cel outline + semantic glyph colors (money is gold, eridium is purple),
// kept constant across themes so the icons stay readable and meaningful.
const OUTLINE: Color32 = Color32::from_rgb(0x0C, 0x0B, 0x09);
const COIN: Color32 = Color32::from_rgb(0xF5, 0xB1, 0x1E);
const COIN_DK: Color32 = Color32::from_rgb(0x8A, 0x59, 0x0A);
const GEM: Color32 = Color32::from_rgb(0x9B, 0x5C, 0xF0);
pub const DANGER: Color32 = Color32::from_rgb(0xF0, 0x54, 0x40);

/// A selectable colour scheme (chrome only — semantic glyphs stay constant).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    VaultHunter,
    Eridium,
    Slaughter,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::VaultHunter
    }
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::VaultHunter, Theme::Eridium, Theme::Slaughter];

    pub fn label(self) -> &'static str {
        match self {
            Theme::VaultHunter => "Vault Hunter",
            Theme::Eridium => "Eridium",
            Theme::Slaughter => "Slaughter",
        }
    }

    fn palette(self) -> Palette {
        match self {
            // Warm near-black + amber gold.
            Theme::VaultHunter => Palette {
                ink: Color32::from_rgb(0x1B, 0x1A, 0x16),
                ink_soft: Color32::from_rgb(0x27, 0x25, 0x1E),
                extreme: Color32::from_rgb(0x12, 0x11, 0x0E),
                text: Color32::from_rgb(0xEC, 0xE6, 0xD6),
                accent: Color32::from_rgb(0xF5, 0xB1, 0x1E),
            },
            // Deep indigo + violet.
            Theme::Eridium => Palette {
                ink: Color32::from_rgb(0x16, 0x12, 0x20),
                ink_soft: Color32::from_rgb(0x23, 0x1D, 0x33),
                extreme: Color32::from_rgb(0x0F, 0x0C, 0x18),
                text: Color32::from_rgb(0xE7, 0xE2, 0xF2),
                accent: Color32::from_rgb(0xB0, 0x7C, 0xF5),
            },
            // Dark blood + red-orange.
            Theme::Slaughter => Palette {
                ink: Color32::from_rgb(0x1E, 0x13, 0x11),
                ink_soft: Color32::from_rgb(0x30, 0x1D, 0x18),
                extreme: Color32::from_rgb(0x14, 0x0D, 0x0B),
                text: Color32::from_rgb(0xF0, 0xE2, 0xD8),
                accent: Color32::from_rgb(0xE8, 0x53, 0x36),
            },
        }
    }

    /// The accent colour for headings, keys, the emblem, etc.
    pub fn accent(self) -> Color32 {
        self.palette().accent
    }

    /// The primary text colour.
    pub fn text(self) -> Color32 {
        self.palette().text
    }
}

struct Palette {
    ink: Color32,
    ink_soft: Color32,
    extreme: Color32,
    text: Color32,
    accent: Color32,
}

/// Apply the chosen theme to the whole context. Call at startup and on change.
pub fn apply(ctx: &egui::Context, theme: Theme) {
    let p = theme.palette();
    let mut v = egui::Visuals::dark();
    v.panel_fill = p.ink;
    v.window_fill = p.ink;
    v.faint_bg_color = p.ink_soft; // striped grid rows
    v.extreme_bg_color = p.extreme;
    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.accent.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    ctx.set_visuals(v);
}

fn alloc(ui: &mut egui::Ui, size: f32) -> (egui::Rect, egui::Painter) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    (rect, painter)
}

/// Our original app emblem: a bold hexagon badge (in the theme accent) with an
/// inner diamond. Not the vault symbol — a plain geometric mark of our own.
pub fn emblem(ui: &mut egui::Ui, size: f32, accent: Color32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.46;
    let hex: Vec<Pos2> = (0..6)
        .map(|i| {
            let a = std::f32::consts::TAU * (i as f32) / 6.0 - std::f32::consts::FRAC_PI_2;
            Pos2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    p.add(Shape::convex_polygon(hex, accent, Stroke::new(size * 0.11, OUTLINE)));
    let d = size * 0.20;
    let diamond = vec![
        Pos2::new(c.x, c.y - d),
        Pos2::new(c.x + d, c.y),
        Pos2::new(c.x, c.y + d),
        Pos2::new(c.x - d, c.y),
    ];
    p.add(Shape::convex_polygon(diamond, OUTLINE, Stroke::new(1.0_f32, OUTLINE)));
}

/// A gold coin glyph for the money row (semantic — same in every theme).
pub fn coin(ui: &mut egui::Ui, size: f32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.45;
    p.circle(c, r, COIN, Stroke::new(size * 0.11, OUTLINE));
    p.circle_stroke(c, r * 0.52, Stroke::new(size * 0.10, COIN_DK));
}

/// A purple gem glyph for the eridium row (semantic — same in every theme).
pub fn eridium(ui: &mut egui::Ui, size: f32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.44;
    let gem = vec![
        Pos2::new(c.x, c.y - r),
        Pos2::new(c.x + r * 0.72, c.y),
        Pos2::new(c.x, c.y + r),
        Pos2::new(c.x - r * 0.72, c.y),
    ];
    p.add(Shape::convex_polygon(gem, GEM, Stroke::new(size * 0.11, OUTLINE)));
}
