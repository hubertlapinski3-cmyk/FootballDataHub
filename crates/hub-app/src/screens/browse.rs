//! Onboarding, home, match list, match hub, report, players, compare, settings.

use super::{boards_by_section, STEPS};
use crate::library::{self, Source};
use matchhub::catalog::{format_metric, metric_index, percentile, season_unit, ScoutIndex, ScoutPlayerRow, ScoutRole, METRICS};
use crate::style::*;
use crate::{App, Screen};
use egui::{Align, Color32, Layout, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};

impl App {
    // ------------------------------------------------------------------ 1 onboarding
    pub fn onboarding(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        backdrop(ui, rect);
        // build the bundled sample in the background so the preview shows a real board
        if self.lib.entries.is_empty() && !self.build.lock().unwrap().running && self.build.lock().unwrap().step.is_empty() {
            self.open_on_done = false;
            library::spawn_build(self.root.clone(), Source::Sample, false, None, None, self.build.clone(), ui.ctx().clone());
        }
        let preview = self.lib.entries.iter().position(|e| e.s.source == "statsbomb").or(if self.lib.entries.is_empty() { None } else { Some(0) });
        ScrollArea::vertical().id_salt("onb").show(ui, |ui| egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 0)).show(ui, |ui| {
            ui.add_space(36.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Data Hub").font(bold(13.0)).color(SECONDARY));
                // the headline gives back size as the text-size setting grows, so its longest word never breaks mid-word
                let k = (1.2 / (text_scale_pct() as f32 / 100.0)).min(1.0);
                ui.label(RichText::new("Every match, read like a broadcast").font(head(40.0 * k)).color(WHITE));
                ui.add_space(4.0);
                ui.label(RichText::new("31 analysis boards and a written report from one WhoScored match").font(body(15.0)).color(DARK_TEXT2));
            });
            ui.add_space(20.0);
            egui::Frame::new().fill(BOARD_BG).corner_radius(20).inner_margin(12).stroke(Stroke::new(1.0, ROOM_LINE)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let w = ui.available_width();
                let h = w * 1130.0 / 1600.0;
                match preview {
                    Some(i) => {
                        self.draw_board(ui, i, "01_xg_flow", w, false, true);
                    }
                    None => {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                        ui.painter().rect_filled(r, 10.0, ROOM_CARD);
                        let st = self.build.lock().unwrap().clone();
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("Preparing a sample board… {}", st.step), body(13.0), DARK_TEXT2);
                    }
                }
                let cap = preview.map(|i| { let s = &self.lib.entries[i].s; format!("Sample board: xG flow · {} {}–{} {} · {}", s.home, s.score[0], s.score[1], s.away, s.competition) }).unwrap_or_else(|| "Sample board: xG flow".into());
                ui.label(RichText::new(cap).font(body(12.0)).color(DARK_TEXT2));
            });
            ui.add_space(16.0);
            ui.label(RichText::new("How the win model works").font(head(22.0)).color(WHITE));
            ui.label(RichText::new("Every shot is replayed at its expected-goals value thousands of times to price the result, and the live win curve is a transparent heuristic built only from the score, the time left and each side's xG so far. It is not a betting model, and every board lists the models, sources and fallbacks behind it.").font(body(14.5)).color(DARK_TEXT2));
            ui.add_space(24.0);
            ui.vertical_centered_justified(|ui| {
                if cta(ui, "Get started", true) {
                    self.prefs.onboarded = true;
                    self.save_prefs();
                    self.open_on_done = true;
                    self.go_home();
                }
            });
            ui.add_space(24.0);
        }));
    }

    // ------------------------------------------------------------------ 2 home
    pub fn home(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().id_salt("home").show(ui, |ui| {
            ui.set_max_width(ui.available_width()); // wrap to the viewport, never widen it
            egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
                ui.label(RichText::new("Data Hub").font(head(34.0)).color(fg()));
                ui.label(RichText::new("Paste a WhoScored match ID or URL").font(body(13.5)).color(sub()));
                let running = self.build.lock().unwrap().running;
                ui.horizontal_wrapped(|ui| {
                    let pill_w = 92.0 * text_scale_pct() as f32 / 100.0;
                    // narrow screens: the field takes the full row and the Paste pill wraps beneath it
                    let fw = if ui.available_width() < 340.0 { ui.available_width() } else { ui.available_width() - pill_w - 8.0 };
                    ui.add(egui::TextEdit::singleline(&mut self.input).hint_text("e.g. 1650630 or https://www.whoscored.com/Matches/…").desired_width(fw.max(80.0)).min_size(Vec2::new(0.0, 48.0)).font(body(15.0)));
                    if let Some(t) = self.paste_pill(ui) {
                        self.input = t.trim().to_string();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    let ok = !self.input.trim().is_empty() && !running;
                    if cta(ui, "Build match", ok) {
                        let s = self.input.trim().to_string();
                        let is_file = std::path::Path::new(&s).exists();
                        if !is_file && !self.prefs.online() {
                            self.toast(ui.ctx(), "Online fetching is off. Set a scrape relay or the personal-use flag in Settings, or import a saved page.");
                        } else {
                            self.open_on_done = true;
                            library::spawn_build(self.root.clone(), Source::WhoScored(s), self.prefs.online(), self.prefs.relay(), self.browser(), self.build.clone(), ui.ctx().clone());
                        }
                    }
                    if ghost(ui, "Sample match", lt()) && !running {
                        self.open_on_done = true;
                        library::spawn_build(self.root.clone(), Source::Sample, false, None, None, self.build.clone(), ui.ctx().clone());
                    }
                });
                // saved pages dropped into the import folder
                let imports = self.import_files();
                if !imports.is_empty() {
                    section_title(ui, "Saved pages to import", lt());
                    for f in imports.iter().take(6) {
                        let name = f.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                        if ui.add(egui::Button::new(RichText::new(format!("⤓  {name}")).font(body(14.0))).min_size(Vec2::new(ui.available_width(), 48.0))).clicked() && !running {
                            self.open_on_done = true;
                            library::spawn_build(self.root.clone(), Source::WhoScored(f.display().to_string()), self.prefs.online(), self.prefs.relay(), self.browser(), self.build.clone(), ui.ctx().clone());
                        }
                    }
                } else {
                    ui.label(RichText::new(format!("Saved WhoScored pages (.html / .json / .gz) placed in {} appear here.", self.import_dir().display())).font(body(12.0)).color(sub()));
                }
                ui.add_space(8.0);
                self.build_status(ui, lt());
                if let Some((inp, msg)) = self.failed.clone() {
                    let lower = msg.to_lowercase();
                    let blocked = ["blocked", "403", "cloudflare", "bot wall", "429", "too many requests"].iter().any(|k| lower.contains(k));
                    let offline = !blocked && ["could not reach", "dns", "timed out", "network"].iter().any(|k| lower.contains(k));
                    card(lt()).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(if blocked { "The site blocked the request" } else if offline { "Could not reach the site" } else { "Build failed" }).font(bold(14.0)).color(if lt() { ERROR_ROOM } else { ERROR_PRINT }));
                        ui.label(RichText::new(msg.chars().take(220).collect::<String>()).font(body(12.0)).color(sub()));
                        ui.horizontal(|ui| {
                            if ghost(ui, "Retry", lt()) && !running && !inp.trim().is_empty() {
                                self.failed = None;
                                self.open_on_done = true;
                                library::spawn_build(self.root.clone(), Source::WhoScored(inp.trim().to_string()), self.prefs.online(), self.prefs.relay(), self.browser(), self.build.clone(), ui.ctx().clone());
                            }
                            if ghost(ui, "Dismiss", lt()) {
                                self.failed = None;
                            }
                        });
                    });
                }
                section_title(ui, "Latest built matches", lt());
                if self.lib.entries.is_empty() {
                    ui.label(RichText::new("No matches yet. Build one above or try the sample.").color(sub()));
                }
                let mut open = None;
                if self.lib.entries.len() == 1 {
                    // one match: a full-width card, outside any horizontal scroller (which would give it unbounded width)
                    let w = ui.available_width() - 34.0;
                    if self.match_card(ui, 0, w) {
                        open = Some(0);
                    }
                } else {
                    ScrollArea::horizontal().id_salt("latest").show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for i in 0..self.lib.entries.len().min(8) {
                                if self.match_card(ui, i, 280.0) {
                                    open = Some(i);
                                }
                            }
                        });
                    });
                }
                if let Some(i) = open {
                    self.push(Screen::Hub(i));
                }
                section_title(ui, "Leagues", lt());
                self.league_controls(ui, "home", false);
                if self.data.league != 0 {
                    ui.add_space(8.0);
                    self.remote_list(ui, Some(3));
                    if ghost(ui, "All matches", lt()) {
                        self.push(Screen::Matches);
                    }
                }
                section_title(ui, "Quick actions", lt());
                let ncol = if ui.available_width() < 380.0 || self.prefs.text_scale > 115 { 2 } else { 4 };
                let items = [("list", "Matches"), ("scout", "Scout"), ("grid", "Boards"), ("compare", "Compare"), ("gear", "Settings")];
                for chunk in items.chunks(ncol) {
                ui.columns(ncol, |cols| {
                    for (k, (icon, label)) in chunk.iter().enumerate() {
                        let r = card(lt()).show(&mut cols[k], |ui| {
                            ui.vertical_centered(|ui| {
                                let (ir, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                                draw_icon(ui, ir, icon);
                                ui.label(RichText::new(*label).font(semi(12.0)).color(fg()));
                            });
                        });
                        if r.response.interact(Sense::click()).clicked() {
                            match *label {
                                "Matches" => self.push(Screen::Matches),
                                "Boards" => {
                                    if self.lib.entries.is_empty() {
                                        self.toast(cols[k].ctx(), "Build a match first");
                                    } else {
                                        self.push(Screen::Hub(0))
                                    }
                                }
                                "Scout" => self.push(Screen::Scout),
                                "Compare" => self.push(Screen::Compare),
                                _ => self.push(Screen::Settings),
                            }
                        }
                    }
                });
                }
                ui.add_space(20.0);
            });
        });
    }

    pub fn import_dir(&self) -> std::path::PathBuf {
        #[cfg(target_os = "android")]
        if let Some(p) = self.android.as_ref().and_then(|a| a.external_data_path()) {
            return p.join("import");
        }
        self.root.join("import")
    }

    fn import_files(&self) -> Vec<std::path::PathBuf> {
        let d = self.import_dir();
        let _ = std::fs::create_dir_all(&d);
        let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(&d)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "html" || x == "htm" || x == "json" || x == "gz").unwrap_or(false)).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    // ------------------------------------------------------------------ match list
    pub fn matches(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            if top_bar(ui, "Matches", lt()) {
                self.back();
            }
            let mut open = None;
            let mut del = None;
            self.league_controls(ui, "matches", true);
            ui.add_space(8.0);
            if self.data.league != 0 {
                // league matches from the catalog: downloaded on demand, built on the device
                ScrollArea::vertical().id_salt("remote_matches").show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    if self.build.lock().unwrap().running {
                        self.build_status(ui, lt());
                        ui.add_space(8.0);
                    }
                    self.remote_list(ui, None);
                    ui.add_space(24.0);
                });
                return;
            }
            if self.lib.entries.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No matches yet").font(head(26.0)).color(fg()));
                    ui.label(RichText::new("Build a match from Home or open the sample to see its boards here").font(body(14.0)).color(sub()));
                    ui.add_space(12.0);
                    if cta(ui, "Go to Home", true) {
                        self.go_home();
                    }
                });
                return;
            }
            ScrollArea::vertical().id_salt("matches").show(ui, |ui| {
                ui.set_max_width(ui.available_width()); // wrap to the viewport, never widen it
                for i in 0..self.lib.entries.len() {
                    let a = self.stagger(i);
                    ui.set_opacity(a);
                    let w = ui.available_width() - 34.0;
                    if self.match_card(ui, i, w) {
                        open = Some(i);
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} boards · {}", self.lib.entries[i].s.boards.len(), self.lib.entries[i].s.source)).font(body(12.0)).color(sub()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            // destructive: a second tap confirms, Cancel backs out
                            if self.confirm_delete == Some(i) {
                                if ui.add(egui::Button::new(RichText::new("Delete for good").font(semi(14.0)).color(WHITE)).fill(ERROR).corner_radius(24).min_size(Vec2::new(0.0, 48.0))).clicked() {
                                    del = Some(i);
                                    self.confirm_delete = None;
                                }
                                if ghost(ui, "Cancel", lt()) {
                                    self.confirm_delete = None;
                                }
                            } else if ui.add(egui::Button::new(RichText::new("Delete").font(semi(14.0)).color(if lt() { ERROR_ON_DARK } else { ERROR })).fill(Color32::TRANSPARENT).min_size(Vec2::new(64.0, 48.0))).clicked() {
                                self.confirm_delete = Some(i);
                            }
                        });
                    });
                    ui.add_space(8.0);
                }
            });
            if let Some(i) = open {
                self.push(Screen::Hub(i));
            }
            if let Some(i) = del {
                self.lib.delete(i);
                self.raster.clear();
                self.crest_tex.clear();
                self.report_cache = None;
                self.manifest_cache = None;
            }
        });
    }

    // ------------------------------------------------------------------ 3 match hub
    pub fn hub(&mut self, ui: &mut Ui, m: usize) {
        if m >= self.lib.entries.len() {
            self.go_home();
            return;
        }
        let e = self.lib.entries[m].clone();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            if top_bar(ui, "Match hub", true) {
                self.back();
            }
            ScrollArea::vertical().id_salt("hub").show(ui, |ui| {
                ui.set_max_width(ui.available_width()); // wrap to the viewport, never widen it
                self.match_header(ui, m, "Full match");
                ui.add_space(8.0);
                ui.label(RichText::new(&e.s.headline).font(head(24.0)).color(WHITE));
                super::super::style::definition_chip(ui, "box entry followed by a shot = shot later in the same possession", true);
                ui.add_space(4.0);
                self.momentum_strip(ui, m, 70.0);
                // chips are frames, which wrapped layouts cannot measure ahead: chunk rows by width instead
                let f = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or("–".into());
                let chips = [("npxG", format!("{:.2}–{:.2}", e.s.npxg[0], e.s.npxg[1])), ("xPts", format!("{:.1}–{:.1}", e.s.xpts[0], e.s.xpts[1])), ("PPDA", format!("{}–{}", f(e.s.ppda[0]), f(e.s.ppda[1])))];
                let per_row = ((ui.available_width() / (150.0 * text_scale_pct() as f32 / 100.0)).floor() as usize).clamp(1, 3);
                for row in chips.chunks(per_row) {
                    ui.horizontal(|ui| {
                        for (k, v) in row {
                            metric_chip(ui, k, v, true);
                        }
                    });
                }
                ui.add_space(8.0);
                if let Some(i) = tab_row(ui, &["Boards", "Report", "Players", "Compare"], 0) {
                    match i {
                        1 => self.push(Screen::Report(m)),
                        2 => self.push(Screen::Players(m)),
                        3 => {
                            self.compare.left = Some((m, 0));
                            self.compare.right = Some((m, 1));
                            self.compare.players_mode = false;
                            self.push(Screen::Compare);
                        }
                        _ => {}
                    }
                }
                ui.add_space(8.0);
                if !e.s.fallbacks.is_empty() {
                    // one plain line here; the full fallback list lives in the Info sheet
                    let n = e.s.fallbacks.len();
                    let line = if n == 1 { "Using one fallback source; details in Info".to_string() } else { format!("Using {n} fallback sources; details in Info") };
                    ui.label(RichText::new(line).font(body(12.0)).color(DARK_TEXT2));
                }
                let w = ui.available_width();
                // two cards per row; one per row on compact widths or large text so titles stay readable
                let cols = if w / (text_scale_pct() as f32 / 100.0) >= 300.0 { 2usize } else { 1 };
                let tw = (w - 12.0 * (cols as f32 - 1.0)) / cols as f32;
                let mut open_board: Option<String> = None;
                let mut tile_i = 0usize;
                let mut sec_no = 0u32;
                for (sec, boards) in boards_by_section(&e) {
                    sec_no += 1;
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&sec).font(semi(15.0)).color(DARK_TEXT2));
                        // editorial index marker
                        ui.label(RichText::new(format!("No. {sec_no:02}")).font(body(12.0)).color(SAGE));
                    });
                    // titles stay plain: the card badge carries the team, half and scope
                    let label = |b: &library::BoardMeta| b.title.clone();
                    let mut items: Vec<(String, String)> = boards.iter().map(|b| (b.id.clone(), label(b))).collect();
                    if sec == "Players" {
                        items.push(("__players".into(), "Player cards".into()));
                    }
                    for row in items.chunks(cols) {
                        let a = self.stagger(tile_i);
                        tile_i += cols;
                        ui.horizontal_top(|ui| {
                            ui.set_opacity(a);
                            for (id, title) in row {
                                if id == "__players" {
                                    if self.players_card(ui, m, tw, e.s.players.len()).clicked() {
                                        self.push(Screen::Players(m));
                                    }
                                } else {
                                    let badge = board_badge(&e, id);
                                    if self.board_card(ui, m, id, tw, title, &badge).clicked() {
                                        open_board = Some(id.clone());
                                    }
                                }
                            }
                        });
                    }
                }
                if let Some(b) = open_board {
                    self.push(Screen::Viewer { m, board: b });
                }
                ui.add_space(24.0);
            });
        });
    }

    // ------------------------------------------------------------------ 5 report
    pub fn report(&mut self, ui: &mut Ui, m: usize) {
        if self.report_cache.as_ref().map(|r| r.0) != Some(m) {
            let p = self.lib.entries[m].dir.join("report.json");
            match std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str(&s).ok()) {
                Some(r) => self.report_cache = Some((m, std::sync::Arc::new(r))),
                None => {
                    self.toast(ui.ctx(), "Report missing for this match");
                    self.back();
                    return;
                }
            }
        }
        let rep = self.report_cache.as_ref().unwrap().1.clone();
        let e = self.lib.entries[m].clone();
        let (hc, ac) = (self.team_col_light(m, 0), self.team_col_light(m, 1));
        egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 4)).show(ui, |ui| {
            if top_bar(ui, "Match report", lt()) {
                self.back();
            }
            ScrollArea::vertical().id_salt("report").show(ui, |ui| {
                ui.set_max_width(ui.available_width()); // wrap to the viewport, never widen it
                ui.label(RichText::new(&rep.headline).font(head(34.0)).color(fg()));
                rich_paragraph(ui, &rep.standfirst, &e.s.home, &e.s.away, hc, ac, 16.0);
                ui.add_space(8.0);
                for sec in &rep.sections {
                    ui.add_space(8.0);
                    ui.label(RichText::new(&sec.title).font(head(24.0)).color(fg()));
                    let (r, _) = ui.allocate_exact_size(Vec2::new(48.0, 3.0), Sense::hover());
                    ui.painter().rect_filled(r, 1.0, PRIMARY);
                    let guide = sec.title == "Complete visual guide";
                    for p in &sec.paragraphs {
                        rich_paragraph(ui, p, &e.s.home, &e.s.away, hc, ac, if guide { 13.0 } else { 15.0 });
                    }
                    if !sec.boards.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for b in &sec.boards {
                                let title = e.s.boards.iter().find(|x| &x.id == b).map(|x| x.title.clone()).unwrap_or(b.clone());
                                if ghost(ui, &format!("Open board · {title}"), lt()) {
                                    self.push(Screen::Viewer { m, board: b.clone() });
                                }
                            }
                        });
                    }
                }
                ui.add_space(32.0);
            });
        });
    }

    // ------------------------------------------------------------------ 6 players
    pub fn players(&mut self, ui: &mut Ui, m: usize) {
        let e = self.lib.entries[m].clone();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            if top_bar(ui, "Players", true) {
                self.back();
            }
            // two tabs must both stay visible: the long labels give way to short ones at large text or on narrow screens
            let short = ui.available_width() < 360.0 || text_scale_pct() > 125;
            if let Some(i) = tab_row(ui, if short { &["Squad", "Scout"] } else { &["Match squad", "League scout"] }, self.data.scout_on as usize) {
                self.data.scout_on = i == 1;
            }
            if self.data.scout_on {
                ScrollArea::vertical().id_salt("players_scout").show(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    self.scout_view(ui);
                    ui.add_space(24.0);
                });
                return;
            }
            ScrollArea::vertical().id_salt("players").show(ui, |ui| {
                ui.set_max_width(ui.available_width()); // wrap to the viewport, never widen it
                for side in 0..2 {
                    let col = self.team_col(m, side);
                    ui.add_space(8.0);
                    ui.label(RichText::new(if side == 0 { e.s.home.clone() } else { e.s.away.clone() }).font(head(22.0)).color(col));
                    let mut ps: Vec<&library::PlayerMeta> = e.s.players.iter().filter(|p| p.side == side).collect();
                    ps.sort_by(|a, b| b.minutes.partial_cmp(&a.minutes).unwrap());
                    for (k, p) in ps.into_iter().enumerate() {
                        let a = self.stagger(k);
                        ui.set_opacity(1.0);
                        let r = glass().show(ui, |ui| {
                            ui.set_opacity(a);
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (rr, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                                ui.painter().circle_filled(rr.center(), 17.0, col);
                                ui.painter().text(rr.center(), egui::Align2::CENTER_CENTER, p.shirt.to_string(), bold(13.0), on(col));
                                // text column takes what is left after the value; on narrow screens the value moves under the text
                                let v = p.vaep;
                                let value = RichText::new(format!("{}{:.2}", if v >= 0.0 { "▲ +" } else { "▼ " }, v)).font(head(18.0)).color(if v >= 0.0 { SECONDARY } else { ERROR_ROOM });
                                let reserve = 96.0 * text_scale_pct() as f32 / 100.0;
                                let narrow = ui.available_width() - reserve < 170.0;
                                let col_w = if narrow { ui.available_width() } else { ui.available_width() - reserve };
                                ui.allocate_ui_with_layout(Vec2::new(col_w, 0.0), Layout::top_down(Align::Min), |ui| {
                                    ui.set_max_width(col_w);
                                    ui.label(RichText::new(&p.name).font(semi(15.0)).color(WHITE));
                                    ui.label(RichText::new(format!("{} · {:.0} min · {} touches · {} shots", p.role, p.minutes, p.touches, p.shots)).font(body(12.0)).color(DARK_TEXT2));
                                    if narrow {
                                        ui.label(value.clone());
                                    }
                                });
                                if !narrow {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(value);
                                    });
                                }
                            });
                        });
                        let resp = r.response.interact(Sense::click());
                        press_overlay(ui, &resp, 16.0, true);
                        if resp.clicked() {
                            self.push(Screen::Player { m, id: p.id });
                        }
                    }
                }
                ui.add_space(20.0);
            });
        });
    }

    pub fn player(&mut self, ui: &mut Ui, m: usize, id: u64) {
        let e = self.lib.entries[m].clone();
        let Some(p) = e.s.players.iter().find(|p| p.id == id).cloned() else {
            self.back();
            return;
        };
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            if top_bar(ui, &p.short, true) {
                self.back();
            }
            ScrollArea::vertical().id_salt("player").show(ui, |ui| {
                ui.set_max_width(ui.available_width()); // wrap to the viewport, never widen it
                // name, scoreboard, key figures and sections all live in the card itself: shown once
                if p.role == "Goalkeeper" {
                    if ghost(ui, "Open goalkeeper goal frames", true) {
                        self.push(Screen::Viewer { m, board: "22_goalkeeper_frames".into() });
                    }
                }
                if ghost(ui, "Compare player", true) {
                    self.open_player_compare(m, &p);
                }
                let w = ui.available_width();
                if self.draw_board(ui, m, &p.board, w, false, true).clicked() {
                    self.push(Screen::Viewer { m, board: p.board.clone() });
                }
                ui.label(RichText::new("Tap the card to open it full screen, zoom and inspect each wedge").font(body(12.0)).color(DARK_TEXT2));
                ui.add_space(20.0);
            });
        });
    }

    // ------------------------------------------------------------------ 7 compare
    pub fn compare_screen(&mut self, ui: &mut Ui) {
        let teams: Vec<(usize, usize, String)> = self.lib.entries.iter().enumerate().flat_map(|(i, e)| (0..2).map(move |s| (i, s, format!("{} ({} {}–{} {}, {})", if s == 0 { &e.s.home } else { &e.s.away }, e.s.home_short, e.s.score[0], e.s.score[1], e.s.away_short, e.s.date)))).collect();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            if top_bar(ui, "Compare", true) {
                self.back();
            }
            if teams.len() < 2 {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Nothing to compare yet").font(head(26.0)).color(WHITE));
                    ui.label(RichText::new("Compare needs one built match: it lines up its two teams side by side").font(body(14.0)).color(DARK_TEXT2));
                    ui.add_space(12.0);
                    if cta(ui, "Go to Home", true) {
                        self.go_home();
                    }
                });
                return;
            }
            if self.compare.left.is_none() {
                self.compare.left = Some((teams[0].0, teams[0].1));
                self.compare.right = Some((teams[1].0, teams[1].1));
            }
            ScrollArea::vertical().id_salt("compare_screen").show(ui, |ui| {
                ui.set_max_width(ui.available_width()); // wrap to the viewport, never widen it
                if let Some(i) = tab_row(ui, &["Teams", "Players"], self.compare.players_mode as usize) {
                    self.compare.players_mode = i == 1;
                }
                if self.compare.players_mode {
                    self.compare_players(ui);
                    return;
                }
                for (k, label) in [(0, "Left team"), (1, "Right team")] {
                    let cur = if k == 0 { self.compare.left } else { self.compare.right };
                    let cur_label = teams.iter().find(|t| Some((t.0, t.1)) == cur).map(|t| t.2.clone()).unwrap_or_default();
                    let col = cur.map(|(m, s)| self.team_col(m, s)).unwrap_or(WHITE);
                    ui.label(RichText::new(label).font(bold(12.0)).color(DARK_TEXT2));
                    egui::ComboBox::from_id_salt(("cmp", k)).selected_text(RichText::new(&cur_label).color(col)).width(ui.available_width()).show_ui(ui, |ui| {
                        for t in &teams {
                            let sel = Some((t.0, t.1)) == cur;
                            if ui.selectable_label(sel, &t.2).clicked() {
                                if k == 0 {
                                    self.compare.left = Some((t.0, t.1));
                                } else {
                                    self.compare.right = Some((t.0, t.1));
                                }
                                self.compare.ran = false;
                                self.compare.swipe = 0.0;
                            }
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    for (i, g) in ["Attack", "Passing", "Pressing", "Defence"].iter().enumerate() {
                        if chip(ui, g, self.compare.groups[i], true) {
                            self.compare.groups[i] = !self.compare.groups[i];
                        }
                    }
                });
                if !self.compare.ran {
                    ui.add_space(8.0);
                    egui::Frame::new().fill(ROOM_CARD_HI).corner_radius(24).inner_margin(8).show(ui, |ui| {
                        ui.spacing_mut().slider_width = ui.available_width() - 20.0;
                        ui.visuals_mut().selection.bg_fill = SECONDARY_VARIANT;
                        ui.add(egui::Slider::new(&mut self.compare.swipe, 0.0..=1.0).show_value(false).trailing_fill(true));
                    });
                    ui.vertical_centered(|ui| ui.label(RichText::new("Swipe to run →").font(semi(13.0)).color(SECONDARY)));
                    if self.compare.swipe >= 0.98 {
                        self.compare.ran = true;
                    }
                    return;
                }
                let (Some(l), Some(r)) = (self.compare.left, self.compare.right) else { return };
                let (el, er) = (self.lib.entries[l.0].clone(), self.lib.entries[r.0].clone());
                let (tl, tr) = (&el.s.teams[l.1], &er.s.teams[r.1]);
                let (cl, cr) = (self.team_col(l.0, l.1), self.team_col(r.0, r.1));
                ui.add_space(8.0);
                ui.set_max_width(ui.available_width() - 26.0);
                versus_header(ui, &tl.name, cl, &tr.name, cr, 22.0);
                let ratio = |x: (usize, usize)| format!("{}/{}", x.0, x.1);
                let mut rows: Vec<(&str, String, String, f64, f64, Option<&str>)> = vec![];
                if self.compare.groups[0] {
                    rows.extend([
                        ("Goals", tl.goals.to_string(), tr.goals.to_string(), tl.goals as f64, tr.goals as f64, None),
                        ("xG", format!("{:.2}", tl.xg), format!("{:.2}", tr.xg), tl.xg, tr.xg, None),
                        ("xGOT", format!("{:.2}", tl.xgot), format!("{:.2}", tr.xgot), tl.xgot, tr.xgot, None),
                        ("Shots", tl.shots.to_string(), tr.shots.to_string(), tl.shots as f64, tr.shots as f64, None),
                        ("Big chances", tl.big_chances.to_string(), tr.big_chances.to_string(), tl.big_chances as f64, tr.big_chances as f64, Some("big chance = Opta big chance or npxG ≥ 0.12")),
                        ("Box entries", tl.box_entries.to_string(), tr.box_entries.to_string(), tl.box_entries as f64, tr.box_entries as f64, Some("completed pass/carry into the box, corners excluded")),
                    ]);
                }
                if self.compare.groups[1] {
                    rows.extend([
                        ("Pass xT", format!("{:.2}", tl.pass_xt), format!("{:.2}", tr.pass_xt), tl.pass_xt, tr.pass_xt, None),
                        ("Progressive passes", tl.progressive.to_string(), tr.progressive.to_string(), tl.progressive as f64, tr.progressive as f64, None),
                        ("Line-breaking", ratio(tl.line_breaking), ratio(tr.line_breaking), tl.line_breaking.0 as f64, tr.line_breaking.0 as f64, Some("event-only approximation")),
                        ("Field tilt %", format!("{:.1}", tl.field_tilt * 100.0), format!("{:.1}", tr.field_tilt * 100.0), tl.field_tilt, tr.field_tilt, None),
                    ]);
                }
                if self.compare.groups[2] {
                    let f = |v: Option<f64>| v.map(|x| format!("{x:.2}")).unwrap_or("–".into());
                    rows.extend([
                        ("PPDA", f(tl.ppda), f(tr.ppda), 1.0 / tl.ppda.unwrap_or(99.0), 1.0 / tr.ppda.unwrap_or(99.0), None),
                        ("High regains", tl.high_regains.to_string(), tr.high_regains.to_string(), tl.high_regains as f64, tr.high_regains as f64, Some("open-play win in the attacking 40%")),
                        ("Counterpress", ratio(tl.counterpress), ratio(tr.counterpress), tl.counterpress.0 as f64, tr.counterpress.0 as f64, Some("regain within 5 s of losing the ball")),
                        ("Transitions", tl.transitions.to_string(), tr.transitions.to_string(), tl.transitions as f64, tr.transitions as f64, Some("open-play regain, ≥20 m / final third / shot within 12 s")),
                    ]);
                }
                if self.compare.groups[3] {
                    rows.extend([
                        ("Tackles", tl.tackles.to_string(), tr.tackles.to_string(), tl.tackles as f64, tr.tackles as f64, None),
                        ("Interceptions", tl.interceptions.to_string(), tr.interceptions.to_string(), tl.interceptions as f64, tr.interceptions as f64, None),
                        ("Blocks", tl.blocks.to_string(), tr.blocks.to_string(), tl.blocks as f64, tr.blocks as f64, None),
                        ("Clearances", tl.clearances.to_string(), tr.clearances.to_string(), tl.clearances as f64, tr.clearances as f64, None),
                        ("Recoveries", tl.recoveries.to_string(), tr.recoveries.to_string(), tl.recoveries as f64, tr.recoveries as f64, None),
                        ("Aerials won", ratio(tl.aerials), ratio(tr.aerials), tl.aerials.0 as f64, tr.aerials.0 as f64, None),
                    ]);
                }
                for (label, a, b, fa, fb, def) in rows {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 26.0, 40.0), Sense::hover());
                    let p = ui.painter();
                    let tot = (fa + fb).max(1e-9) as f32;
                    let mid = rect.center().x;
                    let bw = rect.width() * 0.22;
                    p.text(egui::pos2(rect.left(), rect.center().y), egui::Align2::LEFT_CENTER, &a, head(19.0), WHITE);
                    p.text(egui::pos2(rect.right(), rect.center().y), egui::Align2::RIGHT_CENTER, &b, head(19.0), WHITE);
                    p.text(egui::pos2(mid, rect.top() + 10.0), egui::Align2::CENTER_CENTER, label, semi(11.5), DARK_TEXT2);
                    let wl = bw * fa as f32 / tot;
                    let wr = bw * fb as f32 / tot;
                    p.rect_filled(egui::Rect::from_min_size(egui::pos2(mid - 6.0 - wl, rect.top() + 22.0), Vec2::new(wl, 8.0)), 3.0, cl);
                    p.rect_filled(egui::Rect::from_min_size(egui::pos2(mid + 6.0, rect.top() + 22.0), Vec2::new(wr, 8.0)), 3.0, cr);
                    // win/loss decoded by arrow as well as colour
                    if (fa - fb).abs() > 1e-9 {
                        let (x, txt) = if fa > fb { (mid - 10.0 - wl - 8.0, "▲") } else { (mid + 10.0 + wr + 8.0, "▲") };
                        // the arrow gives way to the figures when large text leaves no room for it
                        let (lw, rw) = (p.layout_no_wrap(a.clone(), head(19.0), WHITE).size().x, p.layout_no_wrap(b.clone(), head(19.0), WHITE).size().x);
                        if x - 6.0 > rect.left() + lw + 4.0 && x + 6.0 < rect.right() - rw - 4.0 {
                            p.text(egui::pos2(x, rect.top() + 26.0), egui::Align2::CENTER_CENTER, txt, body(9.0), SECONDARY);
                        }
                    }
                    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, DARK_LINE));
                    if let Some(d) = def {
                        definition_chip(ui, d, true);
                    }
                }
                if self.compare.groups[2] {
                    let band = |v: Option<f64>| match v {
                        Some(v) if v < 8.0 => "Elite press (<8)",
                        Some(v) if v <= 12.0 => "Medium block (8 to 12)",
                        Some(_) => "Low block (>12)",
                        None => "No data",
                    };
                    ui.columns(2, |c| {
                        c[0].label(RichText::new(band(tl.ppda)).font(bold(12.0)).color(cl));
                        c[1].with_layout(Layout::right_to_left(Align::Min), |ui| ui.label(RichText::new(band(tr.ppda)).font(bold(12.0)).color(cr)));
                    });
                }
                section_title(ui, "Shot maps side by side", true);
                let w = (ui.available_width() - 10.0) / 2.0;
                ui.horizontal(|ui| {
                    for (mi, si) in [l, r] {
                        let id = format!("07_shot_map_{}", if si == 0 { "home" } else { "away" });
                        if self.draw_board(ui, mi, &id, w, true, false).clicked() {
                            self.push(Screen::Viewer { m: mi, board: id });
                        }
                    }
                });
                if ghost(ui, "Reset", true) {
                    self.compare.ran = false;
                    self.compare.swipe = 0.0;
                }
                ui.add_space(20.0);
            });
        });
    }

    fn open_player_compare(&mut self, m: usize, p: &library::PlayerMeta) {
        let e = self.lib.entries[m].clone();
        // opponent: same role on the other side first, then that side's most-used player
        let opp = e.s.players.iter().filter(|q| q.side != p.side).max_by(|a, b| (a.role == p.role).cmp(&(b.role == p.role)).then(a.minutes.total_cmp(&b.minutes))).map(|q| q.id);
        self.compare.players_mode = true;
        self.compare.pl_left = Some((m, p.id));
        self.compare.pl_right = opp.map(|id| (m, id));
        self.push(Screen::Compare);
    }

    /// Two players side by side: pickers, a dual profile radar and grouped comparison bars.
    fn compare_players(&mut self, ui: &mut Ui) {
        use super::scout::SCOUT_KEY;
        let ix = self.data.index.clone();
        let all: Vec<(usize, library::PlayerMeta)> = self.lib.entries.iter().enumerate().flat_map(|(m, e)| e.s.players.iter().map(move |p| (m, p.clone()))).collect();
        // a pick is a built-match player (match index, player id) or a scouting-index player (SCOUT_KEY, id)
        let find = |key: Option<(usize, u64)>| -> Option<CmpSrc> {
            let (m, id) = key?;
            if m == SCOUT_KEY {
                return ix.as_ref()?.find(id).cloned().map(CmpSrc::Scout);
            }
            all.iter().find(|(mi, p)| *mi == m && p.id == id).map(|(mi, p)| CmpSrc::Match(*mi, p.clone()))
        };
        // defaults: the most-used player on each side
        let top = |side: usize| all.iter().filter(|(_, p)| p.side == side).max_by(|a, b| a.1.minutes.total_cmp(&b.1.minutes)).map(|(m, p)| (*m, p.id));
        if find(self.compare.pl_left).is_none() {
            self.compare.pl_left = top(0);
        }
        if find(self.compare.pl_right).is_none() {
            self.compare.pl_right = top(1);
        }
        // league players: found by name or club, then offered in the pickers
        let mut scout_hits: Vec<ScoutPlayerRow> = vec![];
        if let Some(ixr) = &ix {
            ui.label(RichText::new("Search league players").font(bold(12.0)).color(DARK_TEXT2));
            ui.add_sized([ui.available_width() - 26.0, 48.0], egui::TextEdit::singleline(&mut self.data.c_query).hint_text(RichText::new("Name or club").color(DARK_TEXT2)).font(body(15.0)));
            let q = self.data.c_query.trim().to_lowercase();
            if q.len() >= 2 {
                let mut hits: Vec<&ScoutPlayerRow> = ixr.players.iter().filter(|p| p.name.to_lowercase().contains(&q) || p.team.to_lowercase().contains(&q)).collect();
                hits.sort_by(|a, b| b.minutes.partial_cmp(&a.minutes).unwrap());
                scout_hits = hits.into_iter().take(25).cloned().collect();
            }
            ui.add_space(8.0);
        }
        let label_of = |s: &CmpSrc| match s {
            CmpSrc::Match(_, p) => format!("#{} {} · {} ({:.0}')", p.shirt, p.name, p.role, p.minutes),
            CmpSrc::Scout(r) => format!("{} · {} · {} ({:.0}')", r.short, r.team, role_text(r.role), r.minutes),
        };
        for (k, title) in [(0usize, "Left player"), (1, "Right player")] {
            let cur = if k == 0 { self.compare.pl_left } else { self.compare.pl_right };
            let cur_p = find(cur);
            let col = match &cur_p {
                Some(CmpSrc::Match(m, p)) => self.team_col(*m, p.side),
                _ => if k == 0 { SAGE } else { WHITE },
            };
            let text = cur_p.as_ref().map(&label_of).unwrap_or_default();
            ui.label(RichText::new(title).font(bold(12.0)).color(DARK_TEXT2));
            egui::ComboBox::from_id_salt(("cmpp", k)).selected_text(RichText::new(text).color(col)).width(ui.available_width()).show_ui(ui, |ui| {
                for (m, p) in &all {
                    let key = Some((*m, p.id));
                    if ui.selectable_label(key == cur, label_of(&CmpSrc::Match(*m, p.clone()))).clicked() {
                        if k == 0 { self.compare.pl_left = key } else { self.compare.pl_right = key }
                    }
                }
                for r in &scout_hits {
                    let key = Some((SCOUT_KEY, r.id));
                    if ui.selectable_label(key == cur, label_of(&CmpSrc::Scout(r.clone()))).clicked() {
                        if k == 0 { self.compare.pl_left = key } else { self.compare.pl_right = key }
                    }
                }
            });
        }
        let (Some(l), Some(r)) = (find(self.compare.pl_left), find(self.compare.pl_right)) else { return };
        // team colours for match players; league players take the two accents
        let colour = |s: &CmpSrc, slot: usize| match s {
            CmpSrc::Match(m, p) => self.team_col(*m, p.side),
            CmpSrc::Scout(_) => if slot == 0 { SAGE } else { WHITE },
        };
        let (cl, cr) = (colour(&l, 0), colour(&r, 1));
        let (pl, pr) = (CmpP::new(&l), CmpP::new(&r));
        ui.add_space(8.0);
        versus_header(ui, &pl.short, cl, &pr.short, cr, 20.0);
        // radar: role percentiles of the season pool when there is a scouting index, else per 90 on a shared scale
        let (fa, fb, pct_mode) = match &ix {
            Some(ixr) => (pl.fractions(ixr), pr.fractions(ixr), true),
            None => {
                let (va, vb) = (pl.axis_values(), pr.axis_values());
                let max: Vec<f64> = (0..8).map(|i| if i == 4 { 1.0 } else { va[i].max(vb[i]).max(1e-9) }).collect();
                let f = |v: [f64; 8]| -> [f64; 8] { std::array::from_fn(|i| (v[i] / max[i]).clamp(0.0, 1.0)) };
                (f(va), f(vb), false)
            }
        };
        section_title(ui, if pct_mode { "Role percentile per 90" } else { "Profile per 90 minutes" }, true);
        dual_radar(ui, [&format!("{} · {}", pl.short, pl.role_text), &format!("{} · {}", pr.short, pr.role_text)], fa, fb, cl, cr);
        let both_match = matches!((&l, &r), (CmpSrc::Match(..), CmpSrc::Match(..)));
        if !both_match {
            ui.label(RichText::new("All figures are per 90 minutes").font(body(12.0)).color(DARK_TEXT2));
        }
        let groups = ["Attacking & Shooting", "Passing & Progression", "Involvement & Defending"];
        for (g, name) in groups.iter().enumerate() {
            section_title(ui, name, true);
            let rows = match (&l, &r) {
                (CmpSrc::Match(_, a), CmpSrc::Match(_, b)) => group_rows(a, b, g),
                _ => group_rows_per90(&pl.vals, &pr.vals, g),
            };
            for (label, a, b, fa, fb) in rows {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 26.0, 68.0), Sense::hover());
                let p = ui.painter();
                let tot = (fa + fb).max(1e-9) as f32;
                let mid = rect.center().x;
                let bw = rect.width() * 0.22;
                // label on its own line so long names never meet the figures
                p.text(egui::pos2(mid, rect.top() + 4.0), egui::Align2::CENTER_TOP, label, semi(11.5), DARK_TEXT2);
                p.text(egui::pos2(rect.left(), rect.top() + 22.0), egui::Align2::LEFT_TOP, &a, head(19.0), cl);
                p.text(egui::pos2(rect.right(), rect.top() + 22.0), egui::Align2::RIGHT_TOP, &b, head(19.0), cr);
                let wl = bw * fa as f32 / tot;
                let wr = bw * fb as f32 / tot;
                let by = rect.top() + 52.0;
                p.rect_filled(egui::Rect::from_min_size(egui::pos2(mid - 6.0 - wl, by), Vec2::new(wl, 8.0)), 3.0, cl);
                p.rect_filled(egui::Rect::from_min_size(egui::pos2(mid + 6.0, by), Vec2::new(wr, 8.0)), 3.0, cr);
                // the leader is marked by an arrow beside its bar, so the lead reads without colour
                if (fa - fb).abs() > 1e-9 {
                    let x = if fa > fb { mid - 10.0 - wl - 8.0 } else { mid + 10.0 + wr + 8.0 };
                    p.text(egui::pos2(x, by + 4.0), egui::Align2::CENTER_CENTER, "▲", body(9.0), SECONDARY);
                }
                p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, DARK_LINE));
            }
        }
        ui.add_space(20.0);
    }

    // ------------------------------------------------------------------ 8 settings
    pub fn settings(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 4)).show(ui, |ui| {
            if top_bar(ui, "Settings", lt()) {
                self.back();
            }
            ScrollArea::vertical().id_salt("settings").show(ui, |ui| {
                ui.set_max_width(ui.available_width()); // wrap to the viewport, never widen it
                section_title(ui, "Data sources", lt());
                for (n, d) in [
                    ("Opta via WhoScored", "Primary event stream: every on-ball event with coordinates and qualifiers."),
                    ("FotMob · SofaScore", "Team colours, crests, per-shot xG and xGOT (best effort, recorded as fallbacks)."),
                    ("Transfermarkt", "Squad master data where available; ages come from WhoScored."),
                    ("StatsBomb Open Data", "Training data for the in-house xG, PSxG, xT and VAEP models. Attributed on those boards."),
                    ("ClubElo · football-data.co.uk · Understat", "Context and calibration sources supported by the pipeline."),
                ] {
                    ui.label(RichText::new(n).font(semi(14.0)).color(fg()));
                    ui.label(RichText::new(d).font(body(12.5)).color(sub()));
                }
                section_title(ui, "Personal use", lt());
                card(lt()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut v = self.prefs.personal_fetch;
                    if ui.checkbox(&mut v, RichText::new("Allow fetching from WhoScored, FotMob and SofaScore on this device").font(body(14.0))).changed() {
                        self.prefs.personal_fetch = v;
                        self.save_prefs();
                    }
                    ui.label(RichText::new("These sites' terms generally prohibit automated scraping and redistribution. Keep this for personal use, one polite request per match, cached afterwards. If WhoScored shows a bot wall, save the page in a browser and import it instead.").font(body(12.0)).color(sub()));
                });
                section_title(ui, "Scrape relay (GitHub Actions)", lt());
                card(lt()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("WhoScored blocks plain requests, so links are scraped by a headless browser in your repository's scrape-whoscored workflow (built on ricardoherediaj/football-analytics-tutorials). The app reads finished matches from the repo and, with a token, starts the run itself.").font(body(12.5)).color(sub()));
                    ui.label(RichText::new("Repository (owner/name)").font(semi(13.0)));
                    ui.horizontal_wrapped(|ui| {
                        let pill_w = 92.0 * text_scale_pct() as f32 / 100.0;
                        let fw = if ui.available_width() < 340.0 { ui.available_width() } else { ui.available_width() - pill_w - 8.0 };
                        let rr = ui.add(egui::TextEdit::singleline(&mut self.prefs.relay_repo).desired_width(fw.max(80.0)).min_size(Vec2::new(0.0, 48.0)));
                        if rr.gained_focus() {
                            rr.scroll_to_me(Some(Align::Center));
                        }
                        if rr.lost_focus() {
                            self.save_prefs();
                        }
                        if let Some(t) = self.paste_pill(ui) {
                            // accept a pasted GitHub link: keep owner/name
                            let t = t.trim().trim_end_matches('/').trim_end_matches(".git");
                            let t = t.split("github.com/").last().unwrap_or(t);
                            self.prefs.relay_repo = t.split('/').take(2).collect::<Vec<_>>().join("/");
                            self.save_prefs();
                        }
                    });
                    ui.label(RichText::new("Token (fine-grained: Actions read/write + Contents read on that repo only)").font(semi(13.0)));
                    ui.horizontal_wrapped(|ui| {
                        let pill_w = 92.0 * text_scale_pct() as f32 / 100.0;
                        let fw = if ui.available_width() < 340.0 { ui.available_width() } else { ui.available_width() - pill_w - 96.0 };
                        let r = ui.add(egui::TextEdit::singleline(&mut self.prefs.relay_token).password(!self.show_token).desired_width(fw.max(60.0)).min_size(Vec2::new(0.0, 48.0)));
                        if r.gained_focus() {
                            r.scroll_to_me(Some(Align::Center));
                        }
                        if r.lost_focus() {
                            self.save_prefs();
                        }
                        if let Some(t) = self.paste_pill(ui) {
                            self.prefs.relay_token = t.trim().to_string();
                            self.save_prefs();
                        }
                        if chip(ui, if self.show_token { "Hide" } else { "Show" }, false, lt()) {
                            self.show_token = !self.show_token;
                        }
                    });
                    ui.label(RichText::new("Stored only in this app's private storage, never in the repository. Without a token the app still reads matches you scrape by hand (Actions > scrape-whoscored > Run workflow).").font(body(12.0)).color(sub()));
                    if ghost(ui, "Save relay", lt()) {
                        self.save_prefs();
                        self.toast(ui.ctx(), if self.prefs.relay().is_some() { "Relay saved" } else { "Relay disabled (enter owner/name)" });
                    }
                });
                section_title(ui, "League data", lt());
                card(lt()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("The nightly job publishes a catalog of the seven leagues and a season scouting index to the data-catalog branch of the repository above. The app reads it with the same token; matches are downloaded one at a time when you open them.").font(body(12.0)).color(sub()));
                    let (matches, players) = (self.data.catalog.as_ref().map(|c| c.total_matches()).unwrap_or(0), self.data.index.as_ref().map(|i| i.players.len()).unwrap_or(0));
                    ui.label(RichText::new(format!("{matches} matches · {players} players cached")).font(semi(13.0)).color(fg()));
                    let (running, err) = {
                        let j = self.data.job.lock().unwrap();
                        (j.running, j.error.clone())
                    };
                    if let Some(e) = err {
                        ui.label(RichText::new(e.chars().take(200).collect::<String>()).font(body(12.0)).color(sub()));
                    }
                    if ghost(ui, if running { "Updating" } else { "Refresh now" }, lt()) && !running {
                        let ctx = ui.ctx().clone();
                        self.refresh_data(&ctx);
                    }
                });
                section_title(ui, "Theme", lt());
                card(lt()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        for (k, label) in [("dark", "Darkroom"), ("light", "Print")] {
                            if chip(ui, label, self.prefs.theme == k || (k == "dark" && self.prefs.theme != "light"), lt()) {
                                self.prefs.theme = k.to_string();
                                self.save_prefs();
                            }
                        }
                    });
                    ui.label(RichText::new("Darkroom is the dark forest look. Print is warm paper for Home, Matches, Report and Settings; boards always stay in Darkroom.").font(body(12.0)).color(sub()));
                    ui.label(RichText::new("Per screen (overrides the choice above)").font(semi(13.0)).color(fg()));
                    for (key, name) in [("home", "Home"), ("matches", "Matches"), ("report", "Report"), ("settings", "Settings")] {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(name).font(body(14.0)).color(fg()));
                            let cur = self.prefs.screen_theme.get(key).cloned().unwrap_or_default();
                            for (v, label) in [("", "Auto"), ("light", "Light"), ("dark", "Dark")] {
                                if chip(ui, label, cur == v, lt()) {
                                    if v.is_empty() {
                                        self.prefs.screen_theme.remove(key);
                                    } else {
                                        self.prefs.screen_theme.insert(key.to_string(), v.to_string());
                                    }
                                    self.save_prefs();
                                }
                            }
                        });
                    }
                });
                section_title(ui, "Text size", lt());
                card(lt()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        for pct in [0u32, 100, 115, 130, 150] {
                            let label = if pct == 0 { format!("System ({}%)", self.sys_text_scale) } else { format!("{pct}%") };
                            if chip(ui, &label, self.prefs.text_scale == pct, lt()) {
                                self.prefs.text_scale = pct;
                                self.save_prefs();
                            }
                        }
                    });
                    ui.label(RichText::new("System follows Android's font size. Applies to every screen; boards keep their own type and zoom with pinch.").font(body(12.0)).color(sub()));
                });
                section_title(ui, "Motion", lt());
                card(lt()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut v = self.prefs.reduced_motion;
                    if ui.checkbox(&mut v, RichText::new("Reduce motion (no entrance animations on boards and lists)").font(body(14.0))).changed() {
                        self.prefs.reduced_motion = v;
                        self.save_prefs();
                    }
                });
                section_title(ui, "Cache", lt());
                let cache = self.root.join("cache");
                let mdir = self.root.join("matches");
                ui.label(format!("Source cache: {:.1} MB   ·   Built matches: {:.1} MB", library::dir_size(&cache) as f64 / 1e6, library::dir_size(&mdir) as f64 / 1e6));
                ui.horizontal_wrapped(|ui| {
                    if ghost(ui, "Clear source cache", lt()) {
                        let _ = std::fs::remove_dir_all(&cache);
                        self.toast(ui.ctx(), "Source cache cleared");
                    }
                    if ui.add(egui::Button::new(RichText::new("Delete all matches").color(WHITE)).fill(ERROR).corner_radius(24).min_size(Vec2::new(0.0, 48.0))).clicked() {
                        let _ = std::fs::remove_dir_all(&mdir);
                        self.lib.reload();
                        self.raster.clear();
                        self.crest_tex.clear();
                        self.report_cache = None;
                        self.toast(ui.ctx(), "All built matches deleted");
                    }
                });
                section_title(ui, "Theme overrides (theme_overrides.toml)", lt());
                ui.add(egui::TextEdit::multiline(&mut self.theme_text).font(egui::TextStyle::Monospace).desired_rows(10).desired_width(f32::INFINITY));
                if ghost(ui, "Save overrides", lt()) {
                    match toml_check(&self.theme_text) {
                        Ok(()) => {
                            let _ = std::fs::write(self.root.join("theme_overrides.toml"), &self.theme_text);
                            self.toast(ui.ctx(), "Saved. Applies to the next build.");
                        }
                        Err(e) => self.toast(ui.ctx(), &format!("Invalid TOML: {e}")),
                    }
                }
                section_title(ui, "Fallback notes (latest match)", lt());
                if let Some(e) = self.lib.entries.first() {
                    for f in e.s.fallbacks.iter().chain(e.s.theme_decisions.iter()) {
                        ui.label(RichText::new(format!("• {f}")).font(body(12.0)).color(sub()));
                    }
                }
                section_title(ui, "Metric version log", lt());
                ui.label(RichText::new(format!("App metric_version {}", matchhub::METRIC_VERSION)).font(body(12.5)));
                for e in &self.lib.entries {
                    ui.label(RichText::new(format!("{} v {} · metric {} · built {}", e.s.home, e.s.away, e.s.metric_version, &e.s.built_at[..16.min(e.s.built_at.len())])).font(body(12.0)).color(sub()));
                }
                ui.label(RichText::new(format!("Import folder: {}", self.import_dir().display())).font(body(12.0)).color(sub()));
                ui.add_space(32.0);
            });
        });
    }
}

fn toml_check(s: &str) -> Result<(), String> {
    // lightweight validation through the core's TOML parser
    matchhub::theme::parse_overrides(s).map(|_| ()).map_err(|e| e.to_string())
}

/// Paragraph with team names in team colours and numbers highlighted in deep violet.
pub fn rich_paragraph(ui: &mut Ui, text: &str, home: &str, away: &str, hc: Color32, ac: Color32, size: f32) {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    job.wrap.max_width = ui.available_width();
    // body prose follows the theme (Print ink on paper, Paper on Darkroom); numbers use the emerald accent
    let base = TextFormat { font_id: body(size), color: fg(), line_height: Some(size * 1.5), ..Default::default() };
    let num_col = if lt() { SAGE } else { PRIMARY_VARIANT };
    let mut rest = text;
    while !rest.is_empty() {
        let hit = [(home, hc), (away, ac)].iter().filter(|(n, _)| !n.is_empty()).filter_map(|(n, c)| rest.find(*n).map(|i| (i, *n, *c))).min_by_key(|x| x.0);
        let (seg, team) = match hit {
            Some((i, n, c)) => (&rest[..i], Some((n, c))),
            None => (rest, None),
        };
        for tok in seg.split_inclusive(' ') {
            let numeric = tok.chars().any(|c| c.is_ascii_digit());
            let fmt = if numeric { TextFormat { font_id: bold(size), color: num_col, ..base.clone() } } else { base.clone() };
            job.append(tok, 0.0, fmt);
        }
        match team {
            Some((n, c)) => {
                job.append(n, 0.0, TextFormat { font_id: bold(size), color: c, ..base.clone() });
                rest = &rest[seg.len() + n.len()..];
            }
            None => rest = "",
        }
    }
    ui.label(job);
}

#[allow(dead_code)]
fn unused(_: [&str; 6]) {
    let _ = STEPS;
}

fn draw_icon(ui: &Ui, r: egui::Rect, kind: &str) {
    draw_icon_col(ui, r, kind, PRIMARY);
}

/// Simple vector icons (no icon font or emoji needed on device).
pub fn draw_icon_col(ui: &Ui, r: egui::Rect, kind: &str, colr: Color32) {
    let p = ui.painter();
    let st = Stroke::new(2.4, colr);
    let c = r.center();
    match kind {
        "list" => {
            for i in 0..3 {
                let y = r.top() + 7.0 + i as f32 * 7.0;
                p.circle_filled(egui::pos2(r.left() + 5.0, y), 2.2, colr);
                p.line_segment([egui::pos2(r.left() + 11.0, y), egui::pos2(r.right() - 2.0, y)], st);
            }
        }
        "scout" => {
            // magnifier: lens and handle
            p.circle_stroke(egui::pos2(c.x - 2.0, c.y - 2.0), 7.0, st);
            p.line_segment([egui::pos2(c.x + 3.0, c.y + 3.0), egui::pos2(r.right() - 3.0, r.bottom() - 3.0)], st);
        }
        "grid" => {
            for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let a = egui::pos2(r.left() + 3.0 + dx * 12.0, r.top() + 3.0 + dy * 12.0);
                p.rect_filled(egui::Rect::from_min_size(a, Vec2::splat(10.0)), 2.0, colr);
            }
        }
        "home" => {
            let c = r.center();
            p.add(egui::Shape::convex_polygon(vec![egui::pos2(c.x, r.top() + 1.0), egui::pos2(r.right() - 1.0, c.y - 1.0), egui::pos2(r.left() + 1.0, c.y - 1.0)], Color32::TRANSPARENT, st));
            p.rect_stroke(egui::Rect::from_min_max(egui::pos2(r.left() + 4.0, c.y - 1.0), egui::pos2(r.right() - 4.0, r.bottom() - 1.0)), 2.0, st, egui::StrokeKind::Middle);
        }
        "share" => {
            // tray with an arrow leaving it
            p.line_segment([egui::pos2(c.x, r.top() + 2.0), egui::pos2(c.x, c.y + 3.0)], st);
            p.line_segment([egui::pos2(c.x - 5.0, r.top() + 7.0), egui::pos2(c.x, r.top() + 2.0)], st);
            p.line_segment([egui::pos2(c.x + 5.0, r.top() + 7.0), egui::pos2(c.x, r.top() + 2.0)], st);
            p.line_segment([egui::pos2(r.left() + 3.0, c.y + 1.0), egui::pos2(r.left() + 3.0, r.bottom() - 2.0)], st);
            p.line_segment([egui::pos2(r.left() + 3.0, r.bottom() - 2.0), egui::pos2(r.right() - 3.0, r.bottom() - 2.0)], st);
            p.line_segment([egui::pos2(r.right() - 3.0, r.bottom() - 2.0), egui::pos2(r.right() - 3.0, c.y + 1.0)], st);
        }
        "info" => {
            p.circle_stroke(c, r.width() / 2.0 - 2.0, st);
            p.circle_filled(egui::pos2(c.x, c.y - 5.0), 1.8, colr);
            p.line_segment([egui::pos2(c.x, c.y - 1.0), egui::pos2(c.x, c.y + 6.0)], st);
        }
        "compare" => {
            p.line_segment([egui::pos2(r.left() + 3.0, c.y - 5.0), egui::pos2(r.right() - 3.0, c.y - 5.0)], st);
            p.line_segment([egui::pos2(r.right() - 9.0, c.y - 10.0), egui::pos2(r.right() - 3.0, c.y - 5.0)], st);
            p.line_segment([egui::pos2(r.left() + 3.0, c.y + 5.0), egui::pos2(r.right() - 3.0, c.y + 5.0)], st);
            p.line_segment([egui::pos2(r.left() + 9.0, c.y + 10.0), egui::pos2(r.left() + 3.0, c.y + 5.0)], st);
        }
        _ => {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                p.line_segment([c + Vec2::angled(a) * 8.0, c + Vec2::angled(a) * 12.5], Stroke::new(2.4, colr));
            }
            p.circle_stroke(c, 7.5, st);
            p.circle_filled(c, 2.5, colr);
        }
    }
}

/// One-line scope / metric badge for a hub card: who and what the board covers.
/// Two names facing each other across the row: left-aligned and right-aligned inside the content width
/// less the scrollbar gutter, shrunk together when they would meet.
fn versus_header(ui: &mut Ui, left: &str, lc: Color32, right: &str, rc: Color32, size: f32) {
    let w = ui.available_width() - 28.0;
    let width = |ui: &Ui, t: &str, k: f32| ui.painter().layout_no_wrap(t.to_string(), head(size * k), lc).size().x;
    let total = width(ui, left, 1.0) + width(ui, right, 1.0) + 16.0;
    let k = if total > w { (w / total).clamp(0.55, 1.0) } else { 1.0 };
    let h = ui.painter().layout_no_wrap("Ag".to_string(), head(size * k), lc).size().y;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h + 4.0), Sense::hover());
    ui.painter().text(rect.left_top(), egui::Align2::LEFT_TOP, left, head(size * k), lc);
    ui.painter().text(rect.right_top(), egui::Align2::RIGHT_TOP, right, head(size * k), rc);
}

/// A player in Compare: from a built match, or a season line of the scouting index.
#[derive(Clone)]
enum CmpSrc {
    Match(usize, library::PlayerMeta),
    Scout(ScoutPlayerRow),
}

fn role_text(r: Option<ScoutRole>) -> &'static str {
    r.map(|r| r.label()).unwrap_or("–")
}

/// Radar axes: (name on the radar, metric in the scouting index).
const RADAR: [(&str, &str); 8] = [("Goals", "Goals"), ("xG", "xG"), ("xA", "Inferred xA"), ("Shots", "Shots"), ("Pass %", "Completion"), ("Prog. actions", "Progressive actions"), ("Def. actions", "Defensive actions"), ("VAEP", "VAEP")];

/// Match totals of a match player, aligned with `METRICS` (what the match summary carries; the rest is 0).
fn match_totals(p: &library::PlayerMeta) -> Vec<f64> {
    let e = p.extra.clone().unwrap_or_default();
    METRICS
        .iter()
        .map(|(l, _)| match *l {
            "Goals" => p.goals as f64,
            "Shots" => p.shots as f64,
            "xG" => p.xg,
            "Inferred xA" => p.xa,
            "xGChain" => e.xgchain,
            "Box entries" => e.box_entries as f64,
            "Final-third receptions" => e.ft_receptions as f64,
            "Touches" => p.touches as f64,
            "Passes" => e.passes as f64,
            "Completion" => e.pass_pct,
            "Progressive actions" => e.prog_passes as f64,
            "Line-breaking passes" => e.line_breaks as f64,
            "Positive xT" => e.positive_xt,
            "Take-ons won" => e.take_ons_won as f64,
            "Recoveries" => e.recoveries as f64,
            "Aerials won" => e.aerials_won as f64,
            "Defensive actions" => e.def_actions as f64,
            "VAEP" => p.vaep,
            _ => 0.0,
        })
        .collect()
}

/// One side of a comparison in season units (per 90 for counts), whatever its source.
struct CmpP {
    short: String,
    role_text: String,
    role: ScoutRole,
    vals: Vec<f64>,
}

impl CmpP {
    fn new(s: &CmpSrc) -> CmpP {
        match s {
            CmpSrc::Scout(r) => CmpP { short: r.short.clone(), role_text: role_text(r.role).to_string(), role: r.role.unwrap_or(ScoutRole::CM), vals: r.m.iter().map(|v| *v as f64).collect() },
            CmpSrc::Match(_, p) => {
                let role = match p.role.as_str() {
                    "Goalkeeper" => ScoutRole::GK,
                    "Defender" => ScoutRole::CB,
                    "Forward" => ScoutRole::FW,
                    _ => ScoutRole::CM,
                };
                let totals = match_totals(p);
                let vals = totals.iter().enumerate().map(|(i, v)| season_unit(i, *v, p.minutes)).collect();
                CmpP { short: p.short.clone(), role_text: p.role.clone(), role, vals }
            }
        }
    }
    fn axis_values(&self) -> [f64; 8] {
        std::array::from_fn(|i| metric_index(RADAR[i].1).map(|k| self.vals[k]).unwrap_or(0.0))
    }
    /// Role percentile of every radar axis against the season pool, as 0..1.
    fn fractions(&self, ix: &ScoutIndex) -> [f64; 8] {
        let v = self.axis_values();
        std::array::from_fn(|i| percentile(v[i], &ix.role_values(self.role, RADAR[i].1, ix.pool_minutes, 20)) / 100.0)
    }
}

fn dual_radar(ui: &mut Ui, names: [&str; 2], fa: [f64; 8], fb: [f64; 8], ca: Color32, cb: Color32) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let w = ui.available_width();
    let r = ((w * 0.5) - 84.0).clamp(60.0, 110.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 2.0 * (r + 44.0)), Sense::hover());
    let c = rect.center();
    let point = |rad: f32, i: usize| {
        let ang = -FRAC_PI_2 + i as f32 * TAU / 8.0;
        egui::pos2(c.x + rad * ang.cos(), c.y + rad * ang.sin())
    };
    let p = ui.painter();
    for k in 1..=4 {
        let rr = r * k as f32 / 4.0;
        p.add(egui::Shape::closed_line((0..8).map(|i| point(rr, i)).collect(), Stroke::new(1.0, DARK_LINE)));
    }
    for i in 0..8 {
        p.line_segment([c, point(r, i)], Stroke::new(1.0, DARK_LINE));
    }
    for (prof, col) in [(fa, ca), (fb, cb)] {
        let poly: Vec<egui::Pos2> = (0..8).map(|i| point(r * prof[i].clamp(0.0, 1.0) as f32, i)).collect();
        p.add(egui::Shape::convex_polygon(poly.clone(), Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 60), Stroke::NONE));
        p.add(egui::Shape::closed_line(poly, Stroke::new(2.4, col)));
    }
    // axis labels last, on a backing, so no spoke or polygon edge runs through them
    for i in 0..8 {
        let g = p.layout_no_wrap(RADAR[i].0.to_string(), semi(11.0), DARK_TEXT2);
        let lr = egui::Align2::CENTER_CENTER.anchor_size(point(r + 26.0, i), g.size());
        p.rect_filled(lr.expand(2.0), 4.0, INK);
        p.galley(lr.min, g, DARK_TEXT2);
    }
    ui.horizontal_wrapped(|ui| {
        for (name, col) in [(names[0], ca), (names[1], cb)] {
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 6.0, col);
            ui.label(RichText::new(name).font(semi(12.0)).color(WHITE));
            ui.add_space(12.0);
        }
    });
}

/// Rows of one comparison group from season figures: (label, left text, right text, left value, right value).
fn group_rows_per90(a: &[f64], b: &[f64], g: usize) -> Vec<(&'static str, String, String, f64, f64)> {
    let labels: &[&'static str] = match g {
        0 => &["Goals", "xG", "Shots", "Box entries", "Final-third receptions"],
        1 => &["Passes", "Completion", "Progressive actions", "Line-breaking passes", "Inferred xA", "xGChain"],
        _ => &["Touches", "Take-ons won", "Positive xT", "Recoveries", "Aerials won", "VAEP"],
    };
    labels
        .iter()
        .map(|l| {
            let k = metric_index(l).unwrap_or(0);
            (*l, format_metric(l, a[k]), format_metric(l, b[k]), a[k], b[k])
        })
        .collect()
}

/// Rows of one comparison group: (label, left text, right text, left value, right value).
fn group_rows(a: &library::PlayerMeta, b: &library::PlayerMeta, g: usize) -> Vec<(&'static str, String, String, f64, f64)> {
    let ea = a.extra.clone().unwrap_or_default();
    let eb = b.extra.clone().unwrap_or_default();
    let d2 = |v: f64| format!("{v:.2}");
    let pct = |v: f64| format!("{:.0}%", v * 100.0);
    let ratio = |x: usize, y: usize| format!("{x}/{y}");
    let row = |label: &'static str, sa: String, sb: String, fa: f64, fb: f64| (label, sa, sb, fa, fb);
    match g {
        0 => vec![
            row("Goals", a.goals.to_string(), b.goals.to_string(), a.goals as f64, b.goals as f64),
            row("xG", d2(a.xg), d2(b.xg), a.xg, b.xg),
            row("Shots", a.shots.to_string(), b.shots.to_string(), a.shots as f64, b.shots as f64),
            row("Box entries", ea.box_entries.to_string(), eb.box_entries.to_string(), ea.box_entries as f64, eb.box_entries as f64),
            row("Final-third receptions", ea.ft_receptions.to_string(), eb.ft_receptions.to_string(), ea.ft_receptions as f64, eb.ft_receptions as f64),
        ],
        1 => vec![
            row("Passes", ea.passes.to_string(), eb.passes.to_string(), ea.passes as f64, eb.passes as f64),
            row("Pass %", pct(ea.pass_pct), pct(eb.pass_pct), ea.pass_pct, eb.pass_pct),
            row("Progressive passes", ea.prog_passes.to_string(), eb.prog_passes.to_string(), ea.prog_passes as f64, eb.prog_passes as f64),
            row("Line-breaking passes", ea.line_breaks.to_string(), eb.line_breaks.to_string(), ea.line_breaks as f64, eb.line_breaks as f64),
            row("Inferred xA", d2(a.xa), d2(b.xa), a.xa, b.xa),
            row("xGChain", d2(ea.xgchain), d2(eb.xgchain), ea.xgchain, eb.xgchain),
        ],
        _ => vec![
            row("Touches", a.touches.to_string(), b.touches.to_string(), a.touches as f64, b.touches as f64),
            row("Take-ons won", ea.take_ons_won.to_string(), eb.take_ons_won.to_string(), ea.take_ons_won as f64, eb.take_ons_won as f64),
            row("Positive xT", d2(ea.positive_xt), d2(eb.positive_xt), ea.positive_xt, eb.positive_xt),
            row("Recoveries", ea.recoveries.to_string(), eb.recoveries.to_string(), ea.recoveries as f64, eb.recoveries as f64),
            row("Aerials won", ratio(ea.aerials_won, ea.aerials), ratio(eb.aerials_won, eb.aerials), ea.aerials_won as f64, eb.aerials_won as f64),
            row("VAEP", d2(a.vaep), d2(b.vaep), a.vaep, b.vaep),
        ],
    }
}

fn board_badge(e: &library::Entry, id: &str) -> String {
    let s = &e.s;
    let f = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or("–".into());
    match id {
        "01_xg_flow" => format!("npxG {:.2}–{:.2}", s.npxg[0], s.npxg[1]),
        "03_win_odds" => format!("xPts {:.1}–{:.1}", s.xpts[0], s.xpts[1]),
        "14_ppda_timeline" => format!("PPDA {}–{}", f(s.ppda[0]), f(s.ppda[1])),
        _ => {
            let team = if id.contains("_home") { Some(&s.home_short) } else if id.contains("_away") { Some(&s.away_short) } else { None };
            let half = if id.ends_with("_h1") { " · 1st half" } else if id.ends_with("_h2") { " · 2nd half" } else { "" };
            match team {
                Some(t) => format!("{t}{half}"),
                None => "Full match · both teams".to_string(),
            }
        }
    }
}
