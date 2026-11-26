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
                ui.painter().rect_filled(rect, 10.0, Color32::from_rgb(0x18, 0x13, 0x22));
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Board unavailable: rebuild this match", body(12.0), DARK_TEXT2);
            }
        }
        // rounded mask corners
        let r = 10.0;
        ui.painter().rect_stroke(rect, r, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 26)), egui::StrokeKind::Inside);
        press_overlay(ui, &resp, r, true);
        resp
    }

    /// Bottom navigation for top-level destinations (≤ 5 items, labelled icons).
    /// Floating pill navigation: glass capsule, accent disc under the active icon.
    pub fn bottom_nav(&mut self, ui: &mut Ui, rect: egui::Rect, cur: &Screen, dark: bool) {
        let pill = egui::Rect::from_min_max(egui::pos2(rect.left() + 14.0, rect.top() + 8.0), egui::pos2(rect.right() - 14.0, rect.bottom() - 6.0));
        let p = ui.painter();
        let (fill, line) = if dark { (Color32::from_rgba_unmultiplied(34, 26, 70, 235), Color32::from_rgba_unmultiplied(255, 255, 255, 36)) } else { (Color32::from_rgb(0xF4, 0xF0, 0xFC), Color32::from_rgb(0xDD, 0xD3, 0xF2)) };
        p.rect_filled(pill.translate(Vec2::new(0.0, 3.0)), pill.height() / 2.0, Color32::from_black_alpha(if dark { 70 } else { 22 }));
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
                ui.painter().circle_stroke(disc.center(), d / 2.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)));
            }
            let ir = egui::Rect::from_center_size(egui::pos2(r.center().x, cy), Vec2::splat(22.0));
            crate::screens::browse::draw_icon_col(ui, ir, icon, c);
            ui.painter().text(egui::pos2(r.center().x, r.bottom() - 10.0), egui::Align2::CENTER_CENTER, *label, egui::FontId::new(if rect.width() < 360.0 { 10.0 } else { 11.0 }, if active { egui::FontFamily::Name("semibold".into()) } else { egui::FontFamily::Proportional }), if active { if dark { WHITE } else { PRIMARY } } else { idle });
            if resp.is_pointer_button_down_on() {
                ui.painter().circle_filled(egui::pos2(r.center().x, cy), 24.0, Color32::from_rgba_unmultiplied(255, 255, 255, 22));
            }
            if resp.clicked() && !active {
                self.tab(target.clone());
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
        let s = self.lib.entries[m].s.clone();
        let (hc, ac) = (self.team_col(m, 0), self.team_col(m, 1));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let w = ui.available_width();
            // names shrink to fit the side (full -> short -> code) so the row never widens the screen
            let chars = (((w * 0.38 - 40.0) / (bold(14.0).size * 0.62)).floor() as usize).clamp(3, 12);
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
        p.rect_filled(rect, 10.0, Color32::from_rgb(0x10, 0x1a, 0x1f));
        let n = s.momentum[0].len().max(2);
        let mx = s.momentum[0].iter().chain(&s.momentum[1]).cloned().fold(0.01f32, f32::max);
        let mid = rect.center().y;
        let x = |i: usize| rect.left() + 6.0 + i as f32 / (n - 1) as f32 * (rect.width() - 12.0);
        for (k, sgn, col) in [(0usize, -1.0f32, SECONDARY), (1, 1.0, SECONDARY_VARIANT)] {
            let mut pts = vec![egui::pos2(x(0), mid)];
            for (i, v) in s.momentum[k].iter().enumerate() {
                pts.push(egui::pos2(x(i), mid + sgn * v / mx * (h / 2.0 - 6.0)));
            }
            pts.push(egui::pos2(x(n - 1), mid));
            p.add(egui::Shape::convex_polygon(vec![], Color32::TRANSPARENT, Stroke::NONE));
            p.add(egui::Shape::line(pts, Stroke::new(2.0, col)));
        }
        p.line_segment([egui::pos2(rect.left() + 6.0, mid), egui::pos2(rect.right() - 6.0, mid)], Stroke::new(1.0, Color32::from_gray(80)));
        for (side, minute, _) in &s.goals {
            let i = (*minute as usize).min(n - 1);
            let c = hex(&s.colours[*side]);
            p.circle(egui::pos2(x(i), mid), 5.0, c, Stroke::new(1.5, WHITE));
        }
        p.text(rect.left_top() + Vec2::new(8.0, 6.0), egui::Align2::LEFT_TOP, "MOMENTUM · home above, away below", body(12.0), DARK_TEXT2);
    }

    pub fn build_status(&self, ui: &mut Ui, dark: bool) {
        let st = self.build.lock().unwrap().clone();
        if !st.running && st.step.is_empty() {
            return;
        }
        card(dark).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(if st.running { "● BUILDING" } else { "✓ LAST BUILD" }).font(bold(12.0)).color(if st.running { SECONDARY_VARIANT } else { sub() }));
                ui.label(RichText::new(&st.step).font(body(12.0)));
            });
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
                p.text(egui::pos2(x, rect.top() + 34.0), egui::Align2::CENTER_CENTER, *name, body(12.0), if dark { DARK_TEXT2 } else { TEXT2 });
            }
            if st.running {
                ui.add(egui::ProgressBar::new(st.frac).fill(SECONDARY_VARIANT).desired_height(6.0));
            }
        });
    }

    /// Match card used on home and the match list.
    pub fn match_card(&mut self, ui: &mut Ui, m: usize, width: f32) -> bool {
        let e: Entry = self.lib.entries[m].clone();
        let mut clicked = false;
        let r = card(lt()).show(ui, |ui| ui.vertical(|ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                self.crest(ui, m, 0, 28.0);
                let chars = (((width - 90.0) / 2.0 / (head(19.0).size * 0.5)).floor() as usize).clamp(3, 11);
                ui.label(RichText::new(format!("{}  {}–{}  {}", fit_name(&e.s.home, &e.s.home_short, chars), e.s.score[0], e.s.score[1], fit_name(&e.s.away, &e.s.away_short, chars))).font(head(19.0)).color(fg()));
                self.crest(ui, m, 1, 28.0);
            });
            ui.label(RichText::new(format!("{} · {}", if e.s.competition.is_empty() { "Match" } else { &e.s.competition }, e.s.date)).font(body(12.0)).color(sub()));
            ui.horizontal_wrapped(|ui| {
                metric_chip(ui, "npxG", &format!("{:.2}–{:.2}", e.s.npxg[0], e.s.npxg[1]), lt());
                let f = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or("–".into());
                metric_chip(ui, "PPDA", &format!("{}–{}", f(e.s.ppda[0]), f(e.s.ppda[1])), lt());
                metric_chip(ui, "xPts", &format!("{:.1}–{:.1}", e.s.xpts[0], e.s.xpts[1]), lt());
            });
        }));
        let resp = r.response.interact(Sense::click());
        press_overlay(ui, &resp, 18.0, lt());
        if resp.clicked() {
            clicked = true;
        }
        clicked
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
