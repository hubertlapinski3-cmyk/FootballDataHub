//! Minimal L2-regularised logistic regression with feature standardisation.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Logistic {
    pub names: Vec<String>,
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
    pub w: Vec<f64>,
    pub b: f64,
    pub n_train: usize,
    pub log_loss: f64,
    pub base_rate: f64,
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

impl Logistic {
    pub fn fit(names: &[&str], x: &[Vec<f64>], y: &[f64], l2: f64, iters: usize) -> Logistic {
        let n = x.len();
        let d = names.len();
        let mut mean = vec![0.0; d];
        let mut std = vec![0.0; d];
        for r in x {
            for j in 0..d {
                mean[j] += r[j];
            }
        }
        for m in mean.iter_mut() {
            *m /= n.max(1) as f64;
        }
        for r in x {
            for j in 0..d {
                std[j] += (r[j] - mean[j]).powi(2);
            }
        }
        for s in std.iter_mut() {
            *s = (*s / n.max(1) as f64).sqrt();
            if *s < 1e-9 {
                *s = 1.0;
            }
        }
        let xs: Vec<Vec<f64>> = x.iter().map(|r| (0..d).map(|j| (r[j] - mean[j]) / std[j]).collect()).collect();
        let base = y.iter().sum::<f64>() / n.max(1) as f64;
        let mut w = vec![0.0; d];
        let mut b = (base.clamp(1e-4, 1.0 - 1e-4) / (1.0 - base.clamp(1e-4, 1.0 - 1e-4))).ln();
        // Adam
        let (mut mw, mut vw) = (vec![0.0; d], vec![0.0; d]);
        let (mut mb, mut vb) = (0.0, 0.0);
        let lr = 0.05;
        for it in 1..=iters {
            let mut gw = vec![0.0; d];
            let mut gb = 0.0;
            for (r, &t) in xs.iter().zip(y) {
                let z = b + r.iter().zip(&w).map(|(a, c)| a * c).sum::<f64>();
                let e = sigmoid(z) - t;
                for j in 0..d {
                    gw[j] += e * r[j];
                }
                gb += e;
            }
            for j in 0..d {
                gw[j] = gw[j] / n as f64 + l2 * w[j];
                mw[j] = 0.9 * mw[j] + 0.1 * gw[j];
                vw[j] = 0.999 * vw[j] + 0.001 * gw[j] * gw[j];
                let mh = mw[j] / (1.0 - 0.9f64.powi(it as i32));
                let vh = vw[j] / (1.0 - 0.999f64.powi(it as i32));
                w[j] -= lr * mh / (vh.sqrt() + 1e-8);
            }
            gb /= n as f64;
            mb = 0.9 * mb + 0.1 * gb;
            vb = 0.999 * vb + 0.001 * gb * gb;
            b -= lr * (mb / (1.0 - 0.9f64.powi(it as i32))) / ((vb / (1.0 - 0.999f64.powi(it as i32))).sqrt() + 1e-8);
        }
        let mut model = Logistic { names: names.iter().map(|s| s.to_string()).collect(), mean, std, w, b, n_train: n, log_loss: 0.0, base_rate: base };
        let ll: f64 = x.iter().zip(y).map(|(r, &t)| {
            let p = model.predict(r).clamp(1e-9, 1.0 - 1e-9);
            -(t * p.ln() + (1.0 - t) * (1.0 - p).ln())
        }).sum::<f64>() / n.max(1) as f64;
        model.log_loss = ll;
        model
    }

    pub fn predict(&self, r: &[f64]) -> f64 {
        if self.w.is_empty() {
            return self.base_rate;
        }
        let z = self.b + (0..self.w.len()).map(|j| (r[j] - self.mean[j]) / self.std[j] * self.w[j]).sum::<f64>();
        sigmoid(z)
    }
}
