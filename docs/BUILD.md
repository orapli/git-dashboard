# Build & Install Instructions

Git Dashboard is a native GUI app built with Rust + eframe/egui.
It can be built from source on macOS / Windows / Linux.

---

## Common Prerequisites

- **Rust (stable)** — this project uses `edition = "2024"`, so **Rust 1.85 or later** is required.
  - Install: https://rustup.rs
  - If already installed, update with `rustup update`.
- **Git** — the app shells out to the `git` command internally, so `git` must be installed and
  **on PATH** in the runtime environment.

The build output is a single standalone executable that runs as-is
(a Windows installer can also be built → [BUILD_WINDOWS.md](BUILD_WINDOWS.md)).
Config files (`config.json` / `members.json` / `tech_rules.json` / `themes.json` / `prefs.json`)
are auto-generated in the OS config directory (resolved per-OS via the `directories` crate).

| OS | Config file location |
|----|----------------------|
| macOS | `~/Library/Application Support/com.git-dashboard.git-dashboard/` |
| Windows | `%APPDATA%\git-dashboard\git-dashboard\config\` |
| Linux | `~/.config/git-dashboard/` |

---

## Building on Windows (native)

### 1. Tool setup

1. **Rust** — run `rustup-init.exe` from https://rustup.rs.
   - If prompted to install the **MSVC build tools (Visual Studio C++ Build Tools)**, do so
     (required for eframe's linking). The default `x86_64-pc-windows-msvc` target is fine.
2. **Git for Windows** — https://git-scm.com/download/win.
   During install, choose "Add to PATH" (so `git` is usable from Command Prompt/PowerShell).

### 2. Build & run

```cmd
git clone <repository-url>
cd git-dashboard
cargo build --release
```

This produces `target\release\git-dashboard.exe`. Double-click it, or run:

```cmd
target\release\git-dashboard.exe
```

### Notes

- Japanese text rendering auto-loads OS-bundled fonts (Yu Gothic / Meiryo / MS Gothic)
  (Windows paths are already registered in `src/theme.rs`'s `font_paths`).
- To distribute the app, just copy `git-dashboard.exe` — it runs standalone.
  For the NSIS-based installer, see [BUILD_WINDOWS.md](BUILD_WINDOWS.md)
  (it can be built entirely via cross-compilation on macOS).

---

## Building on macOS

```sh
# if cargo is not on PATH
export PATH="$HOME/.cargo/bin:$PATH"

git clone <repository-url>
cd git-dashboard
cargo build --release
./target/release/git-dashboard      # run
# or, for a dev build
cargo run
```

---

## Building on Linux

Requires your distro's dev tools plus the system libraries eframe/egui need.

```sh
# Example for Debian/Ubuntu
sudo apt install build-essential pkg-config libgtk-3-dev libxcb-render0-dev \
                 libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev

git clone <repository-url>
cd git-dashboard
cargo build --release
./target/release/git-dashboard
```

---

## Cross-Compiling (macOS → Windows)

Cross-compiling from macOS to Windows (`x86_64-pc-windows-gnu` + mingw-w64) is supported, and the
installer can be built entirely on macOS (verified working). See [BUILD_WINDOWS.md](BUILD_WINDOWS.md)
for the steps.

---

## Common Errors

- `error: edition 2024 is unstable` / edition-related errors — your Rust is too old. Run
  `rustup update` to get 1.85+.
- App launches but git info doesn't load — `git` is not on PATH. Install Git and add it to PATH.
- Japanese text renders as tofu boxes (□) — the OS lacks a matching font. Add an available
  font path to `font_paths` in `src/theme.rs`.
