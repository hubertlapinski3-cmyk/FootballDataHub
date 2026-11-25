//! App chrome (layer one) palette, typography and reusable widgets.
//! Board visuals (layer two) come pre-rendered from hub-core and are never restyled.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, RichText, Sense, Shadow, Stroke, Ui, Vec2};

pub const PRIMARY: Color32 = Color32::from_rgb(0x62, 0x00, 0xEE);
pub const PRIMARY_VARIANT: Color32 = Color32::from_rgb(0x37, 0x00, 0xB3);
pub const PRIMARY_ON_DARK: Color32 = Color32::from_rgb(0xBB, 0x86, 0xFF);
pub const SECONDARY: Color32 = Color32::from_rgb(0x03, 0xDA, 0xC6);
pub const SECONDARY_VARIANT: Color32 = Color32::from_rgb(0x01, 0x87, 0x86);
pub const ERROR: Color32 = Color32::from_rgb(0xB0, 0x00, 0x20);
pub const WHITE: Color32 = Color32::WHITE;
pub const BLACK: Color32 = Color32::BLACK;
pub const TEXT2: Color32 = Color32::from_rgb(0x75, 0x75, 0x75);
pub const DIVIDER: Color32 = Color32::from_rgb(0xE0, 0xE0, 0xE0);
pub const DARK_BASE: Color32 = Color32::from_rgb(0x12, 0x12, 0x12);
pub const BOARD_BG: Color32 = Color32::from_rgb(0x0b, 0x0a, 0x12);
pub const BOARD_BG2: Color32 = Color32::from_rgb(0x14, 0x0c, 0x18);
pub const DARK_CARD: Color32 = Color32::from_rgb(0x1b, 0x15, 0x26);
pub const DARK_TEXT2: Color32 = Color32::from_rgb(0xb8, 0xb8, 0xc0);
pub const DARK_LINE: Color32 = Color32::from_rgb(0x2a, 0x26, 0x33);

static FORCE_DARK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_force_dark(v: bool) {
    FORCE_DARK.store(v, std::sync::atomic::Ordering::Relaxed);
}
/// True when the user's theme forces dark chrome on screens that are light by default.
pub fn lt() -> bool {
    FORCE_DARK.load(std::sync::atomic::Ordering::Relaxed)
}
/// Primary text colour for the default-light screens.
pub fn fg() -> Color32 {
    if lt() { WHITE } else { BLACK }
}
/// Secondary text colour for the default-light screens.
pub fn sub() -> Color32 {
    if lt() { DARK_TEXT2 } else { TEXT2 }
}

pub const CONDENSED: &str = "condensed";

pub fn hex(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(s.get(i..i + 2).unwrap_or("80"), 16).unwrap_or(128);
    Color32::from_rgb(p(0), p(2), p(4))
}

pub fn luminance(c: Color32) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

/// Text colour for a fill: never white on teal/light fills.
pub fn on(c: Color32) -> Color32 {
    if luminance(c) > 0.35 { BLACK } else { WHITE }
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let add = |fonts: &mut FontDefinitions, name: &str, data: &'static [u8]| {
        fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(FontData::from_static(data)));
    };
    for (data, name) in matchhub::fonts::FONTS {
        add(&mut fonts, name, data);
    }
    let fb: Vec<String> = ["egui-icons", "NotoEmoji-Regular", "emoji-icon-font"].iter().filter(|k| fonts.font_data.contains_key(**k)).map(|s| s.to_string()).collect();
    let fam = |v: &[&str]| v.iter().map(|s| s.to_string()).chain(fb.clone()).collect::<Vec<String>>();
    fonts.families.insert(FontFamily::Name(CONDENSED.into()), fam(&["Barlow Condensed Bold", "Inter Bold"]));
    fonts.families.insert(FontFamily::Name("bold".into()), fam(&["Inter Bold", "Inter SemiBold"]));
    fonts.families.insert(FontFamily::Name("semibold".into()), fam(&["Inter SemiBold", "Inter Bold"]));
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "Inter Regular".into());
    ctx.set_fonts(fonts);
}

pub fn head(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(CONDENSED.into()))
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn semi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn apply_visuals(ctx: &egui::Context, dark: bool) {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = if dark { BOARD_BG } else { WHITE };
    v.window_fill = v.panel_fill;
    v.extreme_bg_color = if dark { DARK_CARD } else { Color32::from_rgb(0xF4, 0xF2, 0xF8) };
    v.selection.bg_fill = PRIMARY;
    v.selection.stroke = Stroke::new(1.0, if dark { PRIMARY_ON_DARK } else { PRIMARY });
    v.hyperlink_color = if dark { PRIMARY_ON_DARK } else { PRIMARY };
    v.override_text_color = Some(if dark { WHITE } else { BLACK });
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, if dark { DARK_LINE } else { DIVIDER });
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.interact_size.y = 44.0; // touch targets
        s.spacing.button_padding = Vec2::new(16.0, 10.0);
        s.spacing.item_spacing = Vec2::new(10.0, 10.0);
        s.text_styles.insert(egui::TextStyle::Body, body(15.0));
        s.text_styles.insert(egui::TextStyle::Button, semi(15.0));
        s.text_styles.insert(egui::TextStyle::Small, body(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, head(28.0));
    });
}

pub fn card(dark: bool) -> Frame {
    Frame::new()
        .fill(if dark { Color32::from_rgba_unmultiplied(255, 255, 255, 13) } else { WHITE })
        .corner_radius(CornerRadius::same(20))
        .inner_margin(Margin::same(16))
        .shadow(if dark { Shadow::NONE } else { Shadow { offset: [0, 3], blur: 14, spread: 0, color: Color32::from_black_alpha(38) } })
        .stroke(if dark { Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 28)) } else { Stroke::NONE })
}

/// Glass card for dark surfaces: translucent fill, hairline light border.
pub fn glass() -> Frame {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 14))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(12))
        .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 30)))
}

/// Pressed-state feedback for custom tappable surfaces (no layout shift).
pub fn press_overlay(ui: &Ui, r: &egui::Response, radius: f32, dark: bool) {
    if r.is_pointer_button_down_on() {
        ui.painter().rect_filled(r.rect, radius, if dark { Color32::from_rgba_unmultiplied(255, 255, 255, 22) } else { Color32::from_rgba_unmultiplied(98, 0, 238, 20) });
    } else if r.hovered() {
        ui.painter().rect_stroke(r.rect, radius, Stroke::new(1.0, if dark { Color32::from_rgba_unmultiplied(187, 134, 255, 90) } else { Color32::from_rgba_unmultiplied(98, 0, 238, 60) }), egui::StrokeKind::Inside);
    }
}

/// Primary CTA button.
pub fn cta(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let btn = egui::Button::new(RichText::new(label).font(semi(16.0)).color(WHITE)).fill(if enabled { PRIMARY } else { Color32::from_gray(160) }).corner_radius(CornerRadius::same(24)).min_size(Vec2::new(0.0, 48.0));
    ui.add_enabled(enabled, btn).clicked()
}

pub fn ghost(ui: &mut Ui, label: &str, dark: bool) -> bool {
    let c = if dark { PRIMARY_ON_DARK } else { PRIMARY };
    let btn = egui::Button::new(RichText::new(label).font(semi(14.0)).color(c)).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.5, c)).corner_radius(CornerRadius::same(22)).min_size(Vec2::new(0.0, 44.0));
    ui.add(btn).clicked()
}

/// Selectable chip; active chips use the primary purple.
pub fn chip(ui: &mut Ui, label: &str, active: bool, dark: bool) -> bool {
    let (fill, text, stroke) = if active {
        (PRIMARY, WHITE, Stroke::NONE)
    } else if dark {
        (DARK_CARD, DARK_TEXT2, Stroke::new(1.0, DARK_LINE))
    } else {
        (Color32::from_rgb(0xF1, 0xEC, 0xFB), PRIMARY_VARIANT, Stroke::NONE)
    };
    let btn = egui::Button::new(RichText::new(label).font(semi(12.5)).color(text)).fill(fill).stroke(stroke).corner_radius(CornerRadius::same(18)).min_size(Vec2::new(0.0, 36.0));
    ui.add(btn).clicked()
}

/// Static metric chip, e.g. "npxG 2.51 – 1.30".
pub fn metric_chip(ui: &mut Ui, label: &str, value: &str, dark: bool) {
    Frame::new()
        .fill(if dark { DARK_CARD } else { Color32::from_rgb(0xF1, 0xEC, 0xFB) })
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(RichText::new(label).font(semi(11.0)).color(if dark { DARK_TEXT2 } else { TEXT2 }));
                ui.label(RichText::new(value).font(head(16.0)).color(if dark { WHITE } else { PRIMARY_VARIANT }));
            });
        });
}

/// Shared definition chip: a reconciled definition label shown next to a number.
pub fn definition_chip(ui: &mut Ui, text: &str, dark: bool) {
    Frame::new()
        .stroke(Stroke::new(1.0, if dark { DARK_LINE } else { DIVIDER }))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(format!("Definition: {text}")).font(body(12.0)).color(if dark { DARK_TEXT2 } else { TEXT2 }));
        });
}

/// Top bar with a back arrow; returns true when back was pressed.
pub fn top_bar(ui: &mut Ui, title: &str, dark: bool) -> bool {
    let mut back = false;
    ui.horizontal(|ui| {
        ui.set_min_height(52.0);
        let c = if dark { WHITE } else { BLACK };
        let r = ui.add(egui::Button::new(RichText::new("←").font(bold(22.0)).color(c)).fill(Color32::TRANSPARENT).min_size(Vec2::splat(44.0)));
        if r.clicked() {
            back = true;
        }
        ui.label(RichText::new(title).font(head(24.0)).color(c));
    });
    back
}

pub fn section_title(ui: &mut Ui, t: &str, dark: bool) {
    ui.add_space(8.0);
    ui.label(RichText::new(t.to_uppercase()).font(bold(12.5)).color(if dark { DARK_TEXT2 } else { TEXT2 }));
}

/// Screen backdrop for dark surfaces: deep navy fading through indigo, with a soft teal glow low-right.
pub fn backdrop(ui: &Ui, rect: egui::Rect) {
    let top = Color32::from_rgb(0x1d, 0x14, 0x4a);
    let mid = Color32::from_rgb(0x0f, 0x0e, 0x24);
    let bot = Color32::from_rgb(0x07, 0x09, 0x14);
    let h = rect.height();
    let a = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + h * 0.45));
    let b = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + h * 0.45), rect.max);
    gradient(ui, a, top, mid);
    gradient(ui, b, mid, bot);
    let p = ui.painter();
    for (i, r) in [220.0f32, 160.0, 100.0].iter().enumerate() {
        p.circle_filled(egui::pos2(rect.right() - 20.0, rect.bottom() - 140.0), *r, Color32::from_rgba_unmultiplied(3, 218, 198, 6 + i as u8 * 2));
    }
    for (i, r) in [200.0f32, 140.0, 80.0].iter().enumerate() {
        p.circle_filled(egui::pos2(rect.left() + 10.0, rect.top() + 80.0), *r, Color32::from_rgba_unmultiplied(120, 70, 255, 8 + i as u8 * 3));
    }
}

/// Underlined text tab row; returns the index tapped.
pub fn tab_row(ui: &mut Ui, labels: &[&str], active: usize) -> Option<usize> {
    let mut hit = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 22.0;
        for (i, l) in labels.iter().enumerate() {
            let on = i == active;
            let col = if on { WHITE } else { DARK_TEXT2 };
            let r = ui.add(egui::Label::new(RichText::new(l.to_uppercase()).font(bold(15.0)).color(col)).sense(Sense::click()));
            if on {
                let y = r.rect.bottom() + 3.0;
                ui.painter().line_segment([egui::pos2(r.rect.left(), y), egui::pos2(r.rect.right(), y)], Stroke::new(3.0, SECONDARY));
            }
            if r.clicked() {
                hit = Some(i);
            }
        }
    });
    hit
}

/// Paint a vertical gradient (top -> bottom) into a rect.
pub fn gradient(ui: &Ui, rect: egui::Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// Monogram disc used when no crest image is available.
pub fn monogram(ui: &mut Ui, name: &str, col: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().circle_filled(rect.center(), size / 2.0, col);
    let ini: String = name.split_whitespace().filter(|w| w.len() > 2 || name.split_whitespace().count() == 1).take(2).filter_map(|w| w.chars().next()).collect::<String>().to_uppercase();
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, ini, head(size * 0.5), on(col));
}

/// Fit a team name by width: full -> short -> abbreviation -> 3-letter code. Never truncate mid-word.
pub fn fit_name(full: &str, short: &str, max_chars: usize) -> String {
    let up = full.to_uppercase();
    if up.chars().count() <= max_chars {
        return up;
    }
    let s = short.to_uppercase();
    if s.chars().count() <= max_chars {
        return s;
    }
    let words: Vec<&str> = full.split_whitespace().collect();
    if words.len() >= 2 {
        let ab = format!("{}. {}", &words[0][..1], words.last().unwrap()).to_uppercase();
        if ab.chars().count() <= max_chars {
            return ab;
        }
    }
    full.chars().filter(|c| c.is_alphabetic()).take(3).collect::<String>().to_uppercase()
}
