//! Pitch geometry: maps canonical team-perspective metres to screen space.

use super::svg::Svg;
use crate::theme;

#[derive(Clone, Copy)]
pub enum Orient {
    /// attack up the page
    Up,
    /// attack to the right
    Right,
}

#[derive(Clone, Copy)]
pub struct Pitch {
    pub x0: f64,
    pub y0: f64,
    pub w: f64,
    pub h: f64,
    pub orient: Orient,
    /// visible length range (metres along attack axis)
    pub lmin: f64,
    pub lmax: f64,
}

impl Pitch {
    /// Vertical pitch fitted into a box, keeping 68:len aspect.
    pub fn up(bx: f64, by: f64, bw: f64, bh: f64, lmin: f64) -> Pitch {
        let len = 105.0 - lmin;
        let s = (bw / 68.0).min(bh / len);
        let (w, h) = (68.0 * s, len * s);
        Pitch { x0: bx + (bw - w) / 2.0, y0: by + (bh - h) / 2.0, w, h, orient: Orient::Up, lmin, lmax: 105.0 }
    }
    pub fn right(bx: f64, by: f64, bw: f64, bh: f64) -> Pitch {
        let s = (bw / 105.0).min(bh / 68.0);
        let (w, h) = (105.0 * s, 68.0 * s);
        Pitch { x0: bx + (bw - w) / 2.0, y0: by + (bh - h) / 2.0, w, h, orient: Orient::Right, lmin: 0.0, lmax: 105.0 }
    }
    pub fn scale(&self) -> f64 {
        match self.orient {
            Orient::Up => self.w / 68.0,
            Orient::Right => self.w / 105.0,
        }
    }
    /// x along attack (0..105), y 0 = right touchline .. 68 = left touchline.
    pub fn p(&self, x: f64, y: f64) -> (f64, f64) {
        match self.orient {
            Orient::Up => (self.x0 + (68.0 - y) / 68.0 * self.w, self.y0 + (self.lmax - x) / (self.lmax - self.lmin) * self.h),
            Orient::Right => (self.x0 + x / 105.0 * self.w, self.y0 + (68.0 - y) / 68.0 * self.h),
        }
    }
    pub fn len_px(&self, m: f64) -> f64 {
        m * self.scale()
    }
    fn seg(&self, s: &mut Svg, pts: &[(f64, f64)], col: &str, sw: f64) {
        let clipped: Vec<(f64, f64)> = pts.iter().filter(|(x, _)| *x >= self.lmin - 0.01).map(|&(x, y)| self.p(x, y)).collect();
        if clipped.len() >= 2 {
            s.polyline(&clipped, col, sw, r#"stroke-linejoin="round" opacity="0.75""#);
        }
    }
    pub fn draw(&self, s: &mut Svg) {
        self.draw_with(s, theme::PITCH_LINE, 1.6);
    }
    pub fn draw_with(&self, s: &mut Svg, col: &str, sw: f64) {
        let l = self.lmin;
        self.seg(s, &[(l, 0.0), (105.0, 0.0), (105.0, 68.0), (l, 68.0), (l, 0.0)], col, sw);
        if l <= 52.5 {
            self.seg(s, &[(52.5, 0.0), (52.5, 68.0)], col, sw);
            let c: Vec<(f64, f64)> = (0..=72).map(|i| {
                let a = i as f64 / 72.0 * std::f64::consts::TAU;
                (52.5 + 9.15 * a.cos(), 34.0 + 9.15 * a.sin())
            }).filter(|(x, _)| *x >= l).collect();
            self.seg(s, &c, col, sw);
            let (cx, cy) = self.p(52.5, 34.0);
            s.circle(cx, cy, 2.2, col, "none", 0.0, "");
        }
        for end in [105.0, 0.0] {
            if end == 0.0 && l > 16.5 {
                continue;
            }
            let dir = if end == 105.0 { -1.0 } else { 1.0 };
            self.seg(s, &[(end, 13.84), (end + dir * 16.5, 13.84), (end + dir * 16.5, 54.16), (end, 54.16)], col, sw);
            self.seg(s, &[(end, 24.84), (end + dir * 5.5, 24.84), (end + dir * 5.5, 43.16), (end, 43.16)], col, sw);
            let spot = end + dir * 11.0;
            if spot >= l {
                let (sx, sy) = self.p(spot, 34.0);
                s.circle(sx, sy, 2.0, col, "none", 0.0, "");
            }
            let arc: Vec<(f64, f64)> = (0..=40).map(|i| {
                let a = -0.93 + 1.86 * i as f64 / 40.0;
                (spot - dir * 9.15 * a.cos() * -1.0, 34.0 + 9.15 * a.sin())
            }).filter(|(x, _)| if dir < 0.0 { *x <= 88.5 } else { *x >= 16.5 }).collect();
            self.seg(s, &arc, col, sw);
            self.seg(s, &[(end, 30.34), (end - dir * 1.6, 30.34), (end - dir * 1.6, 37.66), (end, 37.66)], col, sw);
        }
    }
    pub fn label_attack(&self, s: &mut Svg) {
        match self.orient {
            Orient::Up => s.t(self.x0 + self.w / 2.0, self.y0 - 12.0, "ATTACK", 13.0, 700, theme::TEXT, super::svg::Anchor::Middle),
            Orient::Right => s.t(self.x0 + self.w / 2.0, self.y0 - 12.0, "ATTACK →", 13.0, 700, theme::TEXT, super::svg::Anchor::Middle),
        }
    }
    /// Shade a rectangle in pitch metres.
    pub fn zone(&self, s: &mut Svg, x0: f64, x1: f64, y0: f64, y1: f64, fill: &str, op: f64) {
        let (ax, ay) = self.p(x0.max(self.lmin), y0);
        let (bx, by) = self.p(x1, y1);
        s.rect(ax.min(bx), ay.min(by), (bx - ax).abs(), (by - ay).abs(), fill, &format!(r#"opacity="{op}""#));
    }
}
