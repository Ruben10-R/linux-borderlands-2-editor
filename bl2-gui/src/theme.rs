//! Original Borderlands-*flavored* theme — palette + hand-drawn emblems.
//!
//! IMPORTANT: everything here is our own art, drawn in code. We deliberately do
//! NOT use Gearbox/2K's copyrighted icons, logo, or the vault symbol. We evoke the
//! cel-shaded look (warm near-black + amber, bold black outlines) with original
//! shapes only. See ASSETS.md.

use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke, Vec2};

pub const INK: Color32 = Color32::from_rgb(0x1B, 0x1A, 0x16); // warm near-black background
pub const INK_SOFT: Color32 = Color32::from_rgb(0x27, 0x25, 0x1E); // panels / striped rows
pub const CREAM: Color32 = Color32::from_rgb(0xEC, 0xE6, 0xD6); // primary text
pub const GOLD: Color32 = Color32::from_rgb(0xF5, 0xB1, 0x1E); // amber accent
pub const GOLD_DK: Color32 = Color32::from_rgb(0x8A, 0x59, 0x0A);
pub const ERIDIUM: Color32 = Color32::from_rgb(0x9B, 0x5C, 0xF0); // purple
pub const OUTLINE: Color32 = Color32::from_rgb(0x0C, 0x0B, 0x09); // bold cel outline
pub const DANGER: Color32 = Color32::from_rgb(0xE0, 0x54, 0x3C);

/// Apply the palette to the whole context (call once at startup).
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = INK;
    v.window_fill = INK;
    v.faint_bg_color = INK_SOFT; // striped grid rows
    v.extreme_bg_color = Color32::from_rgb(0x12, 0x11, 0x0E);
    v.override_text_color = Some(CREAM);
    v.hyperlink_color = GOLD;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0xF5, 0xB1, 0x1E, 90);
    v.selection.stroke = Stroke::new(1.0_f32,GOLD);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32,CREAM);
    ctx.set_visuals(v);
}

fn alloc<'u>(ui: &'u mut egui::Ui, size: f32) -> (egui::Rect, egui::Painter) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    (rect, painter)
}

/// Our original app emblem: a bold gold hexagon badge with an inner diamond.
/// (Not the vault symbol — a plain geometric mark of our own.)
pub fn emblem(ui: &mut egui::Ui, size: f32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.46;
    let hex: Vec<Pos2> = (0..6)
        .map(|i| {
            let a = std::f32::consts::TAU * (i as f32) / 6.0 - std::f32::consts::FRAC_PI_2;
            Pos2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    p.add(Shape::convex_polygon(hex, GOLD, Stroke::new(size * 0.11, OUTLINE)));
    let d = size * 0.20;
    let diamond = vec![
        Pos2::new(c.x, c.y - d),
        Pos2::new(c.x + d, c.y),
        Pos2::new(c.x, c.y + d),
        Pos2::new(c.x - d, c.y),
    ];
    p.add(Shape::convex_polygon(diamond, OUTLINE, Stroke::new(1.0_f32,OUTLINE)));
}

/// A gold coin glyph for the money row.
pub fn coin(ui: &mut egui::Ui, size: f32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.45;
    p.circle(c, r, GOLD, Stroke::new(size * 0.11, OUTLINE));
    p.circle_stroke(c, r * 0.52, Stroke::new(size * 0.10, GOLD_DK));
}

/// A purple gem glyph for the eridium row.
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
    p.add(Shape::convex_polygon(gem, ERIDIUM, Stroke::new(size * 0.11, OUTLINE)));
}
