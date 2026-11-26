//! Page scraping: everything that turns downloaded (or browser-rendered) HTML into data lives here,
//! apart from any I/O, so it is unit-testable with saved fixtures.
//!
//! - [`classify`]: what kind of page this is, and when it is a bot wall rather than a match page.
//! - [`json_blobs`]: embedded state (`__NEXT_DATA__`, `require.config.params`, `window.X = {...}`,
//!   `application/ld+json`) as parsed JSON.
//! - [`tables`]: HTML tables as header + rows (CSS-selected with `scraper`).
//! - [`MatchLink`]: the URL shapes the app accepts.
//! - [`ScrapeError`]: failures worded for the person who has to act on them.

use anyhow::{anyhow, Result};
use scraper::{Html, Selector};
use serde_json::Value;

// ---------------------------------------------------------------- errors

#[derive(Debug, Clone, PartialEq)]
pub enum ScrapeError {
    /// A bot wall or JavaScript challenge answered instead of the page.
    Blocked { host: String, vendor: String, status: Option<u16> },
    RateLimited { host: String, retry_after: Option<u64> },
    Http { host: String, status: u16 },
    /// The page loaded but holds no match data.
    NoMatchData { host: String },
    Cancelled,
}

impl std::fmt::Display for ScrapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScrapeError::Blocked { host, vendor, status } => write!(
                f,
                "{host} blocked the request ({vendor}{}). Its protection runs JavaScript that plain HTTP cannot pass. Wait before retrying; repeated requests prolong the block.",
                status.map(|s| format!(", HTTP {s}")).unwrap_or_default()
            ),
            ScrapeError::RateLimited { host, retry_after } => match retry_after {
                Some(s) => write!(f, "{host} says too many requests (HTTP 429). Try again in {s} seconds."),
                None => write!(f, "{host} says too many requests (HTTP 429). Wait a few minutes before trying again."),
            },
            ScrapeError::Http { host, status } => write!(f, "{}", crate::net::describe_status(&format!("https://{host}/"), *status, None)),
            ScrapeError::NoMatchData { host } => write!(f, "The page from {host} has no match data. The match may not have been played yet, the id may be wrong, or the page layout changed."),
            ScrapeError::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl std::error::Error for ScrapeError {}

/// Does this error mean "a browser engine could pass where plain HTTP could not"?
pub fn is_blocked(e: &anyhow::Error) -> bool {
    matches!(e.downcast_ref::<ScrapeError>(), Some(ScrapeError::Blocked { .. }))
}

// ---------------------------------------------------------------- page classification

#[derive(Debug, Clone, PartialEq)]
pub enum PageKind {
    /// WhoScored match centre (carries `matchCentreData`).
    WhoScoredMatch,
    /// Bot wall / challenge page; the string names the vendor.
    BotWall(String),
    /// Anything else.
    Other,
}

pub fn classify(html: &str) -> PageKind {
    let head: String = html.chars().take(20_000).collect::<String>().to_lowercase();
    if html.contains("matchCentreData") {
        return PageKind::WhoScoredMatch;
    }
    for (needle, vendor) in [
        ("attention required! | cloudflare", "Cloudflare"),
        ("just a moment...", "Cloudflare"),
        ("cf-browser-verification", "Cloudflare"),
        ("challenge-platform", "Cloudflare"),
        ("_incapsula_resource", "Imperva"),
        ("incapsula incident", "Imperva"),
        ("request unsuccessful", "Imperva"),
        ("px-captcha", "PerimeterX"),
        ("access denied", "an access filter"),
        ("g-recaptcha", "a captcha"),
        ("are you a robot", "a captcha"),
    ] {
        if head.contains(needle) {
            return PageKind::BotWall(vendor.to_string());
        }
    }
    PageKind::Other
}

/// Page text to match data or a worded error.
pub fn require_match_page(html: &str, host: &str, status: Option<u16>) -> std::result::Result<(), ScrapeError> {
    match classify(html) {
        PageKind::WhoScoredMatch => Ok(()),
        PageKind::BotWall(vendor) => Err(ScrapeError::Blocked { host: host.to_string(), vendor, status }),
        PageKind::Other => Err(ScrapeError::NoMatchData { host: host.to_string() }),
    }
}

/// Map an HTTP status to the right error (None for 2xx).
pub fn status_error(host: &str, status: u16, retry_after: Option<u64>, body: &str) -> Option<ScrapeError> {
    match status {
        200..=299 => None,
        429 => Some(ScrapeError::RateLimited { host: host.to_string(), retry_after }),
        403 | 503 => match classify(body) {
            PageKind::BotWall(vendor) => Some(ScrapeError::Blocked { host: host.to_string(), vendor, status: Some(status) }),
            _ if status == 403 => Some(ScrapeError::Blocked { host: host.to_string(), vendor: "a bot filter".into(), status: Some(status) }),
            _ => Some(ScrapeError::Http { host: host.to_string(), status }),
        },
        s => Some(ScrapeError::Http { host: host.to_string(), status: s }),
    }
}

// ---------------------------------------------------------------- embedded JSON

#[derive(Debug, Clone)]
pub struct JsonBlob {
    /// Where it came from: `__NEXT_DATA__`, `ld+json`, `window.__NUXT__`, `matchCentreData`, ...
    pub label: String,
    pub value: Value,
}

/// Balanced `{...}` or `[...]` starting at `start`, string- and escape-aware.
pub fn balanced(text: &str, start: usize) -> Option<&str> {
    let b = text.as_bytes();
    let open = *b.get(start)?;
    let close = match open {
        b'{' => b'}',
        b'[' => b']',
        _ => return None,
    };
    let (mut depth, mut in_str, mut esc) = (0i32, false, false);
    for (i, &c) in b.iter().enumerate().skip(start) {
        if in_str {
            if esc {
                esc = false
            } else if c == b'\\' {
                esc = true
            } else if c == b'"' {
                in_str = false
            }
        } else if c == b'"' {
            in_str = true
        } else if c == open {
            depth += 1
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return text.get(start..=i);
            }
        }
    }
    None
}

/// Every parseable JSON state blob in the page.
pub fn json_blobs(html: &str) -> Vec<JsonBlob> {
    let mut out = vec![];
    let doc = Html::parse_document(html);
    // <script id="__NEXT_DATA__" type="application/json"> and <script type="application/ld+json">
    if let Ok(sel) = Selector::parse("script") {
        for s in doc.select(&sel) {
            let text: String = s.text().collect();
            let ty = s.value().attr("type").unwrap_or("");
            let id = s.value().attr("id").unwrap_or("");
            let label = if id == "__NEXT_DATA__" {
                "__NEXT_DATA__"
            } else if ty.contains("ld+json") {
                "ld+json"
            } else if ty == "application/json" && !id.is_empty() {
                id
            } else {
                continue;
            };
            if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
                out.push(JsonBlob { label: label.to_string(), value: v });
            }
        }
    }
    // inline assignments: window.__NUXT__ = {...}; window.__INITIAL_STATE__ = {...}; var data = {...}
    let re = regex::Regex::new(r#"(?:window\.|var |let |const )?(__[A-Z_]+__|initialState|initialData|pageData)\s*=\s*"#).unwrap();
    for m in re.captures_iter(html) {
        let end = m.get(0).unwrap().end();
        if let Some(obj) = balanced(html, end) {
            if let Ok(v) = serde_json::from_str::<Value>(obj) {
                let name = m.get(1).unwrap().as_str();
                if name != "__NEXT_DATA__" || !out.iter().any(|b| b.label == "__NEXT_DATA__") {
                    out.push(JsonBlob { label: format!("window.{name}"), value: v });
                }
            }
        }
    }
    // WhoScored: require.config.params["args"] = { matchId, matchCentreData: {...}, ... }
    if let Some(i) = html.find("matchCentreData") {
        if let Some(j) = html[i..].find('{') {
            if let Some(obj) = balanced(html, i + j) {
                if let Ok(v) = serde_json::from_str::<Value>(obj) {
                    out.push(JsonBlob { label: "matchCentreData".into(), value: v });
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------- tables

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub caption: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// All `<table>`s as headers (from `<th>`) and rows (from `<td>`), with cell text normalised.
pub fn tables(html: &str) -> Vec<Table> {
    let doc = Html::parse_document(html);
    let (Ok(t), Ok(cap), Ok(tr), Ok(th), Ok(td)) = (Selector::parse("table"), Selector::parse("caption"), Selector::parse("tr"), Selector::parse("th"), Selector::parse("td")) else {
        return vec![];
    };
    let mut out = vec![];
    for table in doc.select(&t) {
        let caption = table.select(&cap).next().map(|c| clean(&c.text().collect::<String>())).unwrap_or_default();
        let mut headers: Vec<String> = vec![];
        let mut rows = vec![];
        for row in table.select(&tr) {
            let ths: Vec<String> = row.select(&th).map(|c| clean(&c.text().collect::<String>())).collect();
            let tds: Vec<String> = row.select(&td).map(|c| clean(&c.text().collect::<String>())).collect();
            if !ths.is_empty() && tds.is_empty() && headers.is_empty() {
                headers = ths;
            } else if !tds.is_empty() {
                rows.push(tds);
            }
        }
        if !rows.is_empty() || !headers.is_empty() {
            out.push(Table { caption, headers, rows });
        }
    }
    out
}

/// First `<title>` text.
pub fn title(html: &str) -> String {
    let doc = Html::parse_document(html);
    Selector::parse("title").ok().and_then(|s| doc.select(&s).next().map(|t| clean(&t.text().collect::<String>()))).unwrap_or_default()
}

// ---------------------------------------------------------------- match links

/// A link or id the app can scrape.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchLink {
    pub id: String,
    /// Host the user's link points at (`fr.whoscored.com`), or `www.whoscored.com` for a bare id.
    pub host: String,
}

impl MatchLink {
    /// Accepts: `2029131`, `whoscored.com/matches/2029131`, `https://fr.whoscored.com/matches/2029131/live/europe-...`,
    /// `/Matches/2029131/Live/...`, `.../show/...`, `.../matchreport/...`, `.../preview/...`.
    pub fn parse(input: &str) -> Option<MatchLink> {
        let s = input.trim();
        if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
            return Some(MatchLink { id: s.to_string(), host: "www.whoscored.com".into() });
        }
        let re = regex::Regex::new(r"(?i)^(?:https?://)?((?:[a-z0-9-]+\.)*whoscored\.com)/(?:[a-z]{2}(?:-[a-z]{2})?/)?matches/(\d+)").unwrap();
        let c = re.captures(s)?;
        Some(MatchLink { id: c[2].to_string(), host: c[1].to_lowercase() })
    }

    /// The match-centre page to load. The language/regional host is kept, since the cookies and
    /// protection of each host are separate.
    pub fn page_url(&self) -> String {
        format!("https://{}/Matches/{}/Live", self.host, self.id)
    }
}

/// Brace-matched `matchCentreData` object text (kept for the WhoScored loader).
pub fn match_centre_blob(html: &str) -> Result<String> {
    let key = "matchCentreData";
    let base = html.find("require.config.params[\"args\"]").unwrap_or(0);
    let start = html[base..].find(key).map(|i| i + base).or_else(|| html.find(key)).ok_or_else(|| anyhow!("matchCentreData not found in page"))?;
    let brace = html[start..].find('{').ok_or_else(|| anyhow!("matchCentreData has no object"))? + start;
    balanced(html, brace).map(|s| s.to_string()).ok_or_else(|| anyhow!("unterminated matchCentreData object"))
}
