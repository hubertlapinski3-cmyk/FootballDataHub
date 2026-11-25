//! Best-effort cross-source enrichment: FotMob and SofaScore are located
//! automatically from the WhoScored date + team names, then provide team
//! colours, crests and per-shot xG / xGOT. Every failure is recorded as a
//! fallback in the manifest instead of aborting the build.

use crate::model::*;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::Path;

pub fn normalise(s: &str) -> String {
    let mut o = String::new();
    for c in s.to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            'ß' => 's',
            c => c,
        };
        if c.is_alphanumeric() || c == ' ' {
            o.push(c);
        }
    }
    let stop = ["fc", "cf", "afc", "sc", "ac", "bv", "club", "de", "the", "calcio", "ssc", "as", "rc", "cd", "ud", "1", "09"];
    o.split_whitespace().filter(|w| !stop.contains(w)).collect::<Vec<_>>().join(" ")
}

pub fn name_sim(a: &str, b: &str) -> f64 {
    let a = normalise(a);
    let b = normalise(b);
    if a == b {
        return 1.0;
    }
    if a.contains(&b) || b.contains(&a) {
        return 0.9;
    }
    let ta: Vec<&str> = a.split_whitespace().collect();
    let tb: Vec<&str> = b.split_whitespace().collect();
    let inter = ta.iter().filter(|w| tb.iter().any(|v| v == *w || (v.len() > 4 && w.len() > 4 && (v.starts_with(*w) || w.starts_with(*v))))).count();
    inter as f64 / ta.len().max(tb.len()).max(1) as f64
}

fn get_cached(cache: &Path, key: &str, url: &str) -> Result<Value> {
    let p = cache.join(key);
    if p.exists() {
        return Ok(serde_json::from_str(&std::fs::read_to_string(&p)?)?);
    }
    let txt = crate::net::get_text(url, &[("Accept", "application/json")], 20)?;
    let v: Value = serde_json::from_str(&txt)?;
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, txt)?;
    Ok(v)
}

pub fn enrich(m: &mut Match, cache: &Path, online: bool) {
    if !online {
        m.fallbacks.push("enrichment skipped (--offline): colours from overrides/crest/defaults, xGOT from in-house model".into());
        return;
    }
    if let Err(e) = sofascore(m, cache) {
        m.fallbacks.push(format!("SofaScore unavailable: {e}"));
    }
    if let Err(e) = fotmob(m, cache) {
        m.fallbacks.push(format!("FotMob unavailable: {e}"));
    }
}

fn date_compact(d: &str) -> String {
    d.replace('-', "")
}

fn sofascore(m: &mut Match, cache: &Path) -> Result<()> {
    let date = &m.date;
    let v = get_cached(cache, &format!("sofascore/day_{date}.json"), &format!("https://api.sofascore.com/api/v1/sport/football/scheduled-events/{date}"))?;
    let (home, away) = (m.teams[0].name.clone(), m.teams[1].name.clone());
    let ev = v["events"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .map(|e| (name_sim(e["homeTeam"]["name"].as_str().unwrap_or(""), &home) + name_sim(e["awayTeam"]["name"].as_str().unwrap_or(""), &away), e))
                .filter(|(s, _)| *s >= 1.2)
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                .map(|(_, e)| e.clone())
        })
        .ok_or_else(|| anyhow!("no SofaScore event matched {home} v {away} on {date}"))?;
    for (i, k) in ["homeTeam", "awayTeam"].iter().enumerate() {
        let tc = &ev[*k]["teamColors"];
        for c in ["primary", "secondary"] {
            if let Some(h) = tc[c].as_str() {
                m.teams[i].colour_hints.push((format!("sofascore:{c}"), h.to_string()));
            }
        }
        if let Some(sn) = ev[*k]["shortName"].as_str() {
            m.teams[i].short_name = sn.to_string();
        }
        if let Some(id) = ev[*k]["id"].as_u64() {
            m.teams[i].crest_url.get_or_insert(format!("https://api.sofascore.app/api/v1/team/{id}/image"));
        }
    }
    let eid = ev["id"].as_u64().ok_or_else(|| anyhow!("no event id"))?;
    let sm = get_cached(cache, &format!("sofascore/shotmap_{eid}.json"), &format!("https://api.sofascore.com/api/v1/event/{eid}/shotmap"))?;
    let shots: Vec<(Side, f64, String, Option<f64>, Option<f64>)> = sm["shotmap"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    (
                        if s["isHome"].as_bool().unwrap_or(true) { Side::Home } else { Side::Away },
                        s["time"].as_f64().unwrap_or(0.0) + s["addedTime"].as_f64().unwrap_or(0.0),
                        s["player"]["name"].as_str().unwrap_or("").to_string(),
                        s["xg"].as_f64(),
                        s["xgot"].as_f64(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    attach_shot_values(m, &shots, "sofascore");
    Ok(())
}

fn fotmob(m: &mut Match, cache: &Path) -> Result<()> {
    let d = date_compact(&m.date);
    let v = get_cached(cache, &format!("fotmob/day_{d}.json"), &format!("https://www.fotmob.com/api/data/matches?date={d}"))
        .or_else(|_| get_cached(cache, &format!("fotmob/day_{d}_legacy.json"), &format!("https://www.fotmob.com/api/matches?date={d}")))?;
    let (home, away) = (m.teams[0].name.clone(), m.teams[1].name.clone());
    let mut best: Option<(f64, u64)> = None;
    for lg in v["leagues"].as_array().cloned().unwrap_or_default() {
        for mt in lg["matches"].as_array().cloned().unwrap_or_default() {
            let s = name_sim(mt["home"]["name"].as_str().unwrap_or(""), &home) + name_sim(mt["away"]["name"].as_str().unwrap_or(""), &away);
            if s >= 1.2 && best.map(|b| s > b.0).unwrap_or(true) {
                best = Some((s, mt["id"].as_u64().unwrap_or(0)));
            }
        }
    }
    let (_, fid) = best.ok_or_else(|| anyhow!("no FotMob match for {home} v {away} on {d}"))?;
    let md = get_cached(cache, &format!("fotmob/match_{fid}.json"), &format!("https://www.fotmob.com/api/data/matchDetails?matchId={fid}"))?;
    let tc = &md["general"]["teamColors"]["darkMode"];
    for (i, k) in ["home", "away"].iter().enumerate() {
        if let Some(h) = tc[*k].as_str() {
            m.teams[i].colour_hints.insert(0, ("fotmob:dark".into(), h.to_string()));
        }
        if let Some(t) = md["header"]["teams"].as_array().and_then(|a| a.get(i)) {
            if let Some(u) = t["imageUrl"].as_str() {
                m.teams[i].crest_url = Some(u.to_string());
            }
        }
    }
    let home_id = md["general"]["homeTeam"]["id"].as_u64();
    let shots: Vec<(Side, f64, String, Option<f64>, Option<f64>)> = md["content"]["shotmap"]["shots"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    (
                        if s["teamId"].as_u64() == home_id { Side::Home } else { Side::Away },
                        s["min"].as_f64().unwrap_or(0.0) + s["minAdded"].as_f64().unwrap_or(0.0),
                        s["playerName"].as_str().unwrap_or("").to_string(),
                        s["expectedGoals"].as_f64(),
                        s["expectedGoalsOnTarget"].as_f64(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    attach_shot_values(m, &shots, "fotmob");
    Ok(())
}

/// Match external shots to our shot events by side, minute (±1) and player surname.
fn attach_shot_values(m: &mut Match, ext: &[(Side, f64, String, Option<f64>, Option<f64>)], src: &str) {
    let names: Vec<(u64, String)> = m.teams.iter().flat_map(|t| t.players.iter().map(|p| (p.id, p.name.clone()))).collect();
    let mut matched = 0;
    for (side, minute, pname, xg, xgot) in ext {
        let best = m
            .events
            .iter_mut()
            .filter(|e| e.kind == Kind::Shot && e.side == *side && (e.minute as f64 + 1.0 - minute).abs() <= 1.5)
            .filter(|e| e.player.and_then(|p| names.iter().find(|n| n.0 == p)).map(|n| name_sim(&n.1, pname) > 0.3).unwrap_or(false))
            .find(|e| e.shot.as_ref().map(|s| s.xgot_ext.is_none() || src == "fotmob").unwrap_or(false));
        if let Some(e) = best {
            let s = e.shot.as_mut().unwrap();
            if xg.is_some() {
                s.xg_ext = *xg;
            }
            if xgot.is_some() {
                s.xgot_ext = *xgot;
            }
            matched += 1;
        }
    }
    if matched < ext.len() {
        m.fallbacks.push(format!("{src}: matched {matched}/{} shots", ext.len()));
    }
}
