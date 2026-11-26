//! The eight screens of the Data Hub app.

pub mod browse;
mod viewer;

use crate::library::{self, Entry};
use crate::style::{self, *};
use crate::{App, Screen};
use egui::{Align, Color32, Layout, RichText, Sense, Stroke, Ui, Vec2};

pub const STEPS: [&str; 6] = ["ingest", "enrich", "model", "derive", "render", "report"];

impl App {
    pub fn team_col(&self, m: usize, side: usize) -> Color32 {
        hex(&self.lib.entries[m].s.colours[side])
    }

    /// Team colour adjusted for legibility on white surfaces (darkened if too light).
    pub fn team_col_light(&self, m: usize, side: usize) -> Color32 {
        let c = self.team_col(m, side);
        if luminance(c) > 0.35 {
            let k = 0.55;
            Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8)
        } else {
            c
        }
    }

    /// Crest textures for native board rendering.
    pub fn crest_ids(&mut self, ctx: &egui::Context, m: usize) -> [Option<egui::TextureId>; 2] {
        let mut out = [None, None];
        for side in 0..2 {
            let e = &self.lib.entries[m];
            let key = format!("{}#{}", e.s.id, side);
            if !self.crest_tex.contains_key(&key) {
                if let Some(f) = &e.s.crest_files[side] {
                    if let Ok(img) = image::open(e.dir.join(f)) {
                        let img = img.to_rgba8();
                        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], &img);
                        let t = ctx.load_texture(&key, ci, egui::TextureOptions::LINEAR);
                        self.crest_tex.insert(key.clone(), t);
                    }
                }
            }
            out[side] = self.crest_tex.get(&key).map(|t| t.id());
        }
        out
    }

    pub fn scene_path(&self, m: usize, board: &str) -> std::path::PathBuf {
        self.lib.entries[m].dir.join("boards").join(format!("{board}.scene.json"))
    }

    /// Draw a board natively (vector) at `width`; returns the click response.
    pub fn draw_board(&mut self, ui: &mut Ui, m: usize, board: &str, width: f32, thumb: bool, animate: bool) -> egui::Response {
        let sc = self.scenes.get(&self.scene_path(m, board));
        let aspect = sc.as_ref().map(|s| s.h / s.w).unwrap_or(1130.0 / 1600.0);
        // tall portrait boards are cropped to a tile in thumbnails; the viewer shows them whole
        let tile_h = if thumb && aspect > 1.1 { width * 0.95 } else { width * aspect };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, tile_h), Sense::click());
        match sc {
            Some(sc) => {
                let crests = self.crest_ids(ui.ctx(), m);
                let anim = if animate && !self.prefs.reduced_motion { Some((self.now - self.screen_t0) as f32) } else { None };
                let mut tile = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                tile.set_clip_rect(rect.intersect(ui.clip_rect()));
                // portrait boards: crop to the pitch, skipping the header block
                let full = if thumb && aspect > 1.1 {
                    egui::Rect::from_min_size(egui::pos2(rect.min.x, rect.min.y - width * 0.5), Vec2::new(width, width * aspect))
                } else {
                    egui::Rect::from_min_size(rect.min, Vec2::new(width, width * aspect))
                };
                crate::scene::paint(&tile, full, &sc, &crate::scene::PaintOpts { anim, hidden: None, selected: None, thumb, crests });
            }
            None => {
                ui.painter().rect_filled(rect, 10.0, ROOM_CARD);
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Board unavailable: rebuild this match", body(12.0), DARK_TEXT2);
            }
        }
        // rounded mask corners
        let r = 10.0;
        ui.painter().rect_stroke(rect, r, Stroke::new(1.0, ROOM_LINE), egui::StrokeKind::Inside);
        press_overlay(ui, &resp, r, true);
        resp
    }

    /// Bottom navigation for top-level destinations (≤ 5 items, labelled icons).
    /// Floating pill navigation: glass capsule, accent disc under the active icon.
    pub fn bottom_nav(&mut self, ui: &mut Ui, rect: egui::Rect, cur: &Screen, dark: bool) {
        let pill = egui::Rect::from_min_max(egui::pos2(rect.left() + 14.0, rect.top() + 8.0), egui::pos2(rect.right() - 14.0, rect.bottom() - 6.0));
        let p = ui.painter();
        let (fill, line) = if dark { (ROOM_CARD, ROOM_LINE) } else { (PRINT_CARD, PRINT_LINE) };
        p.rect_filled(pill, pill.height() / 2.0, fill);
        p.rect_stroke(pill, pill.height() / 2.0, Stroke::new(1.0, line), egui::StrokeKind::Inside);
        let items = [("Home", "home", Screen::Home), ("Matches", "list", Screen::Matches), ("Compare", "compare", Screen::Compare), ("Settings", "gear", Screen::Settings)];
        let w = pill.width() / items.len() as f32;
        for (i, (label, icon, target)) in items.iter().enumerate() {
            let r = egui::Rect::from_min_size(egui::pos2(pill.left() + i as f32 * w, pill.top()), Vec2::new(w, pill.height()));
            let resp = ui.interact(r, ui.scope_id().with(("nav", i)), Sense::click());
            let active = cur == target;
            let idle = if dark { DARK_TEXT2 } else { TEXT2 };
            let c = if active { WHITE } else { idle };
            let cy = r.center().y - 7.0;
            if active {
                let d = 44.0;
                let disc = egui::Rect::from_center_size(egui::pos2(r.center().x, cy), Vec2::splat(d));
                ui.painter().circle_filled(disc.center(), d / 2.0, PRIMARY);
                ui.painter().circle_stroke(disc.center(), d / 2.0, Stroke::new(1.0, SAGE));
            }
            let ir = egui::Rect::from_center_size(egui::pos2(r.center().x, cy), Vec2::splat(22.0));
            crate::screens::browse::draw_icon_col(ui, ir, icon, c);
            ui.painter().text(egui::pos2(r.center().x, r.bottom() - 10.0), egui::Align2::CENTER_CENTER, *label, egui::FontId::new(if rect.width() < 360.0 { 10.0 } else { 11.0 }, if active { egui::FontFamily::Name("semibold".into()) } else { egui::FontFamily::Proportional }), if active { if dark { PRINT } else { FOREST } } else { idle });
            if resp.is_pointer_button_down_on() {
                ui.painter().circle_filled(egui::pos2(r.center().x, cy), 24.0, Color32::from_rgba_unmultiplied(255, 255, 255, 22));
            }
            if resp.clicked() && !active {
                self.tab(target.clone());
            }
        }
    }

    /// Hub card for one board: tonal Darkroom card, a top-cropped live preview (no illegible small print),
    /// the board title and a one-line scope badge. Tap opens the viewer.
    pub fn board_card(&mut self, ui: &mut Ui, m: usize, board: &str, width: f32, title: &str, badge: &str) -> egui::Response {
        let preview_h = width * 0.66;
        let card_h = preview_h + 12.0 + card_text_h();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, card_h), Sense::click());
        ui.painter().rect_filled(rect, 20.0, ROOM_CARD);
        let inner = egui::Rect::from_min_size(rect.min + Vec2::splat(8.0), Vec2::new(width - 16.0, preview_h));
        let sc = self.scenes.get(&self.scene_path(m, board));
        match sc {
            Some(sc) => {
                let crests = self.crest_ids(ui.ctx(), m);
                // start the crop below the intro (headline, subtitle, divider) so the data fills the preview
                let skip = sc.prims.iter().find_map(|p| match p {
                    matchhub::render::svg::Prim::R { y, h, .. } if (*h - 3.0).abs() < 0.01 => Some(*y + 18.0),
                    _ => None,
                }).unwrap_or(0.0);
                let scale = inner.width() / sc.w;
                let full = egui::Rect::from_min_size(egui::pos2(inner.left(), inner.top() - skip * scale), Vec2::new(inner.width(), sc.h * scale));
                let mut tile = ui.new_child(egui::UiBuilder::new().max_rect(inner));
                tile.set_clip_rect(inner.intersect(ui.clip_rect()));
                // preview crop is rounded by painting a card-coloured frame over its corners
                crate::scene::paint(&tile, full, &sc, &crate::scene::PaintOpts { anim: None, hidden: None, selected: None, thumb: true, crests });
                ui.painter().rect_stroke(inner, 14.0, Stroke::new(1.0, ROOM_LINE), egui::StrokeKind::Inside);
            }
            None => {
                ui.painter().rect_filled(inner, 14.0, ROOM_CARD_HI);
                ui.painter().text(inner.center(), egui::Align2::CENTER_CENTER, "Rebuild this match", body(12.0), DARK_TEXT2);
            }
        }
        let tx = rect.left() + 14.0;
        let maxw = width - 28.0;
        // both lines stay on one line each: a longer text ends in an ellipsis
        let title_g = one_line(ui, title, semi(14.0), PRINT, maxw);
        let badge_g = one_line(ui, badge, body(12.0), SAGE, maxw);
        ui.painter().galley(egui::pos2(tx, inner.bottom() + 10.0), title_g, PRINT);
        ui.painter().galley(egui::pos2(tx, rect.bottom() - 10.0 - badge_g.size().y), badge_g, SAGE);
        press_overlay(ui, &resp, 20.0, true);
        resp
    }

    /// The "Player cards" entry in the Players section: same card shape, halftone motif instead of a preview.
    pub fn players_card(&mut self, ui: &mut Ui, _m: usize, width: f32, n: usize) -> egui::Response {
        let preview_h = width * 0.66;
        let card_h = preview_h + 12.0 + card_text_h();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, card_h), Sense::click());
        ui.painter().rect_filled(rect, 20.0, ROOM_CARD);
        let inner = egui::Rect::from_min_size(rect.min + Vec2::splat(8.0), Vec2::new(width - 16.0, preview_h));
        ui.painter().rect_filled(inner, 14.0, ROOM_CARD_HI);
        let p = ui.painter().with_clip_rect(inner);
        let c = inner.center();
        let step = 9.0;
        for iy in -8..=8 {
            for ix in -10..=10 {
                let x = ix as f32 * step + if iy % 2 == 0 { 0.0 } else { step / 2.0 };
                let y = iy as f32 * step * 0.87;
                let d = ((x * x + y * y).sqrt() / 70.0).min(1.0);
                let r = (1.0 - d) * step * 0.42;
                if r > 0.5 {
                    p.circle_filled(egui::pos2(c.x + x, c.y + y), r, Color32::from_rgba_unmultiplied(FIXER.r(), FIXER.g(), FIXER.b(), ((1.0 - d) * 200.0) as u8));
                }
            }
        }
        p.text(c, egui::Align2::CENTER_CENTER, n.to_string(), head(34.0), PRINT);
        let tx = rect.left() + 14.0;
        ui.painter().text(egui::pos2(tx, inner.bottom() + 10.0), egui::Align2::LEFT_TOP, "Player cards", semi(14.0), PRINT);
        ui.painter().text(egui::pos2(tx, rect.bottom() - 10.0), egui::Align2::LEFT_BOTTOM, "Every player in the match", body(12.0), SAGE);
        press_overlay(ui, &resp, 20.0, true);
        resp
    }

    /// "Paste" pill (48dp) for link and token fields. Returns the clipboard text when tapped.
    /// On desktop the text fields already take Ctrl+V, so the pill only explains that.
    pub fn paste_pill(&mut self, ui: &mut Ui) -> Option<String> {
        let btn = egui::Button::new(RichText::new("Paste").font(semi(14.0)).color(if lt() { PRINT } else { INK })).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, if lt() { PRINT } else { INK })).corner_radius(24).min_size(Vec2::new(84.0, 48.0));
        if !ui.add(btn).on_hover_text("Paste from the clipboard").clicked() {
            return None;
        }
        match self.clipboard_text() {
            Some(t) if !t.trim().is_empty() => Some(t),
            _ => {
                let msg = if cfg!(target_os = "android") { "Clipboard is empty. Copy the link first." } else { "Use Ctrl+V on desktop." };
                self.toast(ui.ctx(), msg);
                None
            }
        }
    }

    pub fn crest(&mut self, ui: &mut Ui, m: usize, side: usize, size: f32) {
        let e = &self.lib.entries[m];
        let name = if side == 0 { e.s.home.clone() } else { e.s.away.clone() };
        let col = hex(&e.s.colours[side]);
        let key = format!("{}#{}", e.s.id, side);
        if !self.crest_tex.contains_key(&key) {
            if let Some(f) = &e.s.crest_files[side] {
                if let Ok(img) = image::open(e.dir.join(f)) {
                    let img = img.to_rgba8();
                    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], &img);
                    let t = ui.ctx().load_texture(&key, ci, egui::TextureOptions::LINEAR);
                    self.crest_tex.insert(key.clone(), t);
                }
            }
        }
        match self.crest_tex.get(&key) {
            Some(t) => {
                ui.add(egui::Image::new(t).fit_to_exact_size(Vec2::splat(size)));
            }
            None => monogram(ui, &name, col, size),
        }
    }

    /// Board-style header: crests, names in team colours, score, scope label and split rule.
    pub fn match_header(&mut self, ui: &mut Ui, m: usize, scope: &str) {
        let entry = self.lib.entries[m].clone();
        let s = &entry.s;
        let (hc, ac) = (self.team_col(m, 0), self.team_col(m, 1));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let w = ui.available_width();
            // names shrink to fit the side (full -> short -> code) so the row never widens the screen
            let chars = (((w * 0.38 - 44.0) / (bold(14.0).size * 0.72)).floor() as usize).clamp(3, 12);
            ui.allocate_ui_with_layout(Vec2::new(w * 0.38, 40.0), Layout::right_to_left(Align::Center), |ui| {
                self.crest(ui, m, 0, 30.0);
                ui.label(RichText::new(fit_name(&s.home, &s.home_short, chars)).font(bold(14.0)).color(hc));
            });
            ui.allocate_ui_with_layout(Vec2::new(w * 0.2, 40.0), Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
                ui.label(RichText::new(format!("{}–{}", s.score[0], s.score[1])).font(head(30.0)).color(WHITE));
            });
            ui.allocate_ui_with_layout(Vec2::new(w * 0.38, 40.0), Layout::left_to_right(Align::Center), |ui| {
                self.crest(ui, m, 1, 30.0);
                ui.label(RichText::new(fit_name(&s.away, &s.away_short, chars)).font(bold(14.0)).color(ac));
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(scope).font(bold(12.0)).color(DARK_TEXT2));
            ui.label(RichText::new(format!("{} · {}", s.competition, s.date)).font(body(12.0)).color(DARK_TEXT2));
        });
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 3.0), Sense::hover());
        ui.painter().rect_filled(egui::Rect::from_min_max(r.min, egui::pos2(r.center().x, r.max.y)), 0.0, hc);
        ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(r.center().x, r.min.y), r.max), 0.0, ac);
    }

    /// Teal momentum strip summarising the match spikes.
    pub fn momentum_strip(&self, ui: &mut Ui, m: usize, h: f32) {
        let s = &self.lib.entries[m].s;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 10.0, ROOM_CARD);
        let n = s.momentum[0].len().max(2);
        let mx = s.momentum[0].iter().chain(&s.momentum[1]).cloned().fold(0.01f32, f32::max);
        let mid = rect.center().y;
        let x = |i: usize| rect.left() + 6.0 + i as f32 / (n - 1) as f32 * (rect.width() - 12.0);
        // club colours, as on the Match momentum board: home above the midline, away below
        let cols = [self.team_col(m, 0), self.team_col(m, 1)];
        for (k, sgn) in [(0usize, -1.0f32), (1, 1.0)] {
            let col = cols[k];
            let mut pts = vec![egui::pos2(x(0), mid)];
            for (i, v) in s.momentum[k].iter().enumerate() {
                pts.push(egui::pos2(x(i), mid + sgn * v / mx * (h / 2.0 - 6.0)));
            }
            pts.push(egui::pos2(x(n - 1), mid));
            // filled area: split into one quad per step (a concave outline cannot be a convex polygon)
            let fill = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 190);
            let mut mesh = egui::Mesh::default();
            for w in pts.windows(2) {
                let base = mesh.vertices.len() as u32;
                for q in [egui::pos2(w[0].x, mid), w[0], egui::pos2(w[1].x, mid), w[1]] {
                    mesh.colored_vertex(q, fill);
                }
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base + 1, base + 3, base + 2);
            }
            p.add(egui::Shape::mesh(mesh));
            p.add(egui::Shape::line(pts, Stroke::new(1.5, col)));
        }
        p.line_segment([egui::pos2(rect.left() + 6.0, mid), egui::pos2(rect.right() - 6.0, mid)], Stroke::new(1.0, ROOM_LINE));
        for (side, minute, _) in &s.goals {
            let i = (*minute as usize).min(n - 1);
            let c = hex(&s.colours[*side]);
            p.circle(egui::pos2(x(i), mid), 5.0, c, Stroke::new(1.5, WHITE));
        }
        let label = if rect.width() < 360.0 * text_scale_pct() as f32 / 100.0 { "Momentum" } else { "Momentum · home above, away below" };
        p.text(rect.left_top() + Vec2::new(8.0, 6.0), egui::Align2::LEFT_TOP, label, body(12.0), DARK_TEXT2);
    }

    pub fn build_status(&self, ui: &mut Ui, dark: bool) {
        let st = self.build.lock().unwrap().clone();
        if !st.running && st.step.is_empty() {
            return;
        }
        card(dark).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(if st.running { "Building" } else if st.step == "done" { "Saved match" } else { "Last build" }).font(bold(13.0)).color(if st.running { SAGE } else { sub() }));
                ui.label(RichText::new(step_text(&st.step, &st.detail)).font(body(13.0)).color(if dark { PRINT } else { INK }));
            });
            if st.running && ui.add(egui::Button::new(RichText::new("Cancel").font(semi(14.0)).color(if dark { PRINT } else { INK })).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, if dark { PRINT } else { INK })).corner_radius(24).min_size(Vec2::new(96.0, 48.0))).clicked() {
                matchhub::cancel::request();
            }
            let cur = STEPS.iter().position(|s| *s == st.step).unwrap_or(if st.step == "done" { STEPS.len() } else { 0 });
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::hover());
            let p = ui.painter();
            let n = STEPS.len();
            for (i, name) in STEPS.iter().enumerate() {
                let x = rect.left() + 14.0 + i as f32 / (n - 1) as f32 * (rect.width() - 28.0);
                let done = i < cur || st.step == "done";
                let active = i == cur && st.running;
                let c = if done { SECONDARY } else if active { SECONDARY_VARIANT } else { DIVIDER };
                if i + 1 < n {
                    let x2 = rect.left() + 14.0 + (i + 1) as f32 / (n - 1) as f32 * (rect.width() - 28.0);
                    p.line_segment([egui::pos2(x, rect.top() + 12.0), egui::pos2(x2, rect.top() + 12.0)], Stroke::new(3.0, if done { SECONDARY } else { DIVIDER }));
                }
                p.circle_filled(egui::pos2(x, rect.top() + 12.0), if active { 8.0 } else { 6.0 }, c);
                // captions only when they fit: at narrow widths or large text the headline names the step
                if rect.width() / n as f32 >= 62.0 * text_scale_pct() as f32 / 100.0 {
                    p.text(egui::pos2(x, rect.top() + 34.0), egui::Align2::CENTER_CENTER, *name, body(12.0), if dark { DARK_TEXT2 } else { TEXT2 });
                }
            }
            if st.running {
                ui.add(egui::ProgressBar::new(st.frac).fill(SECONDARY_VARIANT).desired_height(6.0));
            }
        });
    }

    /// Match card used on home and the match list.
    pub fn match_card(&mut self, ui: &mut Ui, m: usize, width: f32) -> bool {
        let e = self.lib.entries[m].clone();
        let mut clicked = false;
        let r = card(lt()).show(ui, |ui| ui.vertical(|ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                self.crest(ui, m, 0, 28.0);
                let chars = (((width - 100.0) / 2.0 / (head(19.0).size * 0.66)).floor() as usize).clamp(3, 11);
                ui.label(RichText::new(format!("{}  {}–{}  {}", fit_name(&e.s.home, &e.s.home_short, chars), e.s.score[0], e.s.score[1], fit_name(&e.s.away, &e.s.away_short, chars))).font(head(19.0)).color(fg()));
                self.crest(ui, m, 1, 28.0);
            });
            ui.label(RichText::new(format!("{} · {}", if e.s.competition.is_empty() { "Match" } else { &e.s.competition }, e.s.date)).font(body(12.0)).color(sub()));
            // two chips per row on compact widths or large text, three otherwise: never clipped
            let f = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or("–".into());
            let chips = [("npxG", format!("{:.2}–{:.2}", e.s.npxg[0], e.s.npxg[1])), ("PPDA", format!("{}–{}", f(e.s.ppda[0]), f(e.s.ppda[1]))), ("xPts", format!("{:.1}–{:.1}", e.s.xpts[0], e.s.xpts[1]))];
            let chip_w = 150.0 * style::text_scale_pct() as f32 / 100.0;
            let per_row = ((width / chip_w).floor() as usize).clamp(1, 3);
            for row in chips.chunks(per_row) {
                ui.horizontal(|ui| {
                    for (k, v) in row {
                        metric_chip(ui, k, v, lt());
                    }
                });
            }
        }));
        let resp = r.response.interact(Sense::click());
        press_overlay(ui, &resp, 18.0, lt());
        if resp.clicked() {
            clicked = true;
        }
        clicked
    }
}

/// Height of the title + badge block under a hub card preview; grows with the text size.
fn card_text_h() -> f32 {
    (14.0 + 12.0) * text_scale_pct() as f32 / 100.0 * 1.35 + 28.0
}

/// A galley that is never wider than `max_w`: the text is cut with an ellipsis instead of wrapping.
fn one_line(ui: &Ui, text: &str, font: egui::FontId, col: Color32, max_w: f32) -> std::sync::Arc<egui::Galley> {
    let whole = ui.painter().layout_no_wrap(text.to_string(), font.clone(), col);
    if whole.size().x <= max_w {
        return whole;
    }
    let chars: Vec<char> = text.chars().collect();
    for n in (1..chars.len()).rev() {
        let cut: String = chars[..n].iter().collect::<String>().trim_end().to_string() + "…";
        let g = ui.painter().layout_no_wrap(cut, font.clone(), col);
        if g.size().x <= max_w {
            return g;
        }
    }
    ui.painter().layout_no_wrap("…".to_string(), font, col)
}

/// What the progress card says: fetching, parsing, ..., saved.
fn step_text(step: &str, detail: &str) -> String {
    let d = detail.to_lowercase();
    match step {
        "ingest" if d.contains("built-in browser") => "Opening the page in the built-in browser…".into(),
        "ingest" if d.starts_with("browser:") => format!("Built-in browser · {}", detail.trim_start_matches("browser:").trim()),
        "ingest" if d.contains("relay") => "Checking the relay for this match…".into(),
        "ingest" if d.contains("fetching") => "Fetching page…".into(),
        "ingest" if d.starts_with("blocked") => "Blocked: asking the GitHub scraper…".into(),
        "ingest" if d.starts_with("waiting") => format!("Waiting for the GitHub scraper · {}", detail.rsplit('·').next().unwrap_or("").trim()),
        "ingest" => "Parsing events…".into(),
        "enrich" => "Adding colours, crests and ages…".into(),
        "model" => "Fitting the models…".into(),
        "derive" => "Computing metrics…".into(),
        "render" => "Drawing boards…".into(),
        "report" => "Writing the report…".into(),
        "done" => "Open it from Latest built matches".into(),
        "queued" => "Starting…".into(),
        other => other.to_string(),
    }
}

pub fn boards_by_section(e: &Entry) -> Vec<(String, Vec<library::BoardMeta>)> {
    let order = ["Chances and goals", "Creation and progression", "Territory and defence", "Match state and simulation", "Context", "Players"];
    order
        .iter()
        .map(|sec| (sec.to_string(), e.s.boards.iter().filter(|b| b.section == *sec && !b.id.starts_with("31_")).cloned().collect::<Vec<_>>()))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// Variants of a board family for the scope switcher: (label, board id, enabled).
pub fn scope_variants(e: &Entry, id: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let ids: Vec<&String> = e.s.boards.iter().map(|b| &b.id).collect();
    if id.starts_with("10_pass_network_") {
        let half = if id.ends_with("_h2") { "h2" } else { "h1" };
        let side = if id.contains("_home_") { "home" } else { "away" };
        for s in ["home", "away"] {
            let cand = format!("10_pass_network_{s}_{half}");
            if ids.iter().any(|x| **x == cand) {
                out.push(((if s == "home" { e.s.home.clone() } else { e.s.away.clone() }), cand));
            }
        }
        for h in ["h1", "h2"] {
            out.push((if h == "h1" { "1st half".into() } else { "2nd half".into() }, format!("10_pass_network_{side}_{h}")));
        }
        return out;
    }
    let base = id.trim_end_matches("_home").trim_end_matches("_away");
    if base != id {
        for s in ["home", "away"] {
            let cand = format!("{base}_{s}");
            if ids.iter().any(|x| **x == cand) {
                out.push(((if s == "home" { e.s.home.clone() } else { e.s.away.clone() }), cand));
            }
        }
    } else {
        out.push(("Full match".into(), id.to_string()));
    }
    out
}

pub fn style_ref() -> &'static str {
    style::CONDENSED
}
