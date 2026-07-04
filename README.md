# Git Dashboard

A desktop-native Git repository analytics and visualization dashboard built with Rust and egui/eframe.
Overview activity across multiple repositories, inspect commits, branches, tags, contributors, and file diffs seamlessly.

## Features

### Dashboard
- **Multi-Repository Management:** Register multiple local Git repositories from Settings (supports bulk registration via folder selection). View them on the Home screen and switch with a single click.
- **Statistics Visualization:** Daily, hourly, and day-of-week commit charts for the past 30 days, author/extension breakdown donut charts, and KPI summary cards.
- **Commit Graph:** Interactive branch graph view. Click a row to view commit details; click ● markers to pick base/target for range diffs.
- **Tag List Comparison:** Built for release management workflows; compare any 2 tags or compare with the previous tag using ● markers.
- **Stash Management:** List, apply, drop, and inspect stash diffs.
- **Bulk Pull / Fetch:** Sync all registered repositories simultaneously (asynchronous, non-blocking UI).

### Diff View
- **Split Diff Display:** Similarity-based line pairing (heavily modified lines shown as paired deletion + addition) with word-level inline highlights. Accurate rendering for multibyte characters and mixed tabs.
- **Syntax Highlighting / Blame View / Hunk Navigation (n/p) / In-File Search.**
- **Commit & Tag Picker with Search:** Incremental filtering by hash, author, message, or tag name.
- **Line Detail:** Select a line to display old and new versions side-by-side with character-level diffs.

### General & Infrastructure
- **SSH Remote Repositories:** Register repositories on virtual machines or remote servers using `ssh://user@host/absolute/path` format (supports analysis, diffs, blame, tag comparison, and Pull/Fetch). Requires pre-configured SSH key authentication and remote `git`.
- **Contributor Alias Mapping:** Unify multiple accounts/emails for the same contributor in `members.json`. Supports active/inactive filtering.
- **Tech Stack Detection:** Auto-detects frameworks (Rails, Node.js, Django, etc.) from configuration files (`Gemfile.lock`, `.nvmrc`, etc.). Custom rules can be added in `tech_rules.json`.
- **Theme Switcher:** Includes 5 default themes (One Dark, One Light, Ayu Mirage, Ayu Light, Nord). Fully customizable via `themes.json`.
- **Persistent Preferences:** Window geometry and application preferences are automatically saved in the standard OS configuration directory.

## Setup and Running

### Requirements
- Rust toolchain (**Rust 1.85+** for edition 2024 support)
- `git` command available in system `PATH`

### Build & Run
```bash
cargo run --release
```

For OS-specific instructions (such as Linux GUI dependencies), see [docs/BUILD.md](docs/BUILD.md).
For Windows installer creation (on macOS), see [docs/BUILD_WINDOWS.md](docs/BUILD_WINDOWS.md).

## Using Remote Repositories via SSH

Register remote repositories by entering `ssh://user@host/absolute/path/to/repo` in the path field on the Settings screen
(e.g., `ssh://dev@192.168.1.10/home/dev/myapp`. Host aliases in `~/.ssh/config` are supported).

Prerequisites:
- SSH **Key Authentication** must be configured so connections succeed without a password prompt.
- `git` must be installed on the remote machine.
- Initial connection must be completed (remote host key added to `known_hosts`).

Limitations: Opening in local editor, tech stack detection, and file size listings are unavailable for remote SSH repositories.

### Recommended: SSH Connection Multiplexing (Performance Boost)

Since the app executes individual `ssh` commands for Git operations, analyzing a single repository can initiate multiple SSH handshakes (TCP + key exchange). Enabling **connection multiplexing** in `~/.ssh/config` reuses a persistent master connection, significantly speeding up repository analysis and diff rendering.

```sshconfig
Host myvm                       # ← Host specified in ssh:// (or SSH alias)
    HostName 192.168.1.10
    User dev
    # Connection Multiplexing
    ControlMaster auto
    ControlPath ~/.ssh/sockets/%r@%h:%p
    ControlPersist 10m          # Keeps master connection open for 10 minutes after last session
    # Keepalive
    ServerAliveInterval 30
    ServerAliveCountMax 3
```

Ensure the socket directory exists:

```sh
mkdir -p ~/.ssh/sockets
```

Verify multiplexing with `ssh -O check <host>` (returns `Master running` when active).

> **Note (Windows):** Win32-OpenSSH does not natively support `ControlMaster` multiplexing. `ServerAliveInterval` remains effective on Windows. Multiplexing works when using SSH within WSL or Git for Windows bash environments.

## Configuration Files

Configuration files are automatically generated in the standard OS config directory:

| OS | Location |
|----|----------|
| macOS | `~/Library/Application Support/com.git-dashboard.git-dashboard/` |
| Windows | `%APPDATA%\git-dashboard\git-dashboard\config\` |
| Linux | `~/.config/git-dashboard/` |

| File | Description |
|------|-------------|
| `config.json` | Registered repository names and paths |
| `members.json` | Contributor alias mappings and active status |
| `tech_rules.json` | Regex rules for tech stack detection |
| `themes.json` | Theme color definitions |
| `prefs.json` | UI preferences (active theme, language, sidebar width) |

## Developer Guide

For developer commands, codebase structure, and UI guidelines, refer to [CLAUDE.md](CLAUDE.md).
CI checks enforce `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test`.
