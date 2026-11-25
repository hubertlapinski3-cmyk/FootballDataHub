use anyhow::Result;
use clap::{Parser, Subcommand};
use matchhub::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "matchhub", version, about = "Data Hub match analysis pipeline")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Train xG / PSxG / xT / VAEP on StatsBomb Open Data
    Train {
        #[arg(long)]
        sb_root: PathBuf,
        #[arg(long, default_value = "assets/models.json")]
        out: PathBuf,
        /// competition/season pairs, e.g. 43/106 55/43
        #[arg(long, num_args = 1.., default_values = ["43/106", "55/43", "43/3"])]
        seasons: Vec<String>,
    },
    /// Build every board, the report and manifests for one match
    Build {
        /// WhoScored match id, URL, saved .html or .json
        #[arg(long)]
        whoscored: Option<String>,
        /// StatsBomb match id (requires --sb-root)
        #[arg(long)]
        statsbomb: Option<u64>,
        #[arg(long)]
        sb_root: Option<PathBuf>,
        #[arg(long, default_value = "out")]
        out: PathBuf,
        #[arg(long)]
        theme_override: Option<PathBuf>,
        #[arg(long, default_value = ".hub_cache")]
        cache: PathBuf,
        #[arg(long, default_value = ".hub_cache/team_match.jsonl")]
        store: PathBuf,
        #[arg(long)]
        offline: bool,
        #[arg(long)]
        no_png: bool,
        #[arg(long, default_value = "Data Hub")]
        creator: String,
        #[arg(long)]
        models: Option<PathBuf>,
    },
    /// Fill the competition store from every StatsBomb match of a season
    Season {
        #[arg(long)]
        sb_root: PathBuf,
        #[arg(long)]
        competition: u64,
        #[arg(long)]
        season: u64,
        #[arg(long, default_value = ".hub_cache/team_match.jsonl")]
        store: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Train { sb_root, out, seasons } => {
            let mut ms = vec![];
            for cs in seasons {
                let (c, s) = cs.split_once('/').expect("competition/season");
                let metas = sources::statsbomb::ensure_season(&sb_root, c.parse()?, s.parse()?)?;
                for m in metas {
                    let id = m["match_id"].as_u64().unwrap();
                    if sources::statsbomb::ensure_match_files(&sb_root, id).is_err() {
                        continue;
                    }
                    match sources::statsbomb::load(&sb_root, id) {
                        Ok(x) => ms.push(x),
                        Err(e) => eprintln!("skip {id}: {e}"),
                    }
                }
            }
            eprintln!("training on {} matches", ms.len());
            let models = ml::Models::train(&ms);
            std::fs::write(&out, serde_json::to_string(&models)?)?;
            eprintln!("wrote {}", out.display());
        }
        Cmd::Build { whoscored, statsbomb, sb_root, out, theme_override, cache, store, offline, no_png, creator, models } => {
            let input = match (whoscored, statsbomb) {
                (Some(w), _) => Input::WhoScored(w),
                (None, Some(id)) => Input::StatsBomb(sb_root.expect("--sb-root required"), id),
                _ => anyhow::bail!("pass --whoscored or --statsbomb"),
            };
            let models = match models {
                Some(p) => serde_json::from_str(&std::fs::read_to_string(p)?)?,
                None => embedded_models(),
            };
            let opts = BuildOptions { cache_dir: cache, online: !offline, overrides: theme_override, creator, store_path: Some(store), logo: None };
            let t0 = std::time::Instant::now();
            let o = build(&input, &opts, &models, &mut |s, f| eprintln!("[{:>3.0}%] {s}", f * 100.0))?;
            write_output(&o, &out, !no_png)?;
            eprintln!("{} v {}: {} boards in {:.1}s -> {}", o.m.teams[0].name, o.m.teams[1].name, o.boards.len(), t0.elapsed().as_secs_f32(), out.display());
            for c in &o.manifest.checks {
                eprintln!("  [{}] {}: {}", if c.ok { "ok" } else { "FAIL" }, c.name, c.detail);
            }
            for f in &o.manifest.fallbacks {
                eprintln!("  fallback: {f}");
            }
            for d in &o.theme.decisions {
                eprintln!("  theme: {d}");
            }
        }
        Cmd::Season { sb_root, competition, season, store } => {
            let models = embedded_models();
            let metas = sources::statsbomb::ensure_season(&sb_root, competition, season)?;
            for meta in metas {
                let id = meta["match_id"].as_u64().unwrap();
                sources::statsbomb::ensure_match_files(&sb_root, id)?;
                let mut m = sources::statsbomb::load(&sb_root, id)?;
                let v = models.apply(&mut m);
                let a = derive::metrics::analyse(&m, v, sim::simulate(&m, 2000), sim::live_curve(&m, 5.0));
                store::upsert(&store, &store::rows_for(&m, &a))?;
                eprintln!("stored {} v {}", m.teams[0].name, m.teams[1].name);
            }
        }
    }
    Ok(())
}
