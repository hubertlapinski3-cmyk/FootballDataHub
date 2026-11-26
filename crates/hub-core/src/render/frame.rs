//! Shared board chrome: header (kicker, headline, subtitle, crests, score),
//! team-colour rule and footer.

use super::svg::*;
use crate::derive::metrics::Analysis;
use crate::model::*;
use crate::theme::{self, MatchTheme};

pub struct Ctx<'a> {
    pub m: &'a Match,
    pub a: &'a Analysis,
    pub th: &'a MatchTheme,
    pub crests: [Option<String>; 2],
    pub creator: String,
    pub logo: Option<String>,
    pub footer_tag: String,
}

impl<'a> Ctx<'a> {
    pub fn col(&self, s: Side) -> &str {
        self.th.c(s)
    }
    pub fn name(&self, s: Side) -> String {
        self.m.team(s).name.clone()
    }
    pub fn short(&self, s: Side) -> String {
        self.m.team(s).short_name.clone()
    }
    pub fn pname(&self, side: Side, id: Option<u64>) -> String {
        self.m.player_name(side, id)
    }
}

pub enum Scope {
    Full,
    Team(Side),
    Player(Side),
}

/// Fit a team name into `max_w` px: full -> short -> abbreviation -> 3-letter code.
pub fn fit_name(m: &Match, s: Side, size: f64, max_w: f64) -> String {
    let t = m.team(s);
    let cands = [t.name.to_uppercase(), t.short_name.to_uppercase(), abbreviate(&t.name), code3(&t.name)];
    for c in cands.iter() {
        if c.chars().count() as f64 * size * 0.68 <= max_w {
            return c.clone();
        }
    }
    code3(&t.name)
}
fn abbreviate(n: &str) -> String {
    let w: Vec<&str> = n.split_whitespace().collect();
    if w.len() >= 2 {
        let last = w.last().unwrap();
        format!("{}. {}", &w[0][..1], last).to_uppercase()
    } else {
        n.to_uppercase()
    }
}
fn code3(n: &str) -> String {
    let w: Vec<&str> = n.split_whitespace().filter(|x| x.len() > 2).collect();
    let s = if w.len() >= 3 { w.iter().take(3).map(|x| &x[..1]).collect::<String>() } else { n.chars().filter(|c| c.is_alphabetic()).take(3).collect() };
    s.to_uppercase()
}

pub fn crest(s: &mut Svg, cx: f64, cy: f64, r: f64, data: &Option<String>, name: &str, col: &str, key: &str) {
    match data {
        Some(href) => s.image_keyed(cx - r, cy - r, 2.0 * r, 2.0 * r, href, Some(key)),
        None => {
            s.circle(cx, cy, r, col, "none", 0.0, "");
            let ini: String = name.split_whitespace().filter(|w| w.len() > 2 || name.split_whitespace().count() == 1).take(2).map(|w| w.chars().next().unwrap()).collect::<String>().to_uppercase();
            let on = if crate::theme::Rgb::hex(col).map(|c| c.luminance() > 0.4).unwrap_or(false) { "#000" } else { "#fff" };
            s.h(cx, cy + r * 0.38, &ini, r * 1.05, on, Anchor::Middle);
        }
    }
}

pub fn header(s: &mut Svg, c: &Ctx, title: &str, subtitle: &str, scope: Scope) {
    let w = s.w;
    s.rrect(18.0, 14.0, w - 36.0, 120.0, 10.0, "#12211A", "#24382D", 1.0);
    // headline auto-shrink to fit left area
    let right_w = 520.0;
    let max_w = w - 36.0 - right_w - 30.0;
    let mut size = 44.0;
    while text_w(title, size, true) > max_w && size > 24.0 {
        size -= 2.0;
    }
    // HighlightText-style headline: team names inside the sentence carry their colours
    let runs = highlight_runs(title, &[(c.name(Side::Home), c.col(Side::Home).to_string()), (c.short(Side::Home), c.col(Side::Home).to_string()), (c.name(Side::Away), c.col(Side::Away).to_string()), (c.short(Side::Away), c.col(Side::Away).to_string())]);
    s.text_runs(36.0, 88.0, &runs, size, 700, Anchor::Start, HEAD);
    s.t(36.0, 114.0, subtitle, 13.0, 400, theme::MUTED, Anchor::Start);
    // teams + score
    let rx = w - 60.0;
    let cy = 52.0;
    let hc = c.col(Side::Home).to_string();
    let ac = c.col(Side::Away).to_string();
    crest(s, rx - 470.0, cy, 20.0, &c.crests[0], &c.name(Side::Home), &hc, "crest:0");
    crest(s, rx - 20.0, cy, 20.0, &c.crests[1], &c.name(Side::Away), &ac, "crest:1");
    let hn = fit_name(c.m, Side::Home, 15.0, 112.0);
    let an = fit_name(c.m, Side::Away, 15.0, 112.0);
    s.t(rx - 335.0, cy + 6.0, &hn, 15.0, 800, &hc, Anchor::End);
    s.t(rx - 160.0, cy + 6.0, &an, 15.0, 800, &ac, Anchor::Start);
    let score = format!("{} — {}", c.m.team(Side::Home).score, c.m.team(Side::Away).score);
    s.h(rx - 247.0, cy + 10.0, &score, 30.0, theme::TEXT, Anchor::Middle);
    let scope_s = match scope {
        Scope::Full => "FULL MATCH".to_string(),
        Scope::Team(t) => format!("TEAM VIEW · {}", c.name(t).to_uppercase()),
        Scope::Player(t) => format!("PLAYER VIEW · {}", c.name(t).to_uppercase()),
    };
    s.t(rx - 470.0, 104.0, &scope_s, 10.5, 700, theme::MUTED, Anchor::Start);
    // team-colour rule
    s.rect(18.0, 136.0, (w - 36.0) / 2.0, 4.0, &hc, "");
    s.rect(18.0 + (w - 36.0) / 2.0, 136.0, (w - 36.0) / 2.0, 4.0, &ac, "");
}

pub fn footer(s: &mut Svg, _c: &Ctx, how: &str) {
    s.how = how.to_string();
    s.scene.footer_start = s.scene.prims.len();
    let h = s.h;
    // wrap the how-to-read sentence at ~170 chars
    let mut lines = vec![];
    let mut cur = String::new();
    for word in how.split_whitespace() {
        if cur.len() + word.len() > 165 {
            lines.push(cur.clone());
            cur.clear();
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    let n = lines.len() as f64;
    for (i, l) in lines.iter().enumerate() {
        s.t(36.0, h - 24.0 - (n - 1.0 - i as f64) * 18.0, l, 13.0, 400, "#F5F3EE", Anchor::Start);
    }
}

pub fn panel(s: &mut Svg, x: f64, y: f64, w: f64, h: f64) {
    s.rrect(x, y, w, h, 8.0, "#12211A", "#24382D", 1.0);
}

pub fn legend_item(s: &mut Svg, x: f64, y: f64, kind: &str, col: &str, label: &str) -> f64 {
    // a legend entry whose key matches a layer becomes a tap-to-toggle control in the app
    let key = label.split(" (").next().unwrap_or(label).to_string();
    if s.scene.layers.iter().any(|l| *l == key) {
        let w = 22.0 + text_w(label, 12.5, false);
        s.legend_hit(&key, x - 4.0, y - 18.0, w + 8.0, 26.0);
    }
    let saved = s.layer.take();
    match kind {
        "circle" => s.circle(x + 6.0, y - 5.0, 6.0, col, "none", 0.0, ""),
        "ring" => s.circle(x + 6.0, y - 5.0, 6.0, "none", col, 2.0, ""),
        "square" => s.square(x + 6.0, y - 5.0, 5.5, col, "none", 0.0),
        "diamond" => s.diamond(x + 6.0, y - 5.0, 7.0, col, "none", 0.0),
        "triangle" => s.triangle(x + 6.0, y - 5.0, 7.0, col, "none", 0.0),
        "star" => s.star(x + 6.0, y - 5.0, 8.0, col, "none"),
        "cross" => s.cross(x + 6.0, y - 5.0, 5.0, col, 2.4),
        "line" => s.line(x, y - 5.0, x + 18.0, y - 5.0, col, 3.0, ""),
        "dash" => s.line(x, y - 5.0, x + 18.0, y - 5.0, col, 2.0, r#"stroke-dasharray="4 3""#),
        _ => {}
    }
    s.t(x + 22.0, y, label, 12.5, 500, theme::TEXT, Anchor::Start);
    s.layer = saved;
    x + 22.0 + text_w(label, 12.5, false) + 24.0
}

/// Split `text` into (run, colour) pairs, colouring every occurrence of the given names.
pub fn highlight_runs(text: &str, names: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut rest = text;
    while !rest.is_empty() {
        let hit = names.iter().filter(|(n, _)| !n.is_empty()).filter_map(|(n, c)| rest.find(n.as_str()).map(|i| (i, n, c))).min_by(|a, b| a.0.cmp(&b.0).then(b.1.len().cmp(&a.1.len())));
        match hit {
            Some((i, n, c)) => {
                if i > 0 {
                    out.push((rest[..i].to_string(), theme::TEXT.to_string()));
                }
                out.push((n.clone(), c.clone()));
                rest = &rest[i + n.len()..];
            }
            None => {
                out.push((rest.to_string(), theme::TEXT.to_string()));
                rest = "";
            }
        }
    }
    out
}
