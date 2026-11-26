//! Player register lookup from the Reep v0 register (withqwerty/reep, CC0 1.0, frozen 2026-06-21).
//!
//! A trimmed copy (`data/reep_players.tsv.gz`, built by `scripts/trim_reep.py`) is embedded:
//! names and aliases, date of birth, nationality, position and WhoScored id for 373k players.
//! It fills ages the event feed does not carry (StatsBomb open data has none).

use crate::model::{Match, Role, Side};
use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

static DATA: &[u8] = include_bytes!("../data/reep_players.tsv.gz");

pub struct Person {
    pub dob: String,
    pub nationality: String,
    pub position: String,
}

struct Index {
    people: Vec<Person>,
    by_name: HashMap<String, Vec<u32>>,
    by_whoscored: HashMap<u64, u32>,
}

/// Lowercase, accents folded, punctuation dropped, single spaces.
pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        let c = match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'ī' | 'ı' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' | 'ő' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ç' | 'ć' | 'č' => 'c',
            'ñ' | 'ń' | 'ň' => 'n',
            'š' | 'ś' | 'ş' => 's',
            'ž' | 'ź' | 'ż' => 'z',
            'ý' | 'ÿ' => 'y',
            'ł' => 'l',
            'đ' | 'ď' => 'd',
            'ř' => 'r',
            'ğ' => 'g',
            'ß' => 's',
            c => c,
        };
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }
    }
    out.trim_end().to_string()
}

fn index() -> &'static Index {
    static IX: OnceLock<Index> = OnceLock::new();
    IX.get_or_init(|| {
        let mut txt = String::new();
        let _ = flate2::read::GzDecoder::new(DATA).read_to_string(&mut txt);
        let mut ix = Index { people: Vec::with_capacity(380_000), by_name: HashMap::with_capacity(500_000), by_whoscored: HashMap::new() };
        for line in txt.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 5 {
                continue;
            }
            let i = ix.people.len() as u32;
            ix.people.push(Person { dob: f[1].to_string(), nationality: f[2].to_string(), position: f[3].to_string() });
            for n in f[0].split('|') {
                let k = norm(n);
                if !k.is_empty() {
                    let v = ix.by_name.entry(k).or_default();
                    if !v.contains(&i) {
                        v.push(i);
                    }
                }
            }
            if let Ok(w) = f[4].parse::<u64>() {
                ix.by_whoscored.insert(w, i);
            }
        }
        ix
    })
}

/// Years between an ISO birth date and an ISO match date.
pub fn age_at(dob: &str, date: &str) -> Option<f64> {
    let p = |s: &str| -> Option<(i32, u32, u32)> {
        let mut it = s.get(0..10)?.split('-');
        Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?))
    };
    let (by, bm, bd) = p(dob)?;
    let (my, mm, md) = p(date)?;
    let days = |y: i32, m: u32, d: u32| y as f64 * 365.2425 + (m as f64 - 1.0) * 30.437 + d as f64;
    Some((days(my, mm, md) - days(by, bm, bd)) / 365.2425)
}

fn role_word(r: Role) -> &'static str {
    match r {
        Role::Goalkeeper => "goalkeeper",
        Role::Defender => "defender",
        Role::Midfielder => "midfielder",
        Role::Forward => "forward",
    }
}

/// National-team names that differ from the register's nationality field.
fn nation_matches(team: &str, nationality: &str) -> bool {
    let t = norm(team);
    let n = norm(nationality);
    if n.is_empty() {
        return false;
    }
    if t == n || t.contains(&n) || n.contains(&t) {
        return true;
    }
    matches!((t.as_str(), n.as_str()), ("england" | "scotland" | "wales" | "northern ireland", "united kingdom") | ("turkiye", "turkey") | ("czechia", "czech republic") | ("ivory coast", "cote d ivoire") | ("usa" | "united states", "united states of america"))
}

/// Fill missing player ages from the register. Returns (filled, missing before).
pub fn fill_ages(m: &mut Match) -> (usize, usize) {
    let date = m.date.clone();
    let mut want = 0;
    let mut got = 0;
    for side in Side::both() {
        let team_name = m.team(side).name.clone();
        let ix_needed = m.team(side).players.iter().any(|p| p.age.is_none());
        if !ix_needed {
            continue;
        }
        let ix = index();
        for p in m.teams[side.idx()].players.iter_mut().filter(|p| p.age.is_none()) {
            want += 1;
            let mut cands: Vec<u32> = ix.by_name.get(&norm(&p.name)).cloned().unwrap_or_default();
            if cands.is_empty() {
                cands = ix.by_name.get(&norm(&p.short)).cloned().unwrap_or_default();
            }
            // plausible playing age at the match
            cands.retain(|&i| age_at(&ix.people[i as usize].dob, &date).map(|a| (15.0..46.0).contains(&a)).unwrap_or(false));
            if cands.len() > 1 {
                let nat: Vec<u32> = cands.iter().copied().filter(|&i| nation_matches(&team_name, &ix.people[i as usize].nationality)).collect();
                if !nat.is_empty() {
                    cands = nat;
                }
            }
            if cands.len() > 1 {
                let role = role_word(p.role);
                let pos: Vec<u32> = cands.iter().copied().filter(|&i| ix.people[i as usize].position.contains(role)).collect();
                if !pos.is_empty() {
                    cands = pos;
                }
            }
            if cands.len() == 1 {
                if let Some(a) = age_at(&ix.people[cands[0] as usize].dob, &date) {
                    p.age = Some((a * 10.0).round() / 10.0);
                    got += 1;
                }
            }
        }
    }
    (got, want)
}

/// Register entry for a WhoScored player id.
pub fn by_whoscored(id: u64) -> Option<&'static Person> {
    let ix = index();
    ix.by_whoscored.get(&id).map(|&i| &ix.people[i as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_aliases_and_ages() {
        assert_eq!(norm("Martín  Zubimendi"), "martin zubimendi");
        let ix = index();
        assert!(ix.people.len() > 300_000);
        let c = ix.by_name.get("daniel carvajal").expect("alias of Dani Carvajal");
        assert!(c.iter().any(|&i| ix.people[i as usize].dob == "1992-01-11"));
        let a = age_at("1992-01-11", "2024-07-14").unwrap();
        assert!((a - 32.5).abs() < 0.1);
    }
}
