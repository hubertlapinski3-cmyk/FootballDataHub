//! Onboarding, home, match list, match hub, report, players, compare, settings.

use super::{boards_by_section, STEPS};
use crate::library::{self, Source};
use crate::style::*;
use crate::{App, Screen};
use egui::{Align, Color32, Layout, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};

impl App {
    // ------------------------------------------------------------------ 1 onboarding
    pub fn onboarding(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        gradient(ui, rect, PRIMARY_VARIANT, Color32::from_rgb(0x07, 0x06, 0x0c));
        // build the bundled sample in the background so the preview shows a real board
        if self.lib.entries.is_empty() && !self.build.lock().unwrap().running && self.build.lock().unwrap().step.is_empty() {
            self.open_on_done = false;
            library::spawn_build(self.root.clone(), Source::Sample, false, self.build.clone(), ui.ctx().clone());
        }
        let preview = self.lib.entries.iter().position(|e| e.s.source == "statsbomb").or(if self.lib.entries.is_empty() { None } else { Some(0) });
        ScrollArea::vertical().id_salt("onb").show(ui, |ui| egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 0)).show(ui, |ui| {
            ui.add_space(36.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("DATA HUB").font(bold(13.0)).color(SECONDARY));
                ui.label(RichText::new("Every match, read like a broadcast").font(head(40.0)).color(WHITE));
                ui.add_space(4.0);
                ui.label(RichText::new("31 analysis boards and a written report from one WhoScored match").font(body(15.0)).color(DARK_TEXT2));
            });
            ui.add_space(20.0);
            egui::Frame::new().fill(BOARD_BG).corner_radius(18).inner_margin(10).stroke(Stroke::new(1.0, Color32::from_rgb(0x4a, 0x2a, 0x9a))).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let w = ui.available_width();
                let h = w * 1130.0 / 1600.0;
                let tex = preview.and_then(|i| self.raster.get(self.board_path(i, "01_xg_flow"), 0.45));
                match tex {
                    Some(t) => {
                        ui.add(egui::Image::new(&t).fit_to_exact_size(Vec2::new(w, h)).corner_radius(10));
                    }
                    None => {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                        ui.painter().rect_filled(r, 10.0, Color32::from_rgb(0x15, 0x10, 0x1e));
                        let st = self.build.lock().unwrap().clone();
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("Preparing a sample board… {}", st.step), body(13.0), DARK_TEXT2);
                    }
                }
                let cap = preview.map(|i| { let s = &self.lib.entries[i].s; format!("Sample board: xG flow · {} {}–{} {} · {}", s.home, s.score[0], s.score[1], s.away, s.competition) }).unwrap_or_else(|| "Sample board: xG flow".into());
                ui.label(RichText::new(cap).font(body(11.5)).color(DARK_TEXT2));
            });
            ui.add_space(18.0);
            ui.label(RichText::new("How the win model works").font(head(22.0)).color(WHITE));
            ui.label(RichText::new("Every shot is replayed at its expected-goals value thousands of times to price the result, and the live win curve is a transparent heuristic built only from the score, the time left and each side's xG so far. It is not a betting model, and every board lists the models, sources and fallbacks behind it.").font(body(14.5)).color(DARK_TEXT2));
            ui.add_space(22.0);
            ui.vertical_centered_justified(|ui| {
                if cta(ui, "Get Started", true) {
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
            egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
                ui.label(RichText::new("Data Hub").font(head(34.0)).color(BLACK));
                ui.label(RichText::new("Paste a WhoScored match ID or URL").font(body(13.5)).color(TEXT2));
                let running = self.build.lock().unwrap().running;
                ui.add(egui::TextEdit::singleline(&mut self.input).hint_text("e.g. 1650630 or https://www.whoscored.com/Matches/…").desired_width(f32::INFINITY).min_size(Vec2::new(0.0, 46.0)).font(body(15.0)));
                ui.horizontal(|ui| {
                    let ok = !self.input.trim().is_empty() && !running;
                    if cta(ui, "Build Match", ok) {
                        let s = self.input.trim().to_string();
                        let is_file = std::path::Path::new(&s).exists();
                        if !is_file && !self.prefs.personal_fetch {
                            self.toast(ui.ctx(), "Online fetching is off. Enable the personal-use flag in Settings, or import a saved page.");
                        } else {
                            self.open_on_done = true;
                            library::spawn_build(self.root.clone(), Source::WhoScored(s), self.prefs.personal_fetch, self.build.clone(), ui.ctx().clone());
                        }
                    }
                    if ghost(ui, "Sample match", false) && !running {
                        self.open_on_done = true;
                        library::spawn_build(self.root.clone(), Source::Sample, false, self.build.clone(), ui.ctx().clone());
                    }
                });
                // saved pages dropped into the import folder
                let imports = self.import_files();
                if !imports.is_empty() {
                    section_title(ui, "Saved pages to import", false);
                    for f in imports.iter().take(6) {
                        let name = f.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                        if ui.add(egui::Button::new(RichText::new(format!("⤓  {name}")).font(body(14.0))).min_size(Vec2::new(ui.available_width(), 44.0))).clicked() && !running {
                            self.open_on_done = true;
                            library::spawn_build(self.root.clone(), Source::WhoScored(f.display().to_string()), self.prefs.personal_fetch, self.build.clone(), ui.ctx().clone());
                        }
                    }
                } else {
                    ui.label(RichText::new(format!("Saved WhoScored pages (.html / .json / .gz) placed in {} appear here.", self.import_dir().display())).font(body(11.5)).color(TEXT2));
                }
                ui.add_space(6.0);
                self.build_status(ui, false);
                section_title(ui, "Latest built matches", false);
                if self.lib.entries.is_empty() {
                    ui.label(RichText::new("No matches yet. Build one above or try the sample.").color(TEXT2));
                }
                let mut open = None;
                ScrollArea::horizontal().id_salt("latest").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for i in 0..self.lib.entries.len().min(8) {
                            if self.match_card(ui, i, 280.0) {
                                open = Some(i);
                            }
                        }
                    });
                });
                if let Some(i) = open {
                    self.push(Screen::Hub(i));
                }
                section_title(ui, "Quick actions", false);
                ui.columns(4, |cols| {
                    let items = [("list", "Matches"), ("grid", "Boards"), ("compare", "Compare"), ("gear", "Settings")];
                    for (k, (icon, label)) in items.iter().enumerate() {
                        let r = card(false).show(&mut cols[k], |ui| {
                            ui.vertical_centered(|ui| {
                                let (ir, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                                draw_icon(ui, ir, icon);
                                ui.label(RichText::new(*label).font(semi(12.0)).color(BLACK));
                            });
                        });
                        if r.response.interact(Sense::click()).clicked() {
                            match k {
                                0 => self.push(Screen::Matches),
                                1 => {
                                    if self.lib.entries.is_empty() {
                                        self.toast(cols[k].ctx(), "Build a match first");
                                    } else {
                                        self.push(Screen::Hub(0))
                                    }
                                }
                                2 => self.push(Screen::Compare),
                                _ => self.push(Screen::Settings),
                            }
                        }
                    }
                });
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
            if top_bar(ui, "Matches", false) {
                self.back();
            }
            let mut open = None;
            let mut del = None;
            ScrollArea::vertical().id_salt("matches").show(ui, |ui| {
                for i in 0..self.lib.entries.len() {
                    let w = ui.available_width() - 34.0;
                    if self.match_card(ui, i, w) {
                        open = Some(i);
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} boards · {}", self.lib.entries[i].s.boards.len(), self.lib.entries[i].s.source)).font(body(11.0)).color(TEXT2));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("Delete").color(ERROR)).fill(Color32::TRANSPARENT)).clicked() {
                                del = Some(i);
                            }
                        });
                    });
                    ui.add_space(6.0);
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
                self.match_header(ui, m, "FULL MATCH");
                ui.add_space(8.0);
                ui.label(RichText::new(&e.s.headline).font(head(24.0)).color(WHITE));
                super::super::style::definition_chip(ui, "box entry followed by a shot = shot later in the same possession", true);
                ui.add_space(4.0);
                self.momentum_strip(ui, m, 70.0);
                ui.horizontal_wrapped(|ui| {
                    metric_chip(ui, "npxG", &format!("{:.2}–{:.2}", e.s.npxg[0], e.s.npxg[1]), true);
                    metric_chip(ui, "xPts", &format!("{:.1}–{:.1}", e.s.xpts[0], e.s.xpts[1]), true);
                    let f = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or("–".into());
                    metric_chip(ui, "PPDA", &format!("{}–{}", f(e.s.ppda[0]), f(e.s.ppda[1])), true);
                });
                ui.horizontal(|ui| {
                    if chip(ui, "Read the report", false, true) {
                        self.push(Screen::Report(m));
                    }
                    if chip(ui, "Players", false, true) {
                        self.push(Screen::Players(m));
                    }
                    if chip(ui, "Compare", false, true) {
                        self.compare.left = Some((m, 0));
                        self.compare.right = Some((m, 1));
                        self.push(Screen::Compare);
                    }
                });
                if !e.s.fallbacks.is_empty() {
                    ui.label(RichText::new(format!("Note: {} source fallback(s): {}", e.s.fallbacks.len(), e.s.fallbacks[0])).font(body(11.0)).color(DARK_TEXT2));
                }
                let w = ui.available_width();
                let tw = (w - 12.0) / 2.0;
                for (sec, boards) in boards_by_section(&e) {
                    section_title(ui, &sec, true);
                    let label = |b: &library::BoardMeta| {
                        let team = if b.id.contains("_home") { Some(&e.s.home_short) } else if b.id.contains("_away") { Some(&e.s.away_short) } else { None };
                        let half = if b.id.ends_with("_h1") { " · 1st half" } else if b.id.ends_with("_h2") { " · 2nd half" } else { "" };
                        match team { Some(t) => format!("{} · {}{}", b.title, t, half), None => b.title.clone() }
                    };
                    let mut items: Vec<(String, String)> = boards.iter().map(|b| (b.id.clone(), label(b))).collect();
                    if sec == "Players" {
                        items.push(("__players".into(), "Player cards".into()));
                    }
                    for row in items.chunks(2) {
                        ui.horizontal(|ui| {
                            for (id, title) in row {
                                let r = ui.allocate_ui(Vec2::new(tw, tw * 1130.0 / 1600.0 + 40.0), |ui| {
                                    ui.vertical(|ui| {
                                        let th = tw * 1130.0 / 1600.0;
                                        if id == "__players" {
                                            let (r, _) = ui.allocate_exact_size(Vec2::new(tw, th), Sense::hover());
                                            ui.painter().rect_filled(r, 10.0, DARK_CARD);
                                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("{} players →", e.s.players.len()), head(22.0), WHITE);
                                        } else {
                                            match self.raster.get(self.board_path(m, id), 0.22) {
                                                Some(t) => {
                                                    ui.add(egui::Image::new(&t).fit_to_exact_size(Vec2::new(tw, th)).corner_radius(10));
                                                }
                                                None => {
                                                    let (r, _) = ui.allocate_exact_size(Vec2::new(tw, th), Sense::hover());
                                                    ui.painter().rect_filled(r, 10.0, Color32::from_rgb(0x18, 0x13, 0x22));
                                                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "rendering…", body(11.0), DARK_TEXT2);
                                                }
                                            }
                                        }
                                        ui.label(RichText::new(title).font(semi(12.5)).color(WHITE));
                                    });
                                });
                                if r.response.interact(Sense::click()).clicked() {
                                    if id == "__players" {
                                        self.push(Screen::Players(m));
                                    } else {
                                        self.push(Screen::Viewer { m, board: id.clone() });
                                    }
                                }
                            }
                        });
                    }
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
                Some(r) => self.report_cache = Some((m, r)),
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
            if top_bar(ui, "Match report", false) {
                self.back();
            }
            ScrollArea::vertical().id_salt("report").show(ui, |ui| {
                ui.label(RichText::new(&rep.headline).font(head(34.0)).color(BLACK));
                rich_paragraph(ui, &rep.standfirst, &e.s.home, &e.s.away, hc, ac, 16.0);
                ui.add_space(6.0);
                for sec in &rep.sections {
                    ui.add_space(10.0);
                    ui.label(RichText::new(&sec.title).font(head(24.0)).color(BLACK));
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
                                if ghost(ui, &format!("Open board · {title}"), false) {
                                    self.push(Screen::Viewer { m, board: b.clone() });
                                }
                            }
                        });
                    }
                }
                ui.add_space(30.0);
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
            ScrollArea::vertical().id_salt("players").show(ui, |ui| {
                for side in 0..2 {
                    let col = self.team_col(m, side);
                    ui.add_space(6.0);
                    ui.label(RichText::new(if side == 0 { e.s.home.to_uppercase() } else { e.s.away.to_uppercase() }).font(head(22.0)).color(col));
                    let mut ps: Vec<&library::PlayerMeta> = e.s.players.iter().filter(|p| p.side == side).collect();
                    ps.sort_by(|a, b| b.minutes.partial_cmp(&a.minutes).unwrap());
                    for p in ps {
                        let r = egui::Frame::new().fill(DARK_CARD).corner_radius(12).inner_margin(10).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (rr, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                                ui.painter().circle_filled(rr.center(), 17.0, col);
                                ui.painter().text(rr.center(), egui::Align2::CENTER_CENTER, p.shirt.to_string(), bold(13.0), on(col));
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&p.name).font(semi(15.0)).color(WHITE));
                                    ui.label(RichText::new(format!("{} · {:.0} min · {} touches · {} shots", p.role, p.minutes, p.touches, p.shots)).font(body(11.5)).color(DARK_TEXT2));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let v = p.vaep;
                                    ui.label(RichText::new(format!("{}{:.2}", if v >= 0.0 { "▲ +" } else { "▼ " }, v)).font(head(18.0)).color(if v >= 0.0 { SECONDARY } else { Color32::from_rgb(0xff, 0x6b, 0x81) }));
                                });
                            });
                        });
                        if r.response.interact(Sense::click()).clicked() {
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
        let col = self.team_col(m, p.side);
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            if top_bar(ui, &p.short, true) {
                self.back();
            }
            ScrollArea::vertical().id_salt("player").show(ui, |ui| {
                ui.label(RichText::new(&p.name).font(head(32.0)).color(WHITE));
                ui.label(RichText::new(format!("{} · {} · Touches {} · Shots {}", if p.side == 0 { &e.s.home } else { &e.s.away }, p.role, p.touches, p.shots)).font(body(13.0)).color(DARK_TEXT2));
                ui.columns(3, |c| {
                    for (k, (v, l)) in [(format!("{:.2}", p.xg), "xG"), (format!("{:.2}", p.xa), "INFERRED xA"), (format!("{}{:.2}", if p.vaep >= 0.0 { "+" } else { "" }, p.vaep), "VAEP")].iter().enumerate() {
                        c[k].label(RichText::new(v).font(head(34.0)).color(col));
                        c[k].label(RichText::new(*l).font(bold(11.0)).color(WHITE));
                    }
                });
                if p.role == "Goalkeeper" {
                    if ghost(ui, "Open goalkeeper goal frames", true) {
                        self.push(Screen::Viewer { m, board: "22_goalkeeper_frames".into() });
                    }
                }
                let w = ui.available_width();
                let h = w * 1500.0 / 1600.0;
                match self.raster.get(self.board_path(m, &p.board), 0.7) {
                    Some(t) => {
                        let r = ui.add(egui::Image::new(&t).fit_to_exact_size(Vec2::new(w, h)).corner_radius(10).sense(Sense::click()));
                        if r.clicked() {
                            self.push(Screen::Viewer { m, board: p.board.clone() });
                        }
                    }
                    None => {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                        ui.painter().rect_filled(r, 10.0, DARK_CARD);
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "Rendering player card…", body(13.0), DARK_TEXT2);
                    }
                }
                ui.label(RichText::new("Tap the card to zoom. Percentiles compare players in the same role with 30+ minutes; below four peers the pool widens to every 30+ minute player and the caption says so.").font(body(11.5)).color(DARK_TEXT2));
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
                ui.label(RichText::new("Build at least one match to compare teams.").color(DARK_TEXT2));
                return;
            }
            if self.compare.left.is_none() {
                self.compare.left = Some((teams[0].0, teams[0].1));
                self.compare.right = Some((teams[1].0, teams[1].1));
            }
            ScrollArea::vertical().id_salt("compare_screen").show(ui, |ui| {
                for (k, label) in [(0, "Left team"), (1, "Right team")] {
                    let cur = if k == 0 { self.compare.left } else { self.compare.right };
                    let cur_label = teams.iter().find(|t| Some((t.0, t.1)) == cur).map(|t| t.2.clone()).unwrap_or_default();
                    let col = cur.map(|(m, s)| self.team_col(m, s)).unwrap_or(WHITE);
                    ui.label(RichText::new(label).font(bold(11.0)).color(DARK_TEXT2));
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
                    ui.add_space(6.0);
                    egui::Frame::new().fill(Color32::from_rgb(0x0d, 0x2b, 0x2a)).corner_radius(24).inner_margin(6).show(ui, |ui| {
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
                let (tl, tr) = (self.lib.entries[l.0].s.teams[l.1].clone(), self.lib.entries[r.0].s.teams[r.1].clone());
                let (cl, cr) = (self.team_col(l.0, l.1), self.team_col(r.0, r.1));
                ui.add_space(6.0);
                ui.set_max_width(ui.available_width() - 26.0);
                ui.columns(2, |c| {
                    c[0].label(RichText::new(tl.name.to_uppercase()).font(head(22.0)).color(cl));
                    c[1].with_layout(Layout::right_to_left(Align::Min), |ui| ui.label(RichText::new(tr.name.to_uppercase()).font(head(22.0)).color(cr)));
                });
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
                        p.text(egui::pos2(x, rect.top() + 26.0), egui::Align2::CENTER_CENTER, txt, body(9.0), SECONDARY);
                    }
                    p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, DARK_LINE));
                    if let Some(d) = def {
                        definition_chip(ui, d, true);
                    }
                }
                if self.compare.groups[2] {
                    let band = |v: Option<f64>| match v {
                        Some(v) if v < 8.0 => "ELITE PRESS (<8)",
                        Some(v) if v <= 12.0 => "MEDIUM BLOCK (8–12)",
                        Some(_) => "LOW BLOCK (>12)",
                        None => "NO DATA",
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
                        match self.raster.get(self.board_path(mi, &id), 0.22) {
                            Some(t) => {
                                if ui.add(egui::Image::new(&t).fit_to_exact_size(Vec2::new(w, w * 1130.0 / 1600.0)).corner_radius(8).sense(Sense::click())).clicked() {
                                    self.push(Screen::Viewer { m: mi, board: id });
                                }
                            }
                            None => {
                                ui.allocate_exact_size(Vec2::new(w, w * 0.7), Sense::hover());
                            }
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

    // ------------------------------------------------------------------ 8 settings
    pub fn settings(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 4)).show(ui, |ui| {
            if top_bar(ui, "Settings & sources", false) {
                self.back();
            }
            ScrollArea::vertical().id_salt("settings").show(ui, |ui| {
                section_title(ui, "Data sources", false);
                for (n, d) in [
                    ("Opta via WhoScored", "Primary event stream: every on-ball event with coordinates and qualifiers."),
                    ("FotMob · SofaScore", "Team colours, crests, per-shot xG and xGOT (best effort, recorded as fallbacks)."),
                    ("Transfermarkt", "Squad master data where available; ages come from WhoScored."),
                    ("StatsBomb Open Data", "Training data for the in-house xG, PSxG, xT and VAEP models. Attributed on those boards."),
                    ("ClubElo · football-data.co.uk · Understat", "Context and calibration sources supported by the pipeline."),
                ] {
                    ui.label(RichText::new(n).font(semi(14.0)).color(BLACK));
                    ui.label(RichText::new(d).font(body(12.5)).color(TEXT2));
                }
                section_title(ui, "Personal use", false);
                card(false).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut v = self.prefs.personal_fetch;
                    if ui.checkbox(&mut v, RichText::new("Allow fetching from WhoScored, FotMob and SofaScore on this device").font(body(14.0))).changed() {
                        self.prefs.personal_fetch = v;
                        self.save_prefs();
                    }
                    ui.label(RichText::new("These sites' terms generally prohibit automated scraping and redistribution. Keep this for personal use, one polite request per match, cached afterwards. If WhoScored shows a bot wall, save the page in a browser and import it instead.").font(body(11.5)).color(TEXT2));
                });
                section_title(ui, "Cache", false);
                let cache = self.root.join("cache");
                let mdir = self.root.join("matches");
                ui.label(format!("Source cache: {:.1} MB   ·   Built matches: {:.1} MB", library::dir_size(&cache) as f64 / 1e6, library::dir_size(&mdir) as f64 / 1e6));
                ui.horizontal(|ui| {
                    if ghost(ui, "Clear source cache", false) {
                        let _ = std::fs::remove_dir_all(&cache);
                        self.toast(ui.ctx(), "Source cache cleared");
                    }
                    if ui.add(egui::Button::new(RichText::new("Delete all matches").color(WHITE)).fill(ERROR).corner_radius(22).min_size(Vec2::new(0.0, 44.0))).clicked() {
                        let _ = std::fs::remove_dir_all(&mdir);
                        self.lib.reload();
                        self.raster.clear();
                        self.crest_tex.clear();
                        self.report_cache = None;
                        self.toast(ui.ctx(), "All built matches deleted");
                    }
                });
                section_title(ui, "Theme overrides (theme_overrides.toml)", false);
                ui.add(egui::TextEdit::multiline(&mut self.theme_text).font(egui::TextStyle::Monospace).desired_rows(10).desired_width(f32::INFINITY));
                if ghost(ui, "Save overrides", false) {
                    match toml_check(&self.theme_text) {
                        Ok(()) => {
                            let _ = std::fs::write(self.root.join("theme_overrides.toml"), &self.theme_text);
                            self.toast(ui.ctx(), "Saved. Applies to the next build.");
                        }
                        Err(e) => self.toast(ui.ctx(), &format!("Invalid TOML: {e}")),
                    }
                }
                section_title(ui, "Fallback notes (latest match)", false);
                if let Some(e) = self.lib.entries.first() {
                    for f in e.s.fallbacks.iter().chain(e.s.theme_decisions.iter()) {
                        ui.label(RichText::new(format!("• {f}")).font(body(12.0)).color(TEXT2));
                    }
                }
                section_title(ui, "Metric version log", false);
                ui.label(RichText::new(format!("App metric_version {}", matchhub::METRIC_VERSION)).font(body(12.5)));
                for e in &self.lib.entries {
                    ui.label(RichText::new(format!("{} v {} · metric {} · built {}", e.s.home, e.s.away, e.s.metric_version, &e.s.built_at[..16.min(e.s.built_at.len())])).font(body(11.5)).color(TEXT2));
                }
                ui.label(RichText::new(format!("Import folder: {}", self.import_dir().display())).font(body(11.5)).color(TEXT2));
                ui.add_space(30.0);
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
    let base = TextFormat { font_id: body(size), color: Color32::from_rgb(0x1a, 0x1a, 0x1a), line_height: Some(size * 1.5), ..Default::default() };
    let mut rest = text;
    while !rest.is_empty() {
        let hit = [(home, hc), (away, ac)].iter().filter(|(n, _)| !n.is_empty()).filter_map(|(n, c)| rest.find(*n).map(|i| (i, *n, *c))).min_by_key(|x| x.0);
        let (seg, team) = match hit {
            Some((i, n, c)) => (&rest[..i], Some((n, c))),
            None => (rest, None),
        };
        for tok in seg.split_inclusive(' ') {
            let numeric = tok.chars().any(|c| c.is_ascii_digit());
            let fmt = if numeric { TextFormat { font_id: bold(size), color: PRIMARY_VARIANT, ..base.clone() } } else { base.clone() };
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

/// Simple vector icons (no icon font needed on device).
fn draw_icon(ui: &Ui, r: egui::Rect, kind: &str) {
    let p = ui.painter();
    let st = Stroke::new(2.4, PRIMARY);
    let c = r.center();
    match kind {
        "list" => {
            for i in 0..3 {
                let y = r.top() + 7.0 + i as f32 * 7.0;
                p.circle_filled(egui::pos2(r.left() + 5.0, y), 2.2, PRIMARY);
                p.line_segment([egui::pos2(r.left() + 11.0, y), egui::pos2(r.right() - 2.0, y)], st);
            }
        }
        "grid" => {
            for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let a = egui::pos2(r.left() + 3.0 + dx * 12.0, r.top() + 3.0 + dy * 12.0);
                p.rect_filled(egui::Rect::from_min_size(a, Vec2::splat(10.0)), 2.0, PRIMARY);
            }
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
                p.line_segment([c + Vec2::angled(a) * 8.0, c + Vec2::angled(a) * 12.5], Stroke::new(3.5, PRIMARY));
            }
            p.circle_stroke(c, 7.5, st);
            p.circle_filled(c, 2.5, PRIMARY);
        }
    }
}
