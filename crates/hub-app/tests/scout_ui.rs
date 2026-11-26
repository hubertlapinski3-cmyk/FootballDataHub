//! League browser, league scout and cross-league compare, headless, against the bundled multi-league
//! fixture (synthetic clubs and players; no network).
use hub_app::{data::COLUMNS, screens::SCOUT_KEY, App, Screen};
use matchhub::catalog::*;

fn run(app: &mut App, ctx: &egui::Context, w: f32, frames: usize) {
    for _ in 0..frames {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, 915.0))), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
    }
}

fn app_with_fixture(tag: &str) -> (App, egui::Context) {
    let dir = std::env::temp_dir().join(format!("datahub_scoutui_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("catalog")).unwrap();
    let fx = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hub-core/tests/fixtures/scout");
    for f in ["catalog.json", "scout_index.json"] {
        std::fs::copy(fx.join(f), dir.join("catalog").join(f)).unwrap();
    }
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, dir);
    app.prefs.onboarded = true;
    app.data.refresh_started = true; // never touch the network in tests
    (app, ctx)
}

#[test]
fn cached_files_load_at_start_and_filters_narrow_the_leaderboard() {
    let (mut app, _ctx) = app_with_fixture("filters");
    assert!(app.data.catalog.is_some() && app.data.index.is_some());
    let all = app.data.leaderboard().len();
    assert!(all > 100);
    // role, league and minutes filters
    app.data.s_role = 1; // FW (first of ScoutRole::ALL)
    let fw = app.data.leaderboard();
    assert!(!fw.is_empty() && fw.iter().all(|p| p.role == Some(ScoutRole::FW)));
    let fw_n = fw.len();
    app.data.s_league = 1; // Top 5
    assert!(app.data.leaderboard().iter().all(|p| League::from_slug(&p.league).unwrap().top5()));
    app.data.s_league = 8; // Ekstraklasa
    assert!(app.data.leaderboard().iter().all(|p| p.league == "ekstraklasa"));
    app.data.s_league = 0;
    app.data.s_min = 2; // 900'
    assert!(app.data.leaderboard().len() <= fw_n && app.data.leaderboard().iter().all(|p| p.minutes >= 900.0));
    app.data.s_min = 0;
    // sorting: every column, both directions
    for k in 0..COLUMNS.len() {
        app.data.s_sort = k;
        app.data.s_desc = true;
        let v: Vec<f64> = app.data.leaderboard().iter().map(|p| p.value(COLUMNS[k].1)).collect();
        assert!(v.windows(2).all(|w| w[0] >= w[1]), "{} descending", COLUMNS[k].0);
        app.data.s_desc = false;
        let v: Vec<f64> = app.data.leaderboard().iter().map(|p| p.value(COLUMNS[k].1)).collect();
        assert!(v.windows(2).all(|w| w[0] <= w[1]), "{} ascending", COLUMNS[k].0);
    }
    // search by club
    let team = app.data.leaderboard()[0].team.clone();
    app.data.s_query = team.to_uppercase();
    assert!(app.data.leaderboard().iter().all(|p| p.team == team));
}

#[test]
fn league_pills_scout_and_season_card_screens_render_at_phone_and_compact_widths() {
    let (mut app, ctx) = app_with_fixture("screens");
    let ix = app.data.index.clone().unwrap();
    let fw = ix.players.iter().filter(|p| p.role == Some(ScoutRole::FW) && p.minutes >= 270.0).map(|p| p.id).take(2).collect::<Vec<_>>();
    for w in [412.0, 320.0] {
        for (league, label) in [(0usize, "saved"), (1, "all"), (3, "one")] {
            app.data.league = league;
            for s in [Screen::Home, Screen::Matches] {
                app.stack = vec![Screen::Home, s];
                run(&mut app, &ctx, w, 3);
            }
            let _ = label;
        }
        app.data.league = 0;
        app.data.scout_on = true;
        app.stack = vec![Screen::Home, Screen::Scout];
        run(&mut app, &ctx, w, 3);
        app.stack = vec![Screen::Home, Screen::ScoutPlayer(fw[0])];
        run(&mut app, &ctx, w, 3);
        // cross-league compare of two league players
        app.compare.players_mode = true;
        app.compare.pl_left = Some((SCOUT_KEY, fw[0]));
        app.compare.pl_right = Some((SCOUT_KEY, fw[1]));
        app.stack = vec![Screen::Home, Screen::Compare];
        run(&mut app, &ctx, w, 3);
    }
    // an unknown player id goes back instead of panicking
    app.stack = vec![Screen::Home, Screen::ScoutPlayer(1)];
    run(&mut app, &ctx, 412.0, 2);
    assert_eq!(app.screen(), Screen::Home);
}

#[test]
fn opening_a_remote_match_without_a_repository_says_so_and_does_not_start_a_job() {
    let (mut app, ctx) = app_with_fixture("remote");
    app.prefs.relay_repo = String::new();
    let m = app.data.catalog.clone().unwrap().leagues[0].matches[0].clone();
    app.open_remote(&ctx, &m);
    assert!(!app.build.lock().unwrap().running);
    assert!(app.toast.is_some());
}
