//! The background scrape job: runs off the UI thread, reports progress, finishes with a saved match,
//! and a cancel stops a hung request promptly.

use hub_app::library::{self, BuildState, Source};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hub-core/tests/fixtures").join(name)
}

fn wait(state: &Arc<Mutex<BuildState>>, secs: u64) -> BuildState {
    let t0 = Instant::now();
    loop {
        let s = state.lock().unwrap().clone();
        if !s.running {
            return s;
        }
        assert!(t0.elapsed() < Duration::from_secs(secs), "job still running: {}", s.step);
        std::thread::sleep(Duration::from_millis(50));
    }
}

// The two jobs share the process-wide cancel flag, so they run one after the other in one test.
#[test]
fn job_runs_in_background_and_cancel_stops_a_hung_request() {
    let dir = std::env::temp_dir().join(format!("datahub_scrape_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ctx = egui::Context::default();

    // ---- a saved page goes through the whole pipeline on a worker thread
    let state: Arc<Mutex<BuildState>> = Default::default();
    library::spawn_build(dir.clone(), Source::WhoScored(fixture("whoscored_match.html").display().to_string()), false, None, None, state.clone(), ctx.clone());
    {
        // spawn returns at once with the job marked running
        let s = state.lock().unwrap();
        assert!(s.running || s.done.is_some() || s.error.is_some());
    }
    let s = wait(&state, 120);
    assert!(s.error.is_none(), "build failed: {:?}", s.error);
    assert_eq!(s.done.as_deref(), Some("ws-2029131"));
    assert_eq!(s.step, "done");
    assert!(s.log.iter().any(|l| l.starts_with("ingest")), "{:?}", s.log);
    assert!(dir.join("matches/ws-2029131/summary.json").exists());

    // ---- a server that never answers: Cancel returns the UI in well under the 30 s request timeout
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut held = vec![];
        for s in l.incoming().flatten() {
            held.push(s); // accept and stay silent
        }
    });
    // the job's page fetch is `get_page_cancellable`; exercise it against the silent server
    let t0 = Instant::now();
    matchhub::cancel::reset();
    let h = std::thread::spawn(move || matchhub::net::get_page_cancellable(&format!("http://{addr}/hang"), &[], 30));
    std::thread::sleep(Duration::from_millis(600));
    matchhub::cancel::request();
    let r = h.join().unwrap();
    assert!(r.is_err() && r.err().unwrap().to_string() == "Cancelled");
    assert!(t0.elapsed() < Duration::from_secs(5), "cancel took {:?}", t0.elapsed());
    matchhub::cancel::reset();
}
