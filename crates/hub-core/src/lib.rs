//! Data Hub core: ingest -> enrich -> models -> derive -> theme -> boards -> report.
//! Platform-agnostic: used by the CLI and by the Android/desktop app.

pub mod cancel;
pub mod catalog;
pub mod derive;
pub mod fonts;
pub mod ingest;
pub mod ml;
pub mod model;
pub mod net;
pub mod narrative;
pub mod reep;
pub mod render;
pub mod scrape;
pub mod sim;
pub mod sources;
pub mod store;
pub mod theme;

use anyhow::{Context, Result};
use model::*;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const METRIC_VERSION: &str = "1.0.0";
/// Models trained on StatsBomb Open Data, embedded so the app works offline.
pub static EMBEDDED_MODELS: &str = include_str!("../assets/models.json");

pub fn embedded_models() -> ml::Models {
    serde_json::from_str(EMBEDDED_MODELS).unwrap_or_default()
}

#[derive(Debug, Clone)]
pub enum Input {
    /// WhoScored match id, URL, saved .html page or extracted .json.
    WhoScored(String),
    /// StatsBomb open-data root + match id.
    StatsBomb(PathBuf, u64),
    /// A match packet from the league catalog (gzip JSON of the parsed `Match`): no scraping at all.
    Packet(PathBuf),
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub cache_dir: PathBuf,
    pub online: bool,
    pub overrides: Option<PathBuf>,
    pub creator: String,
    pub store_path: Option<PathBuf>,
    pub logo: Option<PathBuf>,
    pub relay: Option<sources::whoscored::Relay>,
    /// Browser-engine fallback for challenge pages (Android WebView); None on desktop.
    pub browser: Option<sources::whoscored::Browser>,
    /// Season scouting index for season-long role percentiles on player cards; None ranks within the match.
    pub scout: Option<std::sync::Arc<catalog::ScoutIndex>>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions { cache_dir: PathBuf::from(".hub_cache"), online: true, overrides: None, creator: "Data Hub".into(), store_path: None, logo: None, relay: None, browser: None, scout: None }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub match_id: String,
    pub source: String,
    pub metric_version: String,
    pub models: serde_json::Value,
    pub sources_used: Vec<String>,
    pub built_at: String,
    pub fallbacks: Vec<String>,
    pub theme: theme::MatchTheme,
    pub checks: Vec<derive::metrics::Check>,
}

pub struct BuildOutput {
    pub m: Match,
    pub a: derive::metrics::Analysis,
    pub theme: theme::MatchTheme,
    pub boards: Vec<render::Board>,
    pub report: narrative::Report,
    pub manifest: Manifest,
    pub crests: [Option<String>; 2],
    pub crest_paths: [Option<PathBuf>; 2],
}

/// Progress callback: (step name, fraction 0..1).
pub type Progress<'a> = &'a mut dyn FnMut(&str, f32);

fn fetch_crest(url: &str, cache: &Path, key: &str) -> Option<PathBuf> {
    let p = cache.join("crests").join(format!("{key}.png"));
    if p.exists() {
        return Some(p);
    }
    let b = net::get_ok(url, &[], 15).ok()?;
    std::fs::create_dir_all(p.parent()?).ok()?;
    std::fs::write(&p, &b).ok()?;
    Some(p)
}

pub fn load_match(input: &Input, opts: &BuildOptions, progress: &mut dyn FnMut(&str)) -> Result<Match> {
    match input {
        Input::WhoScored(s) => sources::whoscored::load(s, &opts.cache_dir, if opts.online { opts.relay.as_ref() } else { None }, if opts.online { opts.browser.as_ref() } else { None }, progress),
        Input::StatsBomb(root, id) => sources::statsbomb::load(root, *id),
        Input::Packet(p) => catalog::read_packet(p),
    }
}

pub fn build(input: &Input, opts: &BuildOptions, models: &ml::Models, progress: Progress) -> Result<BuildOutput> {
    progress("ingest", 0.05);
    let mut m = load_match(input, opts, &mut |msg: &str| progress(&format!("ingest: {msg}"), 0.05)).context("ingest")?;
    progress("enrich", 0.15);
    sources::enrich::enrich(&mut m, &opts.cache_dir, opts.online);
    // ages the event feed lacks come from the bundled Reep register (CC0)
    let (got, want) = reep::fill_ages(&mut m);
    if want > 0 {
        m.fallbacks.retain(|f| !f.contains("player ages unavailable"));
        if got < want {
            m.fallbacks.push(format!("ages for {} of {} players not found in the Reep register", want - got, want));
        }
    }
    let ov = theme::load_overrides(opts.overrides.as_deref());
    for t in m.teams.iter_mut() {
        if let Some(o) = ov.teams.get(&t.source_id).or_else(|| ov.teams.iter().find(|(k, _)| k.eq_ignore_ascii_case(&t.name)).map(|(_, v)| v)) {
            if let Some(sn) = &o.short_name {
                t.short_name = sn.clone();
            }
            if let Some(c) = &o.crest {
                t.crest_url = Some(c.clone());
            }
        }
    }
    progress("model", 0.3);
    let values = models.apply(&mut m);
    let win = sim::simulate(&m, 20_000);
    let live = sim::live_curve(&m, 1.0);
    progress("derive", 0.45);
    let a = derive::metrics::analyse(&m, values, win, live);

    // crests (online or local override path)
    let mut crest_paths: [Option<PathBuf>; 2] = [None, None];
    for (i, t) in m.teams.iter().enumerate() {
        if let Some(u) = &t.crest_url {
            let p = Path::new(u);
            if p.exists() {
                crest_paths[i] = Some(p.to_path_buf());
            } else if opts.online {
                crest_paths[i] = fetch_crest(u, &opts.cache_dir, &t.source_id.replace(':', "_"));
            }
        }
    }
    if crest_paths.iter().any(|c| c.is_none()) {
        m.fallbacks.push("crest unavailable for at least one team: monogram disc used".into());
    }
    let th = theme::resolve(&m, &ov, [crest_paths[0].as_deref(), crest_paths[1].as_deref()]);
    let crests = [crest_paths[0].as_deref().and_then(render::crest_data_uri), crest_paths[1].as_deref().and_then(render::crest_data_uri)];

    progress("render", 0.6);
    let store_rows = store::rows_for(&m, &a);
    let mut all_rows = opts.store_path.as_deref().map(store::load).unwrap_or_default();
    all_rows.retain(|r| r.match_id != m.id);
    all_rows.extend(store_rows.clone());
    if let Some(p) = &opts.store_path {
        let _ = store::upsert(p, &store_rows);
    }
    let ctx = render::frame::Ctx {
        m: &m,
        a: &a,
        th: &th,
        crests: crests.clone(),
        creator: opts.creator.clone(),
        logo: opts.logo.as_deref().and_then(render::crest_data_uri),
        footer_tag: "FULL VISUAL REDESIGN · REAL MATCH DATA".into(),
        scout: opts.scout.as_deref(),
    };
    let boards = render::all_boards(&ctx, &all_rows);
    progress("report", 0.9);
    let report = narrative::generate(&m, &a, &boards);
    let manifest = Manifest {
        match_id: m.id.clone(),
        source: m.source.clone(),
        metric_version: METRIC_VERSION.into(),
        models: serde_json::json!({
            "xg": format!("{} (logistic, {} shots, log-loss {:.4})", models.version, models.xg.n_train, models.xg.log_loss),
            "psxg_local": format!("{} (placement logistic, {} shots)", models.version, models.psxg.n_train),
            "xt": format!("{} (16x12 value iteration)", models.version),
            "vaep": format!("{} (logistic pair, {} actions)", models.version, models.vaep_score.n_train),
            "training_set": format!("{} ({} matches)", models.trained_on, models.training_matches),
        }),
        sources_used: {
            let mut v = vec![m.source.clone()];
            if m.events.iter().any(|e| e.shot.as_ref().map(|s| s.xgot_ext.is_some()).unwrap_or(false)) {
                v.push("fotmob/sofascore xGOT".into());
            }
            v
        },
        built_at: chrono::Utc::now().to_rfc3339(),
        fallbacks: m.fallbacks.clone(),
        theme: th.clone(),
        checks: a.checks.clone(),
    };
    progress("done", 1.0);
    Ok(BuildOutput { m, a, theme: th, boards, report, manifest, crests, crest_paths })
}

/// Write SVG + PNG boards, report and manifests to a directory.
pub fn write_output(out: &BuildOutput, dir: &Path, png: bool) -> Result<()> {
    let bdir = dir.join("boards");
    std::fs::create_dir_all(&bdir)?;
    for b in &out.boards {
        std::fs::write(bdir.join(format!("{}.svg", b.id)), &b.svg)?;
        std::fs::write(bdir.join(format!("{}.scene.json", b.id)), &b.scene)?;
        if png {
            render::save_png(&b.svg, &bdir.join(format!("{}.png", b.id)), 1.0)?;
        }
        let mut man = serde_json::to_value(&out.manifest)?;
        man["board"] = serde_json::Value::String(b.id.clone());
        std::fs::write(bdir.join(format!("{}.manifest.json", b.id)), serde_json::to_string_pretty(&man)?)?;
    }
    std::fs::write(dir.join("report.md"), narrative::to_markdown(&out.report))?;
    std::fs::write(dir.join("report.json"), serde_json::to_string_pretty(&out.report)?)?;
    std::fs::write(dir.join("analysis.json"), serde_json::to_string(&out.a)?)?;
    std::fs::write(dir.join("manifest.json"), serde_json::to_string_pretty(&out.manifest)?)?;
    let index: Vec<&render::Board> = out.boards.iter().collect();
    std::fs::write(dir.join("boards.json"), serde_json::to_string_pretty(&index)?)?;
    Ok(())
}
