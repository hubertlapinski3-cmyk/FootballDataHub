//! Portrait reflow for phone screens.
//!
//! Boards are composed on a 1600 px landscape canvas. `reflow` recomposes the recorded
//! vector scene into one phone-width column: a compact stacked header, then the body cut into
//! blocks (XY-cut: whitespace gaps split the body into bands and columns). Blocks that fit side by
//! side stay in a row; wide blocks are stacked and scaled to the full width. Only the display list
//! changes; marks keep their tooltips and legend layers.

use super::svg::{text_w, LegendHit, Prim, Scene};

const PW: f32 = 640.0;
const PAD: f32 = 20.0;
const TEXT_BOOST: f32 = 1.3;
/// Smallest type on a portrait board, in board px (about 11 dp on a phone).
const MIN_TEXT: f32 = 14.0;
const PANEL_FILL: &str = "#0f0c15";
const GAP: f32 = 22.0;

type Box4 = (f32, f32, f32, f32);

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

fn bbox1(p: &Prim) -> Box4 {
    match p {
        Prim::R { x, y, w, h, .. } | Prim::I { x, y, w, h, .. } => (*x, *y, x + w, y + h),
        Prim::C { x, y, r, .. } => (x - r, y - r, x + r, y + r),
        Prim::L { p, .. } | Prim::P { p, .. } => {
            let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for q in p {
                b = (b.0.min(q[0]), b.1.min(q[1]), b.2.max(q[0]), b.3.max(q[1]));
            }
            b
        }
        Prim::T { x, y, runs, sz, hd, a, .. } => {
            let txt: String = runs.iter().map(|r| r.0.as_str()).collect();
            // estimate runs slightly wide so a block never clips its own text
            let w = text_w(&txt, *sz as f64, *hd) as f32 * 1.12;
            let (x0, x1) = match a {
                0 => (*x, x + w),
                1 => (x - w / 2.0, x + w / 2.0),
                _ => (x - w, *x),
            };
            (x0, y - sz, x1, y + sz * 0.3)
        }
    }
}

fn union(a: Box4, b: Box4) -> Box4 {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

fn bbox(ps: &[&Prim]) -> Option<Box4> {
    ps.iter().map(|p| bbox1(p)).reduce(union)
}

/// Affine map p' = ((p - o) * s) + t with independent x / y scale; strokes and type use the x scale.
fn xf(p: &Prim, ox: f32, oy: f32, sx: f32, sy: f32, tx: f32, ty: f32) -> Prim {
    let fx = |x: f32| (x - ox) * sx + tx;
    let fy = |y: f32| (y - oy) * sy + ty;
    let mut q = p.clone();
    match &mut q {
        Prim::R { x, y, w, h, r, sw, .. } => {
            *x = fx(*x);
            *y = fy(*y);
            *w *= sx;
            *h *= sy;
            *r *= sx;
            *sw *= sx;
        }
        Prim::C { x, y, r, sw, .. } => {
            *x = fx(*x);
            *y = fy(*y);
            *r *= sx;
            *sw *= sx;
        }
        Prim::L { p, sw, d, .. } => {
            for q in p.iter_mut() {
                q[0] = fx(q[0]);
                q[1] = fy(q[1]);
            }
            *sw *= sx;
            if let Some(d) = d {
                d[0] *= sx;
                d[1] *= sx;
            }
        }
        Prim::P { p, sw, .. } => {
            for q in p.iter_mut() {
                q[0] = fx(q[0]);
                q[1] = fy(q[1]);
            }
            *sw *= sx;
        }
        Prim::T { x, y, sz, .. } => {
            *x = fx(*x);
            *y = fy(*y);
            *sz *= sx;
        }
        Prim::I { x, y, w, h, .. } => {
            *x = fx(*x);
            *y = fy(*y);
            *w *= sx;
            *h *= sx.min(sy);
        }
    }
    q
}

fn boost(mut p: Prim, k: f32) -> Prim {
    if let Prim::T { sz, .. } = &mut p {
        *sz = (*sz * k).max(MIN_TEXT);
    }
    p
}

fn is_footer(p: &Prim) -> bool {
    match p {
        Prim::T { runs, sz, .. } => runs.iter().any(|r| (r.1 == "#cfcdd8" && *sz == 13.0) || (r.1 == crate::theme::GREY && *sz == 11.0)),
        _ => false,
    }
}

// ---------------------------------------------------------------- XY-cut

enum Node {
    Leaf { items: Vec<usize>, b: Box4 },
    /// children stacked top to bottom
    Rows(Vec<Node>),
    /// children left to right
    Cols(Vec<Node>),
}

fn node_box(n: &Node) -> Box4 {
    match n {
        Node::Leaf { b, .. } => *b,
        Node::Rows(c) | Node::Cols(c) => c.iter().map(node_box).reduce(union).unwrap_or((0.0, 0.0, 1.0, 1.0)),
    }
}

fn cut(items: &[(usize, Box4)], rows: bool, stuck: u8) -> Node {
    let min_gap = if rows { 16.0 } else { 14.0 };
    let key = |b: &Box4| if rows { (b.1, b.3) } else { (b.0, b.2) };
    let mut v: Vec<(usize, Box4)> = items.to_vec();
    v.sort_by(|a, b| key(&a.1).0.partial_cmp(&key(&b.1).0).unwrap());
    let mut groups: Vec<Vec<(usize, Box4)>> = vec![];
    let mut end = f32::MIN;
    for it in v {
        let (s, e) = key(&it.1);
        if groups.is_empty() || s > end + min_gap {
            groups.push(vec![it]);
            end = e;
        } else {
            groups.last_mut().unwrap().push(it);
            end = end.max(e);
        }
    }
    if groups.len() > 1 {
        let kids: Vec<Node> = groups.iter().map(|g| cut(g, !rows, 0)).collect();
        return if rows { Node::Rows(kids) } else { Node::Cols(kids) };
    }
    // one glued block: wide spanners (a legend under two pitches) may be what joins the columns
    if !rows && items.len() > 3 {
        let total = items.iter().map(|x| x.1).reduce(union).map(|b| b.2 - b.0).unwrap_or(0.0);
        let (span, rest): (Vec<_>, Vec<_>) = items.iter().cloned().partition(|x| (x.1 .2 - x.1 .0) > 0.55 * total);
        if !span.is_empty() && !rest.is_empty() {
            let mut gs: Vec<(f32, f32)> = vec![];
            let mut r2 = rest.clone();
            r2.sort_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap());
            let mut end = f32::MIN;
            for it in &r2 {
                if gs.is_empty() || it.1 .0 > end + min_gap {
                    gs.push((it.1 .0, it.1 .2));
                } else if let Some(l) = gs.last_mut() {
                    l.1 = l.1.max(it.1 .2);
                }
                end = end.max(it.1 .2);
            }
            if gs.len() > 1 {
                let span_top = span.iter().map(|x| x.1 .1).fold(f32::MAX, f32::min);
                let rest_top = rest.iter().map(|x| x.1 .1).fold(f32::MAX, f32::min);
                let a = cut(&rest, false, 0);
                let b = cut(&span, true, 1);
                return if span_top < rest_top { Node::Rows(vec![b, a]) } else { Node::Rows(vec![a, b]) };
            }
        }
    }
    if stuck >= 1 || items.len() <= 1 {
        let b = items.iter().map(|x| x.1).reduce(union).unwrap_or((0.0, 0.0, 1.0, 1.0));
        return Node::Leaf { items: items.iter().map(|x| x.0).collect(), b };
    }
    cut(items, !rows, stuck + 1)
}

struct Placed {
    prims: Vec<Prim>,
    /// (source box, ox, oy, sx, sy, tx, ty) per leaf, to move legend hit areas the same way
    maps: Vec<(Box4, f32, f32, f32, f32, f32, f32)>,
}

fn has_panel(items: &[usize], sc: &Scene) -> bool {
    items.iter().any(|&i| matches!(&sc.prims[i], Prim::R { f, w, .. } if f == PANEL_FILL && *w > 200.0))
}

fn nat(n: &Node) -> (f32, f32) {
    let b = node_box(n);
    (b.2 - b.0, b.3 - b.1)
}

fn leaf(items: &[usize], b: Box4, sc: &Scene, x: f32, y: f32, sx: f32, sy: f32, out: &mut Placed) {
    // a soft card behind sizeable blocks that do not already carry their own panel
    let (w, h) = ((b.2 - b.0) * sx, (b.3 - b.1) * sy);
    if w > 160.0 && h > 90.0 && !has_panel(items, sc) {
        out.prims.push(Prim::R { x: x - 10.0, y: y - 10.0, w: w + 20.0, h: h + 20.0, r: 16.0, f: "#ffffff".into(), s: "none".into(), sw: 0.0, o: 0.045, t: -1, l: -1 });
    }
    for &i in items {
        let mut q = boost(xf(&sc.prims[i], b.0, b.1, sx, sy, x, y), TEXT_BOOST);
        // data lines keep a legible weight after scaling
        if let Prim::L { sw, t, l, .. } = &mut q {
            if *t >= 0 || *l >= 0 {
                *sw = sw.max(2.2);
            }
        }
        out.prims.push(q);
    }
    out.maps.push((b, b.0, b.1, sx, sy, x, y));
}

fn place_scaled(n: &Node, sc: &Scene, x: f32, y: f32, s: f32, out: &mut Placed) -> (f32, f32) {
    match n {
        Node::Leaf { items, b } => {
            leaf(items, *b, sc, x, y, s, s, out);
            ((b.2 - b.0) * s, (b.3 - b.1) * s)
        }
        Node::Rows(c) => {
            let mut yy = y;
            let mut w: f32 = 0.0;
            for k in c {
                let (kw, kh) = place_scaled(k, sc, x, yy, s, out);
                yy += kh + GAP;
                w = w.max(kw);
            }
            (w, yy - y - GAP)
        }
        Node::Cols(c) => {
            let mut xx = x;
            let mut h: f32 = 0.0;
            for k in c {
                let (kw, kh) = place_scaled(k, sc, xx, y, s, out);
                xx += kw + GAP;
                h = h.max(kh);
            }
            (xx - x - GAP, h)
        }
    }
}

fn place(n: &Node, sc: &Scene, y: f32, out: &mut Placed) -> f32 {
    let avail = PW - 2.0 * PAD;
    match n {
        Node::Leaf { items, b } => {
            let (w, h) = (b.2 - b.0, b.3 - b.1);
            let cap = if w < 300.0 { 1.0 } else { 1.5 };
            let sx = (avail / w).min(cap);
            // wide, flat charts get extra height so they read on a tall screen
            let sy = if w / h > 1.8 { sx * (w / h / 1.2).min(1.9) } else { sx };
            leaf(items, *b, sc, (PW - w * sx) / 2.0, y, sx, sy, out);
            h * sy
        }
        Node::Rows(c) => {
            let mut yy = y;
            for k in c {
                yy += place(k, sc, yy, out) + GAP;
            }
            yy - y - GAP
        }
        Node::Cols(c) => {
            let (nw, _) = nat(n);
            // natural width includes the gaps between columns
            let total = nw;
            let s_row = avail / total;
            if s_row >= 0.8 {
                let s = s_row.min(1.5);
                let used = total * s;
                let (_, h) = place_scaled(n, sc, (PW - used) / 2.0, y, s, out);
                h
            } else {
                let mut yy = y;
                for k in c {
                    yy += place(k, sc, yy, out) + GAP;
                }
                yy - y - GAP
            }
        }
    }
}

/// Recompose a landscape scene into a single phone-width column.
/// Returns `None` for scenes that are already portrait.
pub fn reflow(sc: &Scene) -> Option<Scene> {
    if sc.w < 1500.0 {
        return None;
    }
    let mut head_frame: Vec<&Prim> = vec![];
    let mut head_left: Vec<&Prim> = vec![];
    let mut head_right: Vec<&Prim> = vec![];
    let mut body: Vec<usize> = vec![];
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
        } else {
            body.push(i);
        }
    }
    if body.is_empty() {
        return None;
    }

    // ---- header: wrapped headline and subtitle, then the teams / score row
    let mut prims: Vec<Prim> = vec![];
    let mut headline: Option<Vec<(String, String)>> = None;
    let mut subtitle: Option<(String, String)> = None;
    for p in &head_left {
        match p {
            Prim::T { y, runs, hd, .. } if *hd && *y > 70.0 && *y < 100.0 => headline = Some(runs.clone()),
            Prim::T { y, runs, .. } if *y > 105.0 => subtitle = runs.first().cloned(),
            _ => {}
        }
    }
    let mut y = 40.0;
    if let Some(runs) = &headline {
        let size = 34.0f32;
        let per_line = ((PW - 48.0) / (size * 0.47)) as usize;
        let mut words: Vec<(String, String)> = vec![];
        for (t, c) in runs {
            for w in t.split(' ').filter(|w| !w.is_empty()) {
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
        // one line only: the full method text lives in the Info sheet
        let mut line: String = txt.chars().take(per_line).collect();
        if txt.chars().count() > per_line {
            line = line.trim_end().trim_end_matches(|c: char| !c.is_alphanumeric()).to_string();
            line.push('…');
        }
        y += 2.0;
        prims.push(Prim::T { x: 24.0, y: y + size, runs: vec![(line, col.clone())], sz: size, w: 400, hd: false, a: 0, o: 1.0, l: -1 });
        y += size + 6.0;
    }
    // teams / score cluster, centred (right cluster spans about x 1070..1540 on the landscape canvas)
    let row_y = y + 34.0;
    let rb = bbox(&head_right);
    let cx = rb.map(|b| (b.0 + b.2) / 2.0).unwrap_or(1305.0);
    for p in &head_right {
        if matches!(p, Prim::T { runs, .. } if runs.iter().any(|r| r.0 == "FULL MATCH")) {
            continue; // the period selector already says it
        }
        let q = xf(p, 0.0, 0.0, 1.0, 1.0, PW / 2.0 - cx, row_y - 52.0);
        prims.push(match q {
            Prim::T { sz, .. } if sz < 12.0 => boost(q, 1.2),
            other => other,
        });
    }
    let head_h = row_y + 44.0;
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

    // ---- body
    let items: Vec<(usize, Box4)> = body.iter().map(|&i| (i, bbox1(&sc.prims[i]))).collect();
    let root = cut(&items, true, 0);
    let mut placed = Placed { prims: vec![], maps: vec![] };
    let body_h = place(&root, sc, head_h + 40.0, &mut placed);
    prims.extend(placed.prims);

    // ---- legend hit areas follow their block
    let mut legend = vec![];
    for h in &sc.legend {
        let (cx, cy) = (h.x + h.w / 2.0, h.y + h.h / 2.0);
        if let Some((_, ox, oy, sx, sy, tx, ty)) = placed.maps.iter().find(|(b, ..)| cx >= b.0 - 2.0 && cx <= b.2 + 2.0 && cy >= b.1 - 2.0 && cy <= b.3 + 2.0) {
            legend.push(LegendHit { layer: h.layer, x: (h.x - ox) * sx + tx, y: (h.y - oy) * sy + ty, w: h.w * sx, h: h.h * sy });
        }
    }
    Some(Scene { w: PW, h: head_h + 40.0 + body_h + 36.0, prims, tips: sc.tips.clone(), layers: sc.layers.clone(), legend })
}
