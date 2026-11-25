//! Templated match report generated from the derived numbers.

use crate::derive::metrics::*;
use crate::model::*;
use crate::render::Board;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub paragraphs: Vec<String>,
    pub boards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub headline: String,
    pub standfirst: String,
    pub sections: Vec<Section>,
}

fn pl(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", n, if n == 1 { one } else { many })
}

pub fn generate(m: &Match, a: &Analysis, boards: &[Board]) -> Report {
    let (hn, an) = (m.team(Side::Home).name.clone(), m.team(Side::Away).name.clone());
    let name = |s: Side| m.team(s).name.clone();
    let (hs, as_) = (m.team(Side::Home).score, m.team(Side::Away).score);
    let (h, aw) = (a.t(Side::Home), a.t(Side::Away));
    let lead = if h.npxg >= aw.npxg { Side::Home } else { Side::Away };
    let tl = a.t(lead);
    let to = a.t(lead.other());
    let pick = |prefix: &str| -> Vec<String> { boards.iter().filter(|b| b.id.starts_with(prefix)).map(|b| b.id.clone()).collect() };

    let result_word = if hs == as_ { "drew" } else { "" };
    let headline = if hs == as_ {
        format!("{} {}–{} {}", hn, hs, as_, an)
    } else {
        format!("{} {}–{} {}", hn, hs, as_, an)
    };
    let mut stand = if hs == as_ {
        format!("{} and {} {} {}–{}.", hn, an, result_word, hs, as_)
    } else {
        let w = if hs > as_ { Side::Home } else { Side::Away };
        format!("{} beat {} {}–{}.", name(w), name(w.other()), hs.max(as_), hs.min(as_))
    };
    stand.push_str(&format!(" {} had {:.0}% of the ball and {:.0}% of the final-third passing.", name(lead), tl.possession_pct * 100.0, tl.field_tilt * 100.0));

    // Score narrative: leads lost, never led, came from behind, clean sheets.
    let goals = m.goals_timeline();
    let mut sc = [0i32; 2];
    let mut led = [false; 2];
    let mut trailed = [false; 2];
    for (s, _) in &goals {
        sc[s.idx()] += 1;
        if sc[0] > sc[1] {
            led[0] = true;
            trailed[1] = true;
        }
        if sc[1] > sc[0] {
            led[1] = true;
            trailed[0] = true;
        }
    }
    let mut story = vec![];
    for s in Side::both() {
        let (gf, ga) = (m.team(s).score, m.team(s.other()).score);
        if led[s.idx()] && gf <= ga {
            story.push(format!("{} led and could not hold on.", name(s)));
        }
        if trailed[s.idx()] && gf > ga {
            story.push(format!("{} came from behind to win.", name(s)));
        }
        if !led[s.idx()] && gf < ga {
            story.push(format!("{} never led.", name(s)));
        }
        if ga == 0 {
            story.push(format!("{} kept a clean sheet.", name(s)));
        }
    }

    let mut sections = vec![];
    // 1. How the match was decided
    let mut p1 = vec![];
    if goals.is_empty() {
        p1.push("Neither side scored.".into());
    } else {
        let parts: Vec<String> = goals.iter().map(|(s, e)| {
            let og = e.shot.as_ref().unwrap().own_goal;
            let g = a.goals.iter().find(|g| m.events[g.shot].id == e.id);
            let how = g.map(|g| format!(", {} ({} passes, {:.0} s)", g.label.to_lowercase(), g.passes, g.secs)).unwrap_or_default();
            format!("{} {} for {}{}{}", e.label_minute(), m.player_name(e.side, e.player), name(*s), if og { " (own goal)" } else { "" }, how)
        }).collect();
        p1.push(format!("The goals came in this order: {}.", parts.join("; ")));
    }
    let wood: Vec<String> = m.events.iter().filter(|e| e.shot.as_ref().map(|s| s.outcome == ShotOutcome::Woodwork).unwrap_or(false)).map(|e| format!("{} hit the woodwork on {}", m.player_name(e.side, e.player), e.label_minute())).collect();
    if !wood.is_empty() {
        p1.push(format!("Around the goals: {}.", wood.join("; ")));
    }
    p1.extend(story.clone());
    if (tl.npxg - to.npxg).abs() >= 0.5 {
        p1.push(format!("On chances {} deserved more: {:.2} expected goals to {:.2}.", name(lead), tl.npxg, to.npxg));
    } else {
        p1.push(format!("The chances were close: {:.2} expected goals to {:.2}.", tl.npxg, to.npxg));
    }
    let mut b = pick("01_");
    b.extend(pick("02_"));
    sections.push(Section { title: "How the match was decided".into(), paragraphs: p1, boards: b });

    // 2. What the chances were worth
    let w = &a.win;
    let live = &a.live;
    let mut p2 = vec![format!(
        "Replay every shot at its expected-goals value and {} win {:.0}% of the time, {} {:.0}%, with {:.0}% draws: {:.1} expected points to {:.1}. {} were the likelier winner for {:.0}% of the match, {} for {:.0}%.",
        hn, w.p_home * 100.0, an, w.p_away * 100.0, w.p_draw * 100.0, w.xpts_home, w.xpts_away, hn, live.likelier_home * 100.0, an, live.likelier_away * 100.0
    )];
    p2.push(format!(
        "{} took {} and {} brought a shot within ten seconds. {} took {} and {} brought a shot within ten seconds.",
        hn, pl(h.corners, "corner", "corners"), h.corners_shot10, an, pl(aw.corners, "corner", "corners"), aw.corners_shot10
    ));
    let mut b = pick("03_");
    b.extend(pick("04_"));
    sections.push(Section { title: "What the chances were worth".into(), paragraphs: p2, boards: b });

    // 3. Creation corridors and shot anatomy
    let mut p3 = vec![];
    for s in Side::both() {
        let t = a.t(s);
        p3.push(format!(
            "{} completed {} half-space passes ({} left, {} right) and {} into Zone 14. They produced {:.2} non-penalty xG: {} in the six-yard box ({:.2}), {} inside the box ({:.2}) and {} from outside ({:.2}), including {} and {} first-touch efforts.",
            name(s), t.hs_left.len() + t.hs_right.len(), t.hs_left.len(), t.hs_right.len(), t.z14.len(), t.npxg, pl(t.six.n, "shot", "shots"), t.six.xg, t.in_box.n, t.in_box.xg, t.outside.n, t.outside.xg, pl(t.headed.n, "header", "headers"), t.first_touch.n
        ));
    }
    let mut b = pick("05_");
    b.extend(pick("06_"));
    b.extend(pick("07_"));
    sections.push(Section { title: "Creation corridors and shot anatomy".into(), paragraphs: p3, boards: b });

    // 4/5. Each team with and without the ball
    for s in Side::both() {
        let t = a.t(s);
        let o = a.t(s.other());
        let form = m.team(s).formations.first().map(|f| f.name.clone()).unwrap_or_else(|| "their usual shape".into());
        let lanes = ["left", "centre", "right"];
        let (li, lv) = t.ft_lanes.iter().enumerate().max_by_key(|x| x.1).map(|(i, v)| (i, *v)).unwrap_or((1, 0));
        let shot_entries = t.entries.iter().filter(|e| e.shot_followed).count();
        let with = vec![
            format!("Set up in a {}, {} had {:.0}% of possession.", form, name(s), t.possession_pct * 100.0),
            format!("{:.0}% of their attempts to play out from their own third reached midfield cleanly; they completed {} passes from their own third into midfield.", t.buildup_success as f64 / t.buildup_attempts.max(1) as f64 * 100.0, t.own_third_to_mid),
            format!("In midfield they completed {:.0}% of their passes and reached the final third {} times; {:.0}% of those entries went down the {}.", t.mid_pass_pct * 100.0, t.ft_entries, lv as f64 / t.ft_entries.max(1) as f64 * 100.0, lanes[li]),
            format!("They got into the box {} and {} of those entries led to a shot. It added up to {} and {:.2} expected goals, {} clear.", pl(t.entries.len(), "time", "times"), shot_entries, pl(t.shots.len(), "shot", "shots"), t.xg, t.clear_chances.len()),
        ];
        let mut b = pick(&format!("08_box_entries_{}", if s == Side::Home { "home" } else { "away" }));
        b.extend(pick(&format!("09_passing_by_third_{}", if s == Side::Home { "home" } else { "away" })));
        sections.push(Section { title: format!("{} with the ball", name(s)), paragraphs: with, boards: b });
        let shots_hr = t.high_regains.iter().filter(|r| r.outcome == RegainOutcome::Shot).count();
        let xg_hr: f64 = t.high_regains.iter().map(|r| r.xg).sum();
        let ppda = t.ppda.map(|v| format!("{v:.1}")).unwrap_or("–".into());
        let without = vec![
            format!("{} allowed {} build-up passes for every defensive action and won the ball on average {:.0} m from their own goal.", name(s), ppda, t.def_height),
            format!("They won the ball back high {} and {} of those regains led to a shot, worth {:.2} expected goals.", pl(t.high_regains.len(), "time", "times"), shots_hr, xg_hr),
            format!("Closer to their own goal, {} got into the box {} and shot {} times.", name(s.other()), pl(o.entries.len(), "time", "times"), o.shots.len()),
            format!("Losing the ball cost them {} times: that many of their {} open-play losses became a {} shot inside twelve seconds, {:.2} expected goals in all.", t.losses_to_shot, t.open_play_losses, name(s.other()), t.losses_to_shot_xg),
        ];
        let mut b = pick(&format!("11_defensive_activity_{}", if s == Side::Home { "home" } else { "away" }));
        b.extend(pick(&format!("12_high_regains_{}", if s == Side::Home { "home" } else { "away" })));
        sections.push(Section { title: format!("{} without the ball", name(s)), paragraphs: without, boards: b });
    }

    // 6. Pressing through the match
    let mut p6 = vec![];
    for s in Side::both() {
        let t = a.t(s);
        let best = t.ppda_bins.iter().filter(|b| b.1.is_some()).min_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
        let f = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or("–".into());
        let change = match (t.ppda_h1, t.ppda_h2) {
            (Some(a1), Some(a2)) if a2 > a1 * 1.4 => "stopped pressing after the break",
            (Some(a1), Some(a2)) if a2 < a1 * 0.7 => "pressed harder after the break",
            _ => "pressed at much the same intensity in both halves",
        };
        p6.push(format!("{} {} (PPDA {} before, {} after){}.", name(s), change, f(t.ppda_h1), f(t.ppda_h2), best.map(|b| format!(", and were at their most aggressive between {} ({:.1})", b.0, b.1.unwrap())).unwrap_or_default()));
        let hto = t.high_turnovers.iter().filter(|x| x.shot.is_some()).count();
        p6.push(format!("{} forced {} high turnovers in the attacking third ({} leading to a shot) and logged {}.", name(s), t.high_turnovers.len(), hto, pl(t.own_errors.len(), "defensive error", "defensive errors")));
    }
    let mut b = pick("14_");
    b.extend(pick("13_"));
    sections.push(Section { title: "Pressing through the match".into(), paragraphs: p6, boards: b });

    // 7. Changes and game state
    let mut p7 = vec![];
    for s in Side::both() {
        let subs: Vec<&Player> = m.team(s).players.iter().filter(|p| !p.starter && !p.on_t.is_nan()).collect();
        if subs.is_empty() {
            p7.push(format!("{} made no changes.", name(s)));
        } else {
            let first = subs.iter().map(|p| p.on_t).fold(f64::INFINITY, f64::min);
            let names: Vec<String> = subs.iter().take(3).map(|p| p.short.clone()).collect();
            p7.push(format!("{} made {}, the first on {}', bringing on {}{}.", name(s), pl(subs.len(), "change", "changes"), (first / 60.0).floor() as i64 + 1, names.join(", "), if subs.len() > 3 { " among others" } else { "" }));
        }
    }
    for r in &a.states {
        if r.minutes > 0.5 && r.label != "SCORE LEVEL" {
            p7.push(format!("With {} ({:.0} min), the xG ran {:.2} to {:.2}.", r.label.to_lowercase(), r.minutes, r.xg[0], r.xg[1]));
        }
    }
    let mut b = pick("16_");
    b.extend(pick("15_"));
    sections.push(Section { title: "Changes, game state and the last half-hour".into(), paragraphs: p7, boards: b });

    // 8. Players
    let mut ps: Vec<&PlayerStats> = a.players.iter().collect();
    ps.sort_by(|x, y| (y.vaep_off + y.vaep_def).partial_cmp(&(x.vaep_off + x.vaep_def)).unwrap());
    let top: Vec<String> = ps.iter().take(3).map(|p| format!("{} ({})", p.short, crate::render::svg::signed(p.vaep_off + p.vaep_def, 2))).collect();
    let mut p8 = vec![format!("On VAEP, which values every action by how much it changed the chance of scoring and of conceding, the biggest contributions came from {}.", top.join(", "))];
    for s in Side::both() {
        let scorers: Vec<String> = goals.iter().filter(|(gs, e)| *gs == s && !e.shot.as_ref().unwrap().own_goal).map(|(_, e)| m.player_name(e.side, e.player)).collect();
        if !scorers.is_empty() {
            p8.push(format!("For {}, {} scored.", name(s), scorers.join(" and ")));
        }
        if let Some(c) = a.players.iter().filter(|p| p.side == Some(s)).max_by(|x, y| x.inferred_xa.partial_cmp(&y.inferred_xa).unwrap()) {
            if c.inferred_xa > 0.1 {
                p8.push(format!("{} set up chances worth {:.2} expected goals with {}.", c.name, c.inferred_xa, pl(c.progressive_passes, "progressive pass", "progressive passes")));
            }
        }
    }
    let mut b = pick("17_");
    b.extend(pick("26_"));
    sections.push(Section { title: "The players who shaped it".into(), paragraphs: p8, boards: b });

    // 9. What decided it
    let fin = |t: &TeamStats| if t.xgot < t.xg_on_target { "struck their shots worse than the chances were worth" } else { "struck their shots better than the chances were worth" };
    let p9 = vec![
        format!("{} held {:.0}% of the final-third passing and carried the stronger chance edge ({:.2} expected goals to {:.2}).", name(lead), tl.field_tilt * 100.0, tl.npxg, to.npxg),
        format!("{} {}; {} {}.", name(Side::Home), fin(h), name(Side::Away), fin(aw)),
    ];
    let mut b = pick("18_");
    b.extend(pick("19_"));
    b.extend(pick("24_"));
    sections.push(Section { title: "What decided it".into(), paragraphs: p9, boards: b });

    // 10. Visual guide
    let guide: Vec<String> = boards.iter().enumerate().filter(|(_, b)| !b.id.starts_with("31_")).map(|(i, b)| {
        let next = boards.get(i + 1).map(|n| format!(" Next: {}.", n.title)).unwrap_or_default();
        format!("{}: {}.{}", b.title, crate::render::section_of(&b.id), next)
    }).collect();
    sections.push(Section { title: "Complete visual guide".into(), paragraphs: guide, boards: vec![] });

    Report { headline, standfirst: stand, sections }
}

pub fn to_markdown(r: &Report) -> String {
    let mut s = format!("# {}\n\n{}\n\n", r.headline, r.standfirst);
    for sec in &r.sections {
        s.push_str(&format!("## {}\n\n", sec.title));
        for p in &sec.paragraphs {
            s.push_str(p);
            s.push_str("\n\n");
        }
        for b in &sec.boards {
            s.push_str(&format!("![{b}](boards/{b}.png)\n\n"));
        }
    }
    s
}
