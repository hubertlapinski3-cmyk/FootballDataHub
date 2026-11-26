//! On-device library of built matches: build jobs, persistence, summaries.

use anyhow::{Context, Result};
use matchhub::model::Side;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardMeta {
    pub id: String,
    pub title: String,
    pub section: String,
    pub how: String,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMeta {
    pub id: u64,
    pub name: String,
    pub short: String,
    pub side: usize,
    pub role: String,
    pub shirt: u32,
    pub minutes: f64,
    pub touches: usize,
    pub shots: usize,
    pub goals: usize,
    pub xg: f64,
    pub xa: f64,
    pub vaep: f64,
    pub board: String,
    /// Event-derived figures for the player comparison; absent in summaries built before they existed.
    #[serde(default)]
    pub extra: Option<PlayerExtra>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlayerExtra {
    pub passes: usize,
    pub pass_pct: f64,
    pub prog_passes: usize,
    pub line_breaks: usize,
    pub xgchain: f64,
    pub box_entries: usize,
    pub ft_receptions: usize,
    pub take_ons_won: usize,
    pub positive_xt: f64,
    pub recoveries: usize,
    pub aerials_won: usize,
    pub aerials: usize,
    pub def_actions: usize,
    pub key_passes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamCompare {
    pub name: String,
    pub goals: usize,
    pub xg: f64,
    pub npxg: f64,
    pub xgot: f64,
    pub shots: usize,
    pub big_chances: usize,
    pub pass_xt: f64,
    pub progressive: usize,
    pub line_breaking: (usize, usize),
    pub box_entries: usize,
    pub box_entries_shot: usize,
    pub field_tilt: f64,
    pub possession: f64,
    pub ppda: Option<f64>,
    pub tackles: usize,
    pub interceptions: usize,
    pub blocks: usize,
    pub clearances: usize,
    pub recoveries: usize,
    pub regains: usize,
    pub high_regains: usize,
    pub fouls: usize,
    pub aerials: (usize, usize),
    pub transitions: usize,
    pub counterpress: (usize, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub id: String,
    pub source: String,
    pub home: String,
    pub away: String,
    pub home_short: String,
    pub away_short: String,
    pub score: [u32; 2],
    pub competition: String,
    pub season: String,
    pub date: String,
    pub colours: [String; 2],
    pub muted: [String; 2],
    pub on_primary: [String; 2],
    pub headline: String,
    pub npxg: [f64; 2],
    pub ppda: [Option<f64>; 2],
    pub xpts: [f64; 2],
    pub momentum: [Vec<f32>; 2],
    pub goals: Vec<(usize, f32, String)>,
    pub boards: Vec<BoardMeta>,
    pub players: Vec<PlayerMeta>,
    pub teams: [TeamCompare; 2],
    pub built_at: String,
    pub metric_version: String,
    pub fallbacks: Vec<String>,
    pub theme_decisions: Vec<String>,
    pub crest_files: [Option<String>; 2],
}

#[derive(Clone)]
pub struct Entry {
    pub dir: PathBuf,
    pub s: Summary,
}

pub struct Library {
    pub root: PathBuf,
    /// shared, immutable match summaries: screens take cheap `Arc` handles, never deep copies
    pub entries: Vec<std::sync::Arc<Entry>>,
}

impl Library {
    pub fn open(root: &Path) -> Library {
        let mut lib = Library { root: root.to_path_buf(), entries: vec![] };
        lib.reload();
        lib
    }
    pub fn matches_dir(&self) -> PathBuf {
        self.root.join("matches")
    }
    pub fn reload(&mut self) {
        let mut v = vec![];
        if let Ok(rd) = std::fs::read_dir(self.matches_dir()) {
            for d in rd.flatten() {
                let p = d.path().join("summary.json");
                if let Ok(s) = std::fs::read_to_string(&p) {
                    if let Ok(s) = serde_json::from_str::<Summary>(&s) {
                        v.push(std::sync::Arc::new(Entry { dir: d.path(), s }));
                    }
                }
            }
        }
        v.sort_by(|a, b| b.s.built_at.cmp(&a.s.built_at));
        self.entries = v;
    }
    pub fn delete(&mut self, idx: usize) {
        if let Some(e) = self.entries.get(idx) {
            let _ = std::fs::remove_dir_all(&e.dir);
        }
        self.reload();
    }
}

fn team_compare(a: &matchhub::derive::metrics::Analysis, s: Side) -> TeamCompare {
    let t = a.t(s);
    TeamCompare {
        name: t.name.clone(),
        goals: t.goals + t.own_goals_for,
        xg: t.xg,
        npxg: t.npxg,
        xgot: t.xgot,
        shots: t.shots.len(),
        big_chances: t.clear_chances.len(),
        pass_xt: t.positive_xt,
        progressive: t.progressive_passes,
        line_breaking: t.line_breaking,
        box_entries: t.entries.len(),
        box_entries_shot: t.entries.iter().filter(|e| e.shot_followed).count(),
        field_tilt: t.field_tilt,
        possession: t.possession_pct,
        ppda: t.ppda,
        tackles: t.tackles,
        interceptions: t.interceptions,
        blocks: t.blocks,
        clearances: t.clearances,
        recoveries: t.recoveries,
        regains: t.regains_all,
        high_regains: t.high_regains.len(),
        fouls: t.fouls,
        aerials: t.aerials,
        transitions: t.transitions.len(),
        counterpress: t.counterpress,
    }
}

/// Data-driven one-line headline for the match hub.
fn headline(o: &matchhub::BuildOutput) -> String {
    let a = &o.a;
    let lead = if a.t(Side::Home).npxg >= a.t(Side::Away).npxg { Side::Home } else { Side::Away };
    let t = a.t(lead);
    let shot_n = t.entries.iter().filter(|e| e.shot_followed).count();
    format!("{} got into the box {} times, {} led to a shot", t.name, t.entries.len(), shot_n)
}

#[derive(Clone, Default)]
pub struct BuildState {
    pub running: bool,
    pub step: String,
    /// what the step is doing right now (page fetch, browser, relay wait, ...)
    pub detail: String,
    pub frac: f32,
    pub error: Option<String>,
    pub done: Option<String>,
    pub log: Vec<String>,
}

pub type Shared = Arc<Mutex<BuildState>>;

pub enum Source {
    WhoScored(String),
    Sample,
}

/// Write the bundled StatsBomb sample (Euro 2024 final) into the data dir.
pub fn ensure_sample(root: &Path) -> Result<PathBuf> {
    use std::io::Read;
    let sb = root.join("statsbomb");
    let files: [(&str, &[u8]); 3] = [
        ("matches/55/282.json", include_bytes!("../assets/sample/matches/55/282.json")),
        ("events/3943043.json.gz", include_bytes!("../assets/sample/events/3943043.json.gz")),
        ("lineups/3943043.json", include_bytes!("../assets/sample/lineups/3943043.json")),
    ];
    for (rel, data) in files {
        let (rel, data): (String, Vec<u8>) = if let Some(r) = rel.strip_suffix(".gz") {
            let mut s = vec![];
            flate2::read::GzDecoder::new(data).read_to_end(&mut s)?;
            (r.to_string(), s)
        } else {
            (rel.to_string(), data.to_vec())
        };
        let p = sb.join(rel);
        if !p.exists() {
            std::fs::create_dir_all(p.parent().unwrap())?;
            std::fs::write(&p, data)?;
        }
    }
    Ok(sb)
}

pub fn build_match(root: &Path, src: Source, online: bool, relay: Option<matchhub::sources::whoscored::Relay>, browser: Option<matchhub::sources::whoscored::Browser>, state: Shared) -> Result<String> {
    let models = matchhub::embedded_models();
    let input = match src {
        Source::WhoScored(s) => matchhub::Input::WhoScored(s),
        Source::Sample => matchhub::Input::StatsBomb(ensure_sample(root)?, 3943043),
    };
    let overrides = root.join("theme_overrides.toml");
    let opts = matchhub::BuildOptions {
        cache_dir: root.join("cache"),
        online,
        overrides: overrides.exists().then_some(overrides),
        creator: "Data Hub".into(),
        store_path: Some(root.join("cache").join("team_match.jsonl")),
        logo: None,
        relay,
        browser,
    };
    let st = state.clone();
    let mut progress = move |step: &str, f: f32| {
        let mut g = st.lock().unwrap();
        // "ingest: fetching WhoScored" -> step "ingest", detail "fetching WhoScored"
        let (key, detail) = step.split_once(": ").unwrap_or((step, ""));
        g.step = key.to_string();
        g.detail = detail.to_string();
        g.frac = f;
        g.log.push(format!("{step} {:.0}%", f * 100.0));
    };
    let o = matchhub::build(&input, &opts, &models, &mut progress).context("build")?;
    let dir = root.join("matches").join(o.m.id.replace(['/', ':'], "_"));
    let _ = std::fs::remove_dir_all(&dir);
    matchhub::cancel::check()?;
    matchhub::write_output(&o, &dir, false)?;
    let mut crest_files: [Option<String>; 2] = [None, None];
    for i in 0..2 {
        if let Some(p) = &o.crest_paths[i] {
            let dst = dir.join(format!("crest_{i}.png"));
            if std::fs::copy(p, &dst).is_ok() {
                crest_files[i] = Some(dst.file_name().unwrap().to_string_lossy().into());
            }
        }
    }
    let side_idx = |s: Side| s.idx();
    let players = o
        .a
        .players
        .iter()
        .map(|p| {
            let sd = p.side.unwrap();
            PlayerMeta {
                id: p.id,
                name: p.name.clone(),
                short: p.short.clone(),
                side: side_idx(sd),
                role: p.role.map(|r| r.label().to_string()).unwrap_or_default(),
                shirt: p.shirt,
                minutes: p.minutes,
                touches: p.touches,
                shots: p.shots,
                goals: p.goals,
                xg: p.xg,
                xa: p.inferred_xa,
                vaep: p.vaep_off + p.vaep_def,
                board: format!("31_player_{}_{}", if sd == Side::Home { "home" } else { "away" }, p.id),
                extra: Some(PlayerExtra {
                    passes: p.passes,
                    pass_pct: if p.passes > 0 { p.passes_completed as f64 / p.passes as f64 } else { 0.0 },
                    prog_passes: p.progressive_passes,
                    line_breaks: p.line_breaks,
                    xgchain: p.xgchain,
                    box_entries: p.box_entries,
                    ft_receptions: p.ft_receptions,
                    take_ons_won: p.take_ons.0,
                    positive_xt: p.positive_xt,
                    recoveries: p.recoveries,
                    aerials_won: p.aerials.0,
                    aerials: p.aerials.1,
                    def_actions: p.def_actions,
                    key_passes: p.key_passes,
                }),
            }
        })
        .collect();
    let goals = o.m.goals_timeline().iter().map(|(s, e)| (s.idx(), (e.t / 60.0) as f32, format!("{} {}", o.m.player_name(e.side, e.player), e.label_minute()))).collect();
    let s = Summary {
        id: o.m.id.clone(),
        source: o.m.source.clone(),
        home: o.m.teams[0].name.clone(),
        away: o.m.teams[1].name.clone(),
        home_short: o.m.teams[0].short_name.clone(),
        away_short: o.m.teams[1].short_name.clone(),
        score: [o.m.teams[0].score, o.m.teams[1].score],
        competition: o.m.competition.clone(),
        season: o.m.season.clone(),
        date: o.m.date.clone(),
        colours: [o.theme.home.primary.clone(), o.theme.away.primary.clone()],
        muted: [o.theme.home.muted.clone(), o.theme.away.muted.clone()],
        on_primary: [o.theme.home.on_primary.clone(), o.theme.away.on_primary.clone()],
        headline: headline(&o),
        npxg: [o.a.t(Side::Home).npxg, o.a.t(Side::Away).npxg],
        ppda: [o.a.t(Side::Home).ppda, o.a.t(Side::Away).ppda],
        xpts: [o.a.win.xpts_home, o.a.win.xpts_away],
        momentum: [o.a.momentum.home.iter().map(|v| *v as f32).collect(), o.a.momentum.away.iter().map(|v| *v as f32).collect()],
        goals,
        boards: o.boards.iter().map(|b| BoardMeta { id: b.id.clone(), title: b.title.clone(), section: b.section.clone(), how: b.how.clone(), width: b.width, height: b.height }).collect(),
        players,
        teams: [team_compare(&o.a, Side::Home), team_compare(&o.a, Side::Away)],
        built_at: chrono::Utc::now().to_rfc3339(),
        metric_version: matchhub::METRIC_VERSION.into(),
        fallbacks: o.manifest.fallbacks.clone(),
        theme_decisions: o.theme.decisions.clone(),
        crest_files,
    };
    std::fs::write(dir.join("summary.json"), serde_json::to_string(&s)?)?;
    {
        let mut g = state.lock().unwrap();
        g.step = "done".into();
        g.detail.clear();
        g.frac = 1.0;
    }
    Ok(s.id)
}

pub fn spawn_build(root: PathBuf, src: Source, online: bool, relay: Option<matchhub::sources::whoscored::Relay>, browser: Option<matchhub::sources::whoscored::Browser>, state: Shared, ctx: egui::Context) {
    {
        let mut g = state.lock().unwrap();
        *g = BuildState { running: true, step: "queued".into(), ..Default::default() };
    }
    matchhub::cancel::reset();
    std::thread::spawn(move || {
        let r = build_match(&root, src, online, relay, browser, state.clone());
        let mut g = state.lock().unwrap();
        g.running = false;
        match r {
            Ok(id) => g.done = Some(id),
            Err(e) => g.error = Some(format!("{e:#}")),
        }
        ctx.request_repaint();
    });
}

pub fn dir_size(p: &Path) -> u64 {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            let m = e.metadata();
            if let Ok(m) = m {
                if m.is_dir() {
                    n += dir_size(&e.path());
                } else {
                    n += m.len();
                }
            }
        }
    }
    n
}
