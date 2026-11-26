//! App chrome (layer one) palette, typography and reusable widgets.
//! Board visuals (layer two) come pre-rendered from hub-core and are never restyled.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, RichText, Sense, Shadow, Stroke, Ui, Vec2};

// ---------------------------------------------------------------- fixa tokens (see DESIGN.md)
// Brand
pub const FIXER: Color32 = Color32::from_rgb(0x1C, 0x7C, 0x54); // primary emerald
pub const FOREST: Color32 = Color32::from_rgb(0x14, 0x5A, 0x3D); // emerald scale 1 (deep)
pub const SAGE: Color32 = Color32::from_rgb(0x6B, 0xA6, 0x8B); // emerald scale 3 (text/links on Darkroom)
pub const MINT: Color32 = Color32::from_rgb(0xC5, 0xDF, 0xD2); // emerald scale 4 (pale)
// Print (light)
pub const PRINT: Color32 = Color32::from_rgb(0xF5, 0xF3, 0xEE); // surface
pub const PRINT_CARD: Color32 = Color32::from_rgb(0xE9, 0xEF, 0xEA); // surfaceContainer
pub const PRINT_CARD_HI: Color32 = Color32::from_rgb(0xDC, 0xE7, 0xDF); // surfaceContainerHigh
pub const PRINT_MUTED: Color32 = Color32::from_rgb(0x4F, 0x5F, 0x56); // onSurfaceVariant (6.1:1 on Print)
pub const PRINT_LINE: Color32 = Color32::from_rgb(0xC9, 0xD6, 0xCD); // outlineVariant
// Darkroom (dark)
pub const INK: Color32 = Color32::from_rgb(0x0B, 0x15, 0x10); // surface / ink
pub const ROOM_CARD: Color32 = Color32::from_rgb(0x12, 0x21, 0x1A); // surfaceContainer
pub const ROOM_CARD_HI: Color32 = Color32::from_rgb(0x1A, 0x2E, 0x24); // surfaceContainerHigh
pub const ROOM_MUTED: Color32 = Color32::from_rgb(0xA3, 0xB3, 0xAA); // onSurfaceVariant (8.5:1 on Ink)
pub const ROOM_LINE: Color32 = Color32::from_rgb(0x24, 0x38, 0x2D); // outlineVariant
// Feedback
pub const ERROR_PRINT: Color32 = Color32::from_rgb(0xB3, 0x26, 0x1E);
pub const ERROR_ROOM: Color32 = Color32::from_rgb(0xF2, 0xB8, 0xB5);

// Role aliases used across the screens (kept so every screen reads from the same tokens)
pub const PRIMARY: Color32 = FIXER;
pub const PRIMARY_VARIANT: Color32 = FOREST;
pub const PRIMARY_ON_DARK: Color32 = SAGE;
pub const SECONDARY: Color32 = SAGE;
pub const SECONDARY_VARIANT: Color32 = FIXER;
pub const ERROR: Color32 = ERROR_PRINT;
pub const WHITE: Color32 = PRINT;
pub const BLACK: Color32 = INK;
pub const TEXT2: Color32 = PRINT_MUTED;
pub const DIVIDER: Color32 = PRINT_LINE;
pub const DARK_BASE: Color32 = INK;
pub const BOARD_BG: Color32 = INK;
pub const BOARD_BG2: Color32 = Color32::from_rgb(0x0F, 0x1C, 0x15);
pub const DARK_CARD: Color32 = ROOM_CARD;
pub const DARK_TEXT2: Color32 = ROOM_MUTED;
pub const DARK_LINE: Color32 = ROOM_LINE;
/// Darkroom scrim for tooltips and toasts.
pub const SCRIM: Color32 = Color32::from_rgba_premultiplied(10, 19, 14, 240);

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
    // app type: Schibsted Grotesk static instances with tabular figures frozen into the digits
    add(&mut fonts, "Schibsted Regular", include_bytes!("../assets/fonts/SchibstedGrotesk-Regular.ttf"));
    add(&mut fonts, "Schibsted Medium", include_bytes!("../assets/fonts/SchibstedGrotesk-Medium.ttf"));
    add(&mut fonts, "Schibsted SemiBold", include_bytes!("../assets/fonts/SchibstedGrotesk-SemiBold.ttf"));
    let fb: Vec<String> = ["egui-icons", "NotoEmoji-Regular", "emoji-icon-font"].iter().filter(|k| fonts.font_data.contains_key(**k)).map(|s| s.to_string()).collect();
    let fam = |v: &[&str]| v.iter().map(|s| s.to_string()).chain(fb.clone()).collect::<Vec<String>>();
    fonts.families.insert(FontFamily::Name(CONDENSED.into()), fam(&["Schibsted SemiBold", "Inter SemiBold"]));
    fonts.families.insert(FontFamily::Name("bold".into()), fam(&["Schibsted SemiBold", "Inter SemiBold"]));
    fonts.families.insert(FontFamily::Name("semibold".into()), fam(&["Schibsted Medium", "Inter Medium"]));
    // boards keep the faces their layouts were measured with (Inter / Barlow Condensed)
    fonts.families.insert(FontFamily::Name("b_head".into()), fam(&["Barlow Condensed Bold", "Inter Bold"]));
    fonts.families.insert(FontFamily::Name("b_bold".into()), fam(&["Inter Bold", "Inter SemiBold"]));
    fonts.families.insert(FontFamily::Name("b_semi".into()), fam(&["Inter SemiBold", "Inter Bold"]));
    fonts.families.insert(FontFamily::Name("b_body".into()), fam(&["Inter Regular"]));
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "Schibsted Regular".into());
    // symbols Schibsted lacks (▲ ▼ ■) fall back to Inter before the emoji fonts
    prop.insert(1, "Inter Regular".into());
    ctx.set_fonts(fonts);
}

static TEXT_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);
/// In-app text size (percent). Android's NativeActivity does not expose the system font scale,
/// so the setting lives in the app; every font helper below applies it.
pub fn set_text_scale(pct: u32) {
    TEXT_SCALE.store(pct.clamp(100, 160), std::sync::atomic::Ordering::Relaxed);
}
/// Current text size in percent.
pub fn text_scale_pct() -> u32 {
    TEXT_SCALE.load(std::sync::atomic::Ordering::Relaxed)
}
fn ts() -> f32 {
    TEXT_SCALE.load(std::sync::atomic::Ordering::Relaxed) as f32 / 100.0
}

/// Shared surface tokens (dark / violet look).
pub const ERROR_ON_DARK: Color32 = ERROR_ROOM;

/// Board type: unscaled, in the faces the board generator measured.
pub fn board_font(size: f32, hd: bool, weight: u16) -> FontId {
    let fam = if hd { "b_head" } else if weight >= 700 { "b_bold" } else if weight >= 600 { "b_semi" } else { "b_body" };
    FontId::new(size, FontFamily::Name(fam.into()))
}

pub fn head(size: f32) -> FontId {
    let size = size * ts();
    FontId::new(size, FontFamily::Name(CONDENSED.into()))
}
pub fn bold(size: f32) -> FontId {
    let size = size * ts();
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn semi(size: f32) -> FontId {
    let size = size * ts();
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub fn body(size: f32) -> FontId {
    let size = size * ts();
    FontId::new(size, FontFamily::Proportional)
}

pub fn apply_visuals(ctx: &egui::Context, dark: bool) {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = if dark { INK } else { PRINT };
    v.window_fill = v.panel_fill;
    v.extreme_bg_color = if dark { ROOM_CARD } else { PRINT_CARD };
    v.widgets.inactive.corner_radius = CornerRadius::same(24);
    v.widgets.hovered.corner_radius = CornerRadius::same(24);
    v.widgets.active.corner_radius = CornerRadius::same(24);
    v.selection.bg_fill = PRIMARY;
    v.selection.stroke = Stroke::new(1.0, if dark { PRIMARY_ON_DARK } else { PRIMARY });
    v.hyperlink_color = if dark { PRIMARY_ON_DARK } else { PRIMARY };
    v.override_text_color = Some(if dark { WHITE } else { BLACK });
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, if dark { DARK_LINE } else { DIVIDER });
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.interact_size.y = 48.0; // Android minimum touch target (48dp)
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
        .fill(if dark { ROOM_CARD } else { PRINT_CARD })
        .corner_radius(CornerRadius::same(20))
        .inner_margin(Margin::same(16))
        .shadow(Shadow::NONE)
        .stroke(Stroke::NONE)
}

/// Glass card for dark surfaces: translucent fill, hairline light border.
pub fn glass() -> Frame {
    Frame::new()
        .fill(ROOM_CARD)
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(12))
        .stroke(Stroke::NONE)
}

/// Pressed-state feedback for custom tappable surfaces (no layout shift).
pub fn press_overlay(ui: &Ui, r: &egui::Response, radius: f32, dark: bool) {
    if r.is_pointer_button_down_on() {
        ui.painter().rect_filled(r.rect, radius, if dark { Color32::from_rgba_unmultiplied(255, 255, 255, 22) } else { Color32::from_rgba_unmultiplied(28, 124, 84, 26) });
    } else if r.hovered() {
        ui.painter().rect_stroke(r.rect, radius, Stroke::new(1.0, if dark { Color32::from_rgba_unmultiplied(107, 166, 139, 110) } else { Color32::from_rgba_unmultiplied(28, 124, 84, 80) }), egui::StrokeKind::Inside);
    }
}

/// Primary CTA button.
pub fn cta(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let (fill, text) = if enabled { (FIXER, PRINT) } else if lt() { (ROOM_CARD_HI, ROOM_MUTED) } else { (PRINT_CARD_HI, PRINT_MUTED) };
    let btn = egui::Button::new(RichText::new(label).font(semi(16.0)).color(text)).fill(fill).corner_radius(CornerRadius::same(24)).min_size(Vec2::new(0.0, 48.0));
    ui.add_enabled(enabled, btn).clicked()
}

pub fn ghost(ui: &mut Ui, label: &str, dark: bool) -> bool {
    // crisp 1dp outlined pill in ink (Print) or paper (Darkroom)
    let c = if dark { PRINT } else { INK };
    let btn = egui::Button::new(RichText::new(label).font(semi(14.0)).color(c)).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, c)).corner_radius(CornerRadius::same(24)).min_size(Vec2::new(0.0, 48.0));
    ui.add(btn).clicked()
}

/// Selectable chip; active chips use the primary purple.
pub fn chip(ui: &mut Ui, label: &str, active: bool, dark: bool) -> bool {
    let (fill, text, stroke) = if active {
        (FIXER, PRINT, Stroke::NONE)
    } else if dark {
        (Color32::TRANSPARENT, ROOM_MUTED, Stroke::new(1.0, ROOM_LINE))
    } else {
        (Color32::TRANSPARENT, PRINT_MUTED, Stroke::new(1.0, PRINT_LINE))
    };
    let btn = egui::Button::new(RichText::new(label).font(semi(12.5)).color(text)).fill(fill).stroke(stroke).corner_radius(CornerRadius::same(18)).min_size(Vec2::new(0.0, 48.0));
    ui.add(btn).clicked()
}

/// Static metric chip, e.g. "npxG 2.51 – 1.30".
pub fn metric_chip(ui: &mut Ui, label: &str, value: &str, dark: bool) {
    Frame::new()
        .fill(if dark { ROOM_CARD_HI } else { PRINT_CARD_HI })
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(RichText::new(label).font(semi(11.0)).color(if dark { DARK_TEXT2 } else { TEXT2 }));
                ui.label(RichText::new(value).font(head(16.0)).color(if dark { PRINT } else { INK }).extra_letter_spacing(-0.3));
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
        let r = back_button(ui, c);
        if r.clicked() {
            back = true;
        }
        // long titles truncate instead of widening the screen
        let w = ui.available_width();
        ui.allocate_ui_with_layout(Vec2::new(w, 48.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_max_width(w);
            ui.add(egui::Label::new(RichText::new(title).font(head(24.0)).color(c)).truncate());
        });
    });
    back
}

/// Material-style Up arrow drawn as a vector icon (one icon style across the app), 48dp target.
pub fn back_button(ui: &mut Ui, c: Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::click());
    let k = r.center();
    let st = Stroke::new(2.4, c);
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.circle_filled(k, 22.0, Color32::from_rgba_unmultiplied(255, 255, 255, 24));
    }
    p.line_segment([egui::pos2(k.x - 8.0, k.y), egui::pos2(k.x + 9.0, k.y)], st);
    p.line_segment([egui::pos2(k.x - 8.0, k.y), egui::pos2(k.x - 1.0, k.y - 7.0)], st);
    p.line_segment([egui::pos2(k.x - 8.0, k.y), egui::pos2(k.x - 1.0, k.y + 7.0)], st);
    if resp.has_focus() {
        p.circle_stroke(k, 22.0, Stroke::new(2.0, SECONDARY));
    }
    resp.on_hover_text("Back")
}

pub fn section_title(ui: &mut Ui, t: &str, dark: bool) {
    ui.add_space(24.0);
    ui.label(RichText::new(t).font(semi(15.0)).color(if dark { DARK_TEXT2 } else { TEXT2 }));
    ui.add_space(8.0);
}

/// Screen backdrop for dark surfaces: flat `INK` with the halftone motif top right (no gradient, no glow).
pub fn backdrop(ui: &Ui, rect: egui::Rect) {
    ui.painter().rect_filled(rect, 0.0, INK);
    halftone(ui, egui::pos2(rect.right() - 30.0, rect.top() + 70.0), 170.0, FIXER, 0.55);
}

/// Halftone / dot-matrix disc: the fixa signature motif. Dots shrink toward the edge.
pub fn halftone(ui: &Ui, c: egui::Pos2, radius: f32, col: Color32, strength: f32) {
    let p = ui.painter();
    let step = 9.0;
    let n = (radius / step) as i32;
    for iy in -n..=n {
        for ix in -n..=n {
            let x = ix as f32 * step + if iy % 2 == 0 { 0.0 } else { step / 2.0 };
            let y = iy as f32 * step * 0.87;
            let d = (x * x + y * y).sqrt() / radius;
            if d >= 1.0 {
                continue;
            }
            let r = (1.0 - d) * step * 0.42;
            if r < 0.5 {
                continue;
            }
            let a = ((1.0 - d) * strength * 255.0) as u8;
            p.circle_filled(egui::pos2(c.x + x, c.y + y), r, Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), a));
        }
    }
}

/// Underlined text tab row; returns the index tapped.
pub fn tab_row(ui: &mut Ui, labels: &[&str], active: usize) -> Option<usize> {
    let mut hit = None;
    // scrollable tab row: never wider than the screen at large text sizes
    egui::ScrollArea::horizontal().auto_shrink([false, true]).id_salt(("tabs", labels.len())).show(ui, |ui| ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 22.0;
        // at normal text size the tabs share the width evenly, so every label fits; larger text scrolls
        let even = text_scale_pct() <= 125;
        let cell = if even { ((ui.available_width() - 22.0 * (labels.len() as f32 - 1.0)) / labels.len() as f32).max(40.0) } else { 0.0 };
        for (i, l) in labels.iter().enumerate() {
            let on = i == active;
            let col = if on { PRINT } else { ROOM_MUTED };
            let r = ui.add(egui::Button::new(RichText::new(*l).font(bold(15.0)).color(col)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).min_size(Vec2::new(cell, 48.0)));
            if on {
                let y = r.rect.bottom() - 4.0;
                ui.painter().line_segment([egui::pos2(r.rect.left(), y), egui::pos2(r.rect.right(), y)], Stroke::new(3.0, SAGE));
            }
            if r.clicked() {
                hit = Some(i);
            }
        }
    }));
    hit
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
