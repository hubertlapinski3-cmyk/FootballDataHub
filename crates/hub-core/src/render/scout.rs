//! Season player card for the league scout: composed natively for a phone column (640 wide), from one
//! row of the scouting index (no match needed): hero, key figures, role percentile ring, average position.

use super::pitch::Pitch;
use super::svg::{Anchor, Scene, Svg};
use crate::catalog::*;

const PW: f64 = 640.0;
const PAPER: &str = "#F5F3EE";
const SAGE: &str = "#6BA68B";
const CARD: &str = "#12211A";
const CARD_HI: &str = "#1A2E24";
const HAIR: &str = "#24382D";
const SOFT: &str = "#A3B3AA";

fn wrap(text: &str, size: f64, max_w: f64) -> Vec<String> {
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

/// The season card of one player. `colour` fills the percentile wedges (the app passes its accent).
pub fn scout_scene(row: &ScoutPlayerRow, idx: &ScoutIndex, colour: &str) -> Scene {
    let role = row.role.unwrap_or(ScoutRole::CM);
    let league = League::from_slug(&row.league).map(|l| l.name()).unwrap_or(row.league.as_str());
    let labels = ring_labels(role);
    let pcts: Vec<f64> = labels.iter().map(|l| row.percentile_of(l)).collect();
    let pool_n = idx.pool(Some(role), LeagueFilter::All, idx.pool_minutes).len();
    let mut s = Svg::new(PW, 3000.0);

    // hero
    let mut y = 64.0;
    s.h(24.0, y, &row.name, 46.0, PAPER, Anchor::Start);
    y += 30.0;
    s.t(24.0, y, &format!("{} · {} · {}", row.team, league, role.label()), 17.0, 500, SOFT, Anchor::Start);
    y += 22.0;
    s.t(24.0, y, &format!("{:.0} minutes in {} matches this season", row.minutes, row.matches), 16.0, 400, SOFT, Anchor::Start);
    y += 26.0;

    // key figures: the three strongest wedges and minutes
    let mut order: Vec<usize> = (0..8).collect();
    order.sort_by(|a, b| pcts[*b].partial_cmp(&pcts[*a]).unwrap());
    let mut kpis: Vec<(String, String, String)> = order.iter().take(3).map(|&i| (format_metric(labels[i], row.value(labels[i])), labels[i].to_string(), format!("percentile {:.0}", pcts[i]))).collect();
    kpis.push((format!("{:.0}", row.minutes), "Minutes played".into(), String::new()));
    let (cw, ch, gap) = ((PW - 40.0 - 12.0) / 2.0, 128.0, 12.0);
    for (k, (v, l, r)) in kpis.iter().enumerate() {
        let x = 20.0 + (k % 2) as f64 * (cw + gap);
        let yy = y + (k / 2) as f64 * (ch + gap);
        s.rrect(x, yy, cw, ch, 16.0, CARD, "none", 0.0);
        s.h(x + 18.0, yy + 56.0, v, 54.0, if k < 3 { SAGE } else { PAPER }, Anchor::Start);
        s.t(x + 18.0, yy + 92.0, l, 16.0, 600, PAPER, Anchor::Start);
        if !r.is_empty() {
            s.rrect(x + 18.0, yy + 102.0, 124.0, 22.0, 11.0, CARD_HI, "none", 0.0);
            s.t(x + 80.0, yy + 118.0, r, 13.0, 500, SOFT, Anchor::Middle);
        }
    }
    y += 2.0 * ch + gap + 34.0;

    // percentile ring: eight role wedges, season per 90, against the role pool of all leagues
    s.t(24.0, y, "Season percentile ring", 19.0, 700, PAPER, Anchor::Start);
    y += 28.0;
    let (cx, cy, r0, r1) = (PW / 2.0, y + 190.0, 34.0, 132.0);
    for i in 0..8 {
        let a0 = -std::f64::consts::FRAC_PI_2 + i as f64 / 8.0 * std::f64::consts::TAU + 0.02;
        let a1 = a0 + std::f64::consts::TAU / 8.0 - 0.04;
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
        s.tip = Some(format!("{}: {} per 90 · role percentile {:.0}", labels[i], format_metric(labels[i], row.value(labels[i])), pcts[i]));
        s.path(&wedge(rr), colour, "none", 0.0, r#"opacity="0.95""#);
        // the grey line marks the middle of the pool
        let ra = r0 + (r1 - r0) * 0.5;
        let pts: Vec<(f64, f64)> = (0..=12)
            .map(|k| {
                let a = a0 + (a1 - a0) * k as f64 / 12.0;
                (cx + ra * a.cos(), cy + ra * a.sin())
            })
            .collect();
        s.polyline(&pts, SOFT, 2.0, "");
        let am = (a0 + a1) / 2.0;
        let (lx, ly) = (cx + (r1 + 16.0) * am.cos(), cy + (r1 + 16.0) * am.sin());
        let anchor = if am.cos() > 0.3 { Anchor::Start } else if am.cos() < -0.3 { Anchor::End } else { Anchor::Middle };
        let dy = if am.sin() < -0.5 { -22.0 } else if am.sin() > 0.5 { 14.0 } else { -4.0 };
        s.t(lx, ly + dy, labels[i], 14.5, 600, PAPER, anchor);
        let top = order.iter().take(3).any(|&o| o == i);
        s.t(lx, ly + dy + 17.0, &format_metric(labels[i], row.value(labels[i])), 15.0, 800, if top { SAGE } else { SOFT }, anchor);
    }
    y = cy + r1 + 62.0;
    let note = format!("Ranked against {pool_n} {} players with {:.0}+ minutes in the seven leagues, season per 90. The grey ring marks the middle of the pool.", role.label(), idx.pool_minutes);
    for l in wrap(&note, 14.5, PW - 48.0) {
        s.t(24.0, y, &l, 14.5, 400, SOFT, Anchor::Start);
        y += 20.0;
    }
    y += 22.0;

    // average position on a vertical pitch, attacking up
    s.t(24.0, y, "Average position", 19.0, 700, PAPER, Anchor::Start);
    y += 14.0;
    let ph = 460.0;
    s.rrect(20.0, y, PW - 40.0, ph + 28.0, 16.0, CARD, "none", 0.0);
    let pm = Pitch::up(40.0, y + 34.0, PW - 80.0, ph - 20.0, 0.0);
    pm.label_attack(&mut s);
    pm.draw(&mut s);
    if row.avg_x > 0.0 {
        let (px, py) = pm.p(row.avg_x as f64, row.avg_y as f64);
        s.tip = Some(format!("Average position this season: {:.0} m up the pitch", row.avg_x));
        s.circle(px, py, 14.0, colour, PAPER, 2.0, "");
    }
    y += ph + 28.0 + 30.0;
    for l in wrap("Season average of the positions of this player's recorded actions, in the team's own attacking direction.", 14.5, PW - 48.0) {
        s.t(24.0, y, &l, 14.5, 400, SOFT, Anchor::Start);
        y += 20.0;
    }
    y += 24.0;
    s.h = y;
    s.scene.h = y as f32;
    s.scene
}
