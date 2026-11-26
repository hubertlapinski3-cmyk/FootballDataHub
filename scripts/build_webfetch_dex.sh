#!/usr/bin/env bash
# Builds crates/hub-app/assets/webfetch.dex from crates/hub-app/android/dev/datahub/WebFetch.java.
#
# Needs a JDK and the Android SDK (platform android-34 and build-tools with d8), i.e. what the
# android-apk workflow already installs. The committed assets/webfetch.dex is an empty placeholder
# so desktop and test builds compile; the app reports "built-in browser not part of this build" if it
# is still empty at run time. CI runs this script before `cargo apk build`.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
sdk="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
[ -n "$sdk" ] || { echo "set ANDROID_SDK_ROOT to an Android SDK with platforms;android-34 and build-tools;34.0.0" >&2; exit 1; }
platform="${ANDROID_PLATFORM:-android-34}"
tools="${ANDROID_BUILD_TOOLS:-34.0.0}"
jar="$sdk/platforms/$platform/android.jar"
d8="$sdk/build-tools/$tools/d8"
[ -f "$jar" ] || { echo "missing $jar" >&2; exit 1; }
[ -x "$d8" ] || { echo "missing $d8" >&2; exit 1; }
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/classes" "$work/out"
javac --release 8 -Xlint:-options -cp "$jar" -d "$work/classes" "$root/crates/hub-app/android/dev/datahub/WebFetch.java"
(cd "$work/classes" && jar cf ../helper.jar .)
"$d8" --release --min-api 26 --lib "$jar" --output "$work/out" "$work/helper.jar"
mkdir -p "$root/crates/hub-app/assets"
cp "$work/out/classes.dex" "$root/crates/hub-app/assets/webfetch.dex"
echo "wrote crates/hub-app/assets/webfetch.dex ($(wc -c < "$root/crates/hub-app/assets/webfetch.dex") bytes)"
