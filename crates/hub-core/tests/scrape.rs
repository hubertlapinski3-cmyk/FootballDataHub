//! Scraper tests: parsers against saved fixtures, error classification, link formats, and the HTTP
//! layer against a local server (headers sent, gzip, redirects, 403 / 429 / 503 handling).

use matchhub::net;
use matchhub::scrape::{self, MatchLink, PageKind, ScrapeError};
use matchhub::sources::whoscored;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn fixture(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

// ---------------------------------------------------------------- match pages

#[test]
fn whoscored_page_is_classified_and_parsed() {
    let html = fixture("whoscored_match.html");
    assert_eq!(scrape::classify(&html), PageKind::WhoScoredMatch);
    scrape::require_match_page(&html, "www.whoscored.com", Some(200)).expect("a match page");

    let blob = whoscored::extract_blob(&html).expect("blob");
    let v: serde_json::Value = serde_json::from_str(&blob).expect("blob is JSON");
    assert_eq!(v["home"]["name"], "Arsenal");
    assert_eq!(v["events"].as_array().unwrap().len(), 12);

    let cache = std::env::temp_dir().join("matchhub_test_cache");
    let m = whoscored::load(fixture_path("whoscored_match.html").to_str().unwrap(), &cache, None, None, &mut |_| {}).expect("load saved page");
    assert_eq!(m.team(matchhub::model::Side::Home).name, "Arsenal");
    assert_eq!(m.team(matchhub::model::Side::Away).name, "Borussia Dortmund");
    assert_eq!(m.team(matchhub::model::Side::Home).score, 2);
    assert_eq!(m.competition, "Champions League");
    assert_eq!(m.season, "2026/2027");
    assert_eq!(m.date, "2026-11-25");
    let goals = m.events.iter().filter(|e| e.shot.as_ref().map(|s| s.outcome == matchhub::model::ShotOutcome::Goal).unwrap_or(false)).count();
    assert_eq!(goals, 3);
    assert_eq!(m.id, "ws-2029131");
}

#[test]
fn saved_json_blob_loads_too() {
    let cache = std::env::temp_dir().join("matchhub_test_cache");
    let m = whoscored::load(fixture_path("whoscored_match.json").to_str().unwrap(), &cache, None, None, &mut |_| {}).expect("load saved json");
    assert_eq!(m.team(matchhub::model::Side::Away).name, "Borussia Dortmund");
    assert!(m.events.len() >= 12);
}

#[test]
fn gzipped_blob_loads_too() {
    let raw = fixture("whoscored_match.json");
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(raw.as_bytes()).unwrap();
    let path = std::env::temp_dir().join("matchhub_test_2029131.json.gz");
    std::fs::write(&path, gz.finish().unwrap()).unwrap();
    let cache = std::env::temp_dir().join("matchhub_test_cache");
    let m = whoscored::load(path.to_str().unwrap(), &cache, None, None, &mut |_| {}).expect("load gz");
    assert_eq!(m.team(matchhub::model::Side::Home).name, "Arsenal");
}

// ---------------------------------------------------------------- bot walls and bad pages

#[test]
fn cloudflare_block_page_is_a_wall_not_a_match() {
    let html = fixture("cloudflare_403.html");
    assert_eq!(scrape::classify(&html), PageKind::BotWall("Cloudflare".into()));
    let err = scrape::status_error("www.whoscored.com", 403, None, &html).expect("403 is an error");
    assert_eq!(err, ScrapeError::Blocked { host: "www.whoscored.com".into(), vendor: "Cloudflare".into(), status: Some(403) });
    let text = err.to_string();
    assert!(text.contains("blocked") && text.contains("Cloudflare") && text.contains("HTTP 403") && text.contains("JavaScript"), "{text}");

    // extraction on the wall reports "blocked", which is what triggers the browser fallback
    let e = whoscored::extract_blob(&html).unwrap_err();
    assert!(scrape::is_blocked(&e), "{e:#}");
}

#[test]
fn javascript_challenge_page_is_recognised() {
    assert_eq!(scrape::classify(&fixture("cloudflare_challenge.html")), PageKind::BotWall("Cloudflare".into()));
}

#[test]
fn rate_limit_message_has_the_wait_time() {
    let e = scrape::status_error("www.whoscored.com", 429, Some(90), "").unwrap();
    assert_eq!(e, ScrapeError::RateLimited { host: "www.whoscored.com".into(), retry_after: Some(90) });
    assert!(e.to_string().contains("90 seconds"), "{e}");
    assert!(scrape::status_error("h", 429, None, "").unwrap().to_string().contains("few minutes"));
}

#[test]
fn statuses_map_to_readable_errors() {
    assert!(scrape::status_error("h", 200, None, "").is_none());
    assert!(scrape::status_error("h", 204, None, "").is_none());
    assert!(matches!(scrape::status_error("h", 403, None, "plain"), Some(ScrapeError::Blocked { .. })));
    assert!(matches!(scrape::status_error("h", 503, None, "overloaded"), Some(ScrapeError::Http { status: 503, .. })));
    assert!(matches!(scrape::status_error("h", 503, None, "<title>Just a moment...</title>"), Some(ScrapeError::Blocked { .. })));
    assert!(net::describe_status("https://x.test/p", 404, None).contains("no page at this link"));
    assert!(net::describe_status("https://x.test/p", 500, None).contains("server problem"));
}

#[test]
fn malformed_and_foreign_pages_fail_cleanly() {
    // cut off in the middle of the data
    let cut = fixture("whoscored_truncated.html");
    let e = whoscored::extract_blob(&cut).unwrap_err();
    assert!(e.to_string().contains("unterminated"), "{e:#}");
    // no match data at all
    let other = "<html><head><title>Home</title></head><body><p>Hello</p></body></html>";
    assert_eq!(scrape::classify(other), PageKind::Other);
    assert_eq!(scrape::require_match_page(other, "x.test", Some(200)), Err(ScrapeError::NoMatchData { host: "x.test".into() }));
    assert!(whoscored::extract_blob(other).is_err());
    // empty input
    assert!(whoscored::extract_blob("").is_err());
    assert!(scrape::json_blobs("").is_empty());
    assert!(scrape::tables("").is_empty());
    // a saved file that is neither JSON nor a match page
    let junk = std::env::temp_dir().join("matchhub_test_junk.html");
    std::fs::write(&junk, other).unwrap();
    let cache = std::env::temp_dir().join("matchhub_test_cache");
    assert!(whoscored::load(junk.to_str().unwrap(), &cache, None, None, &mut |_| {}).is_err());
}

// ---------------------------------------------------------------- embedded state and tables

#[test]
fn embedded_json_blobs_are_found() {
    let blobs = scrape::json_blobs(&fixture("next_data.html"));
    let get = |l: &str| blobs.iter().find(|b| b.label == l).unwrap_or_else(|| panic!("missing {l} in {:?}", blobs.iter().map(|b| &b.label).collect::<Vec<_>>()));
    assert_eq!(get("__NEXT_DATA__").value["props"]["pageProps"]["general"]["homeTeam"]["name"], "Arsenal");
    assert_eq!(get("ld+json").value["@type"], "SportsEvent");
    // braces and quotes inside strings must not confuse the matcher
    assert_eq!(get("window.__NUXT__").value["state"]["count"], 3);
    assert!(get("window.__NUXT__").value["state"]["label"].as_str().unwrap().contains("a } brace"));
    assert_eq!(get("window.initialState").value["a"][2]["b"], "c");
    // no duplicate of the Next data blob
    assert_eq!(blobs.iter().filter(|b| b.label.contains("NEXT_DATA")).count(), 1);

    let ws = scrape::json_blobs(&fixture("whoscored_match.html"));
    assert!(ws.iter().any(|b| b.label == "matchCentreData" && b.value["away"]["name"] == "Borussia Dortmund"));
}

#[test]
fn html_tables_become_rows() {
    let t = scrape::tables(&fixture("table.html"));
    assert_eq!(t.len(), 2, "empty tables are skipped: {t:?}");
    assert_eq!(t[0].caption, "Team stats");
    assert_eq!(t[0].headers, ["Team", "Shots", "xG"]);
    assert_eq!(t[0].rows[0], ["Arsenal", "16", "1.79"]);
    assert_eq!(t[0].rows[1][0], "Borussia Dortmund", "whitespace and line breaks are normalised");
    assert!(t[1].headers.is_empty());
    assert_eq!(t[1].rows[0], ["no header", "x"]);
    assert!(scrape::title(&fixture("next_data.html")).contains("FotMob"));
}

// ---------------------------------------------------------------- link formats

#[test]
fn match_links_in_every_supported_shape() {
    let id = |s: &str| MatchLink::parse(s).map(|l| (l.id, l.host));
    assert_eq!(id("2029131"), Some(("2029131".into(), "www.whoscored.com".into())));
    assert_eq!(id("  2029131 "), Some(("2029131".into(), "www.whoscored.com".into())));
    for url in [
        "https://fr.whoscored.com/matches/2029131/live/europe-champions-league-2026-2027-arsenal-borussia-dortmund",
        "https://fr.whoscored.com/matches/2029131/show/europe-champions-league-2026-2027-arsenal-borussia-dortmund",
        "https://fr.whoscored.com/Matches/2029131/Live",
        "http://fr.whoscored.com/matches/2029131/matchreport/x",
        "fr.whoscored.com/matches/2029131/preview/x",
        "https://fr.whoscored.com/en/matches/2029131/live/x",
    ] {
        assert_eq!(id(url), Some(("2029131".into(), "fr.whoscored.com".into())), "{url}");
    }
    assert_eq!(id("https://www.whoscored.com/Matches/1650630/Live").unwrap().1, "www.whoscored.com");
    assert_eq!(MatchLink::parse("https://fr.whoscored.com/matches/2029131/live/x").unwrap().page_url(), "https://fr.whoscored.com/Matches/2029131/Live");
    // not WhoScored, not a match
    assert_eq!(id("https://www.example.com/matches/1"), None);
    assert_eq!(id("https://evil-whoscored.com.example.org/matches/1"), None);
    assert_eq!(id("https://www.whoscored.com/Teams/167"), None);
    assert_eq!(id(""), None);
    assert_eq!(id("abc"), None);
    assert_eq!(whoscored::match_id_from("https://fr.whoscored.com/matches/2029131/live/x"), Some("2029131".into()));
}

// ---------------------------------------------------------------- HTTP layer against a local server

struct Served {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

/// One-shot server: answers each incoming connection with the next canned response.
fn serve(responses: Vec<Vec<u8>>) -> Served {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(vec![]));
    let log = requests.clone();
    std::thread::spawn(move || {
        for resp in responses {
            let Ok((mut s, _)) = listener.accept() else { return };
            let mut buf = vec![0u8; 8192];
            let n = s.read(&mut buf).unwrap_or(0);
            log.lock().unwrap().push(String::from_utf8_lossy(&buf[..n]).into_owned());
            let _ = s.write_all(&resp);
            let _ = s.flush();
        }
    });
    Served { base, requests }
}

fn http(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len()).into_bytes();
    for (k, v) in headers {
        out.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out
}

#[test]
fn page_requests_carry_browser_headers() {
    let srv = serve(vec![http("200 OK", &[("Content-Type", "text/html")], b"<html>ok</html>")]);
    let r = net::get_page(&format!("{}/Matches/1/Live", srv.base), &[("Referer", "https://www.whoscored.com/")], 10).unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.body, b"<html>ok</html>");
    let req = srv.requests.lock().unwrap()[0].to_lowercase();
    assert!(req.contains("user-agent: mozilla/5.0"), "{req}");
    assert!(req.contains("accept: text/html"), "{req}");
    assert!(req.contains("accept-language: en-gb"), "{req}");
    assert!(req.contains("referer: https://www.whoscored.com/"), "{req}");
    assert!(req.contains("sec-fetch-mode: navigate"), "{req}");
    assert!(req.contains("accept-encoding:") && req.contains("gzip"), "compression is negotiated: {req}");
}

#[test]
fn gzip_bodies_are_decoded() {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(b"<html>compressed page</html>").unwrap();
    let body = gz.finish().unwrap();
    let srv = serve(vec![http("200 OK", &[("Content-Encoding", "gzip")], &body)]);
    let r = net::get_page(&format!("{}/x", srv.base), &[], 10).unwrap();
    assert_eq!(String::from_utf8_lossy(&r.body), "<html>compressed page</html>");
}

#[test]
fn redirects_are_followed() {
    let srv = serve(vec![http("301 Moved Permanently", &[("Location", "/final")], b""), http("200 OK", &[], b"landed")]);
    let r = net::get_page(&format!("{}/start", srv.base), &[], 10).unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.body, b"landed");
    assert!(r.final_url.ends_with("/final"), "{}", r.final_url);
    assert_eq!(srv.requests.lock().unwrap().len(), 2);
}

#[test]
fn a_403_wall_is_returned_not_retried() {
    let srv = serve(vec![http("403 Forbidden", &[("Content-Type", "text/html")], fixture("cloudflare_403.html").as_bytes())]);
    let r = net::get_page(&format!("{}/Matches/1/Live", srv.base), &[], 10).unwrap();
    assert_eq!(r.status, 403);
    let body = String::from_utf8_lossy(&r.body).into_owned();
    assert!(matches!(scrape::status_error("127.0.0.1", r.status, r.retry_after, &body), Some(ScrapeError::Blocked { vendor, .. }) if vendor == "Cloudflare"));
    assert_eq!(srv.requests.lock().unwrap().len(), 1, "a block is never hammered");
}

#[test]
fn a_429_carries_retry_after_and_is_not_retried() {
    let srv = serve(vec![http("429 Too Many Requests", &[("Retry-After", "42")], b"slow down")]);
    let r = net::get_page(&format!("{}/x", srv.base), &[], 10).unwrap();
    assert_eq!(r.status, 429);
    assert_eq!(r.retry_after, Some(42));
    assert!(net::describe_status(&format!("{}/x", srv.base), 429, r.retry_after).contains("42 seconds"));
    assert_eq!(srv.requests.lock().unwrap().len(), 1);
}

#[test]
fn a_503_is_retried_once() {
    let srv = serve(vec![http("503 Service Unavailable", &[("Retry-After", "0")], b"busy"), http("200 OK", &[], b"fine")]);
    let r = net::get_page(&format!("{}/x", srv.base), &[], 10).unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.body, b"fine");
    assert_eq!(srv.requests.lock().unwrap().len(), 2);
}

#[test]
fn transport_failures_are_explained() {
    // nothing listens on this port
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    drop(l);
    let e = net::get_page(&format!("http://{addr}/x"), &[], 5).err().expect("connection refused");
    let text = format!("{e:#}");
    assert!(text.contains("Could not reach") && text.contains(&addr.to_string()), "{text}");
    assert!(net::describe_transport("https://www.whoscored.com/x", "dns error: failed to lookup address").contains("could not be found"));
    assert!(net::describe_transport("https://www.whoscored.com/x", "timed out reading response").contains("too long"));
}

#[test]
fn get_ok_turns_http_errors_into_readable_messages() {
    let srv = serve(vec![http("404 Not Found", &[], b"nope")]);
    let e = net::get_ok(&format!("{}/missing", srv.base), &[], 10).unwrap_err();
    assert!(e.to_string().contains("no page at this link"), "{e}");
}

#[test]
fn brotli_bodies_are_decoded() {
    // an uncompressed-meta-block brotli stream holding "hello"
    let body = [0x0b, 0x02, 0x80, b'h', b'e', b'l', b'l', b'o', 0x03];
    let srv = serve(vec![http("200 OK", &[("Content-Encoding", "br")], &body)]);
    let r = net::get_page(&format!("{}/x", srv.base), &[], 10).unwrap();
    assert_eq!(String::from_utf8_lossy(&r.body), "hello");
    let req = srv.requests.lock().unwrap()[0].to_lowercase();
    assert!(req.contains("br"), "brotli is offered: {req}");
}
