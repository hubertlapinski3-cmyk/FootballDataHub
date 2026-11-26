//! Screen 4: full-screen board viewer with pinch-zoom, pan, double-tap to fit,
//! scope switcher, how-to-read line, export and provenance sheet.

use super::scope_variants;
use crate::style::*;
use crate::{App, Screen};
use egui::{Align, Color32, Layout, Rect, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};

/// Height available to the board canvas for a window of the given height.
fn canvas_h_hint(window_h: f32) -> f32 {
    (window_h - 128.0 - 8.0).max(100.0)
}

impl App {
    pub fn board_viewer(&mut self, ui: &mut Ui, m: usize, board: &str) {
        if m >= self.lib.entries.len() {
            self.go_home();
            return;
        }
        let e = self.lib.entries[m].clone();
        let Some(meta) = e.s.boards.iter().find(|b| b.id == board) else {
            self.back();
            return;
        };
        let full = ui.max_rect();
        // Fit / Fill only when it changes the view meaningfully: a whole-fit board that fills the width already
        let fill_zoom = (canvas_h_hint(ui.max_rect().height()) / (ui.max_rect().width() * (meta.height / meta.width) as f32)).clamp(1.0, 3.0);
        let show_fill = (meta.height / meta.width) < 1.1 && (fill_zoom > 1.15 || self.viewer.zoom > 1.01);
        let chrome = if scope_variants(&e, board).len() > 1 { 128.0 } else { 70.0 };
        // ---- chrome top
        let mut top = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(full.min, Vec2::new(full.width(), chrome))));
        egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 2)).show(&mut top, |ui| {
            ui.horizontal(|ui| {
                if back_button(ui, WHITE).clicked() {
                    self.back();
                }
                // the title gets what the two icons leave, then truncates
                let title_w = (ui.available_width() - 2.0 * 48.0 - 8.0 - if show_fill { 60.0 } else { 0.0 }).max(60.0);
                ui.allocate_ui_with_layout(Vec2::new(title_w, 48.0), Layout::left_to_right(Align::Center), |ui| {
                    ui.set_max_width(title_w);
                    // shrink to fit (down to 12) before truncating, so "Match momentum" stays whole
                    let mut size: f32 = if meta.title.chars().count() > 12 { 19.0 } else { 22.0 };
                    let est = |sz: f32| meta.title.chars().count() as f32 * sz * 0.62 * text_scale_pct() as f32 / 100.0;
                    while size > 12.0 && est(size) > title_w {
                        size -= 1.0;
                    }
                    ui.add(egui::Label::new(RichText::new(&meta.title).font(head(size)).color(WHITE)).truncate());
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if show_fill && ui.add(egui::Button::new(RichText::new(if self.viewer.zoom > 1.01 { "Fit" } else { "Fill" }).font(semi(14.0)).color(PRIMARY_ON_DARK)).fill(Color32::TRANSPARENT).min_size(Vec2::splat(48.0))).clicked() {
                        // ease to the new zoom (see the animation step below) instead of snapping
                        self.viewer.target_zoom = Some(if self.viewer.zoom > 1.01 { 1.0 } else { fill_zoom });
                        self.viewer.target_pan = Vec2::ZERO;
                        self.viewer.pan_vel = Vec2::ZERO;
                    }
                    let icon_btn = |ui: &mut Ui, kind: &str, id: &str| -> bool {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::click());
                        let c = if resp.is_pointer_button_down_on() { WHITE } else { PRIMARY_ON_DARK };
                        crate::screens::browse::draw_icon_col(ui, Rect::from_center_size(r.center(), Vec2::splat(24.0)), kind, c);
                        let _ = id;
                        resp.clicked()
                    };
                    if icon_btn(ui, "info", "info") {
                        self.viewer.show_manifest = !self.viewer.show_manifest;
                    }
                    ui.add_space(8.0);
                    if icon_btn(ui, "share", "export") {
                        self.viewer.show_export = true;
                    }
                });
            });
            if scope_variants(&e, board).len() > 1 { ScrollArea::horizontal().id_salt("scopes").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let vars = scope_variants(&e, board);
                    let has_full = vars.iter().any(|v| v.0 == "Full match");
                    if !has_full {
                        let _ = chip(ui, "Full match", false, true);
                    }
                    let _ = has_full;
                    for (label, id) in vars {
                        let active = id == board && !(label.ends_with("HALF") && false);
                        if chip(ui, &label, active, true) && id != board {
                            if let Some(Screen::Viewer { board: b, .. }) = self.stack.last_mut() {
                                *b = id.clone();
                            }
                            let now = self.now;
                            self.viewer.hidden.clear();
                            self.viewer.selected = None;
                            self.viewer.opened_at = now;
                            self.viewer.zoom = 1.0;
                            self.viewer.pan = Vec2::ZERO;
                            self.viewer.at_top = true;
                            self.viewer.target_zoom = None;
                            self.viewer.pan_vel = Vec2::ZERO;
                            self.viewer.scrubbing = false;
                        }
                    }
                });
            }); }
        });
        // ---- board canvas
        let canvas = Rect::from_min_max(egui::pos2(full.left(), full.top() + chrome), egui::pos2(full.right(), full.bottom() - 8.0));
        let resp = ui.interact(canvas, ui.scope_id().with("canvas"), Sense::click_and_drag());
        self.exclusion = Some(canvas);
        let aspect = (meta.height / meta.width) as f32;
        // portrait boards fit the width and scroll; landscape boards fit whole
        let fit_w = if aspect > 1.1 { canvas.width() } else { canvas.width().min(canvas.height() / aspect) };
        let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.05);
        let ctx = ui.ctx().clone();
        let fits_whole = aspect <= 1.1;
        // pinch / ctrl-scroll zoom: direct manipulation, cancels any easing
        let (zoom_delta, multi) = ui.input(|i| (i.zoom_delta(), i.multi_touch()));
        if resp.hovered() || multi.is_some() {
            let z = (self.viewer.zoom * zoom_delta).clamp(1.0, 5.0);
            if (z - self.viewer.zoom).abs() > 1e-4 {
                if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                    let c = canvas.center() + self.viewer.pan;
                    let k = z / self.viewer.zoom;
                    self.viewer.pan += (p - c) * (1.0 - k);
                }
                self.viewer.zoom = z;
                self.viewer.target_zoom = None;
                self.viewer.pan_vel = Vec2::ZERO;
            }
        }
        // double tap: ease in at the tapped point, or back out
        if resp.double_clicked() {
            if self.viewer.zoom > 1.01 {
                self.viewer.target_zoom = Some(1.0);
                self.viewer.target_pan = if fits_whole { Vec2::ZERO } else { Vec2::new(0.0, self.viewer.pan.y) };
            } else if let Some(p) = resp.interact_pointer_pos() {
                let k = 2.5 / self.viewer.zoom;
                self.viewer.target_zoom = Some(2.5);
                self.viewer.target_pan = self.viewer.pan + (p - (canvas.center() + self.viewer.pan)) * (1.0 - k);
            }
            self.viewer.pan_vel = Vec2::ZERO;
        }
        // press tracking: a finger held still for 0.3 s on the board starts scrubbing
        let down = resp.is_pointer_button_down_on();
        let pointer = ui.input(|i| i.pointer.interact_pos());
        if down {
            let now = self.now;
            if self.viewer.press_t0.is_none() {
                self.viewer.press_t0 = Some(now);
                self.viewer.press_pos = pointer.unwrap_or(canvas.center());
            } else if !self.viewer.scrubbing && !resp.dragged() && now - self.viewer.press_t0.unwrap_or(now) > 0.3 && pointer.map(|p| (p - self.viewer.press_pos).length() < 10.0).unwrap_or(false) {
                self.viewer.scrubbing = true;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
        let was_scrubbing = self.viewer.scrubbing;
        if !down {
            self.viewer.press_t0 = None;
            self.viewer.scrubbing = false;
        }
        // a board that fits whole at 1x has nothing to pan: dragging scrubs the tooltip instead
        let scrub_mode = self.viewer.scrubbing || (fits_whole && self.viewer.zoom <= 1.01 && self.viewer.target_zoom.is_none() && down);
        let mut animating = false;
        if scrub_mode {
            self.viewer.pan_vel = Vec2::ZERO;
        } else if resp.dragged() {
            let d = resp.drag_delta();
            self.viewer.pan += d;
            self.viewer.pan_vel = self.viewer.pan_vel * 0.5 + d / dt * 0.5;
            self.viewer.target_zoom = None;
        } else if self.viewer.pan_vel.length() > 1.0 {
            // inertial fling: exponential decay after release
            self.viewer.pan += self.viewer.pan_vel * dt;
            self.viewer.pan_vel *= 0.88_f32.powf(dt * 60.0);
            animating = true;
        } else {
            self.viewer.pan_vel = Vec2::ZERO;
        }
        // eased zoom toward its target
        if let Some(tz) = self.viewer.target_zoom {
            let k = 1.0 - 0.001_f32.powf(dt);
            self.viewer.zoom += (tz - self.viewer.zoom) * k;
            let (tp, p) = (self.viewer.target_pan, self.viewer.pan);
            self.viewer.pan += (tp - p) * k;
            if (tz - self.viewer.zoom).abs() < 0.004 && (self.viewer.target_pan - self.viewer.pan).length() < 0.5 {
                self.viewer.zoom = tz;
                self.viewer.pan = self.viewer.target_pan;
                self.viewer.target_zoom = None;
            }
            animating = true;
        }
        let size = Vec2::new(fit_w, fit_w * aspect) * self.viewer.zoom;
        // keep the board on screen
        let max_pan = ((size - canvas.size()) / 2.0).max(Vec2::ZERO);
        if self.viewer.at_top {
            self.viewer.pan = Vec2::new(0.0, max_pan.y);
            self.viewer.at_top = false;
        }
        let before = self.viewer.pan;
        self.viewer.pan = self.viewer.pan.clamp(-max_pan, max_pan);
        if self.viewer.pan.x != before.x {
            self.viewer.pan_vel.x = 0.0;
        }
        if self.viewer.pan.y != before.y {
            self.viewer.pan_vel.y = 0.0;
        }
        if animating {
            ctx.request_repaint();
        }
        let mut centre = canvas.center() + self.viewer.pan;
        if size.y < canvas.height() {
            // short board on a tall phone: sit just under the chrome instead of floating mid-screen
            centre.y = canvas.top() + size.y / 2.0 + 6.0;
        }
        let rect = Rect::from_center_size(centre, size);
        // native vector board: crisp at any zoom, tappable marks and legend
        let sc = self.scenes.get(&self.scene_path(m, board));
        let crests = self.crest_ids(ui.ctx(), m);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(canvas));
        child.set_clip_rect(canvas);
        match &sc {
            Some(sc) => {
                let anim = if self.prefs.reduced_motion { None } else { Some((self.now - self.viewer.opened_at) as f32) };
                crate::scene::paint(&child, rect, sc, &crate::scene::PaintOpts { anim, hidden: Some(&self.viewer.hidden), selected: self.viewer.selected.map(|s| s.0), thumb: false, crests });
                // live scrubbing: the tooltip follows the finger across marks
                if scrub_mode && down {
                    if let Some(pos) = pointer {
                        if let crate::scene::Hit::Mark(i, t) = crate::scene::hit(rect, sc, pos, &self.viewer.hidden) {
                            self.viewer.selected = Some((i, t));
                        }
                    }
                }
                if resp.clicked() && !was_scrubbing {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        match crate::scene::hit(rect, sc, pos, &self.viewer.hidden) {
                            crate::scene::Hit::Legend(l) => {
                                if !self.viewer.hidden.remove(&l) {
                                    self.viewer.hidden.insert(l);
                                }
                                self.viewer.selected = None;
                            }
                            crate::scene::Hit::Mark(i, t) => {
                                // on whole-fit boards the press already selected it: a tap must not toggle it off
                                let toggle = !(fits_whole && self.viewer.zoom <= 1.01);
                                self.viewer.selected = if toggle && self.viewer.selected.map(|s| s.0) == Some(i) { None } else { Some((i, t)) };
                            }
                            crate::scene::Hit::None => self.viewer.selected = None,
                        }
                    }
                }
                // desktop hover preview
                let mut shown = false;
                if let Some((i, t)) = self.viewer.selected {
                    if let (Some(a), Some(txt)) = (crate::scene::mark_anchor(rect, sc, i), sc.tips.get(t as usize)) {
                        crate::scene::tooltip(&child, a, txt, canvas);
                        shown = true;
                    }
                }
                if !shown && !ui.input(|i| i.any_touches()) {
                    if let Some(pos) = resp.hover_pos() {
                        if let crate::scene::Hit::Mark(i, t) = crate::scene::hit(rect, sc, pos, &self.viewer.hidden) {
                            if let (Some(a), Some(txt)) = (crate::scene::mark_anchor(rect, sc, i), sc.tips.get(t as usize)) {
                                crate::scene::tooltip(&child, a, txt, canvas);
                            }
                        }
                    }
                }
            }
            None => {
                child.painter().rect_filled(rect, 8.0, ROOM_CARD);
                child.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Board data missing: rebuild this match", body(13.0), DARK_TEXT2);
            }
        }
        // ---- provenance sheet
        if self.viewer.show_manifest {
            self.manifest_sheet(ui, m, board, full, &meta.how);
        }
        if self.viewer.show_export {
            self.export_sheet(ui, m, board, full);
        }
    }

    fn manifest_sheet(&mut self, ui: &mut Ui, m: usize, board: &str, full: Rect, how: &str) {
        let p = self.lib.entries[m].dir.join("boards").join(format!("{board}.manifest.json"));
        let v: serde_json::Value = std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let sheet = Rect::from_min_max(egui::pos2(full.left(), full.top() + full.height() * 0.35), full.max);
        ui.painter().rect_filled(sheet, egui::CornerRadius { nw: 22, ne: 22, sw: 0, se: 0 }, ROOM_CARD_HI);
        ui.painter().line_segment([sheet.left_top() + Vec2::new(sheet.width() / 2.0 - 24.0, 10.0), sheet.left_top() + Vec2::new(sheet.width() / 2.0 + 24.0, 10.0)], Stroke::new(4.0, ROOM_LINE));
        let mut su = ui.new_child(egui::UiBuilder::new().max_rect(sheet.shrink(16.0)));
        su.add_space(12.0);
        su.horizontal(|ui| {
            ui.label(RichText::new("Provenance").font(head(24.0)).color(WHITE));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if chip(ui, "Close", false, true) {
                    self.viewer.show_manifest = false;
                }
            });
        });
        ScrollArea::vertical().id_salt("manifest").show(&mut su, |ui| {
            let row = |ui: &mut Ui, k: &str, val: String| {
                ui.label(RichText::new(k.to_uppercase()).font(bold(12.0)).color(DARK_TEXT2));
                ui.label(RichText::new(val).font(body(13.0)).color(WHITE));
            };
            if !how.is_empty() {
                ui.label(RichText::new("HOW TO READ").font(bold(12.0)).color(DARK_TEXT2));
                ui.label(RichText::new(how).font(body(14.0)).color(WHITE));
                ui.label(RichText::new("Pinch to zoom, tap a mark for details, tap a legend entry to hide it.").font(body(12.0)).color(SECONDARY));
                ui.add_space(8.0);
            }
            row(ui, "board", v["board"].as_str().unwrap_or(board).to_string());
            row(ui, "metric_version", v["metric_version"].as_str().unwrap_or("?").to_string());
            if let Some(o) = v["models"].as_object() {
                for (k, x) in o {
                    row(ui, k, x.as_str().unwrap_or("").to_string());
                }
            }
            row(ui, "sources used", v["sources_used"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default());
            row(ui, "built at", v["built_at"].as_str().unwrap_or("").to_string());
            ui.label(RichText::new("FALLBACKS").font(bold(12.0)).color(DARK_TEXT2));
            for f in v["fallbacks"].as_array().cloned().unwrap_or_default() {
                ui.label(RichText::new(format!("• {}", f.as_str().unwrap_or(""))).font(body(12.0)).color(WHITE));
            }
            ui.label(RichText::new("THEME DECISIONS").font(bold(12.0)).color(DARK_TEXT2));
            for f in v["theme"]["decisions"].as_array().cloned().unwrap_or_default() {
                ui.label(RichText::new(format!("• {}", f.as_str().unwrap_or(""))).font(body(12.0)).color(WHITE));
            }
            ui.label(RichText::new("CONSISTENCY GATES").font(bold(12.0)).color(DARK_TEXT2));
            for c in v["checks"].as_array().cloned().unwrap_or_default() {
                let ok = c["ok"].as_bool().unwrap_or(false);
                ui.label(RichText::new(format!("{} {}: {}", if ok { "✓" } else { "✗" }, c["name"].as_str().unwrap_or(""), c["detail"].as_str().unwrap_or(""))).font(body(12.0)).color(if ok { SECONDARY } else { Color32::from_rgb(0xff, 0x6b, 0x81) }));
            }
        });
    }

    /// PNG bytes of a board at 2x, rendered from its SVG.
    fn board_png(&self, m: usize, board: &str) -> Result<Vec<u8>, String> {
        let svg = std::fs::read_to_string(self.board_path(m, board)).map_err(|e| e.to_string())?;
        let (w, h, px) = matchhub::render::rasterize(&svg, 2.0).map_err(|e| e.to_string())?;
        let img = image::RgbaImage::from_raw(w, h, px).ok_or("bad image size")?;
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }

    fn export_name(&self, m: usize, board: &str) -> String {
        format!("{}_{board}.png", self.lib.entries[m].s.id)
    }

    /// Writes the PNG next to the app's data (desktop, and Android older than 10).
    fn export_to_folder(&mut self, ctx: &egui::Context, m: usize, board: &str) {
        #[allow(unused_mut)]
        let mut dir = self.root.join("exports");
        #[cfg(target_os = "android")]
        if let Some(p) = self.android.as_ref().and_then(|a| a.external_data_path()) {
            dir = p.join("exports");
        }
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join(self.export_name(m, board));
        match self.board_png(m, board).and_then(|b| std::fs::write(&out, b).map_err(|e| e.to_string())) {
            Ok(()) => self.toast(ctx, &format!("Saved to {}", out.display())),
            Err(e) => self.toast(ctx, &format!("Export failed: {e}")),
        }
    }

    /// Bottom sheet: save to Pictures (Android MediaStore) or the export folder, share, cancel.
    fn export_sheet(&mut self, ui: &mut Ui, m: usize, board: &str, full: Rect) {
        let sheet_h = 4.0 * 56.0 + 64.0;
        let sheet = Rect::from_min_max(egui::pos2(full.left(), (full.bottom() - sheet_h).max(full.top())), full.max);
        // scrim: tapping outside closes the sheet
        let scrim = Rect::from_min_max(full.min, egui::pos2(full.right(), sheet.top()));
        ui.painter().rect_filled(scrim, 0.0, Color32::from_black_alpha(140));
        if ui.interact(scrim, ui.scope_id().with("export_scrim"), Sense::click()).clicked() {
            self.viewer.show_export = false;
        }
        ui.painter().rect_filled(sheet, egui::CornerRadius { nw: 22, ne: 22, sw: 0, se: 0 }, ROOM_CARD_HI);
        ui.painter().line_segment([sheet.left_top() + Vec2::new(sheet.width() / 2.0 - 24.0, 10.0), sheet.left_top() + Vec2::new(sheet.width() / 2.0 + 24.0, 10.0)], Stroke::new(4.0, ROOM_LINE));
        let mut su = ui.new_child(egui::UiBuilder::new().max_rect(sheet.shrink2(Vec2::new(16.0, 0.0))));
        su.add_space(24.0);
        let w = su.available_width();
        let row = |ui: &mut Ui, label: &str, hint: &str| -> bool {
            let r = ui.add(egui::Button::new(RichText::new(label).font(semi(16.0)).color(PRINT)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).min_size(Vec2::new(w, 48.0)));
            ui.label(RichText::new(hint).font(body(12.0)).color(DARK_TEXT2));
            r.clicked()
        };
        let ctx = ui.ctx().clone();
        let android = cfg!(target_os = "android");
        if row(&mut su, if android { "Save to Pictures" } else { "Save to exports folder" }, if android { "Pictures/DataHub, shows up in your gallery" } else { "PNG at 2x" }) {
            self.viewer.show_export = false;
            #[cfg(target_os = "android")]
            {
                let name = self.export_name(m, board);
                let saved = self.board_png(m, board).ok().and_then(|png| self.android.as_ref().and_then(|a| crate::android_bridge::save_png_to_pictures(a, &name, &png)));
                match saved {
                    Some(_) => self.toast(&ctx, "Saved to Pictures/DataHub"),
                    None => self.export_to_folder(&ctx, m, board),
                }
            }
            #[cfg(not(target_os = "android"))]
            self.export_to_folder(&ctx, m, board);
        }
        if android && row(&mut su, "Share…", "Send it to Drive, Files, a chat or any other app") {
            self.viewer.show_export = false;
            #[cfg(target_os = "android")]
            {
                let name = self.export_name(m, board);
                let ok = self.board_png(m, board).ok().and_then(|png| self.android.as_ref().and_then(|a| crate::android_bridge::save_png_to_pictures(a, &name, &png).and_then(|uri| crate::android_bridge::share_png(a, &uri))));
                if ok.is_none() {
                    self.toast(&ctx, "Could not open the share sheet");
                }
            }
        }
        if row(&mut su, "Cancel", "") {
            self.viewer.show_export = false;
        }
    }
}
