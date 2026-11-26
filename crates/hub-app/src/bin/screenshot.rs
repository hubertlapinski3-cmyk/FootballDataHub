//! Headless screenshot harness: runs the real App UI through egui without a GPU
//! and rasterises the tessellated meshes in software. Used to verify every screen.
//!
//! usage: screenshot <data_dir> <out_dir> [whoscored_file]

use egui::epaint::{ClippedPrimitive, ImageDelta, Primitive};
use egui::{Color32, ImageData, Pos2, RawInput, Rect, TextureId, Vec2};
use hub_app::library::{self, Source};
use hub_app::{App, Screen};
use std::collections::HashMap;
use std::path::PathBuf;

struct Tex {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

fn apply_delta(texs: &mut HashMap<TextureId, Tex>, id: TextureId, d: &ImageDelta) {
    let img = match &d.image {
        ImageData::Color(c) => c.clone(),
    };
    let (w, h) = (img.size[0], img.size[1]);
    match d.pos {
        None => {
            texs.insert(id, Tex { w, h, px: img.pixels.clone() });
        }
        Some([x0, y0]) => {
            if let Some(t) = texs.get_mut(&id) {
                for y in 0..h {
                    for x in 0..w {
                        let (tx, ty) = (x0 + x, y0 + y);
                        if tx < t.w && ty < t.h {
                            t.px[ty * t.w + tx] = img.pixels[y * w + x];
                        }
                    }
                }
            }
        }
    }
}

fn sample(t: &Tex, u: f32, v: f32) -> [f32; 4] {
    let x = (u * t.w as f32 - 0.5).clamp(0.0, (t.w - 1) as f32);
    let y = (v * t.h as f32 - 0.5).clamp(0.0, (t.h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let g = |xx: usize, yy: usize| {
        let c = t.px[yy * t.w + xx];
        [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
    };
    let (a, b, c, d) = (g(x0, y0), g(x1, y0), g(x0, y1), g(x1, y1));
    let mut o = [0.0; 4];
    for i in 0..4 {
        o[i] = (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy;
    }
    o
}

fn raster(prims: &[ClippedPrimitive], texs: &HashMap<TextureId, Tex>, w: usize, h: usize, ppp: f32) -> Vec<u8> {
    let mut buf = vec![[0.0f32; 4]; w * h];
    for cp in prims {
        let Primitive::Mesh(mesh) = &cp.primitive else { continue };
        let Some(tex) = texs.get(&mesh.texture_id) else { continue };
        let clip = Rect::from_min_max(cp.clip_rect.min * ppp, cp.clip_rect.max * ppp);
        for tri in mesh.indices.chunks(3) {
            let v = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
            let p: Vec<Pos2> = v.iter().map(|x| (x.pos.to_vec2() * ppp).to_pos2()).collect();
            let area = (p[1].x - p[0].x) * (p[2].y - p[0].y) - (p[2].x - p[0].x) * (p[1].y - p[0].y);
            if area.abs() < 1e-6 {
                continue;
            }
            let minx = p.iter().map(|q| q.x).fold(f32::MAX, f32::min).max(clip.min.x).max(0.0).floor() as i32;
            let maxx = p.iter().map(|q| q.x).fold(f32::MIN, f32::max).min(clip.max.x).min(w as f32).ceil() as i32;
            let miny = p.iter().map(|q| q.y).fold(f32::MAX, f32::min).max(clip.min.y).max(0.0).floor() as i32;
            let maxy = p.iter().map(|q| q.y).fold(f32::MIN, f32::max).min(clip.max.y).min(h as f32).ceil() as i32;
            for y in miny..maxy {
                for x in minx..maxx {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((p[1].x - px) * (p[2].y - py) - (p[2].x - px) * (p[1].y - py)) / area;
                    let w1 = ((p[2].x - px) * (p[0].y - py) - (p[0].x - px) * (p[2].y - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                        continue;
                    }
                    let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                    let vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                    let s = sample(tex, u, vv);
                    let mut c = [0.0f32; 4];
                    for i in 0..4 {
                        let vc = [v[0].color, v[1].color, v[2].color].map(|k| [k.r(), k.g(), k.b(), k.a()][i] as f32);
                        let col = w0 * vc[0] + w1 * vc[1] + w2 * vc[2];
                        c[i] = col / 255.0 * s[i] / 255.0; // premultiplied * premultiplied
                    }
                    let d = &mut buf[y as usize * w + x as usize];
                    for i in 0..4 {
                        d[i] = c[i] + d[i] * (1.0 - c[3]);
                    }
                }
            }
        }
    }
    buf.iter().flat_map(|c| {
        let a = c[3].max(1e-6);
        [(c[0] / a.max(1.0) * 255.0).min(255.0) as u8, (c[1] / a.max(1.0) * 255.0).min(255.0) as u8, (c[2] / a.max(1.0) * 255.0).min(255.0) as u8, 255]
    }).collect()
}

struct Harness {
    time: f64,
    ctx: egui::Context,
    app: App,
    texs: HashMap<TextureId, Tex>,
    size: Vec2,
    ppp: f32,
}

impl Harness {
    fn step(&mut self, events: Vec<egui::Event>) -> Vec<ClippedPrimitive> {
        self.time += 0.1;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        self.ctx.set_pixels_per_point(self.ppp);
        let app = &mut self.app;
        let out = self.ctx.run_ui(input, |ui| app.frame(ui));
        for (id, ds) in &out.textures_delta.set {
            for d in ds.iter() {
                apply_delta(&mut self.texs, *id, d);
            }
        }
        for id in &out.textures_delta.free {
            self.texs.remove(id);
        }
        self.ctx.tessellate(out.shapes, out.pixels_per_point)
    }
    /// Run frames until background rasterisation settles, then capture.
    fn shot(&mut self, path: &str, scroll: f32) {
        for _ in 0..3 {
            self.step(vec![]);
        }
        for _ in 0..200 {
            self.step(vec![]);
            if !self.app.raster.is_pending() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if scroll != 0.0 {
            self.step(vec![egui::Event::PointerMoved(Pos2::new(200.0, 500.0))]);
            for _ in 0..6 {
                self.step(vec![egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, -scroll / 6.0), modifiers: Default::default(), phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown }]);
            }
            for _ in 0..200 {
                self.step(vec![]);
                if !self.app.raster.is_pending() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        let prims = self.step(vec![]);
        let (w, h) = ((self.size.x * self.ppp) as usize, (self.size.y * self.ppp) as usize);
        let px = raster(&prims, &self.texs, w, h, self.ppp);
        image::save_buffer(path, &px, w as u32, h as u32, image::ColorType::Rgba8).unwrap();
        eprintln!("wrote {path}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = PathBuf::from(args.get(1).cloned().unwrap_or("shot_data".into()));
    let out = PathBuf::from(args.get(2).cloned().unwrap_or("shots".into()));
    std::fs::create_dir_all(&out).unwrap();
    let ctx = egui::Context::default();
    let mut app = App::new(&ctx, root.clone());
    if app.lib.entries.is_empty() {
        eprintln!("building sample match…");
        library::build_match(&root, Source::Sample, false, None, Default::default()).expect("sample build");
        if let Some(ws) = args.get(3) {
            eprintln!("building WhoScored match…");
            library::build_match(&root, Source::WhoScored(ws.clone()), false, None, Default::default()).expect("ws build");
        }
        app.lib.reload();
    }
    let w: f32 = std::env::var("SHOT_W").ok().and_then(|v| v.parse().ok()).unwrap_or(412.0);
    let mut app = app;
    if let Ok(t) = std::env::var("THEME") {
        app.prefs.theme = t;
    }
    if let Some(t) = std::env::var("TEXT_SCALE").ok().and_then(|v| v.parse().ok()) {
        app.prefs.text_scale = t;
    }
    let mut h = Harness { time: 100.0, ctx, app, texs: HashMap::new(), size: Vec2::new(w, 915.0), ppp: 2.0 };
    let o = |n: &str| out.join(n).display().to_string();
    let sample = h.app.lib.entries.iter().position(|e| e.s.source == "statsbomb").unwrap_or(0);

    // BOARDS=id1,id2: render those boards whole (viewer, tall window) and stop
    if let Ok(ids) = std::env::var("BOARDS") {
        for id in ids.split(',') {
            let meta = h.app.lib.entries[sample].s.boards.iter().find(|b| b.id == id).cloned();
            let Some(meta) = meta else { continue };
            h.size = Vec2::new(412.0, (412.0 * (meta.height / meta.width) as f32 + 300.0).min(4000.0));
            h.app.stack = vec![Screen::Home, Screen::Hub(sample), Screen::Viewer { m: sample, board: id.into() }];
            h.app.viewer = hub_app::ViewerState::fresh(-10.0);
            h.shot(&o(&format!("board_{id}.png")), 0.0);
        }
        return;
    }
    h.app.stack = vec![Screen::Onboarding];
    h.shot(&o("1_onboarding.png"), 0.0);
    h.app.stack = vec![Screen::Home];
    h.shot(&o("2_home.png"), 0.0);
    h.app.stack = vec![Screen::Home, Screen::Hub(sample)];
    h.shot(&o("3_hub.png"), 0.0);
    h.shot(&o("3b_hub_scrolled.png"), 1400.0);
    h.app.stack = vec![Screen::Home, Screen::Hub(sample), Screen::Viewer { m: sample, board: "08_box_entries_home".into() }];
    h.app.viewer = hub_app::ViewerState::fresh(-10.0);
    h.shot(&o("4_viewer.png"), 0.0);
    // interactive state: tooltip on the biggest shot of the home shot map, one legend layer hidden
    h.app.stack = vec![Screen::Home, Screen::Hub(sample), Screen::Viewer { m: sample, board: "07_shot_map_home".into() }];
    h.app.viewer = hub_app::ViewerState::fresh(-10.0);
    {
        let p = h.app.lib.entries[sample].dir.join("boards").join("07_shot_map_home.scene.json");
        let sc: matchhub::render::svg::Scene = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        let mut best = None;
        for (i, pr) in sc.prims.iter().enumerate() {
            if let matchhub::render::svg::Prim::C { r, t, .. } = pr {
                if *t >= 0 && best.map(|(_, _, br)| *r > br).unwrap_or(true) {
                    best = Some((i, *t, *r));
                }
            }
        }
        if let Some((i, t, _)) = best {
            h.app.viewer.selected = Some((i, t));
        }
        if let Some(l) = sc.layers.iter().position(|x| x == "Blocked") {
            h.app.viewer.hidden.insert(l as i32);
        }
    }
    h.shot(&o("4c_viewer_tooltip.png"), 0.0);
    h.app.stack = vec![Screen::Home, Screen::Hub(sample), Screen::Viewer { m: sample, board: "08_box_entries_home".into() }];
    h.app.viewer = hub_app::ViewerState::fresh(-10.0);
    h.app.viewer.show_manifest = true;
    h.shot(&o("4b_viewer_manifest.png"), 0.0);
    h.app.viewer.show_manifest = false;
    h.app.stack = vec![Screen::Home, Screen::Report(sample)];
    h.shot(&o("5_report.png"), 0.0);
    h.app.stack = vec![Screen::Home, Screen::Players(sample)];
    h.shot(&o("6_players.png"), 0.0);
    let pid = h.app.lib.entries[sample].s.players.iter().max_by(|a, b| a.vaep.partial_cmp(&b.vaep).unwrap()).unwrap().id;
    h.app.stack = vec![Screen::Home, Screen::Player { m: sample, id: pid }];
    h.shot(&o("6b_player.png"), 0.0);
    h.app.stack = vec![Screen::Home, Screen::Compare];
    h.app.compare.left = Some((sample, 0));
    h.app.compare.right = Some((sample, 1));
    h.app.compare.ran = true;
    h.shot(&o("7_compare.png"), 0.0);
    h.app.stack = vec![Screen::Home, Screen::Settings];
    h.shot(&o("8_settings.png"), 0.0);
    h.app.stack = vec![Screen::Home, Screen::Matches];
    h.shot(&o("9_matches.png"), 0.0);
}
