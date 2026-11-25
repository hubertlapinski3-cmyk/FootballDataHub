//! Portrait reflow for phone screens.
//!
//! Boards with a pitch on the left and a stats panel on the right are laid out
//! on a 1600 px landscape canvas. On a portrait phone that canvas shows the pitch at
//! under half the screen width. `reflow` recomposes the recorded vector scene into one
//! column: compact header, the pitch scaled to the full width, then the stats panel
//! scaled to the full width below it. Only the display list changes; marks keep their
//! tooltips and legend layers.

use super::svg::{text_w, Prim, Scene};

const PW: f32 = 640.0;
/// Text is boosted on top of the geometric scale: board type was sized for a 1600 px canvas.
const TEXT_BOOST: f32 = 1.3;
const PANEL_FILL: &str = "#0f0c15";

fn anchor(p: &Prim) -> (f32, f32) {
    match p {
        Prim::R { x, y, .. } | Prim::C { x, y, .. } | Prim::T { x, y, .. } | Prim::I { x, y, .. } => (*x, *y),
        Prim::L { p, .. } | Prim::P { p, .. } => {
            let x = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min);
            let y = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min);
            (x, y)
        }
    }
}

fn bbox(ps: &[&Prim]) -> Option<(f32, f32, f32, f32)> {
    let mut b: Option<(f32, f32, f32, f32)> = None;
    let mut add = |x0: f32, y0: f32, x1: f32, y1: f32| {
        b = Some(match b {
            None => (x0, y0, x1, y1),
            Some((a, bb, c, d)) => (a.min(x0), bb.min(y0), c.max(x1), d.max(y1)),
        });
    };
    for p in ps {
        match p {
            Prim::R { x, y, w, h, .. } | Prim::I { x, y, w, h, .. } => add(*x, *y, x + w, y + h),
            Prim::C { x, y, r, .. } => add(x - r, y - r, x + r, y + r),
            Prim::L { p, .. } | Prim::P { p, .. } => {
                for q in p {
                    add(q[0], q[1], q[0], q[1]);
                }
            }
            Prim::T { x, y, runs, sz, hd, a, .. } => {
                let txt: String = runs.iter().map(|r| r.0.as_str()).collect();
                let w = text_w(&txt, *sz as f64, *hd) as f32;
                let (x0, x1) = match a {
                    0 => (*x, x + w),
                    1 => (x - w / 2.0, x + w / 2.0),
                    _ => (x - w, *x),
                };
                add(x0, y - sz, x1, y + sz * 0.3);
            }
        }
    }
    b
}

/// Uniform scale + translate: p' = (p - o) * s + t.
fn xf(p: &Prim, ox: f32, oy: f32, s: f32, tx: f32, ty: f32) -> Prim {
    let fx = |x: f32| (x - ox) * s + tx;
    let fy = |y: f32| (y - oy) * s + ty;
    let mut q = p.clone();
    match &mut q {
        Prim::R { x, y, w, h, r, sw, .. } => {
            *x = fx(*x);
            *y = fy(*y);
            *w *= s;
            *h *= s;
            *r *= s;
            *sw *= s;
        }
        Prim::C { x, y, r, sw, .. } => {
            *x = fx(*x);
            *y = fy(*y);
            *r *= s;
            *sw *= s;
        }
        Prim::L { p, sw, d, .. } => {
            for q in p.iter_mut() {
                q[0] = fx(q[0]);
                q[1] = fy(q[1]);
            }
            *sw *= s;
            if let Some(d) = d {
                d[0] *= s;
                d[1] *= s;
            }
        }
        Prim::P { p, sw, .. } => {
            for q in p.iter_mut() {
                q[0] = fx(q[0]);
                q[1] = fy(q[1]);
            }
            *sw *= s;
        }
        Prim::T { x, y, sz, .. } => {
            *x = fx(*x);
            *y = fy(*y);
            *sz *= s;
        }
        Prim::I { x, y, w, h, .. } => {
            *x = fx(*x);
            *y = fy(*y);
            *w *= s;
            *h *= s;
        }
    }
    q
}

fn is_footer(p: &Prim) -> bool {
    match p {
        Prim::T { runs, sz, .. } => runs.iter().any(|r| (r.1 == "#cfcdd8" && *sz == 13.0) || (r.1 == crate::theme::GREY && *sz == 11.0)),
        _ => false,
    }
}

/// Recompose a landscape pitch-and-panel scene into a single phone-width column.
/// Returns `None` for boards that do not have a right-hand stats panel.
pub fn reflow(sc: &Scene) -> Option<Scene> {
    if sc.w < 1500.0 {
        return None;
    }
    // split column: left edge of the first stats panel on the right
    let split = sc.prims.iter().filter_map(|p| match p {
        Prim::R { x, y, w, f, .. } if f == PANEL_FILL && *x >= 700.0 && *w >= 220.0 && *y >= 150.0 => Some(*x),
        _ => None,
    }).fold(f32::MAX, f32::min);
    if split == f32::MAX {
        return None;
    }
    let split = split - 12.0;

    let mut head_frame: Vec<&Prim> = vec![];
    let mut head_left: Vec<&Prim> = vec![];
    let mut head_right: Vec<&Prim> = vec![];
    let mut left: Vec<(usize, &Prim)> = vec![];
    let mut right: Vec<(usize, &Prim)> = vec![];
    for (i, p) in sc.prims.iter().enumerate() {
        if is_footer(p) {
            continue;
        }
        let (ax, ay) = anchor(p);
        if ay < 142.0 {
            if matches!(p, Prim::R { w, .. } if *w > 600.0 || (*w > 100.0 && ay > 130.0)) {
                head_frame.push(p);
            } else if ax < 1040.0 {
                head_left.push(p);
            } else {
                head_right.push(p);
            }
        } else if ax >= split {
            right.push((i, p));
        } else {
            left.push((i, p));
        }
    }
    let lp: Vec<&Prim> = left.iter().map(|x| x.1).collect();
    let rp: Vec<&Prim> = right.iter().map(|x| x.1).collect();
    let (lx0, ly0, lx1, ly1) = bbox(&lp)?;
    let (rx0, ry0, rx1, ry1) = bbox(&rp)?;

    let mut prims: Vec<Prim> = vec![];
    // ---- header: logo row, wrapped headline, wrapped subtitle, then teams / score row
    let mut headline: Option<(Vec<(String, String)>, f32)> = None;
    let mut subtitle: Option<(String, String)> = None;
    let mut head_prims: Vec<Prim> = vec![];
    for p in &head_left {
        match p {
            Prim::T { x, y, runs, sz, hd, .. } if *x >= 130.0 && *hd && *y > 70.0 && *y < 100.0 => {
                let _ = sz;
                headline = Some((runs.clone(), *sz));
            }
            Prim::T { x, y, runs, .. } if *x >= 130.0 && *y > 105.0 => {
                subtitle = runs.first().cloned();
            }
            Prim::T { sz, .. } if *sz > 12.0 && *sz < 14.0 => head_prims.push(xf(p, 0.0, 0.0, 1.0, 0.0, 0.0).boost(1.25)),
            _ => head_prims.push((*p).clone()),
        }
    }
    prims.extend(head_prims);
    let mut y = 150.0;
    if let Some((runs, _)) = &headline {
        let size = 34.0f32;
        let per_line = ((PW - 48.0) / (size * 0.47)) as usize;
        // flatten into words keeping their colours
        let mut words: Vec<(String, String)> = vec![];
        for (t, c) in runs {
            for (i, w) in t.split(' ').enumerate() {
                if w.is_empty() {
                    continue;
                }
                let _ = i;
                words.push((w.to_string(), c.clone()));
            }
        }
        let mut lines: Vec<Vec<(String, String)>> = vec![vec![]];
        let mut len = 0usize;
        for (w, c) in words {
            if len + w.chars().count() + 1 > per_line && len > 0 {
                lines.push(vec![]);
                len = 0;
            }
            let last = lines.last_mut().unwrap();
            let txt = if last.is_empty() { w.clone() } else { format!(" {w}") };
            len += txt.chars().count();
            last.push((txt, c));
        }
        for l in lines {
            prims.push(Prim::T { x: 24.0, y, runs: l, sz: size, w: 700, hd: true, a: 0, o: 1.0, l: -1 });
            y += size + 6.0;
        }
    }
    if let Some((txt, col)) = &subtitle {
        let size = 15.0f32;
        let per_line = ((PW - 48.0) / (size * 0.56)) as usize;
        let mut cur = String::new();
        let mut lines = vec![];
        for w in txt.split_whitespace() {
            if cur.chars().count() + w.chars().count() + 1 > per_line && !cur.is_empty() {
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
        y += 4.0;
        for l in lines {
            prims.push(Prim::T { x: 24.0, y: y + size, runs: vec![(l, col.clone())], sz: size, w: 400, hd: false, a: 0, o: 1.0, l: -1 });
            y += size + 6.0;
        }
    }
    // teams / score cluster, centred
    let row_y = y + 34.0;
    for p in &head_right {
        let q = xf(p, 0.0, 0.0, 1.0, -985.0, row_y - 52.0);
        prims.push(match q {
            Prim::T { sz, .. } if sz < 12.0 => q.boost(1.2),
            other => other,
        });
    }
    let head_h = row_y + 60.0;
    for p in &head_frame {
        if let Prim::R { x, h, f, s, sw, o, t, l, r, .. } = p {
            if *h > 30.0 {
                prims.insert(0, Prim::R { x: 12.0, y: 12.0, w: PW - 24.0, h: head_h - 12.0, r: *r, f: f.clone(), s: s.clone(), sw: *sw, o: *o, t: *t, l: *l });
            } else {
                let half = (PW - 24.0) / 2.0;
                let left_half = *x < 400.0;
                prims.push(Prim::R { x: if left_half { 12.0 } else { 12.0 + half }, y: head_h + 2.0, w: half, h: *h, r: 0.0, f: f.clone(), s: s.clone(), sw: *sw, o: *o, t: *t, l: *l });
            }
        }
    }

    // ---- pitch column, scaled to the full width
    let sl = ((PW - 40.0) / (lx1 - lx0)).min(1.1);
    let body_top = head_h + 40.0;
    let lcx = (lx0 + lx1) / 2.0;
    let mut y_bottom = body_top;
    for (_, p) in &left {
        prims.push(xf(p, lcx, ly0, sl, PW / 2.0, body_top).boost(1.2));
    }
    y_bottom = y_bottom.max(body_top + (ly1 - ly0) * sl);

    // ---- stats column below
    let sr = ((PW - 40.0) / (rx1 - rx0)).min(1.2);
    let rcx = (rx0 + rx1) / 2.0;
    let rtop = y_bottom + 28.0;
    for (_, p) in &right {
        prims.push(xf(p, rcx, ry0, sr, PW / 2.0, rtop).boost(TEXT_BOOST));
    }
    let total_h = rtop + (ry1 - ry0) * sr + 36.0;

    // ---- re-map the hit data: tooltip / layer indices live on the prims, legend hits need the same transform
    let mut legend = vec![];
    for h in &sc.legend {
        let (hx, hy) = (h.x, h.y);
        let (s_, ox, oy, tx, ty) = if hy < 142.0 {
            continue;
        } else if hx >= split {
            (sr, rcx, ry0, PW / 2.0, rtop)
        } else {
            (sl, lcx, ly0, PW / 2.0, body_top)
        };
        legend.push(super::svg::LegendHit { layer: h.layer, x: (hx - ox) * s_ + tx, y: (hy - oy) * s_ + ty, w: h.w * s_, h: h.h * s_ });
    }
    Some(Scene { w: PW, h: total_h, prims, tips: sc.tips.clone(), layers: sc.layers.clone(), legend })
}

trait Boost {
    fn boost(self, k: f32) -> Prim;
}
impl Boost for Prim {
    /// Enlarge text only; geometry stays put.
    fn boost(mut self, k: f32) -> Prim {
        if let Prim::T { sz, .. } = &mut self {
            *sz *= k;
        }
        self
    }
}
