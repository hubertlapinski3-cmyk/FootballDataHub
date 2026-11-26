//! Multi-league catalog and scouting index.
//!
//! A nightly job (`matchhub ingest`) turns every completed match of the seven covered leagues into a
//! compact match packet (the parsed `Match`, gzip JSON) and a small file of per-player match rows. From
//! those it publishes two light files the app downloads over plain HTTPS:
//!
//! * `catalog.json`: leagues, teams, scores, dates and the relative URL of every packet;
//! * `scout_index.json`: season-to-date per-90 figures and role percentiles for every player.
//!
//! The app never scrapes and never bundles raw matches: it reads the catalog, downloads one packet when
//! a match is opened and builds the boards from it locally.

use crate::derive::metrics::{Analysis, PlayerStats};
use crate::model::{Match, Role, Side};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const CATALOG_VERSION: u32 = 1;

// ---------------------------------------------------------------- leagues

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum League {
    PremierLeague,
    LaLiga,
    Bundesliga,
    SerieA,
    Ligue1,
    ProLeague,
    Ekstraklasa,
}

impl League {
    pub const ALL: [League; 7] = [League::PremierLeague, League::LaLiga, League::Bundesliga, League::SerieA, League::Ligue1, League::ProLeague, League::Ekstraklasa];
    pub fn slug(self) -> &'static str {
        match self {
            League::PremierLeague => "premier-league",
            League::LaLiga => "la-liga",
            League::Bundesliga => "bundesliga",
            League::SerieA => "serie-a",
            League::Ligue1 => "ligue-1",
            League::ProLeague => "pro-league",
            League::Ekstraklasa => "ekstraklasa",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            League::PremierLeague => "Premier League",
            League::LaLiga => "La Liga",
            League::Bundesliga => "Bundesliga",
            League::SerieA => "Serie A",
            League::Ligue1 => "Ligue 1",
            League::ProLeague => "Belgian Pro League",
            League::Ekstraklasa => "Ekstraklasa",
        }
    }
    /// Short label for filter pills.
    pub fn pill(self) -> &'static str {
        match self {
            League::ProLeague => "Pro League",
            l => l.name(),
        }
    }
    pub fn top5(self) -> bool {
        !matches!(self, League::ProLeague | League::Ekstraklasa)
    }
    pub fn from_slug(s: &str) -> Option<League> {
        League::ALL.iter().copied().find(|l| l.slug() == s)
    }
    /// Reads a competition name as the sources write it ("Premier League", "LaLiga", "Jupiler Pro League", ...).
    pub fn from_competition(name: &str) -> Option<League> {
        let n: String = name.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        if n.contains("premierleague") && !n.contains("belgian") && !n.contains("jupiler") && !n.contains("russia") && !n.contains("ukrain") {
            Some(League::PremierLeague)
        } else if n.contains("laliga") && !n.contains("laliga2") {
            Some(League::LaLiga)
        } else if n.contains("bundesliga") && !n.contains("austria") && !n.contains("2bundesliga") {
            Some(League::Bundesliga)
        } else if n.contains("seriea") {
            Some(League::SerieA)
        } else if n.contains("ligue1") {
            Some(League::Ligue1)
        } else if n.contains("jupiler") || n.contains("belgianprolague") || n.contains("belgianprofessional") || n.contains("proleague") || n.contains("belgium") {
            Some(League::ProLeague)
        } else if n.contains("ekstraklasa") {
            Some(League::Ekstraklasa)
        } else {
            None
        }
    }
}

/// Filter used by the app: one league, the top five, or everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeagueFilter {
    All,
    Top5,
    One(League),
}

impl LeagueFilter {
    pub fn matches(self, slug: &str) -> bool {
        match (self, League::from_slug(slug)) {
            (LeagueFilter::All, _) => true,
            (LeagueFilter::Top5, Some(l)) => l.top5(),
            (LeagueFilter::One(o), Some(l)) => o == l,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------- scouting roles

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScoutRole {
    GK,
    CB,
    FB,
    CM,
    AM,
    W,
    FW,
}

impl ScoutRole {
    pub const ALL: [ScoutRole; 7] = [ScoutRole::FW, ScoutRole::W, ScoutRole::AM, ScoutRole::CM, ScoutRole::FB, ScoutRole::CB, ScoutRole::GK];
    pub fn label(self) -> &'static str {
        match self {
            ScoutRole::GK => "GK",
            ScoutRole::CB => "CB",
            ScoutRole::FB => "FB",
            ScoutRole::CM => "CM",
            ScoutRole::AM => "AM",
            ScoutRole::W => "W",
            ScoutRole::FW => "FW",
        }
    }
    pub fn broad(self) -> Role {
        match self {
            ScoutRole::GK => Role::Goalkeeper,
            ScoutRole::CB | ScoutRole::FB => Role::Defender,
            ScoutRole::CM | ScoutRole::AM => Role::Midfielder,
            ScoutRole::W | ScoutRole::FW => Role::Forward,
        }
    }
    /// From a WhoScored / StatsBomb position code or name, falling back to the broad role.
    pub fn from_position(pos: &str, role: Role) -> ScoutRole {
        let p = pos.trim().to_uppercase();
        match p.as_str() {
            "GK" => return ScoutRole::GK,
            "DC" | "CB" | "RCB" | "LCB" => return ScoutRole::CB,
            "DL" | "DR" | "DML" | "DMR" | "LB" | "RB" | "LWB" | "RWB" | "WB" => return ScoutRole::FB,
            "DMC" | "MC" | "CM" | "DM" | "CDM" => return ScoutRole::CM,
            "AMC" | "CAM" | "SS" => return ScoutRole::AM,
            "AML" | "AMR" | "ML" | "MR" | "LW" | "RW" | "LM" | "RM" => return ScoutRole::W,
            "FW" | "FWL" | "FWR" | "ST" | "CF" => return ScoutRole::FW,
            _ => {}
        }
        if p.contains("GOALKEEPER") {
            ScoutRole::GK
        } else if p.contains("CENTRE BACK") || p.contains("CENTER BACK") {
            ScoutRole::CB
        } else if p.contains("BACK") {
            ScoutRole::FB
        } else if p.contains("ATTACKING MIDFIELD") {
            ScoutRole::AM
        } else if p.contains("WING") && !p.contains("BACK") {
            ScoutRole::W
        } else if p.contains("MIDFIELD") {
            ScoutRole::CM
        } else if p.contains("FORWARD") || p.contains("STRIKER") {
            ScoutRole::FW
        } else {
            match role {
                Role::Goalkeeper => ScoutRole::GK,
                Role::Defender => ScoutRole::CB,
                Role::Midfielder => ScoutRole::CM,
                Role::Forward => ScoutRole::FW,
            }
        }
    }
}

// ---------------------------------------------------------------- metrics

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    /// a count: season value is per 90 minutes
    Per90,
    /// a rate or level: season value is the minutes-weighted mean
    Mean,
}

/// Every figure the scouting index keeps, in a fixed order; rows store values aligned with this list.
/// The labels match the percentile-ring wedges of the player card.
pub const METRICS: &[(&str, MetricKind)] = &[
    ("Goals", MetricKind::Per90),
    ("Shots", MetricKind::Per90),
    ("xG", MetricKind::Per90),
    ("Inferred xA", MetricKind::Per90),
    ("xGChain", MetricKind::Per90),
    ("Box entries", MetricKind::Per90),
    ("Final-third receptions", MetricKind::Per90),
    ("Touches", MetricKind::Per90),
    ("Passes", MetricKind::Per90),
    ("Completion", MetricKind::Mean),
    ("Progressive pass share", MetricKind::Mean),
    ("Progressive actions", MetricKind::Per90),
    ("Progression metres", MetricKind::Per90),
    ("Line-breaking passes", MetricKind::Per90),
    ("xA per 100 passes", MetricKind::Mean),
    ("Positive xT", MetricKind::Per90),
    ("Take-ons won", MetricKind::Per90),
    ("Recoveries", MetricKind::Per90),
    ("Aerials won", MetricKind::Per90),
    ("PAdj defensive actions", MetricKind::Mean),
    ("Defensive height", MetricKind::Mean),
    ("Saves", MetricKind::Per90),
    ("Claims", MetricKind::Per90),
    ("Sweeps", MetricKind::Per90),
    ("VAEP", MetricKind::Per90),
    ("Defensive actions", MetricKind::Per90),
];

pub fn metric_index(label: &str) -> Option<usize> {
    METRICS.iter().position(|(l, _)| *l == label)
}

/// One player's figures for one match, aligned with `METRICS` (match totals and match rates).
pub fn metric_vector(p: &PlayerStats) -> Vec<f64> {
    let completion = if p.passes > 0 { p.passes_completed as f64 / p.passes as f64 } else { 0.0 };
    METRICS
        .iter()
        .map(|(l, _)| match *l {
            "Goals" => p.goals as f64,
            "Shots" => p.shots as f64,
            "xG" => p.xg,
            "Inferred xA" => p.inferred_xa,
            "xGChain" => p.xgchain,
            "Box entries" => p.box_entries as f64,
            "Final-third receptions" => p.ft_receptions as f64,
            "Touches" => p.touches as f64,
            "Passes" => p.passes as f64,
            "Completion" => completion,
            "Progressive pass share" => p.prog_pass_share,
            "Progressive actions" => (p.progressive_passes + p.progressive_carries) as f64,
            "Progression metres" => p.progression_m,
            "Line-breaking passes" => p.line_breaks as f64,
            "xA per 100 passes" => p.xa_per100,
            "Positive xT" => p.positive_xt,
            "Take-ons won" => p.take_ons.0 as f64,
            "Recoveries" => p.recoveries as f64,
            "Aerials won" => p.aerials.0 as f64,
            "PAdj defensive actions" => p.padj_def,
            "Defensive height" => p.def_height,
            "Saves" => p.saves as f64,
            "Claims" => p.claims as f64,
            "Sweeps" => p.sweeps as f64,
            "VAEP" => p.vaep_off + p.vaep_def,
            "Defensive actions" => p.def_actions as f64,
            _ => 0.0,
        })
        .collect()
}

/// Turns a single-match value into the unit the season index uses (per 90 for counts).
pub fn season_unit(idx: usize, value: f64, minutes: f64) -> f64 {
    match METRICS[idx].1 {
        MetricKind::Per90 if minutes > 0.0 => value * 90.0 / minutes,
        _ => value,
    }
}

// ---------------------------------------------------------------- published files

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogMatch {
    pub id: String,
    pub date: String,
    pub home: String,
    pub away: String,
    pub home_short: String,
    pub away_short: String,
    pub score: [u32; 2],
    /// packet path relative to the catalog base, e.g. `matches/premier-league/1234567.json.gz`
    pub url: String,
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LeagueCatalog {
    pub slug: String,
    pub name: String,
    pub teams: Vec<String>,
    pub matches: Vec<CatalogMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Catalog {
    pub version: u32,
    pub season: String,
    pub generated_at: String,
    pub leagues: Vec<LeagueCatalog>,
}

impl Catalog {
    pub fn league(&self, slug: &str) -> Option<&LeagueCatalog> {
        self.leagues.iter().find(|l| l.slug == slug)
    }
    pub fn total_matches(&self) -> usize {
        self.leagues.iter().map(|l| l.matches.len()).sum()
    }
    /// Matches of the filter, newest first, optionally narrowed by a team search (case-insensitive substring).
    pub fn matches<'a>(&'a self, f: LeagueFilter, query: &str) -> Vec<(&'a LeagueCatalog, &'a CatalogMatch)> {
        let q = query.trim().to_lowercase();
        let mut v: Vec<(&LeagueCatalog, &CatalogMatch)> = self
            .leagues
            .iter()
            .filter(|l| f.matches(&l.slug))
            .flat_map(|l| l.matches.iter().map(move |m| (l, m)))
            .filter(|(_, m)| q.is_empty() || m.home.to_lowercase().contains(&q) || m.away.to_lowercase().contains(&q) || m.home_short.to_lowercase() == q || m.away_short.to_lowercase() == q)
            .collect();
        v.sort_by(|a, b| b.1.date.cmp(&a.1.date).then(a.1.id.cmp(&b.1.id)));
        v
    }
}

/// One player's season-to-date line in the scouting index.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoutPlayerRow {
    pub id: u64,
    pub name: String,
    pub short: String,
    pub team: String,
    pub league: String,
    pub role: Option<ScoutRole>,
    pub matches: u32,
    pub minutes: f32,
    /// aligned with `METRICS`: per 90 for counts, minutes-weighted mean for rates
    pub m: Vec<f32>,
    /// minutes-weighted average position in the team's own attacking frame (metres)
    #[serde(default)]
    pub avg_x: f32,
    #[serde(default)]
    pub avg_y: f32,
    /// role percentile (0..100) of every metric among players of the same role with at least
    /// `ScoutIndex::pool_minutes` minutes, all leagues together; aligned with `METRICS`
    #[serde(default)]
    pub pct: Vec<u8>,
}

impl ScoutPlayerRow {
    pub fn value(&self, label: &str) -> f64 {
        metric_index(label).and_then(|i| self.m.get(i)).copied().unwrap_or(0.0) as f64
    }
    pub fn percentile_of(&self, label: &str) -> f64 {
        metric_index(label).and_then(|i| self.pct.get(i)).copied().unwrap_or(50) as f64
    }
    pub fn xg90(&self) -> f64 {
        self.value("xG")
    }
    pub fn xa90(&self) -> f64 {
        self.value("Inferred xA")
    }
    pub fn prog90(&self) -> f64 {
        self.value("Progressive actions")
    }
    pub fn vaep90(&self) -> f64 {
        self.value("VAEP")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoutIndex {
    pub version: u32,
    pub season: String,
    pub generated_at: String,
    /// minimum minutes a player needs to enter the percentile pools stored in `pct`
    pub pool_minutes: f32,
    pub metrics: Vec<String>,
    pub players: Vec<ScoutPlayerRow>,
}

impl ScoutIndex {
    pub fn find(&self, id: u64) -> Option<&ScoutPlayerRow> {
        self.players.iter().find(|p| p.id == id)
    }
    /// Players of a role over a league filter and minutes floor.
    pub fn pool(&self, role: Option<ScoutRole>, f: LeagueFilter, min_minutes: f32) -> Vec<&ScoutPlayerRow> {
        self.players.iter().filter(|p| role.map(|r| p.role == Some(r)).unwrap_or(true) && f.matches(&p.league) && p.minutes >= min_minutes).collect()
    }
    /// Percentile of `value` for a metric among a pool (the same rule the match player card uses).
    pub fn percentile(&self, label: &str, value: f64, pool: &[&ScoutPlayerRow]) -> f64 {
        let vals: Vec<f64> = pool.iter().map(|p| p.value(label)).collect();
        percentile(value, &vals)
    }
    /// Values of one metric over the role pool: what the match player card ranks a player against.
    pub fn role_values(&self, role: ScoutRole, label: &str, min_minutes: f32, fallback_to_broad: usize) -> Vec<f64> {
        let exact = self.pool(Some(role), LeagueFilter::All, min_minutes);
        if exact.len() >= fallback_to_broad {
            return exact.iter().map(|p| p.value(label)).collect();
        }
        let broad = role.broad();
        self.players.iter().filter(|p| p.minutes >= min_minutes && p.role.map(|r| r.broad()) == Some(broad)).map(|p| p.value(label)).collect()
    }
}

/// Three decimals keep the published index small without changing any ranking that matters.
fn round3(v: f64) -> f32 {
    ((v * 1000.0).round() / 1000.0) as f32
}

/// Share of the pool strictly below the value plus half the ties, 0..100.
pub fn percentile(v: f64, pool: &[f64]) -> f64 {
    if pool.is_empty() {
        return 50.0;
    }
    let below = pool.iter().filter(|x| **x < v).count() as f64;
    let eq = pool.iter().filter(|x| **x == v).count() as f64;
    ((below + 0.5 * eq) / pool.len() as f64 * 100.0).clamp(0.0, 100.0)
}

// ---------------------------------------------------------------- per-match rows and the index builder

/// One player in one match: what the nightly job keeps so the season index can be rebuilt without
/// re-analysing any match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMatchRow {
    pub match_id: String,
    pub league: String,
    pub date: String,
    pub player_id: u64,
    pub name: String,
    pub short: String,
    pub team: String,
    pub position: String,
    pub role: Option<ScoutRole>,
    pub minutes: f64,
    /// aligned with `METRICS`, match totals and rates
    pub m: Vec<f64>,
    /// average position in the team's own attacking frame (metres: x 0 own goal .. 105, y 0..68)
    #[serde(default)]
    pub avg_x: f64,
    #[serde(default)]
    pub avg_y: f64,
}

/// Wedges of the season percentile ring, by role: a forward is measured on attacking output only.
pub fn ring_labels(role: ScoutRole) -> [&'static str; 8] {
    match role {
        ScoutRole::GK => ["Saves", "Claims", "Sweeps", "Progressive pass share", "Passes", "Completion", "Progression metres", "xGChain"],
        ScoutRole::CB | ScoutRole::FB => ["Progressive pass share", "PAdj defensive actions", "Aerials won", "Defensive height", "Progression metres", "Line-breaking passes", "xGChain", "Recoveries"],
        ScoutRole::CM | ScoutRole::AM => ["Progressive pass share", "PAdj defensive actions", "Progression metres", "Line-breaking passes", "xA per 100 passes", "xGChain", "Final-third receptions", "Box entries"],
        ScoutRole::W | ScoutRole::FW => ["xG", "Inferred xA", "xGChain", "Box entries", "Final-third receptions", "Progressive pass share", "Take-ons won", "Shots"],
    }
}

/// A figure as the app shows it: shares as percentages, everything else with sensible decimals.
pub fn format_metric(label: &str, v: f64) -> String {
    match label {
        "Completion" | "Progressive pass share" => format!("{:.0}%", v * 100.0),
        "Defensive height" | "Progression metres" | "Touches" | "Passes" | "Final-third receptions" => format!("{v:.0}"),
        "xG" | "Inferred xA" | "xGChain" | "Positive xT" | "VAEP" | "Goals" | "xA per 100 passes" | "PAdj defensive actions" => format!("{v:.2}"),
        _ => format!("{v:.1}"),
    }
}

pub fn match_rows(m: &Match, a: &Analysis, league: League) -> Vec<PlayerMatchRow> {
    a.players
        .iter()
        .filter(|p| p.minutes > 0.0)
        .filter_map(|p| {
            let side: Side = p.side?;
            let pl = m.team(side).player(p.id)?;
            Some(PlayerMatchRow {
                match_id: m.id.clone(),
                league: league.slug().to_string(),
                date: m.date.clone(),
                player_id: p.id,
                name: pl.name.clone(),
                short: pl.short.clone(),
                team: m.team(side).name.clone(),
                position: pl.position.clone(),
                role: Some(ScoutRole::from_position(&pl.position, pl.role)),
                minutes: p.minutes,
                m: metric_vector(p),
                avg_x: p.avg_x,
                avg_y: p.avg_y,
            })
        })
        .collect()
}

pub fn catalog_entry(m: &Match, url: &str, bytes: u64) -> CatalogMatch {
    CatalogMatch {
        id: m.id.clone(),
        date: m.date.clone(),
        home: m.teams[0].name.clone(),
        away: m.teams[1].name.clone(),
        home_short: m.teams[0].short_name.clone(),
        away_short: m.teams[1].short_name.clone(),
        score: [m.teams[0].score, m.teams[1].score],
        url: url.to_string(),
        bytes,
    }
}

/// Groups catalog entries by league (in the fixed league order), listing each league's teams.
pub fn build_catalog(season: &str, entries: &[(League, CatalogMatch)], generated_at: &str) -> Catalog {
    let mut leagues = vec![];
    for l in League::ALL {
        let mut matches: Vec<CatalogMatch> = entries.iter().filter(|(x, _)| *x == l).map(|(_, m)| m.clone()).collect();
        matches.sort_by(|a, b| b.date.cmp(&a.date).then(a.id.cmp(&b.id)));
        matches.dedup_by(|a, b| a.id == b.id);
        let mut teams: Vec<String> = matches.iter().flat_map(|m| [m.home.clone(), m.away.clone()]).collect();
        teams.sort();
        teams.dedup();
        leagues.push(LeagueCatalog { slug: l.slug().into(), name: l.name().into(), teams, matches });
    }
    Catalog { version: CATALOG_VERSION, season: season.into(), generated_at: generated_at.into(), leagues }
}

/// Season-to-date index from per-match rows: per-90 figures, minutes-weighted rates and role percentiles.
pub fn build_index(rows: &[PlayerMatchRow], season: &str, generated_at: &str, pool_minutes: f32) -> ScoutIndex {
    #[derive(Default)]
    struct Acc {
        row: Option<PlayerMatchRow>,
        matches: u32,
        minutes: f64,
        sums: Vec<f64>,
        last_date: String,
        ax: f64,
        ay: f64,
    }
    let mut acc: BTreeMap<u64, Acc> = BTreeMap::new();
    let mut seen: std::collections::HashSet<(String, u64)> = Default::default();
    for r in rows {
        // a match listed twice (re-ingest) counts once
        if !seen.insert((r.match_id.clone(), r.player_id)) || r.minutes <= 0.0 || r.m.len() != METRICS.len() {
            continue;
        }
        let a = acc.entry(r.player_id).or_default();
        if a.sums.is_empty() {
            a.sums = vec![0.0; METRICS.len()];
        }
        a.matches += 1;
        a.minutes += r.minutes;
        a.ax += r.avg_x * r.minutes;
        a.ay += r.avg_y * r.minutes;
        for (i, (_, kind)) in METRICS.iter().enumerate() {
            a.sums[i] += match kind {
                MetricKind::Per90 => r.m[i],
                MetricKind::Mean => r.m[i] * r.minutes,
            };
        }
        // the latest match names the club and the role
        if r.date >= a.last_date {
            a.last_date = r.date.clone();
            a.row = Some(r.clone());
        }
    }
    let mut players: Vec<ScoutPlayerRow> = acc
        .into_values()
        .filter_map(|a| {
            let r = a.row?;
            let m: Vec<f32> = METRICS
                .iter()
                .enumerate()
                .map(|(i, (_, kind))| match kind {
                    MetricKind::Per90 => round3(a.sums[i] * 90.0 / a.minutes),
                    MetricKind::Mean => round3(a.sums[i] / a.minutes),
                })
                .collect();
            Some(ScoutPlayerRow { id: r.player_id, name: r.name, short: r.short, team: r.team, league: r.league, role: r.role, matches: a.matches, minutes: a.minutes as f32, avg_x: (a.ax / a.minutes) as f32, avg_y: (a.ay / a.minutes) as f32, m, pct: vec![] })
        })
        .collect();
    players.sort_by(|a, b| b.minutes.partial_cmp(&a.minutes).unwrap().then(a.id.cmp(&b.id)));
    // role percentiles over the qualifying pool
    let mut pools: BTreeMap<ScoutRole, Vec<usize>> = BTreeMap::new();
    for (i, p) in players.iter().enumerate() {
        if let (Some(r), true) = (p.role, p.minutes >= pool_minutes) {
            pools.entry(r).or_default().push(i);
        }
    }
    let values: Vec<Vec<f32>> = players.iter().map(|p| p.m.clone()).collect();
    for (_, idx) in &pools {
        for k in 0..METRICS.len() {
            let pool: Vec<f64> = idx.iter().map(|&i| values[i][k] as f64).collect();
            for &i in idx {
                let pc = percentile(values[i][k] as f64, &pool).round() as u8;
                if players[i].pct.is_empty() {
                    players[i].pct = vec![50; METRICS.len()];
                }
                players[i].pct[k] = pc;
            }
        }
    }
    for p in players.iter_mut() {
        if p.pct.is_empty() {
            p.pct = vec![50; METRICS.len()];
        }
    }
    ScoutIndex { version: CATALOG_VERSION, season: season.into(), generated_at: generated_at.into(), pool_minutes, metrics: METRICS.iter().map(|(l, _)| l.to_string()).collect(), players }
}

// ---------------------------------------------------------------- packets

pub fn encode_packet(m: &Match) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(m)?;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(&json)?;
    Ok(enc.finish()?)
}

pub fn decode_packet(bytes: &[u8]) -> Result<Match> {
    let mut json = Vec::new();
    if bytes.starts_with(&[0x1f, 0x8b]) {
        flate2::read::GzDecoder::new(bytes).read_to_end(&mut json).context("gunzip packet")?;
    } else {
        json = bytes.to_vec();
    }
    serde_json::from_slice(&json).context("read match packet")
}

pub fn read_packet(path: &Path) -> Result<Match> {
    decode_packet(&std::fs::read(path).with_context(|| format!("read {}", path.display()))?)
}

// ---------------------------------------------------------------- local cache and refresh

/// Local cache of the two published files (and the packets downloaded on demand).
#[derive(Debug, Clone)]
pub struct CatalogStore {
    pub dir: PathBuf,
}

/// Where a published data branch is read from: `https://raw.githubusercontent.com/<repo>/<branch>/`.
#[derive(Debug, Clone)]
pub struct Remote {
    pub repo: String,
    pub branch: String,
    pub token: Option<String>,
}

impl Remote {
    pub fn base(&self) -> String {
        format!("https://raw.githubusercontent.com/{}/{}/", self.repo.trim().trim_matches('/'), self.branch)
    }
    fn headers(&self) -> Vec<(&str, String)> {
        match self.token.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            Some(t) => vec![("Authorization", format!("Bearer {t}"))],
            None => vec![],
        }
    }
    pub fn fetch(&self, relative: &str, timeout: u64) -> Result<Vec<u8>> {
        let url = format!("{}{}", self.base(), relative.trim_start_matches('/'));
        let h = self.headers();
        let hs: Vec<(&str, &str)> = h.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let (status, body) = crate::net::get(&url, &hs, timeout)?;
        match status {
            200 => Ok(body),
            404 => anyhow::bail!("{} has no file {relative} yet (the nightly job has not published, or the repository is private and the token is missing)", self.repo),
            401 | 403 => anyhow::bail!("GitHub refused {relative} (HTTP {status}): check the token in Settings"),
            s => anyhow::bail!("GitHub answered HTTP {s} for {relative}"),
        }
    }
}

impl CatalogStore {
    pub fn new(dir: &Path) -> CatalogStore {
        CatalogStore { dir: dir.to_path_buf() }
    }
    fn catalog_path(&self) -> PathBuf {
        self.dir.join("catalog.json")
    }
    fn scout_path(&self) -> PathBuf {
        self.dir.join("scout_index.json")
    }
    pub fn load_catalog(&self) -> Option<Catalog> {
        serde_json::from_slice(&std::fs::read(self.catalog_path()).ok()?).ok()
    }
    pub fn load_scout(&self) -> Option<ScoutIndex> {
        serde_json::from_slice(&std::fs::read(self.scout_path()).ok()?).ok()
    }
    pub fn save_catalog(&self, c: &Catalog) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.catalog_path(), serde_json::to_vec(c)?)?;
        Ok(())
    }
    pub fn save_scout(&self, s: &ScoutIndex) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.scout_path(), serde_json::to_vec(s)?)?;
        Ok(())
    }
    /// Age of the cached catalog in seconds, `None` when there is none.
    pub fn age_secs(&self) -> Option<u64> {
        std::fs::metadata(self.catalog_path()).ok()?.modified().ok()?.elapsed().ok().map(|d| d.as_secs())
    }
    /// Downloads both published files and caches them. A failed scout index keeps the old one.
    pub fn refresh(&self, remote: &Remote) -> Result<(Catalog, Option<ScoutIndex>)> {
        let c: Catalog = serde_json::from_slice(&remote.fetch("catalog.json", 30)?).context("catalog.json is not valid")?;
        self.save_catalog(&c)?;
        let s = remote.fetch("scout_index.json", 60).ok().and_then(|b| serde_json::from_slice::<ScoutIndex>(&b).ok());
        if let Some(s) = &s {
            self.save_scout(s)?;
        }
        Ok((c, s.or_else(|| self.load_scout())))
    }
    /// Local path of a packet, downloading it when it is not cached yet.
    pub fn packet(&self, remote: &Remote, m: &CatalogMatch) -> Result<PathBuf> {
        let path = self.dir.join("packets").join(m.url.trim_start_matches('/').replace('/', "_"));
        if path.exists() && read_packet(&path).is_ok() {
            return Ok(path);
        }
        let bytes = remote.fetch(&m.url, 60)?;
        decode_packet(&bytes).context("downloaded packet is not a match")?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
}
