//! Runs every screen headlessly through egui (no GPU) in empty and populated states.
use hub_app::{App, Screen};

fn run(app: &mut App, ctx: &egui::Context, frames: usize) {
    for _ in 0..frames {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(412.0, 915.0))), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
    }
}

#[test]
fn empty_library_screens_do_not_panic() {
    let dir = std::env::temp_dir().join(format!("datahub_smoke_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, dir.clone());
    app.prefs.onboarded = true;
    for s in [Screen::Home, Screen::Matches, Screen::Compare, Screen::Settings] {
        app.stack = vec![Screen::Home, s];
        run(&mut app, &ctx, 3);
    }
    // stale indices must not panic
    app.stack = vec![Screen::Home, Screen::Hub(7)];
    run(&mut app, &ctx, 2);
    assert_eq!(app.screen(), Screen::Home);
}

#[test]
fn built_match_screens_and_back_navigation() {
    let dir = std::env::temp_dir().join(format!("datahub_smoke_b_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, dir.clone());
    hub_app::library::build_match(&dir, hub_app::library::Source::Sample, false, None, None, Default::default()).unwrap();
    app.lib.reload();
    let e = &app.lib.entries[0].s;
    assert_eq!(e.score, [2, 1]);
    assert!(e.boards.len() >= 39, "boards: {}", e.boards.len());
    let pid = e.players[0].id;
    let screens = vec![Screen::Hub(0), Screen::Viewer { m: 0, board: "10_pass_network_home_h1".into() }, Screen::Report(0), Screen::Players(0), Screen::Player { m: 0, id: pid }, Screen::Compare, Screen::Onboarding];
    for s in screens {
        app.stack = vec![Screen::Home, s];
        run(&mut app, &ctx, 3);
    }
    // Android back button pops the stack
    app.stack = vec![Screen::Home, Screen::Hub(0)];
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(412.0, 915.0))),
        events: vec![egui::Event::Key { key: egui::Key::BrowserBack, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }],
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
    assert_eq!(app.screen(), Screen::Home);
}
