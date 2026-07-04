# Changelog

## Unreleased

## v0.2.4

- [infra] Replace the NSIS-based Windows installer (cross-compiled with mingw-w64) with an
  MSI built by `cargo-wix` natively on `windows-latest`, matching the sister product
  aero-grep. The installer now creates a Start Menu shortcut only (no desktop shortcut),
  and the installer UI is English-only — see `docs/BUILD_WINDOWS.md`.

## v0.2.3

- [task_033] Localize pull and fetch operation messages via user language preference
- [task_034] Replace Japanese child-process wait error message in run_with_timeout with English
- [task_035] Localize application window title using startup language preference
- [task_036] Sort member list in Settings by name (case-insensitive ascending)
- [task_037] Replace ComboBox with filterable member picker in Add Alias dialog
- [task_038] Render icon-64.rgba as sidebar header logo instead of text placeholder
- [task_039] Apply sort to sidebar repository list and add compact sort cycle button
- [i18n-audit] Add 5 missing translation keys (home, theme_label, remote_repo_hint, working_tree) found via a systematic used-vs-defined key audit; fix a Japanese/English key-value swap in the 90-day filter label; deduplicate a "delete" call site onto the existing delete_action key
- rust-i18n migration: replace the hand-rolled match-based i18n backend with rust-i18n 3.x
  (locales/en.yml, locales/ja.yml)
