//! WhoScored (Opta) match centre parser and fetcher.
//!
//! Input: the `matchCentreData` object embedded in a WhoScored match page
//! (`require.config.params["args"] = { matchId, matchCentreData: {...}, ... }`).
//! Accepts a match id, a URL, a saved .html page or the extracted .json blob.

use crate::model::*;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::Path;

pub fn load(input: &str, cache_dir: &Path) -> Result<Match> {
    let p = Path::new(input);
    let raw = if p.exists() {
        let bytes = std::fs::read(p).with_context(|| format!("reading {input}"))?;
        if bytes.starts_with(&[0x1f, 0x8b]) {
            use std::io::Read;
            let mut s = String::new();
            flate2::read::GzDecoder::new(&bytes[..]).read_to_string(&mut s)?;
            s
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        }
    } else {
        let id = match_id_from(input).ok_or_else(|| anyhow!("not a file, id or WhoScored URL: {input}"))?;
        let cached = cache_dir.join("whoscored").join(format!("{id}.json"));
        if cached.exists() {
            std::fs::read_to_string(&cached)?
        } else {
            let html = fetch_page(&id)?;
            let blob = extract_blob(&html)?;
            std::fs::create_dir_all(cached.parent().unwrap())?;
            std::fs::write(&cached, &blob)?;
            blob
        }
    };
    let is_html = !raw.trim_start().starts_with('{');
    let json_text = if is_html { extract_blob(&raw)? } else { raw.clone() };
    let mut v: Value = serde_json::from_str(&json_text).context("parsing matchCentreData JSON")?;
    if is_html {
        if let Some((comp, season)) = breadcrumb(&raw) {
            v["competitionName"] = Value::String(comp);
            v["seasonName"] = Value::String(season);
        }
        if let Some(c) = regex::Regex::new(r"matchId:\s*(\d+)").unwrap().captures(&raw) {
            v["matchId"] = Value::from(c[1].parse::<u64>().unwrap_or(0));
        }
    }
    let id = match_id_from(input).or_else(|| v["matchId"].as_u64().map(|x| x.to_string())).unwrap_or_else(|| "unknown".into());
    parse(&v, &id)
}

/// Competition and season from the breadcrumb ("LaLiga - 2022/2023").
fn breadcrumb(html: &str) -> Option<(String, String)> {
    let i = html.find("id=\"breadcrumb-nav\"")?;
    let frag = &html[i..(i + 4000).min(html.len())];
    let re = regex::Regex::new(r"<a[^>]*>([^<]+)</a>").unwrap();
    let txt = re.captures(frag)?[1].trim().to_string();
    let parts: Vec<&str> = txt.split(" - ").collect();
    if parts.len() >= 2 {
        Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
    } else {
        Some((txt, String::new()))
    }
}

pub fn match_id_from(s: &str) -> Option<String> {
    if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
        return Some(s.to_string());
    }
    let re = regex::Regex::new(r"(?i)matches/(\d+)").unwrap();
    re.captures(s).map(|c| c[1].to_string())
}

fn fetch_page(id: &str) -> Result<String> {
    let url = format!("https://www.whoscored.com/Matches/{id}/Live");
    // Politeness: one request per match, realistic headers, cached afterwards.
    let (status, body) = crate::net::get(&url, &[("Referer", "https://www.whoscored.com/"), ("Accept", "text/html,application/xhtml+xml"), ("Accept-Language", "en-GB,en;q=0.9")], 30)
        .with_context(|| format!("GET {url}"))?;
    if status == 403 {
        return Err(anyhow!("WhoScored bot wall (HTTP 403) for {url}. Open the page in a browser, save it (Ctrl+S / Share > Save page) and import the .html file instead. Wait before retrying: repeated requests prolong the block."));
    }
    if !(200..300).contains(&status) {
        return Err(anyhow!("WhoScored returned HTTP {status} for {url}"));
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Extract the matchCentreData JSON object from page HTML with a brace matcher
/// (regex alone is unreliable on nested objects).
pub fn extract_blob(html: &str) -> Result<String> {
    // Search after the args script marker first (same approach as the
    // Scrape-Whoscored-Event-Data SDK), then anywhere.
    let key = "matchCentreData";
    let base = html.find("require.config.params[\"args\"]").unwrap_or(0);
    let start = html[base..].find(key).map(|i| i + base).or_else(|| html.find(key)).ok_or_else(|| anyhow!("matchCentreData not found in page (bot wall or not a match-centre page?)"))?;
    let bytes = html.as_bytes();
    let mut i = start + key.len();
    while i < bytes.len() && bytes[i] != b'{' {
        i += 1;
    }
    let begin = i;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if esc {
                esc = false
            } else if c == b'\\' {
                esc = true
            } else if c == b'"' {
                in_str = false
            }
        } else if c == b'"' {
            in_str = true
        } else if c == b'{' {
            depth += 1
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Ok(html[begin..=i].to_string());
            }
        }
        i += 1;
    }
    Err(anyhow!("unterminated matchCentreData object"))
}

fn qnames(e: &Value) -> Vec<(String, String)> {
    e["qualifiers"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|q| {
                    (
                        q["type"]["displayName"].as_str().unwrap_or("").to_string(),
                        q["value"].as_str().map(|s| s.to_string()).unwrap_or_default(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn has(q: &[(String, String)], name: &str) -> bool {
    q.iter().any(|(n, _)| n == name)
}
fn qval(q: &[(String, String)], name: &str) -> Option<f64> {
    q.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.parse().ok())
}

fn fmt_formation(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_digit()) {
        s.chars().map(|c| c.to_string()).collect::<Vec<_>>().join("-")
    } else {
        s.to_string()
    }
}

pub fn parse(v: &Value, id: &str) -> Result<Match> {
    let mut teams: Vec<Team> = vec![];
    let mut team_ids: Vec<i64> = vec![];
    let mut gk_ids = std::collections::HashSet::new();
    for key in ["home", "away"] {
        let t = &v[key];
        let tid = t["teamId"].as_i64().ok_or_else(|| anyhow!("missing {key}.teamId"))?;
        team_ids.push(tid);
        let mut players = vec![];
        for p in t["players"].as_array().cloned().unwrap_or_default() {
            let pid = p["playerId"].as_u64().unwrap_or(0);
            let name = p["name"].as_str().unwrap_or("?").to_string();
            let pos = p["position"].as_str().unwrap_or("Sub").to_string();
            let starter = p["isFirstEleven"].as_bool().unwrap_or(false);
            if pos == "GK" {
                gk_ids.insert(pid);
            }
            players.push(Player {
                id: pid,
                short: short_name(&name),
                name,
                shirt: p["shirtNo"].as_u64().unwrap_or(0) as u32,
                role: Role::from_position(&pos),
                position: pos,
                starter,
                age: p["age"].as_f64(),
                on_t: if starter { 0.0 } else { p["subbedInExpandedMinute"].as_f64().map(|m| m * 60.0).unwrap_or(f64::NAN) },
                off_t: p["subbedOutExpandedMinute"].as_f64().map(|m| m * 60.0),
                replaced: p["subbedOutPlayerId"].as_u64(),
            });
        }
        // Subs inherit the role of the player they replaced.
        let roles: Vec<(u64, Role, String)> = players.iter().map(|p| (p.id, p.role, p.position.clone())).collect();
        for p in players.iter_mut() {
            if p.position == "Sub" {
                if let Some(r) = p.replaced.and_then(|rid| roles.iter().find(|x| x.0 == rid)) {
                    p.role = r.1;
                    p.position = r.2.clone();
                }
            }
        }
        let formations = t["formations"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|f| FormationSpell {
                        name: fmt_formation(f["formationName"].as_str().unwrap_or("?")),
                        start_t: f["startMinuteExpanded"].as_f64().unwrap_or(0.0) * 60.0,
                        end_t: f["endMinuteExpanded"].as_f64().unwrap_or(0.0) * 60.0,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let name = t["name"].as_str().unwrap_or("?").to_string();
        teams.push(Team {
            source_id: format!("ws:{tid}"),
            short_name: name.clone(),
            name,
            manager: t["managerName"].as_str().map(|s| s.to_string()),
            players,
            formations,
            score: t["scores"]["fulltime"].as_u64().or(t["scores"]["running"].as_u64()).unwrap_or(0) as u32,
            colour_hints: vec![],
            crest_url: Some(format!("https://d2zywfiolv4f83.cloudfront.net/img/teams/{tid}.png")),
        });
    }

    let mut events = vec![];
    for e in v["events"].as_array().cloned().unwrap_or_default() {
        let tname = e["type"]["displayName"].as_str().unwrap_or("");
        let tid = e["teamId"].as_i64().unwrap_or(-1);
        let side = if tid == team_ids[0] { Side::Home } else if tid == team_ids[1] { Side::Away } else { continue };
        let period = e["period"]["value"].as_u64().unwrap_or(1) as u8;
        if period > 5 {
            continue;
        }
        let ok = e["outcomeType"]["value"].as_u64() == Some(1);
        let q = qnames(&e);
        let player = e["playerId"].as_u64();
        let x = e["x"].as_f64().unwrap_or(0.0) * PITCH_L / 100.0;
        let y = e["y"].as_f64().unwrap_or(0.0) * PITCH_W / 100.0;
        let end_x = e["endX"].as_f64().map(|v| v * PITCH_L / 100.0).or(qval(&q, "PassEndX").map(|v| v * PITCH_L / 100.0));
        let end_y = e["endY"].as_f64().map(|v| v * PITCH_W / 100.0).or(qval(&q, "PassEndY").map(|v| v * PITCH_W / 100.0));
        let is_gk = player.map(|p| gk_ids.contains(&p)).unwrap_or(false);
        let restart = if has(&q, "CornerTaken") {
            Some(Restart::Corner)
        } else if has(&q, "FreekickTaken") || has(&q, "IndirectFreekickTaken") {
            Some(Restart::FreeKick)
        } else if has(&q, "ThrowIn") {
            Some(Restart::ThrowIn)
        } else if has(&q, "GoalKick") {
            Some(Restart::GoalKick)
        } else if has(&q, "KickOff") {
            Some(Restart::KickOff)
        } else if has(&q, "Penalty") && e["isShot"].as_bool().unwrap_or(false) {
            Some(Restart::Penalty)
        } else if has(&q, "DirectFreekick") && e["isShot"].as_bool().unwrap_or(false) {
            Some(Restart::FreeKick)
        } else {
            None
        };
        let body = if has(&q, "Head") {
            Body::Head
        } else if has(&q, "RightFoot") {
            Body::Right
        } else if has(&q, "LeftFoot") {
            Body::Left
        } else {
            Body::Other
        };
        let (kind, success) = match tname {
            "Pass" | "OffsidePass" => (Kind::Pass, ok && tname == "Pass"),
            "TakeOn" => (Kind::TakeOn, ok),
            "Foul" => {
                if ok {
                    continue; // the "fouled" half of the pair
                }
                (Kind::Foul, false)
            }
            "Tackle" => (Kind::Tackle, ok),
            "Interception" => (Kind::Interception, true),
            "BallRecovery" => (Kind::Recovery, true),
            "Clearance" => (Kind::Clearance, true),
            "BlockedPass" => (Kind::Block, true),
            "Save" => {
                if is_gk {
                    (Kind::Save, true)
                } else {
                    (Kind::Block, true)
                }
            }
            "Claim" | "KeeperPickup" | "Punch" => (Kind::Claim, ok || tname == "KeeperPickup"),
            "KeeperSweeper" | "Smother" => (Kind::Sweeper, ok),
            "Aerial" => (Kind::Aerial, ok),
            "Challenge" => (Kind::Challenge, false),
            "Dispossessed" => (Kind::Dispossessed, false),
            "Error" => (Kind::Error, false),
            "BallTouch" => (Kind::BadTouch, ok),
            "Card" => (Kind::Card, true),
            "SubstitutionOff" => (Kind::SubOff, true),
            "SubstitutionOn" => (Kind::SubOn, true),
            "Goal" | "SavedShot" | "MissedShots" | "ShotOnPost" => (Kind::Shot, tname == "Goal"),
            _ => continue,
        };
        let mut ev = Event {
            id: e["eventId"].as_u64().unwrap_or(0) + if side == Side::Away { 1_000_000 } else { 0 },
            period,
            minute: e["minute"].as_u64().unwrap_or(0) as u32,
            second: e["second"].as_u64().unwrap_or(0) as u32,
            t: e["expandedMinute"].as_f64().unwrap_or(e["minute"].as_f64().unwrap_or(0.0)) * 60.0 + e["second"].as_f64().unwrap_or(0.0),
            side,
            player,
            kind,
            success,
            x,
            y,
            end_x,
            end_y,
            restart,
            pass: None,
            shot: None,
            handling: None,
            inferred: false,
            related: e["relatedEventId"].as_u64().map(|r| r + if side == Side::Away { 1_000_000 } else { 0 }),
            possession: 0,
            sequence: 0,
            related_player: e["relatedPlayerId"].as_u64(),
        };
        match kind {
            Kind::Pass => {
                let pi = PassInfo {
                    cross: has(&q, "Cross"),
                    through: has(&q, "Throughball") || has(&q, "ThroughBall"),
                    key_pass: has(&q, "KeyPass") || has(&q, "IntentionalAssist") || has(&q, "ShotAssist"),
                    assist: has(&q, "IntentionalGoalAssist"),
                    long: has(&q, "Longball"),
                    launch: has(&q, "Launch"),
                    head: has(&q, "HeadPass"),
                    cutback: false,
                };
                ev.pass = Some(pi);
            }
            Kind::Shot => {
                let outcome = match tname {
                    "Goal" => ShotOutcome::Goal,
                    "ShotOnPost" => ShotOutcome::Woodwork,
                    "MissedShots" => ShotOutcome::OffTarget,
                    _ => {
                        if has(&q, "Blocked") {
                            ShotOutcome::Blocked
                        } else {
                            ShotOutcome::Saved
                        }
                    }
                };
                let gmy = e["goalMouthY"].as_f64().or(qval(&q, "GoalMouthY"));
                let gmz = e["goalMouthZ"].as_f64().or(qval(&q, "GoalMouthZ"));
                ev.shot = Some(ShotInfo {
                    outcome,
                    body,
                    first_time: has(&q, "FirstTouch"),
                    big_chance: has(&q, "BigChance"),
                    penalty: has(&q, "Penalty"),
                    own_goal: e["isOwnGoal"].as_bool().unwrap_or(false) || has(&q, "OwnGoal"),
                    direct_free_kick: has(&q, "DirectFreekick"),
                    fast_break: has(&q, "FastBreak"),
                    goal_u: gmy.map(|g| (g - 50.0) / 4.8),
                    goal_z: gmz.map(|g| g / 38.0),
                    assist_cross: false,
                    assist_through: false,
                    assist_cutback: false,
                    xg_ext: None,
                    xgot_ext: None,
                    xg: 0.0,
                    psxg: None,
                });
                if ev.shot.as_ref().unwrap().own_goal {
                    ev.success = true;
                }
            }
            Kind::Save => {
                ev.handling = Some(if has(&q, "ParriedDanger") {
                    Handling::ParriedDanger
                } else if has(&q, "ParriedSafe") || has(&q, "Fingertip") {
                    Handling::ParriedSafe
                } else {
                    Handling::Held
                });
            }
            _ => {}
        }
        events.push(ev);
    }

    let date = v["startDate"].as_str().or(v["startTime"].as_str()).unwrap_or("").chars().take(10).collect();
    let teams: [Team; 2] = [teams.remove(0), teams.remove(0)];
    let mut m = Match {
        id: format!("ws-{id}"),
        source: "whoscored".into(),
        competition: v["competitionName"].as_str().unwrap_or("").to_string(),
        season: v["seasonName"].as_str().unwrap_or("").to_string(),
        date,
        teams,
        events,
        periods: vec![],
        fallbacks: vec![],
    };
    super::finalize(&mut m, false);
    Ok(m)
}
