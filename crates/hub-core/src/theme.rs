//! Per-match colour resolution: candidates -> normalisation -> clash handling
//! -> semantic-colour collision handling -> derived tokens.

use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const BG: &str = "#0b0a12";
pub const BG2: &str = "#140c18";
pub const PANEL: &str = "#100d17";
pub const LINE: &str = "#2a2633";
pub const TEXT: &str = "#f2f2f5";
pub const MUTED: &str = "#a7a5b2";
pub const GREY: &str = "#6b6878";
pub const PITCH_LINE: &str = "#e8e8ee";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb(pub f64, pub f64, pub f64);

impl Rgb {
    pub fn hex(s: &str) -> Option<Rgb> {
        let s = s.trim().trim_start_matches('#');
        let s = if s.len() == 3 { s.chars().flat_map(|c| [c, c]).collect::<String>() } else { s.to_string() };
        if s.len() < 6 {
            return None;
        }
        let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok().map(|v| v as f64 / 255.0);
        Some(Rgb(p(0)?, p(2)?, p(4)?))
    }
    pub fn to_hex(self) -> String {
        let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", c(self.0), c(self.1), c(self.2))
    }
    fn lin(v: f64) -> f64 {
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    }
    fn delin(v: f64) -> f64 {
        if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    }
    pub fn luminance(self) -> f64 {
        0.2126 * Self::lin(self.0) + 0.7152 * Self::lin(self.1) + 0.0722 * Self::lin(self.2)
    }
    pub fn contrast(self, o: Rgb) -> f64 {
        let (a, b) = (self.luminance(), o.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    pub fn oklab(self) -> (f64, f64, f64) {
        let (r, g, b) = (Self::lin(self.0), Self::lin(self.1), Self::lin(self.2));
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
        let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
        (
            0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
            0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
        )
    }
    pub fn from_oklab(l: f64, a: f64, b: f64) -> Rgb {
        let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
        let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
        let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
        let r = 4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_;
        let g = -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_;
        let bb = -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_;
        Rgb(Self::delin(r.clamp(0.0, 1.0)), Self::delin(g.clamp(0.0, 1.0)), Self::delin(bb.clamp(0.0, 1.0)))
    }
    pub fn oklch(self) -> (f64, f64, f64) {
        let (l, a, b) = self.oklab();
        (l, (a * a + b * b).sqrt(), b.atan2(a))
    }
    pub fn from_oklch(l: f64, c: f64, h: f64) -> Rgb {
        Self::from_oklab(l, c * h.cos(), c * h.sin())
    }
    pub fn lab(self) -> (f64, f64, f64) {
        let (r, g, b) = (Self::lin(self.0), Self::lin(self.1), Self::lin(self.2));
        let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
        let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
        (116.0 * f(y) - 16.0, 500.0 * (f(x) - f(y)), 200.0 * (f(y) - f(z)))
    }
    pub fn mix(self, o: Rgb, t: f64) -> Rgb {
        let (a1, b1, c1) = self.oklab();
        let (a2, b2, c2) = o.oklab();
        Rgb::from_oklab(a1 + (a2 - a1) * t, b1 + (b2 - b1) * t, c1 + (c2 - c1) * t)
    }
}

/// CIEDE2000 colour difference.
pub fn delta_e2000(c1: Rgb, c2: Rgb) -> f64 {
    let (l1, a1, b1) = c1.lab();
    let (l2, a2, b2) = c2.lab();
    let deg = std::f64::consts::PI / 180.0;
    let c1s = (a1 * a1 + b1 * b1).sqrt();
    let c2s = (a2 * a2 + b2 * b2).sqrt();
    let cb = (c1s + c2s) / 2.0;
    let g = 0.5 * (1.0 - (cb.powi(7) / (cb.powi(7) + 25f64.powi(7))).sqrt());
    let a1p = (1.0 + g) * a1;
    let a2p = (1.0 + g) * a2;
    let c1p = (a1p * a1p + b1 * b1).sqrt();
    let c2p = (a2p * a2p + b2 * b2).sqrt();
    let h = |b: f64, a: f64| {
        let v = b.atan2(a) / deg;
        if v < 0.0 { v + 360.0 } else { v }
    };
    let h1p = h(b1, a1p);
    let h2p = h(b2, a2p);
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let mut dh = h2p - h1p;
    if c1p * c2p != 0.0 {
        if dh > 180.0 {
            dh -= 360.0
        } else if dh < -180.0 {
            dh += 360.0
        }
    } else {
        dh = 0.0
    }
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh * deg / 2.0).sin();
    let lb = (l1 + l2) / 2.0;
    let cbp = (c1p + c2p) / 2.0;
    let mut hb = h1p + h2p;
    if c1p * c2p != 0.0 {
        if (h1p - h2p).abs() > 180.0 {
            hb = if hb < 360.0 { (hb + 360.0) / 2.0 } else { (hb - 360.0) / 2.0 };
        } else {
            hb /= 2.0;
        }
    }
    let t = 1.0 - 0.17 * ((hb - 30.0) * deg).cos() + 0.24 * ((2.0 * hb) * deg).cos() + 0.32 * ((3.0 * hb + 6.0) * deg).cos() - 0.20 * ((4.0 * hb - 63.0) * deg).cos();
    let dtheta = 30.0 * (-((hb - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cbp.powi(7) / (cbp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lb - 50.0).powi(2) / (20.0 + (lb - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cbp;
    let sh = 1.0 + 0.015 * cbp * t;
    let rt = -(2.0 * dtheta * deg).sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh)).sqrt()
}

#[derive(Debug, Clone, Serialize)]
pub struct TeamTheme {
    pub primary: String,
    pub muted: String,
    pub on_primary: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Semantic {
    pub goal: String,
    pub saved: String,
    pub woodwork: String,
    pub off_target: String,
    pub blocked: String,
    pub hs_left: String,
    pub hs_right: String,
    pub z14: String,
    pub cutback: String,
    pub through: String,
    pub touch: String,
    pub defending: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchTheme {
    pub bg: String,
    pub home: TeamTheme,
    pub away: TeamTheme,
    pub semantic: Semantic,
    pub decisions: Vec<String>,
}

impl MatchTheme {
    pub fn team(&self, s: Side) -> &TeamTheme {
        match s {
            Side::Home => &self.home,
            Side::Away => &self.away,
        }
    }
    pub fn c(&self, s: Side) -> &str {
        &self.team(s).primary
    }
    /// Sequential scale from background to the team colour (t in 0..1).
    pub fn seq(&self, s: Side, t: f64) -> String {
        Rgb::hex("#120d16").unwrap().mix(Rgb::hex(self.c(s)).unwrap(), t.clamp(0.0, 1.0)).to_hex()
    }
    /// Diverging scale: -1 away .. 0 near-black .. +1 home.
    pub fn div(&self, v: f64) -> String {
        let mid = Rgb::hex("#16121a").unwrap();
        let v = v.clamp(-1.0, 1.0);
        if v >= 0.0 {
            mid.mix(Rgb::hex(&self.home.primary).unwrap(), v).to_hex()
        } else {
            mid.mix(Rgb::hex(&self.away.primary).unwrap(), -v).to_hex()
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Overrides {
    #[serde(default)]
    pub teams: HashMap<String, TeamOverride>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct TeamOverride {
    pub primary: String,
    pub secondary: Option<String>,
    pub short_name: Option<String>,
    pub crest: Option<String>,
}

pub fn parse_overrides(s: &str) -> Result<Overrides, toml::de::Error> {
    toml::from_str(s)
}

pub fn load_overrides(path: Option<&Path>) -> Overrides {
    path.and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
}

fn find_override<'a>(o: &'a Overrides, t: &Team) -> Option<&'a TeamOverride> {
    o.teams.get(&t.source_id).or_else(|| o.teams.iter().find(|(k, _)| k.eq_ignore_ascii_case(&t.name)).map(|(_, v)| v))
}

const SEMANTIC: [(&str, &str, &str); 11] = [
    ("goal", "#f5c518", "#f8fafc"),
    ("saved", "#2cc4b4", "#a3e635"),
    ("woodwork", "#9b6bd6", "#f472b6"),
    ("off_target", "#f08a3c", "#fb7185"),
    ("hs_left", "#3b82f6", "#818cf8"),
    ("hs_right", "#14b8a6", "#4ade80"),
    ("cutback", "#facc15", "#e879f9"),
    ("through", "#f97316", "#f472b6"),
    ("touch", "#38bdf8", "#a78bfa"),
    ("defending", "#ec4899", "#fb923c"),
    ("blocked", "#b8b6c2", "#9ca3af"),
];

const FALLBACK_SET: [&str; 4] = ["#fde100", "#22d3ee", "#f5f5f5", "#84cc16"];

/// Ensure contrast >= 3:1 against the background by raising OKLCH lightness.
fn normalise(c: Rgb) -> Rgb {
    let bg = Rgb::hex(BG).unwrap();
    let (mut l, mut ch, h) = c.oklch();
    let mut out = c;
    let mut guard = 0;
    while out.contrast(bg) < 3.0 && guard < 40 {
        l = (l + 0.03).min(0.98);
        ch = ch.min(0.25);
        out = Rgb::from_oklch(l, ch, h);
        guard += 1;
    }
    out
}

fn crest_colours(path: &Path) -> Vec<Rgb> {
    let Ok(img) = image::open(path) else { return vec![] };
    let img = img.to_rgba8();
    let mut px: Vec<Rgb> = vec![];
    for (i, p) in img.pixels().enumerate() {
        if i % 3 != 0 || p[3] < 200 {
            continue;
        }
        let c = Rgb(p[0] as f64 / 255.0, p[1] as f64 / 255.0, p[2] as f64 / 255.0);
        let (l, ch, _) = c.oklch();
        if ch < 0.05 || l < 0.2 || l > 0.95 {
            continue;
        }
        px.push(c);
    }
    if px.len() < 20 {
        return vec![];
    }
    // k-means k=3 in OKLab
    let pts: Vec<(f64, f64, f64)> = px.iter().map(|c| c.oklab()).collect();
    let mut cent: Vec<(f64, f64, f64)> = (0..3).map(|k| pts[k * pts.len() / 3]).collect();
    let mut assign = vec![0usize; pts.len()];
    for _ in 0..12 {
        for (i, p) in pts.iter().enumerate() {
            assign[i] = (0..3).min_by(|&a, &b| dist3(*p, cent[a]).partial_cmp(&dist3(*p, cent[b])).unwrap()).unwrap();
        }
        for k in 0..3 {
            let mem: Vec<&(f64, f64, f64)> = pts.iter().zip(&assign).filter(|(_, a)| **a == k).map(|(p, _)| p).collect();
            if !mem.is_empty() {
                let n = mem.len() as f64;
                cent[k] = (mem.iter().map(|p| p.0).sum::<f64>() / n, mem.iter().map(|p| p.1).sum::<f64>() / n, mem.iter().map(|p| p.2).sum::<f64>() / n);
            }
        }
    }
    let mut counts: Vec<(usize, usize)> = (0..3).map(|k| (k, assign.iter().filter(|&&a| a == k).count())).collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    counts.iter().map(|(k, _)| Rgb::from_oklab(cent[*k].0, cent[*k].1, cent[*k].2)).collect()
}
fn dist3(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)
}

fn candidates(t: &Team, ov: Option<&TeamOverride>, crest: Option<&Path>) -> Vec<(Rgb, String)> {
    let mut v = vec![];
    if let Some(o) = ov {
        if let Some(c) = Rgb::hex(&o.primary) {
            v.push((c, "override:primary".into()));
        }
        if let Some(c) = o.secondary.as_deref().and_then(Rgb::hex) {
            v.push((c, "override:secondary".into()));
        }
    }
    for (src, h) in &t.colour_hints {
        if let Some(c) = Rgb::hex(h) {
            v.push((c, src.clone()));
        }
    }
    if let Some(p) = crest {
        for c in crest_colours(p) {
            v.push((c, "crest".into()));
        }
    }
    v
}

pub fn resolve(m: &Match, overrides: &Overrides, crests: [Option<&Path>; 2]) -> MatchTheme {
    let mut decisions = vec![];
    let bg = Rgb::hex(BG).unwrap();
    let mut picked: Vec<(Rgb, String, Vec<(Rgb, String)>)> = vec![];
    for (i, s) in Side::both().iter().enumerate() {
        let t = m.team(*s);
        let ov = find_override(overrides, t);
        let mut cands = candidates(t, ov, crests[i]);
        let default = if i == 0 { Rgb::hex("#e3242b").unwrap() } else { Rgb::hex("#fde100").unwrap() };
        cands.push((default, "default".into()));
        // prefer a chromatic candidate; grey/black/white is the last resort
        let chosen = cands.iter().find(|(c, _)| c.oklch().1 >= 0.04).cloned().unwrap_or_else(|| cands[0].clone());
        if chosen.1 == "default" {
            decisions.push(format!("{}: no colour source available, default palette", t.name));
        } else if chosen.1 != cands[0].1 {
            decisions.push(format!("{}: {} rejected (achromatic), using {}", t.name, cands[0].1, chosen.1));
        }
        let n = normalise(chosen.0);
        if n != chosen.0 {
            decisions.push(format!("{}: lightened {} -> {} for 3:1 contrast on dark background", t.name, chosen.0.to_hex(), n.to_hex()));
        }
        picked.push((n, chosen.1, cands));
    }
    // Clash handling: home keeps primary, away moves.
    let home = picked[0].0;
    let mut away = picked[1].0;
    let mut away_src = picked[1].1.clone();
    let de = delta_e2000(home, away);
    if de < 25.0 {
        let mut options: Vec<(Rgb, String)> = picked[1].2.iter().filter(|(c, s)| s != &picked[1].1 && c.oklch().1 >= 0.04).cloned().collect();
        for f in FALLBACK_SET {
            options.push((Rgb::hex(f).unwrap(), "fallback-set".into()));
        }
        if let Some((c, s)) = options.into_iter().map(|(c, s)| (normalise(c), s)).find(|(c, _)| delta_e2000(home, *c) >= 25.0 && c.contrast(bg) >= 3.0) {
            decisions.push(format!("away switched to {} ({}): ΔE {:.1} vs home", c.to_hex(), s, de));
            away = c;
            away_src = s;
        }
    }
    // Semantic collisions.
    let mut sem: HashMap<&str, String> = HashMap::new();
    for (k, base, alt) in SEMANTIC {
        let b = Rgb::hex(base).unwrap();
        let clash = delta_e2000(b, home).min(delta_e2000(b, away));
        if clash < 20.0 {
            decisions.push(format!("semantic '{k}' {base} -> {alt} (ΔE {:.1} to a team colour)", clash));
            sem.insert(k, alt.to_string());
        } else {
            sem.insert(k, base.to_string());
        }
    }
    let tt = |c: Rgb, src: String| TeamTheme {
        primary: c.to_hex(),
        muted: c.mix(Rgb::hex(BG).unwrap(), 0.55).to_hex(),
        on_primary: if c.contrast(Rgb(0.0, 0.0, 0.0)) >= c.contrast(Rgb(1.0, 1.0, 1.0)) { "#000000".into() } else { "#ffffff".into() },
        source: src,
    };
    MatchTheme {
        bg: BG.into(),
        home: tt(home, picked[0].1.clone()),
        away: tt(away, away_src),
        semantic: Semantic {
            goal: sem["goal"].clone(),
            saved: sem["saved"].clone(),
            woodwork: sem["woodwork"].clone(),
            off_target: sem["off_target"].clone(),
            blocked: sem["blocked"].clone(),
            hs_left: sem["hs_left"].clone(),
            hs_right: sem["hs_right"].clone(),
            z14: "#ffffff".into(),
            cutback: sem["cutback"].clone(),
            through: sem["through"].clone(),
            touch: sem["touch"].clone(),
            defending: sem["defending"].clone(),
        },
        decisions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn de2000_reference() {
        // Sharma et al. test pair 1 is in Lab; check sanity on sRGB extremes instead.
        let a = Rgb::hex("#ff0000").unwrap();
        let b = Rgb::hex("#ff0000").unwrap();
        assert!(delta_e2000(a, b) < 1e-6);
        let w = Rgb::hex("#ffffff").unwrap();
        let k = Rgb::hex("#000000").unwrap();
        assert!((delta_e2000(w, k) - 100.0).abs() < 1.0);
        assert!(delta_e2000(Rgb::hex("#c8102e").unwrap(), Rgb::hex("#da291c").unwrap()) < 25.0);
    }
    #[test]
    fn contrast_normalisation() {
        let dark_blue = Rgb::hex("#001489").unwrap();
        let n = normalise(dark_blue);
        assert!(n.contrast(Rgb::hex(BG).unwrap()) >= 3.0);
    }
}
