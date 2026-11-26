//! Boards 1-10: chance flow, goal origins, win odds, set pieces, corridors,
//! shot anatomy, shot maps, box entries, passing by third, pass networks.

use super::frame::*;
use super::pitch::*;
use super::svg::*;
use super::Board;
use crate::model::*;
use crate::theme;

pub const W: f64 = 1600.0;
pub const H: f64 = 1130.0;
pub const TOP: f64 = 165.0;

pub fn shot_r(xg: f64) -> f64 {
    5.0 + 24.0 * xg.max(0.0).sqrt()
}

/// Tooltip text for a shot.
pub fn shot_tip(c: &Ctx, e: &Event) -> String {
    let sh = e.shot.as_ref().unwrap();
    let outcome = match sh.outcome {
        ShotOutcome::Goal => "Goal",
        ShotOutcome::Saved => "Saved",
        ShotOutcome::OffTarget => "Off target",
        ShotOutcome::Woodwork => "Woodwork",
        ShotOutcome::Blocked => "Blocked",
    };
    let body = match sh.body {
        Body::Right => "right foot",
        Body::Left => "left foot",
        Body::Head => "header",
        Body::Other => "other",
    };
    let ps = sh.xgot_ext.or(sh.psxg).map(|v| format!(" · PSxG {v:.2}")).unwrap_or_default();
    format!("{} {} · {} · {:.2} xG{} · {}{}", c.pname(e.side, e.player), e.label_minute(), outcome, sh.xg, ps, body, if sh.penalty { " · penalty" } else { "" })
}

fn mins(t: f64) -> String {
    format!("{}'", (t / 60.0).floor() as i64 + 1)
}

// ---------------------------------------------------------------- 1 xG flow
pub fn xg_flow(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    let (h, a) = (c.a.t(Side::Home), c.a.t(Side::Away));
    let lead = if h.npxg > a.npxg { Side::Home } else { Side::Away };
    let title = if (h.npxg - a.npxg).abs() >= 0.5 {
        format!("On chances {} deserved more: {:.2} xG to {:.2}", c.name(lead), c.a.t(lead).npxg, c.a.t(lead.other()).npxg)
    } else {
        format!("Chances were close: {:.2} xG to {:.2}", h.npxg, a.npxg)
    };
    header(&mut s, c, &title, "Cumulative non-penalty xG by minute · each step is one shot, scaled by chance quality", Scope::Full);
    let (x0, y0, w, hh) = (110.0, 220.0, 1400.0, 680.0);
    let dur = c.a.duration_min.max(90.0);
    let ymax = (h.xg.max(a.xg) * 1.15).max(1.0).ceil();
    let px = |m: f64| x0 + m / dur * w;
    let py = |v: f64| y0 + hh - v / ymax * hh;
    for i in 0..=(ymax as i32 * 2) {
        let v = i as f64 * 0.5;
        s.line(x0, py(v), x0 + w, py(v), theme::LINE, 1.0, "");
        s.t(x0 - 12.0, py(v) + 4.0, &format!("{v:.1}"), 12.0, 400, theme::MUTED, Anchor::End);
    }
    for mm in (0..=(dur as i32)).step_by(15) {
        s.t(px(mm as f64), y0 + hh + 24.0, &format!("{mm}'"), 12.0, 400, theme::MUTED, Anchor::Middle);
    }
    if let Some(p2) = c.m.periods.iter().find(|p| p.period == 2) {
        let x = px(p2.start_t / 60.0);
        s.line(x, y0, x, y0 + hh, theme::GREY, 1.2, r#"stroke-dasharray="5 5""#);
        s.t(x + 6.0, y0 + 14.0, "HT", 11.0, 600, theme::MUTED, Anchor::Start);
    }
    for side in Side::both() {
        let col = c.col(side).to_string();
        let mut pts = vec![(px(0.0), py(0.0))];
        let mut cum = 0.0;
        for &q in &c.a.t(side).shots {
            let e = &c.m.events[q];
            let sh = e.shot.as_ref().unwrap();
            let m = e.t / 60.0;
            pts.push((px(m), py(cum)));
            cum += sh.xg;
            pts.push((px(m), py(cum)));
        }
        pts.push((px(dur), py(cum)));
        s.polyline(&pts, &col, 3.0, "");
        for &q in &c.a.t(side).shots {
            let e = &c.m.events[q];
            let sh = e.shot.as_ref().unwrap();
            let before: f64 = c.a.t(side).shots.iter().take_while(|&&k| k != q).map(|&k| c.m.events[k].shot.as_ref().unwrap().xg).sum();
            let (gx, gy) = (px(e.t / 60.0), py(before + sh.xg));
            s.tip = Some(shot_tip(c, e));
            s.circle(gx, gy, 3.5, &col, "none", 0.0, r#"opacity="0.9""#);
            if sh.outcome == ShotOutcome::Goal {
                s.circle(gx, gy, 8.0, &col, "#fff", 2.0, "");
                s.t(gx, gy - 14.0, &format!("{} {}", c.pname(side, e.player), e.label_minute()), 12.5, 700, theme::TEXT, Anchor::Middle);
            } else if sh.outcome == ShotOutcome::Woodwork {
                s.diamond(gx, gy, 7.0, &c.th.semantic.woodwork, "none", 0.0);
            }
        }
        s.t(px(dur) + 8.0, py(cum) + 5.0, &format!("{:.2}", cum), 15.0, 800, &col, Anchor::Start);
    }
    // drawarrow-style annotation on the single biggest chance
    let best = Side::both().iter().flat_map(|&sd| c.a.t(sd).shots.iter().map(move |&q| (sd, q))).filter(|(_, q)| !c.m.events[*q].shot.as_ref().unwrap().penalty).max_by(|a, b| c.m.events[a.1].shot.as_ref().unwrap().xg.partial_cmp(&c.m.events[b.1].shot.as_ref().unwrap().xg).unwrap());
    if let Some((sd, q)) = best {
        let e = &c.m.events[q];
        let sh = e.shot.as_ref().unwrap();
        let before: f64 = c.a.t(sd).shots.iter().take_while(|&&k| k != q).map(|&k| c.m.events[k].shot.as_ref().unwrap().xg).sum();
        let (gx, gy) = (px(e.t / 60.0), py(before + sh.xg));
        let (lx, ly) = (gx.clamp(x0 + 160.0, x0 + w - 160.0) - 120.0, (gy - 120.0).max(y0 + 30.0));
        s.curved_arrow(lx + 110.0, ly + 8.0, gx - 6.0, gy - 8.0, 0.25, "#F5F3EE", 1.6);
        let runs = vec![("Biggest chance: ".to_string(), theme::MUTED.to_string()), (format!("{} {} · {:.2} xG", c.pname(sd, e.player), e.label_minute(), sh.xg), c.col(sd).to_string())];
        s.text_runs(lx - 60.0, ly, &runs, 14.0, 700, Anchor::Start, BODY);
    }
    let mut lx = x0;
    lx = legend_item(&mut s, lx, y0 + hh + 60.0, "line", c.col(Side::Home), &c.name(Side::Home));
    lx = legend_item(&mut s, lx, y0 + hh + 60.0, "line", c.col(Side::Away), &c.name(Side::Away));
    lx = legend_item(&mut s, lx, y0 + hh + 60.0, "ring", "#fff", "Goal");
    legend_item(&mut s, lx, y0 + hh + 60.0, "diamond", &c.th.semantic.woodwork, "Woodwork");
    footer(&mut s, c, "Each upward step marks a shot scaled by chance quality; flat stretches are spells without an attempt. Penalties are excluded from the curve.");
    Board::new("01_xg_flow", "xG flow", s)
}

// ---------------------------------------------------------------- 2 goal origins
pub fn goal_origins(c: &Ctx) -> Vec<Board> {
    let goals = &c.a.goals;
    if goals.is_empty() {
        let mut s = Svg::new(W, H);
        header(&mut s, c, "Goal Origins", "No goals were scored from open play or set pieces", Scope::Full);
        let best = Side::both().iter().flat_map(|&sd| c.a.t(sd).shots.iter().map(move |&q| (sd, q))).max_by(|a, b| c.m.events[a.1].shot.as_ref().unwrap().xg.partial_cmp(&c.m.events[b.1].shot.as_ref().unwrap().xg).unwrap());
        let msg = match best {
            Some((sd, q)) => {
                let e = &c.m.events[q];
                format!("No goals · biggest chance: {} {} ({:.2} xG)", c.pname(sd, e.player), e.label_minute(), e.shot.as_ref().unwrap().xg)
            }
            None => "No goals and no shots".into(),
        };
        s.h(W / 2.0, H / 2.0, &msg, 40.0, theme::TEXT, Anchor::Middle);
        footer(&mut s, c, "Each goal is linked to its recorded build-up sequence and elapsed event time.");
        return vec![Board::new("02_goal_origins", "Goal origins", s)];
    }
    let mut out = vec![];
    for (page, chunk) in goals.chunks(4).enumerate() {
        let mut s = Svg::new(W, H);
        header(&mut s, c, "Goal Origins", "Every goal drawn from where its possession began · numbered touches, arrows for the passes that built it", Scope::Full);
        let n = chunk.len();
        let per_row = if n <= 2 { n } else { 2 };
        let rows = n.div_ceil(per_row);
        let cell_w = (W - 80.0) / per_row as f64;
        let cell_h = (H - TOP - 120.0) / rows as f64;
        for (k, g) in chunk.iter().enumerate() {
            let (r, cl) = (k / per_row, k % per_row);
            let bx = 40.0 + cl as f64 * cell_w;
            let by = TOP + 20.0 + r as f64 * cell_h;
            let text_h = if rows == 1 { 190.0 } else { 120.0 };
            let pitch_w = if rows == 1 { cell_w * 0.62 } else { cell_w * 0.45 };
            let p = if rows == 1 { Pitch::up(bx + (cell_w - pitch_w) / 2.0, by + 20.0, pitch_w, cell_h - text_h - 30.0, 0.0) } else { Pitch::up(bx + 20.0, by + 20.0, pitch_w, cell_h - 30.0, 0.0) };
            p.label_attack(&mut s);
            p.draw(&mut s);
            let col = c.col(g.side).to_string();
            let ev = &c.m.events;
            let mut num = 1;
            for (j, &q) in g.chain.iter().enumerate() {
                let e = &ev[q];
                let (sx, sy) = p.p(e.x, e.y);
                if e.kind == Kind::Shot {
                    let (gx, gy) = p.p(105.0, 34.0 + e.shot.as_ref().unwrap().goal_u.unwrap_or(0.0) * 3.66);
                    s.line(sx, sy, gx, gy, "#fff", 2.0, r#"stroke-dasharray="5 4""#);
                    s.star(sx, sy, 11.0, "#fff", "none");
                    continue;
                }
                if matches!(e.kind, Kind::Pass | Kind::Carry) {
                    let (ex, ey) = p.p(e.end().0, e.end().1);
                    if e.kind == Kind::Pass {
                        s.arrow(sx, sy, ex, ey, &col, 2.2, 8.0, "");
                    } else {
                        s.line(sx, sy, ex, ey, &col, 1.8, r#"stroke-dasharray="3 3""#);
                    }
                }
                if e.kind != Kind::Carry {
                    s.tip = Some(format!("{}. {} · {:?} · {}'{:02}", num, c.pname(e.side, e.player), e.kind, e.minute, e.second));
                    s.circle(sx, sy, 10.0, "#12211A", &col, 2.0, "");
                    s.t(sx, sy + 4.0, &num.to_string(), 10.5, 700, theme::TEXT, Anchor::Middle);
                    num += 1;
                }
                let _ = j;
            }
            let shot = &ev[g.shot];
            let (tx, ty) = if rows == 1 { (p.x0, p.y0 + p.h + 50.0) } else { (p.x0 + p.w + 30.0, by + 70.0) };
            s.h(tx, ty, &format!("{}'", shot.minute + 1), 48.0, &col, Anchor::Start);
            s.h(tx + 80.0, ty - 14.0, &c.pname(g.side, shot.player), 30.0, theme::TEXT, Anchor::Start);
            s.t(tx + 80.0, ty + 6.0, &g.label, 12.0, 800, theme::MUTED, Anchor::Start);
            s.t(tx, ty + 44.0, &format!("{} passes · {:.0} s · {} players", g.passes, g.secs, g.players), 15.0, 500, theme::TEXT, Anchor::Start);
            s.t(tx, ty + 68.0, &format!("Started: {}", g.start), 15.0, 400, theme::MUTED, Anchor::Start);
            let body = match g.body {
                Body::Right => "right foot",
                Body::Left => "left foot",
                Body::Head => "header",
                Body::Other => "other",
            };
            let assist = g.assist.map(|a| c.pname(g.side, Some(a))).unwrap_or_else(|| "unassisted".into());
            s.t(tx, ty + 92.0, &format!("Assist: {} · {} · {:.2} xG", assist, body, g.xg), 15.0, 400, theme::TEXT, Anchor::Start);
        }
        footer(&mut s, c, "Each goal is linked to its recorded build-up sequence and elapsed event time. Numbers follow the touches in order; the star is the shot, dashed lines are carries.");
        let id = if goals.len() > 4 { format!("02_goal_origins_{}", page + 1) } else { "02_goal_origins".into() };
        out.push(Board::new(&id, "Goal origins", s));
    }
    out
}

// ---------------------------------------------------------------- 3 win odds
pub fn win_odds(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    let w = &c.a.win;
    let title = format!("The chances were worth {:.1} points to {} and {:.1} to {}", w.xpts_home, c.name(Side::Home), w.xpts_away, c.name(Side::Away));
    header(&mut s, c, &title, "Expected points: every shot replayed at its xG · the curve is each side's chance of winning as the match went on", Scope::Full);
    let l = &c.a.live;
    s.t(60.0, 205.0, &format!("LIKELIER WINNER FOR   {} {:.0}% OF THE MATCH   ·   {} {:.0}%   ·   LEVEL {:.0}%", c.name(Side::Home).to_uppercase(), l.likelier_home * 100.0, c.name(Side::Away).to_uppercase(), l.likelier_away * 100.0, l.likelier_draw * 100.0), 15.0, 800, theme::TEXT, Anchor::Start);
    let (bx, by, bw, bh) = (60.0, 235.0, W - 120.0, 120.0);
    let segs = [(w.p_home, c.col(Side::Home).to_string(), format!("{} win", c.name(Side::Home))), (w.p_draw, "#A3B3AA".to_string(), "Draw".into()), (w.p_away, c.col(Side::Away).to_string(), format!("{} win", c.name(Side::Away)))];
    let mut x = bx;
    for (p, col, lab) in segs.iter() {
        let ww = bw * p;
        s.rect(x, by, ww, bh, col, "");
        if ww > 90.0 {
            let on = if theme::Rgb::hex(col).unwrap().luminance() > 0.42 { "#0B1510" } else { "#F5F3EE" };
            s.t(x + ww / 2.0, by + bh / 2.0 - 4.0, lab, 17.0, 700, on, Anchor::Middle);
            s.t(x + ww / 2.0, by + bh / 2.0 + 18.0, &pct(*p), 17.0, 700, on, Anchor::Middle);
        }
        x += ww;
    }
    let (x0, y0, cw, ch) = (110.0, 420.0, 1380.0, 520.0);
    let dur = l.minutes.last().copied().unwrap_or(90.0).max(1.0);
    let px = |m: f64| x0 + m / dur * cw;
    let py = |v: f64| y0 + ch - v * ch;
    for i in 0..=5 {
        let v = i as f64 * 0.2;
        s.line(x0, py(v), x0 + cw, py(v), theme::LINE, 1.0, "");
        s.t(x0 - 10.0, py(v) + 4.0, &pct(v), 12.0, 400, theme::MUTED, Anchor::End);
    }
    for mm in (0..=(dur as i32)).step_by(20) {
        s.t(px(mm as f64), y0 + ch + 22.0, &mm.to_string(), 12.0, 400, theme::MUTED, Anchor::Middle);
    }
    s.t(x0 + cw / 2.0, y0 + ch + 46.0, "Minute", 13.0, 500, theme::MUTED, Anchor::Middle);
    let hp: Vec<(f64, f64)> = l.minutes.iter().zip(&l.home).map(|(m, v)| (px(*m), py(*v))).collect();
    let mut area = hp.clone();
    area.push((px(dur), py(0.0)));
    area.push((px(0.0), py(0.0)));
    s.poly(&area, c.col(Side::Home), "none", 0.0, r#"opacity="0.35""#);
    s.polyline(&hp, c.col(Side::Home), 3.0, "");
    let ap: Vec<(f64, f64)> = l.minutes.iter().zip(&l.away).map(|(m, v)| (px(*m), py(*v))).collect();
    s.polyline(&ap, c.col(Side::Away), 3.0, "");
    let dp: Vec<(f64, f64)> = l.minutes.iter().zip(&l.draw).map(|(m, v)| (px(*m), py(*v))).collect();
    s.polyline(&dp, "#A3B3AA", 2.0, r#"stroke-dasharray="6 5""#);
    let mut lx = x0;
    lx = legend_item(&mut s, lx, y0 - 16.0, "line", c.col(Side::Home), &format!("{} win", c.name(Side::Home)));
    lx = legend_item(&mut s, lx, y0 - 16.0, "line", c.col(Side::Away), &format!("{} win", c.name(Side::Away)));
    legend_item(&mut s, lx, y0 - 16.0, "dash", "#A3B3AA", "Draw");
    s.t(x0, y0 + ch + 76.0, &format!("Replayed {} times. The curve is a transparent heuristic on score, time and xG, not a betting model.", w.sims), 12.0, 400, theme::GREY, Anchor::Start);
    footer(&mut s, c, "Read the stated event, time window and denominator before comparing the teams. At most one goal is drawn per possession so rebounds are not double counted.");
    Board::new("03_win_odds", "Simulated win odds and expected points", s)
}

// ---------------------------------------------------------------- 4 set pieces
pub fn set_pieces(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, "Set-Piece Contribution", "Every shot traced back to how the possession started · the source is read off the delivery, not the shot", Scope::Full);
    let maxn = Side::both().iter().map(|&sd| {
        let t = c.a.t(sd);
        t.sp_open.n.max(t.sp_corner.n).max(t.sp_fk.n).max(t.sp_pen.n)
    }).max().unwrap_or(1).max(5) as f64;
    for (k, sd) in Side::both().iter().enumerate() {
        let t = c.a.t(*sd);
        let col = c.col(*sd).to_string();
        let bx = 60.0 + k as f64 * 760.0;
        // one card per team: the phone layout scales a card as a whole, never its rows
        panel(&mut s, bx - 20.0, 190.0, 730.0, 880.0);
        s.h(bx, 225.0, &c.name(*sd).to_uppercase(), 30.0, &col, Anchor::Start);
        let tot = t.npxg.max(1e-9);
        let dead = (t.sp_corner.xg + t.sp_fk.xg) / tot;
        s.t(bx, 252.0, &format!("{:.0}% of xG came from a dead ball (corners and free kicks)", dead * 100.0), 13.0, 400, theme::MUTED, Anchor::Start);
        let rows = [("Open play", &t.sp_open), ("Corners", &t.sp_corner), ("Free kicks", &t.sp_fk), ("Penalties", &t.sp_pen)];
        let (lx, cw) = (bx + 110.0, 440.0);
        for (i, (lab, b)) in rows.iter().enumerate() {
            if *lab == "Penalties" && b.n == 0 {
                continue;
            }
            let y = 300.0 + i as f64 * 160.0;
            s.t(lx - 12.0, y + 60.0, lab, 14.0, 500, theme::TEXT, Anchor::End);
            if b.n == 0 {
                s.t(lx + 8.0, y + 64.0, "none", 13.0, 400, theme::MUTED, Anchor::Start);
                continue;
            }
            let ww = b.n as f64 / maxn * cw;
            s.rect(lx, y + 20.0, ww, 85.0, &col, "");
            for g in 1..(b.n / 5 + 1) {
                let gx = lx + (g * 5) as f64 / maxn * cw;
                if gx < lx + ww {
                    s.line(gx, y + 20.0, gx, y + 105.0, "#000", 0.6, r#"opacity="0.35""#);
                }
            }
            s.t(lx + ww + 10.0, y + 68.0, &format!("{} · {:.2} xG · {}G", b.n, b.xg, b.goals), 13.0, 700, theme::TEXT, Anchor::Start);
        }
        s.line(lx, 960.0, lx + cw, 960.0, theme::LINE, 1.0, "");
        for g in 0..=((maxn / 5.0) as i32) {
            let gx = lx + (g * 5) as f64 / maxn * cw;
            s.t(gx, 980.0, &(g * 5).to_string(), 12.0, 400, theme::MUTED, Anchor::Middle);
        }
        s.t(lx + cw / 2.0, 1002.0, "Shots", 12.0, 500, theme::MUTED, Anchor::Middle);
        s.t(bx, 1030.0, &format!("Corners taken {} · {} brought a shot within 10 s", t.corners, t.corners_shot10), 13.0, 500, theme::TEXT, Anchor::Start);
    }
    footer(&mut s, c, "Restart categories separate corners, free kicks and penalties. Compare the number of deliveries or possessions with the xG of the resulting shots.");
    Board::new("04_set_pieces", "Set-piece contribution", s)
}

// ---------------------------------------------------------------- 5 creation corridors
pub fn corridors(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, &format!("Creation Corridors: Zone 14 & Half-Spaces · {} vs {}", c.name(Side::Home), c.name(Side::Away)), "Completed passes into Zone 14 & half-spaces · cutbacks & through-balls highlighted", Scope::Full);
    let sem = &c.th.semantic;
    for (k, sd) in Side::both().iter().enumerate() {
        let t = c.a.t(*sd);
        let col = c.col(*sd).to_string();
        let bx = 40.0 + k as f64 * 780.0;
        s.t(bx + 370.0, 190.0, &format!("{}  ·  Zone 14: {}  ·  Half-spaces: {}", c.name(*sd), t.z14.len(), t.hs_left.len() + t.hs_right.len()), 14.0, 800, &col, Anchor::Middle);
        s.t(bx + 370.0, 212.0, &format!("L {} / R {}  ·  Cutbacks: {}  ·  Through-balls: {}", t.hs_left.len(), t.hs_right.len(), t.cutbacks.len(), t.through_balls.len()), 12.5, 600, &col, Anchor::Middle);
        let p = Pitch::up(bx, 240.0, 740.0, 700.0, 45.0);
        p.zone(&mut s, 70.0, 88.5, 24.85, 43.15, "#ffffff", 0.08);
        p.zone(&mut s, 60.0, 88.5, 43.15, 54.16, &sem.hs_left, 0.10);
        p.zone(&mut s, 60.0, 88.5, 13.84, 24.85, &sem.hs_right, 0.10);
        p.label_attack(&mut s);
        p.draw(&mut s);
        let ev = &c.m.events;
        let draw = |s: &mut Svg, idx: &[usize], colr: &str, w: f64, layer: &str, dash: bool| {
            for &q in idx {
                let e = &ev[q];
                // clip to the visible pitch so no line leaves it
                let Some(((x1, y1), (x2, y2), end_in)) = clip_seg((e.x, e.y), e.end(), p.lmin, 105.0, 0.0, 68.0) else { continue };
                let (a1, b1) = p.p(x1, y1);
                let (a2, b2) = p.p(x2, y2);
                s.layer = Some(layer.to_string());
                s.tip = Some(format!("{} {} · {} · {} · {}", c.pname(*sd, e.player), e.label_minute(), layer, if e.success { "completed" } else { "incomplete" }, c.name(*sd)));
                // the dash keeps through-balls distinguishable without colour
                s.line(a1, b1, a2, b2, colr, w, if dash { r#"opacity="0.9" stroke-dasharray="7 5""# } else { r#"opacity="0.9""# });
                if end_in {
                    s.circle(a2, b2, 3.4, colr, "none", 0.0, "");
                }
                s.layer = None;
            }
        };
        draw(&mut s, &t.hs_left, &sem.hs_left, 2.2, "Left half-space", false);
        draw(&mut s, &t.hs_right, &sem.hs_right, 2.2, "Right half-space", false);
        draw(&mut s, &t.z14, &sem.z14, 2.2, "Zone 14 pass", false);
        draw(&mut s, &t.cutbacks, &sem.cutback, 3.4, "Cutback", false);
        draw(&mut s, &t.through_balls, &sem.through, 3.4, "Through-ball attempt", true);
        for (lab, xm, n) in [("LHS", 48.6, t.hs_left.len()), ("ZONE 14", 34.0, t.z14.len()), ("RHS", 19.4, t.hs_right.len())] {
            let (lx, ly) = p.p(60.0, xm);
            s.rrect(lx - 38.0, ly - 12.0, 76.0, 22.0, 4.0, "#0B1510", "#F5F3EE", 1.0);
            s.t(lx, ly + 4.0, &format!("{lab}: {n}"), 11.0, 700, theme::TEXT, Anchor::Middle);
        }
    }
    let mut lx = 200.0;
    let y = 990.0;
    lx = legend_item(&mut s, lx, y, "circle", &sem.z14, "Zone 14 pass");
    lx = legend_item(&mut s, lx, y, "circle", &sem.hs_right, "Right half-space");
    legend_item(&mut s, lx, y, "circle", &sem.hs_left, "Left half-space");
    let mut lx = 200.0;
    let y = 1024.0;
    lx = legend_item(&mut s, lx, y, "diamond", &sem.cutback, "Cutback (inside box)");
    legend_item(&mut s, lx, y, "diamond", &sem.through, "Through-ball attempt");
    for k in ["Zone 14 pass", "Right half-space", "Left half-space", "Cutback", "Through-ball attempt"] {
        s.layer_idx(k);
    }
    let _ = col_unused();
    footer(&mut s, c, "Each pitch maps completed passes into central Zone 14 and the left/right half-spaces outside the box, alongside cutbacks and through-balls. Compare interior pocket access with each corridor's pass total.");
    Board::new("05_creation_corridors", "Creation corridors", s)
}
fn col_unused() {}

/// Liang-Barsky clip of a segment to a rectangle (pitch metres). Returns the clipped ends and whether
/// the original end point lies inside the rectangle.
pub fn clip_seg(a: (f64, f64), b: (f64, f64), x0: f64, x1: f64, y0: f64, y1: f64) -> Option<((f64, f64), (f64, f64), bool)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - x0), (dx, x1 - a.0), (-dy, a.1 - y0), (dy, y1 - a.1)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                t0 = t0.max(r);
            } else {
                if r < t0 {
                    return None;
                }
                t1 = t1.min(r);
            }
        }
    }
    let end_in = b.0 >= x0 && b.0 <= x1 && b.1 >= y0 && b.1 <= y1;
    Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy), end_in))
}

// ---------------------------------------------------------------- 6 shot anatomy
pub fn shot_anatomy(c: &Ctx) -> Board {
    let mut s = Svg::new(W, H);
    header(&mut s, c, &format!("Shot Anatomy & Pitch Zones (npxG) · {} vs {}", c.name(Side::Home), c.name(Side::Away)), "6-yard box vs 18-yard box vs outside box · marker shape = header / first-touch / foot · size = xG", Scope::Full);
    for (k, sd) in Side::both().iter().enumerate() {
        let t = c.a.t(*sd);
        let col = c.col(*sd).to_string();
        let bx = 40.0 + k as f64 * 780.0;
        s.t(bx + 370.0, 205.0, &format!("{}  ·  npxG {:.2}  ·  Box {} ({:.2} xG)", c.name(*sd), t.npxg, t.in_box.n, t.in_box.xg), 15.0, 800, &col, Anchor::Middle);
        let p = Pitch::up(bx, 245.0, 740.0, 560.0, 60.0);
        p.zone(&mut s, 88.5, 105.0, 13.84, 54.16, "#3b5bdb", 0.12);
        p.zone(&mut s, 99.5, 105.0, 24.84, 43.16, "#ffffff", 0.12);
        p.label_attack(&mut s);
        p.draw(&mut s);
        for &q in &t.shots {
            let e = &c.m.events[q];
            let sh = e.shot.as_ref().unwrap();
            if sh.penalty {
                continue;
            }
            let fill = if crate::derive::in_six(e.x, e.y) { "#ffffff" } else if crate::derive::in_box(e.x, e.y) { "#3b82f6" } else { "#A3B3AA" };
            let (x, y) = p.p(e.x, e.y);
            let r = shot_r(sh.xg) * 0.8;
            s.tip = Some(shot_tip(c, e));
            if sh.body == Body::Head {
                s.triangle(x, y, r + 2.0, fill, "none", 0.0);
            } else if sh.first_time {
                s.diamond(x, y, r + 1.0, fill, "none", 0.0);
            } else {
                s.circle(x, y, r, fill, "none", 0.0, r#"opacity="0.9""#);
            }
            if sh.outcome == ShotOutcome::Goal {
                s.circle(x, y, r + 4.0, "none", &c.th.semantic.goal, 3.0, "");
            }
        }
        let cells = [("6-YD BOX", &t.six), ("ALL BOX", &t.in_box), ("OUTSIDE", &t.outside), ("HEADED", &t.headed), ("1ST-TOUCH", &t.first_touch)];
        let tw = 740.0 / 5.0;
        panel(&mut s, bx, 840.0, 740.0, 90.0);
        for (i, (lab, b)) in cells.iter().enumerate() {
            let x = bx + tw * i as f64 + tw / 2.0;
            s.t(x, 870.0, lab, 11.0, 700, theme::MUTED, Anchor::Middle);
            let v = format!("{} ({:.2})", b.n, b.xg);
            s.t(x, 905.0, &v, 18.0, 800, if *lab == "ALL BOX" { "#60a5fa" } else { theme::TEXT }, Anchor::Middle);
        }
    }
    let mut lx = 180.0;
    let y = 980.0;
    lx = legend_item(&mut s, lx, y, "circle", "#ffffff", "6-yard box");
    lx = legend_item(&mut s, lx, y, "circle", "#3b82f6", "18-yard box");
    lx = legend_item(&mut s, lx, y, "circle", "#A3B3AA", "Outside box");
    lx = legend_item(&mut s, lx, y, "triangle", "#F5F3EE", "Headed shot");
    lx = legend_item(&mut s, lx, y, "diamond", "#F5F3EE", "First-touch shot");
    legend_item(&mut s, lx, y, "ring", &c.th.semantic.goal, "Goal (gold ring)");
    footer(&mut s, c, "Shading separates the 6-yard box, the rest of the 18-yard box and outside the box; marker shape identifies headed vs first-touch efforts and size scales with xG. Penalties excluded.");
    Board::new("06_shot_anatomy", "Shot anatomy", s)
}

// ---------------------------------------------------------------- 7 shot map per team
pub fn shot_map(c: &Ctx, sd: Side) -> Board {
    let mut s = Svg::new(W, H);
    let t = c.a.t(sd);
    let title = format!("{}: {} shot{} worth {:.2} xG, {} goal{}", c.name(sd), t.shots.len(), if t.shots.len() == 1 { "" } else { "s" }, t.xg, t.goals, if t.goals == 1 { "" } else { "s" });
    header(&mut s, c, &title, "Shot map: location, outcome and chance quality · marker size = xG", Scope::Team(sd));
    let sem = &c.th.semantic;
    let p = Pitch::up(80.0, 200.0, 760.0, 800.0, 0.0);
    p.label_attack(&mut s);
    p.draw(&mut s);
    let mut counts = [0usize; 5];
    for &q in &t.shots {
        let e = &c.m.events[q];
        let sh = e.shot.as_ref().unwrap();
        let (x, y) = p.p(e.x, e.y);
        let r = shot_r(sh.xg) * 0.6;
        s.tip = Some(shot_tip(c, e));
        s.layer = Some(match sh.outcome {
            ShotOutcome::OffTarget => "Off target",
            ShotOutcome::Blocked => "Blocked",
            ShotOutcome::Woodwork => "Woodwork",
            ShotOutcome::Saved => "Saved",
            ShotOutcome::Goal => "Goal",
        }.to_string());
        match sh.outcome {
            ShotOutcome::OffTarget => {
                s.cross(x, y, r * 0.8, &sem.off_target, 3.0);
                counts[0] += 1
            }
            ShotOutcome::Blocked => {
                s.square(x, y, r * 0.8, &sem.blocked, "none", 0.0);
                counts[1] += 1
            }
            ShotOutcome::Woodwork => {
                s.diamond(x, y, r, &sem.woodwork, "none", 0.0);
                counts[2] += 1
            }
            ShotOutcome::Saved => {
                s.circle(x, y, r * 0.8, &sem.saved, "none", 0.0, "");
                counts[3] += 1
            }
            ShotOutcome::Goal => {
                s.star(x, y, r + 4.0, &sem.goal, "none");
                counts[4] += 1
            }
        }
        if t.clear_chances.contains(&q) && sh.xg >= 0.2 {
            s.t(x + r + 4.0, y - 4.0, &format!("{:.2}", sh.xg), 12.0, 700, theme::TEXT, Anchor::Start);
        }
        s.layer = None;
    }
    for k in ["Off target", "Blocked", "Woodwork", "Saved", "Goal"] {
        s.layer_idx(k);
    }
    let mut lx = 80.0;
    let y = 1040.0;
    lx = legend_item(&mut s, lx, y, "cross", &sem.off_target, &format!("Off target ({})", counts[0]));
    lx = legend_item(&mut s, lx, y, "square", &sem.blocked, &format!("Blocked ({})", counts[1]));
    lx = legend_item(&mut s, lx, y, "diamond", &sem.woodwork, &format!("Woodwork ({})", counts[2]));
    lx = legend_item(&mut s, lx, y, "circle", &sem.saved, &format!("Saved ({})", counts[3]));
    legend_item(&mut s, lx, y, "star", &sem.goal, &format!("Goal ({})", counts[4]));
    let (px0, py0) = (960.0, 200.0);
    panel(&mut s, px0, py0, 580.0, 640.0);
    s.t(px0 + 28.0, py0 + 40.0, "SHOT OUTPUT", 14.0, 800, theme::TEXT, Anchor::Start);
    s.line(px0 + 28.0, py0 + 58.0, px0 + 552.0, py0 + 58.0, theme::LINE, 1.0, "");
    let rows = [("SHOTS", t.shots.len().to_string()), ("XG", format!("{:.2}", t.xg)), ("XG / SHOT", format!("{:.3}", t.xg / t.shots.len().max(1) as f64)), ("ON TARGET", t.on_target.to_string())];
    for (i, (l, v)) in rows.iter().enumerate() {
        let yy = py0 + 100.0 + i as f64 * 82.0;
        s.t(px0 + 28.0, yy, l, 12.0, 700, theme::MUTED, Anchor::Start);
        s.h(px0 + 28.0, yy + 38.0, v, 36.0, theme::TEXT, Anchor::Start);
    }
    s.t(px0 + 28.0, py0 + 452.0, "GOAL FRAME TARGETED", 12.0, 700, theme::MUTED, Anchor::Start);
    for (i, (k, n)) in t.frame_zones.iter().take(6).enumerate() {
        let yy = py0 + 488.0 + i as f64 * 26.0;
        s.t(px0 + 28.0, yy, k, 13.0, 500, theme::TEXT, Anchor::Start);
        s.t(px0 + 552.0, yy, &n.to_string(), 13.0, 700, theme::TEXT, Anchor::End);
    }
    if t.frame_zones.is_empty() {
        s.t(px0 + 28.0, py0 + 488.0, "No shots on target", 13.0, 400, theme::MUTED, Anchor::Start);
    }
    footer(&mut s, c, "Location shows where attempts were taken; symbol size encodes xG. Use this to separate shot volume from the quality and location of chances.");
    Board::new(&format!("07_shot_map_{}", side_tag(sd)), "Shot map", s)
}

pub fn side_tag(s: Side) -> &'static str {
    match s {
        Side::Home => "home",
        Side::Away => "away",
    }
}

// ---------------------------------------------------------------- 8 box entries per team
pub fn box_entries(c: &Ctx, sd: Side) -> Board {
    let mut s = Svg::new(W, H);
    let t = c.a.t(sd);
    let shot_n = t.entries.iter().filter(|e| e.shot_followed).count();
    let title = format!("{} got into the box {} time{} · {} led to a shot", c.name(sd), t.entries.len(), if t.entries.len() == 1 { "" } else { "s" }, shot_n);
    header(&mut s, c, &title, "Box entries: completed actions entering the penalty area · highlighted = a shot followed in that possession", Scope::Team(sd));
    let col = c.col(sd).to_string();
    let p = Pitch::up(80.0, 200.0, 760.0, 820.0, 0.0);
    p.label_attack(&mut s);
    p.draw(&mut s);
    for en in &t.entries {
        let e = &c.m.events[en.idx];
        let (a1, b1) = p.p(e.x, e.y);
        let (a2, b2) = p.p(e.end().0, e.end().1);
        let lc = if en.shot_followed { "#ffffff" } else { col.as_str() };
        s.layer = Some(if en.shot_followed { "Shot followed" } else { "No shot followed" }.to_string());
        s.tip = Some(format!("{} {} · {} into the box{}", c.pname(sd, e.player), e.label_minute(), if en.carry { "carry" } else { "pass" }, if en.shot_followed { " · shot followed" } else { "" }));
        if en.shot_followed {
            s.arrow(a1, b1, a2, b2, lc, 2.2, 9.0, "");
        } else {
            s.line(a1, b1, a2, b2, lc, 1.6, r#"opacity="0.8""#);
        }
        if en.carry {
            s.diamond(a2, b2, 6.0, lc, "none", 0.0);
        } else {
            s.circle(a2, b2, 5.5, lc, "#000", 0.6, "");
        }
        s.layer = None;
    }
    let (px0, py0) = (960.0, 200.0);
    panel(&mut s, px0, py0, 580.0, 720.0);
    s.t(px0 + 28.0, py0 + 40.0, "ENTRY CONTRIBUTORS", 14.0, 800, theme::TEXT, Anchor::Start);
    let mut by: std::collections::HashMap<Option<u64>, usize> = std::collections::HashMap::new();
    for en in &t.entries {
        *by.entry(en.player).or_default() += 1;
    }
    let mut v: Vec<(Option<u64>, usize)> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    let top: Vec<&(Option<u64>, usize)> = v.iter().take(5).collect();
    let others: usize = v.iter().skip(5).map(|x| x.1).sum();
    let mut yy = py0 + 90.0;
    for (pl, n) in top {
        s.t(px0 + 28.0, yy, &c.pname(sd, *pl), 15.0, 500, theme::TEXT, Anchor::Start);
        s.t(px0 + 552.0, yy, &n.to_string(), 15.0, 800, theme::TEXT, Anchor::End);
        s.line(px0 + 28.0, yy + 14.0, px0 + 552.0, yy + 14.0, theme::LINE, 1.0, "");
        yy += 50.0;
    }
    if others > 0 {
        s.t(px0 + 28.0, yy, "Others", 15.0, 500, theme::TEXT, Anchor::Start);
        s.t(px0 + 552.0, yy, &others.to_string(), 15.0, 800, theme::TEXT, Anchor::End);
        yy += 50.0;
    }
    yy += 20.0;
    s.t(px0 + 28.0, yy, "ENTRY METHOD LEGEND", 12.0, 800, theme::MUTED, Anchor::Start);
    let passes = t.entries.iter().filter(|e| !e.carry).count();
    legend_item(&mut s, px0 + 28.0, yy + 40.0, "circle", &col, &format!("Pass entry ({passes})"));
    legend_item(&mut s, px0 + 28.0, yy + 75.0, "diamond", &col, &format!("Carry / take-on ({})", t.entries.len() - passes));
    legend_item(&mut s, px0 + 28.0, yy + 110.0, "line", &col, "No shot followed");
    legend_item(&mut s, px0 + 28.0, yy + 145.0, "line", "#ffffff", &format!("Shot followed ({shot_n})"));
    s.t(px0 + 28.0, yy + 190.0, &format!("Total entries: {}", t.entries.len()), 15.0, 800, theme::TEXT, Anchor::Start);
    footer(&mut s, c, "Successful movements from outside into the penalty area; highlighted entries were followed by a shot in the same possession. Repeated entries in one possession count separately; corner deliveries are excluded.");
    Board::new(&format!("08_box_entries_{}", side_tag(sd)), "Box entries", s)
}

// ---------------------------------------------------------------- 9 passing by third
pub fn passing_by_third(c: &Ctx, sd: Side) -> Board {
    let mut s = Svg::new(W, H);
    let t = c.a.t(sd);
    header(&mut s, c, &format!("Passing by Third · {}", c.name(sd)), "Every pass, split by the third it was played from · completed in team colour, incomplete grey · key passes white", Scope::Team(sd));
    let col = c.col(sd).to_string();
    let names = ["BUILD-UP · OWN THIRD", "PROGRESSION · MIDDLE THIRD", "FINAL THIRD"];
    for k in 0..3 {
        let bt = &t.by_third[k];
        let bx = 40.0 + k as f64 * 520.0;
        s.h(bx, 210.0, names[k], 24.0, &col, Anchor::Start);
        let n = bt.passes.len().max(1) as f64;
        s.t(bx, 238.0, &format!("{} passes · {:.0}% completed · {:.0}% forward", bt.passes.len(), bt.completed as f64 / n * 100.0, bt.forward as f64 / n * 100.0), 14.0, 500, theme::TEXT, Anchor::Start);
        let sub = if k == 2 { format!("{} completed into the box · {} key passes", bt.into_next, bt.key_passes) } else { format!("{} completed into the next third · {} key passes", bt.into_next, bt.key_passes) };
        s.t(bx, 260.0, &sub, 13.0, 400, theme::MUTED, Anchor::Start);
        let p = Pitch::up(bx, 300.0, 480.0, 700.0, 0.0);
        let (a, b) = match k {
            0 => (0.0, 35.0),
            1 => (35.0, 70.0),
            _ => (70.0, 105.0),
        };
        p.zone(&mut s, a, b, 0.0, 68.0, "#c9a227", 0.10);
        p.label_attack(&mut s);
        p.draw(&mut s);
        for x in [35.0, 70.0] {
            let (l1, y1) = p.p(x, 0.0);
            let (l2, y2) = p.p(x, 68.0);
            s.line(l1, y1, l2, y2, "#ffffff", 1.0, r#"stroke-dasharray="5 5" opacity="0.6""#);
        }
        let mut order: Vec<usize> = bt.passes.clone();
        order.sort_by_key(|&q| {
            let e = &c.m.events[q];
            (e.success as u8) + 2 * e.pass.as_ref().map(|p| p.key_pass as u8).unwrap_or(0)
        });
        for q in order {
            let e = &c.m.events[q];
            let key = e.pass.as_ref().map(|p| p.key_pass).unwrap_or(false);
            let lc = if key { "#ffffff" } else if e.success { col.as_str() } else { "#A3B3AA" };
            let (a1, b1) = p.p(e.x, e.y);
            let (a2, b2) = p.p(e.end().0, e.end().1);
            s.line(a1, b1, a2, b2, lc, if key { 2.4 } else { 1.0 }, if e.success { r#"opacity="0.75""# } else { r#"opacity="0.5""# });
            s.circle(a2, b2, 1.8, lc, "none", 0.0, "");
        }
    }
    footer(&mut s, c, "Each pitch holds the passes played from one third: completed in the team colour, incomplete in grey, key passes in white. Read the build-up, the progression and the final third separately, and the line under each title for how often the ball went on to the next stage.");
    Board::new(&format!("09_passing_by_third_{}", side_tag(sd)), "Passing by third", s)
}

// ---------------------------------------------------------------- 10 pass networks
pub fn pass_network(c: &Ctx, sd: Side, half: u8) -> Board {
    let mut s = Svg::new(W, H);
    let m = c.m;
    let ev = &m.events;
    let col = c.col(sd).to_string();
    let in_half = |e: &Event| if half == 1 { e.period == 1 } else { e.period >= 2 && e.period <= 4 };
    let (h0, h1) = {
        let ps: Vec<&PeriodInfo> = m.periods.iter().filter(|p| if half == 1 { p.period == 1 } else { p.period >= 2 && p.period <= 4 }).collect();
        (ps.iter().map(|p| p.start_t).fold(f64::INFINITY, f64::min), ps.iter().map(|p| p.end_t).fold(0.0, f64::max))
    };
    // positions and touches
    let mut pos: std::collections::HashMap<u64, (f64, f64, f64)> = std::collections::HashMap::new();
    for e in ev.iter().filter(|e| e.side == sd && in_half(e) && e.kind.is_on_ball() && !matches!(e.kind, Kind::Foul)) {
        if let Some(p) = e.player {
            let en = pos.entry(p).or_insert((0.0, 0.0, 0.0));
            en.0 += e.x;
            en.1 += e.y;
            en.2 += 1.0;
        }
    }
    let mut pairs: std::collections::HashMap<(u64, u64), usize> = std::collections::HashMap::new();
    let mut links = 0;
    for (i, e) in ev.iter().enumerate() {
        if e.side == sd && in_half(e) && e.kind == Kind::Pass && e.success {
            if let (Some(a), Some(b)) = (e.player, c.a.receiver[i]) {
                if a != b && pos.contains_key(&b) {
                    let k = if a < b { (a, b) } else { (b, a) };
                    *pairs.entry(k).or_default() += 1;
                    links += 1;
                }
            }
        }
    }
    let team = m.team(sd);
    let participants: Vec<&Player> = team.players.iter().filter(|p| pos.contains_key(&p.id)).collect();
    let title = format!("Pass Network · {} · {} Half", c.name(sd), if half == 1 { "First" } else { "Second" });
    header(&mut s, c, &title, &format!("{} participants · each circle sits at the player's average touch position", participants.len()), Scope::Team(sd));
    let p = Pitch::up(60.0, 200.0, 740.0, 840.0, 0.0);
    p.label_attack(&mut s);
    p.draw(&mut s);
    let maxp = pairs.values().copied().max().unwrap_or(1) as f64;
    for ((a, b), n) in &pairs {
        if *n < 2 {
            continue;
        }
        let pa = pos[a];
        let pb = pos[b];
        let (x1, y1) = p.p(pa.0 / pa.2, pa.1 / pa.2);
        let (x2, y2) = p.p(pb.0 / pb.2, pb.1 / pb.2);
        let w = 1.0 + 9.0 * (*n as f64 / maxp);
        s.tip = Some(format!("{} ↔ {} · {} completed passes", c.pname(sd, Some(*a)), c.pname(sd, Some(*b)), n));
        s.line(x1, y1, x2, y2, "#A3B3AA", w, &format!(r#"opacity="{:.2}" stroke-linecap="round""#, 0.25 + 0.6 * (*n as f64 / maxp)));
    }
    // replacement dotted lines
    for pl in &participants {
        if let Some(r) = pl.replaced {
            if let (Some(pa), Some(pb)) = (pos.get(&pl.id), pos.get(&r)) {
                let (x1, y1) = p.p(pa.0 / pa.2, pa.1 / pa.2);
                let (x2, y2) = p.p(pb.0 / pb.2, pb.1 / pb.2);
                s.line(x1, y1, x2, y2, "#ffffff", 2.0, r#"stroke-dasharray="2 5" stroke-linecap="round""#);
            }
        }
    }
    let maxt = pos.values().map(|v| v.2).fold(1.0, f64::max);
    for pl in &participants {
        let v = pos[&pl.id];
        let (x, y) = p.p(v.0 / v.2, v.1 / v.2);
        let r = 14.0 + 10.0 * (v.2 / maxt).sqrt();
        let came_on = !pl.starter && pl.on_t >= h0 - 1.0 && pl.on_t <= h1;
        let went_off = pl.off_t.map(|o| o >= h0 && o <= h1 + 1.0).unwrap_or(false);
        s.tip = Some(format!("#{} {} · {:.0} touches this half{}{}", pl.shirt, pl.name, v.2, if came_on { " · came on" } else { "" }, if went_off { " · went off" } else { "" }));
        if came_on {
            s.square(x, y, r * 0.85, &col, "#ffffff", 1.5);
        } else if went_off {
            s.circle(x, y, r, "#1A2E24", "#ffffff", 3.0, "");
            s.circle(x, y, r - 5.0, &col, "none", 0.0, r#"opacity="0.65""#);
        } else {
            s.circle(x, y, r, &col, "#ffffff", 1.5, "");
        }
        let on = &c.th.team(sd).on_primary;
        s.t(x, y + 5.0, &pl.shirt.to_string(), 13.0, 800, on, Anchor::Middle);
        s.t(x, y + r + 16.0, &pl.short, 12.5, 700, theme::TEXT, Anchor::Middle);
    }
    // right panel
    let (px0, py0) = (840.0, 200.0);
    panel(&mut s, px0, py0, 700.0, 230.0);
    s.t(px0 + 24.0, py0 + 34.0, "TOP PAIRS", 13.0, 800, theme::TEXT, Anchor::Start);
    s.t(px0 + 676.0, py0 + 34.0, "PASSES BOTH WAYS", 11.0, 700, theme::MUTED, Anchor::End);
    let mut pv: Vec<(&(u64, u64), &usize)> = pairs.iter().collect();
    pv.sort_by(|a, b| b.1.cmp(a.1));
    for (i, ((a, b), n)) in pv.iter().take(4).enumerate() {
        let yy = py0 + 76.0 + i as f64 * 40.0;
        s.t(px0 + 24.0, yy, &format!("{} ↔ {}", c.pname(sd, Some(*a)), c.pname(sd, Some(*b))), 15.0, 700, theme::TEXT, Anchor::Start);
        s.h(px0 + 676.0, yy + 2.0, &n.to_string(), 26.0, theme::TEXT, Anchor::End);
        s.rect(px0 + 24.0, yy + 9.0, 600.0 * (**n as f64 / maxp), 4.0, &col, "");
    }
    panel(&mut s, px0, py0 + 250.0, 700.0, 470.0);
    let changes: Vec<&Player> = team.players.iter().filter(|p| !p.starter && p.on_t >= h0 - 1.0 && p.on_t <= h1).collect();
    s.t(px0 + 24.0, py0 + 284.0, "LINE-UP", 13.0, 800, theme::TEXT, Anchor::Start);
    s.t(px0 + 676.0, py0 + 284.0, &if changes.is_empty() { "NO CHANGES".to_string() } else { format!("{} CHANGE{}", changes.len(), if changes.len() == 1 { "" } else { "S" }) }, 11.0, 700, theme::MUTED, Anchor::End);
    s.t(px0 + 24.0, py0 + 314.0, "STARTED THE HALF", 10.5, 700, theme::MUTED, Anchor::Start);
    s.t(px0 + 380.0, py0 + 314.0, "CAME ON", 10.5, 700, theme::MUTED, Anchor::Start);
    let mut started: Vec<&Player> = team.players.iter().filter(|p| !p.on_t.is_nan() && p.on_t < h0 + 1.0 && p.off_t.map(|o| o > h0 + 1.0).unwrap_or(true)).collect();
    started.sort_by_key(|p| p.shirt);
    for (i, pl) in started.iter().enumerate().take(14) {
        let yy = py0 + 344.0 + i as f64 * 25.0;
        s.t(px0 + 24.0, yy, &pl.shirt.to_string(), 13.0, 700, theme::TEXT, Anchor::Start);
        s.t(px0 + 60.0, yy, &pl.short, 13.0, 600, theme::TEXT, Anchor::Start);
        if let Some(o) = pl.off_t.filter(|o| *o >= h0 && *o <= h1 + 1.0) {
            s.t(px0 + 330.0, yy, &format!("▼ {}", mins(o)), 12.0, 600, theme::MUTED, Anchor::End);
        }
    }
    if changes.is_empty() {
        s.t(px0 + 380.0, py0 + 344.0, "No changes in this half", 13.0, 400, theme::MUTED, Anchor::Start);
    }
    for (i, pl) in changes.iter().enumerate().take(6) {
        let yy = py0 + 344.0 + i as f64 * 50.0;
        s.t(px0 + 380.0, yy, &format!("▲  {}  {}", pl.shirt, pl.short), 13.0, 700, theme::TEXT, Anchor::Start);
        s.t(px0 + 676.0, yy, &mins(pl.on_t), 12.0, 600, theme::MUTED, Anchor::End);
        if let Some(r) = pl.replaced {
            s.t(px0 + 405.0, yy + 18.0, &format!("for {}", c.pname(sd, Some(r))), 11.5, 400, theme::MUTED, Anchor::Start);
        }
    }
    panel(&mut s, px0, py0 + 740.0, 700.0, 100.0);
    let mut deg: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    for ((a, b), n) in &pairs {
        *deg.entry(*a).or_default() += n;
        *deg.entry(*b).or_default() += n;
    }
    let hub = deg.iter().max_by_key(|x| x.1).map(|x| *x.0);
    s.h(px0 + 30.0, py0 + 800.0, &links.to_string(), 34.0, theme::TEXT, Anchor::Start);
    s.t(px0 + 30.0, py0 + 822.0, "COMPLETED LINKS", 10.0, 700, theme::MUTED, Anchor::Start);
    s.h(px0 + 220.0, py0 + 800.0, &participants.len().to_string(), 34.0, theme::TEXT, Anchor::Start);
    s.t(px0 + 220.0, py0 + 822.0, "PLAYERS USED", 10.0, 700, theme::MUTED, Anchor::Start);
    s.h(px0 + 380.0, py0 + 800.0, &c.pname(sd, hub).to_uppercase(), 30.0, theme::TEXT, Anchor::Start);
    s.t(px0 + 380.0, py0 + 822.0, "MOST PASSING ROUTES RAN THROUGH HIM", 10.0, 700, theme::MUTED, Anchor::Start);
    let mut lx = 60.0;
    lx = legend_item(&mut s, lx, 1070.0, "circle", &col, "STARTED, STAYED");
    lx = legend_item(&mut s, lx, 1070.0, "ring", "#ffffff", "WENT OFF");
    lx = legend_item(&mut s, lx, 1070.0, "square", &col, "CAME ON");
    legend_item(&mut s, lx, 1070.0, "dash", "#ffffff", "REPLACEMENT");
    footer(&mut s, c, "Circles are touch-based average player locations within the named half, numbered by shirt. Each line joins a pair weighted by completed passes in both directions (pairs with 2+ shown).");
    Board::new(&format!("10_pass_network_{}_h{}", side_tag(sd), half), "Pass network", s)
}
