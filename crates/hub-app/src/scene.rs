//! Native, interactive board renderer.
//!
//! Boards arrive from hub-core as a vector display list (`<board>.scene.json`).
//! They are drawn with the app's own painter and fonts: crisp at any zoom,
//! with tap/hover tooltips, tap-to-toggle legend layers and an entrance motion.

use crate::style::{self, *};
use egui::{epaint::Mesh, text::LayoutJob, Color32, Pos2, Rect, Shape, Stroke, TextureId, Ui, Vec2};
use matchhub::render::svg::{Prim, Scene};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default)]
pub struct SceneCache {
    map: HashMap<PathBuf, Option<Arc<Scene>>>,
}

impl SceneCache {
    pub fn get(&mut self, path: &Path) -> Option<Arc<Scene>> {
        self.map
            .entry(path.to_path_buf())
            .or_insert_with(|| std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Scene>(&s).ok()).map(Arc::new))
            .clone()
    }
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

pub struct PaintOpts<'a> {
    /// seconds since the board appeared (None = no motion)
    pub anim: Option<f32>,
    pub hidden: Option<&'a HashSet<i32>>,
    pub selected: Option<usize>,
    /// thumbnails skip tiny text and fine strokes
    pub thumb: bool,
    pub crests: [Option<TextureId>; 2],
}

fn col(s: &str, o: f32) -> Option<Color32> {
    if s == "none" || s.is_empty() || s.starts_with("url(") {
        return None;
    }
    let c = style::hex(s);
    Some(Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (o.clamp(0.0, 1.0) * 255.0) as u8))
}

/// Back-out easing (overshoots slightly, like a broadcast graphic settling).
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    let s = 1.4;
    t * t * ((s + 1.0) * t + s) + 1.0
}

fn tri_area(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)
}

/// Ear-clipping triangulation for simple (possibly concave) polygons.
fn fill_polygon(pts: &[Pos2], fill: Color32) -> Shape {
    let n = pts.len();
    let mut mesh = Mesh::default();
    if n < 3 {
        return Shape::Noop;
    }
    for p in pts {
        mesh.colored_vertex(*p, fill);
    }
    let area: f32 = (0..n).map(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        a.x * b.y - b.x * a.y
    }).sum();
    let ccw = area > 0.0;
    let mut idx: Vec<usize> = (0..n).collect();
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            let cr = tri_area(a, b, c);
            if (cr > 0.0) != ccw || cr.abs() < 1e-6 {
                continue;
            }
            let inside = idx.iter().any(|&j| {
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let p = pts[j];
                let (d1, d2, d3) = (tri_area(a, b, p), tri_area(b, c, p), tri_area(c, a, p));
                let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                !(has_neg && has_pos)
            });
            if inside {
                continue;
            }
            mesh.add_triangle(ia as u32, ib as u32, ic as u32);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        mesh.add_triangle(idx[0] as u32, idx[1] as u32, idx[2] as u32);
    } else {
        // degenerate leftovers: fan
        for k in 1..idx.len().saturating_sub(1) {
            mesh.add_triangle(idx[0] as u32, idx[k] as u32, idx[k + 1] as u32);
        }
    }
    Shape::mesh(mesh)
}

fn prim_layer_tip(p: &Prim) -> (i32, i32) {
    match p {
        Prim::R { l, t, .. } | Prim::C { l, t, .. } | Prim::L { l, t, .. } | Prim::P { l, t, .. } => (*l, *t),
        Prim::T { l, .. } => (*l, -1),
        Prim::I { .. } => (-1, -1),
    }
}

/// Draw a scene into `rect` (screen space).
pub fn paint(ui: &Ui, rect: Rect, sc: &Scene, o: &PaintOpts) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let s = rect.width() / sc.w;
    let tp = |x: f32, y: f32| Pos2::new(rect.min.x + x * s, rect.min.y + y * s);
    // board background (layer two visual language)
    let mut bg = Mesh::default();
    bg.colored_vertex(rect.left_top(), BOARD_BG);
    bg.colored_vertex(rect.right_top(), BOARD_BG);
    bg.colored_vertex(rect.left_bottom(), BOARD_BG2);
    bg.colored_vertex(rect.right_bottom(), BOARD_BG2);
    bg.add_triangle(0, 1, 2);
    bg.add_triangle(1, 3, 2);
    painter.add(Shape::mesh(bg));

    // motion: data marks enter in order with a short stagger
    let n_marks = sc.prims.iter().filter(|p| { let (l, t) = prim_layer_tip(p); l >= 0 || t >= 0 }).count().max(1) as f32;
    let mut mark_i = 0f32;
    let visible = ui.clip_rect();
    for (pi, p) in sc.prims.iter().enumerate() {
        let (layer, tip) = prim_layer_tip(p);
        let is_mark = layer >= 0 || tip >= 0;
        let mut alpha = 1.0f32;
        let mut grow = 1.0f32;
        if is_mark {
            if let Some(t) = o.anim {
                let delay = 0.08 + mark_i / n_marks * 0.45;
                let e = ease((t - delay) / 0.32);
                alpha = e.clamp(0.0, 1.0);
                grow = 0.55 + 0.45 * e;
            }
            mark_i += 1.0;
            if let Some(h) = o.hidden {
                if layer >= 0 && h.contains(&layer) {
                    alpha *= 0.1;
                }
            }
        }
        if alpha <= 0.01 {
            continue;
        }
        match p {
            Prim::R { x, y, w, h, r, f, s: st, sw, o: op, .. } => {
                let rr = Rect::from_min_max(tp(*x, *y), tp(x + w, y + h));
                if !rr.intersects(visible) {
                    continue;
                }
                if let Some(c) = col(f, op * alpha) {
                    painter.rect_filled(rr, r * s, c);
                }
                if let Some(c) = col(st, op * alpha) {
                    if *sw > 0.0 {
                        painter.rect_stroke(rr, r * s, Stroke::new(sw * s, c), egui::StrokeKind::Middle);
                    }
                }
            }
            Prim::C { x, y, r, f, s: st, sw, o: op, .. } => {
                let c0 = tp(*x, *y);
                let rad = r * s * grow;
                if !visible.expand(rad).contains(c0) {
                    continue;
                }
                if let Some(c) = col(f, op * alpha) {
                    painter.circle_filled(c0, rad, c);
                }
                if let Some(c) = col(st, op * alpha) {
                    if *sw > 0.0 {
                        painter.circle_stroke(c0, rad, Stroke::new((sw * s).max(0.6), c));
                    }
                }
            }
            Prim::L { p: pts, s: st, sw, o: op, d, .. } => {
                let Some(c) = col(st, op * alpha) else { continue };
                if o.thumb && sw * s < 0.25 {
                    continue;
                }
                let v: Vec<Pos2> = pts.iter().map(|q| tp(q[0], q[1])).collect();
                let stroke = Stroke::new((sw * s).max(0.5), c);
                match d {
                    Some([on, off]) => {
                        for sh in Shape::dashed_line(&v, stroke, (on * s).max(1.0), (off * s).max(1.0)) {
                            painter.add(sh);
                        }
                    }
                    None => {
                        if v.len() == 2 {
                            painter.line_segment([v[0], v[1]], stroke);
                        } else {
                            painter.add(Shape::line(v, stroke));
                        }
                    }
                }
            }
            Prim::P { p: pts, f, s: st, sw, o: op, .. } => {
                let mut v: Vec<Pos2> = pts.iter().map(|q| tp(q[0], q[1])).collect();
                if grow < 0.999 && !v.is_empty() {
                    let cx = v.iter().map(|q| q.x).sum::<f32>() / v.len() as f32;
                    let cy = v.iter().map(|q| q.y).sum::<f32>() / v.len() as f32;
                    for q in v.iter_mut() {
                        *q = Pos2::new(cx + (q.x - cx) * grow, cy + (q.y - cy) * grow);
                    }
                }
                if let Some(c) = col(f, op * alpha) {
                    painter.add(fill_polygon(&v, c));
                }
                if let Some(c) = col(st, op * alpha) {
                    if *sw > 0.0 {
                        let mut closed = v.clone();
                        closed.push(v[0]);
                        painter.add(Shape::line(closed, Stroke::new((sw * s).max(0.5), c)));
                    }
                }
            }
            Prim::T { x, y, runs, sz, w, hd, a, o: op, .. } => {
                let size = sz * s;
                if size < if o.thumb { 4.5 } else { 2.5 } {
                    continue;
                }
                let font = if *hd { head(size) } else if *w >= 700 { bold(size) } else if *w >= 600 { semi(size) } else { body(size) };
                let mut job = LayoutJob::default();
                for (txt, c) in runs {
                    let cc = col(c, op * alpha).unwrap_or(WHITE);
                    job.append(txt, 0.0, egui::TextFormat { font_id: font.clone(), color: cc, ..Default::default() });
                }
                let g = painter.layout_job(job);
                let gw = g.size().x;
                let base = tp(*x, *y);
                let dx = match a {
                    1 => gw / 2.0,
                    2 => gw,
                    _ => 0.0,
                };
                let ascent = if *hd { 0.86 } else { 0.93 } * size;
                let pos = Pos2::new(base.x - dx, base.y - ascent);
                if !visible.expand(gw).contains(pos) {
                    continue;
                }
                painter.galley(pos, g, WHITE);
            }
            Prim::I { x, y, w, h, href } => {
                let r = Rect::from_min_max(tp(*x, *y), tp(x + w, y + h));
                let tex = match href.as_str() {
                    "crest:0" => o.crests[0],
                    "crest:1" => o.crests[1],
                    _ => None,
                };
                if let Some(t) = tex {
                    painter.image(t, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
            }
        }
        if Some(pi) == o.selected {
            if let Some(b) = prim_bounds(p) {
                let rr = Rect::from_min_max(tp(b.min.x, b.min.y), tp(b.max.x, b.max.y)).expand(5.0);
                painter.rect_stroke(rr, 6.0, Stroke::new(2.0, SECONDARY), egui::StrokeKind::Outside);
            }
        }
    }
    // dim legend entries of hidden layers
    if let Some(h) = o.hidden {
        for lg in &sc.legend {
            if h.contains(&lg.layer) {
                let rr = Rect::from_min_size(tp(lg.x, lg.y), Vec2::new(lg.w * s, lg.h * s));
                painter.rect_filled(rr, 4.0, Color32::from_rgba_unmultiplied(11, 10, 18, 190));
                painter.line_segment([rr.left_center(), rr.right_center()], Stroke::new(1.5, DARK_TEXT2));
            }
        }
    }
}

fn prim_bounds(p: &Prim) -> Option<Rect> {
    match p {
        Prim::R { x, y, w, h, .. } => Some(Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(*w, *h))),
        Prim::C { x, y, r, .. } => Some(Rect::from_center_size(Pos2::new(*x, *y), Vec2::splat(2.0 * r))),
        Prim::L { p, .. } | Prim::P { p, .. } => {
            let mut r = Rect::NOTHING;
            for q in p {
                r.extend_with(Pos2::new(q[0], q[1]));
            }
            Some(r)
        }
        _ => None,
    }
}

fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

fn point_in_poly(p: Pos2, pts: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + n - 1) % n]);
        if (a[1] > p.y) != (b[1] > p.y) && p.x < (b[0] - a[0]) * (p.y - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

pub enum Hit {
    Legend(i32),
    Mark(usize, i32),
    None,
}

/// Hit-test a screen position against legend entries, then tooltip marks (topmost first).
pub fn hit(rect: Rect, sc: &Scene, pos: Pos2, hidden: &HashSet<i32>) -> Hit {
    let s = rect.width() / sc.w;
    let q = Pos2::new((pos.x - rect.min.x) / s, (pos.y - rect.min.y) / s);
    for lg in &sc.legend {
        if Rect::from_min_size(Pos2::new(lg.x, lg.y), Vec2::new(lg.w, lg.h)).expand(6.0 / s).contains(q) {
            return Hit::Legend(lg.layer);
        }
    }
    let slop = 14.0 / s; // ~14 px finger slop on screen
    let mut best: Option<(usize, i32, f32)> = None;
    for (i, p) in sc.prims.iter().enumerate().rev() {
        let (l, t) = prim_layer_tip(p);
        if t < 0 || (l >= 0 && hidden.contains(&l)) {
            continue;
        }
        let d = match p {
            Prim::C { x, y, r, .. } => (Pos2::new(*x, *y).distance(q) - r).max(0.0),
            Prim::R { x, y, w, h, .. } => {
                let rr = Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(*w, *h));
                if rr.contains(q) { 0.0 } else { rr.distance_to_pos(q) }
            }
            Prim::L { p, sw, .. } => p.windows(2).map(|w| seg_dist(q, Pos2::new(w[0][0], w[0][1]), Pos2::new(w[1][0], w[1][1]))).fold(f32::MAX, f32::min) - sw / 2.0,
            Prim::P { p, .. } => {
                if point_in_poly(q, p) { 0.0 } else { p.iter().map(|v| Pos2::new(v[0], v[1]).distance(q)).fold(f32::MAX, f32::min) }
            }
            _ => f32::MAX,
        };
        if d <= slop && best.map(|b| d < b.2 - 0.5).unwrap_or(true) {
            best = Some((i, t, d));
            if d == 0.0 {
                break;
            }
        }
    }
    match best {
        Some((i, t, _)) => Hit::Mark(i, t),
        None => Hit::None,
    }
}

/// Screen anchor for a mark (for tooltip placement).
pub fn mark_anchor(rect: Rect, sc: &Scene, idx: usize) -> Option<Pos2> {
    let s = rect.width() / sc.w;
    let b = prim_bounds(sc.prims.get(idx)?)?;
    Some(Pos2::new(rect.min.x + b.center().x * s, rect.min.y + b.min.y * s))
}

/// Glass tooltip bubble.
pub fn tooltip(ui: &Ui, anchor: Pos2, text: &str, bounds: Rect) {
    let painter = ui.painter().clone().with_layer_id(egui::LayerId::new(egui::Order::Tooltip, egui::Id::NULL.with("scene_tip")));
    let mut job = LayoutJob::simple(text.to_string(), body(13.0), WHITE, 250.0);
    job.wrap.max_width = 250.0;
    let g = painter.layout_job(job);
    let size = g.size() + Vec2::new(24.0, 18.0);
    let mut pos = Pos2::new(anchor.x - size.x / 2.0, anchor.y - size.y - 12.0);
    if pos.y < bounds.top() + 4.0 {
        pos.y = anchor.y + 22.0;
    }
    pos.x = pos.x.clamp(bounds.left() + 6.0, (bounds.right() - size.x - 6.0).max(bounds.left() + 6.0));
    let r = Rect::from_min_size(pos, size);
    painter.rect_filled(r.translate(Vec2::new(0.0, 3.0)), 12.0, Color32::from_black_alpha(90));
    painter.rect_filled(r, 12.0, Color32::from_rgba_unmultiplied(28, 22, 40, 240));
    painter.rect_stroke(r, 12.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40)), egui::StrokeKind::Inside);
    painter.circle_filled(anchor, 4.0, SECONDARY);
    painter.galley(pos + Vec2::new(12.0, 9.0), g, WHITE);
}
