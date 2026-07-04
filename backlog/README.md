# Backlog

Index of not-yet-started or pending requirements. Open the target task's `.md` file when you
start working on it. The `.md` files for completed tasks (most of T01–T59) have been removed
during cleanup — see the git log and the initial-history backup bundle for that history.

## List

| ID | Requirement | Priority | Status | Main targets |
|----|-------------|----------|--------|---------------|
| _(none currently)_ | | | | |

Priority: High > Medium > Low / Status: not started / on hold / in progress / done / withdrawn

## Notes (outcomes of past tasks)

- T35 (commit-picker modal overhaul) was withdrawn and later implemented via a different
  approach (a search-enabled picker popup).
- T51 (Windows installer) was implemented with **NSIS + mingw-w64 cross-build** rather than
  cargo-wix (fully on macOS) → [docs/BUILD_WINDOWS.md](../docs/BUILD_WINDOWS.md).
- T52 (i18n: Japanese/English via a custom module) has been implemented — see `src/i18n.rs`
  and the language setting in `src/config.rs`.

## Operating Rules

- When you start a task, set its status to "in progress"; when done, delete its `.md` and
  remove its row from the table above.
- One requirement = one commit, recommended. End the commit message with a `Co-Authored-By` line.
- A task is done once its own "Acceptance Criteria" section is satisfied.
- Add new requirements as `T0X-*.md` using the template (Background / Scope / Approach /
  Acceptance Criteria / Notes) and register them in the table above.

See [CLAUDE.md](../CLAUDE.md) for development conventions and environment notes.
