pub mod enrich;
pub mod statsbomb;
pub mod whoscored;

use crate::model::*;

pub fn in_box(x: f64, y: f64) -> bool {
    x >= 88.5 && (13.84..=54.16).contains(&y)
}

/// Source-independent clean-up shared by every loader.
pub fn finalize(m: &mut Match, has_carries: bool) {
    // Stable chronological order.
    m.events.sort_by(|a, b| (a.period, a.t).partial_cmp(&(b.period, b.t)).unwrap());

    // Periods.
    let mut periods: Vec<PeriodInfo> = vec![];
    for e in &m.events {
        if let Some(p) = periods.iter_mut().find(|p| p.period == e.period) {
            p.start_t = p.start_t.min(e.t);
            p.end_t = p.end_t.max(e.t);
        } else {
            periods.push(PeriodInfo { period: e.period, start_t: e.t, end_t: e.t });
        }
    }
    periods.sort_by_key(|p| p.period);
    m.periods = periods;

    // Key passes, assists and assist types from shot links.
    let ids: std::collections::HashMap<u64, usize> = m.events.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
    let n = m.events.len();
    for i in 0..n {
        if m.events[i].kind != Kind::Shot {
            continue;
        }
        let side = m.events[i].side;
        let t = m.events[i].t;
        let mut kp = m.events[i].related.and_then(|r| ids.get(&r).copied()).filter(|&j| m.events[j].kind == Kind::Pass && m.events[j].side == side);
        if kp.is_none() {
            // Fallback: last completed pass by the same side within 6 s ending near the shot.
            let (sx, sy) = (m.events[i].x, m.events[i].y);
            for j in (0..i).rev() {
                let e = &m.events[j];
                if t - e.t > 6.0 {
                    break;
                }
                if e.side == side && e.kind == Kind::Pass && e.success {
                    let (ex, ey) = e.end();
                    if ((ex - sx).powi(2) + (ey - sy).powi(2)).sqrt() < 8.0 {
                        kp = Some(j);
                    }
                    break;
                }
                if e.side != side && e.kind.controls_ball() {
                    break;
                }
            }
        }
        if let Some(j) = kp {
            let goal = m.events[i].shot.as_ref().map(|s| s.outcome == ShotOutcome::Goal && !s.own_goal).unwrap_or(false);
            let (cross, through, mut cut) = {
                let p = m.events[j].pass.get_or_insert_with(PassInfo::default);
                p.key_pass = true;
                if goal {
                    p.assist = true;
                }
                (p.cross, p.through, p.cutback)
            };
            let pe = &m.events[j];
            if !cut && is_cutback(pe) {
                cut = true;
            }
            if let Some(p) = m.events[j].pass.as_mut() {
                p.cutback = cut;
            }
            m.events[i].related = Some(m.events[j].id);
            if let Some(s) = m.events[i].shot.as_mut() {
                s.assist_cross = cross;
                s.assist_through = through;
                s.assist_cutback = cut;
            }
        }
    }
    for e in m.events.iter_mut() {
        if e.kind == Kind::Pass && e.success && is_cutback(e) {
            if let Some(p) = e.pass.as_mut() {
                p.cutback = true;
            }
        }
    }

    if !has_carries {
        infer_carries(m);
    }
    crate::derive::possession::assign(m);
}

fn is_cutback(e: &Event) -> bool {
    let (ex, ey) = e.end();
    e.kind == Kind::Pass && e.x >= 96.0 && (e.y < 24.85 || e.y > 43.15) && e.x <= 105.0 && ex < e.x - 2.0 && in_box(ex, ey) && (ey - 34.0).abs() < 12.0 && !e.pass.as_ref().map(|p| p.cross && ex > 99.0).unwrap_or(false)
}

/// Insert carries between consecutive same-team actions (socceraction-style),
/// for sources that do not record them (WhoScored/Opta).
fn infer_carries(m: &mut Match) {
    let mut out = Vec::with_capacity(m.events.len() * 2);
    let mut next = 10_000_000u64;
    let evs = std::mem::take(&mut m.events);
    for i in 0..evs.len() {
        if i > 0 {
            let a = &evs[i - 1];
            let b = &evs[i];
            let a_ends_with_ball = matches!(a.kind, Kind::Recovery | Kind::Interception | Kind::Claim) || (a.kind == Kind::Pass && a.success) || (a.kind == Kind::TakeOn && a.success);
            let b_on_ball = matches!(b.kind, Kind::Pass | Kind::Shot | Kind::TakeOn | Kind::Dispossessed | Kind::BadTouch);
            if a_ends_with_ball && b_on_ball && a.side == b.side && a.period == b.period && b.restart.is_none() {
                let (sx, sy) = if a.kind == Kind::Pass { a.end() } else { (a.x, a.y) };
                let d = ((b.x - sx).powi(2) + (b.y - sy).powi(2)).sqrt();
                let dt = b.t - a.t;
                if (3.0..=60.0).contains(&d) && dt > 0.0 && dt <= 10.0 {
                    out.push(Event {
                        id: next,
                        period: b.period,
                        minute: b.minute,
                        second: b.second,
                        t: (a.t + b.t) / 2.0,
                        side: b.side,
                        player: b.player,
                        kind: Kind::Carry,
                        success: true,
                        x: sx,
                        y: sy,
                        end_x: Some(b.x),
                        end_y: Some(b.y),
                        restart: None,
                        pass: None,
                        shot: None,
                        handling: None,
                        inferred: true,
                        related: None,
                        possession: 0,
                        sequence: 0,
                        related_player: None,
                    });
                    next += 1;
                }
            }
        }
        out.push(evs[i].clone());
    }
    m.events = out;
}
