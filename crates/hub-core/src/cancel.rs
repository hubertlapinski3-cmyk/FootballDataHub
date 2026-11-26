//! Cooperative cancellation for a running scrape or build. The app sets the flag from its Cancel
//! button; long-running steps (page fetch, relay polling, browser fallback) check it between steps.

use std::sync::atomic::{AtomicBool, Ordering};

static FLAG: AtomicBool = AtomicBool::new(false);

/// Clear the flag at the start of a job.
pub fn reset() {
    FLAG.store(false, Ordering::SeqCst);
}

/// Ask the running job to stop.
pub fn request() {
    FLAG.store(true, Ordering::SeqCst);
}

pub fn requested() -> bool {
    FLAG.load(Ordering::SeqCst)
}

/// `Err("Cancelled")` when a stop was requested.
pub fn check() -> anyhow::Result<()> {
    if requested() {
        Err(anyhow::anyhow!("Cancelled"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_round_trip() {
        reset();
        assert!(!requested());
        assert!(check().is_ok());
        request();
        assert!(requested());
        assert_eq!(check().unwrap_err().to_string(), "Cancelled");
        reset();
        assert!(check().is_ok());
    }
}
