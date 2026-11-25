pub mod metrics;
pub mod possession;

pub use crate::sources::in_box;

pub fn in_six(x: f64, y: f64) -> bool {
    x >= 99.5 && (24.84..=43.16).contains(&y)
}
pub fn in_z14(x: f64, y: f64) -> bool {
    (70.0..88.5).contains(&x) && (24.85..=43.15).contains(&y)
}
/// Left half-space (attacker's left = high y), outside the box.
pub fn in_hs_left(x: f64, y: f64) -> bool {
    x >= 60.0 && (43.15..=54.16).contains(&y) && !in_box(x, y)
}
pub fn in_hs_right(x: f64, y: f64) -> bool {
    x >= 60.0 && (13.84..=24.85).contains(&y) && !in_box(x, y)
}
pub fn dist_to_goal(x: f64, y: f64) -> f64 {
    ((105.0 - x).powi(2) + (34.0 - y).powi(2)).sqrt()
}
