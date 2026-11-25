//! In-house models: xG, local-placement PSxG, xT and VAEP.
//! Trained on StatsBomb Open Data (`matchhub train`), applied to any canonical match.

pub mod logistic;

use crate::derive::possession::{self, StartType};
use crate::model::*;
use logistic::Logistic;
use serde::{Deserialize, Serialize};

pub const XT_NX: usize = 16;
pub const XT_NY: usize = 12;
pub const PENALTY_XG: f64 = 0.76;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Models {
    pub version: String,
    pub trained_on: String,
    pub training_matches: usize,
    pub xg: Logistic,
    pub psxg: Logistic,
    pub xt: Vec<f64>,
    pub vaep_score: Logistic,
    pub vaep_concede: Logistic,
}

/// Per-event model outputs aligned with m.events.
#[derive(Debug, Clone, Default)]
pub struct Values {
    pub xt_added: Vec<f64>,
    pub vaep_off: Vec<f64>,
    pub vaep_def: Vec<f64>,
}

fn angle_dist(x: f64, y: f64) -> (f64, f64) {
    let dx = GOAL_X - x;
    let dy = (y - GOAL_Y).abs();
    let dist = (dx * dx + dy * dy).sqrt();
    let half = 3.66;
    let mut a = (7.32 * dx / (dx * dx + dy * dy - half * half)).atan();
    if a < 0.0 {
        a += std::f64::consts::PI;
    }
    (dist, a)
}

pub const XG_FEATURES: &[&str] = &[
    "dist", "dist2", "logdist", "angle", "angle2", "header", "other_body", "first_time", "assist_cross", "assist_through", "assist_cutback", "direct_fk", "set_piece", "fast_break",
];

pub fn xg_features(e: &Event, set_piece: bool) -> Vec<f64> {
    let s = e.shot.as_ref().unwrap();
    let (d, a) = angle_dist(e.x, e.y);
    vec![
        d,
        d * d / 100.0,
        (d + 0.5).ln(),
        a,
        a * a,
        (s.body == Body::Head) as u8 as f64,
        (s.body == Body::Other) as u8 as f64,
        s.first_time as u8 as f64,
        s.assist_cross as u8 as f64,
        s.assist_through as u8 as f64,
        s.assist_cutback as u8 as f64,
        s.direct_free_kick as u8 as f64,
        set_piece as u8 as f64,
        s.fast_break as u8 as f64,
    ]
}

pub const PSXG_FEATURES: &[&str] = &["logit_xg", "abs_u", "z", "abs_u_z", "u2", "z2", "corner"];

pub fn psxg_features(xg: f64, u: f64, z: f64) -> Vec<f64> {
    let p = xg.clamp(0.005, 0.995);
    let au = u.abs().min(1.2);
    let z = z.clamp(0.0, 1.2);
    vec![(p / (1.0 - p)).ln(), au, z, au * z, au * au, z * z, ((au > 0.6) && (z > 0.55)) as u8 as f64]
}

fn set_piece_shots(m: &Match) -> std::collections::HashSet<usize> {
    let mut s = std::collections::HashSet::new();
    for p in possession::build(m) {
        if matches!(p.start_type, StartType::Corner | StartType::FreeKick) {
            for &q in &p.shots {
                s.insert(q);
            }
        }
    }
    s
}

fn on_target(o: ShotOutcome) -> bool {
    matches!(o, ShotOutcome::Goal | ShotOutcome::Saved)
}

fn xt_cell(x: f64, y: f64) -> usize {
    let i = ((x / PITCH_L) * XT_NX as f64).floor().clamp(0.0, (XT_NX - 1) as f64) as usize;
    let j = ((y / PITCH_W) * XT_NY as f64).floor().clamp(0.0, (XT_NY - 1) as f64) as usize;
    i * XT_NY + j
}

impl Models {
    pub fn xt_at(&self, x: f64, y: f64) -> f64 {
        if self.xt.is_empty() {
            return 0.0;
        }
        self.xt[xt_cell(x, y)]
    }

    pub fn train(matches: &[Match]) -> Models {
        // ---- xG
        let mut xs = vec![];
        let mut ys = vec![];
        for m in matches {
            let sp = set_piece_shots(m);
            for (i, e) in m.events.iter().enumerate() {
                if let Some(s) = &e.shot {
                    if s.penalty || s.own_goal {
                        continue;
                    }
                    xs.push(xg_features(e, sp.contains(&i)));
                    ys.push((s.outcome == ShotOutcome::Goal) as u8 as f64);
                }
            }
        }
        eprintln!("  xG: {} shots", xs.len());
        let xg = Logistic::fit(XG_FEATURES, &xs, &ys, 1e-3, 600);
        eprintln!("  xG log-loss {:.4} (base rate {:.3})", xg.log_loss, xg.base_rate);

        // ---- PSxG (local placement)
        let (mut px, mut py) = (vec![], vec![]);
        for m in matches {
            let sp = set_piece_shots(m);
            for (i, e) in m.events.iter().enumerate() {
                if let Some(s) = &e.shot {
                    if s.penalty || s.own_goal || !on_target(s.outcome) {
                        continue;
                    }
                    if let (Some(u), Some(z)) = (s.goal_u, s.goal_z) {
                        let x0 = xg.predict(&xg_features(e, sp.contains(&i)));
                        px.push(psxg_features(x0, u, z));
                        py.push((s.outcome == ShotOutcome::Goal) as u8 as f64);
                    }
                }
            }
        }
        eprintln!("  PSxG: {} on-target shots with placement", px.len());
        let psxg = Logistic::fit(PSXG_FEATURES, &px, &py, 1e-3, 600);
        eprintln!("  PSxG log-loss {:.4} (base rate {:.3})", psxg.log_loss, psxg.base_rate);

        // ---- xT
        let n = XT_NX * XT_NY;
        let (mut shots, mut goals, mut moves) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        let mut trans = vec![vec![0.0; n]; n];
        for m in matches {
            for e in &m.events {
                match e.kind {
                    Kind::Shot => {
                        let s = e.shot.as_ref().unwrap();
                        if s.penalty || s.own_goal {
                            continue;
                        }
                        let c = xt_cell(e.x, e.y);
                        shots[c] += 1.0;
                        if s.outcome == ShotOutcome::Goal {
                            goals[c] += 1.0;
                        }
                    }
                    Kind::Pass | Kind::Carry if e.restart.is_none() => {
                        let c = xt_cell(e.x, e.y);
                        moves[c] += 1.0;
                        if e.success {
                            let (ex, ey) = e.end();
                            trans[c][xt_cell(ex, ey)] += 1.0;
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut xt = vec![0.0; n];
        for _ in 0..12 {
            let mut nxt = vec![0.0; n];
            for c in 0..n {
                let tot = shots[c] + moves[c];
                if tot == 0.0 {
                    continue;
                }
                let ps = shots[c] / tot;
                let pm = moves[c] / tot;
                let g = if shots[c] > 0.0 { goals[c] / shots[c] } else { 0.0 };
                let mut mv = 0.0;
                if moves[c] > 0.0 {
                    for d in 0..n {
                        if trans[c][d] > 0.0 {
                            mv += trans[c][d] / moves[c] * xt[d];
                        }
                    }
                }
                nxt[c] = ps * g + pm * mv;
            }
            xt = nxt;
        }

        // ---- VAEP
        let mut vx = vec![];
        let (mut vs, mut vc) = (vec![], vec![]);
        for m in matches {
            let acts = vaep_actions(m);
            for k in 0..acts.len() {
                if k % 2 == 1 && acts.len() > 2000 {
                    // subsample for speed on long matches
                    continue;
                }
                let (f, s, c) = vaep_row(m, &acts, k, true);
                vx.push(f);
                vs.push(s);
                vc.push(c);
            }
        }
        eprintln!("  VAEP: {} actions", vx.len());
        let names: Vec<String> = (0..vx.first().map(|r| r.len()).unwrap_or(0)).map(|i| format!("f{i}")).collect();
        let nref: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let vaep_score = Logistic::fit(&nref, &vx, &vs, 1e-4, 250);
        let vaep_concede = Logistic::fit(&nref, &vx, &vc, 1e-4, 250);
        eprintln!("  VAEP log-loss score {:.4} concede {:.4}", vaep_score.log_loss, vaep_concede.log_loss);

        Models {
            version: "1.0.0".into(),
            trained_on: "StatsBomb Open Data".into(),
            training_matches: matches.len(),
            xg,
            psxg,
            xt,
            vaep_score,
            vaep_concede,
        }
    }

    /// Fill shot xG / PSxG and return per-event xT and VAEP values.
    pub fn apply(&self, m: &mut Match) -> Values {
        let sp = set_piece_shots(m);
        let n = m.events.len();
        for i in 0..n {
            let is_sp = sp.contains(&i);
            let e = &m.events[i];
            if e.kind != Kind::Shot {
                continue;
            }
            let s = e.shot.as_ref().unwrap();
            let xg = if s.own_goal {
                0.0
            } else if s.penalty {
                PENALTY_XG
            } else {
                self.xg.predict(&xg_features(e, is_sp))
            };
            let ps = if on_target(s.outcome) && !s.own_goal {
                if s.penalty {
                    Some(match (s.goal_u, s.goal_z) {
                        (Some(u), Some(z)) => self.psxg.predict(&psxg_features(0.76, u, z)).max(0.5),
                        _ => 0.76,
                    })
                } else {
                    let u = s.goal_u.unwrap_or(0.5);
                    let z = s.goal_z.unwrap_or(0.3);
                    Some(self.psxg.predict(&psxg_features(xg, u, z)))
                }
            } else if matches!(s.outcome, ShotOutcome::Woodwork) {
                None
            } else {
                None
            };
            let sm = m.events[i].shot.as_mut().unwrap();
            sm.xg = xg;
            sm.psxg = ps;
        }
        let mut v = Values { xt_added: vec![0.0; n], vaep_off: vec![0.0; n], vaep_def: vec![0.0; n] };
        for (i, e) in m.events.iter().enumerate() {
            if matches!(e.kind, Kind::Pass | Kind::Carry) && e.success && e.restart != Some(Restart::Corner) {
                let (ex, ey) = e.end();
                v.xt_added[i] = self.xt_at(ex, ey) - self.xt_at(e.x, e.y);
            }
        }
        let acts = vaep_actions(m);
        let mut ps = vec![0.0; acts.len()];
        let mut pc = vec![0.0; acts.len()];
        for k in 0..acts.len() {
            let (f, _, _) = vaep_row(m, &acts, k, false);
            ps[k] = self.vaep_score.predict(&f);
            pc[k] = self.vaep_concede.predict(&f);
        }
        for k in 0..acts.len() {
            let i = acts[k];
            let e = &m.events[i];
            let (prev_s, prev_c) = if k == 0 || m.events[acts[k - 1]].period != e.period || is_goal(&m.events[acts[k - 1]]) {
                (self.vaep_score.base_rate, self.vaep_concede.base_rate)
            } else if m.events[acts[k - 1]].side == e.side {
                (ps[k - 1], pc[k - 1])
            } else {
                (pc[k - 1], ps[k - 1])
            };
            let mut off = ps[k] - prev_s;
            let def = -(pc[k] - prev_c);
            if is_goal(e) {
                off = 1.0 - prev_s;
            }
            v.vaep_off[i] = off;
            v.vaep_def[i] = def;
        }
        v
    }
}

fn is_goal(e: &Event) -> bool {
    e.shot.as_ref().map(|s| s.outcome == ShotOutcome::Goal).unwrap_or(false)
}

const VAEP_KINDS: &[Kind] = &[
    Kind::Pass,
    Kind::Carry,
    Kind::TakeOn,
    Kind::Shot,
    Kind::Tackle,
    Kind::Interception,
    Kind::Recovery,
    Kind::Clearance,
    Kind::Block,
    Kind::Foul,
    Kind::Save,
    Kind::Claim,
    Kind::Sweeper,
    Kind::BadTouch,
    Kind::Dispossessed,
];

pub fn vaep_actions(m: &Match) -> Vec<usize> {
    m.events.iter().enumerate().filter(|(_, e)| VAEP_KINDS.contains(&e.kind) && e.period <= 4).map(|(i, _)| i).collect()
}

fn action_feats(e: &Event, flip: bool, out: &mut Vec<f64>) {
    for k in VAEP_KINDS {
        out.push((e.kind == *k) as u8 as f64);
    }
    let (mut x, mut y) = (e.x, e.y);
    let (mut ex, mut ey) = e.end();
    if flip {
        x = PITCH_L - x;
        y = PITCH_W - y;
        ex = PITCH_L - ex;
        ey = PITCH_W - ey;
    }
    let (d, a) = angle_dist(x, y);
    let (ed, _) = angle_dist(ex, ey);
    out.extend_from_slice(&[e.success as u8 as f64, x / 105.0, (y - 34.0).abs() / 34.0, ex / 105.0, (ey - 34.0).abs() / 34.0, d / 50.0, ed / 50.0, a, (ex - x) / 50.0]);
    let head = e.shot.as_ref().map(|s| s.body == Body::Head).unwrap_or(false) || e.pass.as_ref().map(|p| p.head).unwrap_or(false);
    out.push(head as u8 as f64);
    out.push(e.restart.is_some() as u8 as f64);
}

/// Features for action k plus the two previous actions, and labels.
fn vaep_row(m: &Match, acts: &[usize], k: usize, labels: bool) -> (Vec<f64>, f64, f64) {
    let e = &m.events[acts[k]];
    let mut f = Vec::with_capacity(90);
    action_feats(e, false, &mut f);
    for back in 1..=2 {
        if k >= back {
            let p = &m.events[acts[k - back]];
            action_feats(p, p.side != e.side, &mut f);
            f.push((p.side == e.side) as u8 as f64);
            f.push(((e.t - p.t).clamp(0.0, 30.0)) / 30.0);
        } else {
            action_feats(e, false, &mut f);
            f.push(1.0);
            f.push(0.0);
        }
    }
    let (mut s, mut c) = (0.0, 0.0);
    if labels {
        for j in k..(k + 10).min(acts.len()) {
            let a = &m.events[acts[j]];
            if a.period != e.period {
                break;
            }
            if let Some(sh) = &a.shot {
                if sh.outcome == ShotOutcome::Goal {
                    let scorer = if sh.own_goal { a.side.other() } else { a.side };
                    if scorer == e.side {
                        s = 1.0
                    } else {
                        c = 1.0
                    }
                    break;
                }
            }
        }
    }
    (f, s, c)
}
