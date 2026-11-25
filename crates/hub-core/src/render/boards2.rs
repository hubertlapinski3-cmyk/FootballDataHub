//! Boards 11-20: defensive activity, high regains, high turnovers, PPDA timeline,
//! momentum, game state, VAEP, dominating zones, transitions, match statistics.

use super::boards1::{side_tag, H, W};
use super::frame::*;
use super::pitch::*;
use super::svg::*;
use super::Board;
use crate::derive::metrics::*;
use crate::model::*;
use crate::theme;

/// Gaussian KDE heat layer in pitch metres.
pub fn kde(s: &mut Svg, p: &Pitch, pts: &[(f64, f64)], col: &str, sigma: f64, max_op: f64) {
    if pts.is_empty() {
        return;
    }
    let (nx, ny) = (42usize, 28usize);
    let mut g = vec![0.0; nx * ny];
    for i in 0..nx {
        for j in 0..ny {
            let x = (i as f64 + 0.5) / nx as f64 * 105.0;
            let y = (j as f64 + 0.5) / ny as f64 * 68.0;
            g[i * ny + j] = pts.iter().map(|(px, py)| (-((x - px).powi(2) + (y - py).powi(2)) / (2.0 * sigma * sigma)).exp()).sum();
        }
    }
    let mx = g.iter().cloned().fold(1e-9, f64::max);
    for i in 0..nx {
        for j in 0..ny {
            let v = g[i * ny + j] / mx;
            if v < 0.08 {
                continue;
            }
            let x0 = i as f64 / nx as f64 * 105.0;
            let y0 = j as f64 / ny as f64 * 68.0;
            if x0 + 105.0 / nx as f64 <= p.lmin {
                continue;
            }
            p.zone(s, x0, x0 + 105.0 / nx as f64 + 0.05, y0, y0 + 68.0 / ny as f64 + 0.05, col, (v * max_op).min(max_op));
        }
    }
}

// ---------------------------------------------------------------- 11 defensive activity
pub fn defensive_activity(c: &Ctx, sd: Side) -> Board {
    let mut s = Svg::new(W, H);
    let t = c.a.t(sd);
    let ev = &c.m.events;
    let title = format!("{} made {} ball-winning actions, on average {:.0} m from their goal", c.name(sd), t.won.len(), t.def_height);
    header(&mut s, c, &title, "Defensive activity · left: ball won · right: attack only stopped · shading = where most happened", Scope::Team(sd));
    let col = c.col(sd).to_string();
    let rec = t.won.iter().filter(|&&q| ev[q].kind == Kind::Recovery).count();
    let tck = t.won.iter().filter(|&&q| ev[q].kind == Kind::Tackle).count();
    let int = t.won.iter().filter(|&&q| ev[q].kind == Kind::Interception).count();
    let cl = t.stopped.iter().filter(|&&q| ev[q].kind == Kind::Clearance).count();
    let fo = t.stopped.iter().filter(|&&q| ev[q].kind == Kind::Foul).count();
    let bl = t.stopped.iter().filter(|&&q| ev[q].kind == Kind::Block).count();
    for (k, (lab, idx, avg)) in [(format!("WON THE BALL · {}", t.won.len()), &t.won, t.def_height), (format!("STOPPED, NOT WON · {}", t.stopped.len()), &t.stopped, t.stopped_height)].into_iter().enumerate() {
        let bx = 60.0 + k as f64 * 780.0;
        s.h(bx, 210.0, &lab, 26.0, &col, Anchor::Start);
        let legend_y = 236.0;
        let mut lx = bx;
        if k == 0 {
            lx = legend_item(&mut s, lx, legend_y, "circle", "#ffffff", &format!("{rec} recoveries"));
            lx = legend_item(&mut s, lx, legend_y, "triangle", "#facc15", &format!("{tck} tackles won"));
            legend_item(&mut s, lx, legend_y, "diamond", "#22d3ee", &format!("{int} interceptions"));
        } else {
            lx = legend_item(&mut s, lx, legend_y, "cross", "#f472b6", &format!("{cl} clearances"));
            lx = legend_item(&mut s, lx, legend_y, "square", "#fb923c", &format!("{fo} fouls"));
            legend_item(&mut s, lx, legend_y, "circle", "#e5e7eb", &format!("{bl} blocks"));
        }
        let p = Pitch::up(bx, 270.0, 700.0, 760.0, 0.0);
        let pts: Vec<(f64, f64)> = idx.iter().map(|&q| (ev[q].x, ev[q].y)).collect();
        kde(&mut s, &p, &pts, &col, 7.0, 0.55);
        p.label_attack(&mut s);
        p.draw(&mut s);
        for &q in idx.iter() {
            let e = &ev[q];
            let (x, y) = p.p(e.x, e.y);
            match e.kind {
                Kind::Recovery => s.circle(x, y, 4.5, "#ffffff", "none", 0.0, ""),
                Kind::Tackle => s.triangle(x, y, 6.0, "#facc15", "none", 0.0),
                Kind::Interception => s.diamond(x, y, 6.0, "#22d3ee", "none", 0.0),
                Kind::Clearance => s.cross(x, y, 4.5, "#f472b6", 2.2),
                Kind::Foul => s.square(x, y, 4.5, "#fb923c", "none", 0.0),
                _ => s.circle(x, y, 4.5, "#e5e7eb", "none", 0.0, ""),
            }
        }
        let (l1, y1) = p.p(avg, 0.0);
        let (l2, _) = p.p(avg, 68.0);
        s.line(l2, y1, l1, y1, "#ffffff", 1.5, r#"stroke-dasharray="6 5""#);
        s.t(p.x0 + p.w + 8.0, y1 + 4.0, &format!("avg {:.0} m", avg), 12.0, 700, theme::TEXT, Anchor::Start);
    }
    footer(&mut s, c, "Ball-winning actions (tackles won, interceptions, recoveries) on the left; actions that only stopped the attack (clearances, fouls, blocks) on the right. Event locations do not measure positioning away from the ball.");
    Board::new(&format!("11_defensive_activity_{}", side_tag(sd)), "Defensive activity", s)
}

// ---------------------------------------------------------------- 12 high regains
pub fn high_regains(c: &Ctx, sd: Side) -> Board {
    let mut s = Svg::new(W, H);
    let t = c.a.t(sd);
    let shots = t.high_regains.iter().filter(|r| r.outcome == RegainOutcome::Shot).count();
    let xg: f64 = t.high_regains.iter().map(|r| r.xg).sum();
    let boxn = t.high_regains.iter().filter(|r| r.outcome == RegainOutcome::Box).count();
    let nothing = t.high_regains.len() - shots - boxn;
    let title = format!("{} won the ball back high {} time{} · {} led to a shot", c.name(sd), t.high_regains.len(), if t.high_regains.len() == 1 { "" } else { "s" }, shots);
    header(&mut s, c, &title, "High regains: open-play recoveries in the attacking 40% · coloured by what the possession went on to do", Scope::Team(sd));
    let col = c.col(sd).to_string();
    let p = Pitch::up(80.0, 210.0, 760.0, 760.0, 40.0);
    p.zone(&mut s, HIGH_REGAIN_X, 105.0, 0.0, 68.0, "#ffffff", 0.04);
    p.label_attack(&mut s);
    p.draw(&mut s);
    let (a1, b1) = p.p(HIGH_REGAIN_X, 0.0);
    let (a2, _) = p.p(HIGH_REGAIN_X, 68.0);
    s.line(a2, b1, a1, b1, "#ffffff", 1.5, r#"stroke-dasharray="6 5""#);
    for r in &t.high_regains {
        let e = &c.m.events[r.idx];
        let (x, y) = p.p(e.x, e.y);
        match r.outcome {
            RegainOutcome::Nothing => s.circle(x, y, 6.0, "#8d8b98", "none", 0.0, ""),
            RegainOutcome::Box => s.square(x, y, 7.0, &col, "none", 0.0),
            RegainOutcome::Shot => {
                s.star(x, y, 11.0, "#ffffff", "none");
                s.t(x, y - 14.0, &e.label_minute(), 11.5, 700, theme::TEXT, Anchor::Middle);
            }
        }
    }
    let mut lx = 80.0;
    lx = legend_item(&mut s, lx, 1020.0, "circle", "#8d8b98", &format!("Nothing followed ({nothing})"));
    lx = legend_item(&mut s, lx, 1020.0, "square", &col, &format!("Reached the box ({boxn})"));
    legend_item(&mut s, lx, 1020.0, "star", "#ffffff", &format!("Led to a shot ({shots})"));
    let (px0, py0) = (940.0, 210.0);
    panel(&mut s, px0, py0, 600.0, 560.0);
    s.t(px0 + 28.0, py0 + 40.0, "WHAT THE REGAINS LED TO", 13.0, 800, theme::TEXT, Anchor::Start);
    let rows = [("HIGH REGAINS", t.high_regains.len().to_string()), ("LED TO A SHOT", format!("{} · {:.2} xG", shots, xg)), ("REACHED THE BOX ONLY", boxn.to_string()), ("NOTHING FOLLOWED", nothing.to_string())];
    for (i, (l, v)) in rows.iter().enumerate() {
        let yy = py0 + 100.0 + i as f64 * 110.0;
        s.t(px0 + 28.0, yy, l, 12.0, 700, theme::MUTED, Anchor::Start);
        s.h(px0 + 28.0, yy + 44.0, v, 40.0, theme::TEXT, Anchor::Start);
    }
    footer(&mut s, c, "A high regain is an open-play recovery, interception or tackle won in the attacking 40% (x ≥ 63 m). Each is marked by what the possession went on to do: nothing, a box entry, or a shot.");
    Board::new(&format!("12_high_regains_{}", side_tag(sd)), "High regains", s)
}

// ---------------------------------------------------------------- 13 high turnovers
pub fn high_turnovers(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, &format!("High Turnovers Forced, Errors & Expected Points · {} vs {}", c.name(Side::Home), c.name(Side::Away)), "Turnovers won in the attacking third (x ≥ 66.7%) → shot links · opponent errors forced · xPts & age", Scope::Full);
    let ev = &c.m.events;
    for (k, sd) in Side::both().iter().enumerate() {
        let t = c.a.t(*sd);
        let col = c.col(*sd).to_string();
        let bx = 40.0 + k as f64 * 780.0;
        let with_shot: Vec<&Turnover> = t.high_turnovers.iter().filter(|x| x.shot.is_some()).collect();
        let txg: f64 = with_shot.iter().map(|x| ev[x.shot.unwrap()].shot.as_ref().unwrap().xg).sum();
        let tgoals = with_shot.iter().filter(|x| ev[x.shot.unwrap()].shot.as_ref().unwrap().outcome == ShotOutcome::Goal).count();
        s.t(bx + 370.0, 205.0, &format!("{}  ·  High TO: {} ({} shots, {:.2} xG)", c.name(*sd), t.high_turnovers.len(), with_shot.len(), txg), 15.0, 800, &col, Anchor::Middle);
        let p = Pitch::up(bx, 240.0, 740.0, 600.0, 50.0);
        p.zone(&mut s, 70.0, 105.0, 0.0, 68.0, &col, 0.06);
        p.label_attack(&mut s);
        p.draw(&mut s);
        let (a1, b1) = p.p(70.0, 0.0);
        let (a2, _) = p.p(70.0, 68.0);
        s.line(a2, b1, a1, b1, "#ffffff", 1.5, r#"stroke-dasharray="6 5""#);
        s.t(a2 + 8.0, b1 - 6.0, "HIGH TURNOVER LINE (x = 66.7)", 10.5, 700, theme::TEXT, Anchor::Start);
        for to in &t.high_turnovers {
            let e = &ev[to.idx];
            let (x, y) = p.p(e.x, e.y);
            match to.shot {
                Some(q) => {
                    let se = &ev[q];
                    let (sx, sy) = p.p(se.x, se.y);
                    s.line(x, y, sx, sy, &col, 1.5, r#"stroke-dasharray="4 4""#);
                    s.circle(x, y, 7.0, &col, "none", 0.0, "");
                    if se.shot.as_ref().unwrap().outcome == ShotOutcome::Goal {
                        s.star(sx, sy, 10.0, &c.th.semantic.goal, "none");
                    } else {
                        s.triangle(sx, sy, 6.0, "#ffffff", "none", 0.0);
                    }
                }
                None => s.circle(x, y, 5.0, "#6b6878", "none", 0.0, ""),
            }
        }
        for &q in &t.opp_errors_forced {
            let e = &ev[q];
            let (x, y) = p.p(105.0 - e.x, 68.0 - e.y);
            s.cross(x, y, 6.0, "#facc15", 3.0);
        }
        panel(&mut s, bx, 870.0, 740.0, 90.0);
        let xpts = if *sd == Side::Home { c.a.win.xpts_home } else { c.a.win.xpts_away };
        let age = t.avg_age.map(|a| format!("{a:.1}")).unwrap_or_else(|| "N/A".into());
        let cells = [
            ("HIGH TO", format!("{} → {} sh", t.high_turnovers.len(), with_shot.len())),
            ("TO GOALS/XG", format!("{}G / {:.2}", tgoals, txg)),
            ("OWN ERRORS", format!("{} ({} sh/{}G)", t.own_errors.len(), t.own_errors_shot, t.own_errors_goal)),
            ("XPTS", format!("{:.2}", xpts)),
            ("AVG AGE", age),
        ];
        let tw = 740.0 / 5.0;
        for (i, (l, v)) in cells.iter().enumerate() {
            let x = bx + tw * i as f64 + tw / 2.0;
            s.t(x, 898.0, l, 11.0, 700, theme::MUTED, Anchor::Middle);
            s.t(x, 934.0, v, 17.0, 800, if i == 0 { &col } else { theme::TEXT }, Anchor::Middle);
        }
        if t.age_excluded > 0 && t.avg_age.is_some() {
            s.t(bx + 740.0, 978.0, &format!("age: {} players without DOB excluded", t.age_excluded), 10.5, 400, theme::GREY, Anchor::End);
        }
    }
    let mut lx = 200.0;
    lx = legend_item(&mut s, lx, 1010.0, "circle", "#cfcdd8", "Turnover → shot (team colour)");
    lx = legend_item(&mut s, lx, 1010.0, "circle", "#6b6878", "Turnover without shot");
    lx = legend_item(&mut s, lx, 1010.0, "triangle", "#ffffff", "Resulting shot");
    lx = legend_item(&mut s, lx, 1010.0, "star", &c.th.semantic.goal, "Resulting goal");
    legend_item(&mut s, lx, 1010.0, "cross", "#facc15", "Opponent error forced");
    footer(&mut s, c, "Each pitch plots attacking-third turnovers forced above x ≥ 66.7 (dashed lines to resulting shots within 12 s) alongside opposition errors forced. The strip contrasts high-press danger, errors, xPts and minutes-weighted squad age.");
    Board::new("13_high_turnovers", "High turnovers forced", s)
}

// ---------------------------------------------------------------- 14 PPDA timeline
pub fn ppda_timeline(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    let bins_h = &c.a.t(Side::Home).ppda_bins;
    let bins_a = &c.a.t(Side::Away).ppda_bins;
    let mut best: Option<(f64, Side, String)> = None;
    for (sd, bins) in [(Side::Home, bins_h), (Side::Away, bins_a)] {
        for (l, v) in bins.iter() {
            if let Some(v) = v {
                if best.as_ref().map(|b| *v < b.0).unwrap_or(true) {
                    best = Some((*v, sd, l.clone()));
                }
            }
        }
    }
    let title = match &best {
        Some((_, sd, l)) => format!("{} pressed hardest between {}", c.name(*sd), l.replace('–', "' and ").replace("''", "'")),
        None => "Pressing intensity through the match".into(),
    };
    header(&mut s, c, &title, "PPDA per 15 minutes · opponent passes in their own 60% per defensive action there · lower = more pressing", Scope::Full);
    let (x0, y0, w, h) = (150.0, 230.0, 1300.0, 640.0);
    let n = bins_h.len().max(1);
    let px = |i: usize| x0 + (i as f64 + 0.5) / n as f64 * w;
    let py = |v: f64| y0 + (v.min(30.0) - 1.0) / 29.0 * h;
    for v in [5.0, 10.0, 15.0, 20.0, 25.0, 30.0] {
        s.line(x0, py(v), x0 + w, py(v), theme::LINE, 1.0, "");
        s.t(x0 - 12.0, py(v) + 4.0, &format!("{v:.0}"), 12.0, 400, theme::MUTED, Anchor::End);
    }
    s.t(x0 - 60.0, y0 + h / 2.0, "PPDA (inverted: higher = more pressing)", 12.0, 500, theme::MUTED, Anchor::Middle);
    for (i, (l, _)) in bins_h.iter().enumerate() {
        s.t(px(i), y0 + h + 30.0, l, 13.0, 600, theme::TEXT, Anchor::Middle);
    }
    for (sd, bins) in [(Side::Home, bins_h), (Side::Away, bins_a)] {
        let col = c.col(sd).to_string();
        let pts: Vec<(usize, f64)> = bins.iter().enumerate().filter_map(|(i, (_, v))| v.map(|v| (i, v))).collect();
        let line: Vec<(f64, f64)> = pts.iter().map(|(i, v)| (px(*i), py(*v))).collect();
        s.polyline(&line, &col, 3.5, "");
        for (i, v) in &pts {
            s.circle(px(*i), py(*v), 7.0, &col, "#000", 1.0, "");
            let dy = if sd == Side::Home { -14.0 } else { 24.0 };
            s.t(px(*i), py(*v) + dy, &format!("{v:.1}"), 13.0, 800, &col, Anchor::Middle);
        }
    }
    let mut lx = x0 + w - 380.0;
    lx = legend_item(&mut s, lx, y0 + h - 20.0, "line", c.col(Side::Home), &c.name(Side::Home));
    legend_item(&mut s, lx, y0 + h - 20.0, "line", c.col(Side::Away), &c.name(Side::Away));
    let (h1, h2) = (c.a.t(Side::Home), c.a.t(Side::Away));
    let f = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or("–".into());
    s.t(x0, y0 + h + 70.0, &format!("{}: PPDA {} (1st half {}, 2nd half {})   ·   {}: PPDA {} (1st half {}, 2nd half {})", c.name(Side::Home), f(h1.ppda), f(h1.ppda_h1), f(h1.ppda_h2), c.name(Side::Away), f(h2.ppda), f(h2.ppda_h1), f(h2.ppda_h2)), 14.0, 500, theme::TEXT, Anchor::Start);
    s.t(x0, y0 + h + 96.0, "Bins with no defensive actions are left out; values above 30 are drawn at 30.", 12.0, 400, theme::GREY, Anchor::Start);
    footer(&mut s, c, "Read the stated event, time window and denominator before comparing the teams. Extra-time bins (90–105, 105–120+) appear only when extra time was played.");
    Board::new("14_ppda_timeline", "PPDA timeline", s)
}

// ---------------------------------------------------------------- 15 momentum
pub fn momentum(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Match Momentum", "Threat built in a rolling eight minutes, and the spells the match divided into", Scope::Full);
    let mo = &c.a.momentum;
    let (x0, y0, w, h) = (90.0, 230.0, 1420.0, 560.0);
    let dur = mo.minutes.last().copied().unwrap_or(90.0).max(1.0);
    let mx = mo.home.iter().chain(&mo.away).cloned().fold(0.05, f64::max);
    let px = |m: f64| x0 + m / dur * w;
    let mid = y0 + h / 2.0;
    let py = |v: f64| v / mx * (h / 2.0 - 20.0);
    s.line(x0, mid, x0 + w, mid, "#ffffff", 1.0, r#"opacity="0.5""#);
    let hc = c.col(Side::Home).to_string();
    let ac = c.col(Side::Away).to_string();
    let mut up = vec![(px(0.0), mid)];
    let mut dn = vec![(px(0.0), mid)];
    for (i, m) in mo.minutes.iter().enumerate() {
        up.push((px(*m), mid - py(mo.home[i])));
        dn.push((px(*m), mid + py(mo.away[i])));
    }
    up.push((px(dur), mid));
    dn.push((px(dur), mid));
    s.poly(&up, &hc, "none", 0.0, r#"opacity="0.9""#);
    s.poly(&dn, &ac, "none", 0.0, r#"opacity="0.9""#);
    if let Some(p2) = c.m.periods.iter().find(|p| p.period == 2) {
        let x = px(p2.start_t / 60.0);
        s.line(x, y0, x, y0 + h, "#ffffff", 1.2, r#"stroke-dasharray="4 5" opacity="0.6""#);
        s.t(x + 4.0, y0 + 12.0, "HT", 11.0, 600, theme::MUTED, Anchor::Start);
    }
    for (side, e) in c.m.goals_timeline() {
        let x = px(e.t / 60.0);
        let y = if side == Side::Home { mid - 6.0 } else { mid + 6.0 };
        s.circle(x, y, 8.0, c.col(side), "#fff", 2.0, "");
        let og = e.shot.as_ref().unwrap().own_goal;
        let lab = format!("{} {}{}", c.pname(e.side, e.player), e.label_minute(), if og { " (OG)" } else { "" });
        s.t(x + 12.0, if side == Side::Home { y - 12.0 } else { y + 22.0 }, &lab, 13.0, 700, theme::TEXT, Anchor::Start);
    }
    // substitutions (grouped per minute) and red cards
    for sd in Side::both() {
        let mut subs: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        for p in &c.m.team(sd).players {
            if !p.starter && !p.on_t.is_nan() {
                *subs.entry((p.on_t / 60.0).round() as i64).or_default() += 1;
            }
        }
        for (mn, n) in subs {
            let x = px(mn as f64);
            let y = if sd == Side::Home { y0 - 6.0 } else { y0 + h + 14.0 };
            if sd == Side::Home {
                s.triangle(x, y, 7.0, c.col(sd), "none", 0.0)
            } else {
                s.poly(&[(x, y + 7.0), (x + 7.0, y - 5.0), (x - 7.0, y - 5.0)], c.col(sd), "none", 0.0, "");
            }
            if n > 1 {
                s.t(x + 10.0, y + 4.0, &format!("×{n}"), 11.0, 700, theme::MUTED, Anchor::Start);
            }
        }
    }
    for e in c.m.events.iter().filter(|e| e.kind == Kind::Card) {
        let x = px(e.t / 60.0);
        s.rect(x - 4.0, if e.side == Side::Home { y0 + 4.0 } else { y0 + h - 18.0 }, 8.0, 12.0, "#dc2626", "");
    }
    s.t(x0 + w, y0 + 20.0, &c.name(Side::Home).to_uppercase(), 15.0, 800, &hc, Anchor::End);
    s.t(x0 + w, y0 + h - 8.0, &c.name(Side::Away).to_uppercase(), 15.0, 800, &ac, Anchor::End);
    s.t(x0 - 16.0, mid, "xT built in the window", 11.0, 500, theme::MUTED, Anchor::End);
    // spells
    let sy = y0 + h + 60.0;
    s.t(x0, sy - 10.0, "SPELLS", 12.0, 800, theme::TEXT, Anchor::Start);
    for sp in &mo.spells {
        let (a, b) = (px(sp.start), px(sp.end));
        let dom = if sp.share_home >= 0.5 { &hc } else { &ac };
        s.rect(a + 1.0, sy, (b - a - 2.0).max(1.0), 70.0, dom, "");
        if b - a > 110.0 {
            let on = if theme::Rgb::hex(dom).unwrap().luminance() > 0.45 { "#000" } else { "#fff" };
            s.t((a + b) / 2.0, sy + 30.0, &format!("{:.0}–{:.0}'  {:.0} / {:.0}", sp.start, sp.end, sp.share_home * 100.0, (1.0 - sp.share_home) * 100.0), 14.0, 800, on, Anchor::Middle);
            s.t((a + b) / 2.0, sy + 52.0, &format!("{:.2} – {:.2} xG", sp.xg_home, sp.xg_away), 12.5, 500, on, Anchor::Middle);
        }
    }
    for mm in (0..=(dur as i32)).step_by(15) {
        s.t(px(mm as f64), sy + 96.0, &mm.to_string(), 12.0, 400, theme::MUTED, Anchor::Middle);
    }
    s.t(x0 + w / 2.0, sy + 118.0, "Match minute", 12.0, 500, theme::MUTED, Anchor::Middle);
    footer(&mut s, c, "Threat added by successful moves, totalled over a rolling eight minutes and weighted towards the middle of that window. Spells are cut where the share of threat turns; each is priced in the shot xG it produced. Triangles mark substitutions, red blocks mark cards.");
    Board::new("15_momentum", "Match momentum", s)
}

// ---------------------------------------------------------------- 16 game state
pub fn game_state(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Game-State Output by Match Situation", "Each card is one shared scoreboard situation · values are totals from possessions starting in that situation", Scope::Full);
    let rows = &c.a.states;
    let total: f64 = rows.iter().map(|r| r.minutes).sum::<f64>().max(1.0);
    let (bx, by, bw) = (60.0, 200.0, W - 120.0);
    let cols = ["#6b6878".to_string(), c.col(Side::Home).to_string(), c.col(Side::Away).to_string()];
    let mut x = bx;
    for (i, r) in rows.iter().enumerate() {
        let w = r.minutes / total * bw;
        s.rect(x, by, w, 30.0, &cols[i], "");
        if w > 120.0 {
            let on = if theme::Rgb::hex(&cols[i]).unwrap().luminance() > 0.45 { "#000" } else { "#fff" };
            s.t(x + w / 2.0, by + 20.0, &r.label, 13.0, 800, on, Anchor::Middle);
        }
        x += w;
    }
    s.t(bx, by + 52.0, "MATCH TIME SHARE", 11.0, 700, theme::MUTED, Anchor::Start);
    s.t(bx + bw, by + 52.0, &format!("TOTAL · {:.1} min", total), 11.0, 700, theme::MUTED, Anchor::End);
    let mut lx = bx + 400.0;
    for (i, r) in rows.iter().enumerate() {
        lx = legend_item(&mut s, lx, by + 84.0, "square", &cols[i], &format!("{} · {:.0} min", r.label.to_lowercase(), r.minutes));
    }
    let played: Vec<(usize, &StateRow)> = rows.iter().enumerate().filter(|(_, r)| r.minutes > 0.5).collect();
    let not_played: Vec<&StateRow> = rows.iter().filter(|r| r.minutes <= 0.5).collect();
    let np_text: Vec<String> = not_played.iter().map(|r| format!("NOT PLAYED · {} never {}", r.label.trim_end_matches(" AHEAD"), if r.label.contains("AHEAD") { "led" } else { "level" })).collect();
    if !np_text.is_empty() {
        s.t(W / 2.0, by + 124.0, &np_text.join("   ·   "), 13.0, 800, theme::TEXT, Anchor::Middle);
    }
    let n = played.len().max(1);
    let cw = (W - 120.0 - 40.0 * (n as f64 - 1.0)) / n as f64;
    for (k, (i, r)) in played.iter().enumerate() {
        let cx = 60.0 + k as f64 * (cw + 40.0);
        let cy = 360.0;
        s.rrect(cx, cy, cw, 560.0, 8.0, "#0f0c15", &cols[*i], 2.0);
        s.h(cx + 28.0, cy + 50.0, &r.label, 28.0, theme::TEXT, Anchor::Start);
        s.h(cx + cw - 28.0, cy + 50.0, &format!("{:.1} min", r.minutes), 24.0, theme::TEXT, Anchor::End);
        s.t(cx + cw - 220.0, cy + 110.0, &c.name(Side::Home).to_uppercase(), 12.0, 800, c.col(Side::Home), Anchor::Middle);
        s.t(cx + cw - 80.0, cy + 110.0, &c.name(Side::Away).to_uppercase(), 12.0, 800, c.col(Side::Away), Anchor::Middle);
        let per30 = |xg: f64| if r.minutes > 0.0 { xg / r.minutes * 30.0 } else { 0.0 };
        let lines = [
            ("Shots", r.shots[0].to_string(), r.shots[1].to_string()),
            ("xG", format!("{:.2}", r.xg[0]), format!("{:.2}", r.xg[1])),
            ("Transitions", r.transitions[0].to_string(), r.transitions[1].to_string()),
            ("Box entries", r.entries[0].to_string(), r.entries[1].to_string()),
            ("xG / 30 min", format!("{:.2}", per30(r.xg[0])), format!("{:.2}", per30(r.xg[1]))),
        ];
        for (j, (l, a, b)) in lines.iter().enumerate() {
            let yy = cy + 170.0 + j as f64 * 70.0;
            s.t(cx + 28.0, yy, l, 16.0, 500, theme::TEXT, Anchor::Start);
            s.h(cx + cw - 220.0, yy + 4.0, a, 28.0, theme::TEXT, Anchor::Middle);
            s.h(cx + cw - 80.0, yy + 4.0, b, 28.0, theme::TEXT, Anchor::Middle);
            s.line(cx + 28.0, yy + 22.0, cx + cw - 28.0, yy + 22.0, theme::LINE, 1.0, "");
        }
    }
    footer(&mut s, c, "Game state is assigned at possession start; minutes are counted per half including added time. The score at the time of each event defines the state.");
    Board::new("16_game_state", "Game-state output", s)
}

// ---------------------------------------------------------------- 17 VAEP
pub fn vaep(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    let ps: Vec<&PlayerStats> = c.a.players.iter().collect();
    let best = ps.iter().max_by(|a, b| (a.vaep_off + a.vaep_def).partial_cmp(&(b.vaep_off + b.vaep_def)).unwrap());
    let title = best.map(|b| format!("{} added the most to their side's chances", b.short)).unwrap_or_else(|| "Action value".into());
    header(&mut s, c, &title, "VAEP · every action valued by how much it raised the chance of scoring and lowered the chance of conceding", Scope::Full);
    let (th, ta) = (c.a.t(Side::Home).vaep, c.a.t(Side::Away).vaep);
    s.t(60.0, 205.0, &format!("{}  {}  →", c.name(Side::Home).to_uppercase(), signed(th, 2)), 16.0, 800, c.col(Side::Home), Anchor::Start);
    s.t(1060.0, 205.0, &format!("←  {}  {}", c.name(Side::Away).to_uppercase(), signed(ta, 2)), 16.0, 800, c.col(Side::Away), Anchor::End);
    let p = Pitch::right(60.0, 225.0, 1000.0, 600.0);
    p.draw(&mut s);
    let mx = ps.iter().map(|p| (p.vaep_off + p.vaep_def).abs()).fold(0.05, f64::max);
    let mut labelled = 0;
    let mut sorted = ps.clone();
    sorted.sort_by(|a, b| (b.vaep_off + b.vaep_def).partial_cmp(&(a.vaep_off + a.vaep_def)).unwrap());
    for (rank, pl) in sorted.iter().enumerate() {
        let sd = pl.side.unwrap();
        let (x, y) = if sd == Side::Home { (pl.avg_x, pl.avg_y) } else { (105.0 - pl.avg_x, 68.0 - pl.avg_y) };
        let (sx, sy) = p.p(x, y);
        let tot = pl.vaep_off + pl.vaep_def;
        let r = 6.0 + 26.0 * (tot.max(0.0) / mx).sqrt();
        s.circle(sx, sy, r, c.col(sd), "none", 0.0, r#"opacity="0.85""#);
        let given: f64 = -(pl.vaep_off.min(0.0) + pl.vaep_def.min(0.0));
        if given > 0.02 {
            s.circle(sx, sy, r + 3.0, "none", c.col(sd), 1.6, r#"opacity="0.7""#);
        }
        if rank < 8 {
            s.t(sx, sy + r + 15.0, &format!("{} {}", pl.short, signed(tot, 2)), 12.0, 700, theme::TEXT, Anchor::Middle);
            labelled += 1;
        }
    }
    let _ = labelled;
    // top actions
    let (px0, py0) = (1090.0, 200.0);
    panel(&mut s, px0, py0, 460.0, 640.0);
    s.t(px0 + 24.0, py0 + 36.0, "ACTIONS THAT MOVED THE MATCH MOST", 12.5, 800, theme::TEXT, Anchor::Start);
    let ev = &c.m.events;
    let mut acts: Vec<(usize, f64)> = (0..ev.len()).map(|i| (i, c.a.values.vaep_off[i] + c.a.values.vaep_def[i])).filter(|x| x.1.abs() > 0.0).collect();
    acts.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    for (j, (i, v)) in acts.iter().filter(|(i, _)| ev[*i].shot.as_ref().map(|s| s.outcome != ShotOutcome::Goal).unwrap_or(true)).take(12).enumerate() {
        let e = &ev[*i];
        let yy = py0 + 78.0 + j as f64 * 44.0;
        let kind = format!("{:?}", e.kind).to_lowercase();
        s.t(px0 + 24.0, yy, &format!("{}  {} · {}", e.label_minute(), c.pname(e.side, e.player), kind), 14.0, 600, theme::TEXT, Anchor::Start);
        s.t(px0 + 436.0, yy, &signed(*v, 2), 14.0, 800, c.col(e.side), Anchor::End);
    }
    // bars top 6
    let by = 870.0;
    for (j, pl) in sorted.iter().take(6).enumerate() {
        let yy = by + j as f64 * 26.0;
        let sd = pl.side.unwrap();
        s.t(170.0, yy + 14.0, &pl.short, 12.5, 600, theme::TEXT, Anchor::End);
        let wo = pl.vaep_off.max(0.0) / mx * 600.0;
        let wd = pl.vaep_def.max(0.0) / mx * 600.0;
        s.rect(180.0, yy, wo, 18.0, c.col(sd), "");
        s.rect(180.0 + wo, yy, wd, 18.0, &c.th.team(sd).muted, "");
        s.t(186.0 + wo + wd, yy + 14.0, &signed(pl.vaep_off + pl.vaep_def, 2), 12.5, 800, theme::TEXT, Anchor::Start);
    }
    s.t(1060.0, by + 150.0, "SOLID = ATTACK · FADED = DEFENCE. Disc = a player at the average spot of their actions, sized by value added; ring = gave value away.", 11.5, 400, theme::MUTED, Anchor::End);
    footer(&mut s, c, "Local action values rank the recorded contributions under the named model (VAEP fitted on StatsBomb Open Data; keeper saves count as defence). They are model estimates, not an all-round player rating.");
    Board::new("17_vaep", "VAEP action value", s)
}

// ---------------------------------------------------------------- 18 dominating zones
pub fn zones(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Dominating Zones", &format!("Colour = each side's share of the zone's touches · figure = {} touches minus {}'s", c.name(Side::Home), c.name(Side::Away)), Scope::Full);
    let p = Pitch::up(200.0, 210.0, 600.0, 820.0, 0.0);
    let rows = c.a.zones.len();
    for (r, row) in c.a.zones.iter().enumerate() {
        for (cidx, v) in row.iter().enumerate() {
            let x0 = r as f64 / rows as f64 * 105.0;
            let x1 = (r + 1) as f64 / rows as f64 * 105.0;
            let ya = 68.0 - (cidx + 1) as f64 / 5.0 * 68.0;
            let yb = 68.0 - cidx as f64 / 5.0 * 68.0;
            p.zone(&mut s, x0, x1, ya, yb, &c.th.div(v / 60.0), 1.0);
            let (cx, cy) = p.p((x0 + x1) / 2.0, (ya + yb) / 2.0);
            s.t(cx, cy + 6.0, &format!("{}{:.0}", if *v >= 0.0 { "+" } else { "" }, v), 17.0, 800, theme::TEXT, Anchor::Middle);
        }
    }
    p.label_attack(&mut s);
    p.draw(&mut s);
    // colourbar
    let (bx, by, bh) = (840.0, 300.0, 400.0);
    for i in 0..100 {
        let v = 1.0 - i as f64 / 50.0;
        s.rect(bx, by + i as f64 * bh / 100.0, 18.0, bh / 100.0 + 0.5, &c.th.div(v), "");
    }
    s.t(bx + 26.0, by + 6.0, &c.name(Side::Home), 12.0, 600, theme::TEXT, Anchor::Start);
    s.t(bx + 26.0, by + bh / 2.0 + 4.0, "Balanced", 12.0, 600, theme::MUTED, Anchor::Start);
    s.t(bx + 26.0, by + bh, &c.name(Side::Away), 12.0, 600, theme::TEXT, Anchor::Start);
    let (px0, py0) = (1040.0, 260.0);
    panel(&mut s, px0, py0, 500.0, 460.0);
    let [h, a] = c.a.zone_touches;
    let tot = (h + a).max(1) as f64;
    s.t(px0 + 28.0, py0 + 40.0, "TERRITORY TOTALS", 13.0, 800, theme::TEXT, Anchor::Start);
    s.t(px0 + 28.0, py0 + 100.0, &format!("{} TOUCHES", c.name(Side::Home).to_uppercase()), 12.0, 700, theme::MUTED, Anchor::Start);
    s.h(px0 + 28.0, py0 + 146.0, &format!("{} ({:.0}%)", h, h as f64 / tot * 100.0), 40.0, theme::TEXT, Anchor::Start);
    s.t(px0 + 28.0, py0 + 210.0, &format!("{} TOUCHES", c.name(Side::Away).to_uppercase()), 12.0, 700, theme::MUTED, Anchor::Start);
    s.h(px0 + 28.0, py0 + 256.0, &format!("{} ({:.0}%)", a, a as f64 / tot * 100.0), 40.0, theme::TEXT, Anchor::Start);
    s.t(px0 + 28.0, py0 + 320.0, "DIFFERENCE", 12.0, 700, theme::MUTED, Anchor::Start);
    s.h(px0 + 28.0, py0 + 366.0, &format!("{}{}", if h >= a { "+" } else { "−" }, (h as i64 - a as i64).abs()), 40.0, theme::TEXT, Anchor::Start);
    footer(&mut s, c, "Zone shares compare each team's recorded involvement in the same area, with the home side attacking up the page. Territorial activity can be high without creating high-quality shots.");
    Board::new("18_dominating_zones", "Dominating zones", s)
}

// ---------------------------------------------------------------- 19 transitions
pub fn transitions(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Transition Map & Outcomes", "Regain or turnover to the furthest point reached in 12 seconds · grey = average per zone · coloured = made a chance", Scope::Full);
    for (k, sd) in Side::both().iter().enumerate() {
        let t = c.a.t(*sd);
        let col = c.col(*sd).to_string();
        let bx = 40.0 + k as f64 * 780.0;
        s.h(bx + 230.0, 210.0, &c.name(*sd), 28.0, theme::TEXT, Anchor::Middle);
        let p = Pitch::up(bx, 235.0, 480.0, 760.0, 0.0);
        p.label_attack(&mut s);
        p.draw(&mut s);
        // average arrow per start zone (3 x 3 grid) for non-chance transitions
        let mut agg: std::collections::HashMap<(usize, usize), (f64, f64, f64, f64, f64)> = std::collections::HashMap::new();
        for tr in t.transitions.iter().filter(|t| !t.shot) {
            let key = (((tr.start.0 / 35.0) as usize).min(2), ((tr.start.1 / 22.67) as usize).min(2));
            let en = agg.entry(key).or_insert((0.0, 0.0, 0.0, 0.0, 0.0));
            en.0 += tr.start.0;
            en.1 += tr.start.1;
            en.2 += tr.adv.0;
            en.3 += tr.adv.1;
            en.4 += 1.0;
        }
        for v in agg.values() {
            let (a1, b1) = p.p(v.0 / v.4, v.1 / v.4);
            let (a2, b2) = p.p(v.2 / v.4, v.3 / v.4);
            s.arrow(a1, b1, a2, b2, "#8d8b98", 2.0 + v.4.sqrt() * 1.5, 12.0, r#"opacity="0.8""#);
        }
        for tr in t.transitions.iter().filter(|t| t.shot) {
            let (a1, b1) = p.p(tr.start.0, tr.start.1);
            let (a2, b2) = p.p(tr.adv.0, tr.adv.1);
            let lc = if tr.goal { "#ffffff" } else { col.as_str() };
            s.arrow(a1, b1, a2, b2, lc, 2.2, 9.0, "");
            s.square(a1, b1, 3.5, lc, "none", 0.0);
        }
        let (px0, py0) = (bx + 500.0, 300.0);
        panel(&mut s, px0, py0, 230.0, 560.0);
        s.t(px0 + 20.0, py0 + 36.0, "OUTCOMES", 13.0, 800, theme::TEXT, Anchor::Start);
        let chances = t.transitions.iter().filter(|x| x.shot).count();
        let goals = t.transitions.iter().filter(|x| x.goal).count();
        let txg: f64 = t.transitions.iter().map(|x| x.xg).sum();
        let be = t.transitions.iter().filter(|x| x.box_entry).count();
        let rows = [("TRANSITIONS", t.transitions.len().to_string()), ("CREATED CHANCE", chances.to_string()), ("GOALS", goals.to_string()), ("TRANSITION XG", format!("{txg:.2}")), ("BOX ENTRIES", be.to_string())];
        for (i, (l, v)) in rows.iter().enumerate() {
            let yy = py0 + 80.0 + i as f64 * 84.0;
            s.t(px0 + 20.0, yy, l, 11.0, 700, theme::MUTED, Anchor::Start);
            s.h(px0 + 20.0, yy + 38.0, v, 34.0, theme::TEXT, Anchor::Start);
        }
        s.t(px0 + 20.0, py0 + 530.0, &format!("Chance rate: {:.1}%", chances as f64 / t.transitions.len().max(1) as f64 * 100.0), 13.0, 700, theme::TEXT, Anchor::Start);
    }
    let mut lx = 200.0;
    lx = legend_item(&mut s, lx, 1040.0, "line", "#8d8b98", "Average transition without a shot, by zone");
    lx = legend_item(&mut s, lx, 1040.0, "line", "#cfcdd8", "Created a chance (side's colour)");
    legend_item(&mut s, lx, 1040.0, "line", "#ffffff", "Ended in a goal");
    footer(&mut s, c, "Definition: open-play regain where, within 12 s, the ball progresses ≥ 20 m, reaches the final third or the box, or produces a shot. Restarts excluded. Arrow = start → most advanced point.");
    Board::new("19_transitions", "Transition map", s)
}

// ---------------------------------------------------------------- 20 match statistics
fn gauge(s: &mut Svg, cx: f64, cy: f64, r: f64, v: Option<f64>, label: &str, name: &str, col: &str) {
    let ang = |x: f64| std::f64::consts::PI * (1.0 - (x.clamp(0.0, 20.0) / 20.0));
    let bands = [(0.0, 8.0, "#22c55e"), (8.0, 12.0, "#eab308"), (12.0, 20.0, "#ef4444")];
    for (a, b, colr) in bands {
        let mut pts = vec![];
        let n = 24;
        for i in 0..=n {
            let t = ang(a + (b - a) * i as f64 / n as f64);
            pts.push((cx + r * t.cos(), cy - r * t.sin()));
        }
        s.polyline(&pts, colr, 16.0, "");
    }
    if let Some(v) = v {
        let t = ang(v);
        s.line(cx, cy, cx + (r - 12.0) * t.cos(), cy - (r - 12.0) * t.sin(), col, 5.0, r#"stroke-linecap="round""#);
        s.circle(cx, cy, 8.0, col, "none", 0.0, "");
        s.h(cx, cy + 46.0, &format!("{v:.2}"), 38.0, theme::TEXT, Anchor::Middle);
    } else {
        s.h(cx, cy + 46.0, "–", 38.0, theme::TEXT, Anchor::Middle);
    }
    s.t(cx, cy + 72.0, label, 12.0, 800, theme::TEXT, Anchor::Middle);
    s.t(cx, cy - r - 22.0, name, 15.0, 800, theme::TEXT, Anchor::Middle);
}

pub fn ppda_band(v: Option<f64>) -> &'static str {
    match v {
        Some(v) if v < 8.0 => "ELITE PRESS",
        Some(v) if v <= 12.0 => "MEDIUM BLOCK",
        Some(_) => "LOW BLOCK",
        None => "NO DATA",
    }
}

pub fn match_stats(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Match Statistics", "Attack, passing, pressing and defensive actions · every figure counted from the event stream", Scope::Full);
    let (h, a) = (c.a.t(Side::Home), c.a.t(Side::Away));
    let ratio = |x: (usize, usize)| format!("{}/{}", x.0, x.1);
    let attack: Vec<(&str, String, String, f64, f64)> = vec![
        ("Goals", h.goals.to_string(), a.goals.to_string(), h.goals as f64, a.goals as f64),
        ("xG", format!("{:.2}", h.xg), format!("{:.2}", a.xg), h.xg, a.xg),
        ("xGOT", format!("{:.2}", h.xgot), format!("{:.2}", a.xgot), h.xgot, a.xgot),
        ("Shots", h.shots.len().to_string(), a.shots.len().to_string(), h.shots.len() as f64, a.shots.len() as f64),
        ("Big chances", h.clear_chances.len().to_string(), a.clear_chances.len().to_string(), h.clear_chances.len() as f64, a.clear_chances.len() as f64),
        ("Positive pass xT", format!("{:.2}", h.positive_xt), format!("{:.2}", a.positive_xt), h.positive_xt, a.positive_xt),
        ("Passes attempted", h.passes.to_string(), a.passes.to_string(), h.passes as f64, a.passes as f64),
        ("Key passes", h.key_passes.to_string(), a.key_passes.to_string(), h.key_passes as f64, a.key_passes as f64),
        ("Progressive passes", h.progressive_passes.to_string(), a.progressive_passes.to_string(), h.progressive_passes as f64, a.progressive_passes as f64),
        ("Line-breaking passes", ratio(h.line_breaking), ratio(a.line_breaking), h.line_breaking.0 as f64, a.line_breaking.0 as f64),
        ("Box entries", h.entries.len().to_string(), a.entries.len().to_string(), h.entries.len() as f64, a.entries.len() as f64),
        ("Field tilt %", format!("{:.1}", h.field_tilt * 100.0), format!("{:.1}", a.field_tilt * 100.0), h.field_tilt, a.field_tilt),
    ];
    let defence: Vec<(&str, String, String, f64, f64)> = vec![
        ("Tackles", h.tackles.to_string(), a.tackles.to_string(), h.tackles as f64, a.tackles as f64),
        ("Interceptions", h.interceptions.to_string(), a.interceptions.to_string(), h.interceptions as f64, a.interceptions as f64),
        ("Blocks", h.blocks.to_string(), a.blocks.to_string(), h.blocks as f64, a.blocks as f64),
        ("Clearances", h.clearances.to_string(), a.clearances.to_string(), h.clearances as f64, a.clearances as f64),
        ("Ball recoveries", h.recoveries.to_string(), a.recoveries.to_string(), h.recoveries as f64, a.recoveries as f64),
        ("Possession regains", h.regains_all.to_string(), a.regains_all.to_string(), h.regains_all as f64, a.regains_all as f64),
        ("High regains", h.high_regains.len().to_string(), a.high_regains.len().to_string(), h.high_regains.len() as f64, a.high_regains.len() as f64),
        ("Fouls", h.fouls.to_string(), a.fouls.to_string(), h.fouls as f64, a.fouls as f64),
        ("Aerial duels", ratio(h.aerials), ratio(a.aerials), h.aerials.0 as f64, a.aerials.0 as f64),
        ("Take-on duels", ratio(h.take_ons), ratio(a.take_ons), h.take_ons.0 as f64, a.take_ons.0 as f64),
    ];
    let table = |s: &mut Svg, x: f64, y: f64, title: &str, rows: &[(&str, String, String, f64, f64)], border: &str| {
        let hgt = 70.0 + rows.len() as f64 * 34.0;
        s.rrect(x, y, 700.0, hgt, 6.0, "#0f0c15", border, 1.5);
        s.t(x + 20.0, y + 30.0, title, 13.0, 800, theme::TEXT, Anchor::Start);
        s.t(x + 20.0, y + 56.0, &c.name(Side::Home), 12.0, 800, c.col(Side::Home), Anchor::Start);
        s.t(x + 680.0, y + 56.0, &c.name(Side::Away), 12.0, 800, c.col(Side::Away), Anchor::End);
        for (i, (l, va, vb, fa, fb)) in rows.iter().enumerate() {
            let yy = y + 88.0 + i as f64 * 34.0;
            let tot = (fa + fb).max(1e-9);
            s.t(x + 20.0, yy, va, 14.0, 700, theme::TEXT, Anchor::Start);
            s.rect(x + 100.0 + 120.0 * (1.0 - fa / tot), yy - 10.0, 120.0 * fa / tot, 10.0, c.col(Side::Home), "");
            s.t(x + 350.0, yy, l, 13.0, 500, theme::TEXT, Anchor::Middle);
            s.rect(x + 480.0, yy - 10.0, 120.0 * fb / tot, 10.0, c.col(Side::Away), "");
            s.t(x + 680.0, yy, vb, 14.0, 700, theme::TEXT, Anchor::End);
        }
    };
    table(&mut s, 40.0, 170.0, "■ ATTACK & PASSING", &attack, c.col(Side::Home));
    table(&mut s, 40.0, 170.0 + 70.0 + attack.len() as f64 * 34.0 + 20.0, "■ DEFENSIVE ACTIONS", &defence, c.col(Side::Away));
    s.t(1160.0, 200.0, "Passes per Defensive Action — lower = more aggressive press", 14.0, 600, theme::TEXT, Anchor::Middle);
    gauge(&mut s, 980.0, 400.0, 120.0, h.ppda, ppda_band(h.ppda), &c.name(Side::Home), c.col(Side::Home));
    gauge(&mut s, 1340.0, 400.0, 120.0, a.ppda, ppda_band(a.ppda), &c.name(Side::Away), c.col(Side::Away));
    if let (Some(x), Some(y)) = (h.ppda, a.ppda) {
        let more = if x < y { Side::Home } else { Side::Away };
        s.h(1160.0, 540.0, &format!("{} pressed more aggressively", c.name(more)), 28.0, theme::TEXT, Anchor::Middle);
        s.t(1160.0, 570.0, &format!("PPDA differential: {:.2}", (x - y).abs()), 13.0, 500, theme::MUTED, Anchor::Middle);
    }
    panel(&mut s, 780.0, 620.0, 780.0, 380.0);
    s.t(800.0, 652.0, "READING THIS PAGE", 13.0, 800, theme::TEXT, Anchor::Start);
    let cols = [
        ("ATTACK & PASSING", "Shots and key passes show how often each side created looks at goal. xGOT rates shots on target once placement is known, so it can exceed xG for a clinical finisher."),
        ("PRESSING (PPDA)", "Lower PPDA = more aggressive press. Under 8 is an elite press, 8–12 a medium block, above 12 a low block."),
        ("DEFENSIVE ACTIONS", "Tackles and interceptions describe ground duels. Blocks stop shots with a body. Recoveries and clearances show how pressure was escaped."),
    ];
    for (i, (t, body)) in cols.iter().enumerate() {
        let x = 800.0 + i as f64 * 255.0;
        s.t(x, 690.0, t, 11.5, 800, theme::TEXT, Anchor::Start);
        let mut line = String::new();
        let mut yy = 716.0;
        for w in body.split_whitespace() {
            if line.len() + w.len() > 34 {
                s.t(x, yy, &line, 11.5, 400, theme::MUTED, Anchor::Start);
                yy += 18.0;
                line.clear();
            }
            line.push_str(w);
            line.push(' ');
        }
        s.t(x, yy, &line, 11.5, 400, theme::MUTED, Anchor::Start);
    }
    footer(&mut s, c, "One page of counted actions for both sides, plus the pressing rate. Every figure is a count from the event stream over the whole match: each pair of bars splits the row between the two sides.");
    Board::new("20_match_statistics", "Match statistics", s)
}
