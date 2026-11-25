//! Possession and sequence assignment plus possession summaries.

use crate::model::*;
use serde::Serialize;

pub fn assign(m: &mut Match) {
    let mut cur: Option<Side> = None;
    let mut pid = 0u32;
    let mut sid = 0u32;
    let mut last_period = 0u8;
    for e in m.events.iter_mut() {
        if !e.kind.is_on_ball() {
            e.possession = pid;
            e.sequence = sid;
            continue;
        }
        if e.period != last_period {
            last_period = e.period;
            pid += 1;
            sid += 1;
            cur = None;
        }
        if e.restart.is_some() {
            sid += 1;
            if e.restart == Some(Restart::KickOff) && cur != Some(e.side) {
                pid += 1;
                cur = Some(e.side);
            }
        }
        if e.kind.controls_ball() && cur != Some(e.side) {
            pid += 1;
            sid += 1;
            cur = Some(e.side);
        }
        e.possession = pid;
        e.sequence = sid;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum StartType {
    Recovery,
    Interception,
    Tackle,
    ThrowIn,
    GoalKick,
    Corner,
    FreeKick,
    KickOff,
    Keeper,
    Other,
}
impl StartType {
    pub fn label(self) -> &'static str {
        match self {
            StartType::Recovery => "recovery",
            StartType::Interception => "interception",
            StartType::Tackle => "tackle",
            StartType::ThrowIn => "throw-in",
            StartType::GoalKick => "goal kick",
            StartType::Corner => "corner",
            StartType::FreeKick => "free kick",
            StartType::KickOff => "kick-off",
            StartType::Keeper => "keeper",
            StartType::Other => "loose ball",
        }
    }
    pub fn is_set_piece(self) -> bool {
        matches!(self, StartType::Corner | StartType::FreeKick)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Possession {
    pub id: u32,
    pub side: Side,
    pub start_type: StartType,
    pub start_t: f64,
    pub end_t: f64,
    /// Indices into m.events of the owning side's events in this possession.
    pub idx: Vec<usize>,
    /// Index of the ball-win / restart event that started it (may belong to idx or precede it).
    pub start_idx: usize,
    pub start_x: f64,
    /// Score difference (own - opponent) at possession start.
    pub score_diff: i32,
    pub reached_final_third: bool,
    pub touched_box: bool,
    pub shot_from_box: bool,
    pub shot_on_target_from_box: bool,
    pub shots: Vec<usize>,
    pub max_x: f64,
}

pub fn build(m: &Match) -> Vec<Possession> {
    let mut out: Vec<Possession> = vec![];
    let mut score = [0i32; 2];
    let mut i = 0;
    let ev = &m.events;
    while i < ev.len() {
        let e = &ev[i];
        if !e.kind.is_on_ball() || e.possession == 0 {
            i += 1;
            continue;
        }
        let pid = e.possession;
        // owning side = side of first controlling event in this possession id
        let mut j = i;
        let mut side = None;
        while j < ev.len() && ev[j].possession == pid {
            if ev[j].kind.controls_ball() || ev[j].restart.is_some() {
                side = Some(ev[j].side);
                break;
            }
            j += 1;
        }
        let Some(side) = side else {
            i += 1;
            continue;
        };
        let mut idx = vec![];
        let mut k = i;
        while k < ev.len() && (ev[k].possession == pid || !ev[k].kind.is_on_ball()) {
            if ev[k].possession == pid && ev[k].side == side && ev[k].kind.is_on_ball() {
                idx.push(k);
            }
            k += 1;
        }
        if idx.is_empty() {
            i = k.max(i + 1);
            continue;
        }
        // Start type: restart on first own event, else the ball-winning action just before.
        let first = idx[0];
        let mut start_idx = first;
        let st = if let Some(r) = ev[first].restart {
            match r {
                Restart::Corner => StartType::Corner,
                Restart::FreeKick | Restart::Penalty => StartType::FreeKick,
                Restart::ThrowIn => StartType::ThrowIn,
                Restart::GoalKick => StartType::GoalKick,
                Restart::KickOff => StartType::KickOff,
            }
        } else {
            let mut found = None;
            for b in (first.saturating_sub(6)..=first).rev() {
                let x = &ev[b];
                if x.side != side || ev[first].t - x.t > 5.0 {
                    continue;
                }
                let t = match x.kind {
                    Kind::Recovery => Some(StartType::Recovery),
                    Kind::Interception => Some(StartType::Interception),
                    Kind::Tackle if x.success => Some(StartType::Tackle),
                    Kind::Claim | Kind::Save | Kind::Sweeper => Some(StartType::Keeper),
                    _ => None,
                };
                if let Some(t) = t {
                    found = Some((t, b));
                    break;
                }
            }
            if let Some((t, b)) = found {
                start_idx = b;
                t
            } else {
                StartType::Other
            }
        };
        let diff = score[side.idx()] - score[side.other().idx()];
        let mut p = Possession {
            id: pid,
            side,
            start_type: st,
            start_t: ev[start_idx].t,
            end_t: ev[*idx.last().unwrap()].t,
            start_x: ev[start_idx].x,
            idx: idx.clone(),
            start_idx,
            score_diff: diff,
            reached_final_third: false,
            touched_box: false,
            shot_from_box: false,
            shot_on_target_from_box: false,
            shots: vec![],
            max_x: 0.0,
        };
        for &q in &idx {
            let x = &ev[q];
            let (ex, ey) = if x.kind == Kind::Pass && x.success || x.kind == Kind::Carry { x.end() } else { (x.x, x.y) };
            p.max_x = p.max_x.max(x.x).max(ex);
            if x.x >= 70.0 || ex >= 70.0 && (x.success || x.kind == Kind::Carry) {
                p.reached_final_third = true;
            }
            if super::in_box(x.x, x.y) || ((x.kind == Kind::Pass && x.success) || x.kind == Kind::Carry) && super::in_box(ex, ey) {
                p.touched_box = true;
            }
            if x.kind == Kind::Shot && !x.shot.as_ref().unwrap().own_goal {
                p.shots.push(q);
                if super::in_box(x.x, x.y) {
                    p.shot_from_box = true;
                    if matches!(x.shot.as_ref().unwrap().outcome, ShotOutcome::Goal | ShotOutcome::Saved) {
                        p.shot_on_target_from_box = true;
                    }
                }
            }
        }
        out.push(p);
        // Update score after processing goals in this span (any side).
        for q in i..k {
            if let Some(s) = &ev[q].shot {
                if s.outcome == ShotOutcome::Goal && ev[q].period <= 4 {
                    let gs = if s.own_goal { ev[q].side.other() } else { ev[q].side };
                    score[gs.idx()] += 1;
                }
            }
        }
        i = k.max(i + 1);
    }
    out
}
