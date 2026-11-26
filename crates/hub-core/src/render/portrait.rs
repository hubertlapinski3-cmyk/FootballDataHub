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
const PANEL_FILL: &str = "#12211A";
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

/// The footer block (how-to-read lines) starts at `Scene::footer_start`; the viewer shows that text in its Info sheet.
fn is_footer(sc: &Scene, i: usize) -> bool {
    sc.footer_start > 0 && i >= sc.footer_start
}

// ---------------------------------------------------------------- XY-cut

thread_local! {
    /// container unit -> every primitive that belongs to it (its panel, pitch or plot area and the marks inside)
    static GROUPS: std::cell::RefCell<std::collections::HashMap<usize, Vec<usize>>> = std::cell::RefCell::new(Default::default());
}

/// Expands container units to their members. Other indices stand for themselves.
fn expand(items: &[usize]) -> Vec<usize> {
    GROUPS.with(|g| {
        let g = g.borrow();
        items.iter().flat_map(|i| g.get(i).cloned().unwrap_or_else(|| vec![*i])).collect()
    })
}

/// Builds the atomic units XY-cut may move: every panel (large rectangle) with everything anchored
/// inside it becomes one unit, so a chart's frame can never be cut away from its marks.
fn group_units(body: &[usize], sc: &Scene) -> Vec<(usize, Box4)> {
    let anchor_of = |p: &Prim| -> Option<(f32, f32)> {
        match p {
            Prim::R { x, y, .. } | Prim::C { x, y, .. } | Prim::T { x, y, .. } | Prim::I { x, y, .. } => Some((*x, *y)),
            Prim::L { p, .. } | Prim::P { p, .. } => p.first().map(|q| (q[0], q[1])),
        }
    };
    let rect = |p: &Prim| -> Option<Box4> {
        if let Prim::R { x, y, w, h, .. } = p {
            if *w > 200.0 && *h > 60.0 {
                return Some((*x, *y, x + w, y + h));
            }
        }
        None
    };
    let containers: Vec<(usize, Box4)> = body.iter().filter_map(|&i| rect(&sc.prims[i]).map(|r| (i, r))).collect();
    let inside = |r: Box4, p: (f32, f32)| p.0 >= r.0 && p.0 <= r.2 && p.1 >= r.1 && p.1 <= r.3;
    // parent = the smallest container that holds this primitive's anchor (never itself)
    let mut parent: std::collections::HashMap<usize, usize> = Default::default();
    for &j in body {
        let Some(a) = anchor_of(&sc.prims[j]) else { continue };
        let best = containers
            .iter()
            .filter(|(c, r)| *c != j && inside(*r, a) && rect(&sc.prims[j]).map(|rj| rj != *r).unwrap_or(true))
            .min_by(|x, y| ((x.1 .2 - x.1 .0) * (x.1 .3 - x.1 .1)).partial_cmp(&((y.1 .2 - y.1 .0) * (y.1 .3 - y.1 .1))).unwrap());
        if let Some((c, _)) = best {
            parent.insert(j, *c);
        }
    }
    fn flat(c: usize, parent: &std::collections::HashMap<usize, usize>, body: &[usize]) -> Vec<usize> {
        let mut out = vec![c];
        for &j in body {
            if j != c && parent.get(&j) == Some(&c) {
                out.extend(flat(j, parent, body));
            }
        }
        out
    }
    let mut units = vec![];
    let mut groups: std::collections::HashMap<usize, Vec<usize>> = Default::default();
    for &j in body {
        if parent.contains_key(&j) {
            continue;
        }
        if rect(&sc.prims[j]).is_some() {
            let members = flat(j, &parent, body);
            let b = members.iter().map(|&m| bbox1(&sc.prims[m])).reduce(union).unwrap();
            groups.insert(j, members);
            units.push((j, b));
        } else {
            units.push((j, bbox1(&sc.prims[j])));
        }
    }
    GROUPS.with(|g| *g.borrow_mut() = groups);
    units
}

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
        let (mut span, mut rest): (Vec<_>, Vec<_>) = items.iter().cloned().partition(|x| (x.1 .2 - x.1 .0) > 0.55 * total);
        // a container (a panel rect behind its rows) overlaps the rest vertically: it is not a spanner
        if let Some(rb) = rest.iter().map(|x| x.1).reduce(union) {
            let (keep, back): (Vec<_>, Vec<_>) = span.into_iter().partition(|x| x.1 .3 <= rb.1 + 4.0 || x.1 .1 >= rb.3 - 4.0);
            span = keep;
            rest.extend(back);
        }
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
        // paint order is the board's own: cut() sorts by position, which would put cells over arrows
        let mut idx: Vec<usize> = items.iter().map(|x| x.0).collect();
        idx.sort_unstable();
        return Node::Leaf { items: idx, b };
    }
    cut(items, !rows, stuck + 1)
}

/// Splits items into a left and a right column when a clear vertical gap separates two groups that
/// each hold several items. Returns None for single-column boards.
fn column_split(items: &[(usize, Box4)]) -> Option<(Vec<(usize, Box4)>, Vec<(usize, Box4)>)> {
    if items.len() < 6 {
        return None;
    }
    let mut v = items.to_vec();
    v.sort_by(|a, b| (a.1 .0 + a.1 .2).partial_cmp(&(b.1 .0 + b.1 .2)).unwrap());
    // widest empty x-gap between consecutive item centres that is wide in board px
    let mut best: Option<(f32, usize)> = None;
    let mut run_max = f32::MIN;
    for k in 0..v.len() - 1 {
        run_max = run_max.max(v[k].1 .2);
        let gap = v[k + 1].1 .0 - run_max;
        if gap > 120.0 && best.map(|b| gap > b.0).unwrap_or(true) {
            best = Some((gap, k + 1));
        }
    }
    let (_, cut_at) = best?;
    let (left, right): (Vec<_>, Vec<_>) = v.into_iter().enumerate().partition(|(i, _)| *i < cut_at);
    let left: Vec<_> = left.into_iter().map(|(_, x)| x).collect();
    let right: Vec<_> = right.into_iter().map(|(_, x)| x).collect();
    // both columns must be substantial, and sit side by side rather than one above the other
    let span = |g: &[(usize, Box4)]| g.iter().map(|x| x.1).reduce(union).unwrap();
    let (a, b) = (span(&left), span(&right));
    let overlap_y = a.3.min(b.3) - a.1.max(b.1);
    if left.len() >= 2 && right.len() >= 2 && overlap_y > 0.3 * (a.3 - a.1).min(b.3 - b.1) {
        Some((left, right))
    } else {
        None
    }
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
    let items = &expand(items)[..];
    // a soft card behind sizeable blocks that do not already carry their own panel
    let (w, h) = ((b.2 - b.0) * sx, (b.3 - b.1) * sy);
    if w > 160.0 && h > 90.0 && !has_panel(items, sc) {
        out.prims.push(Prim::R { x: x - 10.0, y: y - 10.0, w: w + 20.0, h: h + 20.0, r: 16.0, f: "#ffffff".into(), s: "none".into(), sw: 0.0, o: 0.045, t: -1, l: -1 });
    }
    for &i in items {
        let mut q = boost(xf(&sc.prims[i], b.0, b.1, sx, sy, x, y), TEXT_BOOST);
        // chart text is capped: a label scaled up past 24 px dominates a phone screen and collides with its neighbours
        if let Prim::T { sz, .. } = &mut q {
            *sz = sz.min(24.0);
        }
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
            // uniform scale: a flat strip stays flat, so circles stay round and charts keep their proportions
            let sy = sx;
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

/// Phone column limits for text: 16 px gutters, as the app's own gutter.
const TEXT_LO: f32 = 16.0;
const TEXT_HI: f32 = 624.0;
/// Smallest text the phone layout accepts (the audit test uses the same bound).
const TEXT_MIN: f32 = 11.5;

fn text_extent(runs: &[(String, String)], sz: f32, hd: bool) -> f32 {
    runs.iter().map(|r| text_w(&r.0, sz as f64, hd) as f32).sum()
}

/// Greedy word wrap that keeps each word's colour. Lines never exceed `max_w` unless a single word does.
fn wrap_runs(runs: &[(String, String)], sz: f32, hd: bool, max_w: f32) -> Vec<Vec<(String, String)>> {
    let mut lines: Vec<Vec<(String, String)>> = vec![];
    let mut cur: Vec<(String, String)> = vec![];
    let mut cur_txt = String::new();
    for (txt, col) in runs {
        for word in txt.split_whitespace() {
            let cand = if cur_txt.is_empty() { word.to_string() } else { format!("{cur_txt} {word}") };
            if !cur_txt.is_empty() && text_w(&cand, sz as f64, hd) as f32 > max_w {
                lines.push(std::mem::take(&mut cur));
                cur_txt.clear();
                cur.push((word.to_string(), col.clone()));
                cur_txt.push_str(word);
            } else {
                let lead = if cur.is_empty() { "" } else { " " };
                cur.push((format!("{lead}{word}"), col.clone()));
                cur_txt = cand;
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Moves every primitive below `y0` down by `dh` (the whole lower board moves together, so nothing is
/// squeezed or overlapped when a text block grows).
fn shift_below(prims: &mut [Prim], y0: f32, dh: f32) {
    for p in prims.iter_mut() {
        match p {
            Prim::R { y, .. } | Prim::C { y, .. } | Prim::T { y, .. } | Prim::I { y, .. } => {
                if *y > y0 + 0.5 {
                    *y += dh;
                }
            }
            Prim::L { p, .. } | Prim::P { p, .. } => {
                if p.iter().all(|q| q[1] > y0 + 0.5) {
                    for q in p.iter_mut() {
                        q[1] += dh;
                    }
                }
            }
        }
    }
}

/// Keeps every text run inside the phone column. A run that is too wide is first shrunk (down to
/// TEXT_MIN); if it still does not fit it is wrapped onto more lines and the board below moves down.
/// Returns the total height added and the (y, dh) shifts, so legend hit areas can follow.
fn fit_texts(prims: &mut Vec<Prim>) -> (f32, Vec<(f32, f32)>) {
    let mut total = 0.0f32;
    let mut shifts: Vec<(f32, f32)> = vec![];
    let mut order: Vec<usize> = (0..prims.len()).filter(|&i| matches!(prims[i], Prim::T { .. })).collect();
    let ty = |p: &Prim| if let Prim::T { y, .. } = p { *y } else { 0.0 };
    order.sort_by(|&a, &b| ty(&prims[a]).total_cmp(&ty(&prims[b])));
    for i in order {
        let Prim::T { x, y, runs, sz, w, hd, a, o, l } = prims[i].clone() else { continue };
        // a position that is not a number cannot be placed: drop the run rather than panic
        if !x.is_finite() || !y.is_finite() || !sz.is_finite() {
            if let Prim::T { runs, .. } = &mut prims[i] {
                runs.clear();
            }
            continue;
        }
        let tw = text_extent(&runs, sz, hd);
        // the anchor decides how much room the text has around its x
        let room = |x: f32| match a {
            0 => TEXT_HI - x,
            1 => 2.0 * (x - TEXT_LO).min(TEXT_HI - x),
            _ => x - TEXT_LO,
        };
        let x = match a {
            0 => x.clamp(TEXT_LO, (TEXT_HI - tw).max(TEXT_LO)),
            1 => x.clamp(TEXT_LO + tw / 2.0, (TEXT_HI - tw / 2.0).max(TEXT_LO + tw / 2.0)),
            _ => x.clamp((TEXT_LO + tw).min(TEXT_HI), TEXT_HI),
        };
        let avail = room(x).max(40.0);
        if tw <= avail {
            prims[i] = Prim::T { x, y, runs, sz, w, hd, a, o, l };
            continue;
        }
        let shrink = avail / tw * 0.995;
        if sz * shrink >= TEXT_MIN {
            prims[i] = Prim::T { x, y, runs, sz: sz * shrink, w, hd, a, o, l };
            continue;
        }
        // wrap: the first line stays in place, further lines go below and the board moves down
        let lines = wrap_runs(&runs, sz, hd, avail);
        let lh = sz * 1.35;
        let dh = (lines.len().saturating_sub(1)) as f32 * lh;
        if dh > 0.0 {
            shift_below(prims, y, dh);
            shifts.push((y, dh));
            total += dh;
        }
        let mut first = true;
        for (k, line) in lines.into_iter().enumerate() {
            let p = Prim::T { x, y: y + k as f32 * lh, runs: line, sz, w, hd, a, o, l };
            if first {
                prims[i] = p;
                first = false;
            } else {
                prims.push(p);
            }
        }
    }
    let (row_grow, row_shifts) = pack_rows(prims);
    total += row_grow;
    shifts.extend(row_shifts);
    let _dropped = decollide(prims);
    (total, shifts)
}

/// Labels that sit on one baseline and overlap sideways (legend rows, value rows) are packed left to
/// right with a gap; a row that runs past the column wraps its tail onto a new line below.
/// Returns the height added and the shifts applied, like `fit_texts`.
fn pack_rows(prims: &mut Vec<Prim>) -> (f32, Vec<(f32, f32)>) {
    const GAP: f32 = 8.0;
    let mut grow = 0.0f32;
    let mut shifts: Vec<(f32, f32)> = vec![];
    let ty = |p: &Prim| if let Prim::T { y, .. } = p { *y } else { 0.0 };
    let mut idx: Vec<usize> = (0..prims.len()).filter(|&i| glyph_box(&prims[i]).is_some()).collect();
    idx.sort_by(|&a, &b| ty(&prims[a]).total_cmp(&ty(&prims[b])));
    // clusters of texts whose baselines are within 4 px
    let mut clusters: Vec<Vec<usize>> = vec![];
    for &i in &idx {
        let y = ty(&prims[i]);
        match clusters.last_mut() {
            Some(c) if (ty(&prims[c[0]]) - y).abs() <= 4.0 => c.push(i),
            _ => clusters.push(vec![i]),
        }
    }
    for c in clusters {
        if c.len() < 2 {
            continue;
        }
        let boxes: Vec<(usize, (f32, f32, f32, f32, f32))> = c.iter().map(|&i| (i, glyph_box(&prims[i]).unwrap())).collect();
        let clash = boxes.iter().enumerate().any(|(k, a)| boxes.iter().skip(k + 1).any(|b| a.1 .2.min(b.1 .2) - a.1 .0.max(b.1 .0) > 2.0));
        if !clash {
            continue;
        }
        // left edge and width of every run in the row, from its current anchor
        let mut items: Vec<(usize, f32, f32, Vec<(String, String)>, f32, bool)> = vec![];
        for &i in &c {
            let Prim::T { x, runs, sz, hd, a, .. } = prims[i].clone() else { continue };
            let w = text_extent(&runs, sz, hd);
            let left = match a {
                0 => x,
                1 => x - w / 2.0,
                _ => x - w,
            };
            items.push((i, left, w, runs, sz, hd));
        }
        items.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut cur_y = ty(&prims[c[0]]);
        let mut cursor = TEXT_LO;
        let mut lh = 0.0f32;
        for (i, _left, w, runs, sz, hd) in items {
            if cursor + w > TEXT_HI && cursor > TEXT_LO {
                // wrap: the rest of the row goes on a new line below and the board below moves down
                let dh = (lh.max(sz) * 1.35).max(1.0);
                shift_below(prims, cur_y, dh);
                shifts.push((cur_y, dh));
                grow += dh;
                cur_y += dh;
                cursor = TEXT_LO;
                lh = 0.0;
            }
            // a run wider than the whole column is shrunk to fit it
            let new_sz = if cursor + w > TEXT_HI { sz * ((TEXT_HI - cursor) / w) * 0.995 } else { sz };
            prims[i] = Prim::T { x: cursor, y: cur_y, runs, sz: new_sz, w: 400, hd, a: 0, o: 1.0, l: -1 };
            cursor += w + GAP;
            lh = lh.max(sz);
        }
    }
    (grow, shifts)
}

/// Text box of a run at its current position (see the layout audit in tests/layout.rs).
fn glyph_box(p: &Prim) -> Option<(f32, f32, f32, f32, f32)> {
    let Prim::T { x, y, runs, sz, hd, a, .. } = p else { return None };
    if runs.is_empty() {
        return None;
    }
    let w = text_extent(runs, *sz, *hd);
    let (x0, x1) = match a {
        0 => (*x, x + w),
        1 => (x - w / 2.0, x + w / 2.0),
        _ => (x - w, *x),
    };
    // the same box the layout audit uses: one em above the baseline, 0.3 em below
    Some((x0, y - sz, x1, y + sz * 0.3, *sz))
}

/// Chart labels that land on each other: the smaller one is nudged up or down in small steps to a free
/// spot, and removed if none is free. Headings (26 px and up) are never moved or removed; chart text is capped at 24 px.
/// Returns how many labels were dropped.
fn decollide(prims: &mut Vec<Prim>) -> usize {
    let mut dropped = 0;
    let idx: Vec<usize> = (0..prims.len()).filter(|&i| glyph_box(&prims[i]).is_some()).collect();
    for k in 0..idx.len() {
        for m in k + 1..idx.len() {
            let (i, j) = (idx[k], idx[m]);
            let (Some(a), Some(b)) = (glyph_box(&prims[i]), glyph_box(&prims[j])) else { continue };
            let overlap = |p: (f32, f32, f32, f32, f32), q: (f32, f32, f32, f32, f32)| p.2.min(q.2) - p.0.max(q.0) > 2.0 && p.3.min(q.3) - p.1.max(q.1) > 2.0;
            if !overlap(a, b) {
                continue;
            }
            // the smaller text moves; two headings are left alone (and reported by the audit)
            let (mv, other) = if a.4 < b.4 { (i, j) } else { (j, i) };

            if prims_size(&prims[mv]) >= 26.0 {
                continue;
            }
            let base = prims[mv].clone();
            let mut placed = false;
            for dy in [4.0f32, -4.0, 8.0, -8.0, 12.0, -12.0, 16.0, -16.0, 20.0, -20.0] {
                let mut cand = base.clone();
                if let Prim::T { y, .. } = &mut cand {
                    *y += dy;
                }
                let cb = glyph_box(&cand).unwrap();
                let clear = idx.iter().filter(|&&q| q != mv).all(|&q| match glyph_box(&prims[q]) {
                    Some(qb) => !(cb.2.min(qb.2) - cb.0.max(qb.0) > 2.0 && cb.3.min(qb.3) - cb.1.max(qb.1) > 2.0),
                    None => true,
                });
                if clear {
                    prims[mv] = cand;
                    placed = true;
                    break;
                }
            }
            if !placed {
                if let Prim::T { runs, .. } = &mut prims[mv] {
                    runs.clear();
                    dropped += 1;
                }
            }
            let _ = other;
        }
    }
    prims.retain(|p| !matches!(p, Prim::T { runs, .. } if runs.is_empty()));
    dropped
}

fn prims_size(p: &Prim) -> f32 {
    if let Prim::T { sz, .. } = p { *sz } else { 0.0 }
}

/// Recompose a landscape scene into a single phone-width column.
/// Returns `None` for scenes that are already portrait.
pub fn reflow(sc: &Scene) -> Option<Scene> {
    if sc.w < 1500.0 {
        return None;
    }
    let mut head_frame: Vec<&Prim> = vec![];
    let mut head_left: Vec<&Prim> = vec![];
    let mut body: Vec<usize> = vec![];
    for (i, p) in sc.prims.iter().enumerate() {
        if is_footer(sc, i) {
            continue;
        }
        let (ax, ay) = anchor(p);
        if ay < 142.0 {
            // only the real header frame (top of the board) and the team-colour rule are header chrome;
            // any other rectangle is a panel and belongs with its contents in the body
            let header_frame = matches!(p, Prim::R { h, .. } if *h > 100.0) && ay < 40.0;
            let rule = matches!(p, Prim::R { h, .. } if *h <= 6.0) && ay >= 130.0;
            if header_frame || rule {
                head_frame.push(p);
            } else if matches!(p, Prim::R { .. }) {
                body.push(i);
            } else if ax < 1040.0 {
                head_left.push(p);
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
    let mut y = 18.0;
    let informative = headline.as_ref().map(|r| {
        let txt: String = r.iter().map(|x| x.0.as_str()).collect();
        txt.split_whitespace().count() >= 5 || txt.chars().any(|c| c.is_ascii_digit())
    }).unwrap_or(false);
    if let (true, Some(runs)) = (informative, &headline) {
        let size = 30.0f32;
        y += size;
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
            y += size * 1.3;
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
        y += size * 1.3;
    }
    // The viewer's own top bar carries the board title and controls, so the poster chrome is gone:
    // no header box, no crests / score cluster. A thin team-colour divider closes the intro.
    y += 6.0;
    let head_h = y;
    let half = (PW - 40.0) / 2.0;
    for p in &head_frame {
        if let Prim::R { x, h, f, s, sw, o, t, l, .. } = p {
            if *h <= 30.0 {
                let left_half = *x < 400.0;
                prims.push(Prim::R { x: if left_half { 20.0 } else { 20.0 + half }, y: head_h, w: half, h: 3.0, r: 0.0, f: f.clone(), s: s.clone(), sw: *sw, o: *o, t: *t, l: *l });
            }
        }
    }

    // ---- body
    let items: Vec<(usize, Box4)> = group_units(&body, sc);
    // two side-by-side charts: keep each column whole, so rows of one chart are never interleaved with the other
    // full-width notes and banners are kept out of the column search: their width would hide the gap
    let (wide, narrow): (Vec<(usize, Box4)>, Vec<(usize, Box4)>) = items.iter().cloned().partition(|x| x.1 .2 - x.1 .0 > 700.0);
    let mid_y = items.iter().map(|x| (x.1 .1 + x.1 .3) / 2.0).sum::<f32>() / items.len().max(1) as f32;
    let (wide_top, wide_bottom): (Vec<_>, Vec<_>) = wide.into_iter().partition(|x| (x.1 .1 + x.1 .3) / 2.0 < mid_y);
    let mut parts: Vec<Node> = vec![];
    if !wide_top.is_empty() {
        parts.push(cut(&wide_top, true, 0));
    }
    if !narrow.is_empty() {
        parts.push(match column_split(&narrow) {
            Some((l, r)) => Node::Cols(vec![cut(&l, true, 0), cut(&r, true, 0)]),
            None => cut(&narrow, true, 0),
        });
    }
    if !wide_bottom.is_empty() {
        parts.push(cut(&wide_bottom, true, 0));
    }
    let root = if parts.len() == 1 { parts.pop().unwrap() } else { Node::Rows(parts) };
    let mut placed = Placed { prims: vec![], maps: vec![] };
    let body_h = place(&root, sc, head_h + 30.0, &mut placed);
    prims.extend(placed.prims);

    // ---- keep all text inside the column (may wrap and move the board below it down)
    let (grow, shifts) = fit_texts(&mut prims);

    // ---- legend hit areas follow their block, and any text shift above them
    let mut legend = vec![];
    for h in &sc.legend {
        let (cx, cy) = (h.x + h.w / 2.0, h.y + h.h / 2.0);
        if let Some((_, ox, oy, sx, sy, tx, ty)) = placed.maps.iter().find(|(b, ..)| cx >= b.0 - 2.0 && cx <= b.2 + 2.0 && cy >= b.1 - 2.0 && cy <= b.3 + 2.0) {
            let mut y = (h.y - oy) * sy + ty;
            for (y0, dh) in &shifts {
                if y > *y0 + 0.5 {
                    y += dh;
                }
            }
            legend.push(LegendHit { layer: h.layer, x: (h.x - ox) * sx + tx, y, w: h.w * sx, h: h.h * sy });
        }
    }
    Some(Scene { w: PW, h: head_h + 30.0 + body_h + 36.0 + grow, prims, tips: sc.tips.clone(), layers: sc.layers.clone(), legend, footer_start: 0 })
}
