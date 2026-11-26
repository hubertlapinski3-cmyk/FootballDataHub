//! Multi-league catalog, scouting index, match packets and season percentiles.
//! Everything runs offline against the bundled fixture in tests/fixtures/scout (synthetic players and
//! clubs, plus one real match packet built from the bundled sample match).

use matchhub::catalog::*;
use matchhub::model::Role;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scout").join(name)
}

fn sample_root(tag: &str) -> PathBuf {
    use std::io::Read;
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hub-app/assets/sample");
    let root = std::env::temp_dir().join(format!("datahub_scout_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["matches/55", "events", "lineups"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::copy(src.join("matches/55/282.json"), root.join("matches/55/282.json")).unwrap();
    std::fs::copy(src.join("lineups/3943043.json"), root.join("lineups/3943043.json")).unwrap();
    let mut events = vec![];
    flate2::read::GzDecoder::new(std::fs::File::open(src.join("events/3943043.json.gz")).unwrap()).read_to_end(&mut events).unwrap();
    std::fs::write(root.join("events/3943043.json"), events).unwrap();
    root
}

// ---------------------------------------------------------------- fixture generator

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
    }
}

/// A plausible match vector for a role: per-90 style figures for counts, rates for ratios.
fn synthetic_vector(role: ScoutRole, quality: f64, rng: &mut Rng) -> Vec<f64> {
    let atk = match role { ScoutRole::FW => 1.0, ScoutRole::W => 0.8, ScoutRole::AM => 0.75, ScoutRole::CM => 0.4, ScoutRole::FB => 0.3, ScoutRole::CB => 0.1, ScoutRole::GK => 0.0 };
    let dfn = match role { ScoutRole::CB => 1.0, ScoutRole::FB => 0.8, ScoutRole::CM => 0.8, ScoutRole::AM => 0.4, ScoutRole::W => 0.3, ScoutRole::FW => 0.2, ScoutRole::GK => 0.1 };
    let gk = if role == ScoutRole::GK { 1.0 } else { 0.0 };
    METRICS
        .iter()
        .map(|(l, _)| {
            let noise = 0.55 + 0.9 * rng.next();
            let q = quality * noise;
            match *l {
                "Goals" => 0.35 * atk * q,
                "Shots" => 2.8 * atk * q,
                "xG" => 0.4 * atk * q,
                "Inferred xA" => 0.22 * atk.max(0.3) * q,
                "xGChain" => 0.5 * (atk + 0.3) * q,
                "Box entries" => 2.4 * atk * q,
                "Final-third receptions" => 9.0 * (atk + 0.2) * q,
                "Touches" => 55.0 * (0.6 + 0.4 * atk + 0.3 * dfn) * q,
                "Passes" => 38.0 * (0.5 + 0.5 * dfn + 0.2 * atk) * q,
                "Completion" => (0.62 + 0.3 * rng.next()).min(0.96),
                "Progressive pass share" => 0.06 + 0.2 * rng.next(),
                "Progressive actions" => 4.0 * (0.3 + 0.7 * (atk + dfn) / 2.0) * q,
                "Progression metres" => 150.0 * (0.3 + 0.7 * (atk + dfn) / 2.0) * q,
                "Line-breaking passes" => 2.0 * (0.3 + dfn) * q,
                "xA per 100 passes" => 0.5 * (atk + 0.2) * q,
                "Positive xT" => 0.08 * (atk + 0.3) * q,
                "Take-ons won" => 1.6 * atk * q,
                "Recoveries" => 5.5 * dfn * q,
                "Aerials won" => 2.4 * (dfn + 0.3 * atk) * q,
                "PAdj defensive actions" => 8.0 * dfn * q,
                "Defensive height" => 30.0 + 25.0 * atk + 8.0 * rng.next(),
                "Saves" => 3.2 * gk * q,
                "Claims" => 1.2 * gk * q,
                "Sweeps" => 0.8 * gk * q,
                "VAEP" => 0.18 * (atk + dfn) / 2.0 * q + 0.02,
                "Defensive actions" => 9.0 * (dfn + 0.1) * q,
                _ => 0.0,
            }
        })
        .collect()
}

fn role_x(r: ScoutRole) -> f64 {
    match r { ScoutRole::GK => 6.0, ScoutRole::CB => 28.0, ScoutRole::FB => 36.0, ScoutRole::CM => 50.0, ScoutRole::AM => 62.0, ScoutRole::W => 68.0, ScoutRole::FW => 74.0 }
}

const FIRST: [&str; 12] = ["Adrian", "Bruno", "Cedric", "Dario", "Emil", "Felix", "Goran", "Hugo", "Ivo", "Jonas", "Kamil", "Luca"];
const LAST: [&str; 14] = ["Varga", "Norrby", "Delmar", "Kovar", "Bellini", "Haas", "Okafor", "Lindqvist", "Marchand", "Zielinski", "Dubois", "Ferrante", "Wolff", "Janssen"];
const CLUB_A: [&str; 8] = ["Northgate", "Harbourside", "Redmoor", "Alder", "Kingsfield", "Westbrook", "Stonebridge", "Lakeside"];
const CLUB_B: [&str; 4] = ["United", "Athletic", "Rovers", "Town"];

/// Writes the synthetic multi-league fixture: catalog.json, scout_index.json and per-match rows.
/// Run with `cargo test -p hub-core --test scout regenerate_fixture -- --ignored`.
#[test]
#[ignore]
fn regenerate_fixture() {
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    let layout = [(ScoutRole::GK, 1), (ScoutRole::CB, 3), (ScoutRole::FB, 2), (ScoutRole::CM, 3), (ScoutRole::AM, 1), (ScoutRole::W, 2), (ScoutRole::FW, 2)];
    let mut entries = vec![];
    let mut rows = vec![];
    let mut next_id = 1_000_000u64;
    let mut next_player = 90_000u64;
    for (li, l) in League::ALL.iter().enumerate() {
        // league strength nudges the figures a little so the leagues are not identical
        let strength = [1.1, 1.05, 1.05, 1.0, 0.98, 0.9, 0.85][li];
        let teams: Vec<(String, Vec<(u64, String, String, ScoutRole, f64)>)> = (0..6)
            .map(|t| {
                let name = format!("{} {}", CLUB_A[(t + li) % 8], CLUB_B[(t + li / 2) % 4]);
                let squad = layout
                    .iter()
                    .flat_map(|(r, n)| (0..*n).map(move |k| (*r, k)))
                    .map(|(r, _)| {
                        next_player += 1;
                        let full = format!("{} {}", FIRST[(next_player as usize * 7) % 12], LAST[(next_player as usize * 5 + li) % 14]);
                        let short = format!("{}. {}", full.chars().next().unwrap(), full.split(' ').last().unwrap());
                        (next_player, full, short, r, strength * (0.7 + 0.6 * rng.next()))
                    })
                    .collect();
                (name, squad)
            })
            .collect();
        for round in 0..10usize {
            let (h, a) = (round % 6, (round * 5 + 1) % 6);
            if h == a {
                continue;
            }
            next_id += 1;
            let date = format!("2026-{:02}-{:02}", 8 + round / 3, 8 + (round % 3) * 7 + li);
            let id = next_id.to_string();
            let score = [(rng.next() * 4.0) as u32, (rng.next() * 3.0) as u32];
            for (ti, (tname, squad)) in [(&teams[h].0, &teams[h].1), (&teams[a].0, &teams[a].1)].into_iter().enumerate() {
                let _ = ti;
                for (pid, name, short, role, q) in squad {
                    // not everyone plays every match
                    if rng.next() < 0.2 {
                        continue;
                    }
                    rows.push(PlayerMatchRow { match_id: id.clone(), league: l.slug().into(), date: date.clone(), player_id: *pid, name: name.clone(), short: short.clone(), team: tname.clone(), position: role.label().into(), role: Some(*role), minutes: if rng.next() < 0.8 { 90.0 } else { 45.0 + 30.0 * rng.next() }, m: synthetic_vector(*role, *q, &mut rng), avg_x: role_x(*role) + 6.0 * rng.next(), avg_y: 14.0 + 40.0 * rng.next() });
                }
            }
            let short = |n: &str| n.split(' ').next().unwrap().chars().take(3).collect::<String>().to_uppercase();
            entries.push((*l, CatalogMatch { id: id.clone(), date, home: teams[h].0.clone(), away: teams[a].0.clone(), home_short: short(&teams[h].0), away_short: short(&teams[a].0), score, url: format!("matches/{}/{id}.json.gz", l.slug()), bytes: 52_000 }));
        }
    }
    let now = "2026-11-26T00:00:00+00:00";
    let cat = build_catalog("2026-27", &entries, now);
    let idx = build_index(&rows, "2026-27", now, 270.0);
    std::fs::write(fixture("catalog.json"), serde_json::to_vec(&cat).unwrap()).unwrap();
    std::fs::write(fixture("scout_index.json"), serde_json::to_vec(&idx).unwrap()).unwrap();
    // one real packet, built from the bundled sample match
    let m = matchhub::sources::statsbomb::load(&sample_root("gen"), 3943043).unwrap();
    std::fs::write(fixture("sample_packet.json.gz"), encode_packet(&m).unwrap()).unwrap();
    eprintln!("{} matches, {} players", cat.total_matches(), idx.players.len());
}

// ---------------------------------------------------------------- tests

#[test]
fn leagues_and_roles_map_from_source_names() {
    assert_eq!(League::from_competition("Premier League"), Some(League::PremierLeague));
    assert_eq!(League::from_competition("LaLiga"), Some(League::LaLiga));
    assert_eq!(League::from_competition("1. Bundesliga"), Some(League::Bundesliga));
    assert_eq!(League::from_competition("Serie A"), Some(League::SerieA));
    assert_eq!(League::from_competition("Ligue 1"), Some(League::Ligue1));
    assert_eq!(League::from_competition("Jupiler Pro League"), Some(League::ProLeague));
    assert_eq!(League::from_competition("PKO BP Ekstraklasa"), Some(League::Ekstraklasa));
    assert_eq!(League::from_competition("Russian Premier League"), None);
    assert_eq!(League::from_slug("pro-league").unwrap().pill(), "Pro League");
    assert!(League::LaLiga.top5() && !League::Ekstraklasa.top5() && !League::ProLeague.top5());
    for (pos, role, want) in [("GK", Role::Goalkeeper, ScoutRole::GK), ("DC", Role::Defender, ScoutRole::CB), ("DR", Role::Defender, ScoutRole::FB), ("DMC", Role::Midfielder, ScoutRole::CM), ("AMC", Role::Midfielder, ScoutRole::AM), ("AML", Role::Forward, ScoutRole::W), ("FW", Role::Forward, ScoutRole::FW), ("Sub", Role::Midfielder, ScoutRole::CM)] {
        assert_eq!(ScoutRole::from_position(pos, role), want, "{pos}");
    }
    assert_eq!(ScoutRole::W.broad(), Role::Forward);
}

#[test]
fn fixture_catalog_and_index_parse_and_filter() {
    let store = CatalogStore::new(&fixture(""));
    let cat = store.load_catalog().expect("catalog.json parses");
    let idx = store.load_scout().expect("scout_index.json parses");
    assert_eq!(cat.version, CATALOG_VERSION);
    assert_eq!(cat.leagues.len(), 7);
    assert!(cat.leagues.iter().all(|l| !l.matches.is_empty() && !l.teams.is_empty()));
    let all = cat.matches(LeagueFilter::All, "");
    assert_eq!(all.len(), cat.total_matches());
    // newest first
    assert!(all.windows(2).all(|w| w[0].1.date >= w[1].1.date));
    let top5 = cat.matches(LeagueFilter::Top5, "").len();
    let pl = cat.matches(LeagueFilter::One(League::PremierLeague), "").len();
    assert!(top5 > pl && top5 < all.len());
    let team = cat.leagues[0].teams[0].clone();
    let by_team = cat.matches(LeagueFilter::All, &team.to_uppercase());
    assert!(!by_team.is_empty() && by_team.iter().all(|(_, m)| m.home == team || m.away == team));
    // index: aligned vectors, every league and role present, percentiles in range
    assert_eq!(idx.metrics.len(), METRICS.len());
    assert!(idx.players.iter().all(|p| p.m.len() == METRICS.len() && p.pct.len() == METRICS.len() && p.pct.iter().all(|x| *x <= 100)));
    for l in League::ALL {
        assert!(idx.players.iter().any(|p| p.league == l.slug()), "{}", l.slug());
    }
    for r in ScoutRole::ALL {
        assert!(idx.pool(Some(r), LeagueFilter::All, 270.0).len() >= 5, "{r:?}");
    }
    assert!(idx.pool(None, LeagueFilter::Top5, 900.0).len() < idx.pool(None, LeagueFilter::All, 270.0).len());
}

#[test]
fn index_percentiles_are_role_pools_over_all_leagues() {
    let mk = |id: u64, league: &str, role: ScoutRole, minutes: f64, xg: f64| {
        let mut m = vec![0.0; METRICS.len()];
        m[metric_index("xG").unwrap()] = xg;
        PlayerMatchRow { match_id: format!("m{id}"), league: league.into(), date: "2026-09-01".into(), player_id: id, name: format!("P{id}"), short: format!("P{id}"), team: "T".into(), position: role.label().into(), role: Some(role), minutes, m, avg_x: 0.0, avg_y: 0.0 }
    };
    let rows = vec![
        mk(1, "premier-league", ScoutRole::FW, 90.0, 0.2),
        mk(2, "ekstraklasa", ScoutRole::FW, 90.0, 0.4),
        mk(3, "pro-league", ScoutRole::FW, 90.0, 0.6),
        mk(4, "la-liga", ScoutRole::FW, 90.0, 0.8),
        // below the pool floor: gets a value but does not shape the percentiles
        mk(5, "ligue-1", ScoutRole::FW, 45.0, 5.0),
        // another role is its own pool
        mk(6, "serie-a", ScoutRole::CB, 90.0, 0.01),
    ];
    let idx = build_index(&rows, "2026-27", "now", 90.0);
    let g = |id: u64, k: &str| idx.find(id).unwrap().percentile_of(k) as i32;
    assert_eq!(g(1, "xG"), 13); // (0 below + 0.5 tie) / 4 = 12.5, rounded
    assert_eq!(g(2, "xG"), 38);
    assert_eq!(g(4, "xG"), 88);
    assert_eq!(g(6, "xG"), 50); // alone in its role
    // the Ekstraklasa forward ranks against Premier League and La Liga forwards, not his own league
    assert!(g(2, "xG") > g(1, "xG") && g(2, "xG") < g(4, "xG"));
    // per 90: 45 minutes of 5.0 xG is 10 per 90; a repeat of the same match is not counted twice
    assert!((idx.find(5).unwrap().xg90() - 10.0).abs() < 1e-3);
    let mut twice = rows.clone();
    twice.push(rows[0].clone());
    let again = build_index(&twice, "2026-27", "now", 90.0);
    assert_eq!(again.find(1).unwrap().matches, 1);
}

#[test]
fn season_values_sum_per_90_and_weight_rates_by_minutes() {
    let ix = metric_index("Shots").unwrap();
    let ic = metric_index("Completion").unwrap();
    let mk = |mid: &str, minutes: f64, shots: f64, comp: f64| {
        let mut m = vec![0.0; METRICS.len()];
        m[ix] = shots;
        m[ic] = comp;
        PlayerMatchRow { match_id: mid.into(), league: "la-liga".into(), date: format!("2026-09-{}", mid), player_id: 7, name: "P".into(), short: "P".into(), team: "T".into(), position: "FW".into(), role: Some(ScoutRole::FW), minutes, m, avg_x: 0.0, avg_y: 0.0 }
    };
    let idx = build_index(&[mk("01", 90.0, 3.0, 0.8), mk("02", 30.0, 3.0, 0.6)], "2026-27", "now", 90.0);
    let p = idx.find(7).unwrap();
    assert_eq!(p.matches, 2);
    assert!((p.minutes - 120.0).abs() < 1e-3);
    assert!((p.value("Shots") - 4.5).abs() < 1e-3, "6 shots in 120' = 4.5 per 90, got {}", p.value("Shots"));
    assert!((p.value("Completion") - 0.75).abs() < 1e-3, "minutes-weighted rate, got {}", p.value("Completion"));
}

#[test]
fn packet_roundtrip_and_rows_from_the_sample_match() {
    let m = matchhub::sources::statsbomb::load(&sample_root("rt"), 3943043).unwrap();
    let bytes = encode_packet(&m).unwrap();
    let back = decode_packet(&bytes).unwrap();
    assert_eq!(back.id, m.id);
    assert_eq!(back.events.len(), m.events.len());
    assert_eq!(back.teams[0].name, m.teams[0].name);
    // the bundled fixture packet reads the same way
    let fixture_packet = read_packet(&fixture("sample_packet.json.gz")).expect("fixture packet");
    assert_eq!(fixture_packet.events.len(), m.events.len());
    // a plain (not gzipped) packet reads too
    assert_eq!(decode_packet(&serde_json::to_vec(&m).unwrap()).unwrap().id, m.id);

    // ingest: packet + rows + catalog entry in the data layout, then the index from the rows
    let out = std::env::temp_dir().join(format!("datahub_ingest_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let rf = matchhub::ingest::ingest_match(m, League::PremierLeague, &out, &out.join("cache"), false).unwrap();
    assert!(rf.entry.url.starts_with("matches/premier-league/") && out.join(&rf.entry.url).exists());
    assert!(rf.entry.bytes > 10_000 && rf.entry.bytes < 600_000, "packet is {} bytes", rf.entry.bytes);
    assert!(rf.rows.len() >= 22, "{} rows", rf.rows.len());
    assert!(rf.rows.iter().all(|r| r.m.len() == METRICS.len() && r.role.is_some() && r.minutes > 0.0));
    let (cat, idx) = matchhub::ingest::finalize(&out, "2026-27", 90.0).unwrap();
    assert_eq!(cat.total_matches(), 1);
    assert_eq!(cat.league("premier-league").unwrap().matches[0].id, rf.entry.id);
    assert!(idx.players.len() >= 22 && out.join("catalog.json").exists() && out.join("scout_index.json").exists());
    // the packet builds boards with no network and no scraping
    let models = matchhub::embedded_models();
    let opts = matchhub::BuildOptions { cache_dir: out.join("cache"), online: false, ..Default::default() };
    let built = matchhub::build(&matchhub::Input::Packet(out.join(&rf.entry.url)), &opts, &models, &mut |_, _| {}).unwrap();
    assert!(built.boards.len() >= 60);
}

#[test]
fn player_cards_rank_against_the_season_pool_when_the_index_is_present() {
    let idx: ScoutIndex = serde_json::from_slice(&std::fs::read(fixture("scout_index.json")).unwrap()).unwrap();
    let models = matchhub::embedded_models();
    let input = matchhub::Input::StatsBomb(sample_root("card"), 3943043);
    let base = matchhub::BuildOptions { cache_dir: std::env::temp_dir().join(format!("datahub_scout_cache_{}", std::process::id())), online: false, ..Default::default() };
    let plain = matchhub::build(&input, &base, &models, &mut |_, _| {}).unwrap();
    let with = matchhub::build(&input, &matchhub::BuildOptions { scout: Some(std::sync::Arc::new(idx)), ..base.clone() }, &models, &mut |_, _| {}).unwrap();
    let cards = |o: &matchhub::BuildOutput| o.boards.iter().filter(|b| b.id.starts_with("31_player_")).map(|b| b.scene.clone()).collect::<Vec<_>>();
    let (a, b) = (cards(&plain), cards(&with));
    assert!(!a.is_empty() && a.len() == b.len());
    assert!(a.iter().all(|s| !s.contains("seven leagues")), "no index: ranked within the match");
    assert!(b.iter().all(|s| s.contains("across the seven leagues")), "index: ranked against the season pool");
    assert!(b.iter().all(|s| !s.contains("fewer than 4 peers")));
}

#[test]
fn remote_urls_and_errors_are_plain() {
    let r = Remote { repo: "owner/repo".into(), branch: "data-catalog".into(), token: Some("  ".into()) };
    assert_eq!(r.base(), "https://raw.githubusercontent.com/owner/repo/data-catalog/");
    let _ = Path::new(".");
}

#[test]
fn season_card_is_role_specific_and_purely_offensive_for_forwards() {
    let idx: ScoutIndex = serde_json::from_slice(&std::fs::read(fixture("scout_index.json")).unwrap()).unwrap();
    let text = |role: ScoutRole| {
        let row = idx.players.iter().filter(|p| p.role == Some(role) && p.minutes >= 270.0).max_by(|a, b| a.minutes.partial_cmp(&b.minutes).unwrap()).unwrap();
        let sc = matchhub::render::scout::scout_scene(row, &idx, "#1C7C54");
        assert_eq!(sc.w, 640.0);
        assert!(sc.h > 900.0 && sc.h < 3000.0, "height {}", sc.h);
        sc.prims
            .iter()
            .filter_map(|p| if let matchhub::render::svg::Prim::T { runs, .. } = p { Some(runs.iter().map(|r| r.0.as_str()).collect::<String>()) } else { None })
            .collect::<Vec<_>>()
            .join("|")
    };
    let fw = text(ScoutRole::FW);
    for l in ring_labels(ScoutRole::FW) {
        assert!(fw.contains(l), "FW card lacks {l}");
    }
    for defensive in ["Recoveries", "Aerials won", "PAdj defensive actions", "Defensive height"] {
        assert!(!fw.contains(defensive), "FW card must not measure {defensive}");
    }
    assert!(fw.contains("Season percentile ring") && fw.contains("Average position") && fw.contains("seven leagues"));
    let cb = text(ScoutRole::CB);
    assert!(cb.contains("Aerials won") && !cb.contains("Take-ons won"));
    let gk = text(ScoutRole::GK);
    assert!(gk.contains("Saves") && gk.contains("Claims"));
}

#[test]
fn league_config_matches_the_league_slugs_the_ingest_reads() {
    let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/leagues.json")).unwrap()).expect("leagues.json parses");
    let slugs: Vec<&str> = cfg["leagues"].as_array().unwrap().iter().map(|l| l["slug"].as_str().unwrap()).collect();
    let want: Vec<&str> = League::ALL.iter().map(|l| l.slug()).collect();
    assert_eq!(slugs, want, "raw/<slug>/ folders must be the slugs League::from_slug knows");
    assert!(cfg["leagues"].as_array().unwrap().iter().all(|l| l.get("verify").is_none() && l["url"].as_str().unwrap().contains("whoscored.com")));
    let pl = &cfg["leagues"][5];
    assert_eq!((pl["region_id"].as_u64(), pl["tournament_id"].as_u64(), pl["season_id"].as_u64(), pl["stage_id"].as_u64()), (Some(22), Some(18), Some(11160), Some(25564)));
    let ek = &cfg["leagues"][6];
    assert_eq!((ek["region_id"].as_u64(), ek["tournament_id"].as_u64(), ek["season_id"].as_u64(), ek["stage_id"].as_u64()), (Some(176), Some(76), Some(11155), Some(25559)));
}
