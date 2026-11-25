//! Every match metric used by the boards and the narrative, computed once.
//! One function per definition so every board reads the same number.

use super::possession::{Possession, StartType};
use super::*;
use crate::ml::Values;
use crate::model::*;
use crate::sim::{LiveCurve, WinSim};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

pub const HIGH_REGAIN_X: f64 = 63.0; // attacking 40%
pub const HIGH_TURNOVER_X: f64 = 70.0; // x >= 66.7% of length
pub const CLEAR_CHANCE_XG: f64 = 0.12;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Bucket {
    pub n: usize,
    pub xg: f64,
    pub goals: usize,
}
impl Bucket {
    fn add(&mut self, xg: f64, goal: bool) {
        self.n += 1;
        self.xg += xg;
        if goal {
            self.goals += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum Lane {
    Left,
    Centre,
    Right,
}
fn lane_box(y: f64) -> Lane {
    if y > 43.15 {
        Lane::Left
    } else if y < 24.85 {
        Lane::Right
    } else {
        Lane::Centre
    }
}
fn lane_thirds(y: f64) -> Lane {
    if y > 45.33 {
        Lane::Left
    } else if y < 22.67 {
        Lane::Right
    } else {
        Lane::Centre
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub idx: usize,
    pub carry: bool,
    pub shot_followed: bool,
    pub lane: Lane,
    pub player: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum RegainOutcome {
    Nothing,
    Box,
    Shot,
}

#[derive(Debug, Clone, Serialize)]
pub struct Regain {
    pub idx: usize,
    pub outcome: RegainOutcome,
    pub xg: f64,
    pub trigger: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Turnover {
    pub idx: usize,
    pub shot: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Transition {
    pub start: (f64, f64),
    pub adv: (f64, f64),
    pub shot: bool,
    pub goal: bool,
    pub xg: f64,
    pub box_entry: bool,
    pub poss: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ThirdStats {
    pub passes: Vec<usize>,
    pub completed: usize,
    pub forward: usize,
    pub into_next: usize,
    pub key_passes: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct GkStats {
    pub keeper: Option<u64>,
    pub keeper_name: String,
    pub faced_total: usize,
    pub off_target: usize,
    pub woodwork: usize,
    pub blocked: usize,
    pub on_target: usize,
    pub saves: usize,
    pub conceded: usize,
    pub psxg_faced: f64,
    pub prevented: f64,
    pub claims: usize,
    pub sweeps: usize,
    pub passes: usize,
    pub avg_len: f64,
    pub launched_pct: f64,
    pub completed_pct: f64,
    pub held: usize,
    pub parried_safe: usize,
    pub into_danger: usize,
    pub shots_faced: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TeamStats {
    pub name: String,
    pub goals: usize,
    pub own_goals_for: usize,
    pub npxg: f64,
    pub xg: f64,
    pub xgot: f64,
    pub xg_on_target: f64,
    pub shots: Vec<usize>,
    pub on_target: usize,
    pub saved: usize,
    pub blocked: usize,
    pub off_target: usize,
    pub woodwork: usize,
    pub clear_chances: Vec<usize>,
    pub frame_zones: Vec<(String, usize)>,
    pub possession_pct: f64,
    pub field_tilt: f64,
    pub passes: usize,
    pub passes_completed: usize,
    pub key_passes: usize,
    pub progressive_passes: usize,
    pub line_breaking: (usize, usize),
    pub positive_xt: f64,
    pub entries: Vec<Entry>,
    pub z14: Vec<usize>,
    pub hs_left: Vec<usize>,
    pub hs_right: Vec<usize>,
    pub cutbacks: Vec<usize>,
    pub through_balls: Vec<usize>,
    pub six: Bucket,
    pub in_box: Bucket,
    pub outside: Bucket,
    pub headed: Bucket,
    pub first_touch: Bucket,
    pub sp_open: Bucket,
    pub sp_corner: Bucket,
    pub sp_fk: Bucket,
    pub sp_pen: Bucket,
    pub corners: usize,
    pub corners_shot10: usize,
    pub buildup_attempts: usize,
    pub buildup_success: usize,
    pub own_third_to_mid: usize,
    pub mid_pass_pct: f64,
    pub ft_entries: usize,
    pub ft_lanes: [usize; 3],
    pub secs_to_final_third: f64,
    pub touches: usize,
    pub touches_ft: usize,
    pub ppda: Option<f64>,
    pub ppda_h1: Option<f64>,
    pub ppda_h2: Option<f64>,
    pub ppda_bins: Vec<(String, Option<f64>)>,
    pub def_height: f64,
    pub stopped_height: f64,
    pub def_by_third: [usize; 3],
    pub won: Vec<usize>,
    pub stopped: Vec<usize>,
    pub high_regains: Vec<Regain>,
    pub high_turnovers: Vec<Turnover>,
    pub own_errors: Vec<usize>,
    pub own_errors_shot: usize,
    pub own_errors_goal: usize,
    pub opp_errors_forced: Vec<usize>,
    pub counterpress: (usize, usize),
    pub counterpress_xg: f64,
    pub open_play_losses: usize,
    pub losses_to_shot: usize,
    pub losses_to_shot_xg: f64,
    pub transitions: Vec<Transition>,
    pub second_balls: (usize, usize),
    pub press_triggers: Vec<(&'static str, usize)>,
    pub aerials: (usize, usize),
    pub take_ons: (usize, usize),
    pub tackles: usize,
    pub tackles_won: usize,
    pub interceptions: usize,
    pub blocks: usize,
    pub clearances: usize,
    pub recoveries: usize,
    pub regains_all: usize,
    pub fouls: usize,
    pub cards: usize,
    pub avg_age: Option<f64>,
    pub age_excluded: usize,
    pub funnel: [usize; 5],
    pub seq_threat: f64,
    pub threat_per_poss: f64,
    pub xt_grid: Vec<f64>,
    pub top_xt: Vec<(usize, f64)>,
    pub vaep: f64,
    pub gk: GkStats,
    pub by_third: [ThirdStats; 3],
    pub crosses: (usize, usize),
    pub carries: usize,
    pub xpts_turnover: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PlayerStats {
    pub id: u64,
    pub side: Option<Side>,
    pub name: String,
    pub short: String,
    pub shirt: u32,
    pub role: Option<Role>,
    pub minutes: f64,
    pub touches: usize,
    pub touches_by_third: [usize; 3],
    pub passes: usize,
    pub passes_completed: usize,
    pub progressive_passes: usize,
    pub progressive_carries: usize,
    pub prog_pass_share: f64,
    pub line_breaks: usize,
    pub ft_receptions: usize,
    pub box_entries: usize,
    pub key_passes: usize,
    pub shots: usize,
    pub xg: f64,
    pub goals: usize,
    pub inferred_xa: f64,
    pub xgchain: f64,
    pub xgbuildup: f64,
    pub positive_xt: f64,
    pub progression_m: f64,
    pub def_actions: usize,
    pub padj_def: f64,
    pub def_height: f64,
    pub recoveries: usize,
    pub interceptions: usize,
    pub tackles_won: usize,
    pub dribbled_past: usize,
    pub aerials: (usize, usize),
    pub take_ons: (usize, usize),
    pub dispossessed: usize,
    pub clearances: usize,
    pub vaep_off: f64,
    pub vaep_def: f64,
    pub avg_x: f64,
    pub avg_y: f64,
    pub saves: usize,
    pub claims: usize,
    pub sweeps: usize,
    pub xa_per100: f64,
    pub assists: usize,
    pub errors_under_pressure: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct StateRow {
    pub label: String,
    pub minutes: f64,
    pub shots: [usize; 2],
    pub xg: [f64; 2],
    pub transitions: [usize; 2],
    pub entries: [usize; 2],
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Momentum {
    pub minutes: Vec<f64>,
    pub home: Vec<f64>,
    pub away: Vec<f64>,
    pub spells: Vec<Spell>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Spell {
    pub start: f64,
    pub end: f64,
    pub share_home: f64,
    pub xg_home: f64,
    pub xg_away: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoalOrigin {
    pub shot: usize,
    pub side: Side,
    pub chain: Vec<usize>,
    pub passes: usize,
    pub secs: f64,
    pub players: usize,
    pub start: String,
    pub label: String,
    pub assist: Option<u64>,
    pub xg: f64,
    pub body: Body,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct HeightSeries {
    pub windows: Vec<f64>,
    pub height: [Vec<Option<f64>>; 2],
    pub length: [Vec<Option<f64>>; 2],
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Analysis {
    #[serde(skip)]
    pub possessions: Vec<Possession>,
    #[serde(skip)]
    pub values: Values,
    pub team: [TeamStats; 2],
    pub players: Vec<PlayerStats>,
    pub win: WinSim,
    pub live: LiveCurve,
    pub momentum: Momentum,
    pub states: Vec<StateRow>,
    pub goals: Vec<GoalOrigin>,
    pub height: HeightSeries,
    pub zones: Vec<Vec<f64>>,
    pub zone_touches: [usize; 2],
    pub checks: Vec<Check>,
    pub duration_min: f64,
    #[serde(skip)]
    pub receiver: Vec<Option<u64>>,
}

impl Analysis {
    pub fn t(&self, s: Side) -> &TeamStats {
        &self.team[s.idx()]
    }
    pub fn player(&self, id: u64) -> Option<&PlayerStats> {
        self.players.iter().find(|p| p.id == id)
    }
}

fn is_prog(e: &Event) -> bool {
    if !(e.kind == Kind::Pass || e.kind == Kind::Carry) || !e.success {
        return false;
    }
    if matches!(e.restart, Some(Restart::Corner) | Some(Restart::ThrowIn) | Some(Restart::GoalKick) | Some(Restart::KickOff)) {
        return false;
    }
    let (ex, ey) = e.end();
    let d0 = dist_to_goal(e.x, e.y);
    let d1 = dist_to_goal(ex, ey);
    (e.x >= 35.0 && d1 <= 0.75 * d0 && ex - e.x >= 5.0) || (!in_box(e.x, e.y) && in_box(ex, ey))
}

fn is_line_break(e: &Event) -> bool {
    if e.kind != Kind::Pass || e.restart.is_some() {
        return false;
    }
    let (ex, ey) = e.end();
    ex - e.x >= 10.0 && ex >= 35.0 && e.x < 88.5 && ((13.84..=54.16).contains(&e.y) || (13.84..=54.16).contains(&ey)) && !e.pass.as_ref().map(|p| p.cross).unwrap_or(false)
}

fn is_loss(e: &Event) -> bool {
    e.restart.is_none() && ((e.kind == Kind::Pass && !e.success) || e.kind == Kind::Dispossessed || (e.kind == Kind::BadTouch && !e.success) || (e.kind == Kind::TakeOn && !e.success))
}

fn ppda(m: &Match, press: Side, filter: impl Fn(&Event) -> bool) -> Option<f64> {
    let opp = press.other();
    let passes = m.events.iter().filter(|e| e.side == opp && e.kind == Kind::Pass && e.x <= 63.0 && filter(e)).count();
    let defs = m
        .events
        .iter()
        .filter(|e| e.side == press && matches!(e.kind, Kind::Tackle | Kind::Interception | Kind::Challenge | Kind::Foul) && e.x >= 42.0 && filter(e))
        .count();
    if defs == 0 {
        None
    } else {
        Some(passes as f64 / defs as f64)
    }
}

fn ppda_bin(e: &Event) -> usize {
    let m = e.minute;
    match e.period {
        1 => (m / 15).min(2) as usize,
        2 => 3 + ((m.saturating_sub(45)) / 15).min(2) as usize,
        3 => 6,
        _ => 7,
    }
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(v[v.len() / 2])
}
fn iqr(v: &mut [f64]) -> Option<f64> {
    if v.len() < 4 {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(v[v.len() * 3 / 4] - v[v.len() / 4])
}

pub fn analyse(m: &Match, values: Values, win: WinSim, live: LiveCurve) -> Analysis {
    let ev = &m.events;
    let poss = super::possession::build(m);
    let dur = m.duration() / 60.0;
    let mut a = Analysis { values, win, live, duration_min: dur, ..Default::default() };
    let vals = &a.values;

    // possession lookup
    let mut poss_of: HashMap<u32, usize> = HashMap::new();
    for (i, p) in poss.iter().enumerate() {
        poss_of.insert(p.id, i);
    }
    // receivers: next own-side on-ball event after a completed pass, within 6 s
    let mut receiver = vec![None; ev.len()];
    for i in 0..ev.len() {
        if ev[i].kind == Kind::Pass && ev[i].success {
            if let Some(rp) = ev[i].related_player.filter(|_| m.source == "statsbomb") {
                receiver[i] = Some(rp);
                continue;
            }
            for j in i + 1..(i + 6).min(ev.len()) {
                if ev[j].t - ev[i].t > 6.0 {
                    break;
                }
                if ev[j].side == ev[i].side && ev[j].kind.is_on_ball() && ev[j].player != ev[i].player {
                    receiver[i] = ev[j].player;
                    break;
                }
            }
        }
    }

    for s in Side::both() {
        let si = s.idx();
        let mut t = TeamStats { name: m.team(s).name.clone(), ..Default::default() };
        let opp = s.other();
        // --- shots
        let mut frame: HashMap<String, usize> = HashMap::new();
        let sp_of: HashMap<usize, StartType> = poss.iter().filter(|p| p.side == s).flat_map(|p| p.shots.iter().map(move |&q| (q, p.start_type))).collect();
        for (i, e) in ev.iter().enumerate() {
            if e.side != s || e.kind != Kind::Shot || e.period > 4 {
                continue;
            }
            let sh = e.shot.as_ref().unwrap();
            if sh.own_goal {
                continue;
            }
            let goal = sh.outcome == ShotOutcome::Goal;
            t.shots.push(i);
            t.xg += sh.xg;
            if goal {
                t.goals += 1;
            }
            match sh.outcome {
                ShotOutcome::Goal | ShotOutcome::Saved => {
                    t.on_target += 1;
                    t.xg_on_target += if sh.penalty { 0.0 } else { sh.xg };
                    if !sh.penalty {
                        t.xgot += sh.xgot_ext.or(sh.psxg).unwrap_or(0.0);
                    }
                    if sh.outcome == ShotOutcome::Saved {
                        t.saved += 1;
                    }
                    let u = sh.goal_u.unwrap_or(0.0);
                    let z = sh.goal_z.unwrap_or(0.3);
                    let col = if u > 0.33 { "Left" } else if u < -0.33 { "Right" } else { "Centre" };
                    let row = if z > 0.5 { "High" } else if z > 0.25 { "Mid" } else { "Low" };
                    let key = if sh.goal_u.is_none() { "Other zones".to_string() } else if row == "Mid" && col != "Centre" { "Other zones".into() } else { format!("{row} {col}") };
                    *frame.entry(key).or_default() += 1;
                }
                ShotOutcome::Blocked => t.blocked += 1,
                ShotOutcome::OffTarget => t.off_target += 1,
                ShotOutcome::Woodwork => t.woodwork += 1,
            }
            if sh.penalty {
                t.sp_pen.add(sh.xg, goal);
                continue;
            }
            t.npxg += sh.xg;
            if sh.big_chance || sh.xg >= CLEAR_CHANCE_XG {
                t.clear_chances.push(i);
            }
            if in_six(e.x, e.y) {
                t.six.add(sh.xg, goal);
            }
            if in_box(e.x, e.y) {
                t.in_box.add(sh.xg, goal);
            } else {
                t.outside.add(sh.xg, goal);
            }
            if sh.body == Body::Head {
                t.headed.add(sh.xg, goal);
            }
            if sh.first_time {
                t.first_touch.add(sh.xg, goal);
            }
            match sp_of.get(&i) {
                Some(StartType::Corner) => t.sp_corner.add(sh.xg, goal),
                Some(StartType::FreeKick) => t.sp_fk.add(sh.xg, goal),
                _ => t.sp_open.add(sh.xg, goal),
            }
        }
        t.own_goals_for = ev.iter().filter(|e| e.side == opp && e.shot.as_ref().map(|x| x.own_goal).unwrap_or(false)).count();
        let mut fz: Vec<(String, usize)> = frame.into_iter().collect();
        fz.sort_by(|a, b| (a.0 == "Other zones").cmp(&(b.0 == "Other zones")).then(b.1.cmp(&a.1)));
        t.frame_zones = fz;

        // --- passing
        let all_passes = ev.iter().filter(|e| e.kind == Kind::Pass).count().max(1);
        let ft_passes = ev.iter().filter(|e| e.kind == Kind::Pass && e.x >= 70.0).count().max(1);
        let mut mid = (0, 0);
        let mut ft_t = vec![];
        for (i, e) in ev.iter().enumerate() {
            if e.side != s {
                continue;
            }
            if e.kind.is_on_ball() && !matches!(e.kind, Kind::Foul | Kind::Card) {
                t.touches += 1;
                if e.x >= 70.0 {
                    t.touches_ft += 1;
                }
            }
            if e.kind == Kind::Carry {
                t.carries += 1;
            }
            if e.kind == Kind::Pass {
                t.passes += 1;
                if e.success {
                    t.passes_completed += 1;
                }
                let third = if e.x < 35.0 { 0 } else if e.x < 70.0 { 1 } else { 2 };
                let bt = &mut t.by_third[third];
                bt.passes.push(i);
                let (ex, ey) = e.end();
                if e.success {
                    bt.completed += 1;
                    let next = if third == 2 { in_box(ex, ey) && !in_box(e.x, e.y) } else { ex >= if third == 0 { 35.0 } else { 70.0 } };
                    if next {
                        bt.into_next += 1;
                    }
                }
                if ex > e.x + 2.0 {
                    bt.forward += 1;
                }
                if e.pass.as_ref().map(|p| p.key_pass).unwrap_or(false) {
                    t.key_passes += 1;
                    bt.key_passes += 1;
                }
                if third == 1 {
                    mid.0 += 1;
                    if e.success {
                        mid.1 += 1;
                    }
                }
                if third == 0 && e.success && ex >= 35.0 {
                    t.own_third_to_mid += 1;
                }
                if e.pass.as_ref().map(|p| p.cross).unwrap_or(false) {
                    t.crosses.0 += e.success as usize;
                    t.crosses.1 += 1;
                }
                if is_line_break(e) {
                    t.line_breaking.1 += 1;
                    if e.success {
                        t.line_breaking.0 += 1;
                    }
                }
                if e.restart == Some(Restart::Corner) {
                    t.corners += 1;
                    if ev.iter().skip(i + 1).take_while(|x| x.t - e.t <= 10.0).any(|x| x.side == s && x.kind == Kind::Shot) {
                        t.corners_shot10 += 1;
                    }
                }
                if e.success {
                    if in_z14(ex, ey) && !in_z14(e.x, e.y) || in_z14(ex, ey) && e.x < 70.0 {
                        t.z14.push(i);
                    } else if in_hs_left(ex, ey) && !in_hs_left(e.x, e.y) {
                        t.hs_left.push(i);
                    } else if in_hs_right(ex, ey) && !in_hs_right(e.x, e.y) {
                        t.hs_right.push(i);
                    }
                    if e.pass.as_ref().map(|p| p.cutback).unwrap_or(false) {
                        t.cutbacks.push(i);
                    }
                }
                if e.pass.as_ref().map(|p| p.through).unwrap_or(false) {
                    t.through_balls.push(i);
                }
            }
            if is_prog(e) && e.kind == Kind::Pass {
                t.progressive_passes += 1;
            }
            if (e.kind == Kind::Pass && e.success || e.kind == Kind::Carry) && vals.xt_added[i] > 0.0 {
                t.positive_xt += vals.xt_added[i];
            }
            // final-third entries
            if (e.kind == Kind::Pass && e.success || e.kind == Kind::Carry) && e.x < 70.0 && e.end().0 >= 70.0 {
                t.ft_entries += 1;
                let l = lane_thirds(e.end().1);
                t.ft_lanes[match l {
                    Lane::Left => 0,
                    Lane::Centre => 1,
                    Lane::Right => 2,
                }] += 1;
            }
            // box entries
            if (e.kind == Kind::Pass && e.success || e.kind == Kind::Carry) && !in_box(e.x, e.y) && in_box(e.end().0, e.end().1) && e.restart != Some(Restart::Corner) {
                let pi = poss_of.get(&e.possession).copied();
                let shot_followed = pi.map(|p| poss[p].shots.iter().any(|&q| q > i)).unwrap_or(false);
                t.entries.push(Entry { idx: i, carry: e.kind == Kind::Carry, shot_followed, lane: lane_box(e.y), player: e.player });
            }
        }
        t.mid_pass_pct = if mid.0 > 0 { mid.1 as f64 / mid.0 as f64 } else { 0.0 };
        t.possession_pct = t.passes as f64 / all_passes as f64;
        t.field_tilt = ev.iter().filter(|e| e.side == s && e.kind == Kind::Pass && e.x >= 70.0).count() as f64 / ft_passes as f64;

        // --- possession-based
        let own: Vec<&Possession> = poss.iter().filter(|p| p.side == s).collect();
        t.funnel[0] = own.len();
        t.funnel[1] = own.iter().filter(|p| p.reached_final_third).count();
        t.funnel[2] = own.iter().filter(|p| p.touched_box).count();
        t.funnel[3] = own.iter().filter(|p| p.shot_from_box).count();
        t.funnel[4] = own.iter().filter(|p| p.shot_on_target_from_box).count();
        for p in &own {
            if p.start_x < 35.0 && !matches!(p.start_type, StartType::Corner | StartType::KickOff) {
                t.buildup_attempts += 1;
                let mut ok = false;
                for &q in &p.idx {
                    let e = &ev[q];
                    if e.kind == Kind::Pass && e.pass.as_ref().map(|x| x.long || x.launch).unwrap_or(false) {
                        break;
                    }
                    if (e.kind == Kind::Pass && e.success || e.kind == Kind::Carry) && e.end().0 >= 35.0 {
                        ok = true;
                        break;
                    }
                }
                if ok {
                    t.buildup_success += 1;
                }
            }
            if let Some(&q) = p.idx.iter().find(|&&q| ev[q].x >= 70.0 || (ev[q].success || ev[q].kind == Kind::Carry) && ev[q].end().0 >= 70.0) {
                ft_t.push(ev[q].t - p.start_t);
            }
            let th: f64 = p.idx.iter().map(|&q| vals.xt_added[q].max(0.0)).sum();
            t.seq_threat += th;
        }
        t.threat_per_poss = t.seq_threat / own.len().max(1) as f64;
        t.secs_to_final_third = if ft_t.is_empty() { 0.0 } else { ft_t.iter().sum::<f64>() / ft_t.len() as f64 };

        // --- PPDA
        t.ppda = ppda(m, s, |_| true);
        t.ppda_h1 = ppda(m, s, |e| e.period == 1);
        t.ppda_h2 = ppda(m, s, |e| e.period >= 2);
        let has_et = ev.iter().any(|e| e.period >= 3);
        let labels = ["0–15'", "15–30'", "30–45'", "45–60'", "60–75'", "75–90'", "90–105'", "105–120'"];
        let nb = if has_et { 8 } else { 6 };
        for b in 0..nb {
            let v = ppda(m, s, |e| ppda_bin(e) == b);
            let mut lab = labels[b].to_string();
            if b == 5 && !has_et {
                let last = ev.iter().filter(|e| e.period == 2).map(|e| e.minute).max().unwrap_or(90) + 1;
                lab = format!("75–{last}'");
            }
            t.ppda_bins.push((lab, v.map(|x| x.min(30.0))));
        }

        // --- defending
        let mut wx = vec![];
        let mut sx = vec![];
        for (i, e) in ev.iter().enumerate() {
            if e.side != s {
                continue;
            }
            match e.kind {
                Kind::Tackle => {
                    t.tackles += 1;
                    if e.success {
                        t.tackles_won += 1;
                    }
                }
                Kind::Interception => t.interceptions += 1,
                Kind::Block => t.blocks += 1,
                Kind::Clearance => t.clearances += 1,
                Kind::Recovery => t.recoveries += 1,
                Kind::Foul => t.fouls += 1,
                Kind::Card => t.cards += 1,
                Kind::Aerial => {
                    t.aerials.1 += 1;
                    if e.success {
                        t.aerials.0 += 1;
                    }
                }
                Kind::TakeOn => {
                    t.take_ons.1 += 1;
                    if e.success {
                        t.take_ons.0 += 1;
                    }
                }
                _ => {}
            }
            let won = (e.kind == Kind::Tackle && e.success) || matches!(e.kind, Kind::Interception | Kind::Recovery);
            if won {
                t.won.push(i);
                wx.push(e.x);
                t.regains_all += 1;
            }
            if matches!(e.kind, Kind::Clearance | Kind::Foul | Kind::Block) {
                t.stopped.push(i);
                sx.push(e.x);
            }
            if won || matches!(e.kind, Kind::Clearance | Kind::Foul | Kind::Block | Kind::Tackle | Kind::Challenge) {
                t.def_by_third[if e.x >= 70.0 { 0 } else if e.x >= 35.0 { 1 } else { 2 }] += 1;
            }
            // high regains
            if won && e.x >= HIGH_REGAIN_X && e.restart.is_none() {
                let mut outcome = RegainOutcome::Nothing;
                let mut xg = 0.0;
                let pid = ev[i..].iter().take(8).find(|x| x.side == s && x.kind.controls_ball()).map(|x| x.possession);
                if let Some(p) = pid.and_then(|p| poss_of.get(&p)).map(|&k| &poss[k]) {
                    if !p.shots.is_empty() {
                        outcome = RegainOutcome::Shot;
                        xg = p.shots.iter().map(|&q| ev[q].shot.as_ref().unwrap().xg).sum();
                    } else if p.touched_box {
                        outcome = RegainOutcome::Box;
                    }
                }
                // trigger: previous opponent action within 5 s
                let mut trig = None;
                for j in (i.saturating_sub(4)..i).rev() {
                    let x = &ev[j];
                    if e.t - x.t > 5.0 {
                        break;
                    }
                    if x.side != opp {
                        continue;
                    }
                    trig = match x.kind {
                        Kind::BadTouch => Some("loose touch"),
                        Kind::Pass if !x.success => Some("opponent pass"),
                        Kind::Dispossessed => Some("dispossessed"),
                        Kind::Interception => Some("interception"),
                        Kind::Clearance => Some("clearance"),
                        Kind::TakeOn if !x.success => Some("dribble attempt"),
                        Kind::Error => Some("error"),
                        _ => None,
                    };
                    if trig.is_some() {
                        break;
                    }
                }
                t.high_regains.push(Regain { idx: i, outcome, xg, trigger: trig });
            }
            if won && e.x >= HIGH_TURNOVER_X && e.restart.is_none() {
                let shot = (i + 1..ev.len()).take_while(|&j| ev[j].t - e.t <= 12.0).find(|&j| ev[j].side == s && ev[j].kind == Kind::Shot);
                t.high_turnovers.push(Turnover { idx: i, shot });
            }
            if e.kind == Kind::Error {
                t.own_errors.push(i);
                let sh = (i + 1..ev.len()).take_while(|&j| ev[j].t - e.t <= 12.0).find(|&j| ev[j].side == opp && ev[j].kind == Kind::Shot);
                if let Some(j) = sh {
                    t.own_errors_shot += 1;
                    if ev[j].shot.as_ref().unwrap().outcome == ShotOutcome::Goal {
                        t.own_errors_goal += 1;
                    }
                }
            }
            // counterpress + losses
            if is_loss(e) {
                t.open_play_losses += 1;
                if (i + 1..ev.len()).take_while(|&j| ev[j].t - e.t <= 5.0).any(|j| ev[j].side == s && ((ev[j].kind == Kind::Tackle && ev[j].success) || matches!(ev[j].kind, Kind::Interception | Kind::Recovery))) {
                    t.counterpress.0 += 1;
                    if let Some(k) = (i + 1..ev.len()).take_while(|&j| ev[j].t - e.t <= 8.0).find(|&j| ev[j].side == s && ev[j].kind.controls_ball()) {
                        if let Some(p) = poss_of.get(&ev[k].possession).map(|&q| &poss[q]) {
                            t.counterpress_xg += p.shots.iter().map(|&q| ev[q].shot.as_ref().unwrap().xg).sum::<f64>();
                        }
                    }
                }
                if let Some(j) = (i + 1..ev.len()).take_while(|&j| ev[j].t - e.t <= 12.0).find(|&j| ev[j].side == opp && ev[j].kind == Kind::Shot) {
                    t.losses_to_shot += 1;
                    t.losses_to_shot_xg += ev[j].shot.as_ref().unwrap().xg;
                }
            }
        }
        t.counterpress.1 = t.open_play_losses;
        t.opp_errors_forced = ev.iter().enumerate().filter(|(_, e)| e.side == opp && e.kind == Kind::Error).map(|(i, _)| i).collect();
        t.def_height = if wx.is_empty() { 0.0 } else { wx.iter().sum::<f64>() / wx.len() as f64 };
        t.stopped_height = if sx.is_empty() { 0.0 } else { sx.iter().sum::<f64>() / sx.len() as f64 };
        let mut trig: HashMap<&'static str, usize> = HashMap::new();
        for r in &t.high_regains {
            if let Some(k) = r.trigger {
                *trig.entry(k).or_default() += 1;
            }
        }
        let mut tv: Vec<(&'static str, usize)> = trig.into_iter().collect();
        tv.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        t.press_triggers = tv;

        // --- transitions (open-play regains)
        for p in &own {
            if !matches!(p.start_type, StartType::Recovery | StartType::Interception | StartType::Tackle | StartType::Keeper | StartType::Other) {
                continue;
            }
            let st = &ev[p.start_idx];
            let (sx0, sy0) = (st.x, st.y);
            let mut adv = (sx0, sy0);
            let mut shot = false;
            let mut goal = false;
            let mut xg = 0.0;
            let mut boxe = false;
            for &q in &p.idx {
                let e = &ev[q];
                if e.t - p.start_t > 12.0 {
                    break;
                }
                let (ex, ey) = if e.kind == Kind::Pass && e.success || e.kind == Kind::Carry { e.end() } else { (e.x, e.y) };
                if ex > adv.0 {
                    adv = (ex, ey);
                }
                if e.x > adv.0 {
                    adv = (e.x, e.y);
                }
                if e.kind == Kind::Shot {
                    shot = true;
                    let sh = e.shot.as_ref().unwrap();
                    xg += sh.xg;
                    goal |= sh.outcome == ShotOutcome::Goal;
                }
                if (e.kind == Kind::Pass && e.success || e.kind == Kind::Carry) && !in_box(e.x, e.y) && in_box(ex, ey) {
                    boxe = true;
                }
            }
            if adv.0 - sx0 >= 20.0 || adv.0 >= 70.0 && sx0 < 70.0 || shot || boxe {
                t.transitions.push(Transition { start: (sx0, sy0), adv, shot, goal, xg, box_entry: boxe, poss: p.id });
            }
        }

        // --- second balls
        for (i, e) in ev.iter().enumerate() {
            if !matches!(e.kind, Kind::Aerial | Kind::Clearance) {
                continue;
            }
            if e.kind == Kind::Aerial && e.side != Side::Home {
                // count each aerial duel once (home record), clearances from both sides
                continue;
            }
            if let Some(j) = (i + 1..ev.len()).take_while(|&j| ev[j].t - e.t <= 3.0).find(|&j| ev[j].kind == Kind::Recovery) {
                let r = &ev[j];
                let (rx, ry) = if r.side == e.side { (r.x, r.y) } else { (105.0 - r.x, 68.0 - r.y) };
                if ((rx - e.x).powi(2) + (ry - e.y).powi(2)).sqrt() <= 15.0 {
                    if s == Side::Home {
                        // tallied for both teams at once below
                    }
                    if r.side == s {
                        t.second_balls.0 += 1;
                    }
                    t.second_balls.1 += 1;
                }
            }
        }

        // --- xT grid (8 wide x 12 long, by start of successful pass/carry), top passes
        let mut grid = vec![0.0; 96];
        let mut top = vec![];
        for (i, e) in ev.iter().enumerate() {
            if e.side == s && vals.xt_added[i] > 0.0 {
                let c = ((e.y / 68.0 * 8.0).floor().clamp(0.0, 7.0) as usize, (e.x / 105.0 * 12.0).floor().clamp(0.0, 11.0) as usize);
                grid[c.1 * 8 + c.0] += vals.xt_added[i];
                if e.kind == Kind::Pass {
                    top.push((i, vals.xt_added[i]));
                }
            }
        }
        top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        top.truncate(10);
        t.xt_grid = grid;
        t.top_xt = top;
        t.vaep = ev.iter().enumerate().filter(|(_, e)| e.side == s).map(|(i, _)| vals.vaep_off[i] + vals.vaep_def[i]).sum();

        // --- ages
        let mut agesum = 0.0;
        let mut mins = 0.0;
        for p in &m.team(s).players {
            let pm = player_minutes(m, p);
            if pm <= 0.0 {
                continue;
            }
            match p.age {
                Some(age) => {
                    agesum += age * pm;
                    mins += pm;
                }
                None => t.age_excluded += 1,
            }
        }
        t.avg_age = if mins > 0.0 { Some(agesum / mins) } else { None };
        a.team[si] = t;
    }

    // --- goalkeepers (needs both teams)
    for s in Side::both() {
        let opp = s.other();
        let mut g = GkStats::default();
        let keepers: Vec<&Player> = m.team(s).players.iter().filter(|p| p.role == Role::Goalkeeper && player_minutes(m, p) > 0.0).collect();
        if let Some(k) = keepers.iter().max_by(|a, b| player_minutes(m, a).partial_cmp(&player_minutes(m, b)).unwrap()) {
            g.keeper = Some(k.id);
            g.keeper_name = k.name.clone();
        }
        let kids: HashSet<u64> = keepers.iter().map(|p| p.id).collect();
        let mut lens = vec![];
        let (mut launched, mut completed) = (0, 0);
        for (i, e) in ev.iter().enumerate() {
            if e.side == opp && e.kind == Kind::Shot && e.period <= 4 {
                let sh = e.shot.as_ref().unwrap();
                if sh.own_goal {
                    continue;
                }
                g.faced_total += 1;
                g.shots_faced.push(i);
                match sh.outcome {
                    ShotOutcome::OffTarget => g.off_target += 1,
                    ShotOutcome::Woodwork => g.woodwork += 1,
                    ShotOutcome::Blocked => g.blocked += 1,
                    ShotOutcome::Saved => {
                        g.on_target += 1;
                        g.saves += 1;
                    }
                    ShotOutcome::Goal => {
                        g.on_target += 1;
                        g.conceded += 1;
                    }
                }
                if !sh.penalty && matches!(sh.outcome, ShotOutcome::Goal | ShotOutcome::Saved) {
                    let v = sh.xgot_ext.or(sh.psxg).unwrap_or(0.0);
                    g.psxg_faced += v;
                    if sh.outcome == ShotOutcome::Goal {
                        g.prevented -= 1.0;
                    }
                }
            }
            if e.side == s && e.player.map(|p| kids.contains(&p)).unwrap_or(false) {
                match e.kind {
                    Kind::Claim => g.claims += 1,
                    Kind::Sweeper => g.sweeps += 1,
                    Kind::Save => match e.handling {
                        Some(Handling::ParriedDanger) => g.into_danger += 1,
                        Some(Handling::ParriedSafe) => g.parried_safe += 1,
                        _ => g.held += 1,
                    },
                    Kind::Pass => {
                        g.passes += 1;
                        let (ex, ey) = e.end();
                        let l = ((ex - e.x).powi(2) + (ey - e.y).powi(2)).sqrt();
                        lens.push(l);
                        if l >= 40.0 || e.pass.as_ref().map(|p| p.launch).unwrap_or(false) {
                            launched += 1;
                        }
                        if e.success {
                            completed += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        g.prevented += g.psxg_faced;
        g.avg_len = if lens.is_empty() { 0.0 } else { lens.iter().sum::<f64>() / lens.len() as f64 };
        g.launched_pct = launched as f64 / g.passes.max(1) as f64;
        g.completed_pct = completed as f64 / g.passes.max(1) as f64;
        a.team[s.idx()].gk = g;
    }

    // --- game states
    let mut goals_t: Vec<(f64, Side)> = m.goals_timeline().iter().map(|(s, e)| (e.t, *s)).collect();
    goals_t.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut rows = vec![
        StateRow { label: "SCORE LEVEL".into(), ..Default::default() },
        StateRow { label: format!("{} AHEAD", m.team(Side::Home).short_name.to_uppercase()), ..Default::default() },
        StateRow { label: format!("{} AHEAD", m.team(Side::Away).short_name.to_uppercase()), ..Default::default() },
    ];
    let state_at = |t: f64| -> usize {
        let mut sc = [0, 0];
        for (gt, s) in &goals_t {
            if *gt < t {
                sc[s.idx()] += 1;
            }
        }
        if sc[0] == sc[1] { 0 } else if sc[0] > sc[1] { 1 } else { 2 }
    };
    let mut bounds = vec![0.0];
    bounds.extend(goals_t.iter().map(|g| g.0));
    bounds.push(m.duration());
    for w in bounds.windows(2) {
        let st = state_at((w[0] + w[1]) / 2.0);
        rows[st].minutes += (w[1] - w[0]) / 60.0;
    }
    for p in &poss {
        let st = state_at(p.start_t);
        let si = p.side.idx();
        for &q in &p.shots {
            rows[st].shots[si] += 1;
            rows[st].xg[si] += ev[q].shot.as_ref().unwrap().xg;
        }
        rows[st].entries[si] += p.idx.iter().filter(|&&q| {
            let e = &ev[q];
            (e.kind == Kind::Pass && e.success || e.kind == Kind::Carry) && !in_box(e.x, e.y) && in_box(e.end().0, e.end().1) && e.restart != Some(Restart::Corner)
        }).count();
    }
    for s in Side::both() {
        for tr in &a.team[s.idx()].transitions {
            if let Some(p) = poss.iter().find(|p| p.id == tr.poss) {
                rows[state_at(p.start_t)].transitions[s.idx()] += 1;
            }
        }
    }
    a.states = rows;

    // --- momentum (xT per minute, rolling 8-min triangular window)
    let nmin = dur.ceil() as usize + 1;
    let mut per = [vec![0.0; nmin], vec![0.0; nmin]];
    for (i, e) in ev.iter().enumerate() {
        let v = a.values.xt_added[i];
        if v > 0.0 {
            let mi = ((e.t / 60.0) as usize).min(nmin - 1);
            per[e.side.idx()][mi] += v;
        }
    }
    let mut mom = Momentum::default();
    for mi in 0..nmin {
        let mut acc = [0.0, 0.0];
        let mut wsum = 0.0;
        for k in 0..8i32 {
            let j = mi as i32 - 4 + k;
            if j < 0 || j as usize >= nmin {
                continue;
            }
            let w = 4.0 - (k as f64 - 3.5).abs();
            wsum += w;
            for s in 0..2 {
                acc[s] += per[s][j as usize] * w;
            }
        }
        mom.minutes.push(mi as f64);
        mom.home.push(acc[0] / wsum.max(1.0) * 8.0);
        mom.away.push(acc[1] / wsum.max(1.0) * 8.0);
    }
    // spells: cut where the dominant side flips (min 5 minutes)
    let mut cur: Option<(usize, bool)> = None;
    let mut spells_raw: Vec<(usize, usize, bool)> = vec![];
    for i in 0..nmin {
        let dom = mom.home[i] >= mom.away[i];
        match cur {
            None => cur = Some((i, dom)),
            Some((st, d)) if d != dom && i - st >= 5 => {
                spells_raw.push((st, i, d));
                cur = Some((i, dom));
            }
            _ => {}
        }
    }
    if let Some((st, d)) = cur {
        spells_raw.push((st, nmin.saturating_sub(1).max(st + 1), d));
    }
    // merge consecutive same-dominance spells
    let mut merged: Vec<(usize, usize)> = vec![];
    let mut last_d: Option<bool> = None;
    for (s0, e0, d) in spells_raw {
        if last_d == Some(d) {
            merged.last_mut().unwrap().1 = e0;
        } else {
            merged.push((s0, e0));
            last_d = Some(d);
        }
    }
    for (s0, e0) in merged {
        let h: f64 = per[0][s0..e0].iter().sum();
        let aw: f64 = per[1][s0..e0].iter().sum();
        let xg = |side: Side| ev.iter().filter(|e| e.side == side && e.kind == Kind::Shot && (e.t / 60.0) >= s0 as f64 && (e.t / 60.0) < e0 as f64).map(|e| e.shot.as_ref().unwrap().xg).sum::<f64>();
        mom.spells.push(Spell { start: s0 as f64, end: e0 as f64, share_home: if h + aw > 0.0 { h / (h + aw) } else { 0.5 }, xg_home: xg(Side::Home), xg_away: xg(Side::Away) });
    }
    a.momentum = mom;

    // --- zones 5 x 7 (home attacks up; away flipped)
    let mut zc = vec![vec![[0usize; 2]; 5]; 7];
    for e in ev.iter().filter(|e| e.kind.is_on_ball() && !matches!(e.kind, Kind::Foul)) {
        let (x, y) = if e.side == Side::Home { (e.x, e.y) } else { (105.0 - e.x, 68.0 - e.y) };
        let col = ((68.0 - y) / 68.0 * 5.0).floor().clamp(0.0, 4.0) as usize; // screen left = attacker's left
        let row = (x / 105.0 * 7.0).floor().clamp(0.0, 6.0) as usize;
        zc[row][col][e.side.idx()] += 1;
        a.zone_touches[e.side.idx()] += 1;
    }
    a.zones = zc.iter().map(|r| r.iter().map(|c| if c[0] + c[1] == 0 { 0.0 } else { (c[0] as f64 - c[1] as f64) / (c[0] + c[1]) as f64 * 100.0 }).collect()).collect();

    // --- height series (5-minute windows)
    let nw = (dur / 5.0).ceil() as usize;
    let mut hs = HeightSeries::default();
    for s in Side::both() {
        let mut hv = vec![];
        let mut lv = vec![];
        for w in 0..nw {
            let (t0, t1) = (w as f64 * 300.0, (w + 1) as f64 * 300.0);
            let mut d: Vec<f64> = ev.iter().filter(|e| e.side == s && e.t >= t0 && e.t < t1 && (e.kind.is_def_action() || e.kind == Kind::Recovery)).map(|e| e.x).collect();
            let mut tch: Vec<f64> = ev.iter().filter(|e| e.side == s && e.t >= t0 && e.t < t1 && e.kind.is_on_ball()).map(|e| e.x).collect();
            hv.push(if d.len() >= 2 { Some(d.iter().sum::<f64>() / d.len() as f64) } else { median(&mut d) });
            lv.push(iqr(&mut tch));
        }
        hs.height[s.idx()] = hv;
        hs.length[s.idx()] = lv;
    }
    hs.windows = (0..nw).map(|w| w as f64 * 5.0 + 2.5).collect();
    a.height = hs;

    // --- goal origins
    for (side, e) in m.goals_timeline() {
        let sh = e.shot.as_ref().unwrap();
        if sh.own_goal {
            continue;
        }
        let qi = ev.iter().position(|x| x.id == e.id).unwrap();
        let Some(p) = poss.iter().find(|p| p.shots.contains(&qi)) else { continue };
        let mut chain: Vec<usize> = vec![];
        if p.start_idx < p.idx[0] {
            chain.push(p.start_idx);
        }
        chain.extend(p.idx.iter().copied().filter(|&q| q <= qi && matches!(ev[q].kind, Kind::Pass | Kind::Carry | Kind::TakeOn | Kind::Recovery | Kind::Interception | Kind::Tackle | Kind::Shot | Kind::Claim)));
        // keep last 12 actions for legibility
        if chain.len() > 12 {
            chain = chain[chain.len() - 12..].to_vec();
        }
        let passes = chain.iter().filter(|&&q| ev[q].kind == Kind::Pass).count();
        let players: HashSet<u64> = chain.iter().filter_map(|&q| ev[q].player).collect();
        let secs = e.t - p.start_t;
        let assist = e.related.and_then(|r| ev.iter().find(|x| x.id == r)).and_then(|x| x.player);
        let first_long = chain.iter().find(|&&q| ev[q].kind == Kind::Pass).map(|&q| ev[q].pass.as_ref().map(|x| x.long).unwrap_or(false)).unwrap_or(false);
        let label = if sh.penalty {
            "PENALTY"
        } else if p.start_type.is_set_piece() {
            "SET PIECE"
        } else if matches!(p.start_type, StartType::Recovery | StartType::Interception | StartType::Tackle) && p.start_x >= 52.5 && secs <= 20.0 {
            "QUICK ATTACK AFTER A REGAIN"
        } else if matches!(p.start_type, StartType::Recovery | StartType::Interception | StartType::Tackle | StartType::Keeper) && secs <= 15.0 {
            "COUNTER-ATTACK"
        } else if first_long && passes <= 3 {
            "LONG BALL"
        } else if p.start_x < 35.0 && passes >= 4 {
            "BUILD-UP FROM THE BACK"
        } else if p.start_type == StartType::ThrowIn {
            "FROM A THROW-IN"
        } else {
            "SUSTAINED ATTACK"
        };
        a.goals.push(GoalOrigin { shot: qi, side, chain, passes, secs, players: players.len().max(1), start: p.start_type.label().to_string(), label: label.into(), assist, xg: sh.xg, body: sh.body });
    }

    // --- players
    let mut opp_touch_min: [Vec<f64>; 2] = [vec![], vec![]];
    for e in ev.iter().filter(|e| e.kind.is_on_ball()) {
        opp_touch_min[e.side.idx()].push(e.t);
    }
    let shot_xg = |q: usize| ev[q].shot.as_ref().map(|s| s.xg).unwrap_or(0.0);
    for s in Side::both() {
        for p in &m.team(s).players {
            let mins = player_minutes(m, p);
            if mins <= 0.0 {
                continue;
            }
            let mut ps = PlayerStats { id: p.id, side: Some(s), name: p.name.clone(), short: p.short.clone(), shirt: p.shirt, role: Some(p.role), minutes: mins, ..Default::default() };
            let (mut sx, mut sy, mut nt) = (0.0, 0.0, 0.0);
            let mut dx = vec![];
            for (i, e) in ev.iter().enumerate() {
                if e.player == Some(p.id) && e.side == s {
                    if e.kind.is_on_ball() && !matches!(e.kind, Kind::Foul) {
                        ps.touches += 1;
                        ps.touches_by_third[if e.x < 35.0 { 0 } else if e.x < 70.0 { 1 } else { 2 }] += 1;
                        sx += e.x;
                        sy += e.y;
                        nt += 1.0;
                    }
                    ps.vaep_off += a.values.vaep_off[i];
                    ps.vaep_def += a.values.vaep_def[i];
                    if a.values.xt_added[i] > 0.0 {
                        ps.positive_xt += a.values.xt_added[i];
                    }
                    match e.kind {
                        Kind::Pass => {
                            ps.passes += 1;
                            if e.success {
                                ps.passes_completed += 1;
                                ps.progression_m += (e.end().0 - e.x).max(0.0);
                            }
                            if is_prog(e) {
                                ps.progressive_passes += 1;
                            }
                            if is_line_break(e) && e.success {
                                ps.line_breaks += 1;
                            }
                            if let Some(pi) = &e.pass {
                                if pi.key_pass {
                                    ps.key_passes += 1;
                                }
                                if pi.assist {
                                    ps.assists += 1;
                                }
                            }
                        }
                        Kind::Carry => {
                            ps.progression_m += (e.end().0 - e.x).max(0.0);
                            if is_prog(e) {
                                ps.progressive_carries += 1;
                            }
                        }
                        Kind::Shot => {
                            if !e.shot.as_ref().unwrap().own_goal {
                                ps.shots += 1;
                                ps.xg += shot_xg(i);
                                if e.shot.as_ref().unwrap().outcome == ShotOutcome::Goal {
                                    ps.goals += 1;
                                }
                            }
                        }
                        Kind::Recovery => ps.recoveries += 1,
                        Kind::Interception => ps.interceptions += 1,
                        Kind::Tackle if e.success => ps.tackles_won += 1,
                        Kind::Challenge => ps.dribbled_past += 1,
                        Kind::Aerial => {
                            ps.aerials.1 += 1;
                            if e.success {
                                ps.aerials.0 += 1;
                            }
                        }
                        Kind::TakeOn => {
                            ps.take_ons.1 += 1;
                            if e.success {
                                ps.take_ons.0 += 1;
                            }
                        }
                        Kind::Dispossessed => ps.dispossessed += 1,
                        Kind::Clearance => ps.clearances += 1,
                        Kind::Save => ps.saves += 1,
                        Kind::Claim => ps.claims += 1,
                        Kind::Sweeper => ps.sweeps += 1,
                        Kind::Error => ps.errors_under_pressure.push(e.t / 60.0),
                        _ => {}
                    }
                    if matches!(e.kind, Kind::Tackle | Kind::Interception | Kind::Recovery | Kind::Clearance | Kind::Challenge | Kind::Block) {
                        ps.def_actions += 1;
                        dx.push(e.x);
                    }
                    if (e.kind == Kind::Pass && e.success || e.kind == Kind::Carry) && !in_box(e.x, e.y) && in_box(e.end().0, e.end().1) {
                        ps.box_entries += 1;
                    }
                }
                if receiver[i] == Some(p.id) && e.side == s && e.end().0 >= 70.0 {
                    ps.ft_receptions += 1;
                }
            }
            ps.avg_x = if nt > 0.0 { sx / nt } else { 52.5 };
            ps.avg_y = if nt > 0.0 { sy / nt } else { 34.0 };
            ps.def_height = if dx.is_empty() { 0.0 } else { dx.iter().sum::<f64>() / dx.len() as f64 };
            ps.prog_pass_share = if ps.passes_completed > 0 { ps.progressive_passes as f64 / ps.passes_completed as f64 } else { 0.0 };
            // PAdj: per 100 opponent touches while on the pitch
            let on = if p.on_t.is_nan() { 0.0 } else { p.on_t };
            let off = p.off_t.unwrap_or(f64::INFINITY);
            let ot = opp_touch_min[s.other().idx()].iter().filter(|&&t| t >= on && t <= off).count();
            ps.padj_def = if ot > 0 { ps.def_actions as f64 / ot as f64 * 100.0 } else { 0.0 };
            a.players.push(ps);
        }
    }
    // xA, xGChain, xGBuildup
    for p in &poss {
        if p.shots.is_empty() {
            continue;
        }
        let xg: f64 = p.shots.iter().map(|&q| shot_xg(q)).sum();
        let shooters: HashSet<u64> = p.shots.iter().filter_map(|&q| ev[q].player).collect();
        let mut kp_players: HashSet<u64> = HashSet::new();
        for &q in &p.shots {
            if let Some(r) = ev[q].related {
                if let Some(k) = p.idx.iter().find(|&&j| ev[j].id == r) {
                    if ev[q].t - ev[*k].t <= 15.0 {
                        if let Some(pl) = ev[*k].player {
                            kp_players.insert(pl);
                            if let Some(ps) = a.players.iter_mut().find(|x| x.id == pl) {
                                ps.inferred_xa += shot_xg(q);
                            }
                        }
                    }
                }
            }
        }
        let involved: HashSet<u64> = p.idx.iter().filter_map(|&q| ev[q].player).collect();
        for pl in involved {
            if let Some(ps) = a.players.iter_mut().find(|x| x.id == pl) {
                ps.xgchain += xg;
                if !shooters.contains(&pl) && !kp_players.contains(&pl) {
                    ps.xgbuildup += xg;
                }
            }
        }
    }
    for ps in a.players.iter_mut() {
        ps.xa_per100 = if ps.passes > 0 { ps.inferred_xa / ps.passes as f64 * 100.0 } else { 0.0 };
    }

    // --- quality gates
    for s in Side::both() {
        let t = &a.team[s.idx()];
        let part = t.on_target + t.blocked + t.off_target + t.woodwork;
        a.checks.push(Check { name: format!("{} shot partition", t.name), ok: part == t.shots.len(), detail: format!("on target {} + blocked {} + off {} + woodwork {} = {} (shots {})", t.on_target, t.blocked, t.off_target, t.woodwork, part, t.shots.len()) });
        let lane_shots: usize = t.entries.iter().filter(|e| e.shot_followed).count();
        a.checks.push(Check { name: format!("{} box-entry shot count single-sourced", t.name), ok: true, detail: format!("{} of {} entries followed by a shot", lane_shots, t.entries.len()) });
        let fun_xg: f64 = poss.iter().filter(|p| p.side == s).flat_map(|p| p.shots.iter()).map(|&q| shot_xg(q)).sum();
        a.checks.push(Check { name: format!("{} xG reconciles (shots vs possessions)", t.name), ok: (fun_xg - t.xg).abs() < 0.02, detail: format!("shot list {:.3} vs possession sum {:.3}", t.xg, fun_xg) });
        let ext: Vec<(f64, f64)> = t.shots.iter().filter_map(|&q| ev[q].shot.as_ref().and_then(|s| s.xg_ext.map(|x| (x, s.xg)))).collect();
        if !ext.is_empty() {
            let big = ext.iter().filter(|(a, b)| (a - b).abs() > 0.15).count();
            a.checks.push(Check { name: format!("{} external vs in-house xG", t.name), ok: true, detail: format!("{} of {} shots differ by > 0.15 (ext total {:.2}, in-house {:.2})", big, ext.len(), ext.iter().map(|x| x.0).sum::<f64>(), ext.iter().map(|x| x.1).sum::<f64>()) });
        }
        let gk_partition = t.gk.off_target + t.gk.woodwork + t.gk.blocked + t.gk.on_target;
        a.checks.push(Check { name: format!("{} keeper shots faced partition", t.name), ok: gk_partition == t.gk.faced_total, detail: format!("{gk_partition} vs {}", t.gk.faced_total) });
    }
    a.possessions = poss;
    a.receiver = receiver;
    a
}

pub fn player_minutes(m: &Match, p: &Player) -> f64 {
    if p.on_t.is_nan() {
        return 0.0;
    }
    let end = p.off_t.unwrap_or(m.duration());
    ((end - p.on_t) / 60.0).max(0.0)
}
