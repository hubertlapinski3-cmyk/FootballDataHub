pub mod boards1;
pub mod boards2;
pub mod boards3;
pub mod frame;
pub mod pitch;
pub mod scout;
pub mod portrait;
pub mod svg;

use crate::model::Side;
use crate::store::TeamMatchRow;
use anyhow::Result;
use frame::Ctx;
use serde::Serialize;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Clone, Serialize)]
pub struct Board {
    pub id: String,
    pub title: String,
    pub section: String,
    pub how: String,
    pub width: f64,
    pub height: f64,
    #[serde(skip)]
    pub svg: String,
    /// JSON vector scene for native, interactive rendering in the app
    #[serde(skip)]
    pub scene: String,
}

impl Board {
    pub fn new(id: &str, title: &str, s: svg::Svg) -> Board {
        let how = s.how.clone();
        // phone layout: pitch-and-panel boards are recomposed into one portrait column
        let portrait = portrait::reflow(&s.scene);
        let (w, h) = match &portrait {
            Some(p) => (p.w as f64, p.h as f64),
            None => (s.w, s.h),
        };
        let scene = serde_json::to_string(portrait.as_ref().unwrap_or(&s.scene)).unwrap_or_default();
        Board { id: id.to_string(), title: title.to_string(), section: section_of(id).into(), how, width: w, height: h, svg: s.finish(crate::theme::BG), scene }
    }
}

pub fn section_of(id: &str) -> &'static str {
    let n: u32 = id.get(0..2).and_then(|x| x.parse().ok()).unwrap_or(0);
    match n {
        1 | 2 | 6 | 7 | 24 => "Chances and goals",
        5 | 8 | 9 | 10 | 21 | 26 => "Creation and progression",
        11 | 12 | 13 | 14 | 18 | 19 | 23 | 25 => "Territory and defence",
        3 | 4 | 15 | 16 | 20 | 27 | 28 => "Match state and simulation",
        29 | 30 => "Context",
        _ => "Players",
    }
}

/// Every board for a match, in report order.
pub fn all_boards(c: &Ctx, store: &[TeamMatchRow]) -> Vec<Board> {
    use boards1::*;
    use boards2::*;
    use boards3::*;
    let mut v = vec![xg_flow(c)];
    v.extend(goal_origins(c));
    v.push(win_odds(c));
    v.push(set_pieces(c));
    v.push(corridors(c));
    v.push(shot_anatomy(c));
    for s in Side::both() {
        v.push(shot_map(c, s));
    }
    for s in Side::both() {
        v.push(box_entries(c, s));
    }
    for s in Side::both() {
        v.push(passing_by_third(c, s));
    }
    for s in Side::both() {
        v.push(pass_network(c, s, 1));
        v.push(pass_network(c, s, 2));
    }
    for s in Side::both() {
        v.push(defensive_activity(c, s));
    }
    for s in Side::both() {
        v.push(high_regains(c, s));
    }
    v.push(high_turnovers(c));
    v.push(ppda_timeline(c));
    v.push(momentum(c));
    v.push(game_state(c));
    v.push(vaep(c));
    v.push(zones(c));
    v.push(transitions(c));
    v.push(match_stats(c));
    for s in Side::both() {
        v.push(xt_heatmap(c, s));
    }
    v.push(gk_frames(c));
    v.push(height_length(c));
    v.push(chance_vs_finishing(c));
    v.push(press_triggers(c));
    v.push(progression_scatter(c));
    v.push(funnel(c));
    v.push(entry_routes(c));
    let comp: Vec<TeamMatchRow> = store.iter().filter(|r| r.competition == c.m.competition && r.metric_version == crate::METRIC_VERSION).cloned().collect();
    v.push(context_territory(c, &comp));
    v.push(context_pressing(c, &comp));
    let mut ps: Vec<&crate::derive::metrics::PlayerStats> = c.a.players.iter().filter(|p| p.minutes >= 1.0).collect();
    ps.sort_by(|a, b| (a.side.map(|s| s.idx()), a.shirt).cmp(&(b.side.map(|s| s.idx()), b.shirt)));
    for p in ps {
        v.push(player_card(c, p));
    }
    v
}

fn fontdb() -> &'static std::sync::Arc<resvg::usvg::fontdb::Database> {
    static DB: OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        for (data, _) in crate::fonts::FONTS {
            db.load_font_data(data.to_vec());
        }
        db.set_sans_serif_family("Inter");
        std::sync::Arc::new(db)
    })
}

/// Rasterise an SVG string to RGBA pixels at the given scale.
pub fn rasterize(svg: &str, scale: f32) -> Result<(u32, u32, Vec<u8>)> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fontdb().clone();
    let tree = resvg::usvg::Tree::from_str(svg, &opt)?;
    let size = tree.size();
    let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h).ok_or_else(|| anyhow::anyhow!("pixmap"))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    // demultiply for consumers expecting straight alpha
    let data = pm.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect();
    Ok((w, h, data))
}

pub fn save_png(svg: &str, path: &Path, scale: f32) -> Result<()> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fontdb().clone();
    let tree = resvg::usvg::Tree::from_str(svg, &opt)?;
    let size = tree.size();
    let mut pm = resvg::tiny_skia::Pixmap::new((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32).ok_or_else(|| anyhow::anyhow!("pixmap"))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    pm.save_png(path)?;
    Ok(())
}

pub fn crest_data_uri(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mime = if bytes.starts_with(b"\x89PNG") { "image/png" } else if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") { "image/svg+xml" } else { "image/jpeg" };
    Some(format!("data:{mime};base64,{}", svg::base64(&bytes)))
}
