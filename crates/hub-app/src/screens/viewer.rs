//! Screen 4: full-screen board viewer with pinch-zoom, pan, double-tap to fit,
//! scope switcher, how-to-read line, export and provenance sheet.

use super::scope_variants;
use crate::style::*;
use crate::{App, Screen};
use egui::{Align, Color32, Layout, Rect, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};

/// Height available to the board canvas for a window of the given height.
fn canvas_h_hint(window_h: f32) -> f32 {
    (window_h - 140.0 - 8.0).max(100.0)
}

impl App {
    pub fn board_viewer(&mut self, ui: &mut Ui, m: usize, board: &str) {
        if m >= self.lib.entries.len() {
            self.go_home();
            return;
        }
        let e = self.lib.entries[m].clone();
        let Some(meta) = e.s.boards.iter().find(|b| b.id == board).cloned() else {
            self.back();
            return;
        };
        let full = ui.max_rect();
        // ---- chrome top
        let mut top = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(full.min, Vec2::new(full.width(), 140.0))));
        egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 2)).show(&mut top, |ui| {
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("←").font(bold(22.0)).color(WHITE)).fill(Color32::TRANSPARENT).min_size(Vec2::splat(48.0))).clicked() {
                    self.back();
                }
                ui.add(egui::Label::new(RichText::new(&meta.title).font(head(if meta.title.chars().count() > 14 { 19.0 } else { 22.0 })).color(WHITE)).truncate());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let fill_zoom = (canvas_h_hint(full.height()) / (full.width() * (meta.height / meta.width) as f32)).clamp(1.0, 3.0);
                    if (meta.height / meta.width) < 1.1 && ui.add(egui::Button::new(RichText::new(if self.viewer.zoom > 1.01 { "Fit" } else { "Fill" }).font(semi(14.0)).color(PRIMARY_ON_DARK)).fill(Color32::TRANSPARENT).min_size(Vec2::splat(48.0))).clicked() {
                        if self.viewer.zoom > 1.01 {
                            self.viewer.zoom = 1.0;
                        } else {
                            self.viewer.zoom = fill_zoom;
                        }
                        self.viewer.pan = Vec2::ZERO;
                    }
                    if ui.add(egui::Button::new(RichText::new("Info").font(semi(14.0)).color(PRIMARY_ON_DARK)).fill(Color32::TRANSPARENT).min_size(Vec2::splat(48.0))).on_hover_text("Provenance").clicked() {
                        self.viewer.show_manifest = !self.viewer.show_manifest;
                    }
                    if ui.add(egui::Button::new(RichText::new("Export").font(semi(14.0)).color(PRIMARY_ON_DARK)).fill(Color32::TRANSPARENT).min_size(Vec2::splat(48.0))).on_hover_text("Export PNG").clicked() {
                        self.export_board(ui.ctx(), m, board);
                    }
                });
            });
            ScrollArea::horizontal().id_salt("scopes").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let vars = scope_variants(&e, board);
                    let has_full = vars.iter().any(|v| v.0 == "FULL MATCH");
                    if !has_full {
                        let _ = chip(ui, "FULL MATCH", false, true);
                    }
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
                        }
                    }
                });
            });
            // subdued fallback notice
            let note = if board.starts_with("22_") || board.starts_with("24_") {
                Some("PSxG from the local placement model where no xGOT was fetched: no shot velocity or keeper position.".to_string())
            } else {
                e.s.fallbacks.iter().find(|f| f.contains("unavailable") || f.contains("skipped")).cloned()
            };
            if let Some(n) = note {
                ui.label(RichText::new(format!("Note: {n}")).font(body(12.0)).color(DARK_TEXT2));
            }
        });
        // ---- board canvas
        let canvas = Rect::from_min_max(egui::pos2(full.left(), full.top() + 140.0), egui::pos2(full.right(), full.bottom() - 8.0));
        let resp = ui.interact(canvas, ui.scope_id().with("canvas"), Sense::click_and_drag());
        let aspect = (meta.height / meta.width) as f32;
        // portrait boards fit the width and scroll; landscape boards fit whole
        let fit_w = if aspect > 1.1 { canvas.width() } else { canvas.width().min(canvas.height() / aspect) };
        // pinch / ctrl-scroll zoom, drag to pan, double tap to fit
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
            }
        }
        if resp.dragged() {
            self.viewer.pan += resp.drag_delta();
        }
        if resp.double_clicked() {
            if self.viewer.zoom > 1.01 {
                self.viewer.zoom = 1.0;
                self.viewer.pan = Vec2::ZERO;
            } else {
                self.viewer.zoom = 2.5;
            }
        }
        let size = Vec2::new(fit_w, fit_w * aspect) * self.viewer.zoom;
        // keep the board on screen
        let max_pan = ((size - canvas.size()) / 2.0).max(Vec2::ZERO);
        if self.viewer.at_top {
            self.viewer.pan = Vec2::new(0.0, max_pan.y);
            self.viewer.at_top = false;
        }
        self.viewer.pan = self.viewer.pan.clamp(-max_pan, max_pan);
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
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        match crate::scene::hit(rect, sc, pos, &self.viewer.hidden) {
                            crate::scene::Hit::Legend(l) => {
                                if !self.viewer.hidden.remove(&l) {
                                    self.viewer.hidden.insert(l);
                                }
                                self.viewer.selected = None;
                            }
                            crate::scene::Hit::Mark(i, t) => {
                                self.viewer.selected = if self.viewer.selected.map(|s| s.0) == Some(i) { None } else { Some((i, t)) };
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
                child.painter().rect_filled(rect, 8.0, Color32::from_rgb(0x15, 0x10, 0x1e));
                child.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Board data missing: rebuild this match", body(13.0), DARK_TEXT2);
            }
        }
        // ---- provenance sheet
        if self.viewer.show_manifest {
            self.manifest_sheet(ui, m, board, full, &meta.how);
        }
    }

    fn manifest_sheet(&mut self, ui: &mut Ui, m: usize, board: &str, full: Rect, how: &str) {
        let p = self.lib.entries[m].dir.join("boards").join(format!("{board}.manifest.json"));
        let v: serde_json::Value = std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let sheet = Rect::from_min_max(egui::pos2(full.left(), full.top() + full.height() * 0.35), full.max);
        ui.painter().rect_filled(sheet, egui::CornerRadius { nw: 22, ne: 22, sw: 0, se: 0 }, Color32::from_rgb(0x1c, 0x16, 0x28));
        ui.painter().line_segment([sheet.left_top() + Vec2::new(sheet.width() / 2.0 - 24.0, 10.0), sheet.left_top() + Vec2::new(sheet.width() / 2.0 + 24.0, 10.0)], Stroke::new(4.0, Color32::from_gray(90)));
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

    fn export_board(&mut self, ctx: &egui::Context, m: usize, board: &str) {
        #[allow(unused_mut)]
        let mut dir = self.root.join("exports");
        #[cfg(target_os = "android")]
        if let Some(p) = self.android.as_ref().and_then(|a| a.external_data_path()) {
            dir = p.join("exports");
        }
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join(format!("{}_{board}.png", self.lib.entries[m].s.id));
        let svg = std::fs::read_to_string(self.board_path(m, board)).unwrap_or_default();
        match matchhub::render::save_png(&svg, &out, 2.0) {
            Ok(()) => self.toast(ctx, &format!("Exported to {}", out.display())),
            Err(e) => self.toast(ctx, &format!("Export failed: {e}")),
        }
    }
}
