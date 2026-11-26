//! HTTP layer: ureq with rustls (ring) and webpki roots, gzip and brotli. Pure Rust apart from
//! `ring`'s C/asm, which the Android NDK clang builds, so no OpenSSL and no JVM certificate glue.
//!
//! Every request goes out with browser-like headers; redirects are followed (up to 8); status codes
//! are returned, never turned into errors, so callers can explain a 403 or 429 properly.

use anyhow::{anyhow, Result};
use std::io::Read;
use std::time::Duration;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

/// Browser-style defaults for page requests (callers may override any of them).
const PAGE_HEADERS: &[(&str, &str)] = &[
    ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    ("Accept-Language", "en-GB,en;q=0.9"),
    ("Upgrade-Insecure-Requests", "1"),
    ("Sec-Fetch-Dest", "document"),
    ("Sec-Fetch-Mode", "navigate"),
    ("Sec-Fetch-Site", "none"),
];

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(timeout))).http_status_as_error(false).max_redirects(8).build().into()
}

/// A response: status, selected headers and the (decompressed) body.
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    pub retry_after: Option<u64>,
    pub final_url: String,
}

fn send(url: &str, headers: &[(&str, &str)], timeout: u64, page: bool) -> Result<Response> {
    let mut req = agent(timeout).get(url).header("User-Agent", UA);
    let given = |k: &str| headers.iter().any(|(h, _)| h.eq_ignore_ascii_case(k));
    if page {
        for (k, v) in PAGE_HEADERS {
            if !given(k) {
                req = req.header(*k, *v);
            }
        }
    }
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut resp = req.call().map_err(|e| anyhow!("{}", describe_transport(url, &e.to_string())))?;
    let status = resp.status().as_u16();
    let retry_after = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok());
    use ureq::ResponseExt;
    let final_url = resp.get_uri().to_string();
    let mut body = vec![];
    resp.body_mut().with_config().limit(64 * 1024 * 1024).reader().read_to_end(&mut body)?;
    Ok(Response { status, body, retry_after, final_url })
}

/// Plain-language reason for a failed connection.
pub fn describe_transport(url: &str, err: &str) -> String {
    let host = host_of(url);
    let low = err.to_lowercase();
    let why = if low.contains("dns") || low.contains("resolve") || low.contains("lookup") || low.contains("name or service") {
        "the address could not be found: check the connection or the link"
    } else if low.contains("timed out") || low.contains("timeout") {
        "the server took too long to answer"
    } else if low.contains("refused") || low.contains("unreachable") || low.contains("network") {
        "the network is not reachable: check the connection"
    } else if low.contains("certificate") || low.contains("tls") || low.contains("handshake") {
        "a secure connection could not be set up (TLS)"
    } else {
        "the request failed"
    };
    format!("Could not reach {host}: {why} ({err})")
}

pub fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split(['/', '?', '#']).next().unwrap_or("").to_string()
}

/// Explains a non-2xx status in words a user can act on.
pub fn describe_status(url: &str, status: u16, retry_after: Option<u64>) -> String {
    let host = host_of(url);
    match status {
        401 => format!("{host} wants a login (HTTP 401)."),
        403 => format!("{host} blocked the request (HTTP 403): it treats the app as a bot. Wait a while before trying again; repeated requests prolong the block."),
        404 => format!("{host} has no page at this link (HTTP 404): check the link or the match id."),
        429 => match retry_after {
            Some(s) => format!("{host} says too many requests (HTTP 429). Try again in {s} seconds."),
            None => format!("{host} says too many requests (HTTP 429). Wait a few minutes before trying again."),
        },
        500..=599 => format!("{host} had a server problem (HTTP {status}). Try again in a minute."),
        _ => format!("{host} answered HTTP {status} for {url}"),
    }
}

/// GET returning (status, body bytes). No browser headers: for APIs.
pub fn get(url: &str, headers: &[(&str, &str)], timeout: u64) -> Result<(u16, Vec<u8>)> {
    let r = send(url, headers, timeout, false)?;
    Ok((r.status, r.body))
}

/// GET of an HTML page with browser headers. One polite retry after a 502/503/504; never on 403 or 429,
/// which would only prolong a block.
pub fn get_page(url: &str, headers: &[(&str, &str)], timeout: u64) -> Result<Response> {
    let mut r = send(url, headers, timeout, true)?;
    if matches!(r.status, 502..=504) && !crate::cancel::requested() {
        std::thread::sleep(Duration::from_secs(r.retry_after.unwrap_or(2).min(5)));
        r = send(url, headers, timeout, true)?;
    }
    Ok(r)
}

/// `get_page` that gives up promptly when the job is cancelled: the request runs on a helper thread
/// and is polled every 200 ms. An abandoned request ends by itself at its own timeout.
pub fn get_page_cancellable(url: &str, headers: &[(&str, &str)], timeout: u64) -> Result<Response> {
    let (url, headers): (String, Vec<(String, String)>) = (url.to_string(), headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let h: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let _ = tx.send(get_page(&url, &h, timeout));
    });
    loop {
        crate::cancel::check()?;
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(r) => return r,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Err(anyhow!("the request thread ended unexpectedly")),
        }
    }
}

pub fn get_ok(url: &str, headers: &[(&str, &str)], timeout: u64) -> Result<Vec<u8>> {
    let r = send(url, headers, timeout, false)?;
    if !(200..300).contains(&r.status) {
        return Err(anyhow!("{}", describe_status(url, r.status, r.retry_after)));
    }
    Ok(r.body)
}

pub fn get_text(url: &str, headers: &[(&str, &str)], timeout: u64) -> Result<String> {
    Ok(String::from_utf8_lossy(&get_ok(url, headers, timeout)?).into_owned())
}

/// POST a JSON body; returns (status, body).
pub fn post_json(url: &str, headers: &[(&str, &str)], body: &str, timeout: u64) -> Result<(u16, Vec<u8>)> {
    let mut req = agent(timeout).post(url).header("User-Agent", UA).header("Content-Type", "application/json");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut resp = req.send(body).map_err(|e| anyhow!("{}", describe_transport(url, &e.to_string())))?;
    let status = resp.status().as_u16();
    let mut buf = vec![];
    resp.body_mut().with_config().limit(8 * 1024 * 1024).reader().read_to_end(&mut buf)?;
    Ok((status, buf))
}

pub fn gunzip_if_needed(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut s = String::new();
        flate2::read::GzDecoder::new(bytes).read_to_string(&mut s)?;
        Ok(s)
    } else {
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}
