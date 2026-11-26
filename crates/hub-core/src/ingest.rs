//! Nightly league ingestion: raw WhoScored match data -> match packet + player rows -> catalog and scouting index.
//!
//! Working layout (the checkout of the published data branch):
//!
//! ```text
//! raw/<league>/<match_id>.json.gz     WhoScored matchCentreData as scripts/ingest_leagues.py saves it (input)
//! matches/<league>/<match_id>.json.gz match packet: the parsed Match, enrichment included (published)
//! rows/<league>/<match_id>.json       catalog entry + one row per player (published, small)
//! catalog.json, scout_index.json      built from the rows (published)
//! ```

use crate::catalog::*;
use crate::model::Match;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowFile {
    pub entry: CatalogMatch,
    pub rows: Vec<PlayerMatchRow>,
}

#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub raw_dir: PathBuf,
    pub out_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub season: String,
    pub league: Option<League>,
    pub max_matches: Option<usize>,
    pub online: bool,
    pub force: bool,
    pub pool_minutes: f32,
}

#[derive(Debug, Default)]
pub struct IngestReport {
    pub processed: usize,
    pub skipped: usize,
    pub failed: Vec<(String, String)>,
}

fn packet_rel(l: League, id: &str) -> String {
    format!("matches/{}/{id}.json.gz", l.slug())
}

fn rows_path(out: &Path, l: League, id: &str) -> PathBuf {
    out.join("rows").join(l.slug()).join(format!("{id}.json"))
}

/// Packet, rows and catalog entry for one parsed match. Enrichment happens before the packet is written,
/// so the app builds it offline with the same sources the nightly job saw.
pub fn ingest_match(mut m: Match, league: League, out: &Path, cache: &Path, online: bool) -> Result<RowFile> {
    crate::sources::enrich::enrich(&mut m, cache, online);
    crate::reep::fill_ages(&mut m);
    let packet = encode_packet(&m)?;
    let rel = packet_rel(league, &m.id);
    let path = out.join(&rel);
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, &packet)?;
    let entry = catalog_entry(&m, &rel, packet.len() as u64);
    // the analysis gets its own copy: models fill expected-goals values in place
    let mut an = m.clone();
    let models = crate::embedded_models();
    let values = models.apply(&mut an);
    let a = crate::derive::metrics::analyse(&an, values, crate::sim::simulate(&an, 2000), crate::sim::live_curve(&an, 5.0));
    let rf = RowFile { entry, rows: match_rows(&an, &a, league) };
    let rp = rows_path(out, league, &m.id);
    std::fs::create_dir_all(rp.parent().unwrap())?;
    std::fs::write(&rp, serde_json::to_vec(&rf)?)?;
    Ok(rf)
}

fn raw_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map(|x| x == "gz" || x == "json").unwrap_or(false)).collect()).unwrap_or_default();
    v.sort();
    v
}

/// Processes the raw files of the chosen leagues (all seven by default), newest data last, up to `max_matches`
/// new matches in total. Matches already ingested are skipped unless `force` is set.
pub fn ingest(opts: &IngestOptions, progress: &mut dyn FnMut(&str)) -> Result<IngestReport> {
    let mut rep = IngestReport::default();
    let leagues: Vec<League> = match opts.league {
        Some(l) => vec![l],
        None => League::ALL.to_vec(),
    };
    'outer: for l in leagues {
        for f in raw_files(&opts.raw_dir.join(l.slug())) {
            let id = f.file_name().and_then(|n| n.to_str()).unwrap_or("").split('.').next().unwrap_or("").to_string();
            if id.is_empty() {
                continue;
            }
            if !opts.force && rows_path(&opts.out_dir, l, &id).exists() && opts.out_dir.join(packet_rel(l, &id)).exists() {
                rep.skipped += 1;
                continue;
            }
            if opts.max_matches.map(|n| rep.processed >= n).unwrap_or(false) {
                break 'outer;
            }
            progress(&format!("{} {id}", l.slug()));
            let res = crate::sources::whoscored::load(f.to_str().unwrap_or(""), &opts.cache_dir, None, None, &mut |_| {}).and_then(|m| ingest_match(m, l, &opts.out_dir, &opts.cache_dir, opts.online));
            match res {
                Ok(_) => rep.processed += 1,
                Err(e) => rep.failed.push((format!("{}/{id}", l.slug()), format!("{e:#}"))),
            }
        }
    }
    Ok(rep)
}

/// Rebuilds `catalog.json` and `scout_index.json` from every row file under `out`.
pub fn finalize(out: &Path, season: &str, pool_minutes: f32) -> Result<(Catalog, ScoutIndex)> {
    let mut entries: Vec<(League, CatalogMatch)> = vec![];
    let mut rows: Vec<PlayerMatchRow> = vec![];
    for l in League::ALL {
        let dir = out.join("rows").join(l.slug());
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map(|x| x == "json").unwrap_or(false)).collect();
        files.sort();
        for f in files {
            let rf: RowFile = serde_json::from_slice(&std::fs::read(&f)?).with_context(|| format!("read {}", f.display()))?;
            entries.push((l, rf.entry));
            rows.extend(rf.rows);
        }
    }
    let now = chrono::Utc::now().to_rfc3339();
    let cat = build_catalog(season, &entries, &now);
    let idx = build_index(&rows, season, &now, pool_minutes);
    std::fs::write(out.join("catalog.json"), serde_json::to_vec(&cat)?)?;
    std::fs::write(out.join("scout_index.json"), serde_json::to_vec(&idx)?)?;
    Ok((cat, idx))
}
