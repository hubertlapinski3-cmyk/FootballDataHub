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
