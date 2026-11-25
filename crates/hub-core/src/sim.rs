//! Monte Carlo shot replay, expected points and the live win-probability curve.

use crate::model::*;
use serde::Serialize;

pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E3779B97F4A7C15)
    }
    pub fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WinSim {
    pub p_home: f64,
    pub p_draw: f64,
    pub p_away: f64,
    pub xpts_home: f64,
    pub xpts_away: f64,
    pub sims: usize,
}

pub fn simulate(m: &Match, sims: usize) -> WinSim {
    // shots grouped per possession: at most one goal per possession
    let mut groups: Vec<(Side, u32, Vec<f64>)> = vec![];
    for e in &m.events {
        if let Some(s) = &e.shot {
            if s.own_goal || e.period > 4 {
                continue;
            }
            if let Some(g) = groups.last_mut() {
                if g.0 == e.side && g.1 == e.possession {
                    g.2.push(s.xg);
                    continue;
                }
            }
            groups.push((e.side, e.possession, vec![s.xg]));
        }
    }
    let groups: Vec<(Side, Vec<f64>)> = groups.into_iter().map(|g| (g.0, g.2)).collect();
    let og = [
        m.events.iter().filter(|e| e.shot.as_ref().map(|s| s.own_goal).unwrap_or(false) && e.side == Side::Away).count() as i32,
        m.events.iter().filter(|e| e.shot.as_ref().map(|s| s.own_goal).unwrap_or(false) && e.side == Side::Home).count() as i32,
    ];
    let mut rng = Rng::new(42);
    let (mut h, mut d, mut a) = (0usize, 0usize, 0usize);
    for _ in 0..sims {
        let mut g = og;
        for (side, xs) in &groups {
            for &x in xs {
                if rng.next_f64() < x {
                    g[side.idx()] += 1;
                    break;
                }
            }
        }
        if g[0] > g[1] {
            h += 1
        } else if g[0] == g[1] {
            d += 1
        } else {
            a += 1
        }
    }
    let n = sims as f64;
    let (ph, pd, pa) = (h as f64 / n, d as f64 / n, a as f64 / n);
    WinSim { p_home: ph, p_draw: pd, p_away: pa, xpts_home: 3.0 * ph + pd, xpts_away: 3.0 * pa + pd, sims }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LiveCurve {
    pub minutes: Vec<f64>,
    pub home: Vec<f64>,
    pub draw: Vec<f64>,
    pub away: Vec<f64>,
    pub likelier_home: f64,
    pub likelier_away: f64,
    pub likelier_draw: f64,
}

fn poisson(l: f64, k: usize) -> f64 {
    let mut p = (-l).exp();
    for i in 1..=k {
        p *= l / i as f64;
    }
    p
}

/// Transparent heuristic: actual score at t + Poisson goals for the remaining time,
/// at each team's xG rate so far shrunk toward a league-average prior.
pub fn live_curve(m: &Match, step_min: f64) -> LiveCurve {
    let total = (m.duration() / 60.0).max(90.0);
    let prior = 1.35 / 90.0;
    let k = 20.0;
    let mut c = LiveCurve::default();
    let mut t = 0.0;
    while t <= total + 1e-9 {
        let mut score = [0i32; 2];
        let mut xg = [0.0; 2];
        for e in &m.events {
            if e.t / 60.0 > t || e.period > 4 {
                continue;
            }
            if let Some(s) = &e.shot {
                let side = if s.own_goal { e.side.other() } else { e.side };
                if s.outcome == ShotOutcome::Goal {
                    score[side.idx()] += 1;
                }
                if !s.own_goal {
                    xg[e.side.idx()] += s.xg;
                }
            }
        }
        let rem = (total - t).max(0.0);
        let lam: Vec<f64> = (0..2).map(|i| ((xg[i] + prior * k) / (t + k)) * rem).collect();
        let (mut ph, mut pd, mut pa) = (0.0, 0.0, 0.0);
        for i in 0..12 {
            for j in 0..12 {
                let p = poisson(lam[0], i) * poisson(lam[1], j);
                let (fh, fa) = (score[0] + i as i32, score[1] + j as i32);
                if fh > fa {
                    ph += p
                } else if fh == fa {
                    pd += p
                } else {
                    pa += p
                }
            }
        }
        let s = ph + pd + pa;
        c.minutes.push(t);
        c.home.push(ph / s);
        c.draw.push(pd / s);
        c.away.push(pa / s);
        t += step_min;
    }
    let n = c.minutes.len() as f64;
    for i in 0..c.minutes.len() {
        let (h, d, a) = (c.home[i], c.draw[i], c.away[i]);
        if h >= d && h >= a {
            c.likelier_home += 1.0
        } else if a >= h && a >= d {
            c.likelier_away += 1.0
        } else {
            c.likelier_draw += 1.0
        }
    }
    c.likelier_home /= n;
    c.likelier_away /= n;
    c.likelier_draw /= n;
    c
}
