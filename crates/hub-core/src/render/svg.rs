//! Tiny SVG builder used by every board.

use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Vector display list recorded alongside the SVG so apps can draw boards
/// natively (crisp zoom, tooltips, legend toggles, motion).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "k")]
pub enum Prim {
    /// rect: x, y, w, h, corner radius
    R { x: f32, y: f32, w: f32, h: f32, r: f32, f: String, s: String, sw: f32, o: f32, t: i32, l: i32 },
    /// circle
    C { x: f32, y: f32, r: f32, f: String, s: String, sw: f32, o: f32, t: i32, l: i32 },
    /// open polyline / line, optional dash [on, off]
    L { p: Vec<[f32; 2]>, s: String, sw: f32, o: f32, d: Option<[f32; 2]>, t: i32, l: i32 },
    /// closed polygon
    P { p: Vec<[f32; 2]>, f: String, s: String, sw: f32, o: f32, t: i32, l: i32 },
    /// text: runs of (text, colour); a: 0 start 1 middle 2 end; hd: condensed headline face
    T { x: f32, y: f32, runs: Vec<(String, String)>, sz: f32, w: u16, hd: bool, a: u8, o: f32, l: i32 },
    /// image: href is a data URI or a key such as "crest:0"
    I { x: f32, y: f32, w: f32, h: f32, href: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendHit {
    pub layer: i32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Scene {
    pub w: f32,
    pub h: f32,
    pub prims: Vec<Prim>,
    pub tips: Vec<String>,
    pub layers: Vec<String>,
    pub legend: Vec<LegendHit>,
}

fn attr_f(extra: &str, key: &str) -> Option<f32> {
    let i = extra.find(&format!("{key}=\""))? + key.len() + 2;
    let j = extra[i..].find('"')? + i;
    extra[i..j].trim().parse().ok()
}
fn attr_dash(extra: &str) -> Option<[f32; 2]> {
    let i = extra.find("stroke-dasharray=\"")? + 18;
    let j = extra[i..].find('"')? + i;
    let v: Vec<f32> = extra[i..j].split([' ', ',']).filter_map(|x| x.parse().ok()).collect();
    match v.len() {
        0 => None,
        1 => Some([v[0], v[0]]),
        _ => Some([v[0], v[1]]),
    }
}

pub const HEAD: &str = "Barlow Condensed";
pub const BODY: &str = "Inter";

pub struct Svg {
    pub w: f64,
    pub h: f64,
    pub buf: String,
    defs: String,
    uid: usize,
    /// how-to-read sentence, captured by the footer for app overlays
    pub how: String,
    pub scene: Scene,
    /// tooltip for the next data mark (consumed by it)
    pub tip: Option<String>,
    /// layer applied to marks until cleared (legend entries with the same key toggle it)
    pub layer: Option<String>,
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn f(v: f64) -> String {
    format!("{:.1}", v)
}

#[derive(Clone, Copy)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

impl Svg {
    pub fn new(w: f64, h: f64) -> Svg {
        Svg { w, h, buf: String::with_capacity(200_000), defs: String::new(), uid: 0, how: String::new(), scene: Scene { w: w as f32, h: h as f32, ..Default::default() }, tip: None, layer: None }
    }
    fn take_tip(&mut self) -> i32 {
        match self.tip.take() {
            Some(t) => {
                self.scene.tips.push(t);
                self.scene.tips.len() as i32 - 1
            }
            None => -1,
        }
    }
    pub fn layer_idx(&mut self, name: &str) -> i32 {
        if let Some(i) = self.scene.layers.iter().position(|l| l == name) {
            return i as i32;
        }
        self.scene.layers.push(name.to_string());
        self.scene.layers.len() as i32 - 1
    }
    fn cur_layer(&mut self) -> i32 {
        match self.layer.clone() {
            Some(l) => self.layer_idx(&l),
            None => -1,
        }
    }
    pub fn legend_hit(&mut self, layer: &str, x: f64, y: f64, w: f64, h: f64) {
        let l = self.layer_idx(layer);
        self.scene.legend.push(LegendHit { layer: l, x: x as f32, y: y as f32, w: w as f32, h: h as f32 });
    }
    pub fn id(&mut self, p: &str) -> String {
        self.uid += 1;
        format!("{p}{}", self.uid)
    }
    pub fn def(&mut self, s: &str) {
        self.defs.push_str(s);
    }
    pub fn raw(&mut self, s: &str) {
        self.buf.push_str(s);
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: &str, extra: &str) {
        let (t, l) = (self.take_tip(), self.cur_layer());
        let st = extra.find("stroke=\"").map(|i| extra[i + 8..].split('"').next().unwrap_or("none").to_string()).unwrap_or_else(|| "none".into());
        self.scene.prims.push(Prim::R { x: x as f32, y: y as f32, w: w.max(0.0) as f32, h: h.max(0.0) as f32, r: 0.0, f: fill.into(), s: st, sw: attr_f(extra, "stroke-width").unwrap_or(0.0), o: attr_f(extra, "opacity").unwrap_or(1.0), t, l });
        let _ = write!(self.buf, r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" {}/>"#, f(x), f(y), f(w.max(0.0)), f(h.max(0.0)), fill, extra);
    }
    pub fn rrect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64, fill: &str, stroke: &str, sw: f64) {
        let (t, l) = (self.take_tip(), self.cur_layer());
        self.scene.prims.push(Prim::R { x: x as f32, y: y as f32, w: w.max(0.0) as f32, h: h.max(0.0) as f32, r: r as f32, f: fill.into(), s: stroke.into(), sw: sw as f32, o: 1.0, t, l });
        let _ = write!(self.buf, r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#, f(x), f(y), f(w.max(0.0)), f(h.max(0.0)), f(r), fill, stroke, sw);
    }
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str, sw: f64, extra: &str) {
        let (t, l) = (self.take_tip(), self.cur_layer());
        self.scene.prims.push(Prim::L { p: vec![[x1 as f32, y1 as f32], [x2 as f32, y2 as f32]], s: stroke.into(), sw: sw as f32, o: attr_f(extra, "opacity").unwrap_or(1.0), d: attr_dash(extra), t, l });
        let _ = write!(self.buf, r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" {}/>"#, f(x1), f(y1), f(x2), f(y2), stroke, sw, extra);
    }
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64, extra: &str) {
        let (t, l) = (self.take_tip(), self.cur_layer());
        self.scene.prims.push(Prim::C { x: cx as f32, y: cy as f32, r: r.max(0.0) as f32, f: fill.into(), s: stroke.into(), sw: sw as f32, o: attr_f(extra, "opacity").unwrap_or(1.0), t, l });
        let _ = write!(self.buf, r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="{}" {}/>"#, f(cx), f(cy), f(r.max(0.0)), fill, stroke, sw, extra);
    }
    pub fn path(&mut self, d: &str, fill: &str, stroke: &str, sw: f64, extra: &str) {
        // only M/L/Z paths are produced by the boards
        let nums: Vec<f32> = d.split(|c: char| c.is_alphabetic() || c == ',' || c.is_whitespace()).filter_map(|x| x.parse().ok()).collect();
        let pts: Vec<[f32; 2]> = nums.chunks(2).filter(|c| c.len() == 2).map(|c| [c[0], c[1]]).collect();
        let (t, l) = (self.take_tip(), self.cur_layer());
        self.scene.prims.push(Prim::P { p: pts, f: fill.into(), s: stroke.into(), sw: sw as f32, o: attr_f(extra, "opacity").unwrap_or(1.0), t, l });
        let _ = write!(self.buf, r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" {}/>"#, d, fill, stroke, sw, extra);
    }
    pub fn poly(&mut self, pts: &[(f64, f64)], fill: &str, stroke: &str, sw: f64, extra: &str) {
        let (t, l) = (self.take_tip(), self.cur_layer());
        self.scene.prims.push(Prim::P { p: pts.iter().map(|(x, y)| [*x as f32, *y as f32]).collect(), f: fill.into(), s: stroke.into(), sw: sw as f32, o: attr_f(extra, "opacity").unwrap_or(1.0), t, l });
        let p: Vec<String> = pts.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect();
        let _ = write!(self.buf, r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{}" {}/>"#, p.join(" "), fill, stroke, sw, extra);
    }
    pub fn polyline(&mut self, pts: &[(f64, f64)], stroke: &str, sw: f64, extra: &str) {
        let (t, l) = (self.take_tip(), self.cur_layer());
        self.scene.prims.push(Prim::L { p: pts.iter().map(|(x, y)| [*x as f32, *y as f32]).collect(), s: stroke.into(), sw: sw as f32, o: attr_f(extra, "opacity").unwrap_or(1.0), d: attr_dash(extra), t, l });
        let p: Vec<String> = pts.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect();
        let _ = write!(self.buf, r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{}" {}/>"#, p.join(" "), stroke, sw, extra);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn text(&mut self, x: f64, y: f64, s: &str, size: f64, weight: u32, fill: &str, anchor: Anchor, family: &str) {
        let a = match anchor {
            Anchor::Start => "start",
            Anchor::Middle => "middle",
            Anchor::End => "end",
        };
        let l = self.cur_layer();
        self.scene.prims.push(Prim::T { x: x as f32, y: y as f32, runs: vec![(s.to_string(), fill.to_string())], sz: size as f32, w: weight as u16, hd: family == HEAD, a: anchor as u8, o: 1.0, l });
        let _ = write!(self.buf, r#"<text x="{}" y="{}" font-family="{}" font-size="{}" font-weight="{}" fill="{}" text-anchor="{}">{}</text>"#, f(x), f(y), family, size, weight, fill, a, esc(s));
    }
    /// HighlightText-style rich text: coloured runs inside one line.
    #[allow(clippy::too_many_arguments)]
    pub fn text_runs(&mut self, x: f64, y: f64, runs: &[(String, String)], size: f64, weight: u32, anchor: Anchor, family: &str) {
        let a = match anchor {
            Anchor::Start => "start",
            Anchor::Middle => "middle",
            Anchor::End => "end",
        };
        let l = self.cur_layer();
        self.scene.prims.push(Prim::T { x: x as f32, y: y as f32, runs: runs.to_vec(), sz: size as f32, w: weight as u16, hd: family == HEAD, a: anchor as u8, o: 1.0, l });
        let spans: String = runs.iter().map(|(t, c)| format!(r#"<tspan fill="{}">{}</tspan>"#, c, esc(t))).collect();
        let _ = write!(self.buf, r#"<text x="{}" y="{}" font-family="{}" font-size="{}" font-weight="{}" text-anchor="{}">{}</text>"#, f(x), f(y), family, size, weight, a, spans);
    }
    /// Curved annotation arrow (quadratic bezier), drawarrow-style.
    pub fn curved_arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, bend: f64, stroke: &str, sw: f64) {
        let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
        let (dx, dy) = (x2 - x1, y2 - y1);
        let (cx, cy) = (mx - dy * bend, my + dx * bend);
        let pts: Vec<(f64, f64)> = (0..=24).map(|i| {
            let t = i as f64 / 24.0;
            ((1.0 - t).powi(2) * x1 + 2.0 * (1.0 - t) * t * cx + t * t * x2, (1.0 - t).powi(2) * y1 + 2.0 * (1.0 - t) * t * cy + t * t * y2)
        }).collect();
        self.polyline(&pts, stroke, sw, r#"stroke-linecap="round""#);
        let (px, py) = pts[22];
        let ang = (y2 - py).atan2(x2 - px);
        let head = 9.0 + sw * 2.0;
        let (a1, a2) = (ang + 2.6, ang - 2.6);
        self.poly(&[(x2, y2), (x2 + head * a1.cos(), y2 + head * a1.sin()), (x2 + head * a2.cos(), y2 + head * a2.sin())], stroke, "none", 0.0, "");
    }
    pub fn t(&mut self, x: f64, y: f64, s: &str, size: f64, weight: u32, fill: &str, anchor: Anchor) {
        self.text(x, y, s, size, weight, fill, anchor, BODY);
    }
    pub fn h(&mut self, x: f64, y: f64, s: &str, size: f64, fill: &str, anchor: Anchor) {
        self.text(x, y, s, size, 700, fill, anchor, HEAD);
    }
    pub fn arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str, sw: f64, head: f64, extra: &str) {
        self.line(x1, y1, x2, y2, stroke, sw, extra);
        let ang = (y2 - y1).atan2(x2 - x1);
        let (a1, a2) = (ang + 2.6, ang - 2.6);
        self.poly(&[(x2, y2), (x2 + head * a1.cos(), y2 + head * a1.sin()), (x2 + head * a2.cos(), y2 + head * a2.sin())], stroke, "none", 0.0, extra);
    }
    pub fn star(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str) {
        let mut pts = vec![];
        for i in 0..10 {
            let rr = if i % 2 == 0 { r } else { r * 0.45 };
            let a = -std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::PI / 5.0;
            pts.push((cx + rr * a.cos(), cy + rr * a.sin()));
        }
        self.poly(&pts, fill, stroke, 1.0, "");
    }
    pub fn diamond(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64) {
        self.poly(&[(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)], fill, stroke, sw, "");
    }
    pub fn triangle(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64) {
        self.poly(&[(cx, cy - r), (cx + r * 0.95, cy + r * 0.7), (cx - r * 0.95, cy + r * 0.7)], fill, stroke, sw, "");
    }
    pub fn square(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64) {
        self.rect(cx - r, cy - r, 2.0 * r, 2.0 * r, fill, &format!(r#"stroke="{stroke}" stroke-width="{sw}""#));
    }
    pub fn cross(&mut self, cx: f64, cy: f64, r: f64, stroke: &str, sw: f64) {
        self.line(cx - r, cy - r, cx + r, cy + r, stroke, sw, r#"stroke-linecap="round""#);
        self.line(cx - r, cy + r, cx + r, cy - r, stroke, sw, r#"stroke-linecap="round""#);
    }
    pub fn image(&mut self, x: f64, y: f64, w: f64, h: f64, href: &str) {
        self.image_keyed(x, y, w, h, href, None);
    }
    /// Image whose scene entry uses a short key (e.g. "crest:0") instead of the data URI.
    pub fn image_keyed(&mut self, x: f64, y: f64, w: f64, h: f64, href: &str, key: Option<&str>) {
        self.scene.prims.push(Prim::I { x: x as f32, y: y as f32, w: w as f32, h: h as f32, href: key.map(|k| k.to_string()).unwrap_or_else(|| href.to_string()) });
        let _ = write!(self.buf, r#"<image x="{}" y="{}" width="{}" height="{}" href="{}" preserveAspectRatio="xMidYMid meet"/>"#, f(x), f(y), f(w), f(h), href);
    }
    pub fn finish(self, bg: &str) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><defs><radialGradient id="bgg" cx="0.75" cy="0.85" r="0.9"><stop offset="0" stop-color="{bg2}"/><stop offset="1" stop-color="{bg}"/></radialGradient>{defs}</defs><rect width="{w}" height="{h}" fill="url(#bgg)"/>{body}</svg>"#,
            w = self.w,
            h = self.h,
            bg = bg,
            bg2 = crate::theme::BG2,
            defs = self.defs,
            body = self.buf
        )
    }
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity(data.len() * 4 / 3 + 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        o.push(T[(n >> 18) as usize & 63] as char);
        o.push(T[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    o
}

/// Rough text width estimate (px) for layout decisions.
pub fn text_w(s: &str, size: f64, condensed: bool) -> f64 {
    s.chars().count() as f64 * size * if condensed { 0.47 } else { 0.56 }
}

pub fn fmt_n(v: f64, d: usize) -> String {
    format!("{:.*}", d, v)
}
pub fn pct(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}
pub fn signed(v: f64, d: usize) -> String {
    if v >= 0.0 {
        format!("+{:.*}", d, v)
    } else {
        format!("−{:.*}", d, -v)
    }
}
