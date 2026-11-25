//! Tiny SVG builder used by every board.

use std::fmt::Write;

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
        Svg { w, h, buf: String::with_capacity(200_000), defs: String::new(), uid: 0, how: String::new() }
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
        let _ = write!(self.buf, r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" {}/>"#, f(x), f(y), f(w.max(0.0)), f(h.max(0.0)), fill, extra);
    }
    pub fn rrect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64, fill: &str, stroke: &str, sw: f64) {
        let _ = write!(self.buf, r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#, f(x), f(y), f(w.max(0.0)), f(h.max(0.0)), f(r), fill, stroke, sw);
    }
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str, sw: f64, extra: &str) {
        let _ = write!(self.buf, r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" {}/>"#, f(x1), f(y1), f(x2), f(y2), stroke, sw, extra);
    }
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64, extra: &str) {
        let _ = write!(self.buf, r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="{}" {}/>"#, f(cx), f(cy), f(r.max(0.0)), fill, stroke, sw, extra);
    }
    pub fn path(&mut self, d: &str, fill: &str, stroke: &str, sw: f64, extra: &str) {
        let _ = write!(self.buf, r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" {}/>"#, d, fill, stroke, sw, extra);
    }
    pub fn poly(&mut self, pts: &[(f64, f64)], fill: &str, stroke: &str, sw: f64, extra: &str) {
        let p: Vec<String> = pts.iter().map(|(x, y)| format!("{},{}", f(*x), f(*y))).collect();
        let _ = write!(self.buf, r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{}" {}/>"#, p.join(" "), fill, stroke, sw, extra);
    }
    pub fn polyline(&mut self, pts: &[(f64, f64)], stroke: &str, sw: f64, extra: &str) {
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
        let _ = write!(self.buf, r#"<text x="{}" y="{}" font-family="{}" font-size="{}" font-weight="{}" fill="{}" text-anchor="{}">{}</text>"#, f(x), f(y), family, size, weight, fill, a, esc(s));
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
