//! League browser (catalog matches) and the cross-league player scout.

use super::one_line;
use crate::Screen;
use crate::data::{COLUMNS, MINUTES};
use crate::library::{self, PacketJob, Source};
use crate::style::*;
use crate::App;
use egui::{Color32, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};
use matchhub::catalog::*;
use std::sync::Arc;

/// Leaderboard geometry (points): fixed so the sticky name column and the scrolling figures line up.
const ROW_H: f32 = 56.0;
const NAME_W: f32 = 150.0;
const CELL_W: f32 = 76.0;

/// Colour of the season wedges and the average-position disc.
fn accent_hex() -> String {
    format!("#{:02X}{:02X}{:02X}", FIXER.r(), FIXER.g(), FIXER.b())
}

/// "FW" badge colours stay in the neutral ramp: the role is a label, not a team.
fn role_label(r: Option<ScoutRole>) -> &'static str {
    r.map(|r| r.label()).unwrap_or("–")
}

impl App {
    // ------------------------------------------------------------------ league browser

    /// Pill strip: Saved, All 7 and the seven leagues. One row that scrolls sideways.
    pub fn league_strip(&mut self, ui: &mut Ui, id: &str) {
        let mut labels: Vec<&str> = vec!["Saved", "All 7"];
        labels.extend(League::ALL.iter().map(|l| l.pill()));
        ScrollArea::horizontal().id_salt(("leagues", id)).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, l) in labels.iter().enumerate() {
                    if chip(ui, l, self.data.league == i, lt()) {
                        self.data.league = i;
                    }
                }
            });
        });
    }

    /// `dark`: the screen is always Darkroom (Players, Scout); the Matches screen follows the theme.
    fn search_box(ui: &mut Ui, text: &mut String, hint: &str, dark: bool) {
        let w = ui.available_width();
        let (fill, ink, muted) = if dark { (ROOM_CARD_HI, PRINT, ROOM_MUTED) } else { (PRINT_CARD_HI, INK, PRINT_MUTED) };
        egui::Frame::new().fill(fill).corner_radius(24).inner_margin(egui::Margin::symmetric(16, 0)).show(ui, |ui| {
            ui.set_width(w - 32.0);
            ui.add_sized([w - 32.0, 48.0], egui::TextEdit::singleline(text).hint_text(RichText::new(hint).color(muted)).frame(egui::Frame::NONE).text_color(ink).font(body(15.0)).align(egui::Align2::LEFT_CENTER));
        });
    }

    /// Search box plus the pill strip (shared by Matches and Home).
    pub fn league_controls(&mut self, ui: &mut Ui, id: &str, with_search: bool) {
        self.league_strip(ui, id);
        if with_search && self.data.league != 0 {
            ui.add_space(8.0);
            Self::search_box(ui, &mut self.data.query, "Team", lt());
        }
    }

    /// Opens a catalog match: the saved copy when there is one, otherwise downloads its packet and builds it.
    pub fn open_remote(&mut self, ctx: &egui::Context, m: &CatalogMatch) {
        if let Some(i) = self.lib.entries.iter().position(|e| e.s.id == m.id) {
            self.push(Screen::Hub(i));
            return;
        }
        if self.build.lock().unwrap().running {
            self.toast(ctx, "Another match is still building");
            return;
        }
        let Some(remote) = self.data_remote() else {
            self.toast(ctx, "Set the repository under Settings to download matches");
            return;
        };
        self.open_on_done = true;
        library::spawn_build(self.root.clone(), Source::Packet(Box::new(PacketJob { remote, entry: m.clone() })), false, None, None, self.build.clone(), ctx.clone());
    }

    /// One catalog match as a card. Returns true when tapped.
    fn remote_card(&self, ui: &mut Ui, l: &LeagueCatalog, m: &CatalogMatch, width: f32) -> bool {
        let saved = self.lib.entries.iter().any(|e| e.s.id == m.id);
        let r = card(lt()).show(ui, |ui| {
            ui.set_width(width);
            let title = format!("{}  {}–{}  {}", if m.home_short.is_empty() { &m.home } else { &m.home_short }, m.score[0], m.score[1], if m.away_short.is_empty() { &m.away } else { &m.away_short });
            ui.add(egui::Label::new(RichText::new(title).font(head(19.0)).color(fg())).truncate());
            ui.add(egui::Label::new(RichText::new(format!("{} v {}", m.home, m.away)).font(body(12.0)).color(sub())).truncate());
            ui.add(egui::Label::new(RichText::new(format!("{} · {}", League::from_slug(&l.slug).map(|x| x.pill()).unwrap_or(&l.name), m.date)).font(body(12.0)).color(sub())).truncate());
            let (txt, col) = if saved { ("On device".to_string(), if lt() { SAGE } else { FOREST }) } else { (format!("Download {} KB", (m.bytes / 1024).max(1)), sub()) };
            ui.label(RichText::new(txt).font(semi(12.0)).color(col));
        });
        let resp = r.response.interact(Sense::click());
        press_overlay(ui, &resp, 18.0, lt());
        resp.clicked()
    }

    /// The catalog matches of the current league filter as cards (at most `limit`).
    pub fn remote_list(&mut self, ui: &mut Ui, limit: Option<usize>) {
        let Some(f) = self.data.league_filter() else { return };
        let ctx = ui.ctx().clone();
        let (running, err) = {
            let j = self.data.job.lock().unwrap();
            (j.running, j.error.clone())
        };
        let Some(cat) = self.data.catalog.clone() else {
            if running {
                ui.label(RichText::new("Loading league data…").font(body(13.0)).color(sub()));
            } else {
                ui.label(RichText::new(err.unwrap_or_else(|| "League data has not been published yet".into())).font(body(13.0)).color(sub()));
                if ghost(ui, "Try again", lt()) {
                    self.refresh_data(&ctx);
                }
            }
            return;
        };
        let rows = cat.matches(f, &self.data.query);
        let shown = limit.map(|n| n.min(rows.len())).unwrap_or(rows.len().min(60));
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{} matches · season {}", rows.len(), cat.season)).font(body(12.0)).color(sub()));
            if running {
                ui.label(RichText::new("· updating").font(body(12.0)).color(sub()));
            }
        });
        if let Some(e) = &err {
            ui.label(RichText::new(e.chars().take(160).collect::<String>()).font(body(12.0)).color(sub()));
        }
        let mut open: Option<CatalogMatch> = None;
        for (l, m) in rows.iter().take(shown) {
            let w = ui.available_width() - 34.0;
            if self.remote_card(ui, l, m, w) {
                open = Some((*m).clone());
            }
            ui.add_space(8.0);
        }
        if rows.is_empty() {
            ui.label(RichText::new("Nothing matches this filter").font(body(13.0)).color(sub()));
        }
        if let Some(m) = open {
            self.open_remote(&ctx, &m);
        }
    }

    // ------------------------------------------------------------------ league scout

    pub fn scout_screen(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            if top_bar(ui, "League scout", true) {
                self.back();
            }
            ScrollArea::vertical().id_salt("scout").show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                self.scout_view(ui);
                ui.add_space(24.0);
            });
        });
    }

    fn scout_chips(ui: &mut Ui, id: &str, labels: &[&str], cur: &mut usize) {
        ScrollArea::horizontal().id_salt(("scoutchips", id)).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, l) in labels.iter().enumerate() {
                    if chip(ui, l, *cur == i, true) {
                        *cur = i;
                    }
                }
            });
        });
    }

    /// Filters and the sortable leaderboard.
    pub fn scout_view(&mut self, ui: &mut Ui) {
        let Some(ix) = self.data.index.clone() else {
            ui.add_space(24.0);
            ui.label(RichText::new("No scouting data yet").font(head(24.0)).color(WHITE));
            let running = self.data.job.lock().unwrap().running;
            let err = self.data.job.lock().unwrap().error.clone();
            ui.label(RichText::new(if running { "Loading…".to_string() } else { err.unwrap_or_else(|| "The nightly job has not published a scouting index yet".into()) }).font(body(13.0)).color(DARK_TEXT2));
            if !running && ghost(ui, "Try again", true) {
                let ctx = ui.ctx().clone();
                self.refresh_data(&ctx);
            }
            return;
        };
        let mut league_labels: Vec<&str> = vec!["All 7", "Top 5"];
        league_labels.extend(League::ALL.iter().map(|l| l.pill()));
        let mut role_labels: Vec<&str> = vec!["All"];
        role_labels.extend(ScoutRole::ALL.iter().map(|r| r.label()));
        let min_labels: Vec<String> = MINUTES.iter().map(|m| format!("{m:.0}'")).collect();
        let min_refs: Vec<&str> = min_labels.iter().map(|s| s.as_str()).collect();

        ui.add_space(4.0);
        Self::search_box(ui, &mut self.data.s_query, "Player or club", true);
        ui.add_space(8.0);
        ui.label(RichText::new("League").font(bold(12.0)).color(DARK_TEXT2));
        Self::scout_chips(ui, "league", &league_labels, &mut self.data.s_league);
        ui.label(RichText::new("Role").font(bold(12.0)).color(DARK_TEXT2));
        Self::scout_chips(ui, "role", &role_labels, &mut self.data.s_role);
        ui.label(RichText::new("Minimum minutes").font(bold(12.0)).color(DARK_TEXT2));
        Self::scout_chips(ui, "min", &min_refs, &mut self.data.s_min);

        let rows = self.data.leaderboard();
        let total = rows.len();
        ui.add_space(8.0);
        ui.label(RichText::new(format!("{total} players · season {} · all figures per 90 · badges are role percentiles", ix.season)).font(body(12.0)).color(DARK_TEXT2));
        ui.add_space(8.0);
        if rows.is_empty() {
            ui.label(RichText::new("No players match these filters").font(body(14.0)).color(WHITE));
            return;
        }
        let shown = self.data.s_rows.min(total);
        // table geometry grows with the text size so names, clubs and badges never collide
        let k = (text_scale_pct() as f32 / 100.0).max(1.0);
        let (row_h, name_w, cell_w) = (ROW_H * k, NAME_W * k.sqrt(), CELL_W * k.sqrt());
        let (bw, bh) = (30.0 * k, 18.0 * k);
        let mut open: Option<u64> = None;
        let mut sort_to: Option<usize> = None;
        // header: the sticky name cell, then the sortable figure columns
        let right_w = cell_w * (3.0 + COLUMNS.len() as f32);
        // two parts placed by hand at the same top edge, so the sticky name column and the scrolling
        // figures stay on the same row lines whatever the scroll area adds around itself
        let top_left = ui.cursor().min;
        let avail_w = ui.available_width();
        let total_h = 40.0 + shown as f32 * row_h;
        let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(top_left, Vec2::new(name_w, total_h))));
        let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(egui::pos2(top_left.x + name_w, top_left.y), Vec2::new((avail_w - name_w).max(60.0), total_h + 24.0))));
        {
            let ui = &mut lui;
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.vertical(|ui| {
                ui.set_width(name_w);
                let (r, _) = ui.allocate_exact_size(Vec2::new(name_w, 40.0), Sense::hover());
                ui.painter().text(egui::pos2(r.left() + 4.0, r.center().y), egui::Align2::LEFT_CENTER, "Player", semi(12.0), DARK_TEXT2);
                for p in rows.iter().take(shown) {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(name_w, row_h), Sense::click());
                    let pt = ui.painter();
                    pt.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, DARK_LINE));
                    let g = one_line(ui, &p.short, semi(14.0), WHITE, name_w - bw - 10.0);
                    pt.galley(egui::pos2(r.left() + 4.0, r.top() + row_h * 0.12), g, WHITE);
                    let g2 = one_line(ui, &p.team, body(11.0), DARK_TEXT2, name_w - 8.0);
                    pt.galley(egui::pos2(r.left() + 4.0, r.top() + row_h * 0.56), g2, DARK_TEXT2);
                    // role badge
                    let br = egui::Rect::from_min_size(egui::pos2(r.right() - bw - 4.0, r.top() + row_h * 0.14), Vec2::new(bw, bh));
                    pt.rect_filled(br, bh / 2.0, ROOM_CARD_HI);
                    pt.text(br.center(), egui::Align2::CENTER_CENTER, role_label(p.role), semi(10.5), SAGE);
                    if resp.clicked() {
                        open = Some(p.id);
                    }
                }
            });
        }
        {
            let ui = &mut rui;
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ScrollArea::horizontal().id_salt("scout_table").content_margin(egui::Margin::ZERO).auto_shrink([false, true]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_width(right_w);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        for (label, w) in [("League", cell_w + 20.0), ("Min", cell_w - 20.0)] {
                            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 40.0), Sense::hover());
                            ui.painter().text(egui::pos2(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER, label, semi(12.0), DARK_TEXT2);
                        }
                        for (k, (head_label, _)) in COLUMNS.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(cell_w, 40.0), Sense::click());
                            let on = self.data.s_sort == k;
                            let arrow = if on { if self.data.s_desc { " ▼" } else { " ▲" } } else { "" };
                            ui.painter().text(egui::pos2(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER, format!("{head_label}{arrow}"), semi(12.0), if on { WHITE } else { DARK_TEXT2 });
                            if resp.clicked() {
                                sort_to = Some(k);
                            }
                        }
                    });
                    for p in rows.iter().take(shown) {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(cell_w + 20.0, row_h), Sense::click());
                            let pt = ui.painter();
                            pt.line_segment([r.left_bottom(), egui::pos2(r.right() + cell_w * 5.0, r.bottom())], Stroke::new(1.0, DARK_LINE));
                            let lg = League::from_slug(&p.league).map(|l| l.pill()).unwrap_or("");
                            let g = one_line(ui, lg, body(12.0), DARK_TEXT2, r.width() - 12.0);
                            let gp = egui::pos2(r.right() - 8.0 - g.size().x, r.center().y - g.size().y / 2.0);
                            pt.galley(gp, g, DARK_TEXT2);
                            if resp.clicked() {
                                open = Some(p.id);
                            }
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(cell_w - 20.0, row_h), Sense::click());
                            ui.painter().text(egui::pos2(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER, format!("{:.0}", p.minutes), head(15.0), WHITE);
                            if resp.clicked() {
                                open = Some(p.id);
                            }
                            for (head_label, metric) in COLUMNS.iter() {
                                let _ = head_label;
                                let (r, resp) = ui.allocate_exact_size(Vec2::new(cell_w, row_h), Sense::click());
                                let pt = ui.painter();
                                pt.text(egui::pos2(r.right() - 8.0, r.top() + row_h * 0.36), egui::Align2::RIGHT_CENTER, format_metric(metric, p.value(metric)), head(16.0), WHITE);
                                // role percentile badge: strong ones in the accent, the rest muted
                                let pc = p.percentile_of(metric);
                                let br = egui::Rect::from_min_size(egui::pos2(r.right() - 8.0 - 36.0 * k, r.top() + row_h * 0.62), Vec2::new(36.0 * k, 16.0 * k));
                                pt.rect_filled(br, 8.0 * k, if pc >= 75.0 { FIXER } else { ROOM_CARD_HI });
                                pt.text(br.center(), egui::Align2::CENTER_CENTER, format!("{pc:.0}"), semi(10.5), if pc >= 75.0 { PRINT } else { DARK_TEXT2 });
                                if resp.clicked() {
                                    open = Some(p.id);
                                }
                            }
                        });
                    }
                });
            });
        }
        ui.allocate_rect(egui::Rect::from_min_size(top_left, Vec2::new(avail_w, total_h)), Sense::hover());
        if shown < total {
            ui.add_space(8.0);
            if ghost(ui, &format!("Show more ({} left)", total - shown), true) {
                self.data.s_rows += 40;
            }
        }
        if let Some(k) = sort_to {
            if self.data.s_sort == k {
                self.data.s_desc = !self.data.s_desc;
            } else {
                self.data.s_sort = k;
                self.data.s_desc = true;
            }
        }
        if let Some(id) = open {
            self.push(Screen::ScoutPlayer(id));
        }
    }

    // ------------------------------------------------------------------ one player's season

    pub fn scout_player(&mut self, ui: &mut Ui, id: u64) {
        let Some(ix) = self.data.index.clone() else {
            self.back();
            return;
        };
        let Some(row) = ix.find(id).cloned() else {
            self.back();
            return;
        };
        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| {
            if top_bar(ui, &row.short, true) {
                self.back();
            }
            ScrollArea::vertical().id_salt("scout_player").show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                if ghost(ui, "Compare player", true) {
                    self.compare_scout_player(&ix, &row);
                }
                ui.add_space(8.0);
                let sc: Arc<matchhub::render::svg::Scene> = self.data.scout_scene(&ix, &row, &accent_hex());
                let w = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, w * sc.h / sc.w), Sense::hover());
                let tile = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                crate::scene::paint(&tile, rect, &sc, &crate::scene::PaintOpts { anim: None, hidden: None, selected: None, thumb: false, crests: [None, None] });
                ui.painter().rect_stroke(rect, 10.0, Stroke::new(1.0, ROOM_LINE), egui::StrokeKind::Inside);
                ui.add_space(24.0);
            });
        });
        let _ = Color32::WHITE;
    }

    /// Opens Compare in Players mode with this scout player on the left and the strongest same-role
    /// player of another league on the right.
    pub fn compare_scout_player(&mut self, ix: &ScoutIndex, row: &ScoutPlayerRow) {
        let rival = ix.players.iter().filter(|p| p.id != row.id && p.role == row.role && p.league != row.league && p.minutes >= ix.pool_minutes).max_by(|a, b| a.minutes.partial_cmp(&b.minutes).unwrap()).map(|p| p.id);
        self.compare.players_mode = true;
        self.compare.pl_left = Some((SCOUT_KEY, row.id));
        self.compare.pl_right = rival.map(|id| (SCOUT_KEY, id));
        self.push(Screen::Compare);
    }
}

/// Match index used in the Compare selection for players that come from the scouting index.
pub const SCOUT_KEY: usize = usize::MAX;
