# Agent notes: Data Hub

This is a **Rust workspace**. There is no Gradle, Kotlin, Java or Jetpack Compose code: do not look for
`Theme.kt`, `build.gradle`, `./gradlew` or Compose Previews.

## Architecture

| Path | What it is |
|---|---|
| `crates/hub-core` | Pipeline and CLI (`matchhub`): ingest (WhoScored, StatsBomb) → enrich (FotMob, SofaScore, Reep ages) → models (xG, PSxG, xT, VAEP) → derive → boards (SVG + vector scene JSON) → report |
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
