# Data Hub

A pure-Rust football match analysis pipeline and an Android app built on top of it. One WhoScored match goes in. Out come 39 analysis boards plus one player card per player, a written match report and a provenance manifest for every board. The app is the viewing and browsing layer on Android, and the same code runs on desktop.

```
crates/
  hub-core/   pipeline: ingest -> enrich -> models -> derive -> theme -> boards -> report (+ CLI `matchhub`)
  hub-app/    egui app (Android NativeActivity + desktop), 8 screens from the UI spec
docs/
  DATA_HUB_UPGRADE.md   full spec the pipeline implements
  screenshots/          headless renders of every app screen
.github/workflows/android-apk.yml   CI: tests + APK artifact
```

## Quick start (desktop)

```bash
cargo run --release -p hub-app --bin datahub          # phone-sized window, same UI as Android
cargo run --release -p hub-core -- build --whoscored saved_page.html --out out/
cargo run --release -p hub-core -- build --whoscored 1650630 --out out/   # online fetch (personal use)
```

For CLI options run `matchhub --help`. The main commands are:
- `build`: one match to SVG/PNG boards, `report.md`, `analysis.json` and manifests.
- `train`: refit xG, PSxG, xT and VAEP on StatsBomb Open Data.
- `season`: fill the competition store used by the two context scatters.

## Android APK

There is no Java, Gradle or Kotlin. The app is a `cdylib` with `android_main`, running on `android-activity` (NativeActivity) through eframe/winit with OpenGL ES (glow).

### Option A: CI (no local setup)

Push the repo to GitHub. The `android-apk` workflow installs the SDK and NDK and the Rust Android targets, runs the tests, builds `DataHub.apk` and uploads it as a workflow artifact. Download it and install with `adb install DataHub.apk`, or open the file on the phone.

### Option B: local

```bash
# once
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-apk
# Android SDK (platform 34, build-tools 34) + NDK r27, then:
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_ROOT=$ANDROID_HOME/ndk/27.2.12479018

cargo apk build -p hub-app --lib --release      # -> target/release/apk/DataHub.apk
cargo apk run   -p hub-app --lib --release      # build, install and launch on a connected device
adb logcat -s datahub                            # app logs
```

Manifest settings live in `crates/hub-app/Cargo.toml` under `[package.metadata.android]`:
- package `com.datahub.app`, min SDK 26, target SDK 34
- `INTERNET` permission
- GLES 3
- `singleTask` launch mode and config changes handled in-process

A release build without a signing profile is signed with the debug keystore. To sign for distribution, add `[package.metadata.android.signing.release]` with your keystore. The Play Store needs an AAB, which cargo-apk does not produce. Wrap the `.so` in a Gradle project, or use xbuild, for that.

### Android notes

- **TLS:** HTTP goes through `ureq` + rustls + webpki-roots, so certificates verify without JVM glue.
- **Fonts:** Barlow Condensed and Inter are embedded, so boards render identically on every device.
- **Soft keyboard:** opened and closed through `AndroidApp::show_soft_input` / `hide_soft_input` when a text field takes focus. The hardware or gesture Back button pops the screen stack.
- **Data:** stored in the app's internal data dir. Saved WhoScored pages can be imported from `/sdcard/Android/data/com.datahub.app/files/import/`, with `adb push page.html /sdcard/Android/data/com.datahub.app/files/import/`. They then appear on Home.
- **Exports:** PNGs go to `.../files/exports/`.

## Getting WhoScored data

WhoScored embeds the whole event feed in the match-centre page. The parser uses the same marker approach as [Scrape-Whoscored-Event-Data](https://github.com/Ali-Hasan-Khan/Scrape-Whoscored-Event-Data): it finds `require.config.params["args"]` and then the `matchCentreData` object. Competition and season are read from the breadcrumb. The parser was checked against the real captured page bundled in that repo (Barcelona 0–0 Rayo Vallecano, 1,465 events). That match is a useful edge case, a 0–0 with a "No goals" empty state.

Online fetching is behind the personal-use flag in Settings. It makes one request per match with browser-like headers and caches the result. On a bot wall (HTTP 403), save the page in a browser and import the `.html`, `.json` or `.html.gz` instead. FotMob and SofaScore enrichment (colours, crests, xGOT) is best effort. Every miss is recorded as a fallback in the manifest.

## Models

The models are fitted with `matchhub train` on 179 StatsBomb Open Data matches (World Cups 2018 and 2022, Euro 2020) and embedded in the binary:

| Model | Type | Training data | Fit |
|---|---|---|---|
| xG | logistic | 4,302 shots | log-loss 0.263 |
| PSxG (local placement) | logistic | 1,334 on-target shots | log-loss 0.480 |
| xT | 16×12 value iteration | same matches | – |
| VAEP | logistic pair | 235k actions | – |

The bundled sample is the Euro 2024 final (StatsBomb Open Data, non-commercial, attributed). It lets the app show real boards offline on first launch.

## Verification done

- `cargo test --workspace`:
  - theme tests: CIEDE2000 and contrast normalisation
  - headless UI smoke tests: every screen, with both an empty and a populated library, stale-index recovery, and the Back key
- `screenshot` binary: drives the real App through egui without a GPU, rasterises in software, and writes `docs/screenshots/`.
- Full builds of the real WhoScored page and the StatsBomb final: all consistency gates pass (shot partition, xG reconciliation, keeper partition).

**Not verified:**
- The APK was not built or run on a device. The sandbox used for development had no access to the Android SDK or NDK.
- The FotMob and SofaScore endpoints were not reachable from the sandbox.

Treat the first device run and the enrichment endpoints as untested.
