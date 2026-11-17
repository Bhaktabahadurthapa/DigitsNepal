#!/bin/sh
# Build DigitsNepal, sign it with the local signing certificate (if present),
# install it to /Applications and relaunch it.
#
# Signing with the same certificate every time gives the app a stable
# identity, so macOS keeps its Accessibility permission across rebuilds.
# Create the certificate once (see docs/RELEASE.md, "Local signing").
set -eu
cd "$(dirname "$0")/.."
IDENTITY="${DN_SIGNING_IDENTITY:-DigitsNepal Local Signing}"
APP=src-tauri/target/release/bundle/macos/DigitsNepal.app

CI=true npx tauri build --bundles app

if security find-identity -p codesigning | grep -q "\"$IDENTITY\""; then
  codesign --force --deep -s "$IDENTITY" "$APP"
  echo "Signed with \"$IDENTITY\"."
else
  echo "No \"$IDENTITY\" certificate; the app stays ad-hoc signed and macOS will ask for Accessibility again." >&2
fi

osascript -e 'tell application "DigitsNepal" to quit' 2>/dev/null || true
sleep 2
pkill -f "/Applications/DigitsNepal.app/Contents/MacOS/digitsnepal" 2>/dev/null || true
sleep 1
rm -rf /Applications/DigitsNepal.app
ditto "$APP" /Applications/DigitsNepal.app
open -a /Applications/DigitsNepal.app
