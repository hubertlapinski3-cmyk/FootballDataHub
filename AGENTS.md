# Agent notes: Data Hub

This is a **Rust workspace**. There is no Gradle, Kotlin or Jetpack Compose: do not look for `Theme.kt`, `build.gradle`,
`./gradlew` or Compose Previews. The only Java is one WebView helper (`WebFetch.java`) that a script
compiles to DEX.

## Architecture

| Path | What it is |
|---|---|
| `crates/hub-core` | Pipeline and CLI (`matchhub`): ingest (WhoScored, StatsBomb) → enrich (FotMob, SofaScore, Reep ages) → models (xG, PSxG, xT, VAEP) → derive → boards (SVG + vector scene JSON) → report |
| `crates/hub-core/src/net.rs` | HTTP: `ureq` + rustls (ring) + webpki roots, gzip and brotli, browser headers, redirects, one retry on 502-504, classified status text. No OpenSSL |
| `crates/hub-core/src/scrape.rs` | All page parsing and error wording, free of I/O: `classify`, `json_blobs`, `tables`, `MatchLink`, `ScrapeError` |
| `crates/hub-core/src/cancel.rs` | Process-wide cancel flag, checked between steps and by the cancellable fetch |
| `crates/hub-core/tests/scrape.rs`, `tests/fixtures/` | Parser, link and HTTP tests against saved pages and a local server |
| `crates/hub-app/android/dev/datahub/WebFetch.java` | Off-screen `WebView` helper, built to DEX by `scripts/build_webfetch_dex.sh` and loaded over JNI |
| `crates/hub-core/src/catalog.rs` | League catalog and scouting index: `League`, `ScoutRole`, `METRICS`, `Catalog`, `ScoutIndex`, per-match rows, index builder (per-90, role percentiles), match packets, `CatalogStore` cache and `Remote` (raw.githubusercontent.com with a token) |
| `crates/hub-core/src/ingest.rs`, `scripts/ingest_leagues.py`, `scripts/leagues.json` | Nightly ingest: raw WhoScored files -> packets, rows, `catalog.json`, `scout_index.json` (`matchhub ingest`); the Python step fetches raw files |
| `.github/workflows/ingest-leagues.yml` | Nightly ingest on a self-hosted runner (WhoScored blocks GitHub's runners); publishes to the `data-catalog` branch |
| `crates/hub-core/src/render/scout.rs` | Season player card of one scouting-index row (percentile ring by role, average position) |
| `crates/hub-app/src/data.rs`, `screens/scout.rs` | Cached catalog and index, background refresh, league pills, catalog match cards, league scout leaderboard and season card |
| `crates/hub-core/src/render/portrait.rs` | Reflows the 1600 px landscape boards into a 640 px phone column |
| `crates/hub-core/data/reep_players.tsv.gz` | Trimmed Reep player register (CC0), rebuilt by `scripts/trim_reep.py` |
| `crates/hub-app` | egui app: Android `NativeActivity` (`android_main` in `src/lib.rs`, built with `cargo apk`) and desktop (`src/main.rs`) from one codebase |
| `crates/hub-app/src/style.rs` | **Design tokens and shared widgets.** Screens take colours, type and components only from here |
| `crates/hub-app/src/android_bridge.rs` | JNI helpers (raw `jni-sys`) for system font scale, keyboard inset and gesture exclusion; no-ops off Android |
| `crates/hub-app/src/scene.rs` | Native painter for board scenes: tooltips, legend toggles, motion |
| `crates/hub-app/src/screens/` | Screens: `browse.rs` (home, matches, hub, report, players, compare, settings), `viewer.rs` (board viewer), `mod.rs` (shared pieces) |
| `crates/hub-app/src/bin/screenshot.rs` | Headless screenshot harness: runs the real `App` without a GPU and rasterises in software |
| `DESIGN.md` | The fixa design system (Print / Darkroom themes, tokens, contrast, type, motif). Read it before any UI change |
| `.github/workflows/android-apk.yml` | CI: tests, then `cargo apk build` → `DataHub-apk` artifact |
| `.github/workflows/scrape-whoscored.yml` | WhoScored scrape relay (currently blocked by Cloudflare on GitHub runners) |

## Scraping

Supported inputs on Home (`scrape::MatchLink::parse`), all WhoScored:

| Input | Example |
|---|---|
| Match id | `2029131` |
| Any language host, any page type | `https://fr.whoscored.com/matches/2029131/live/europe-champions-league-2026-2027-arsenal-borussia-dortmund` (also `/show/`, `/matchreport/`, `/preview/`, `/Matches/2029131/Live`, an `/en/` path, no scheme) |
| Saved page or data | a `.html` page, the extracted `.json`, or a `.json.gz` placed in the app's `import` folder |

Load order in `sources::whoscored::load`: saved file, local cache, GitHub relay read, direct HTTP
(`net::get_page_cancellable`, host kept from the link), then if the answer was a bot wall
(`ScrapeError::Blocked`): the built-in browser (Android `WebView`, `Browser` in `BuildOptions`), then
the relay workflow (needs a token). The job runs on a worker thread (`library::spawn_build`), writes
`BuildState` (step, detail, fraction), and the Cancel button calls `matchhub::cancel::request()`. Errors
reach Home as a banner with Retry.

Why a browser fallback: WhoScored sits behind Cloudflare, which answers plain HTTP and GitHub's runners with
a JavaScript challenge (HTTP 403). A real engine on the phone's own connection passes it.
`android_bridge::webview_fetch` loads `dev.datahub.WebFetch` from `assets/webfetch.dex` with
`InMemoryDexClassLoader` (API 26+) because a native thread cannot implement `Runnable`. The committed
`.dex` is an empty placeholder; `scripts/build_webfetch_dex.sh` (javac + the SDK's `d8`, run by the
android-apk workflow) fills it. Without it the app says the built-in browser is not part of the build.

Rules for new sources: put parsing and messages in `scrape.rs` or `sources/`, never in the app crate;
add a fixture under `crates/hub-core/tests/fixtures/` and a test in `tests/scrape.rs`; send the
request through `net`, and never retry a 403 or 429.

## League data (seven leagues)

Premier League, La Liga, Bundesliga, Serie A, Ligue 1, Belgian Pro League and Ekstraklasa. The app never scrapes
these and never bundles raw matches.

1. `ingest-leagues.yml` runs on a self-hosted runner (labels `self-hosted`, `whoscored`), nightly and on demand
   (`league`, `max_matches`, `backfill`). `scripts/ingest_leagues.py` saves raw WhoScored match data under
   `work/raw/<league>/`; `matchhub ingest` turns it into packets (`matches/<league>/<id>.json.gz`, the parsed
   `Match` with enrichment, about 70 KB), per-player rows (`rows/`), `catalog.json` and `scout_index.json`.
2. Everything is pushed to the orphan branch `data-catalog` (not a release: a release holds at most 1000 assets,
   a season has about 2700 matches). The repository is private: the app reads the branch over HTTPS with the
   token of Settings > Scrape relay (`Remote`), so there is no separate key.
3. The app caches both files under `<data>/catalog/`, refreshes them in the background when older than 6 hours,
   and downloads one packet when a match is opened (`Source::Packet`), then builds its boards offline.
   `library::set_scout` hands the index to every build: player cards rank against the season role pool.
4. `scripts/leagues.json` holds the tournament pages. The last two entries are marked `verify`: check the
   WhoScored URLs once before the first run. Discovery and fetching have not been run against the live site.
5. Test fixture: `crates/hub-core/tests/fixtures/scout/` (synthetic clubs and players, one real packet). Regenerate
   with `cargo test -p hub-core --test scout regenerate_fixture -- --ignored` after changing `METRICS`.

## Rules

- No hardcoded colours in screens: add a token to `style.rs` and document it in `DESIGN.md`.
- Every font goes through `head/bold/semi/body` (app chrome, follows the text size) or `board_font`
  (boards, fixed). Digits are tabular because the bundled Schibsted Grotesk files have `tnum` frozen in.
- Touch targets are 48dp. Rows of frames (chips, cards) must be chunked by width, not put in
  `horizontal_wrapped`: egui cannot measure a frame before placing it, and an overflowing row widens
  the whole screen.
- Long titles truncate (`top_bar`, viewer header) instead of widening the layout.
- `Library::entries` are `Arc<Entry>`: clone the `Arc`, never the summary.
- Android-only code sits behind `#[cfg(target_os = "android")]`; desktop must keep building.

## Commands

```bash
cargo test --workspace
cargo test -p hub-core --test scrape      # scraper parsers, links and the HTTP layer
cargo build -p hub-app
cargo run --release -p hub-app --bin datahub
# headless screenshots of every screen (builds the bundled sample match on first run)
cargo run --release -p hub-app --bin screenshot -- /tmp/shots/data /tmp/shots/out
# variants: theme (dark | print), compact width, text size in percent
THEME=print SHOT_W=320 TEXT_SCALE=150 cargo run --release -p hub-app --bin screenshot -- /tmp/shots/data /tmp/shots/print_320
# whole boards only, for checking board layouts
BOARDS=05_creation_corridors,21_xt_heatmap_home cargo run --release -p hub-app --bin screenshot -- /tmp/shots/data /tmp/shots/boards
```

Delete the data directory after changing `hub-core` rendering: built matches keep their old boards.

There is no Android SDK or NDK in the cloud sandbox, so the APK only builds in CI. Code in
`android_bridge.rs` can be type-checked against `aarch64-linux-android` in a scratch crate that depends
only on `jni-sys` and `winit` (`android-native-activity`); the full app cannot, because `ring` needs the NDK.
