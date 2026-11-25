//! StatsBomb Open Data loader (github.com/statsbomb/open-data).
//! Used for model training and as a second full-event source for testing.
//! Licence: non-commercial use, attribute "StatsBomb Open Data".

use crate::model::*;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

const RAW: &str = "https://raw.githubusercontent.com/statsbomb/open-data/master/data";

/// Download (or reuse) the files for one match into `root` (mirrors the repo layout).
pub fn ensure_match_files(root: &Path, match_id: u64) -> Result<()> {
    for sub in ["events", "lineups"] {
        let p = root.join(sub).join(format!("{match_id}.json"));
        if !p.exists() {
            std::fs::create_dir_all(p.parent().unwrap())?;
            let url = format!("{RAW}/{sub}/{match_id}.json");
            let body = crate::net::get_ok(&url, &[], 60)?;
            std::fs::write(&p, body)?;
        }
    }
    Ok(())
}

pub fn ensure_season(root: &Path, comp: u64, season: u64) -> Result<Vec<Value>> {
    let p = root.join("matches").join(comp.to_string()).join(format!("{season}.json"));
    if !p.exists() {
        std::fs::create_dir_all(p.parent().unwrap())?;
        let url = format!("{RAW}/matches/{comp}/{season}.json");
        let body = crate::net::get_ok(&url, &[], 60)?;
        std::fs::write(&p, body)?;
    }
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&p)?)?;
    Ok(v.as_array().cloned().unwrap_or_default())
}

pub fn find_match_meta(root: &Path, match_id: u64) -> Option<Value> {
    let dir = root.join("matches");
    for comp in std::fs::read_dir(&dir).ok()? {
        let comp = comp.ok()?;
        for f in std::fs::read_dir(comp.path()).ok()? {
            let f = f.ok()?;
            let v: Value = serde_json::from_str(&std::fs::read_to_string(f.path()).ok()?).ok()?;
            if let Some(m) = v.as_array().and_then(|a| a.iter().find(|m| m["match_id"].as_u64() == Some(match_id))) {
                return Some(m.clone());
            }
        }
    }
    None
}

fn sx(x: f64) -> f64 {
    x * PITCH_L / 120.0
}
fn sy(y: f64) -> f64 {
    (80.0 - y) * PITCH_W / 80.0
}
fn loc(v: &Value) -> Option<(f64, f64)> {
    let a = v.as_array()?;
    Some((sx(a.first()?.as_f64()?), sy(a.get(1)?.as_f64()?)))
}
fn ts(s: &str) -> f64 {
    let p: Vec<f64> = s.split(':').filter_map(|x| x.parse().ok()).collect();
    if p.len() == 3 { p[0] * 3600.0 + p[1] * 60.0 + p[2] } else { 0.0 }
}
fn name(v: &Value) -> &str {
    v["name"].as_str().unwrap_or("")
}

pub fn load(root: &Path, match_id: u64) -> Result<Match> {
    let meta = find_match_meta(root, match_id).ok_or_else(|| anyhow!("match {match_id} not in {}/matches", root.display()))?;
    let ev_path = root.join("events").join(format!("{match_id}.json"));
    let lu_path = root.join("lineups").join(format!("{match_id}.json"));
    let events: Value = serde_json::from_str(&std::fs::read_to_string(&ev_path).with_context(|| format!("{}", ev_path.display()))?)?;
    let lineups: Value = serde_json::from_str(&std::fs::read_to_string(&lu_path).with_context(|| format!("{}", lu_path.display()))?)?;
    parse(&meta, &events, &lineups)
}

pub fn parse(meta: &Value, events: &Value, lineups: &Value) -> Result<Match> {
    let home_id = meta["home_team"]["home_team_id"].as_i64().unwrap_or(0);
    let away_id = meta["away_team"]["away_team_id"].as_i64().unwrap_or(0);
    let side_of = |tid: i64| if tid == home_id { Some(Side::Home) } else if tid == away_id { Some(Side::Away) } else { None };

    // Period durations -> expanded clock offsets.
    let evs = events.as_array().ok_or_else(|| anyhow!("events not an array"))?;
    let mut pmax: HashMap<u8, f64> = HashMap::new();
    for e in evs {
        let p = e["period"].as_u64().unwrap_or(1) as u8;
        let t = ts(e["timestamp"].as_str().unwrap_or("0:0:0"));
        let en = pmax.entry(p).or_insert(0.0);
        if t > *en {
            *en = t;
        }
    }
    let mut offs: HashMap<u8, f64> = HashMap::new();
    let mut acc = 0.0;
    for p in 1..=5u8 {
        offs.insert(p, acc);
        acc += pmax.get(&p).copied().unwrap_or(0.0);
    }

    // Players from lineups + Starting XI.
    let mut teams: Vec<Team> = vec![];
    for (tid, key) in [(home_id, "home_team"), (away_id, "away_team")] {
        let tname = meta[key][format!("{key}_name")].as_str().unwrap_or("?").to_string();
        let mut players = vec![];
        if let Some(l) = lineups.as_array().and_then(|a| a.iter().find(|t| t["team_id"].as_i64() == Some(tid))) {
            for p in l["lineup"].as_array().cloned().unwrap_or_default() {
                let positions = p["positions"].as_array().cloned().unwrap_or_default();
                if positions.is_empty() {
                    continue; // unused substitute
                }
                let first = &positions[0];
                let pos = first["position"].as_str().unwrap_or("").to_string();
                let starter = first["start_reason"].as_str() == Some("Starting XI");
                let full = p["player_name"].as_str().unwrap_or("?").to_string();
                let nick = p["player_nickname"].as_str().map(|s| s.to_string());
                let display = nick.clone().unwrap_or(full.clone());
                players.push(Player {
                    id: p["player_id"].as_u64().unwrap_or(0),
                    short: short_name(&display),
                    name: display,
                    shirt: p["jersey_number"].as_u64().unwrap_or(0) as u32,
                    role: Role::from_position(&pos),
                    position: pos,
                    starter,
                    age: None,
                    on_t: if starter { 0.0 } else { f64::NAN },
                    off_t: None,
                    replaced: None,
                });
            }
        }
        let managers = meta[key]["managers"].as_array().and_then(|a| a.first()).and_then(|m| m["nickname"].as_str().or(m["name"].as_str())).map(|s| s.to_string());
        teams.push(Team {
            source_id: format!("sb:{tid}"),
            short_name: tname.clone(),
            name: tname,
            manager: managers,
            players,
            formations: vec![],
            score: meta[format!("{}_score", &key[..4])].as_u64().unwrap_or(0) as u32,
            colour_hints: vec![],
            crest_url: None,
        });
    }

    let mut out = vec![];
    let mut uuid_to_id: HashMap<String, u64> = HashMap::new();
    let mut next_id = 1u64;
    for e in evs {
        let tname = name(&e["type"]);
        let Some(side) = side_of(e["team"]["id"].as_i64().unwrap_or(-1)) else { continue };
        let period = e["period"].as_u64().unwrap_or(1) as u8;
        let t = offs[&period] + ts(e["timestamp"].as_str().unwrap_or("0:0:0"));
        let minute = e["minute"].as_u64().unwrap_or(0) as u32;
        let second = e["second"].as_u64().unwrap_or(0) as u32;
        let player = e["player"]["id"].as_u64();
        let (x, y) = loc(&e["location"]).unwrap_or((52.5, 34.0));
        let id = next_id;
        next_id += 1;
        if let Some(u) = e["id"].as_str() {
            uuid_to_id.insert(u.to_string(), id);
        }
        let mk = |kind: Kind, success: bool| Event {
            id,
            period,
            minute,
            second,
            t,
            side,
            player,
            kind,
            success,
            x,
            y,
            end_x: None,
            end_y: None,
            restart: None,
            pass: None,
            shot: None,
            handling: None,
            inferred: false,
            related: None,
            possession: 0,
            sequence: 0,
            related_player: None,
        };
        let ti = |k: usize| teams_idx(side, k);
        let _ = ti;
        match tname {
            "Starting XI" => {
                if let Some(f) = e["tactics"]["formation"].as_u64() {
                    let f = f.to_string().chars().map(|c| c.to_string()).collect::<Vec<_>>().join("-");
                    teams[side.idx()].formations.push(FormationSpell { name: f, start_t: 0.0, end_t: f64::INFINITY });
                }
            }
            "Tactical Shift" => {
                if let Some(f) = e["tactics"]["formation"].as_u64() {
                    let f = f.to_string().chars().map(|c| c.to_string()).collect::<Vec<_>>().join("-");
                    let tm = &mut teams[side.idx()];
                    if let Some(last) = tm.formations.last_mut() {
                        if last.name == f {
                            continue;
                        }
                        last.end_t = t;
                    }
                    tm.formations.push(FormationSpell { name: f, start_t: t, end_t: f64::INFINITY });
                }
            }
            "Pass" => {
                let p = &e["pass"];
                let outcome = name(&p["outcome"]);
                let mut ev = mk(Kind::Pass, outcome.is_empty());
                let (ex, ey) = loc(&p["end_location"]).unwrap_or((x, y));
                ev.end_x = Some(ex);
                ev.end_y = Some(ey);
                ev.restart = match name(&p["type"]) {
                    "Corner" => Some(Restart::Corner),
                    "Free Kick" => Some(Restart::FreeKick),
                    "Throw-in" => Some(Restart::ThrowIn),
                    "Goal Kick" => Some(Restart::GoalKick),
                    "Kick Off" => Some(Restart::KickOff),
                    _ => None,
                };
                let len = p["length"].as_f64().unwrap_or(0.0) * PITCH_L / 120.0;
                ev.pass = Some(PassInfo {
                    cross: p["cross"].as_bool().unwrap_or(false),
                    through: p["through_ball"].as_bool().unwrap_or(false) || name(&p["technique"]) == "Through Ball",
                    key_pass: p["shot_assist"].as_bool().unwrap_or(false) || p["goal_assist"].as_bool().unwrap_or(false),
                    assist: p["goal_assist"].as_bool().unwrap_or(false),
                    long: len >= 32.0,
                    launch: len >= 40.0 && name(&p["height"]) == "High Pass",
                    head: name(&p["body_part"]) == "Head",
                    cutback: p["cut_back"].as_bool().unwrap_or(false),
                });
                ev.related_player = p["recipient"]["id"].as_u64();
                out.push(ev);
                if p["aerial_won"].as_bool().unwrap_or(false) {
                    out.push(mk(Kind::Aerial, true));
                }
            }
            "Carry" => {
                let mut ev = mk(Kind::Carry, true);
                let (ex, ey) = loc(&e["carry"]["end_location"]).unwrap_or((x, y));
                ev.end_x = Some(ex);
                ev.end_y = Some(ey);
                out.push(ev);
            }
            "Dribble" => out.push(mk(Kind::TakeOn, name(&e["dribble"]["outcome"]) == "Complete")),
            "Dribbled Past" => out.push(mk(Kind::Challenge, false)),
            "Duel" => {
                let dt = name(&e["duel"]["type"]);
                let oc = name(&e["duel"]["outcome"]);
                if dt == "Aerial Lost" {
                    out.push(mk(Kind::Aerial, false));
                } else {
                    out.push(mk(Kind::Tackle, oc == "Won" || oc.starts_with("Success")));
                }
            }
            "Interception" => {
                let oc = name(&e["interception"]["outcome"]);
                out.push(mk(Kind::Interception, oc == "Won" || oc.starts_with("Success")));
            }
            "Ball Recovery" => out.push(mk(Kind::Recovery, !e["ball_recovery"]["recovery_failure"].as_bool().unwrap_or(false))),
            "Clearance" => {
                out.push(mk(Kind::Clearance, true));
                if e["clearance"]["aerial_won"].as_bool().unwrap_or(false) {
                    out.push(mk(Kind::Aerial, true));
                }
            }
            "Block" => out.push(mk(Kind::Block, true)),
            "Foul Committed" => out.push(mk(Kind::Foul, false)),
            "Dispossessed" => out.push(mk(Kind::Dispossessed, false)),
            "Miscontrol" => {
                out.push(mk(Kind::BadTouch, false));
                if e["miscontrol"]["aerial_won"].as_bool().unwrap_or(false) {
                    out.push(mk(Kind::Aerial, true));
                }
            }
            "Error" => out.push(mk(Kind::Error, false)),
            "Bad Behaviour" => out.push(mk(Kind::Card, true)),
            "Substitution" => {
                let mut ev = mk(Kind::SubOff, true);
                ev.related_player = e["substitution"]["replacement"]["id"].as_u64();
                out.push(ev);
            }
            "Goal Keeper" => {
                let gt = name(&e["goalkeeper"]["type"]);
                let oc = name(&e["goalkeeper"]["outcome"]);
                match gt {
                    "Shot Saved" | "Penalty Saved" | "Shot Saved to Post" | "Shot Saved Off Target" | "Save" | "Saved to Post" => {
                        let mut ev = mk(Kind::Save, true);
                        ev.handling = Some(match oc {
                            "In Play Danger" => Handling::ParriedDanger,
                            "In Play Safe" | "Touched Out" | "Clear" | "Touched In" => Handling::ParriedSafe,
                            _ => Handling::Held,
                        });
                        out.push(ev);
                    }
                    "Collected" | "Punch" => out.push(mk(Kind::Claim, oc != "Fail" && !oc.contains("Fail"))),
                    "Keeper Sweeper" | "Smother" => out.push(mk(Kind::Sweeper, !oc.contains("Fail") && oc != "Lost")),
                    _ => {}
                }
            }
            "Shot" => {
                let s = &e["shot"];
                let oc = name(&s["outcome"]);
                let outcome = match oc {
                    "Goal" => ShotOutcome::Goal,
                    "Saved" | "Saved Twice" => ShotOutcome::Saved,
                    "Post" | "Saved to Post" => ShotOutcome::Woodwork,
                    "Blocked" => ShotOutcome::Blocked,
                    _ => ShotOutcome::OffTarget,
                };
                let st = name(&s["type"]);
                let body = match name(&s["body_part"]) {
                    "Head" => Body::Head,
                    "Right Foot" => Body::Right,
                    "Left Foot" => Body::Left,
                    _ => Body::Other,
                };
                let el = s["end_location"].as_array();
                let (gu, gz) = match el {
                    Some(a) if a.len() >= 3 && (a[0].as_f64().unwrap_or(0.0) >= 114.0 || matches!(outcome, ShotOutcome::Goal | ShotOutcome::Saved | ShotOutcome::Woodwork)) => {
                        (Some((40.0 - a[1].as_f64().unwrap_or(40.0)) / 4.0), Some(a[2].as_f64().unwrap_or(0.0) / 2.67))
                    }
                    Some(a) if a.len() >= 2 && matches!(outcome, ShotOutcome::Goal | ShotOutcome::Saved) => (Some((40.0 - a[1].as_f64().unwrap_or(40.0)) / 4.0), None),
                    _ => (None, None),
                };
                let mut ev = mk(Kind::Shot, outcome == ShotOutcome::Goal);
                if let Some((ex, ey)) = loc(&s["end_location"]) {
                    ev.end_x = Some(ex);
                    ev.end_y = Some(ey);
                }
                ev.restart = match st {
                    "Penalty" => Some(Restart::Penalty),
                    "Free Kick" => Some(Restart::FreeKick),
                    "Corner" => Some(Restart::Corner),
                    "Kick Off" => Some(Restart::KickOff),
                    _ => None,
                };
                ev.related = s["key_pass_id"].as_str().and_then(|u| uuid_to_id.get(u).copied());
                ev.shot = Some(ShotInfo {
                    outcome,
                    body,
                    first_time: s["first_time"].as_bool().unwrap_or(false),
                    big_chance: false,
                    penalty: st == "Penalty",
                    own_goal: false,
                    direct_free_kick: st == "Free Kick",
                    fast_break: name(&e["play_pattern"]) == "From Counter",
                    goal_u: gu,
                    goal_z: gz,
                    assist_cross: false,
                    assist_through: false,
                    assist_cutback: false,
                    xg_ext: s["statsbomb_xg"].as_f64(),
                    xgot_ext: None,
                    xg: 0.0,
                    psxg: None,
                });
                if period == 5 {
                    continue; // shootout
                }
                out.push(ev);
                if s["aerial_won"].as_bool().unwrap_or(false) {
                    out.push(mk(Kind::Aerial, true));
                }
            }
            "Own Goal Against" => {
                let mut ev = mk(Kind::Shot, true);
                ev.shot = Some(ShotInfo {
                    outcome: ShotOutcome::Goal,
                    body: Body::Other,
                    first_time: false,
                    big_chance: false,
                    penalty: false,
                    own_goal: true,
                    direct_free_kick: false,
                    fast_break: false,
                    goal_u: None,
                    goal_z: None,
                    assist_cross: false,
                    assist_through: false,
                    assist_cutback: false,
                    xg_ext: None,
                    xgot_ext: None,
                    xg: 0.0,
                    psxg: None,
                });
                out.push(ev);
            }
            _ => {}
        }
    }

    // Substitution times.
    for ev in out.iter().filter(|e| e.kind == Kind::SubOff) {
        let tm = &mut teams[ev.side.idx()];
        if let Some(pid) = ev.player {
            if let Some(p) = tm.players.iter_mut().find(|p| p.id == pid) {
                p.off_t = Some(ev.t);
            }
        }
        if let Some(rid) = ev.related_player {
            if let Some(p) = tm.players.iter_mut().find(|p| p.id == rid) {
                p.on_t = ev.t;
                p.replaced = ev.player;
            }
        }
    }
    // Subs inherit role of the replaced player when SB position is ambiguous.
    for tm in teams.iter_mut() {
        let snapshot: Vec<(u64, Role)> = tm.players.iter().map(|p| (p.id, p.role)).collect();
        for p in tm.players.iter_mut() {
            if !p.starter && p.position.is_empty() {
                if let Some(r) = p.replaced.and_then(|rid| snapshot.iter().find(|x| x.0 == rid)) {
                    p.role = r.1;
                }
            }
        }
        // Final formation spell ends at full time.
        let end = acc;
        for f in tm.formations.iter_mut() {
            if f.end_t.is_infinite() {
                f.end_t = end;
            }
        }
    }

    let teams: [Team; 2] = [teams.remove(0), teams.remove(0)];
    let mut m = Match {
        id: format!("sb-{}", meta["match_id"].as_u64().unwrap_or(0)),
        source: "statsbomb".into(),
        competition: meta["competition"]["competition_name"].as_str().unwrap_or("").to_string(),
        season: meta["season"]["season_name"].as_str().unwrap_or("").to_string(),
        date: meta["match_date"].as_str().unwrap_or("").to_string(),
        teams,
        events: out,
        periods: vec![],
        fallbacks: vec!["player ages unavailable in StatsBomb open data".into()],
    };
    super::finalize(&mut m, true);
    Ok(m)
}

fn teams_idx(s: Side, _k: usize) -> usize {
    s.idx()
}
