//! Minimal HTTP layer (ureq + rustls + webpki roots): works the same on
//! desktop and Android, no JVM certificate glue required.

use anyhow::{anyhow, Result};
use std::io::Read;
use std::time::Duration;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(timeout))).http_status_as_error(false).build().into()
}

/// GET returning (status, body bytes).
pub fn get(url: &str, headers: &[(&str, &str)], timeout: u64) -> Result<(u16, Vec<u8>)> {
    let mut req = agent(timeout).get(url).header("User-Agent", UA);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut resp = req.call().map_err(|e| anyhow!("GET {url}: {e}"))?;
    let status = resp.status().as_u16();
    let mut buf = vec![];
    resp.body_mut().with_config().limit(64 * 1024 * 1024).reader().read_to_end(&mut buf)?;
    Ok((status, buf))
}

pub fn get_ok(url: &str, headers: &[(&str, &str)], timeout: u64) -> Result<Vec<u8>> {
    let (st, b) = get(url, headers, timeout)?;
    if !(200..300).contains(&st) {
        return Err(anyhow!("{url} -> HTTP {st}"));
    }
    Ok(b)
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
    let mut resp = req.send(body).map_err(|e| anyhow!("POST {url}: {e}"))?;
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
