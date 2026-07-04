#!/usr/bin/env bash
# Build a Git Dashboard.app bundle from a release binary, on macOS.
# Must run on macOS (uses sips/iconutil, both macOS-only).
set -euo pipefail
cd "$(dirname "$0")/.."

APP_NAME="Git Dashboard"
BIN_NAME="git-dashboard"
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
BUNDLE_ID="com.orapli.git-dashboard"

command -v sips >/dev/null || { echo "sips not found — this script must run on macOS"; exit 1; }
command -v iconutil >/dev/null || { echo "iconutil not found — this script must run on macOS"; exit 1; }

echo "==> cargo build --release (v$VERSION)"
cargo build --release

BIN="target/release/$BIN_NAME"
[ -f "$BIN" ] || { echo "binary not found at $BIN"; exit 1; }

APP="dist/$APP_NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

echo "==> icon: assets/icon.png -> $APP_NAME.icns"
ICONSET=$(mktemp -d)/icon.iconset
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" assets/icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" assets/icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

cp "$BIN" "$APP/Contents/MacOS/$BIN_NAME"
chmod +x "$APP/Contents/MacOS/$BIN_NAME"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleExecutable</key>
    <string>$BIN_NAME</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon.icns</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

ZIP="dist/GitDashboard-$VERSION-macOS.zip"
rm -f "$ZIP"
(cd dist && zip -qr "$(basename "$ZIP")" "$APP_NAME.app")

echo "==> done: $ZIP"
ls -lh "$ZIP"
echo
echo "NOTE: this build is not code-signed or notarized. On first launch, macOS"
echo "Gatekeeper will block it; users need to right-click > Open, or run:"
echo "  xattr -cr \"/Applications/$APP_NAME.app\""
