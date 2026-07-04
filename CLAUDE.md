# CLAUDE.md (Git Dashboard Developer Guide)

Handover notes for Claude Code, other AI assistants, and human developers.
For user-facing feature overview and setup, see [README.md](README.md).

## 🚀 Command Reference

* **Check / static analysis**: `cargo check` / `cargo clippy`
* **Debug build & run**: `cargo run`
* **Release build & run**: `cargo run --release`
* **Run tests**: `cargo test`
* **Build Windows installer (fully on macOS)**: `./scripts/build-windows-installer.sh`

Verification steps after any change: `cargo fmt` → `cargo clippy --all-targets` (zero warnings)
→ `cargo test` → `cargo run`.
CI (`.github/workflows/ci.yml`) runs `fmt --check` / `clippy -D warnings` / `test`, so
**a commit that doesn't pass fmt and clippy will fail CI**.

## 📄 Document Map

| File | Content |
|---|---|
| `README.md` | User-facing: features, setup, config files |
| `CLAUDE.md` (this file) | Developer/AI-facing: commands, structure, implementation rules |
| `docs/BUILD.md` | Per-OS build instructions (Linux dependencies, etc.) |
| `docs/BUILD_WINDOWS.md` | Building the Windows installer (cross-build on macOS) |
| `backlog/README.md` | Index of not-yet-started requirements and operating rules |

## 📂 Project Structure and Responsibilities

```
git-dashboard/
├── Cargo.toml          # Project dependencies (eframe/egui/regex/serde/chrono etc.)
├── build.rs            # Embeds exe icon/version info for Windows builds (winresource)
├── assets/             # App icons (regeneratable via scripts/generate_icon.py)
├── installer/windows/  # NSIS installer script
├── scripts/            # Installer build / icon generation scripts
├── src/
│   ├── main.rs         # Entry point. eframe init, window setup.
│   ├── app/            # Application UI and state (GitDashboardApp)
│   │   ├── mod.rs      # State definitions, worker threads (Job/AsyncMessage), sidebar, keyboard ops
│   │   ├── home.rs     # Home (repository list)
│   │   ├── dashboard.rs# Per-repo stats (commit graph, donut charts, branch/tag list, tag-range diff)
│   │   ├── diff.rs     # Diff view (split view, inline-highlight rendering, search, blame, pickers)
│   │   ├── settings.rs # Settings (repo/member management, appearance, open config folder)
│   │   └── widgets.rs  # Shared widgets (vector icon buttons, charts)
│   ├── config.rs       # Config file read/write (write_atomic: temp file + rename)
│   ├── git.rs           # External `git` command invocation, unified diff parsing (similarity-based line pairing)
│   ├── syntax.rs        # Lightweight syntax highlighting for diffs (per-language tokenizing)
│   ├── theme.rs          # Theme definitions (Catppuccin Mocha/Latte/High Contrast, One Dark/Light, Ayu, Nord)
│   └── worddiff.rs       # Inline (word/char-level) diff core logic (UI-independent, tested)
```

Config files (config.json / members.json / tech_rules.json / themes.json / prefs.json) are
auto-generated on first launch under the OS config directory
(macOS: `~/Library/Application Support/com.git-dashboard.git-dashboard/`).

## 🎨 UI Implementation Rules

* **Colors always go through `self.theme`** (`let t = self.theme;` at the top of each draw function).
  No hardcoded `Color32`. Exceptions: `dashboard.rs`'s `ansi_256_color` (fixed ANSI palette) and
  white text on accent-colored buttons only.
* **Bump `theme.rs`'s `CURRENT_THEME_VERSION` whenever a theme color changes.**
  Themes are read from `themes.json` on disk, so without a version bump existing installs won't
  see the update. Add retired themes to `RETIRED_THEMES` so migration removes them.
* **Layout**: corner radius unified at `4.0`, vertical margin between main-area blocks `20.0`,
  KPI cards use minimum height `64.0` / `inner_margin(12.0)`.
* **Text vertical alignment**: placing `ui.label` on the same row as a taller widget makes the
  text appear to float — wrap it in
  `ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), ...)`.
* **Light-theme legibility**: translucent colors become nearly imperceptible over a light
  background. Diff-related background colors branch on `t.is_dark`; light theme uses opaque
  pastel colors instead (see `word_hl` in diff.rs).

## ⚠️ Implementation Invariants (breaking these reintroduces known bugs)

* **Inline highlight positions come from the laid-out galley** (`pos_from_ccursor`).
  Do not approximate via "column count × monospace char width" — it drifts with Japanese text
  and tabs (worddiff.rs returns character indices instead).
* **Async work uses a seq (generation number) pattern**: multiple worker threads complete out of
  order, so increment `seq` at request time and discard the result on receipt if `seq` no longer
  matches (see `diff_*_seq` in mod.rs). Follow the same pattern for any new async work.
* **Panel collapse state needs a distinct panel ID per state** (egui remembers width per ID;
  sharing one lets the collapsed width overwrite and lose the expanded width).
* **Config file writes must use `config::write_atomic`** (never `fs::write` directly).
* **Diff DP-family algorithms need a size cap** (prevents freezes/memory blowup on huge lines;
  see the existing cap in worddiff.rs).

## ⚙️ Tech Stack Detection Rules (tech_rules.json)

Ships with detection logic for Ruby on Rails / Node.js / Django by default. Edit the
`tech_rules.json` generated in the config directory to add or edit regex-based detection rules
dynamically.

## 🛠 Development Environment Notes

* cargo may not be on PATH: `export PATH="$HOME/.cargo/bin:$PATH"`
* egui is on the 0.35 series. Use `ComboBox::from_id_salt` / `ScrollArea::id_salt`
  (`from_id_source`/`id_source` are removed).
* Local-only test repos: set up something like `~/work/test-repo-frontend` /
  `~/work/test-repo-backend` to exercise multi-repository behavior and tech-stack detection.
