//! Boards 21-31: xT heatmap, goal frames, height/length, chance quality vs
//! finishing, press triggers, progression scatter, funnel, entry routes,
//! competition context scatters, player cards.

use super::boards1::{shot_r, shot_tip, side_tag, H, W};
use super::boards2::kde;
use super::frame::*;
use super::pitch::*;
use super::svg::*;
use super::Board;
use crate::derive::metrics::*;
use crate::model::*;
use crate::store::TeamMatchRow;
use crate::theme;

// ---------------------------------------------------------------- 21 xT heatmap
pub fn xt_heatmap(c: &Ctx, sd: Side) -> Board {
    let mut s = Svg::new(W, H);
    let t = c.a.t(sd);
    header(&mut s, c, &format!("xT Heatmap · {}", c.name(sd)), "8 × 12 squares over the pitch · xT added where each successful pass or carry began · one scale for both teams", Scope::Team(sd));
    // shared scale: 95th percentile of both teams' cells
    let mut all: Vec<f64> = c.a.team.iter().flat_map(|t| t.xt_grid.iter().copied()).filter(|v| *v > 0.0).collect();
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let top = all.get(((all.len() as f64) * 0.95) as usize).copied().unwrap_or(0.05).max(0.01);
    // the grid is the visual: no pitch markings, cells with thin dark gutters. Top-3 pass vectors sit
    // on a translucent toggleable layer, and the numbers are drawn last, on dark pills, so they stay readable.
    let p = Pitch::up(80.0, 200.0, 760.0, 780.0, 0.0);
    s.rect(p.x0 - 2.0, p.y0 - 2.0, p.w + 4.0, p.h + 4.0, "#24382D", "");
    let mut labels: Vec<(f64, f64, String)> = vec![];
    for row in 0..12 {
        for col in 0..8 {
            let v = t.xt_grid[row * 8 + col];
            let x0 = row as f64 / 12.0 * 105.0;
            let y0 = col as f64 / 8.0 * 68.0;
            let (ax, ay) = p.p(x0, y0);
            let (bx, by) = p.p(x0 + 105.0 / 12.0, y0 + 8.5);
            let (rx, ry, rw, rh) = (ax.min(bx) + 1.0, ay.min(by) + 1.0, (bx - ax).abs() - 2.0, (by - ay).abs() - 2.0);
            let fill = if v > 0.0 { c.th.seq(sd, v / top) } else { "#12211A".to_string() };
            if v > 0.0 {
                s.tip = Some(format!("xT added from this square: {v:.3}"));
            }
            s.rect(rx, ry, rw, rh, &fill, "");
            if v >= 0.04 {
                labels.push((rx + rw / 2.0, ry + rh / 2.0, format!("{v:.2}")));
            }
        }
    }
    p.label_attack(&mut s);
    for (rank, (i, v)) in t.top_xt.iter().take(3).enumerate() {
        let e = &c.m.events[*i];
        let (a1, b1) = p.p(e.x, e.y);
        let (a2, b2) = p.p(e.end().0, e.end().1);
        s.layer = Some("Top 3 xT passes".to_string());
        s.tip = Some(format!("{}. {} {:02}:{:02} · xT {v:.3}", rank + 1, c.pname(sd, e.player), e.minute, e.second));
        s.arrow(a1, b1, a2, b2, "#F5F3EE", 2.0, 11.0, r#"opacity="0.55""#);
        s.circle(a1, b1, 4.0, "#F5F3EE", "none", 0.0, r#"opacity="0.7""#);
        s.layer = None;
    }
    s.layer_idx("Top 3 xT passes");
    // numbers on top of everything, on small dark pills
    for (cx, cy, txt) in &labels {
        s.rrect(cx - 25.0, cy - 14.0, 50.0, 28.0, 14.0, "#0B1510", "none", 0.0);
        s.t(*cx, cy + 7.0, txt, 19.0, 800, "#F5F3EE", Anchor::Middle);
    }
    // horizontal colour scale under the grid
    let (bx, by, bw) = (p.x0, p.y0 + p.h + 30.0, p.w);
    s.t(bx, by - 8.0, "xT per square", 13.0, 700, theme::TEXT, Anchor::Start);
    for i in 0..100 {
        s.rect(bx + i as f64 * bw / 100.0, by, bw / 100.0 + 0.6, 14.0, &c.th.seq(sd, i as f64 / 100.0), "");
    }
    for k in 0..=4 {
        let v = top * k as f64 / 4.0;
        let x = bx + k as f64 / 4.0 * bw;
        s.t(x, by + 34.0, &if k == 4 { format!("{v:.2}+") } else { format!("{v:.2}") }, 12.0, 600, theme::MUTED, if k == 0 { Anchor::Start } else if k == 4 { Anchor::End } else { Anchor::Middle });
    }
    let (px0, py0) = (980.0, 220.0);
    panel(&mut s, px0, py0, 560.0, 620.0);
    s.t(px0 + 24.0, py0 + 40.0, "TOP 10 XT PASSES", 14.0, 800, theme::TEXT, Anchor::Start);
    for (k, (i, v)) in t.top_xt.iter().enumerate() {
        let e = &c.m.events[*i];
        let yy = py0 + 90.0 + k as f64 * 46.0;
        s.t(px0 + 24.0, yy, &format!("{}. {} {:02}:{:02}", k + 1, c.pname(sd, e.player), e.minute, e.second), 15.0, 700, theme::TEXT, Anchor::Start);
        s.t(px0 + 536.0, yy, &format!("{v:.3}"), 15.0, 700, theme::TEXT, Anchor::End);
    }
    legend_item(&mut s, px0 + 24.0, py0 + 590.0, "line", "#F5F3EE", "Top 3 xT passes");
    footer(&mut s, c, "Positive xT is the increase in local zone threat from successful passes or carries, summed in 8 by 12 squares where each began. Both teams share one colour scale; it is not a probability of scoring. xT grid fitted on StatsBomb Open Data.");
    Board::new(&format!("21_xt_heatmap_{}", side_tag(sd)), "xT heatmap", s)
}

// ---------------------------------------------------------------- 22 goalkeeper frames
pub fn gk_frames(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Goalkeeper Goal Frames", "Every shot faced, plotted where it crossed the goal line · colour = outcome · shape = body part · size = xG", Scope::Full);
    let sem = &c.th.semantic;
    for (k, sd) in Side::both().iter().enumerate() {
        let g = &c.a.t(*sd).gk;
        let col = c.col(*sd).to_string();
        let bx = 40.0 + k as f64 * 780.0;
        s.h(bx, 210.0, &g.keeper_name.to_uppercase(), 30.0, &col, Anchor::Start);
        s.t(bx, 236.0, &format!("{} · {} shots faced · {} off target · {} woodwork · {} blocked", c.name(*sd), g.faced_total, g.off_target, g.woodwork, g.blocked), 13.0, 400, theme::MUTED, Anchor::Start);
        // goal: 7.32 x 2.44 -> u in [-1,1], z in [0,1]; plot area u in [-1.8,1.8], z in [0,1.9]
        let (gx0, gy0, gw, gh) = (bx + 40.0, 270.0, 660.0, 320.0);
        let pu = |u: f64| gx0 + (1.8 - u) / 3.6 * gw; // viewed from behind shooter: u>0 (left post) on the left
        let pz = |z: f64| gy0 + gh - z / 1.9 * gh;
        s.line(gx0, pz(0.0), gx0 + gw, pz(0.0), "#F5F3EE", 2.0, "");
        s.rect(pu(1.0), pz(1.0), pu(-1.0) - pu(1.0), pz(0.0) - pz(1.0), "#1A2E24", "");
        for i in 1..8 {
            let x = pu(1.0) + (pu(-1.0) - pu(1.0)) * i as f64 / 8.0;
            s.line(x, pz(1.0), x, pz(0.0), "#24382D", 1.0, "");
        }
        s.polyline(&[(pu(1.0), pz(0.0)), (pu(1.0), pz(1.0)), (pu(-1.0), pz(1.0)), (pu(-1.0), pz(0.0))], "#ffffff", 5.0, "");
        for &q in &g.shots_faced {
            let e = &c.m.events[q];
            let sh = e.shot.as_ref().unwrap();
            if sh.outcome == ShotOutcome::Blocked {
                continue;
            }
            let fill = match sh.outcome {
                ShotOutcome::Goal => sem.goal.as_str(),
                ShotOutcome::Saved => sem.saved.as_str(),
                ShotOutcome::Woodwork => sem.woodwork.as_str(),
                _ => sem.off_target.as_str(),
            };
            // off-target shots without placement sit outside the posts / above the bar
            let (u, z) = match (sh.goal_u, sh.goal_z) {
                (Some(u), Some(z)) => (u, z),
                (Some(u), None) => (u, 0.3),
                _ => {
                    let side = if e.y > 34.0 { 1.0 } else { -1.0 };
                    (side * 1.35, 0.6)
                }
            };
            let (x, y) = (pu(u.clamp(-1.75, 1.75)), pz(z.clamp(0.02, 1.85)));
            let r = 4.0 + 10.0 * sh.xg.sqrt();
            s.tip = Some(shot_tip(c, e));
            s.layer = Some(match sh.outcome {
                ShotOutcome::Goal => "GOAL",
                ShotOutcome::Saved => "SAVED",
                ShotOutcome::Woodwork => "WOODWORK",
                _ => "OFF TARGET",
            }.to_string());
            match sh.body {
                Body::Head => s.triangle(x, y, r + 1.0, fill, "none", 0.0),
                Body::Left => s.square(x, y, r * 0.8, fill, "none", 0.0),
                _ => s.circle(x, y, r, fill, "none", 0.0, ""),
            }
        }
        s.layer = None;
        let tiles = [
            ("FACED", g.on_target.to_string()),
            ("SAVES", g.saves.to_string()),
            ("CONCEDED", g.conceded.to_string()),
            ("PREVENTED", signed(g.prevented, 2)),
            ("SAVE RATE", if g.on_target > 0 { pct(g.saves as f64 / g.on_target as f64) } else { "–".into() }),
            ("CLAIMS", g.claims.to_string()),
            ("SWEEPS", g.sweeps.to_string()),
        ];
        let tw = 700.0 / 7.0;
        for (i, (l, v)) in tiles.iter().enumerate() {
            let x = bx + i as f64 * tw;
            s.rrect(x + 4.0, 640.0, tw - 8.0, 110.0, 6.0, "#12211A", "#24382D", 1.0);
            s.h(x + tw / 2.0, 700.0, v, 34.0, theme::TEXT, Anchor::Middle);
            s.t(x + tw / 2.0, 732.0, l, 10.5, 700, theme::MUTED, Anchor::Middle);
        }
        s.t(bx, 790.0, &format!("DISTRIBUTION   {} passes · avg {:.0} m · {:.0}% launched · {:.0}% completed", g.passes, g.avg_len, g.launched_pct * 100.0, g.completed_pct * 100.0), 13.5, 600, theme::TEXT, Anchor::Start);
        s.t(bx, 816.0, &format!("HANDLING   {} held · {} parried safe · {} into danger", g.held, g.parried_safe, g.into_danger), 13.5, 600, theme::TEXT, Anchor::Start);
    }
    let mut lx = 80.0;
    let y = 880.0;
    lx = legend_item(&mut s, lx, y, "circle", &sem.off_target, "OFF TARGET");
    lx = legend_item(&mut s, lx, y, "circle", &sem.woodwork, "WOODWORK");
    lx = legend_item(&mut s, lx, y, "circle", &sem.saved, "SAVED");
    lx = legend_item(&mut s, lx, y, "circle", &sem.goal, "GOAL");
    lx = legend_item(&mut s, lx, y, "circle", "#ffffff", "RIGHT FOOT");
    lx = legend_item(&mut s, lx, y, "square", "#ffffff", "LEFT FOOT");
    legend_item(&mut s, lx, y, "triangle", "#ffffff", "HEADER");
    let src = if c.m.events.iter().any(|e| e.shot.as_ref().map(|s| s.xgot_ext.is_some()).unwrap_or(false)) { "PSxG from FotMob/SofaScore xGOT." } else { "PSxG from the local placement model: no shot velocity or keeper position." };
    s.t(80.0, 920.0, &format!("Goal frame seen from behind the shooter · off-target shots sit outside the posts · blocked shots are excluded because they never reached the keeper. {src}"), 11.5, 400, theme::GREY, Anchor::Start);
    footer(&mut s, c, "Prevented = post-shot xG faced minus goals conceded, penalties and own goals left out; post-shot xG prices each attempt from its chance and where it crossed the line. Handling reads where each save sent the ball.");
    Board::new("22_goalkeeper_frames", "Goalkeeper goal frames", s)
}

// ---------------------------------------------------------------- 23 height & length
fn rolling_median(v: &[Option<f64>]) -> Vec<Option<f64>> {
    (0..v.len())
        .map(|i| {
            let mut w: Vec<f64> = v[i.saturating_sub(1)..(i + 2).min(v.len())].iter().filter_map(|x| *x).collect();
            if w.is_empty() {
                return None;
            }
            w.sort_by(|a, b| a.partial_cmp(b).unwrap());
            Some(w[w.len() / 2])
        })
        .collect()
}

pub fn height_length(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Defensive Action Height and Touch Spread", "Engagement height and compactness per five-minute window · the band shows which side was higher", Scope::Full);
    let hs = &c.a.height;
    let dur = c.a.duration_min.max(90.0);
    for (panel_i, (series, label, ylab)) in [(&hs.height, "Mean x of defensive actions", "halfway"), (&hs.length, "Length of the team, up the pitch (IQR)", "")].into_iter().enumerate() {
        let (x0, y0, w, h) = (110.0, 200.0 + panel_i as f64 * 430.0, 1380.0, 340.0);
        let py = |v: f64| y0 + h - (v - 10.0) / 65.0 * h;
        let px = |m: f64| x0 + m / dur * w;
        for v in [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0] {
            s.line(x0, py(v), x0 + w, py(v), theme::LINE, 1.0, "");
            s.t(x0 - 10.0, py(v) + 4.0, &format!("{v:.0}"), 12.0, 400, theme::MUTED, Anchor::End);
        }
        s.t(x0 - 60.0, y0 + h / 2.0, label, 12.0, 500, theme::MUTED, Anchor::Middle);
        if !ylab.is_empty() {
            s.line(x0, py(52.5), x0 + w, py(52.5), "#ffffff", 1.0, r#"opacity="0.5""#);
            s.t(x0 + w + 6.0, py(52.5) + 4.0, ylab, 11.0, 500, theme::MUTED, Anchor::Start);
        }
        let smoothed: Vec<Vec<Option<f64>>> = series.iter().map(|v| rolling_median(v)).collect();
        let mut allv: Vec<f64> = smoothed.iter().flatten().filter_map(|x| *x).collect();
        allv.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if let Some(med) = allv.get(allv.len() / 2) {
            s.line(x0, py(*med), x0 + w, py(*med), "#A3B3AA", 1.0, r#"stroke-dasharray="3 4""#);
            s.t(x0 + w + 6.0, py(*med) + 16.0, &format!("match median {:.0}", med), 11.0, 500, theme::MUTED, Anchor::Start);
        }
        // shaded band between the two lines, coloured by the higher side
        for i in 0..hs.windows.len().saturating_sub(1) {
            if let (Some(h0), Some(a0), Some(h1), Some(a1)) = (smoothed[0][i], smoothed[1][i], smoothed[0][i + 1], smoothed[1][i + 1]) {
                let higher = if h0 + h1 >= a0 + a1 { Side::Home } else { Side::Away };
                s.poly(&[(px(hs.windows[i]), py(h0)), (px(hs.windows[i + 1]), py(h1)), (px(hs.windows[i + 1]), py(a1)), (px(hs.windows[i]), py(a0))], c.col(higher), "none", 0.0, r#"opacity="0.18""#);
            }
        }
        for sd in Side::both() {
            for (i, v) in series[sd.idx()].iter().enumerate() {
                if let Some(v) = v {
                    s.circle(px(hs.windows[i]), py(*v), 2.5, c.col(sd), "none", 0.0, r#"opacity="0.45""#);
                }
            }
            let pts: Vec<(f64, f64)> = smoothed[sd.idx()].iter().enumerate().filter_map(|(i, v)| v.map(|v| (px(hs.windows[i]), py(v)))).collect();
            s.polyline(&pts, c.col(sd), 3.0, "");
        }
        if let Some(p2) = c.m.periods.iter().find(|p| p.period == 2) {
            let x = px(p2.start_t / 60.0);
            s.line(x, y0, x, y0 + h, "#ffffff", 1.0, r#"stroke-dasharray="3 5" opacity="0.4""#);
            s.t(x + 4.0, y0 + h - 8.0, "half time", 10.5, 500, theme::MUTED, Anchor::Start);
        }
        for mm in (0..=(dur as i32)).step_by(20) {
            s.t(px(mm as f64), y0 + h + 20.0, &mm.to_string(), 12.0, 400, theme::MUTED, Anchor::Middle);
        }
    }
    let mut lx = W - 420.0;
    lx = legend_item(&mut s, lx, 190.0, "line", c.col(Side::Home), &c.name(Side::Home));
    legend_item(&mut s, lx, 190.0, "line", c.col(Side::Away), &c.name(Side::Away));
    s.t(110.0, 1032.0, "Line = rolling median of three windows · dots = the five-minute samples behind it · Match minute", 12.0, 400, theme::GREY, Anchor::Start);
    footer(&mut s, c, "Lines are a rolling median of five-minute windows: how high each side made its defensive actions, and how far apart its touches were spread up the pitch. Higher on the first panel is a higher block; lower on the second is a more compact team.");
    Board::new("23_height_length", "Defensive height and length", s)
}

// ---------------------------------------------------------------- 24 chance quality vs finishing
pub fn chance_vs_finishing(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Chance Quality against Finishing", "Pre-shot xG, post-shot xG once the placement is known, and the goals that followed", Scope::Full);
    let (h, a) = (c.a.t(Side::Home), c.a.t(Side::Away));
    s.h(60.0, 210.0, "From chance to goal", 24.0, theme::TEXT, Anchor::Start);
    let groups = [("Chances created", h.npxg, a.npxg), ("Reached the frame", h.xg_on_target, a.xg_on_target), ("Value once struck", h.xgot, a.xgot), ("Goals", (h.goals - h.sp_pen.goals) as f64, (a.goals - a.sp_pen.goals) as f64)];
    let mx = groups.iter().map(|g| g.1.max(g.2)).fold(1.0, f64::max) * 1.15;
    let (x0, w) = (230.0, 470.0);
    for (i, (l, hv, av)) in groups.iter().enumerate() {
        let y = 250.0 + i as f64 * 160.0;
        s.t(x0 - 12.0, y + 42.0, l, 14.0, 500, theme::TEXT, Anchor::End);
        s.rect(x0, y, hv / mx * w, 36.0, c.col(Side::Home), "");
        s.t(x0 + hv / mx * w + 8.0, y + 24.0, &if i == 3 { format!("{hv:.0}") } else { format!("{hv:.2}") }, 14.0, 800, theme::TEXT, Anchor::Start);
        s.rect(x0, y + 44.0, av / mx * w, 36.0, c.col(Side::Away), "");
        s.t(x0 + av / mx * w + 8.0, y + 68.0, &if i == 3 { format!("{av:.0}") } else { format!("{av:.2}") }, 14.0, 800, theme::TEXT, Anchor::Start);
    }
    let mut lx = x0;
    lx = legend_item(&mut s, lx, 240.0 - 10.0, "square", c.col(Side::Home), &c.name(Side::Home));
    legend_item(&mut s, lx, 240.0 - 10.0, "square", c.col(Side::Away), &c.name(Side::Away));
    // dumbbells
    s.h(820.0, 210.0, "Every attempt that reached the frame", 24.0, theme::TEXT, Anchor::Start);
    s.t(820.0, 232.0, "A long line to the right is a well-struck ball · star = goal · the rest were saved", 12.0, 400, theme::MUTED, Anchor::Start);
    let mut on: Vec<(Side, usize, f64, f64, bool)> = vec![];
    for sd in Side::both() {
        for &q in &c.a.t(sd).shots {
            let sh = c.m.events[q].shot.as_ref().unwrap();
            if matches!(sh.outcome, ShotOutcome::Goal | ShotOutcome::Saved) && !sh.penalty {
                on.push((sd, q, sh.xg, sh.xgot_ext.or(sh.psxg).unwrap_or(0.0), sh.outcome == ShotOutcome::Goal));
            }
        }
    }
    on.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());
    on.truncate(16);
    let (dx0, dw) = (1000.0, 520.0);
    let dmax = on.iter().map(|o| o.2.max(o.3)).fold(0.3, f64::max).min(1.0);
    let row_h = (720.0 / on.len().max(1) as f64).min(46.0);
    for (i, (sd, q, xg, ps, goal)) in on.iter().enumerate() {
        let y = 275.0 + i as f64 * row_h;
        let e = &c.m.events[*q];
        s.t(dx0 - 12.0, y + 5.0, &format!("{} {}", c.pname(*sd, e.player), e.label_minute()), 13.0, 600, c.col(*sd), Anchor::End);
        let (a1, a2) = (dx0 + xg / dmax * dw, dx0 + ps / dmax * dw);
        s.tip = Some(format!("{} · chance {:.2} xG → struck {:.2} PSxG", shot_tip(c, e), xg, ps));
        s.line(a1, y, a2, y, c.col(*sd), 2.5, "");
        s.circle(a1, y, 6.0, "#0B1510", c.col(*sd), 2.0, "");
        if *goal {
            s.star(a2, y, 10.0, "#ffffff", "none");
        } else {
            s.circle(a2, y, 6.0, c.col(*sd), "none", 0.0, "");
        }
    }
    for k in 0..=4 {
        let v = dmax * k as f64 / 4.0;
        s.t(dx0 + v / dmax * dw, 275.0 + on.len() as f64 * row_h + 10.0, &format!("{v:.2}"), 11.0, 400, theme::MUTED, Anchor::Middle);
    }
    s.t(dx0 + dw / 2.0, 275.0 + on.len() as f64 * row_h + 32.0, "Chance (○) → value once struck (●)", 12.0, 500, theme::MUTED, Anchor::Middle);
    let verdict = |t: &TeamStats| if t.xgot >= t.xg_on_target { "struck them better than the chances were worth" } else { "struck them worse than the chances were worth" };
    s.t(60.0, 940.0, &format!("{}: {:.2} xG created, {:.2} of it reached the frame, worth {:.2} once struck, {} scored · {}.", c.name(Side::Home), h.npxg, h.xg_on_target, h.xgot, h.goals, verdict(h)), 16.0, 600, c.col(Side::Home), Anchor::Start);
    s.t(60.0, 970.0, &format!("{}: {:.2} xG created, {:.2} of it reached the frame, worth {:.2} once struck, {} scored · {}.", c.name(Side::Away), a.npxg, a.xg_on_target, a.xgot, a.goals, verdict(a)), 16.0, 600, c.col(Side::Away), Anchor::Start);
    s.t(60.0, 1004.0, "Post-shot xG is a local placement estimate. It has no shot velocity and no goalkeeper position, so it prices where the ball went, not how hard it was to stop.", 12.0, 400, theme::GREY, Anchor::Start);
    footer(&mut s, c, "Pre-shot xG prices the chance, post-shot xG prices the strike once its placement is known, and goals are what the keeper let through. Penalties excluded.");
    Board::new("24_chance_vs_finishing", "Chance quality against finishing", s)
}

// ---------------------------------------------------------------- 25 press triggers
pub fn press_triggers(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Press Triggers & Rest Defence", "Recorded actions preceding high regains and opponent shots within 12 seconds after losses", Scope::Full);
    for (k, sd) in Side::both().iter().enumerate() {
        let t = c.a.t(*sd);
        let col = c.col(*sd).to_string();
        let bx = 60.0 + k as f64 * 760.0;
        s.h(bx, 215.0, &c.name(*sd).to_uppercase(), 30.0, &col, Anchor::Start);
        let trig_n: usize = t.press_triggers.iter().map(|x| x.1).sum();
        s.t(bx, 242.0, &format!("{} of {} high regains followed an opponent action", trig_n, t.high_regains.len()), 13.0, 400, theme::MUTED, Anchor::Start);
        let mx = t.press_triggers.iter().map(|x| x.1).max().unwrap_or(1) as f64;
        let (lx, w) = (bx + 140.0, 480.0);
        if t.press_triggers.is_empty() {
            s.t(lx, 320.0, "none recorded", 14.0, 400, theme::MUTED, Anchor::Start);
        }
        for (i, (lab, n)) in t.press_triggers.iter().enumerate() {
            let y = 280.0 + i as f64 * 64.0;
            s.t(lx - 10.0, y + 28.0, lab, 14.0, 500, theme::TEXT, Anchor::End);
            s.rect(lx, y, *n as f64 / mx * w, 42.0, &col, "");
            s.t(lx + *n as f64 / mx * w + 8.0, y + 27.0, &format!("{} ({:.0}%)", n, *n as f64 / t.high_regains.len().max(1) as f64 * 100.0), 13.0, 800, theme::TEXT, Anchor::Start);
        }
        let ty = 780.0;
        s.t(bx, ty, "REST DEFENCE & SECOND BALLS", 13.0, 800, theme::TEXT, Anchor::Start);
        let rows = [
            ("Open-play losses", t.open_play_losses.to_string()),
            ("Opponent shot within 12 seconds", format!("{}/{}", t.losses_to_shot, t.open_play_losses)),
            ("Second balls won", format!("{}/{} ({:.0}%)", t.second_balls.0, t.second_balls.1, t.second_balls.0 as f64 / t.second_balls.1.max(1) as f64 * 100.0)),
        ];
        for (i, (l, v)) in rows.iter().enumerate() {
            let y = ty + 44.0 + i as f64 * 44.0;
            s.t(bx, y, l, 15.0, 400, theme::TEXT, Anchor::Start);
            s.t(bx + 640.0, y, v, 15.0, 800, theme::TEXT, Anchor::End);
            s.line(bx, y + 14.0, bx + 640.0, y + 14.0, theme::LINE, 1.0, "");
        }
    }
    footer(&mut s, c, "Pressing-trigger counts are event-based review cues. They identify sequences to inspect and cannot confirm coordinated pressing without video.");
    Board::new("25_press_triggers", "Press triggers and rest defence", s)
}

// ---------------------------------------------------------------- 26 progression scatter
pub fn progression_scatter(c: &Ctx) -> Board {
    let mut s = Svg::new(W, 1000.0);
    let el: Vec<&PlayerStats> = c.a.players.iter().filter(|p| p.minutes >= 30.0 && p.role != Some(Role::Goalkeeper)).collect();
    header(&mut s, c, "Progression and Chance Creation", &format!("{} observations | players with at least 30 minutes, match totals", el.len()), Scope::Full);
    let (x0, y0, w, h) = (140.0, 210.0, 1360.0, 620.0);
    let xs: Vec<f64> = el.iter().map(|p| (p.progressive_passes + p.progressive_carries) as f64).collect();
    let ys: Vec<f64> = el.iter().map(|p| p.inferred_xa).collect();
    let xmax = xs.iter().cloned().fold(4.0, f64::max) * 1.08;
    let ymax = ys.iter().cloned().fold(0.3, f64::max) * 1.1;
    let px = |v: f64| x0 + v / xmax * w;
    let py = |v: f64| y0 + h - v / ymax * h;
    for k in 0..=5 {
        let v = ymax * k as f64 / 5.0;
        s.line(x0, py(v), x0 + w, py(v), theme::LINE, 1.0, "");
        s.t(x0 - 10.0, py(v) + 4.0, &format!("{v:.1}"), 12.0, 400, theme::MUTED, Anchor::End);
    }
    for k in 0..=5 {
        let v = (xmax * k as f64 / 5.0).round();
        s.t(px(v), y0 + h + 22.0, &format!("{v:.0}"), 12.0, 400, theme::MUTED, Anchor::Middle);
    }
    let med = |v: &[f64]| {
        let mut v = v.to_vec();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let (mx, my) = (med(&xs), med(&ys));
    s.line(px(mx), y0, px(mx), y0 + h, "#A3B3AA", 1.0, r#"stroke-dasharray="3 4""#);
    s.line(x0, py(my), x0 + w, py(my), "#A3B3AA", 1.0, r#"stroke-dasharray="3 4""#);
    s.t(x0 + w / 2.0, y0 + h + 48.0, "Progressive passes + explicit carries", 14.0, 600, theme::TEXT, Anchor::Middle);
    s.t(x0, y0 - 26.0, "Inferred xA", 14.0, 600, theme::TEXT, Anchor::Start);
    // simple label repel: shift labels vertically when overlapping
    let mut placed: Vec<(f64, f64)> = vec![];
    for (i, p) in el.iter().enumerate() {
        let (x, y) = (px(xs[i]), py(ys[i]));
        let sd = p.side.unwrap();
        s.tip = Some(format!("{} · {} progressive actions · {:.2} inferred xA · {:.0} min", p.name, xs[i], ys[i], p.minutes));
        if sd == Side::Home {
            s.circle(x, y, 8.0, c.col(sd), "#000", 1.0, "");
        } else {
            s.square(x, y, 7.0, c.col(sd), "#000", 1.0);
        }
        let mut ly = y - 13.0;
        let lw = text_w(&p.short, 12.0, false);
        let mut tries = 0;
        while placed.iter().any(|(ax, ay)| (ax - x).abs() < lw * 0.9 && (ay - ly).abs() < 14.0) && tries < 8 {
            ly = if tries % 2 == 0 { y + 24.0 + 14.0 * (tries / 2) as f64 } else { y - 13.0 - 14.0 * ((tries + 1) / 2) as f64 };
            tries += 1;
        }
        // a label that still sits on another one is left off: the point keeps its tap tooltip with the name
        let clash = placed.iter().any(|(ax, ay)| (ax - x).abs() < lw * 1.1 && (ay - ly).abs() < 17.0);
        if !clash {
            placed.push((x, ly));
            s.t(x, ly, &p.short, 12.0, 600, theme::TEXT, Anchor::Middle);
        }
    }
    // legend sits under the axis title, clear of the points
    let mut lx = x0;
    lx = legend_item(&mut s, lx, y0 + h + 86.0, "circle", c.col(Side::Home), &c.short(Side::Home));
    legend_item(&mut s, lx, y0 + h + 86.0, "square", c.col(Side::Away), &c.short(Side::Away));
    footer(&mut s, c, "Dotted lines: eligible-player medians. Inferred xA = shot xG credited to a prior successful key pass in the same possession (≤ 15 seconds).");
    Board::new("26_progression_scatter", "Progression and chance creation", s)
}

// ---------------------------------------------------------------- 27 funnel
pub fn funnel(c: &Ctx) -> Board {
    let mut s = Svg::new(W, 1130.0);
    header(&mut s, c, "From Possession to a Shot on Target", "Possessions reaching each stage · every bar on one shared scale", Scope::Full);
    let labels = ["Possessions", "Reached final third", "Entered box", "Attempted shot", "On target"];
    let mx = c.a.t(Side::Home).funnel[0].max(c.a.t(Side::Away).funnel[0]).max(1) as f64;
    // one stage per card, all cards the same width so the phone layout keeps one bar scale
    let (x0, cw) = (400.0, 800.0);
    let bar_x = x0 + 220.0;
    let bar_w = cw - 220.0 - 28.0;
    let mut y = 170.0;
    for (i, l) in labels.iter().enumerate() {
        let h = if i == 0 { 128.0 } else { 160.0 };
        panel(&mut s, x0, y, cw, h);
        s.t(x0 + cw / 2.0, y + 34.0, &l.to_uppercase(), 17.0, 700, theme::TEXT, Anchor::Middle);
        for (k, sd) in Side::both().iter().enumerate() {
            let f = &c.a.t(*sd).funnel;
            let ry = y + 52.0 + k as f64 * 46.0;
            let w = (f[i] as f64 / mx * bar_w).max(3.0);
            s.t(x0 + 24.0, ry + 20.0, &c.name(*sd).to_uppercase(), 14.0, 700, c.col(*sd), Anchor::Start);
            s.h(x0 + 200.0, ry + 23.0, &f[i].to_string(), 28.0, theme::TEXT, Anchor::End);
            s.tip = Some(format!("{} · {}: {} of {} possessions", c.name(*sd), l, f[i], f[0]));
            s.rect(bar_x, ry + 4.0, bar_w, 22.0, "#1A2E24", "");
            s.rect(bar_x, ry + 4.0, w, 22.0, c.col(*sd), "");
        }
        if i > 0 {
            let conv = |sd: Side| {
                let f = &c.a.t(sd).funnel;
                if f[i - 1] > 0 { format!("{:.0}%", f[i] as f64 / f[i - 1] as f64 * 100.0) } else { "–".into() }
            };
            let runs = vec![
                (format!("Stage conversion from {}:  ", labels[i - 1].to_lowercase()), theme::MUTED.to_string()),
                (format!("{} {}", c.short(Side::Home), conv(Side::Home)), c.col(Side::Home).to_string()),
                ("   ·   ".to_string(), theme::MUTED.to_string()),
                (format!("{} {}", c.short(Side::Away), conv(Side::Away)), c.col(Side::Away).to_string()),
            ];
            s.text_runs(x0 + 24.0, y + h - 18.0, &runs, 13.0, 600, Anchor::Start, BODY);
        }
        y += h + 22.0;
    }
    footer(&mut s, c, "Each bar counts possessions that reached the stage; the conversion line is the share of the previous stage that got this far. The box stage counts possessions with any touch in the penalty area, so it runs higher than the box-entry figure.");
    Board::new("27_possession_funnel", "Possession funnel", s)
}

// ---------------------------------------------------------------- 28 entry routes
pub fn entry_routes(c: &Ctx) -> Board {
    let mut s = Svg::new(W, 900.0);
    header(&mut s, c, "Box-Entry Routes and Their Next Outcome", "A shot must follow the entry in the same possession; multiple entries can precede one shot", Scope::Full);
    let mx = Side::both().iter().flat_map(|&sd| [Lane::Left, Lane::Centre, Lane::Right].map(|l| c.a.t(sd).entries.iter().filter(|e| e.lane == l).count())).max().unwrap_or(1).max(4) as f64 + 2.0;
    for (k, sd) in Side::both().iter().enumerate() {
        let t = c.a.t(*sd);
        let bx = 60.0 + k as f64 * 770.0;
        s.h(bx + 340.0, 220.0, &c.name(*sd), 24.0, theme::TEXT, Anchor::Middle);
        let (x0, w) = (bx + 90.0, 560.0);
        for (i, (lane, lab)) in [(Lane::Left, "Left"), (Lane::Centre, "Centre"), (Lane::Right, "Right")].iter().enumerate() {
            let y = 300.0 + i as f64 * 160.0;
            let all = t.entries.iter().filter(|e| e.lane == *lane).count();
            let sh = t.entries.iter().filter(|e| e.lane == *lane && e.shot_followed).count();
            s.t(x0 - 14.0, y + 5.0, lab, 14.0, 500, theme::TEXT, Anchor::End);
            s.line(x0, y, x0 + all as f64 / mx * w, y, "#A3B3AA", 4.0, "");
            s.circle(x0 + all as f64 / mx * w, y, 9.0, "#F5F3EE", "none", 0.0, "");
            s.tip = Some(format!("{} · {} lane: {} entries, {} followed by a shot", c.name(*sd), lab, all, sh));
            s.square(x0 + sh as f64 / mx * w, y, 8.0, c.col(*sd), "none", 0.0);
            s.t(x0 + sh as f64 / mx * w, y - 18.0, &format!("{} shot{} · {:.0}%", sh, if sh == 1 { "" } else { "s" }, if all > 0 { sh as f64 / all as f64 * 100.0 } else { 0.0 }), 13.0, 600, theme::TEXT, Anchor::Middle);
            s.t(x0 + all as f64 / mx * w + 14.0, y + 24.0, &format!("{all} entr{}", if all == 1 { "y" } else { "ies" }), 13.0, 500, theme::TEXT, Anchor::Start);
        }
        for g in (0..=(mx as i32)).step_by(2) {
            s.t(x0 + g as f64 / mx * w, 760.0, &g.to_string(), 12.0, 400, theme::MUTED, Anchor::Middle);
        }
        s.t(x0 + w / 2.0, 784.0, "Entries and entries followed by a shot", 12.0, 500, theme::MUTED, Anchor::Middle);
        legend_item(&mut s, bx + 480.0, 260.0, "circle", "#F5F3EE", "All entries");
        legend_item(&mut s, bx + 480.0, 282.0, "square", c.col(*sd), "Shot followed");
        if t.crosses.1 > 0 {
            s.t(bx + 90.0, 812.0, &format!("Crosses: {} of {} found a teammate", t.crosses.0, t.crosses.1), 12.0, 400, theme::GREY, Anchor::Start);
        }
    }
    footer(&mut s, c, "The lane identifies entry origin; a following shot is descriptive, not proof of cause.");
    Board::new("28_entry_routes", "Box-entry routes", s)
}

// ---------------------------------------------------------------- 29/30 competition context
fn context_scatter(c: &Ctx, rows: &[TeamMatchRow], id: &str, title: &str, sub: &str, fx: impl Fn(&TeamMatchRow) -> f64, fy: impl Fn(&TeamMatchRow) -> f64, xl: &str, yl: &str) -> Board {
    let mut s = Svg::new(W, 900.0);
    header(&mut s, c, title, sub, Scope::Full);
    let (x0, y0, w, h) = (130.0, 200.0, 1380.0, 560.0);
    if rows.len() < 3 {
        s.h(W / 2.0, 480.0, "Not enough stored matches yet", 36.0, theme::TEXT, Anchor::Middle);
        s.t(W / 2.0, 520.0, "Run `matchhub season` (or build more matches) to fill the competition store.", 15.0, 400, theme::MUTED, Anchor::Middle);
        footer(&mut s, c, "History is descriptive; input-model provenance is listed in each manifest.");
        return Board::new(id, title, s);
    }
    let xs: Vec<f64> = rows.iter().map(&fx).collect();
    let ys: Vec<f64> = rows.iter().map(&fy).collect();
    let (xmin, xmax) = (xs.iter().cloned().fold(f64::INFINITY, f64::min) * 0.9, xs.iter().cloned().fold(0.0, f64::max) * 1.05);
    let ymax = ys.iter().cloned().fold(1.0, f64::max) * 1.1;
    let px = |v: f64| x0 + (v - xmin) / (xmax - xmin).max(1e-9) * w;
    let py = |v: f64| y0 + h - v / ymax * h;
    for k in 0..=4 {
        let v = ymax * k as f64 / 4.0;
        s.line(x0, py(v), x0 + w, py(v), theme::LINE, 1.0, "");
        s.t(x0 - 10.0, py(v) + 4.0, &format!("{v:.1}"), 12.0, 400, theme::MUTED, Anchor::End);
        let xv = xmin + (xmax - xmin) * k as f64 / 4.0;
        s.t(px(xv), y0 + h + 22.0, &format!("{xv:.0}"), 12.0, 400, theme::MUTED, Anchor::Middle);
    }
    s.t(x0 + w / 2.0, y0 + h + 48.0, xl, 14.0, 600, theme::TEXT, Anchor::Middle);
    s.t(x0 - 70.0, y0 + h / 2.0, yl, 14.0, 600, theme::TEXT, Anchor::Middle);
    let names = [c.m.team(Side::Home).name.clone(), c.m.team(Side::Away).name.clone()];
    let mut later: Vec<(f64, f64, Side, bool)> = vec![];
    for (i, r) in rows.iter().enumerate() {
        let side = names.iter().position(|n| *n == r.team).map(|k| if k == 0 { Side::Home } else { Side::Away });
        match side {
            Some(sd) => later.push((px(xs[i]), py(ys[i]), sd, r.match_id == c.m.id)),
            None => {
                s.tip = Some(format!("{} v {} · {} · {:.1}, {:.1}", r.team, r.opponent, r.date, xs[i], ys[i]));
                s.circle(px(xs[i]), py(ys[i]), 4.5, "#A3B3AA", "none", 0.0, r#"opacity="0.7""#);
            }
        }
    }
    for (x, y, sd, this) in later {
        if this {
            s.circle(x, y, 11.0, c.col(sd), "#ffffff", 2.5, "");
            s.t(x + 14.0, y - 10.0, "this match", 12.0, 600, theme::TEXT, Anchor::Start);
        } else {
            s.circle(x, y, 5.5, c.col(sd), "none", 0.0, "");
        }
    }
    let lx = x0 + 20.0;
    legend_item(&mut s, lx, y0 + 20.0, "circle", c.col(Side::Home), &format!("{} · other matches", c.name(Side::Home)));
    legend_item(&mut s, lx, y0 + 44.0, "ring", c.col(Side::Home), &format!("{} · this match", c.name(Side::Home)));
    legend_item(&mut s, lx, y0 + 68.0, "circle", c.col(Side::Away), &format!("{} · other matches", c.name(Side::Away)));
    legend_item(&mut s, lx, y0 + 92.0, "ring", c.col(Side::Away), &format!("{} · this match", c.name(Side::Away)));
    footer(&mut s, c, "History is descriptive; input-model provenance is listed in each manifest. Only rows with the current metric_version are plotted.");
    Board::new(id, title, s)
}

pub fn context_territory(c: &Ctx, rows: &[TeamMatchRow]) -> Board {
    context_scatter(c, rows, "29_context_territory", "Territory and Chances Across the Competition", &format!("Comparable metric version, one point per team-match, not the two teams of one fixture · {} team-matches", rows.len()), |r| r.field_tilt * 100.0, |r| r.xg, "Field tilt (%)", "xG")
}
pub fn context_pressing(c: &Ctx, rows: &[TeamMatchRow]) -> Board {
    let rows: Vec<TeamMatchRow> = rows.iter().filter(|r| r.ppda.is_some()).cloned().collect();
    context_scatter(c, &rows, "30_context_pressing", "Pressing Frequency and Danger After Losses", "Same competition and metric version | one point per team-match", |r| r.ppda.unwrap_or(0.0).min(32.0), |r| r.dangerous_counter_pct, "PPDA (lower = fewer opponent passes per action)", "Dangerous counters / advanced losses (%)")
}

// ---------------------------------------------------------------- 31 player cards
struct Measure {
    label: &'static str,
    value: f64,
    display: String,
}

fn measures(p: &PlayerStats) -> Vec<Measure> {
    let m = |label: &'static str, value: f64, display: String| Measure { label, value, display };
    match p.role.unwrap_or(Role::Midfielder) {
        Role::Goalkeeper => vec![
            m("Saves", p.saves as f64, p.saves.to_string()),
            m("Claims", p.claims as f64, p.claims.to_string()),
            m("Sweeps", p.sweeps as f64, p.sweeps.to_string()),
            m("Progressive pass share", p.prog_pass_share, pct(p.prog_pass_share)),
            m("Passes", p.passes as f64, p.passes.to_string()),
            m("Completion", if p.passes > 0 { p.passes_completed as f64 / p.passes as f64 } else { 0.0 }, pct(if p.passes > 0 { p.passes_completed as f64 / p.passes as f64 } else { 0.0 })),
            m("Progression metres", p.progression_m, format!("{:.0}", p.progression_m)),
            m("xGChain", p.xgchain, format!("{:.2}", p.xgchain)),
        ],
        Role::Defender => vec![
            m("Progressive pass share", p.prog_pass_share, pct(p.prog_pass_share)),
            m("PAdj defensive actions", p.padj_def, format!("{:.2}", p.padj_def)),
            m("Aerials won", p.aerials.0 as f64, p.aerials.0.to_string()),
            m("Defensive height", p.def_height, format!("{:.0}", p.def_height)),
            m("Progression metres", p.progression_m, format!("{:.0}", p.progression_m)),
            m("Line-breaking passes", p.line_breaks as f64, p.line_breaks.to_string()),
            m("xGChain", p.xgchain, format!("{:.2}", p.xgchain)),
            m("Recoveries", p.recoveries as f64, p.recoveries.to_string()),
        ],
        Role::Midfielder => vec![
            m("Progressive pass share", p.prog_pass_share, pct(p.prog_pass_share)),
            m("PAdj defensive actions", p.padj_def, format!("{:.2}", p.padj_def)),
            m("Progression metres", p.progression_m, format!("{:.0}", p.progression_m)),
            m("Line-breaking passes", p.line_breaks as f64, p.line_breaks.to_string()),
            m("xA per 100 passes", p.xa_per100, format!("{:.2}", p.xa_per100)),
            m("xGChain", p.xgchain, format!("{:.2}", p.xgchain)),
            m("Final-third receptions", p.ft_receptions as f64, p.ft_receptions.to_string()),
            m("Box entries", p.box_entries as f64, p.box_entries.to_string()),
        ],
        Role::Forward => vec![
            m("xG", p.xg, format!("{:.2}", p.xg)),
            m("Inferred xA", p.inferred_xa, format!("{:.2}", p.inferred_xa)),
            m("xGChain", p.xgchain, format!("{:.2}", p.xgchain)),
            m("Box entries", p.box_entries as f64, p.box_entries.to_string()),
            m("Final-third receptions", p.ft_receptions as f64, p.ft_receptions.to_string()),
            m("Progressive pass share", p.prog_pass_share, pct(p.prog_pass_share)),
            m("Take-ons won", p.take_ons.0 as f64, p.take_ons.0.to_string()),
            // a forward is measured on attacking output only
            m("Shots", p.shots as f64, p.shots.to_string()),
        ],
    }
}

fn percentile(v: f64, pool: &[f64]) -> f64 {
    if pool.is_empty() {
        return 50.0;
    }
    let below = pool.iter().filter(|x| **x < v).count() as f64;
    let eq = pool.iter().filter(|x| **x == v).count() as f64;
    ((below + 0.5 * eq) / pool.len() as f64 * 100.0).clamp(0.0, 100.0)
}

/// Greedy word wrap by estimated character width.
fn wrap_text(text: &str, size: f64, max_w: f64) -> Vec<String> {
    let per = ((max_w / (size * 0.56)) as usize).max(8);
    let mut lines = vec![];
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > per {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Player card, composed natively for a phone column (640 wide): hero, scoreboard, 2x2 key figures,
/// percentile ring, touch map, tactical snapshot and grouped stat sections.
pub fn player_card(c: &Ctx, p: &PlayerStats) -> Board {
    const PW: f64 = 640.0;
    const PAPER: &str = "#F5F3EE";
    const SAGE: &str = "#6BA68B";
    const CARD: &str = "#12211A";
    const CARD_HI: &str = "#1A2E24";
    const HAIR: &str = "#24382D";
    const SOFT: &str = "#A3B3AA";
    let mut s = Svg::new(PW, 4000.0);
    let sd = p.side.unwrap();
    let col = c.col(sd).to_string();
    let role = p.role.unwrap_or(Role::Midfielder);
    // peer pool: same role, 30+ min, at least 4 others; else all 30+ min players
    let same: Vec<&PlayerStats> = c.a.players.iter().filter(|q| q.minutes >= 30.0 && q.role == Some(role)).collect();
    let (pool, pool_note) = if same.len() >= 5 {
        (same, format!("Ranked against {} players in the same role (30+ min). The grey ring marks the pool average.", c.a.players.iter().filter(|q| q.minutes >= 30.0 && q.role == Some(role)).count() - 1))
    } else {
        let all: Vec<&PlayerStats> = c.a.players.iter().filter(|q| q.minutes >= 30.0).collect();
        let n = all.len();
        (all, format!("Ranked against all {} players over 30 minutes (fewer than 4 peers in role). The grey ring marks the pool average.", n))
    };
    let ms = measures(p);
    let pcts: Vec<f64> = ms.iter().enumerate().map(|(i, m)| {
        let pv: Vec<f64> = pool.iter().filter(|q| q.id != p.id).map(|q| measures(q)[i].value).collect();
        percentile(m.value, &pv)
    }).collect();
    let pool_avg: Vec<f64> = (0..ms.len()).map(|i| {
        let pv: Vec<f64> = pool.iter().map(|q| measures(q)[i].value).collect();
        let avg = pv.iter().sum::<f64>() / pv.len().max(1) as f64;
        percentile(avg, &pv)
    }).collect();

    // ---- hero: the player once, then the scoreboard pill
    let mut y = 64.0;
    s.h(24.0, y, &p.name, 46.0, PAPER, Anchor::Start);
    y += 30.0;
    s.t(24.0, y, &format!("{} · {} · Touches {} · Shots {}", c.name(sd), role.label(), p.touches, p.shots), 15.0, 500, SOFT, Anchor::Start);
    y += 22.0;
    s.rrect(20.0, y, PW - 40.0, 64.0, 32.0, CARD, "none", 0.0);
    let (hc, ac) = (c.col(Side::Home).to_string(), c.col(Side::Away).to_string());
    super::frame::crest(&mut s, 56.0, y + 32.0, 20.0, &c.crests[0], &c.name(Side::Home), &hc, "crest:0");
    super::frame::crest(&mut s, PW - 56.0, y + 32.0, 20.0, &c.crests[1], &c.name(Side::Away), &ac, "crest:1");
    s.t(88.0, y + 38.0, &fit_name(c.m, Side::Home, 16.0, 150.0), 16.0, 700, PAPER, Anchor::Start);
    s.t(PW - 88.0, y + 38.0, &fit_name(c.m, Side::Away, 16.0, 150.0), 16.0, 700, PAPER, Anchor::End);
    s.h(PW / 2.0, y + 42.0, &format!("{} – {}", c.m.team(Side::Home).score, c.m.team(Side::Away).score), 32.0, PAPER, Anchor::Middle);
    y += 64.0 + 18.0;
    // team-colour rule under the pill
    s.rect(20.0, y - 9.0, (PW - 40.0) / 2.0, 3.0, &hc, "");
    s.rect(PW / 2.0, y - 9.0, (PW - 40.0) / 2.0, 3.0, &ac, "");
    y += 14.0;

    // ---- 2x2 key figures: three strongest measures and minutes
    let mut order: Vec<usize> = (0..ms.len()).collect();
    order.sort_by(|a, b| ((ms[*b].value > 0.0) as u8).cmp(&((ms[*a].value > 0.0) as u8)).then(pcts[*b].partial_cmp(&pcts[*a]).unwrap()));
    let n_pool = pool.len();
    let mut kpis: Vec<(String, String, String)> = order.iter().take(3).map(|&i| {
        let rank = pool.iter().filter(|q| measures(q)[i].value > ms[i].value).count() + 1;
        (ms[i].display.clone(), ms[i].label.to_string(), format!("{} of {} in pool", rank, n_pool))
    }).collect();
    kpis.push((format!("{:.0}", p.minutes), "Minutes played".into(), String::new()));
    let (cw, ch, gap) = ((PW - 40.0 - 12.0) / 2.0, 128.0, 12.0);
    for (k, (v, l, r)) in kpis.iter().enumerate() {
        let x = 20.0 + (k % 2) as f64 * (cw + gap);
        let yy = y + (k / 2) as f64 * (ch + gap);
        s.rrect(x, yy, cw, ch, 16.0, CARD, "none", 0.0);
        // value box ends ~16 px under its baseline; the label starts below that, the badge below the label
        s.h(x + 18.0, yy + 56.0, v, 54.0, if k < 3 { SAGE } else { PAPER }, Anchor::Start);
        s.t(x + 18.0, yy + 92.0, l, 14.0, 600, PAPER, Anchor::Start);
        if !r.is_empty() {
            s.rrect(x + 18.0, yy + 102.0, 112.0, 20.0, 10.0, CARD_HI, "none", 0.0);
            s.t(x + 74.0, yy + 116.0, r, 11.5, 500, SOFT, Anchor::Middle);
        }
    }
    y += 2.0 * ch + gap + 34.0;

    // ---- percentile ring (team colour is the wedge fill only)
    s.t(24.0, y, "Percentile ring", 17.0, 700, PAPER, Anchor::Start);
    y += 28.0;
    let (cx, cy, r0, r1) = (PW / 2.0, y + 190.0, 34.0, 132.0);
    let n = ms.len();
    for i in 0..n {
        let a0 = -std::f64::consts::FRAC_PI_2 + i as f64 / n as f64 * std::f64::consts::TAU + 0.02;
        let a1 = a0 + std::f64::consts::TAU / n as f64 - 0.04;
        let wedge = |rr: f64| {
            let mut d = format!("M {:.1} {:.1}", cx + r0 * a0.cos(), cy + r0 * a0.sin());
            for k in 0..=12 {
                let a = a0 + (a1 - a0) * k as f64 / 12.0;
                d.push_str(&format!(" L {:.1} {:.1}", cx + rr * a.cos(), cy + rr * a.sin()));
            }
            d.push_str(&format!(" L {:.1} {:.1} Z", cx + r0 * a1.cos(), cy + r0 * a1.sin()));
            d
        };
        s.path(&wedge(r1), CARD, HAIR, 1.0, "");
        let rr = r0 + (r1 - r0) * pcts[i] / 100.0;
        s.tip = Some(format!("{}: {} · percentile {:.0} (pool average {:.0})", ms[i].label, ms[i].display, pcts[i], pool_avg[i]));
        s.path(&wedge(rr), &col, "none", 0.0, r#"opacity="0.95""#);
        let ra = r0 + (r1 - r0) * pool_avg[i] / 100.0;
        let pts: Vec<(f64, f64)> = (0..=12).map(|k| {
            let a = a0 + (a1 - a0) * k as f64 / 12.0;
            (cx + ra * a.cos(), cy + ra * a.sin())
        }).collect();
        s.polyline(&pts, "#A3B3AA", 2.0, "");
        // labels sit outside the ring, anchored away from the centre so they never cross it
        let am = (a0 + a1) / 2.0;
        let (lx, ly) = (cx + (r1 + 16.0) * am.cos(), cy + (r1 + 16.0) * am.sin());
        let anchor = if am.cos() > 0.3 { Anchor::Start } else if am.cos() < -0.3 { Anchor::End } else { Anchor::Middle };
        let dy = if am.sin() < -0.5 { -22.0 } else if am.sin() > 0.5 { 14.0 } else { -4.0 };
        let is_top = order.iter().take(3).any(|&o| o == i);
        s.t(lx, ly + dy, ms[i].label, 12.0, 600, PAPER, anchor);
        s.t(lx, ly + dy + 15.0, &ms[i].display, 13.0, 800, if is_top { SAGE } else { SOFT }, anchor);
    }
    y = cy + r1 + 62.0;
    let weakest = *order.last().unwrap();
    for l in wrap_text(&pool_note, 12.5, PW - 48.0) {
        s.t(24.0, y, &l, 12.5, 400, SOFT, Anchor::Start);
        y += 17.0;
    }
    y += 4.0;
    for l in wrap_text(&format!("Shortest wedge: {} at percentile {:.0}, where this performance differed most from the pool.", ms[weakest].label.to_lowercase(), pcts[weakest]), 12.5, PW - 48.0) {
        s.t(24.0, y, &l, 12.5, 400, SOFT, Anchor::Start);
        y += 17.0;
    }
    y += 26.0;

    // ---- touch map
    s.t(24.0, y, "Where he had the ball and where he defended", 17.0, 700, PAPER, Anchor::Start);
    y += 18.0;
    // vertical pitch, full card width, attacking up
    let pm = Pitch::up(20.0, y, PW - 40.0, 720.0, 0.0);
    let ev = &c.m.events;
    let mine: Vec<&Event> = ev.iter().filter(|e| e.player == Some(p.id) && e.side == sd).collect();
    let touches: Vec<(f64, f64)> = mine.iter().filter(|e| e.kind.is_on_ball() && !matches!(e.kind, Kind::Foul)).map(|e| (e.x, e.y)).collect();
    kde(&mut s, &pm, &touches, &c.th.semantic.touch, 6.0, 0.6);
    pm.draw(&mut s);
    for e in &mine {
        let (x, yy) = pm.p(e.x, e.y);
        if matches!(e.kind, Kind::Tackle | Kind::Interception | Kind::Recovery | Kind::Clearance | Kind::Block | Kind::Challenge) {
            s.diamond(x, yy, 6.5, &c.th.semantic.defending, "#000", 0.8);
        } else if e.kind == Kind::Shot {
            s.star(x, yy, 10.0, "#ffffff", "none");
        } else if e.kind.is_on_ball() {
            s.circle(x, yy, 2.6, "#ffffff", "none", 0.0, r#"opacity="0.75""#);
        }
        if e.kind == Kind::Pass && e.success {
            let (ex, ey) = e.end();
            let into_box = !crate::derive::in_box(e.x, e.y) && crate::derive::in_box(ex, ey);
            let (a2, b2) = pm.p(ex, ey);
            if into_box {
                s.arrow(x, yy, a2, b2, "#ffffff", 2.4, 9.0, "");
            } else if ex - e.x >= 15.0 {
                s.line(x, yy, a2, b2, "#F5F3EE", 1.2, r#"opacity="0.6""#);
            }
        }
    }
    if p.def_height > 0.0 {
        // defensive height is a distance up the pitch: a horizontal line on the vertical pitch
        let (_, hy) = pm.p(p.def_height, 0.0);
        s.line(pm.x0, hy, pm.x0 + pm.w, hy, &c.th.semantic.defending, 1.8, r#"stroke-dasharray="6 4""#);
    }
    y = pm.y0 + pm.h + 22.0;
    // legend as wrapped pills
    let items: [(&str, &str, String); 5] = [("square", "Touch density", c.th.semantic.touch.clone()), ("circle", "Touch", "#ffffff".into()), ("diamond", "Defending", c.th.semantic.defending.clone()), ("dash", "Defensive height", c.th.semantic.defending.clone()), ("star", "Shot", "#ffffff".into())];
    let (mut lx, mut ly) = (20.0, y);
    for (kind, label, colr) in items.iter() {
        let w = 12.0 + 22.0 + text_w(label, 12.5, false) + 12.0;
        if lx + w > PW - 20.0 {
            lx = 20.0;
            ly += 42.0;
        }
        s.rrect(lx, ly, w, 32.0, 16.0, CARD_HI, "none", 0.0);
        let _ = legend_item(&mut s, lx + 12.0, ly + 21.0, kind, colr, label);
        lx += w + 12.0;
    }
    y = ly + 32.0 + 30.0;

    // ---- tactical snapshot: three mini stats, then the averages
    s.t(24.0, y, "Tactical snapshot", 17.0, 700, PAPER, Anchor::Start);
    y += 16.0;
    let n_def = mine.iter().filter(|e| matches!(e.kind, Kind::Tackle | Kind::Interception | Kind::Recovery | Kind::Clearance | Kind::Block | Kind::Challenge)).count();
    let tw = (PW - 40.0 - 24.0) / 3.0;
    for (k, (v, l)) in [(p.touches_by_third[0], "Defensive third"), (p.touches_by_third[1], "Middle third"), (p.touches_by_third[2], "Final third")].iter().enumerate() {
        let x = 20.0 + k as f64 * (tw + 12.0);
        s.rrect(x, y, tw, 76.0, 16.0, CARD, "none", 0.0);
        s.h(x + 16.0, y + 40.0, &v.to_string(), 32.0, PAPER, Anchor::Start);
        s.t(x + 16.0, y + 62.0, l, 12.5, 500, SOFT, Anchor::Start);
    }
    y += 76.0 + 12.0;
    let lr = p.avg_y - 34.0;
    s.rrect(20.0, y, PW - 40.0, 76.0, 16.0, CARD, "none", 0.0);
    s.t(36.0, y + 30.0, "Average touch", 12.5, 500, SOFT, Anchor::Start);
    s.t(36.0, y + 54.0, &format!("{:.0} m upfield, {:.0} m {} of centre", p.avg_x, lr.abs(), if lr >= 0.0 { "left" } else { "right" }), 16.0, 700, PAPER, Anchor::Start);
    s.t(PW - 36.0, y + 24.0, "Defensive actions", 12.5, 500, SOFT, Anchor::End);
    s.h(PW - 36.0, y + 60.0, &n_def.to_string(), 30.0, PAPER, Anchor::End);
    y += 76.0 + 34.0;

    // ---- grouped stat sections
    type Group = (&'static str, Vec<(String, String)>);
    let groups: Vec<Group> = match role {
        Role::Goalkeeper => vec![
            ("Shot stopping", vec![("Saves".into(), p.saves.to_string()), ("Claims".into(), p.claims.to_string()), ("Sweeps".into(), p.sweeps.to_string())]),
            ("Distribution", vec![("Passes".into(), format!("{} ({:.0}%)", p.passes, if p.passes > 0 { p.passes_completed as f64 / p.passes as f64 * 100.0 } else { 0.0 })), ("Progressive passes".into(), p.progressive_passes.to_string()), ("Progression metres".into(), format!("{:.0}", p.progression_m))]),
            ("Involvement", vec![("Touches".into(), p.touches.to_string()), ("xGChain".into(), format!("{:.2}", p.xgchain)), ("VAEP".into(), signed(p.vaep_off + p.vaep_def, 2))]),
        ],
        _ => vec![
            ("Possession and passing", vec![
                ("Touches".to_string(), p.touches.to_string()),
                ("Passes".into(), format!("{} ({:.0}%)", p.passes, if p.passes > 0 { p.passes_completed as f64 / p.passes as f64 * 100.0 } else { 0.0 })),
                ("Progressive passes".into(), p.progressive_passes.to_string()),
                ("Line-breaking passes".into(), p.line_breaks.to_string()),
                ("Take-ons won".into(), format!("{} of {}", p.take_ons.0, p.take_ons.1)),
                ("Dispossessed".into(), p.dispossessed.to_string()),
            ]),
            ("Chance creation and shooting", vec![
                ("Shots".to_string(), p.shots.to_string()),
                ("xG".into(), format!("{:.2}", p.xg)),
                ("Inferred xA".into(), format!("{:.2}", p.inferred_xa)),
                ("Box entries".into(), p.box_entries.to_string()),
                ("Final-third receptions".into(), p.ft_receptions.to_string()),
                ("Positive xT".into(), format!("{:.2}", p.positive_xt)),
                ("xGBuildup".into(), format!("{:.2}", p.xgbuildup)),
            ]),
            ("Defending", vec![
                ("Recoveries".to_string(), p.recoveries.to_string()),
                ("Interceptions".into(), p.interceptions.to_string()),
                ("Tackles won".into(), p.tackles_won.to_string()),
                ("Dribbled past".into(), p.dribbled_past.to_string()),
                ("Aerials won".into(), format!("{} of {}", p.aerials.0, p.aerials.1)),
            ]),
        ],
    };
    for (k, (title, rows)) in groups.iter().enumerate() {
        let h = 62.0 + rows.len() as f64 * 40.0 + 10.0;
        s.rrect(20.0, y, PW - 40.0, h, 20.0, CARD, "none", 0.0);
        s.t(38.0, y + 30.0, &format!("No. {:02}", k + 1), 12.5, 500, SAGE, Anchor::Start);
        s.t(PW - 38.0, y + 30.0, &format!("{:02} — {:02}", k + 1, groups.len()), 12.5, 500, SOFT, Anchor::End);
        s.t(38.0, y + 54.0, title, 19.0, 700, PAPER, Anchor::Start);
        for (i, (l, v)) in rows.iter().enumerate() {
            let yy = y + 62.0 + 24.0 + i as f64 * 40.0;
            s.line(38.0, yy - 16.0, PW - 38.0, yy - 16.0, HAIR, 1.0, "");
            s.t(38.0, yy + 4.0, l, 15.0, 400, PAPER, Anchor::Start);
            s.t(PW - 38.0, yy + 4.0, v, 15.0, 800, PAPER, Anchor::End);
        }
        y += h + 16.0;
    }
    y += 6.0;
    for note in ["Raw match totals; unavailable values show as N/A. PAdj defensive actions are tackles, interceptions, recoveries, clearances, challenges and blocks per 100 opponent touches.", "Defensive height is the mean x of those actions. Progression metres are the forward distance of completed passes and carries. xGChain credits possession xG to every player involved; xGBuildup leaves out the shooter and key passer."] {
        for l in wrap_text(note, 11.5, PW - 48.0) {
            s.t(24.0, y, &l, 11.5, 400, SOFT, Anchor::Start);
            y += 16.0;
        }
        y += 6.0;
    }
    y += 24.0;
    // the how-to-read line lives in the viewer's Info sheet
    s.how = "Role and minutes govern comparisons; these are match observations, not a season ability rating. Nominal pitch 105 × 68 m.".to_string();
    s.h = y;
    s.scene.h = y as f32;
    Board::new(&format!("31_player_{}_{}", side_tag(sd), p.id), "Player card", s)
}

pub fn _unused(_: f64) -> f64 {
    shot_r(0.0)
}
