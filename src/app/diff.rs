use crate::app::{DiffMode, GitDashboardApp};
use crate::git::{ChangedFile, DiffRowKind};
use crate::worddiff::{CharOp, changed_word_ranges, diff_chars};
use egui::{Color32, RichText};
use std::collections::HashMap;

/// One row of the ref-picker list as a multi-colored LayoutJob.
fn picker_row_job(parts: &[(String, Color32, bool)]) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for (i, (text, color, mono)) in parts.iter().enumerate() {
        job.append(
            text,
            if i == 0 { 0.0 } else { 8.0 },
            egui::TextFormat {
                font_id: if *mono {
                    egui::FontId::monospace(12.0)
                } else {
                    egui::FontId::proportional(12.0)
                },
                color: *color,
                ..Default::default()
            },
        );
    }
    job
}

/// One fixed-height picker row. The LayoutJob is built only for rows inside
/// the viewport, so a 200-commit popup doesn't lay out text every frame.
fn picker_row<F>(
    ui: &mut egui::Ui,
    selected: bool,
    t: &crate::theme::Theme,
    make_parts: F,
) -> egui::Response
where
    F: FnOnce() -> Vec<(String, Color32, bool)>,
{
    let row_h = 20.0;
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_h),
        egui::Sense::click(),
    );
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if selected {
        ui.painter().rect_filled(rect, 2.0, t.bg_active);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 2.0, t.bg_hover);
    }
    let job = picker_row_job(&make_parts());
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let gy = rect.center().y - galley.rect.height() / 2.0;
    ui.painter_at(rect)
        .galley(egui::pos2(rect.left() + 4.0, gy), galley, t.text);
    resp
}

/// Searchable commit/tag picker popup anchored below `anchor`.
/// Returns the picked ref (short hash, tag name, or "WORKING_TREE").
#[allow(clippy::too_many_arguments)]
fn ref_picker_popup(
    ui: &egui::Ui,
    popup_id: egui::Id,
    anchor: &egui::Response,
    search: &mut String,
    focus_search: &mut bool,
    commits: &[crate::git::CommitSummary],
    tags: &[crate::git::TagInfo],
    branches: &[crate::git::BranchInfo],
    current: Option<&str>,
    include_working_tree: bool,
    commits_loading: bool,
    t: &crate::theme::Theme,
    lang: crate::config::Language,
) -> Option<String> {
    let mut picked: Option<String> = None;
    egui::Popup::new(popup_id, ui.ctx().clone(), anchor, ui.layer_id())
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .open_memory(None)
        .show(|ui| {
            ui.set_min_width(520.0);
            let search_resp = ui.add(
                egui::TextEdit::singleline(search)
                    .hint_text(crate::i18n::t(lang, "search_commit_hint"))
                    .desired_width(f32::INFINITY),
            );
            if *focus_search {
                search_resp.request_focus();
                *focus_search = false;
            }
            // Enter in the search box picks the top hit (recorded in display
            // order while the rows render below)
            let enter_pressed =
                search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let mut first_candidate: Option<String> = None;
            ui.add_space(4.0);

            let q = search.to_lowercase();

            // Attach each tag to its commit in the list (git log --decorate style).
            // Abbreviated hash lengths can differ between `git log` and `git tag`,
            // so fall back to prefix matching when the exact lookup misses. Tags
            // pointing outside the listed commits go to a trailing section.
            let hash_to_idx: HashMap<&str, usize> = commits
                .iter()
                .enumerate()
                .map(|(i, c)| (c.hash.as_str(), i))
                .collect();
            let mut commit_tags: HashMap<usize, Vec<&crate::git::TagInfo>> = HashMap::new();
            let mut leftover_tags: Vec<&crate::git::TagInfo> = Vec::new();
            for tg in tags {
                let idx = if tg.hash.is_empty() {
                    None
                } else {
                    hash_to_idx.get(tg.hash.as_str()).copied().or_else(|| {
                        commits.iter().position(|c| {
                            c.hash.starts_with(&tg.hash) || tg.hash.starts_with(&c.hash)
                        })
                    })
                };
                match idx {
                    Some(i) => commit_tags.entry(i).or_default().push(tg),
                    None => leftover_tags.push(tg),
                }
            }

            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;

                    if include_working_tree
                        && (q.is_empty()
                            || crate::i18n::t(lang, "uncommitted_changes_working")
                                .to_lowercase()
                                .contains(&q)
                            || "uncommitted".contains(&q)
                            || "working".contains(&q)
                            || "未コミット".contains(&q))
                    {
                        let sel = current == Some("WORKING_TREE");
                        if first_candidate.is_none() {
                            first_candidate = Some("WORKING_TREE".to_string());
                        }
                        if picker_row(ui, sel, t, || {
                            vec![(
                                crate::i18n::t(lang, "uncommitted_changes_working").to_string(),
                                t.text,
                                false,
                            )]
                        })
                        .clicked()
                        {
                            picked = Some("WORKING_TREE".to_string());
                        }
                    }

                    if commits_loading && commits.is_empty() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new(crate::i18n::t(lang, "fetching_commits"))
                                    .color(t.text_dim)
                                    .size(12.0),
                            );
                        });
                    }

                    // Branches (local + remote-tracking) are picked by name; with
                    // no query only the most recent few show, the rest via search
                    let branch_hits: Vec<&crate::git::BranchInfo> = if q.is_empty() {
                        branches.iter().take(8).collect()
                    } else {
                        branches
                            .iter()
                            .filter(|b| b.name.to_lowercase().contains(&q))
                            .collect()
                    };
                    if !branch_hits.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(crate::i18n::t(lang, "branches_header"))
                                    .color(t.text_dim)
                                    .size(10.0),
                            );
                            if q.is_empty() && branches.len() > 8 {
                                let filter_hint = match lang {
                                    crate::config::Language::English => {
                                        format!("(Total {} — filter by search)", branches.len())
                                    }
                                    crate::config::Language::Japanese => {
                                        format!("（全{}件 — 検索で絞り込み）", branches.len())
                                    }
                                };
                                ui.label(RichText::new(filter_hint).color(t.text_faint).size(10.0));
                            }
                        });
                        for b in branch_hits {
                            let sel = current == Some(b.name.as_str());
                            if first_candidate.is_none() {
                                first_candidate = Some(b.name.clone());
                            }
                            let name_color = if b.is_remote { t.text_dim } else { t.accent };
                            if picker_row(ui, sel, t, || {
                                vec![
                                    (format!("⎇ {}", b.name), name_color, false),
                                    (b.date.clone(), t.text_dim, false),
                                    (truncate(&b.message, 40), t.text, false),
                                ]
                            })
                            .clicked()
                            {
                                picked = Some(b.name.clone());
                            }
                        }
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(crate::i18n::t(lang, "stat_commits"))
                                .color(t.text_dim)
                                .size(10.0),
                        );
                    }

                    for (idx, c) in commits.iter().enumerate() {
                        let row_tags = commit_tags.get(&idx).map(Vec::as_slice).unwrap_or(&[]);
                        if !q.is_empty() {
                            let mut hay = format!(
                                "{} {} {} {}",
                                c.hash.to_lowercase(),
                                c.date.to_lowercase(),
                                c.author.to_lowercase(),
                                c.message.to_lowercase()
                            );
                            for tg in row_tags {
                                hay.push(' ');
                                hay.push_str(&tg.name.to_lowercase());
                            }
                            if !hay.contains(&q) {
                                continue;
                            }
                        }
                        // Selected either by hash or by a tag name pointing at this commit
                        let sel = current.is_some_and(|h| {
                            c.hash.starts_with(h)
                                || h.starts_with(&c.hash)
                                || row_tags.iter().any(|tg| tg.name == h)
                        });
                        if first_candidate.is_none() {
                            first_candidate = Some(c.hash.clone());
                        }
                        if picker_row(ui, sel, t, || {
                            let date_part = if c.date.len() >= 10 {
                                &c.date[..10]
                            } else {
                                c.date.as_str()
                            };
                            let mut parts = vec![
                                (c.hash.clone(), t.accent, true),
                                (date_part.to_string(), t.text_dim, false),
                            ];
                            for tg in row_tags {
                                parts.push((format!("◆ {}", tg.name), t.warning, false));
                            }
                            parts.push((truncate(&c.author, 12), t.text_dim, false));
                            parts.push((truncate(&c.message, 48), t.text, false));
                            parts
                        })
                        .clicked()
                        {
                            picked = Some(c.hash.clone());
                        }
                    }

                    // Tags pointing at commits outside the listed window (or from an
                    // old cache without hashes) stay selectable by name here
                    let leftover_hits: Vec<&&crate::git::TagInfo> = leftover_tags
                        .iter()
                        .filter(|tg| {
                            q.is_empty()
                                || tg.name.to_lowercase().contains(&q)
                                || tg.message.to_lowercase().contains(&q)
                        })
                        .collect();
                    if !leftover_hits.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(crate::i18n::t(lang, "earlier_commit_tags"))
                                .color(t.text_dim)
                                .size(10.0),
                        );
                        for tg in leftover_hits {
                            let sel = current == Some(tg.name.as_str());
                            if first_candidate.is_none() {
                                first_candidate = Some(tg.name.clone());
                            }
                            if picker_row(ui, sel, t, || {
                                vec![
                                    (format!("◆ {}", tg.name), t.warning, false),
                                    (tg.date.clone(), t.text_dim, false),
                                    (truncate(&tg.message, 48), t.text, false),
                                ]
                            })
                            .clicked()
                            {
                                picked = Some(tg.name.clone());
                            }
                        }
                    }
                });

            // Enter with a non-empty query confirms the top hit without the mouse
            if enter_pressed && !q.is_empty() && picked.is_none() {
                picked = first_candidate;
            }
        });
    if picked.is_some() {
        egui::Popup::close_id(ui.ctx(), popup_id);
    }
    picked
}

/// Short one-line description of a ref for the picker buttons.
fn describe_ref(
    commits: &[crate::git::CommitSummary],
    h: &str,
    lang: crate::config::Language,
) -> String {
    if h == "WORKING_TREE" {
        return crate::i18n::t(lang, "uncommitted_changes_working").to_string();
    }
    commits
        .iter()
        .find(|c| c.hash.starts_with(h) || h.starts_with(&c.hash))
        .map(|c| {
            let date_part = if c.date.len() >= 10 {
                &c.date[..10]
            } else {
                &c.date
            };
            format!(
                "[{}] {} {}  {}",
                c.hash,
                date_part,
                c.author,
                truncate(&c.message, 20)
            )
        })
        // Tag/branch names (and refs outside the recent-commit window) display as-is
        .unwrap_or_else(|| h.to_string())
}

/// Hover tooltip with full commit details, when the ref is a known commit.
fn commit_hover(
    commits: &[crate::git::CommitSummary],
    h: &str,
    lang: crate::config::Language,
) -> Option<String> {
    commits
        .iter()
        .find(|c| c.hash.starts_with(h) || h.starts_with(&c.hash))
        .map(|c| match lang {
            crate::config::Language::English => format!(
                "Hash: {}\nDate: {}\nAuthor: {}\n\n{}",
                c.hash, c.date, c.author, c.message
            ),
            crate::config::Language::Japanese => format!(
                "ハッシュ: {}\n日時: {}\n作者: {}\n\n{}",
                c.hash, c.date, c.author, c.message
            ),
        })
}

pub(super) fn status_color(s: &str, t: &crate::theme::Theme) -> Color32 {
    match s {
        "A" => t.success,
        "D" => t.error,
        "R" => t.warning,
        _ => t.accent,
    }
}

impl GitDashboardApp {
    pub(super) fn draw_diff_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.theme;
        let lang = self.prefs.language;
        let repo_idx = match self.diff_repo_idx {
            Some(i) => i,
            None => {
                self.viewing_diff = false;
                return;
            }
        };

        // Detect search shortcut (Ctrl/Cmd+F)
        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
            self.diff_search_visible = true;
            self.diff_search_focus_input = true;
        }

        // Row 1: navigation + breadcrumb + mode toggle
        ui.horizontal(|ui| {
            // ← repo name (click to go back)
            let repo_name = self
                .repositories
                .get(repo_idx)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| crate::i18n::t(lang, "unknown"));
            let mut back_clicked = false;
            if super::widgets::icon_button(
                ui,
                "back",
                crate::i18n::t(lang, "back_to_dashboard"),
                &t,
            )
            .clicked()
            {
                back_clicked = true;
            }
            ui.add_space(2.0);
            // Fullscreen toggle (OS window) for maximum diff area
            let fs_tip = if self.diff_fullscreen {
                crate::i18n::t(lang, "exit_fullscreen")
            } else {
                crate::i18n::t(lang, "enter_fullscreen")
            };
            if ui
                .selectable_label(self.diff_fullscreen, egui::RichText::new("⛶").monospace())
                .on_hover_text(fs_tip)
                .clicked()
            {
                self.diff_fullscreen = !self.diff_fullscreen;
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.diff_fullscreen));
            }
            ui.add_space(2.0);
            let label_resp = ui.link(egui::RichText::new(repo_name).color(t.text_dim));
            if label_resp.clicked() {
                back_clicked = true;
            }
            if back_clicked {
                self.viewing_diff = false;
                if self.diff_fullscreen {
                    self.diff_fullscreen = false;
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                }
            }

            // Breadcrumb: target hash + commit message
            if let Some(target_hash) = &self.diff_target.clone() {
                let crumb = if target_hash == "WORKING_TREE" {
                    format!("/ {}", crate::i18n::t(lang, "uncommitted_changes_working"))
                } else {
                    self.diff_commit_list
                        .iter()
                        .find(|c| {
                            c.hash.starts_with(target_hash) || target_hash.starts_with(&c.hash)
                        })
                        .map(|c| format!("/ {}  {}", c.hash, truncate(&c.message, 40)))
                        .unwrap_or_else(|| format!("/ {target_hash}"))
                };
                ui.label(RichText::new(crumb).color(t.text_dim).size(12.0));
            }

            // Mode toggle aligned to the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let range_sel = self.diff_mode == DiffMode::Range;
                if ui
                    .selectable_label(range_sel, crate::i18n::t(lang, "range_compare"))
                    .clicked()
                    && !range_sel
                {
                    self.diff_mode = DiffMode::Range;
                }
                let single_sel = self.diff_mode == DiffMode::Single;
                if ui
                    .selectable_label(single_sel, crate::i18n::t(lang, "single_commit_changes"))
                    .clicked()
                    && !single_sel
                {
                    self.diff_mode = DiffMode::Single;
                    self.diff_base = None;
                    if self.diff_target.is_some() {
                        self.reload_diff_files(ctx.clone());
                    }
                }
            });
        });
        ui.separator();

        // Row 2: ref selector UI (searchable picker covering commits/tags/branches)
        let (picker_tags, picker_branches): (
            Vec<crate::git::TagInfo>,
            Vec<crate::git::BranchInfo>,
        ) = self
            .diff_repo_idx
            .and_then(|i| self.repo_cache.get(&i))
            .and_then(|r| r.as_ref().ok())
            .map(|d| (d.tags.clone(), d.branches.clone()))
            .unwrap_or_default();
        let mut new_base: Option<String> = None;
        let mut new_target: Option<String> = None;
        ui.horizontal(|ui| {
            // Picker buttons stretch with the window (clamped for readability);
            // ~260px covers the fixed elements (labels, swap/compare buttons, spacing)
            let n_pickers = if self.diff_mode == DiffMode::Range {
                2.0
            } else {
                1.0
            };
            let picker_w = ((ui.available_width() - 260.0) / n_pickers).clamp(300.0, 640.0);
            // base selector (range mode only)
            if self.diff_mode == DiffMode::Range {
                ui.label(RichText::new("base:").color(t.text_dim).size(12.0));
                let base_label = self
                    .diff_base
                    .as_deref()
                    .map(|h| describe_ref(&self.diff_commit_list, h, lang))
                    .unwrap_or_else(|| crate::i18n::t(lang, "select_placeholder").to_string());
                let base_btn = ui.add_sized(
                    [picker_w, 24.0],
                    egui::Button::new(RichText::new(base_label).size(12.0)),
                );
                let base_btn = base_btn.on_hover_cursor(egui::CursorIcon::PointingHand);
                let base_btn = if let Some(tip) = self
                    .diff_base
                    .as_deref()
                    .and_then(|h| commit_hover(&self.diff_commit_list, h, lang))
                {
                    base_btn.on_hover_text(tip)
                } else {
                    base_btn
                };
                let base_popup_id = ui.make_persistent_id("diff_base_picker");
                if base_btn.clicked() {
                    self.diff_picker_search.clear();
                    self.diff_picker_focus = true;
                    egui::Popup::toggle_id(ui.ctx(), base_popup_id);
                }
                new_base = ref_picker_popup(
                    ui,
                    base_popup_id,
                    &base_btn,
                    &mut self.diff_picker_search,
                    &mut self.diff_picker_focus,
                    &self.diff_commit_list,
                    &picker_tags,
                    &picker_branches,
                    self.diff_base.as_deref(),
                    false,
                    self.diff_commits_loading,
                    &t,
                    self.prefs.language,
                );

                // Swap base/target button
                if super::widgets::icon_button(
                    ui,
                    "swap",
                    crate::i18n::t(lang, "swap_base_target"),
                    &t,
                )
                .clicked()
                {
                    std::mem::swap(&mut self.diff_base, &mut self.diff_target);
                    if self.diff_target.is_some() {
                        self.reload_diff_files(ctx.clone());
                    }
                }
            }

            // target selector
            ui.label(RichText::new("target:").color(t.text_dim).size(12.0));
            let target_label = self
                .diff_target
                .as_deref()
                .map(|h| describe_ref(&self.diff_commit_list, h, lang))
                .unwrap_or_else(|| crate::i18n::t(lang, "select_placeholder").to_string());
            let target_btn = ui.add_sized(
                [picker_w, 24.0],
                egui::Button::new(RichText::new(target_label).size(12.0)),
            );
            let target_btn = target_btn.on_hover_cursor(egui::CursorIcon::PointingHand);
            let target_btn = match self.diff_target.as_deref() {
                Some("WORKING_TREE") => {
                    target_btn.on_hover_text(crate::i18n::t(lang, "working_tree_hover"))
                }
                Some(h) => {
                    if let Some(tip) = commit_hover(&self.diff_commit_list, h, lang) {
                        target_btn.on_hover_text(tip)
                    } else {
                        target_btn
                    }
                }
                None => target_btn,
            };
            let target_popup_id = ui.make_persistent_id("diff_target_picker");
            if target_btn.clicked() {
                self.diff_picker_search.clear();
                self.diff_picker_focus = true;
                egui::Popup::toggle_id(ui.ctx(), target_popup_id);
            }
            new_target = ref_picker_popup(
                ui,
                target_popup_id,
                &target_btn,
                &mut self.diff_picker_search,
                &mut self.diff_picker_focus,
                &self.diff_commit_list,
                &picker_tags,
                &picker_branches,
                self.diff_target.as_deref(),
                true,
                self.diff_commits_loading,
                &t,
                self.prefs.language,
            );

            // Re-run the comparison (picking base/target already runs it
            // automatically, so this is just an explicit refresh)
            if self.diff_mode == DiffMode::Range
                && self.diff_target.is_some()
                && super::widgets::icon_button(
                    ui,
                    "refresh",
                    crate::i18n::t(lang, "re_run_compare"),
                    &t,
                )
                .clicked()
            {
                self.reload_diff_files(ctx.clone());
            }

            // merge-base comparison toggle (range mode only)
            if self.diff_mode == DiffMode::Range {
                let mut three = self.diff_three_dot;
                if ui
                    .checkbox(&mut three, crate::i18n::t(lang, "merge_base_checkbox"))
                    .on_hover_text(crate::i18n::t(lang, "merge_base_tooltip"))
                    .clicked()
                {
                    self.diff_three_dot = three;
                    if self.diff_target.is_some() {
                        self.reload_diff_files(ctx.clone());
                    }
                }

                // Recent comparisons of this repository, re-applied in one click
                let repo_path_str = self
                    .diff_repo_idx
                    .and_then(|i| self.repositories.get(i))
                    .map(|r| r.path.to_string_lossy().to_string());
                if let Some(rp) = repo_path_str {
                    let entries: Vec<(String, String)> = self
                        .prefs
                        .recent_compares
                        .iter()
                        .filter(|r| r.repo_path == rp)
                        .map(|r| (r.base.clone(), r.target.clone()))
                        .collect();
                    if !entries.is_empty() {
                        ui.menu_button(crate::i18n::t(lang, "history_menu"), |ui| {
                            for (b, tg) in &entries {
                                let label = format!("{} → {}", truncate(b, 16), truncate(tg, 16));
                                if ui.button(label).clicked() {
                                    self.diff_base = Some(b.clone());
                                    self.diff_target = Some(tg.clone());
                                    self.reload_diff_files(ctx.clone());
                                    ui.close();
                                }
                            }
                        });
                    }
                }
            }
        });
        if let Some(b) = new_base {
            self.diff_base = Some(b);
            if self.diff_target.is_some() {
                self.reload_diff_files(ctx.clone());
            }
        }
        if let Some(tg) = new_target {
            self.diff_target = Some(tg);
            self.reload_diff_files(ctx.clone());
        }

        // Reversed range warning: a base newer than the target silently shows
        // the diff backwards, which is easy to miss
        if self.diff_mode == DiffMode::Range
            && let (Some(b), Some(tg)) = (self.diff_base.as_deref(), self.diff_target.as_deref())
        {
            let date_of = |h: &str| {
                self.diff_commit_list
                    .iter()
                    .find(|c| c.hash.starts_with(h) || h.starts_with(&c.hash))
                    .map(|c| c.date.clone())
            };
            if let (Some(bd), Some(td)) = (date_of(b), date_of(tg))
                && bd > td
            {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(crate::i18n::t(lang, "reversed_diff_warning"))
                            .color(t.warning)
                            .size(11.0),
                    );
                    if ui
                        .link(RichText::new(crate::i18n::t(lang, "swap_link")).size(11.0))
                        .clicked()
                    {
                        std::mem::swap(&mut self.diff_base, &mut self.diff_target);
                        self.reload_diff_files(ctx.clone());
                    }
                });
            }
        }
        ui.separator();

        // No target selected
        if self.diff_target.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(crate::i18n::t(lang, "select_commits_to_compare"))
                        .color(t.text_dim)
                        .size(14.0),
                );
            });
            return;
        }

        // Main content: left pane (file list) | right pane (diff panel)
        // Left pane: changed files. Collapsed state is a slim rail with a » button;
        // it uses a different panel id so the expanded width stays remembered.
        if self.diff_files_collapsed {
            egui::Panel::left("diff_file_list_collapsed")
                .resizable(false)
                .exact_size(30.0)
                .frame(egui::Frame::NONE.fill(t.bg_sidebar))
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(4.0); // center 22px icon in 30px rail
                        if super::widgets::icon_button(
                            ui,
                            "expand",
                            crate::i18n::t(lang, "show_file_list"),
                            &t,
                        )
                        .clicked()
                        {
                            self.diff_files_collapsed = false;
                        }
                    });
                });
        } else {
            egui::Panel::left("diff_file_list")
                .resizable(true)
                .min_size(180.0)
                .default_size(240.0)
                .max_size(400.0)
                .frame(egui::Frame::NONE.fill(t.bg_sidebar))
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(crate::i18n::t(lang, "changed_files_header"))
                                .strong()
                                .color(t.text_dim)
                                .size(11.0),
                        );
                        // Collapse toggle (« at the header's right edge, same as the sidebar)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(4.0);
                            if super::widgets::icon_button(
                                ui,
                                "collapse",
                                crate::i18n::t(lang, "hide_file_list_tooltip"),
                                &t,
                            )
                            .clicked()
                            {
                                self.diff_files_collapsed = true;
                            }
                        });
                    });
                    ui.add_space(4.0);

                    if self.diff_loading_files {
                        ui.centered_and_justified(|ui| {
                            ui.spinner();
                        });
                        return;
                    }

                    // Build the tree once per changed-files load (the cache is
                    // invalidated wherever diff_changed_files is assigned); the
                    // previous code cloned the file list and rebuilt the tree
                    // on every frame.
                    if self.diff_tree_cache.is_none()
                        && let Some(Ok(files)) = &self.diff_changed_files
                        && !files.is_empty()
                    {
                        let mut tree = build_diff_tree(files);
                        compress_diff_tree(&mut tree);
                        collapse_top_dirs_if_large(
                            &tree,
                            files.len(),
                            &mut self.diff_collapsed_dirs,
                        );
                        self.diff_tree_cache = Some(tree);
                    }
                    let files_error: Option<String> = match &self.diff_changed_files {
                        Some(Err(e)) => Some(e.clone()),
                        _ => None,
                    };
                    let files_empty =
                        matches!(&self.diff_changed_files, Some(Ok(f)) if f.is_empty());

                    if self.diff_changed_files.is_none() {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                RichText::new(crate::i18n::t(lang, "loading")).color(t.text_dim),
                            );
                        });
                    } else if let Some(e) = files_error {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            let err_msg = match lang {
                                crate::config::Language::English => format!("Error: {e}"),
                                crate::config::Language::Japanese => format!("エラー: {e}"),
                            };
                            ui.label(RichText::new(err_msg).color(t.error).size(11.0));
                        });
                    } else if files_empty {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                RichText::new(crate::i18n::t(lang, "no_changes"))
                                    .color(t.text_dim)
                                    .size(12.0),
                            );
                        });
                    } else if let Some(tree) = self.diff_tree_cache.take() {
                        egui::ScrollArea::both().show(ui, |ui| {
                            let mut load_file: Option<String> = None;
                            let selected = self.diff_selected_file.clone();
                            for node in &tree {
                                draw_tree_node(
                                    ui,
                                    node,
                                    8.0,
                                    &t,
                                    lang,
                                    &mut self.diff_collapsed_dirs,
                                    selected.as_deref(),
                                    &mut load_file,
                                );
                            }
                            if let Some(file) = load_file {
                                self.diff_full_file = false;
                                self.load_file_diff(file, ctx.clone());
                            }
                        });
                        self.diff_tree_cache = Some(tree);
                    }
                });
        } // end collapsed/expanded file list

        // Right pane: split diff
        let central_frame = egui::Frame::NONE.fill(t.bg).inner_margin(0.0);
        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ui, |ui| {
                if self.diff_selected_file.is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new(crate::i18n::t(lang, "select_file_hint"))
                                .color(t.text_dim)
                                .size(13.0),
                        );
                    });
                    return;
                }

                if self.diff_loading_content {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new(crate::i18n::t(lang, "fetching_diff")).color(t.text_dim),
                        );
                    });
                    return;
                }

                match &self.diff_file_content.clone() {
                    None => {
                        ui.centered_and_justified(|ui| {
                            ui.spinner();
                        });
                    }
                    Some(Err(e)) => {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            let err_msg = match lang {
                                crate::config::Language::English => format!("Error: {e}"),
                                crate::config::Language::Japanese => format!("エラー: {e}"),
                            };
                            ui.label(RichText::new(err_msg).color(t.error));
                        });
                    }
                    Some(Ok(fd)) => {
                        let fd = fd.clone();
                        if self.diff_search_visible {
                            ui.add_space(4.0);
                            self.draw_search_bar(ui);
                            ui.separator();
                        }
                        // File name header
                        let base_label = if self.diff_mode == DiffMode::Single {
                            self.diff_target
                                .as_deref()
                                .map(|h| {
                                    if h == "WORKING_TREE" {
                                        "HEAD".to_string()
                                    } else {
                                        format!("{h}^")
                                    }
                                })
                                .unwrap_or_default()
                        } else {
                            self.diff_base.clone().unwrap_or_default()
                        };
                        let target_label = if self.diff_target.as_deref() == Some("WORKING_TREE") {
                            crate::i18n::t(lang, "working_tree").to_string()
                        } else {
                            self.diff_target.clone().unwrap_or_default()
                        };

                        self.draw_split_diff(ui, &fd, &base_label, &target_label);
                    }
                }
            });
    }
}

/// Render a single row in the file list
#[derive(Debug, Clone)]
pub(crate) enum DiffTreeNode {
    Dir {
        name: String,
        full_path: String,
        children: Vec<DiffTreeNode>,
    },
    File {
        name: String,
        file: ChangedFile,
    },
}

pub(super) fn build_diff_tree(files: &[ChangedFile]) -> Vec<DiffTreeNode> {
    let mut root_children: Vec<DiffTreeNode> = Vec::new();

    for f in files {
        let parts: Vec<&str> = f.path.split('/').collect();
        insert_into_tree(&mut root_children, &parts, 0, "", f);
    }

    sort_tree_nodes(&mut root_children);
    root_children
}

fn insert_into_tree(
    nodes: &mut Vec<DiffTreeNode>,
    parts: &[&str],
    index: usize,
    parent_path: &str,
    file: &ChangedFile,
) {
    if index == parts.len() - 1 {
        nodes.push(DiffTreeNode::File {
            name: parts[index].to_string(),
            file: file.clone(),
        });
        return;
    }

    let dir_name = parts[index];
    let full_path = if parent_path.is_empty() {
        dir_name.to_string()
    } else {
        format!("{parent_path}/{dir_name}")
    };

    if let Some(pos) = nodes.iter().position(|n| match n {
        DiffTreeNode::Dir { name, .. } => name == dir_name,
        _ => false,
    }) {
        if let DiffTreeNode::Dir { children, .. } = &mut nodes[pos] {
            insert_into_tree(children, parts, index + 1, &full_path, file);
        }
    } else {
        let mut children = Vec::new();
        insert_into_tree(&mut children, parts, index + 1, &full_path, file);
        nodes.push(DiffTreeNode::Dir {
            name: dir_name.to_string(),
            full_path,
            children,
        });
    }
}

fn sort_tree_nodes(nodes: &mut [DiffTreeNode]) {
    nodes.sort_by(|a, b| match (a, b) {
        (DiffTreeNode::Dir { name: a_name, .. }, DiffTreeNode::Dir { name: b_name, .. }) => {
            a_name.cmp(b_name)
        }
        (DiffTreeNode::File { name: a_name, .. }, DiffTreeNode::File { name: b_name, .. }) => {
            a_name.cmp(b_name)
        }
        (DiffTreeNode::Dir { .. }, DiffTreeNode::File { .. }) => std::cmp::Ordering::Less,
        (DiffTreeNode::File { .. }, DiffTreeNode::Dir { .. }) => std::cmp::Ordering::Greater,
    });

    for node in nodes.iter_mut() {
        if let DiffTreeNode::Dir { children, .. } = node {
            sort_tree_nodes(children);
        }
    }
}

/// Start with all top-level directories collapsed when the change set is
/// huge, so the first paint of the tree stays manageable.
pub(super) fn collapse_top_dirs_if_large(
    tree: &[DiffTreeNode],
    file_count: usize,
    collapsed: &mut std::collections::HashSet<String>,
) {
    if file_count <= 500 {
        return;
    }
    for node in tree {
        if let DiffTreeNode::Dir { full_path, .. } = node {
            collapsed.insert(full_path.clone());
        }
    }
}

pub(super) fn compress_diff_tree(nodes: &mut [DiffTreeNode]) {
    for node in nodes.iter_mut() {
        if let DiffTreeNode::Dir {
            name,
            full_path,
            children,
        } = node
        {
            compress_diff_tree(children);

            while children.len() == 1 {
                if let DiffTreeNode::Dir {
                    name: child_name,
                    full_path: child_full_path,
                    children: child_children,
                } = &children[0]
                {
                    *name = format!("{name}/{child_name}");
                    *full_path = child_full_path.clone();
                    *children = child_children.clone();
                } else {
                    break;
                }
            }
        }
    }
}

/// Render one node of a changed-files tree. Decoupled from the diff-view
/// state so the commit-detail panel can reuse it with its own collapse set.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_tree_node(
    ui: &mut egui::Ui,
    node: &DiffTreeNode,
    indent: f32,
    t: &crate::theme::Theme,
    lang: crate::config::Language,
    collapsed_dirs: &mut std::collections::HashSet<String>,
    selected_file: Option<&str>,
    clicked_file: &mut Option<String>,
) {
    let t = *t;
    // Rows outside the viewport only reserve their height: laying out two
    // galleys per node for thousands of offscreen files froze large change
    // sets. Children still recurse — a Dir header above the viewport can
    // have visible children. (Offscreen rows reserve the viewport width, so
    // the horizontal extent may briefly shrink while long rows are scrolled
    // out; harmless next to the per-frame cost.)
    let height = 24.0;
    let cursor_top = ui.cursor().top();
    let clip = ui.clip_rect();
    let row_visible = cursor_top <= clip.bottom() && cursor_top + height >= clip.top();
    match node {
        DiffTreeNode::Dir {
            name,
            full_path,
            children,
        } => {
            let is_collapsed = collapsed_dirs.contains(full_path);

            if !row_visible {
                ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), height),
                    egui::Sense::hover(),
                );
                if !is_collapsed {
                    for child in children {
                        draw_tree_node(
                            ui,
                            child,
                            indent + 12.0,
                            &t,
                            lang,
                            collapsed_dirs,
                            selected_file,
                            clicked_file,
                        );
                    }
                }
                return;
            }

            let name_job = egui::text::LayoutJob::simple_singleline(
                name.to_string(),
                egui::FontId::proportional(12.0),
                t.text,
            );
            let name_galley = ui.fonts_mut(|f| f.layout_job(name_job));
            let name_w = name_galley.rect.width();
            let desired_width = indent + 16.0 + name_w + 16.0;
            let width = f32::max(ui.available_width(), desired_width);

            let (rect, resp) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

            if resp.hovered() {
                ui.painter().rect_filled(rect, 2.0, t.bg_hover);
            }

            if resp.clicked() {
                if is_collapsed {
                    collapsed_dirs.remove(full_path);
                } else {
                    collapsed_dirs.insert(full_path.clone());
                }
            }

            let painter = ui.painter_at(rect);
            let x = rect.left() + indent;
            let cy = rect.center().y;

            let toggle_symbol = if is_collapsed { "▶" } else { "▼" };
            painter.text(
                egui::pos2(x + 4.0, cy),
                egui::Align2::LEFT_CENTER,
                toggle_symbol,
                egui::FontId::proportional(9.0),
                t.text_dim,
            );

            painter.text(
                egui::pos2(x + 16.0, cy),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(12.0),
                t.text,
            );

            if !is_collapsed {
                for child in children {
                    draw_tree_node(
                        ui,
                        child,
                        indent + 12.0,
                        &t,
                        lang,
                        collapsed_dirs,
                        selected_file,
                        clicked_file,
                    );
                }
            }
        }
        DiffTreeNode::File { name, file } => {
            if !row_visible {
                ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), height),
                    egui::Sense::hover(),
                );
                return;
            }

            let is_selected = selected_file == Some(file.path.as_str());
            let bg = if is_selected {
                t.bg_active
            } else {
                Color32::TRANSPARENT
            };

            let display_name = if let Some(ref old) = file.old_path {
                format!("{name} (← {old})")
            } else {
                name.clone()
            };
            let name_job = egui::text::LayoutJob::simple_singleline(
                display_name.clone(),
                egui::FontId::proportional(12.0),
                t.text,
            );
            let name_galley = ui.fonts_mut(|f| f.layout_job(name_job));
            let name_w = name_galley.rect.width();

            let stats = format!("+{} -{}", file.additions, file.deletions);
            let stats_job = egui::text::LayoutJob::simple_singleline(
                stats.clone(),
                egui::FontId::proportional(9.0),
                t.text,
            );
            let stats_galley = ui.fonts_mut(|f| f.layout_job(stats_job));
            let stats_w = stats_galley.rect.width();

            let desired_width = indent + 16.0 + name_w + 16.0 + stats_w + 8.0;
            let width = f32::max(ui.available_width(), desired_width);

            let (rect, resp) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

            if resp.hovered() || is_selected {
                ui.painter()
                    .rect_filled(rect, 2.0, if is_selected { bg } else { t.bg_hover });
            }

            if resp.clicked() {
                *clicked_file = Some(file.path.clone());
            }
            resp.context_menu(|ui| {
                if ui.button(crate::i18n::t(lang, "copy_path")).clicked() {
                    ui.output_mut(|o| {
                        o.commands
                            .push(egui::OutputCommand::CopyText(file.path.clone()))
                    });
                    ui.close();
                }
            });

            let painter = ui.painter_at(rect);
            let x = rect.left() + indent + 4.0;
            let cy = rect.center().y;

            painter.text(
                egui::pos2(x, cy),
                egui::Align2::LEFT_CENTER,
                &file.status,
                egui::FontId::proportional(10.0),
                status_color(&file.status, &t),
            );

            painter.text(
                egui::pos2(x + 14.0, cy - 1.0),
                egui::Align2::LEFT_CENTER,
                &display_name,
                egui::FontId::proportional(12.0),
                if is_selected { t.text } else { t.text_dim },
            );

            painter.text(
                egui::pos2(rect.right() - 4.0, cy),
                egui::Align2::RIGHT_CENTER,
                &stats,
                egui::FontId::proportional(9.0),
                t.text_dim,
            );
        }
    }
}

/// Display-column width of a string (ASCII = 1, CJK/wide = 2 columns).
pub(super) fn str_cols(s: &str) -> usize {
    s.chars()
        .map(|c| if (c as u32) >= 0x1100 { 2 } else { 1 })
        .sum()
}

fn layout_tokens(
    tokens: &[crate::syntax::Token],
    font_id: egui::FontId,
    row_kind: crate::git::DiffRowKind,
    t: &crate::theme::Theme,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();

    let default_color = t.text;

    for token in tokens {
        let color = match token.kind {
            crate::syntax::TokenKind::Text => default_color,
            crate::syntax::TokenKind::Punctuation => match row_kind {
                crate::git::DiffRowKind::Context => t.syntax_punctuation,
                _ => default_color,
            },
            crate::syntax::TokenKind::Keyword => t.syntax_keyword,
            crate::syntax::TokenKind::Type => t.syntax_type,
            crate::syntax::TokenKind::String => t.syntax_string,
            crate::syntax::TokenKind::Comment => t.syntax_comment,
            crate::syntax::TokenKind::Number => t.syntax_number,
        };

        job.append(
            &token.text,
            0.0,
            egui::TextFormat {
                font_id: font_id.clone(),
                color,
                ..Default::default()
            },
        );
    }

    job
}

/// Paint one side (old or new) of a diff row: word-level change highlights
/// behind the text, then the laid-out code itself. Highlight x positions come
/// from the galley so CJK / tab glyph widths can never push the highlight past
/// the actual text.
#[allow(clippy::too_many_arguments)]
fn paint_code_side(
    ui: &egui::Ui,
    painter: &egui::Painter,
    clip: egui::Rect,
    row_rect: egui::Rect,
    x: f32,
    tokens: Option<&[crate::syntax::Token]>,
    text: Option<&str>,
    font: egui::FontId,
    kind: crate::git::DiffRowKind,
    emph: &[(usize, usize)],
    hl_color: Color32,
    t: &crate::theme::Theme,
) {
    if tokens.is_none() && text.is_none() {
        return;
    }
    let galley = if let Some(tokens) = tokens {
        let job = layout_tokens(tokens, font, kind, t);
        ui.fonts_mut(|f| f.layout_job(job))
    } else {
        let txt = text.unwrap_or_default().to_string();
        ui.fonts_mut(|f| f.layout_no_wrap(txt, font, t.text))
    };
    for &(c0, c1) in emph {
        let x0 = galley.pos_from_cursor(egui::text::CCursor::new(c0)).min.x;
        let x1 = galley.pos_from_cursor(egui::text::CCursor::new(c1)).min.x;
        let hl = clip.intersect(egui::Rect::from_min_max(
            egui::pos2(x + x0, row_rect.top() + 1.0),
            egui::pos2(x + x1, row_rect.bottom() - 1.0),
        ));
        if hl.width() > 0.0 {
            painter.rect_filled(hl, 2.0, hl_color);
        }
    }
    let gy = row_rect.center().y - galley.rect.height() / 2.0;
    painter.galley(egui::pos2(x, gy), galley, t.text);
}

impl GitDashboardApp {
    /// Render the main split-diff panel
    fn draw_split_diff(
        &mut self,
        ui: &mut egui::Ui,
        fd: &crate::git::FileDiff,
        base_label: &str,
        target_label: &str,
    ) {
        let t = self.theme;
        let lang = self.prefs.language;
        if fd.is_binary {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(crate::i18n::t(lang, "binary_file_no_diff"))
                        .color(t.text_dim)
                        .size(13.0),
                );
            });
            return;
        }

        let line_height = 18.0;
        let gutter_w = 44.0; // line-number gutter width
        let code_font = egui::FontId::monospace(12.0);
        // Monospace glyph advance width (true per-char width), for content-width and
        // char-index → x mapping. Using bytes/approx caused excessive horizontal scroll.
        let cw = ui.fonts_mut(|f| f.glyph_width(&code_font, 'm')).max(1.0);

        // File name bar
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(&fd.path).color(t.text).size(12.0).monospace());
            if ui
                .small_button(crate::i18n::t(lang, "copy_btn"))
                .on_hover_text(crate::i18n::t(lang, "copy_path_tooltip"))
                .clicked()
            {
                ui.output_mut(|o| {
                    o.commands
                        .push(egui::OutputCommand::CopyText(fd.path.clone()))
                });
            }
            // Position within the changed-file list ([ / ] cycles through)
            if let Some(Ok(files)) = self.diff_changed_files.as_ref()
                && let Some(pos) = files.iter().position(|f| f.path == fd.path)
            {
                ui.label(
                    RichText::new(format!("{}/{}", pos + 1, files.len()))
                        .color(t.text_dim)
                        .size(11.0),
                )
                .on_hover_text(crate::i18n::t(lang, "prev_next_file_hint"));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let mut show_blame = self.diff_show_blame;
                if ui.checkbox(&mut show_blame, "Blame").clicked() {
                    self.diff_show_blame = show_blame;
                    self.prefs.diff_show_blame = show_blame;
                    let _ = crate::config::save_preferences(&self.prefs);
                    if show_blame {
                        self.load_file_blame(ui.ctx().clone());
                    } else {
                        self.diff_blame_content = None;
                    }
                }

                ui.add_space(8.0);
                let mut full = self.diff_full_file;
                if ui
                    .checkbox(&mut full, crate::i18n::t(lang, "full_file_checkbox"))
                    .clicked()
                {
                    self.diff_full_file = full;
                    self.prefs.diff_full_file = full;
                    let _ = crate::config::save_preferences(&self.prefs);
                    let path = fd.path.clone();
                    self.load_file_diff(path, ui.ctx().clone());
                }

                ui.add_space(8.0);
                let mut ignore_ws = self.diff_ignore_whitespace;
                if ui
                    .checkbox(&mut ignore_ws, crate::i18n::t(lang, "ignore_ws_checkbox"))
                    .clicked()
                {
                    self.diff_ignore_whitespace = ignore_ws;
                    self.prefs.diff_ignore_whitespace = ignore_ws;
                    let _ = crate::config::save_preferences(&self.prefs);
                    let path = fd.path.clone();
                    self.load_file_diff(path, ui.ctx().clone());
                }
            });
        });
        // Display width in monospace columns, computed once on FileDiffLoaded
        // (scanning every char of every row per frame cost milliseconds on
        // large diffs — see the per-frame-computation invariant)
        let max_chars = self.diff_max_cols;

        if fd.rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(crate::i18n::t(lang, "no_diff"))
                        .color(t.text_dim)
                        .size(13.0),
                );
            });
            return;
        }

        let show_detail = if let Some(idx) = self.diff_selected_line_idx {
            idx < fd.rows.len()
        } else {
            false
        };

        let available_h = ui.available_height();
        let main_h = if show_detail {
            (available_h - 100.0).max(100.0)
        } else {
            available_h
        };

        // 50/50 split: fixed panes, single vertical scroll, shared manual horizontal offset
        let blame_w = if self.diff_show_blame { 120.0 } else { 0.0 };
        let code_w = (max_chars as f32 * cw + 12.0).max(120.0);
        let avail_w = ui.available_width();
        let pane_w = (avail_w - 1.0) / 2.0;

        // Fixed header (not scrolled)
        ui.horizontal(|ui| {
            let (lr, _) = ui.allocate_exact_size(egui::vec2(pane_w, 24.0), egui::Sense::hover());
            ui.painter().rect_filled(lr, 0.0, t.bg_sidebar);
            if self.diff_show_blame {
                ui.painter().text(
                    egui::pos2(lr.left() + 6.0, lr.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Blame",
                    egui::FontId::proportional(11.0),
                    t.text_dim,
                );
            }
            let old_label = format!("{}{base_label}", crate::i18n::t(lang, "old_label_prefix"));
            ui.painter().text(
                egui::pos2(lr.left() + blame_w + gutter_w + 6.0, lr.center().y),
                egui::Align2::LEFT_CENTER,
                old_label,
                egui::FontId::proportional(11.0),
                t.text_dim,
            );
            let (div, _) = ui.allocate_exact_size(egui::vec2(1.0, 24.0), egui::Sense::hover());
            ui.painter().rect_filled(div, 0.0, t.border);
            let (rr, _) = ui.allocate_exact_size(egui::vec2(pane_w, 24.0), egui::Sense::hover());
            ui.painter().rect_filled(rr, 0.0, t.bg_sidebar);
            let new_label = format!("{}{target_label}", crate::i18n::t(lang, "new_label_prefix"));
            ui.painter().text(
                egui::pos2(rr.left() + gutter_w + 6.0, rr.center().y),
                egui::Align2::LEFT_CENTER,
                new_label,
                egui::FontId::proportional(11.0),
                t.text_dim,
            );
        });

        let scroll_to_idx = self.diff_search_scroll_to;
        let click_result = std::cell::Cell::new(None::<usize>);
        let matches = self.diff_search_matches_cached();

        // Word-level (intra-line) change highlight colors — denser than the line bg,
        // GitHub / Claude Code style. Drawn behind the changed characters.
        let (word_hl_removed, word_hl_added) = if t.is_dark {
            (
                Color32::from_rgba_unmultiplied(t.removed.r(), t.removed.g(), t.removed.b(), 80),
                Color32::from_rgba_unmultiplied(t.added.r(), t.added.g(), t.added.b(), 80),
            )
        } else {
            // Light themes: translucent saturated red/green muddies the dark syntax
            // colors (comments especially); use opaque light tints one step deeper
            // than the line background instead, GitHub-style.
            (
                Color32::from_rgb(0xf4, 0xb6, 0xb1),
                Color32::from_rgb(0xa9, 0xdc, 0xb5),
            )
        };

        // Horizontal scroll bookkeeping (single shared offset; both panes stay in sync)
        let code_content_w = code_w; // = max(max_chars*7.2 + 12, 120)
        let left_code_vp = (pane_w - blame_w - gutter_w).max(1.0);
        let right_code_vp = (pane_w - gutter_w).max(1.0);
        let max_hscroll = (code_content_w - left_code_vp.min(right_code_vp)).max(0.0);
        self.diff_hscroll = self.diff_hscroll.clamp(0.0, max_hscroll);
        let hscroll = self.diff_hscroll;

        // Reserve room under the rows for the drawn horizontal scrollbar
        let hbar_h = 12.0;
        let main_h = if max_hscroll > 0.0 {
            (main_h - hbar_h - 4.0).max(80.0)
        } else {
            main_h
        };

        let scroll_out = egui::ScrollArea::vertical()
            .id_salt("diff_vsroll")
            .max_height(main_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                // Each row spans the full width; left/right halves are painted with a shared
                // horizontal offset and per-pane clipping so the divider stays fixed.
                for (row_idx, row) in fd.rows.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(avail_w, line_height),
                        egui::Sense::click(),
                    );
                    if resp.clicked() {
                        click_result.set(Some(row_idx));
                    }
                    if Some(row_idx) == scroll_to_idx {
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                    }
                    // Skip painting (and per-row diff computation) for offscreen rows;
                    // allocation above keeps scroll geometry and click targets intact.
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }

                    // Right-click: copy either side of the row (the split diff
                    // is painter-rendered, so normal text selection can't work)
                    resp.context_menu(|ui| {
                        if let Some(l) = row.left_text.as_deref()
                            && ui.button(crate::i18n::t(lang, "copy_old_line")).clicked()
                        {
                            ui.output_mut(|o| {
                                o.commands
                                    .push(egui::OutputCommand::CopyText(l.to_string()))
                            });
                            ui.close();
                        }
                        if let Some(r) = row.right_text.as_deref()
                            && ui.button(crate::i18n::t(lang, "copy_new_line")).clicked()
                        {
                            ui.output_mut(|o| {
                                o.commands
                                    .push(egui::OutputCommand::CopyText(r.to_string()))
                            });
                            ui.close();
                        }
                        if ui.button(crate::i18n::t(lang, "copy_path")).clicked() {
                            ui.output_mut(|o| {
                                o.commands
                                    .push(egui::OutputCommand::CopyText(fd.path.clone()))
                            });
                            ui.close();
                        }
                    });

                    // All rows render in regular weight; only intra-line changed chars
                    // (on Modified rows) get a subtle medium emphasis via faux-bold overlay.
                    let current_font = code_font.clone();
                    let is_indent_only =
                        if let (Some(l), Some(r)) = (&row.left_text, &row.right_text) {
                            let lt = l.trim_start();
                            let rt = r.trim_start();
                            lt == rt && !lt.is_empty()
                        } else {
                            false
                        };
                    let cy = rect.center().y;

                    // Intra-line changed-char ranges (Modified rows only), memoized
                    // per row index: the LCS would otherwise run twice per visible
                    // row every frame. Cleared on FileDiffLoaded.
                    let (left_emph, right_emph) = if matches!(row.kind, DiffRowKind::Modified)
                        && let (Some(l), Some(r)) = (&row.left_text, &row.right_text)
                    {
                        self.diff_emph_cache
                            .entry(row_idx)
                            .or_insert_with(|| {
                                (
                                    changed_word_ranges(l, r, true),
                                    changed_word_ranges(l, r, false),
                                )
                            })
                            .clone()
                    } else {
                        (Vec::new(), Vec::new())
                    };

                    // Pane geometry
                    let row_left = rect.left();
                    let left_pane_right = row_left + pane_w;
                    let right_pane_left = left_pane_right + 1.0;
                    let right_pane_right = rect.right();

                    let painter = ui.painter_at(rect);

                    // ── LEFT PANE backgrounds ──
                    let left_bg = match row.kind {
                        DiffRowKind::Removed | DiffRowKind::Modified => {
                            if is_indent_only {
                                Color32::from_rgba_unmultiplied(
                                    t.removed_bg.r(),
                                    t.removed_bg.g(),
                                    t.removed_bg.b(),
                                    t.removed_bg.a() / 2,
                                )
                            } else {
                                t.removed_bg
                            }
                        }
                        _ => t.bg,
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(row_left, rect.top()),
                            egui::pos2(left_pane_right, rect.bottom()),
                        ),
                        0.0,
                        left_bg,
                    );

                    // ── RIGHT PANE backgrounds ──
                    let right_bg = match row.kind {
                        DiffRowKind::Added | DiffRowKind::Modified => {
                            if is_indent_only {
                                Color32::from_rgba_unmultiplied(
                                    t.added_bg.r(),
                                    t.added_bg.g(),
                                    t.added_bg.b(),
                                    t.added_bg.a() / 2,
                                )
                            } else {
                                t.added_bg
                            }
                        }
                        _ => t.bg,
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(right_pane_left, rect.top()),
                            egui::pos2(right_pane_right, rect.bottom()),
                        ),
                        0.0,
                        right_bg,
                    );

                    // Selected row outline / search highlight (whole row)
                    if self.diff_selected_line_idx == Some(row_idx) {
                        painter.rect_stroke(
                            rect,
                            0.0,
                            egui::Stroke::new(1.0, t.accent),
                            egui::StrokeKind::Middle,
                        );
                    }
                    if self.diff_search_visible
                        && !self.diff_search_query.is_empty()
                        && matches.contains(&row_idx)
                    {
                        let focused =
                            matches.get(self.diff_search_active_index).copied() == Some(row_idx);
                        let hl = if focused {
                            Color32::from_rgba_unmultiplied(
                                t.accent.r(),
                                t.accent.g(),
                                t.accent.b(),
                                45,
                            )
                        } else {
                            Color32::from_rgba_unmultiplied(
                                t.warning.r(),
                                t.warning.g(),
                                t.warning.b(),
                                35,
                            )
                        };
                        painter.rect_filled(rect, 0.0, hl);
                    }

                    // ── Blame gutter (left, fixed) ──
                    if self.diff_show_blame {
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(row_left, rect.top()),
                                egui::pos2(row_left + blame_w, rect.bottom()),
                            ),
                            0.0,
                            t.bg_sidebar,
                        );
                        if let Some(ln) = row.left_no
                            && let Some(Ok(entries)) = self.diff_blame_content.as_ref()
                            && ln > 0
                            && ln <= entries.len()
                        {
                            let e = &entries[ln - 1];
                            let show = ln == 1 || entries[ln - 2].hash != e.hash;
                            if show {
                                let a: String = e.author.chars().take(8).collect();
                                painter.text(
                                    egui::pos2(row_left + 6.0, cy),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{} {}", a, e.date),
                                    egui::FontId::proportional(10.0),
                                    t.text_dim,
                                );
                            }
                            ui.interact(
                                egui::Rect::from_min_max(
                                    egui::pos2(row_left, rect.top()),
                                    egui::pos2(row_left + blame_w, rect.bottom()),
                                ),
                                ui.id().with(("blame", row_idx)),
                                egui::Sense::hover(),
                            )
                            .on_hover_text(format!(
                                "Commit: {}\nAuthor: {}\nDate: {}\nSummary: {}",
                                &e.hash[..7.min(e.hash.len())],
                                e.author,
                                e.date,
                                e.summary
                            ));
                        }
                    }

                    // ── Left gutter (fixed) ──
                    let lgs = row_left + blame_w;
                    let lgb = match row.kind {
                        DiffRowKind::Removed | DiffRowKind::Modified => t.removed_bg,
                        _ => t.bg_sidebar,
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(lgs, rect.top()),
                            egui::pos2(lgs + gutter_w, rect.bottom()),
                        ),
                        0.0,
                        lgb,
                    );
                    if let Some(ln) = row.left_no {
                        painter.text(
                            egui::pos2(lgs + gutter_w - 6.0, cy),
                            egui::Align2::RIGHT_CENTER,
                            ln.to_string(),
                            egui::FontId::monospace(10.0),
                            t.text_dim,
                        );
                    }
                    if matches!(row.kind, DiffRowKind::Removed | DiffRowKind::Modified) {
                        painter.text(
                            egui::pos2(lgs + 4.0, cy),
                            egui::Align2::LEFT_CENTER,
                            "-",
                            egui::FontId::monospace(11.0),
                            t.removed,
                        );
                    }

                    // ── Left code (scrolled, clipped) ──
                    let lcs = lgs + gutter_w;
                    let left_code_clip = egui::Rect::from_min_max(
                        egui::pos2(lcs, rect.top()),
                        egui::pos2(left_pane_right, rect.bottom()),
                    );
                    let left_code_painter = ui.painter_at(left_code_clip);
                    let lx = lcs + 4.0 - hscroll;
                    paint_code_side(
                        ui,
                        &left_code_painter,
                        left_code_clip,
                        rect,
                        lx,
                        row.left_tokens.as_deref(),
                        row.left_text.as_deref(),
                        current_font.clone(),
                        row.kind.clone(),
                        &left_emph,
                        word_hl_removed,
                        &t,
                    );

                    // ── Center divider ──
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(left_pane_right, rect.top()),
                            egui::pos2(right_pane_left, rect.bottom()),
                        ),
                        0.0,
                        t.border,
                    );

                    // ── Right gutter (fixed) ──
                    let rgb = match row.kind {
                        DiffRowKind::Added | DiffRowKind::Modified => t.added_bg,
                        _ => t.bg_sidebar,
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(right_pane_left, rect.top()),
                            egui::pos2(right_pane_left + gutter_w, rect.bottom()),
                        ),
                        0.0,
                        rgb,
                    );
                    if let Some(rn) = row.right_no {
                        painter.text(
                            egui::pos2(right_pane_left + gutter_w - 6.0, cy),
                            egui::Align2::RIGHT_CENTER,
                            rn.to_string(),
                            egui::FontId::monospace(10.0),
                            t.text_dim,
                        );
                    }
                    if matches!(row.kind, DiffRowKind::Added | DiffRowKind::Modified) {
                        painter.text(
                            egui::pos2(right_pane_left + 5.0, cy),
                            egui::Align2::LEFT_CENTER,
                            "+",
                            egui::FontId::monospace(11.0),
                            t.added,
                        );
                    }

                    // ── Right code (scrolled, clipped) ──
                    let rcs = right_pane_left + gutter_w;
                    let right_code_clip = egui::Rect::from_min_max(
                        egui::pos2(rcs, rect.top()),
                        egui::pos2(right_pane_right, rect.bottom()),
                    );
                    let right_code_painter = ui.painter_at(right_code_clip);
                    let rx = rcs + 4.0 - hscroll;
                    paint_code_side(
                        ui,
                        &right_code_painter,
                        right_code_clip,
                        rect,
                        rx,
                        row.right_tokens.as_deref(),
                        row.right_text.as_deref(),
                        current_font.clone(),
                        row.kind.clone(),
                        &right_emph,
                        word_hl_added,
                        &t,
                    );
                }

                if fd.truncated {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        let trunc_msg = match lang {
                            crate::config::Language::English => format!(
                                "Diff is too large; display truncated at {} lines",
                                crate::git::MAX_DIFF_LINES_PUB
                            ),
                            crate::config::Language::Japanese => format!(
                                "差分が大きすぎるため {} 行で表示を打ち切りました",
                                crate::git::MAX_DIFF_LINES_PUB
                            ),
                        };
                        ui.label(RichText::new(trunc_msg).color(t.text_dim).size(11.0));
                    });
                }
            });

        // Drawn horizontal scrollbar: mouse-only setups (Windows) have no
        // horizontal gesture, and the vertical ScrollArea can swallow wheel
        // events, so the trackpad path below is not enough on its own.
        if max_hscroll > 0.0 {
            ui.add_space(2.0);
            let (bar_rect, bar_resp) =
                ui.allocate_exact_size(egui::vec2(avail_w, hbar_h), egui::Sense::click_and_drag());
            let viewport_w = left_code_vp.min(right_code_vp);
            let thumb_w =
                (bar_rect.width() * (viewport_w / code_content_w)).clamp(24.0, bar_rect.width());
            let track_w = (bar_rect.width() - thumb_w).max(1.0);
            // Click jumps, drag follows: center the thumb on the pointer
            if (bar_resp.dragged() || bar_resp.clicked())
                && let Some(px) = bar_resp.interact_pointer_pos().map(|p| p.x)
            {
                let frac = ((px - bar_rect.left() - thumb_w / 2.0) / track_w).clamp(0.0, 1.0);
                self.diff_hscroll = frac * max_hscroll;
                ui.ctx().request_repaint();
            }
            let frac = (self.diff_hscroll / max_hscroll).clamp(0.0, 1.0);
            let thumb_rect = egui::Rect::from_min_size(
                egui::pos2(bar_rect.left() + frac * track_w, bar_rect.top() + 2.0),
                egui::vec2(thumb_w, hbar_h - 4.0),
            );
            let painter = ui.painter();
            painter.rect_filled(bar_rect, 4.0, t.bg_sidebar);
            let thumb_color = if bar_resp.dragged() || bar_resp.hovered() {
                t.text_dim
            } else {
                t.border
            };
            painter.rect_filled(thumb_rect, 4.0, thumb_color);
        }

        // Horizontal scroll via trackpad / shift+wheel when hovering the diff viewport
        if max_hscroll > 0.0 && ui.rect_contains_pointer(scroll_out.inner_rect) {
            let dx = ui.input(|i| i.smooth_scroll_delta.x);
            if dx.abs() > 0.01 {
                self.diff_hscroll = (self.diff_hscroll - dx).clamp(0.0, max_hscroll);
                ui.ctx().request_repaint();
            }
        }

        // Clear scroll-to and apply click
        if scroll_to_idx.is_some() {
            self.diff_search_scroll_to = None;
        }
        if let Some(idx) = click_result.get()
            && let Some(row) = fd.rows.get(idx)
        {
            match row.kind {
                DiffRowKind::Added | DiffRowKind::Removed | DiffRowKind::Modified => {
                    if self.diff_selected_line_idx == Some(idx) {
                        self.diff_selected_line_idx = None;
                    } else {
                        self.diff_selected_line_idx = Some(idx);
                    }
                }
                _ => {
                    self.diff_selected_line_idx = None;
                }
            }
        }

        if show_detail
            && let Some(idx) = self.diff_selected_line_idx
            && let Some(row) = fd.rows.get(idx)
        {
            ui.separator();

            let detail_h = 80.0;
            ui.allocate_ui(egui::vec2(ui.available_width(), detail_h), |ui| {
                let before_str = row.left_text.as_deref().unwrap_or("");
                let after_str = row.right_text.as_deref().unwrap_or("");
                let ops = diff_chars(before_str, after_str);
                let mono = egui::FontId::monospace(11.0);

                // Old/New stacked vertically so changed spans line up at the same x
                // even on long lines; the "−"/"＋" prefixes scroll with the text.
                let mut before_job = egui::text::LayoutJob::default();
                before_job.append(
                    "- ",
                    0.0,
                    egui::TextFormat {
                        font_id: mono.clone(),
                        color: t.removed,
                        ..Default::default()
                    },
                );
                let mut after_job = egui::text::LayoutJob::default();
                after_job.append(
                    "+ ",
                    0.0,
                    egui::TextFormat {
                        font_id: mono.clone(),
                        color: t.added,
                        ..Default::default()
                    },
                );
                for op in &ops {
                    match op {
                        CharOp::Equal(s) => {
                            let fmt = egui::TextFormat {
                                font_id: mono.clone(),
                                color: t.text,
                                ..Default::default()
                            };
                            before_job.append(s, 0.0, fmt.clone());
                            after_job.append(s, 0.0, fmt);
                        }
                        CharOp::Delete(s) => {
                            before_job.append(
                                s,
                                0.0,
                                egui::TextFormat {
                                    font_id: mono.clone(),
                                    color: t.removed,
                                    background: t.removed_bg,
                                    ..Default::default()
                                },
                            );
                        }
                        CharOp::Insert(s) => {
                            after_job.append(
                                s,
                                0.0,
                                egui::TextFormat {
                                    font_id: mono.clone(),
                                    color: t.added,
                                    background: t.added_bg,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }

                ui.label(
                    RichText::new(crate::i18n::t(lang, "line_detail_header"))
                        .color(t.text_dim)
                        .size(10.0),
                );
                // One shared horizontal scroll keeps both lines at the same offset
                egui::ScrollArea::horizontal()
                    .id_salt("line_detail_scroll")
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(before_job);
                            ui.label(after_job);
                        });
                    });
            });
        }
    }

    /// Memoized search matches: scanning every diff row per frame is wasteful,
    /// so results are cached until the query or the file content changes.
    fn diff_search_matches_cached(&mut self) -> Vec<usize> {
        if self.diff_search_query.is_empty() {
            return Vec::new();
        }
        if let Some((q, seq, m)) = &self.diff_search_matches_cache
            && *q == self.diff_search_query
            && *seq == super::cur_seq(&self.seqs.content)
        {
            return m.clone();
        }
        let m = self.get_diff_search_matches();
        self.diff_search_matches_cache = Some((
            self.diff_search_query.clone(),
            super::cur_seq(&self.seqs.content),
            m.clone(),
        ));
        m
    }

    fn get_diff_search_matches(&self) -> Vec<usize> {
        let mut matches = Vec::new();
        if self.diff_search_query.is_empty() {
            return matches;
        }
        let Some(Ok(ref fd)) = self.diff_file_content else {
            return matches;
        };
        let q = self.diff_search_query.to_lowercase();
        for (i, row) in fd.rows.iter().enumerate() {
            let left_match = row
                .left_text
                .as_ref()
                .map(|t| t.to_lowercase().contains(&q))
                .unwrap_or(false);
            let right_match = row
                .right_text
                .as_ref()
                .map(|t| t.to_lowercase().contains(&q))
                .unwrap_or(false);
            if left_match || right_match {
                matches.push(i);
            }
        }
        matches
    }

    fn draw_search_bar(&mut self, ui: &mut egui::Ui) {
        let t = self.theme;
        let lang = self.prefs.language;
        let matches = self.diff_search_matches_cached();

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("🔍").size(12.0));

            let query_color = if !self.diff_search_query.is_empty() && matches.is_empty() {
                t.error
            } else {
                t.text
            };

            let mut query = self.diff_search_query.clone();
            let text_edit = egui::TextEdit::singleline(&mut query)
                .hint_text(crate::i18n::t(lang, "search_diff_hint"))
                .desired_width(180.0)
                .text_color(query_color);

            let resp = ui.add(text_edit);

            if self.diff_search_focus_input {
                resp.request_focus();
                self.diff_search_focus_input = false;
            }

            if resp.changed() {
                self.diff_search_query = query;
                self.diff_search_active_index = 0;
                let matches = self.diff_search_matches_cached();
                if !matches.is_empty() {
                    self.diff_search_scroll_to = Some(matches[0]);
                }
            }

            let mut trigger_jump = false;
            let mut jump_direction_prev = false;
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                resp.request_focus();
                trigger_jump = true;
                jump_direction_prev = ui.input(|i| i.modifiers.shift);
            }
            // F3/Shift+F3, per orapli design-system.md §6 ("F3/Shift+F3 for next/previous navigation"):
            // unlike Enter, this works regardless of
            // whether the search field currently has focus, matching how F3
            // behaves in other editors once a search is already active.
            if ui.input(|i| i.key_pressed(egui::Key::F3)) {
                trigger_jump = true;
                jump_direction_prev = ui.input(|i| i.modifiers.shift);
            }

            if trigger_jump && !matches.is_empty() {
                if jump_direction_prev {
                    if self.diff_search_active_index == 0 {
                        self.diff_search_active_index = matches.len() - 1;
                    } else {
                        self.diff_search_active_index -= 1;
                    }
                } else {
                    if self.diff_search_active_index >= matches.len() - 1 {
                        self.diff_search_active_index = 0;
                    } else {
                        self.diff_search_active_index += 1;
                    }
                }
                self.diff_search_scroll_to = Some(matches[self.diff_search_active_index]);
            }

            if self.diff_search_query.is_empty() {
                ui.label(
                    egui::RichText::new(crate::i18n::t(lang, "zero_matches"))
                        .color(t.text_dim)
                        .size(11.0),
                );
            } else if matches.is_empty() {
                ui.label(
                    egui::RichText::new(crate::i18n::t(lang, "no_matches_found"))
                        .color(t.error)
                        .size(11.0),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "{}/{}",
                        self.diff_search_active_index + 1,
                        matches.len()
                    ))
                    .color(t.text_dim)
                    .size(11.0),
                );
            }

            if ui
                .button("↑")
                .on_hover_text(crate::i18n::t(lang, "prev_match_tooltip"))
                .clicked()
                && !matches.is_empty()
            {
                if self.diff_search_active_index == 0 {
                    self.diff_search_active_index = matches.len() - 1;
                } else {
                    self.diff_search_active_index -= 1;
                }
                self.diff_search_scroll_to = Some(matches[self.diff_search_active_index]);
            }
            if ui
                .button("↓")
                .on_hover_text(crate::i18n::t(lang, "next_match_tooltip"))
                .clicked()
                && !matches.is_empty()
            {
                if self.diff_search_active_index >= matches.len() - 1 {
                    self.diff_search_active_index = 0;
                } else {
                    self.diff_search_active_index += 1;
                }
                self.diff_search_scroll_to = Some(matches[self.diff_search_active_index]);
            }

            if ui
                .button("×")
                .on_hover_text(crate::i18n::t(lang, "close_search_tooltip"))
                .clicked()
            {
                self.diff_search_visible = false;
            }
        });
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_and_compress_tree() {
        let files = vec![
            ChangedFile {
                path: "src/app/mod.rs".to_string(),
                status: "M".to_string(),
                additions: 10,
                deletions: 5,
                old_path: None,
            },
            ChangedFile {
                path: "src/app/diff.rs".to_string(),
                status: "A".to_string(),
                additions: 100,
                deletions: 0,
                old_path: None,
            },
            ChangedFile {
                path: "Cargo.toml".to_string(),
                status: "M".to_string(),
                additions: 1,
                deletions: 1,
                old_path: None,
            },
        ];

        let mut tree = build_diff_tree(&files);
        assert_eq!(tree.len(), 2); // Cargo.toml, src (which has app inside)

        compress_diff_tree(&mut tree);
        assert_eq!(tree.len(), 2);

        // Check if "src" and "app" directories are compressed to "src/app"
        if let DiffTreeNode::Dir {
            name,
            full_path,
            children,
        } = &tree[0]
        {
            assert_eq!(name, "src/app");
            assert_eq!(full_path, "src/app");
            assert_eq!(children.len(), 2); // mod.rs and diff.rs
        } else {
            panic!("Expected directory node at index 0");
        }
    }
}
