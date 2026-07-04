#!/usr/bin/env bash
# Build the Windows installer from macOS (or Linux).
# Requirements: rustup target x86_64-pc-windows-gnu, mingw-w64, makensis
#   brew install mingw-w64 makensis
#   rustup target add x86_64-pc-windows-gnu
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=x86_64-pc-windows-gnu
EXE=target/$TARGET/release/git-dashboard.exe
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')

# Preflight checks with actionable messages
command -v x86_64-w64-mingw32-gcc >/dev/null \
  || { echo "mingw-w64 not found: brew install mingw-w64"; exit 1; }
command -v makensis >/dev/null \
  || { echo "makensis not found: brew install makensis"; exit 1; }
rustup target list --installed | grep -q "$TARGET" \
  || { echo "Rust target not installed: rustup target add $TARGET"; exit 1; }

echo "==> cargo build --release --target $TARGET (v$VERSION)"
cargo build --release --target "$TARGET"

echo "==> makensis"
mkdir -p dist
makensis -DAPP_VERSION="$VERSION" -DEXE_PATH="../../$EXE" \
  -DOUT_FILE="../../dist/GitDashboard-$VERSION-setup.exe" \
  installer/windows/installer.nsi

echo "==> done: dist/GitDashboard-$VERSION-setup.exe"
ls -lh "dist/GitDashboard-$VERSION-setup.exe"
