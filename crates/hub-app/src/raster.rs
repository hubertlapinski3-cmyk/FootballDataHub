//! Background SVG -> texture rasterisation with a small texture cache.
//! Boards are stored as SVG and rasterised on demand at the needed scale.

use egui::{ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

struct Job {
    key: String,
    path: PathBuf,
    scale: f32,
}

pub struct Raster {
    tx: Sender<Job>,
    rx: Receiver<(String, Result<ColorImage, String>)>,
    pending: HashSet<String>,
    pub textures: HashMap<String, TextureHandle>,
    pub errors: HashMap<String, String>,
    order: Vec<String>,
    budget: usize,
}

impl Raster {
    pub fn new(ctx: &egui::Context, workers: usize) -> Raster {
        let (tx, jrx) = channel::<Job>();
        let (rtx, rx) = channel();
        let jrx = std::sync::Arc::new(std::sync::Mutex::new(jrx));
        for _ in 0..workers.max(1) {
            let jrx = jrx.clone();
            let rtx = rtx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || loop {
                let job = { jrx.lock().unwrap().recv() };
                let Ok(job) = job else { break };
                let r = std::fs::read_to_string(&job.path).map_err(|e| e.to_string()).and_then(|svg| {
                    matchhub::render::rasterize(&svg, job.scale).map_err(|e| e.to_string()).map(|(w, h, px)| ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &px))
                });
                let _ = rtx.send((job.key, r));
                ctx.request_repaint();
            });
        }
        Raster { tx, rx, pending: HashSet::new(), textures: HashMap::new(), errors: HashMap::new(), order: vec![], budget: 90 }
    }

    /// Returns the texture if ready, otherwise queues it.
    pub fn get(&mut self, path: PathBuf, scale: f32) -> Option<TextureHandle> {
        let key = format!("{}@{scale:.2}", path.display());
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        if !self.pending.contains(&key) && !self.errors.contains_key(&key) {
            self.pending.insert(key.clone());
            let _ = self.tx.send(Job { key, path, scale });
        }
        None
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((key, r)) = self.rx.try_recv() {
            self.pending.remove(&key);
            match r {
                Ok(img) => {
                    let t = ctx.load_texture(&key, img, TextureOptions::LINEAR);
                    self.textures.insert(key.clone(), t);
                    self.order.push(key);
                    // evict oldest big textures first
                    while self.order.len() > self.budget {
                        let k = self.order.remove(0);
                        self.textures.remove(&k);
                    }
                }
                Err(e) => {
                    self.errors.insert(key, e);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.textures.clear();
        self.order.clear();
        self.errors.clear();
    }
}
