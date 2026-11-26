//! Board viewer touch physics: fling inertia, eased zoom and press-and-hold scrubbing.

use egui::{Event, Modifiers, PointerButton, Pos2};
use hub_app::{App, Screen};

struct Rig {
    ctx: egui::Context,
    app: App,
    t: f64,
}

impl Rig {
    fn new() -> Rig {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("datahub_physics_{}_{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = egui::Context::default();
        let mut app = App::new(&ctx, dir.clone());
        app.prefs.onboarded = true;
        hub_app::library::build_match(&dir, hub_app::library::Source::Sample, false, None, None, Default::default()).unwrap();
        app.lib.reload();
        app.stack = vec![Screen::Home, Screen::Hub(0), Screen::Viewer { m: 0, board: "07_shot_map_home".into() }];
        let mut r = Rig { ctx, app, t: 10.0 };
        r.frames(4, vec![]);
        r
    }
    fn frame(&mut self, events: Vec<Event>) {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(412.0, 915.0))), time: Some(self.t), events, ..Default::default() };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
    }
    fn frames(&mut self, n: usize, events: Vec<Event>) {
        for i in 0..n {
            self.frame(if i == 0 { events.clone() } else { vec![] });
        }
    }
    fn press(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p)]);
        self.frame(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
    }
    fn release(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
    }
}

#[test]
fn flick_keeps_moving_after_release_and_slows_down() {
    let mut r = Rig::new();
    let start = r.app.viewer.pan;
    // drag the board upward quickly
    r.press(Pos2::new(200.0, 600.0));
    let mut y = 600.0;
    for _ in 0..3 {
        y -= 25.0;
        r.frame(vec![Event::PointerMoved(Pos2::new(200.0, y))]);
    }
    r.release(Pos2::new(200.0, y));
    let v0 = r.app.viewer.pan_vel.length();
    let p0 = r.app.viewer.pan.y;
    assert!(p0 < start.y - 50.0, "the drag scrolled the board: {start:?} -> {p0}");
    assert!(v0 > 100.0, "release leaves velocity: {v0}");
    r.frames(10, vec![]);
    let p1 = r.app.viewer.pan.y;
    let v1 = r.app.viewer.pan_vel.length();
    assert!(p1 < p0 - 20.0, "keeps gliding after release: {p0} -> {p1}");
    assert!(v1 < v0 * 0.6 && v1 > 0.0, "decays exponentially: {v0} -> {v1}");
    r.frames(240, vec![]);
    assert!(r.app.viewer.pan_vel.length() <= 1.0, "comes to rest: {}", r.app.viewer.pan_vel.length());
}

#[test]
fn zoom_eases_to_its_target() {
    let mut r = Rig::new();
    assert!((r.app.viewer.zoom - 1.0).abs() < 1e-4);
    r.app.viewer.target_zoom = Some(2.5);
    r.frames(1, vec![]);
    let z1 = r.app.viewer.zoom;
    assert!(z1 > 1.0 && z1 < 2.5, "moves part of the way in one frame, not a snap: {z1}");
    r.frames(6, vec![]);
    let z2 = r.app.viewer.zoom;
    assert!(z2 > z1 && z2 < 2.5, "keeps approaching: {z1} -> {z2}");
    r.frames(240, vec![]);
    assert!((r.app.viewer.zoom - 2.5).abs() < 1e-3, "lands on the target: {}", r.app.viewer.zoom);
    assert!(r.app.viewer.target_zoom.is_none());
}

#[test]
fn holding_a_finger_still_starts_scrubbing_and_release_ends_it() {
    let mut r = Rig::new();
    r.press(Pos2::new(200.0, 400.0));
    assert!(!r.app.viewer.scrubbing, "a fresh press is not a scrub yet");
    r.frames(25, vec![]); // about 0.4 s held still
    assert!(r.app.viewer.scrubbing, "held press scrubs");
    // while scrubbing the board does not pan
    let pan = r.app.viewer.pan;
    r.frames(1, vec![Event::PointerMoved(Pos2::new(200.0, 300.0))]);
    assert_eq!(r.app.viewer.pan, pan, "dragging while scrubbing does not move the board");
    r.release(Pos2::new(200.0, 300.0));
    r.frames(2, vec![]);
    assert!(!r.app.viewer.scrubbing);
}
