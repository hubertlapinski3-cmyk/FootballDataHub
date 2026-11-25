//! Data Hub app: the viewing and browsing layer for the hub-core pipeline.
//! One egui codebase for Android (NativeActivity via android-activity) and desktop.

pub mod library;
pub mod raster;
pub mod scene;
pub mod screens;
pub mod style;

use library::{Library, Shared};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Onboarding,
    Home,
    Matches,
    Hub(usize),
    Viewer { m: usize, board: String },
    Report(usize),
    Players(usize),
    Player { m: usize, id: u64 },
    Compare,
    Settings,
}

impl Screen {
    pub fn dark(&self) -> bool {
        matches!(self, Screen::Onboarding | Screen::Hub(_) | Screen::Viewer { .. } | Screen::Players(_) | Screen::Player { .. } | Screen::Compare)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Prefs {
    pub onboarded: bool,
    /// Personal-use flag: allows fetching WhoScored/FotMob/SofaScore from this device.
    pub personal_fetch: bool,
    /// Respect reduced motion: no entrance animations.
    #[serde(default)]
    pub reduced_motion: bool,
    /// GitHub repository running the scrape-whoscored workflow ("owner/repo").
    #[serde(default = "default_relay")]
    pub relay_repo: String,
    /// Token used only to start relay runs / read a private relay repo. Stored in app-private storage.
    #[serde(default)]
    pub relay_token: String,
    /// "system" (default), "light" or "dark".
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Per-screen override: key "home" | "matches" | "report" | "settings", value "light" | "dark" (absent = follow `theme`).
    #[serde(default)]
    pub screen_theme: std::collections::BTreeMap<String, String>,
}

fn default_theme() -> String {
    "system".into()
}

fn default_relay() -> String {
    "hubertlapinski3-cmyk/FootballDataHub".into()
}

impl Prefs {
    pub fn relay(&self) -> Option<matchhub::sources::whoscored::Relay> {
        let repo = self.relay_repo.trim();
        if repo.is_empty() || !repo.contains('/') {
            return None;
        }
        Some(matchhub::sources::whoscored::Relay { repo: repo.to_string(), branch: "main".into(), token: Some(self.relay_token.clone()).filter(|t| !t.trim().is_empty()), wait_secs: 480 })
    }
    /// Online work is allowed with the personal flag, or when a relay is configured.
    pub fn online(&self) -> bool {
        self.personal_fetch || self.relay().is_some()
    }
}

pub struct ViewerState {
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub show_manifest: bool,
    pub hidden: std::collections::HashSet<i32>,
    pub selected: Option<(usize, i32)>,
    pub opened_at: f64,
    /// start a freshly opened tall board at its top edge
    pub at_top: bool,
}

impl ViewerState {
    pub fn fresh(now: f64) -> ViewerState {
        ViewerState { zoom: 1.0, pan: egui::Vec2::ZERO, show_manifest: false, hidden: Default::default(), selected: None, opened_at: now, at_top: true }
    }
}

pub struct CompareState {
    pub left: Option<(usize, usize)>,
    pub right: Option<(usize, usize)>,
    pub groups: [bool; 4],
    pub swipe: f32,
    pub ran: bool,
}

pub struct App {
    pub root: PathBuf,
    pub lib: Library,
    pub prefs: Prefs,
    pub stack: Vec<Screen>,
    pub raster: raster::Raster,
    pub build: Shared,
    pub input: String,
    pub toast: Option<(String, f64)>,
    pub viewer: ViewerState,
    pub compare: CompareState,
    pub theme_text: String,
    pub crest_tex: std::collections::HashMap<String, egui::TextureHandle>,
    pub last_dark: Option<bool>,
    #[cfg(target_os = "android")]
    pub android: Option<winit::platform::android::activity::AndroidApp>,
    pub kb_shown: bool,
    pub open_on_done: bool,
    pub report_cache: Option<(usize, matchhub::narrative::Report)>,
    pub manifest_cache: Option<(usize, serde_json::Value)>,
    pub scenes: scene::SceneCache,
    /// time the current screen appeared (drives staggered entrance motion)
    pub screen_t0: f64,
    pub now: f64,
    pub show_token: bool,
    /// last failed build: (input that was built, message). Drives the error card with Retry.
    pub failed: Option<(String, String)>,
}

impl App {
    pub fn new(ctx: &egui::Context, root: PathBuf) -> App {
        style::install_fonts(ctx);
        let _ = std::fs::create_dir_all(&root);
        let prefs: Prefs = std::fs::read_to_string(root.join("prefs.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let lib = Library::open(&root);
        let start = if prefs.onboarded { Screen::Home } else { Screen::Onboarding };
        if !root.join("theme_overrides.toml").exists() {
            let _ = std::fs::write(root.join("theme_overrides.toml"), DEFAULT_OVERRIDES);
        }
        let theme_text = std::fs::read_to_string(root.join("theme_overrides.toml")).unwrap_or_else(|_| DEFAULT_OVERRIDES.to_string());
        App {
            raster: raster::Raster::new(ctx, 2),
            root,
            lib,
            prefs,
            stack: vec![start],
            build: Default::default(),
            input: String::new(),
            toast: None,
            viewer: ViewerState::fresh(0.0),
            compare: CompareState { left: None, right: None, groups: [true; 4], swipe: 0.0, ran: false },
            theme_text,
            crest_tex: Default::default(),
            last_dark: None,
            #[cfg(target_os = "android")]
            android: None,
            kb_shown: false,
            open_on_done: true,
            report_cache: None,
            manifest_cache: None,
            scenes: Default::default(),
            screen_t0: 0.0,
            now: 0.0,
            show_token: false,
            failed: None,
        }
    }

    pub fn save_prefs(&self) {
        let _ = std::fs::write(self.root.join("prefs.json"), serde_json::to_string(&self.prefs).unwrap_or_default());
    }
    pub fn screen(&self) -> Screen {
        self.stack.last().cloned().unwrap_or(Screen::Home)
    }
    pub fn push(&mut self, s: Screen) {
        if let Screen::Viewer { .. } = s {
            self.viewer = ViewerState::fresh(self.now);
        }
        self.stack.push(s);
        self.screen_t0 = self.now;
    }
    pub fn back(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.screen_t0 = self.now;
        }
    }
    pub fn go_home(&mut self) {
        self.stack = vec![Screen::Home];
        self.screen_t0 = self.now;
    }
    /// Switch between top-level tabs without growing the back stack.
    pub fn tab(&mut self, s: Screen) {
        self.stack = if s == Screen::Home { vec![Screen::Home] } else { vec![Screen::Home, s] };
        self.screen_t0 = self.now;
    }
    /// Staggered entrance (0..1) for list/grid item `i` on the current screen.
    pub fn stagger(&self, i: usize) -> f32 {
        if self.prefs.reduced_motion {
            return 1.0;
        }
        let t = ((self.now - self.screen_t0) as f32 - i as f32 * 0.05) / 0.35;
        let t = t.clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }
    pub fn toast(&mut self, ctx: &egui::Context, msg: &str) {
        self.toast = Some((msg.to_string(), ctx.input(|i| i.time) + 3.5));
    }
    pub fn board_path(&self, m: usize, board: &str) -> PathBuf {
        self.lib.entries[m].dir.join("boards").join(format!("{board}.svg"))
    }

    /// Draw one frame for the current screen into `ui` (shared by eframe and tests).
    #[allow(deprecated)]
    pub fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|i| i.time);
        self.raster.poll(&ctx);
        if !self.prefs.reduced_motion && self.now - self.screen_t0 < 1.5 {
            ctx.request_repaint();
        }
        let scr = self.screen();
        let global = match self.prefs.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => ctx.system_theme() == Some(egui::Theme::Dark),
        };
        let key = match scr {
            Screen::Home => "home",
            Screen::Matches => "matches",
            Screen::Report(_) => "report",
            Screen::Settings => "settings",
            _ => "",
        };
        let force = match self.prefs.screen_theme.get(key).map(|s| s.as_str()) {
            Some("dark") => true,
            Some("light") => false,
            _ => global,
        };
        style::set_force_dark(force);
        let dark = scr.dark() || force;
        if self.last_dark != Some(dark) {
            style::apply_visuals(&ctx, dark);
            self.last_dark = Some(dark);
        }
        // Android back button / desktop Escape
        if ctx.input(|i| i.key_pressed(egui::Key::BrowserBack) || i.key_pressed(egui::Key::Escape)) && self.stack.len() > 1 {
            self.back();
        }
        // finished builds
        let (done, err) = {
            let mut g = self.build.lock().unwrap();
            (g.done.take(), g.error.take())
        };
        if let Some(id) = done {
            self.lib.reload();
            self.report_cache = None;
            self.manifest_cache = None;
            self.scenes.clear();
            // indices changed after reload: rebuild the navigation stack
            let base = if self.screen() == Screen::Onboarding { Screen::Onboarding } else { Screen::Home };
            self.stack = vec![base];
            if self.open_on_done {
                if let Some(i) = self.lib.entries.iter().position(|e| e.s.id == id) {
                    self.push(Screen::Hub(i));
                }
            }
        }
        if let Some(e) = err {
            self.failed = Some((self.input.clone(), e.to_string()));
            self.toast(&ctx, &format!("Build failed: {e}"));
        }
        let bg = if dark { style::BOARD_BG } else { style::WHITE };
        let safe = self.safe_rect(&ctx);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(bg).inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 0 })).show(ui, |ui| {
            let full = ui.max_rect();
            if dark {
                style::backdrop(ui, full);
            }
            let mut inner = full.intersect(safe);
            let top_level = matches!(scr, Screen::Home | Screen::Matches | Screen::Compare | Screen::Settings);
            if top_level {
                let nav = egui::Rect::from_min_max(egui::pos2(inner.left(), inner.bottom() - 80.0), inner.max);
                inner.max.y = nav.top();
                self.bottom_nav(ui, nav, &scr, dark);
            }
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            match scr {
                Screen::Onboarding => self.onboarding(&mut child),
                Screen::Home => self.home(&mut child),
                Screen::Matches => self.matches(&mut child),
                Screen::Hub(m) => self.hub(&mut child, m),
                Screen::Viewer { m, board } => self.board_viewer(&mut child, m, &board),
                Screen::Report(m) => self.report(&mut child, m),
                Screen::Players(m) => self.players(&mut child, m),
                Screen::Player { m, id } => self.player(&mut child, m, id),
                Screen::Compare => self.compare_screen(&mut child),
                Screen::Settings => self.settings(&mut child),
            }
        });
        // toast
        if let Some((msg, until)) = self.toast.clone() {
            if ctx.input(|i| i.time) < until {
                egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -32.0]).show(&ctx, |ui| {
                    egui::Frame::new().fill(egui::Color32::from_rgb(0x21, 0x1d, 0x2b)).corner_radius(14).inner_margin(14).show(ui, |ui| {
                        ui.set_max_width(360.0);
                        ui.label(egui::RichText::new(msg).color(egui::Color32::WHITE));
                    });
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            } else {
                self.toast = None;
            }
        }
        if self.raster.is_pending() || self.build.lock().unwrap().running {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        self.soft_keyboard(&ctx);
    }

    /// Drawable area: the window minus system bars. Android reports the content rect in pixels;
    /// when it claims the full window (edge-to-edge) fixed minimum insets keep the UI clear of
    /// the status bar and the navigation bar.
    #[allow(unused_variables)]
    fn safe_rect(&self, ctx: &egui::Context) -> egui::Rect {
        let full = ctx.content_rect();
        #[cfg(target_os = "android")]
        {
            let ppp = ctx.pixels_per_point().max(0.5);
            let mut top = 0.0f32;
            let mut bottom = 0.0f32;
            if let Some(a) = &self.android {
                let r = a.content_rect();
                if r.right > r.left && r.bottom > r.top {
                    top = r.top as f32 / ppp;
                    bottom = (full.bottom() - r.bottom as f32 / ppp).max(0.0);
                }
            }
            return egui::Rect::from_min_max(egui::pos2(full.left(), full.top() + top.max(36.0)), egui::pos2(full.right(), full.bottom() - bottom.max(44.0)));
        }
        #[cfg(not(target_os = "android"))]
        full
    }

    #[allow(unused_variables)]
    fn soft_keyboard(&mut self, ctx: &egui::Context) {
        let wants = ctx.egui_wants_keyboard_input();
        if wants != self.kb_shown {
            self.kb_shown = wants;
            #[cfg(target_os = "android")]
            if let Some(a) = &self.android {
                if wants {
                    a.show_soft_input(true);
                } else {
                    a.hide_soft_input(true);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
}

pub const DEFAULT_OVERRIDES: &str = r##"# Theme overrides, keyed by team name or source id (e.g. "ws:13").
# Every other team resolves through FotMob / SofaScore / crest colours.
#
# [teams."Arsenal"]
# primary = "#EF0107"
# secondary = "#063672"
# short_name = "Arsenal"
#
# [teams."Borussia Dortmund"]
# primary = "#FDE100"
# short_name = "Dortmund"

[teams."Spain"]
primary = "#C60B1E"
secondary = "#FFC400"

[teams."England"]
primary = "#F5F5F5"
secondary = "#1D3B8B"
"##;

pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([412.0, 915.0]).with_title("Data Hub"), ..Default::default() }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("datahub"));
    let root = app.internal_data_path().unwrap_or_else(|| PathBuf::from("/data/local/tmp/datahub"));
    let mut opts = native_options();
    opts.android_app = Some(app.clone());
    let _ = eframe::run_native(
        "Data Hub",
        opts,
        Box::new(move |cc| {
            let mut a = App::new(&cc.egui_ctx, root);
            a.android = Some(app);
            Ok(Box::new(a))
        }),
    );
}
