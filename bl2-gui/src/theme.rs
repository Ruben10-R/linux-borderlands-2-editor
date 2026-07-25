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
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    VaultHunter,
    Eridium,
    Slaughter,
    Corrosive,
    Shock,
}

impl Theme {
    pub const ALL: [Theme; 5] = [
        Theme::VaultHunter,
        Theme::Eridium,
        Theme::Slaughter,
        Theme::Corrosive,
        Theme::Shock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Theme::VaultHunter => "Vault Hunter",
            Theme::Eridium => "Eridium",
            Theme::Slaughter => "Slaughter",
            Theme::Corrosive => "Corrosive",
            Theme::Shock => "Shock",
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
            // Dark moss + acid green (corrosive).
            Theme::Corrosive => Palette {
                ink: Color32::from_rgb(0x14, 0x1A, 0x10),
                ink_soft: Color32::from_rgb(0x20, 0x29, 0x18),
                extreme: Color32::from_rgb(0x0D, 0x11, 0x0A),
                text: Color32::from_rgb(0xE2, 0xEC, 0xD4),
                accent: Color32::from_rgb(0x9C, 0xD9, 0x2B),
            },
            // Deep navy + electric blue (shock).
            Theme::Shock => Palette {
                ink: Color32::from_rgb(0x10, 0x16, 0x20),
                ink_soft: Color32::from_rgb(0x1A, 0x23, 0x31),
                extreme: Color32::from_rgb(0x0A, 0x0E, 0x16),
                text: Color32::from_rgb(0xDC, 0xE6, 0xF4),
                accent: Color32::from_rgb(0x3A, 0xC6, 0xF2),
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

    /// The darkest palette tone — the app backdrop behind Modern cards.
    pub fn backdrop(self) -> Color32 {
        self.palette().extreme
    }

    /// Stable index for on-disk persistence (never reorder these).
    pub fn index(self) -> u8 {
        match self {
            Theme::VaultHunter => 0,
            Theme::Eridium => 1,
            Theme::Slaughter => 2,
            Theme::Corrosive => 3,
            Theme::Shock => 4,
        }
    }

    pub fn from_index(i: u8) -> Theme {
        match i {
            1 => Theme::Eridium,
            2 => Theme::Slaughter,
            3 => Theme::Corrosive,
            4 => Theme::Shock,
            _ => Theme::VaultHunter,
        }
    }
}

/// Which visual *style* the UI wears. `Classic` is the original flat look;
/// `Modern` is rounded "card" components with elevation and roomier spacing
/// (PrimeVue/Vuetify-flavoured) — still on the Borderlands palette.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    Classic,
    #[default]
    Modern,
}

impl Look {
    pub fn label(self) -> &'static str {
        match self {
            Look::Classic => "Classic",
            Look::Modern => "Modern",
        }
    }

    /// The opposite look — what the floating toggle switches to.
    pub fn other(self) -> Look {
        match self {
            Look::Classic => Look::Modern,
            Look::Modern => Look::Classic,
        }
    }

    /// Stable index for on-disk persistence (never reorder these).
    pub fn index(self) -> u8 {
        match self {
            Look::Classic => 0,
            Look::Modern => 1,
        }
    }

    pub fn from_index(i: u8) -> Look {
        match i {
            0 => Look::Classic,
            _ => Look::Modern,
        }
    }
}

struct Palette {
    ink: Color32,
    ink_soft: Color32,
    extreme: Color32,
    text: Color32,
    accent: Color32,
}

/// Blend two colours: `t=0` → `a`, `t=1` → `b`.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// Lighten a colour towards white by `t`.
fn lighten(c: Color32, t: f32) -> Color32 {
    mix(c, Color32::WHITE, t)
}

/// Apply the chosen theme + look to the whole context. Called every frame (so
/// our styling wins over eframe's system-theme following) and on any change.
pub fn apply(ctx: &egui::Context, theme: Theme, look: Look) {
    let p = theme.palette();
    // Start from a fresh Style each frame so toggling back to Classic also
    // resets the roomier Modern spacing.
    let mut style = egui::Style::default();
    let mut v = egui::Visuals::dark();

    // Palette — shared by both looks.
    v.panel_fill = p.ink;
    v.window_fill = p.ink;
    v.faint_bg_color = p.ink_soft; // striped grid rows
    v.extreme_bg_color = p.extreme;
    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.accent.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);

    if look == Look::Modern {
        // Elevated surfaces derived from the palette's near-black ink.
        let surface = lighten(p.ink, 0.06); // card fill
        let raised = lighten(p.ink, 0.11); // buttons/fields at rest
        let hover = lighten(p.ink, 0.17);
        let border = lighten(p.ink, 0.22);
        let shadow = Color32::from_black_alpha(120);
        let cr = egui::CornerRadius::same(9);

        v.window_fill = surface;
        v.window_corner_radius = egui::CornerRadius::same(14);
        v.window_stroke = Stroke::new(1.0_f32, border);
        v.menu_corner_radius = egui::CornerRadius::same(12);
        v.faint_bg_color = raised; // striped rows a touch lighter on the card
        v.window_shadow = egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: shadow,
        };
        v.popup_shadow = egui::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: shadow,
        };

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = surface;
        w.noninteractive.weak_bg_fill = surface;
        w.noninteractive.bg_stroke = Stroke::new(1.0_f32, border);
        w.noninteractive.corner_radius = cr;

        w.inactive.bg_fill = raised;
        w.inactive.weak_bg_fill = raised;
        w.inactive.bg_stroke = Stroke::new(1.0_f32, border);
        w.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
        w.inactive.corner_radius = cr;

        w.hovered.bg_fill = hover;
        w.hovered.weak_bg_fill = hover;
        w.hovered.bg_stroke = Stroke::new(1.3_f32, p.accent);
        w.hovered.fg_stroke = Stroke::new(1.0_f32, p.text);
        w.hovered.corner_radius = cr;
        w.hovered.expansion = 1.0;

        let pressed = mix(p.ink, p.accent, 0.30);
        w.active.bg_fill = pressed;
        w.active.weak_bg_fill = pressed;
        w.active.bg_stroke = Stroke::new(1.5_f32, p.accent);
        w.active.fg_stroke = Stroke::new(1.0_f32, p.text);
        w.active.corner_radius = cr;
        w.active.expansion = 1.0;

        w.open.bg_fill = raised;
        w.open.weak_bg_fill = raised;
        w.open.bg_stroke = Stroke::new(1.0_f32, border);
        w.open.corner_radius = cr;

        // Roomier component spacing.
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.window_margin = egui::Margin::same(12);
        style.spacing.interact_size.y = 30.0;
        style.spacing.indent = 22.0;
    }

    style.visuals = v;
    ctx.set_global_style(style);
}

/// Wrap `add` in an elevated rounded card when the Modern look is active; in
/// Classic it's a no-op passthrough (keeps the original flat layout).
pub fn card<R>(ui: &mut egui::Ui, look: Look, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    if look == Look::Classic {
        return add(ui);
    }
    let (fill, stroke, shadow) = {
        let v = ui.visuals();
        (v.window_fill, v.window_stroke, v.window_shadow)
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(egui::Margin::same(16))
        .shadow(shadow)
        .show(ui, add)
        .inner
}

/// The floating look-switcher: an accent pill anchored bottom-right, labelled
/// "Switch to Classic/Modern" with a matching glyph. Returns `true` when clicked.
pub fn fab(ctx: &egui::Context, look: Look, accent: Color32) -> bool {
    let mut clicked = false;
    egui::Area::new(egui::Id::new("look_fab"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-22.0, -22.0))
        .show(ctx, |ui| {
            let target = look.other();
            // Dark text/glyph reads well on every bright accent.
            let galley = ui.painter().layout_no_wrap(
                format!("Switch to {}", target.label()),
                egui::FontId::proportional(14.5),
                OUTLINE,
            );
            let pad = Vec2::new(16.0, 11.0);
            let glyph_w = 20.0;
            let gap = 8.0;
            let size = Vec2::new(
                pad.x * 2.0 + glyph_w + gap + galley.size().x,
                galley.size().y.max(18.0) + pad.y * 2.0,
            );
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            let p = ui.painter();
            let cr = egui::CornerRadius::same((rect.height() * 0.5) as u8);
            // Soft drop shadow.
            p.rect_filled(
                rect.translate(Vec2::new(0.0, 3.0)),
                cr,
                Color32::from_black_alpha(80),
            );
            // Body (brightens on hover).
            let body = if resp.hovered() {
                accent
            } else {
                accent.linear_multiply(0.88)
            };
            p.rect_filled(rect, cr, body);
            p.rect_stroke(
                rect,
                cr,
                Stroke::new(2.0_f32, OUTLINE),
                egui::StrokeKind::Inside,
            );
            // Glyph, then label.
            let gc = Pos2::new(rect.left() + pad.x + glyph_w * 0.5, rect.center().y);
            fab_glyph(p, gc, 30.0, target);
            let text_pos = Pos2::new(
                rect.left() + pad.x + glyph_w + gap,
                rect.center().y - galley.size().y * 0.5,
            );
            p.galley(text_pos, galley, OUTLINE);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            clicked = resp.clicked();
        });
    clicked
}

/// The glyph inside the FAB: a sparkle when it will switch *to* Modern, or a
/// plain 3-bar list when it will switch *to* Classic.
fn fab_glyph(p: &egui::Painter, c: Pos2, d: f32, target: Look) {
    match target {
        Look::Modern => {
            let r = d * 0.2;
            let r2 = r * 0.62;
            let s = Stroke::new(d * 0.06, OUTLINE);
            p.line_segment([Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)], s);
            p.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], s);
            p.line_segment(
                [Pos2::new(c.x - r2, c.y - r2), Pos2::new(c.x + r2, c.y + r2)],
                s,
            );
            p.line_segment(
                [Pos2::new(c.x - r2, c.y + r2), Pos2::new(c.x + r2, c.y - r2)],
                s,
            );
        }
        Look::Classic => {
            let w = d * 0.22;
            for i in 0..3 {
                let y = c.y - d * 0.12 + i as f32 * d * 0.12;
                p.line_segment(
                    [Pos2::new(c.x - w, y), Pos2::new(c.x + w, y)],
                    Stroke::new(d * 0.05, OUTLINE),
                );
            }
        }
    }
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
    p.add(Shape::convex_polygon(
        hex,
        accent,
        Stroke::new(size * 0.11, OUTLINE),
    ));
    let d = size * 0.20;
    let diamond = vec![
        Pos2::new(c.x, c.y - d),
        Pos2::new(c.x + d, c.y),
        Pos2::new(c.x, c.y + d),
        Pos2::new(c.x - d, c.y),
    ];
    p.add(Shape::convex_polygon(
        diamond,
        OUTLINE,
        Stroke::new(1.0_f32, OUTLINE),
    ));
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
    p.add(Shape::convex_polygon(
        gem,
        GEM,
        Stroke::new(size * 0.11, OUTLINE),
    ));
}

const SERAPH_RED: Color32 = Color32::from_rgb(0xE0, 0x3A, 0x55); // crimson
const TORGUE_ORANGE: Color32 = Color32::from_rgb(0xF0, 0x7A, 0x1E);

/// A crimson crystal glyph for seraph crystals.
pub fn seraph(ui: &mut egui::Ui, size: f32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.46;
    let gem = vec![
        Pos2::new(c.x, c.y - r),
        Pos2::new(c.x + r * 0.55, c.y - r * 0.15),
        Pos2::new(c.x, c.y + r),
        Pos2::new(c.x - r * 0.55, c.y - r * 0.15),
    ];
    p.add(Shape::convex_polygon(
        gem,
        SERAPH_RED,
        Stroke::new(size * 0.11, OUTLINE),
    ));
    p.line_segment(
        [Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)],
        Stroke::new(size * 0.06, OUTLINE),
    );
}

/// An orange token glyph for torgue tokens.
pub fn torgue(ui: &mut egui::Ui, size: f32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.45;
    p.circle(c, r, TORGUE_ORANGE, Stroke::new(size * 0.11, OUTLINE));
    p.circle_stroke(c, r * 0.5, Stroke::new(size * 0.10, OUTLINE));
}

/// A head/shoulders bust glyph for the Character tab (uses the given color).
pub fn head(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    p.circle(
        Pos2::new(c.x, c.y - size * 0.16),
        size * 0.20,
        color,
        Stroke::new(size * 0.09, OUTLINE),
    );
    let w = size * 0.36;
    let shoulders = vec![
        Pos2::new(c.x - w, c.y + size * 0.42),
        Pos2::new(c.x - w * 0.55, c.y + size * 0.10),
        Pos2::new(c.x + w * 0.55, c.y + size * 0.10),
        Pos2::new(c.x + w, c.y + size * 0.42),
    ];
    p.add(Shape::convex_polygon(
        shoulders,
        color,
        Stroke::new(size * 0.09, OUTLINE),
    ));
}

/// A wheel glyph for the Vehicle tab.
pub fn wheel(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.44;
    p.circle(c, r, color, Stroke::new(size * 0.10, OUTLINE));
    p.circle(c, r * 0.34, OUTLINE, Stroke::new(size * 0.06, OUTLINE));
    for i in 0..4 {
        let a = std::f32::consts::FRAC_PI_2 * i as f32 + std::f32::consts::FRAC_PI_4;
        p.line_segment(
            [
                Pos2::new(c.x + r * 0.34 * a.cos(), c.y + r * 0.34 * a.sin()),
                Pos2::new(c.x + r * 0.9 * a.cos(), c.y + r * 0.9 * a.sin()),
            ],
            Stroke::new(size * 0.07, OUTLINE),
        );
    }
}

/// A flag-on-pole glyph for the General/progression tab.
pub fn flag(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let x = c.x - size * 0.22;
    p.line_segment(
        [
            Pos2::new(x, c.y - size * 0.40),
            Pos2::new(x, c.y + size * 0.44),
        ],
        Stroke::new(size * 0.10, OUTLINE),
    );
    let pennant = vec![
        Pos2::new(x, c.y - size * 0.40),
        Pos2::new(x + size * 0.46, c.y - size * 0.24),
        Pos2::new(x, c.y - size * 0.08),
    ];
    p.add(Shape::convex_polygon(
        pennant,
        color,
        Stroke::new(size * 0.09, OUTLINE),
    ));
}

/// A signpost glyph for the Fast Travel tab (post + arrow sign).
pub fn signpost(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    // Post.
    p.line_segment(
        [
            Pos2::new(c.x, c.y - size * 0.30),
            Pos2::new(c.x, c.y + size * 0.44),
        ],
        Stroke::new(size * 0.10, OUTLINE),
    );
    // Arrow sign pointing right.
    let (t, b, l) = (c.y - size * 0.34, c.y - size * 0.06, c.x - size * 0.30);
    let sign = vec![
        Pos2::new(l, t),
        Pos2::new(c.x + size * 0.20, t),
        Pos2::new(c.x + size * 0.40, (t + b) * 0.5),
        Pos2::new(c.x + size * 0.20, b),
        Pos2::new(l, b),
    ];
    p.add(Shape::convex_polygon(
        sign,
        color,
        Stroke::new(size * 0.09, OUTLINE),
    ));
}

/// A document/list glyph for the Raw inspector tab.
pub fn page(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let (w, h) = (size * 0.28, size * 0.38);
    let page = vec![
        Pos2::new(c.x - w, c.y - h),
        Pos2::new(c.x + w, c.y - h),
        Pos2::new(c.x + w, c.y + h),
        Pos2::new(c.x - w, c.y + h),
    ];
    p.add(Shape::convex_polygon(
        page,
        color,
        Stroke::new(size * 0.09, OUTLINE),
    ));
    for i in 0..3 {
        let y = c.y - h * 0.4 + i as f32 * h * 0.42;
        p.line_segment(
            [Pos2::new(c.x - w * 0.55, y), Pos2::new(c.x + w * 0.55, y)],
            Stroke::new(size * 0.06, OUTLINE),
        );
    }
}

/// An info "i" glyph for the About tab.
pub fn info(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let r = size * 0.42;
    p.circle(c, r, color, Stroke::new(size * 0.10, OUTLINE));
    p.circle_filled(Pos2::new(c.x, c.y - r * 0.42), size * 0.06, OUTLINE);
    p.line_segment(
        [
            Pos2::new(c.x, c.y - r * 0.08),
            Pos2::new(c.x, c.y + r * 0.45),
        ],
        Stroke::new(size * 0.12, OUTLINE),
    );
}

/// A loot-crate glyph for the Items tab (uses the given color).
pub fn crate_icon(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, p) = alloc(ui, size);
    let c = rect.center();
    let h = size * 0.34;
    let square = vec![
        Pos2::new(c.x - h, c.y - h),
        Pos2::new(c.x + h, c.y - h),
        Pos2::new(c.x + h, c.y + h),
        Pos2::new(c.x - h, c.y + h),
    ];
    p.add(Shape::convex_polygon(
        square,
        color,
        Stroke::new(size * 0.10, OUTLINE),
    ));
    let d = h * 0.62;
    let diamond = vec![
        Pos2::new(c.x, c.y - d),
        Pos2::new(c.x + d, c.y),
        Pos2::new(c.x, c.y + d),
        Pos2::new(c.x - d, c.y),
    ];
    p.add(Shape::convex_polygon(
        diamond,
        OUTLINE,
        Stroke::new(1.0_f32, OUTLINE),
    ));
}
