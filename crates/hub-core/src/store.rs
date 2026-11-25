//! Competition store: one JSON line per team-match, used by the context boards
//! and percentile pools. Rows carry metric_version so definition changes never mix.

use crate::derive::metrics::Analysis;
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMatchRow {
    pub match_id: String,
    pub competition: String,
    pub season: String,
    pub date: String,
    pub team: String,
    pub opponent: String,
    pub metric_version: String,
    pub goals: usize,
    pub xg: f64,
    pub npxg: f64,
    pub field_tilt: f64,
    pub ppda: Option<f64>,
    pub dangerous_counter_pct: f64,
    pub box_entries: usize,
    pub high_regains: usize,
}

pub fn rows_for(m: &Match, a: &Analysis) -> Vec<TeamMatchRow> {
    Side::both()
        .iter()
        .map(|&s| {
            let t = a.t(s);
            let ev = &m.events;
            let adv: Vec<usize> = (0..ev.len()).filter(|&i| { let e = &ev[i]; e.side == s && e.x >= 52.5 && e.restart.is_none() && ((e.kind == Kind::Pass && !e.success) || matches!(e.kind, Kind::Dispossessed) || (e.kind == Kind::TakeOn && !e.success)) }).collect();
            let adv_losses = adv.len();
            let dangerous = adv.iter().filter(|&&i| (i + 1..ev.len()).take_while(|&j| ev[j].t - ev[i].t <= 12.0).any(|j| ev[j].side != s && ev[j].kind == Kind::Shot)).count();
            TeamMatchRow {
                match_id: m.id.clone(),
                competition: m.competition.clone(),
                season: m.season.clone(),
                date: m.date.clone(),
                team: m.team(s).name.clone(),
                opponent: m.team(s.other()).name.clone(),
                metric_version: crate::METRIC_VERSION.into(),
                goals: t.goals + t.own_goals_for,
                xg: t.xg,
                npxg: t.npxg,
                field_tilt: t.field_tilt,
                ppda: t.ppda,
                dangerous_counter_pct: if adv_losses > 0 { dangerous as f64 / adv_losses as f64 * 100.0 } else { 0.0 },
                box_entries: t.entries.len(),
                high_regains: t.high_regains.len(),
            }
        })
        .collect()
}

pub fn load(path: &Path) -> Vec<TeamMatchRow> {
    std::fs::read_to_string(path).map(|s| s.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()).unwrap_or_default()
}

/// Upsert rows for a match (replaces existing rows with the same match id).
pub fn upsert(path: &Path, rows: &[TeamMatchRow]) -> anyhow::Result<()> {
    let mut all: Vec<TeamMatchRow> = load(path).into_iter().filter(|r| !rows.iter().any(|n| n.match_id == r.match_id)).collect();
    all.extend_from_slice(rows);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut f = std::fs::File::create(path)?;
    for r in all {
        writeln!(f, "{}", serde_json::to_string(&r)?)?;
    }
    Ok(())
}
