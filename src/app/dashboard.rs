use crate::app::{GitDashboardApp, PullState, RepoData};
use egui::Color32;

/// Short commit hash heuristic. git's `%h` is 7+ hex chars depending on
/// repository size and `core.abbrev` (8+ is common, especially on Windows
/// setups), so accept a range instead of exactly 7.
fn looks_like_short_hash(s: &str) -> bool {
    (7..=12).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Format an integer with thousands separators (12345 → "12,345").
fn format_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Truncate to `max` chars with an ellipsis (char-safe for Japanese text).
fn short(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    } else {
        s.to_string()
    }
}

/// Compact base→target compare controls shared by the commit graph and the tag
/// list: a run icon (enabled when both ends are picked), a truncated selection
/// summary, and a clear icon. Returns true when the comparison should run.
fn compare_controls(
    ui: &mut egui::Ui,
    t: &crate::theme::Theme,
    lang: crate::config::Language,
    base: &mut Option<String>,
    target: &mut Option<String>,
) -> bool {
    let mut run = false;
    ui.horizontal(|ui| {
        let both = base.is_some() && target.is_some();
        ui.add_enabled_ui(both, |ui| {
            // A labeled button: the previous swap-arrows icon read as
            // "swap the two", not "run the comparison"
            if ui
                .button(egui::RichText::new(crate::i18n::t(lang, "compare_btn")).size(11.0))
                .on_hover_text(crate::i18n::t(lang, "compare_hint"))
                .clicked()
            {
                run = true;
            }
        });
        if base.is_none() && target.is_none() {
            ui.label(
                egui::RichText::new(crate::i18n::t(lang, "select_base_target_hint"))
                    .color(t.text_faint)
                    .size(11.0),
            );
        } else {
            let b_s = base
                .as_deref()
                .map(|s| short(s, 16))
                .unwrap_or_else(|| "?".to_string());
            let t_s = target
                .as_deref()
                .map(|s| short(s, 16))
                .unwrap_or_else(|| "?".to_string());
            ui.label(egui::RichText::new(b_s).color(t.accent).size(11.0));
            ui.label(egui::RichText::new("→").color(t.text_dim).size(11.0));
            ui.label(egui::RichText::new(t_s).color(t.success).size(11.0));
            if super::widgets::icon_button(
                ui,
                "close",
                crate::i18n::t(lang, "clear_selection_hint"),
                t,
            )
            .clicked()
            {
                *base = None;
                *target = None;
            }
        }
    });
    run
}

impl GitDashboardApp {
    pub(super) fn draw_dashboard_view(
        &mut self,
        ui: &mut egui::Ui,
        data: &RepoData,
        repo_idx: usize,
    ) {
        let t = self.theme;
        let lang = self.prefs.language;
        let repo_name = self.repositories[repo_idx].name.clone();
        let repo_path = self.repositories[repo_idx].path.clone();
        // Header row: back button + repo name + path + action buttons
        ui.horizontal(|ui| {
            if super::widgets::icon_button(ui, "back", crate::i18n::t(lang, "back_to_home"), &t)
                .clicked()
            {
                self.selected_repo_index = None;
            }
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new(&repo_name).strong().color(t.text));
                    ui.add_space(10.0);
                    // Tech-stack badges
                    if data.tech_info.language != "-" {
                        let lang_text = if data.tech_info.lang_version != "-" {
                            format!(
                                "{} {}",
                                data.tech_info.language, data.tech_info.lang_version
                            )
                        } else {
                            data.tech_info.language.clone()
                        };
                        ui.colored_label(t.success, lang_text);
                    }
                    if data.tech_info.framework != "-" {
                        let fw_text = if data.tech_info.framework_version != "-" {
                            format!(
                                "{} {}",
                                data.tech_info.framework, data.tech_info.framework_version
                            )
                        } else {
                            data.tech_info.framework.clone()
                        };
                        ui.colored_label(t.warning, fw_text);
                    }
                    ui.colored_label(t.accent, &data.summary.current_branch);
                });
                ui.label(
                    egui::RichText::new(&data.summary.repo_path)
                        .size(11.0)
                        .color(t.text_faint),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if super::widgets::icon_button(
                    ui,
                    "refresh",
                    crate::i18n::t(lang, "refresh_status"),
                    &t,
                )
                .clicked()
                {
                    self.load_repo_async(repo_idx, ui.ctx().clone());
                }
                // Background refresh indicator (cached data is on screen meanwhile)
                if self.loading_repos.contains(&repo_idx) {
                    ui.add_space(2.0);
                    ui.spinner();
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "syncing"))
                            .color(t.text_dim)
                            .size(11.0),
                    );
                }
                ui.add_space(4.0);

                if super::widgets::icon_button(ui, "download", crate::i18n::t(lang, "run_pull"), &t)
                    .clicked()
                {
                    let path = repo_path.clone();
                    self.pull_statuses.insert(path.clone(), PullState::Pulling);
                    self.jobs.push(super::Job::PullRepo {
                        path,
                        lang: self.prefs.language,
                        ctx: ui.ctx().clone(),
                    });
                }
                ui.add_space(4.0);

                if super::widgets::icon_button(
                    ui,
                    "editor",
                    crate::i18n::t(lang, "open_in_editor_hint"),
                    &t,
                )
                .clicked()
                    && let Some(err) = self.open_in_editor(&repo_path)
                {
                    self.push_toast(err, super::ToastKind::Error);
                }
                ui.add_space(4.0);

                // Pull result display
                let mut clear_status = false;
                if let Some(status) = self.pull_statuses.get(&repo_path) {
                    ui.horizontal(|ui| {
                        match status {
                            PullState::Pulling => {
                                ui.spinner();
                            }
                            PullState::Success(msg) => {
                                ui.label(
                                    egui::RichText::new(crate::i18n::t(lang, "sync_ok"))
                                        .color(t.success),
                                )
                                .on_hover_text(msg);
                            }
                            PullState::Error(err) => {
                                ui.label(
                                    egui::RichText::new(crate::i18n::t(lang, "sync_fail"))
                                        .color(t.error),
                                )
                                .on_hover_text(err);
                            }
                        }
                        if matches!(status, PullState::Success(_) | PullState::Error(_))
                            && super::widgets::icon_button(
                                ui,
                                "close",
                                crate::i18n::t(lang, "clear_status_hint"),
                                &t,
                            )
                            .clicked()
                        {
                            clear_status = true;
                        }
                    });
                }
                if clear_status {
                    self.pull_statuses.remove(&repo_path);
                }
            });
        });

        // Inline pull error warning
        if let Some(PullState::Error(err)) = self.pull_statuses.get(&repo_path) {
            ui.add_space(8.0);
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(
                    t.error.r(),
                    t.error.g(),
                    t.error.b(),
                    20,
                ))
                .stroke(egui::Stroke::new(1.0, t.error))
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(t.error, crate::i18n::t(lang, "sync_error_prefix"));
                        ui.label(err);
                    });
                });
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        // Sub-tab bar (underline style)
        ui.horizontal(|ui| {
            let tab_labels = [
                crate::i18n::t(lang, "tab_overview"),
                crate::i18n::t(lang, "tab_branches"),
                crate::i18n::t(lang, "tab_contributors"),
                crate::i18n::t(lang, "tab_files"),
                crate::i18n::t(lang, "tab_activity"),
            ];
            for (i, label) in tab_labels.iter().enumerate() {
                let is_active = self.dashboard_tab == i;
                let text = egui::RichText::new(*label).size(13.0).color(if is_active {
                    t.text
                } else {
                    t.text_dim
                });
                let resp = ui
                    .selectable_label(is_active, text)
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                // 2px accent underline on the active tab
                if is_active {
                    let painter = ui.painter();
                    painter.line_segment(
                        [
                            egui::pos2(resp.rect.left(), resp.rect.bottom()),
                            egui::pos2(resp.rect.right(), resp.rect.bottom()),
                        ],
                        egui::Stroke::new(2.0, t.accent),
                    );
                }
                if resp.clicked() {
                    self.dashboard_tab = i;
                }
                ui.add_space(8.0);
            }
        });

        ui.add_space(16.0);

        // Filter and recompute commit data for the selected range
        let mut contributors = data.contributors.clone();
        if self.active_filter {
            contributors.retain(|c| c.is_member && c.is_active);
        }
        let total_commits: usize = contributors.iter().map(|c| c.commit_count).sum();

        for c in &mut contributors {
            c.percentage = if total_commits > 0 {
                (c.commit_count as f64 / total_commits as f64) * 100.0
            } else {
                0.0
            };
        }

        match self.dashboard_tab {
            0 => {
                // Overview tab
                self.draw_overview_tab(
                    ui,
                    data,
                    repo_idx,
                    &repo_path,
                    total_commits,
                    contributors.len(),
                );
            }
            4 => {
                // Activity tab
                self.draw_activity_tab(ui, data, &repo_path);
            }
            2 => {
                // Contributors tab
                self.draw_contributors_tab(ui, &contributors);
            }
            1 => {
                // Branches tab
                self.draw_branches_tab(ui, data, repo_idx, &repo_path);
            }
            3 => {
                // Files tab
                self.draw_files_tab(ui, data);
            }
            _ => {}
        }

        self.render_register_member_modal(ui.ctx());
        self.render_add_alias_modal(ui.ctx());
    }
    fn draw_overview_tab(
        &mut self,
        ui: &mut egui::Ui,
        data: &RepoData,
        repo_idx: usize,
        repo_path: &std::path::Path,
        total_commits: usize,
        contributors_len: usize,
    ) {
        let t = self.theme;
        let lang = self.prefs.language;
        let widget_frame = egui::Frame::NONE
            .fill(t.bg_elevated)
            .corner_radius(4.0)
            .inner_margin(16.0);
        let kpi_frame = egui::Frame::NONE
            .fill(t.bg_elevated)
            .corner_radius(4.0)
            .inner_margin(12.0);
        // Panel height before entering the scroll area, so the README card can
        // use the actual window space instead of a fixed cap
        let panel_h = ui.available_height();

        egui::ScrollArea::vertical()
            .id_salt("overview_tab_scroll")
            .show(ui, |ui| {
                ui.columns(4, |columns| {
                    let metrics = [
                        (
                            crate::i18n::t(lang, "stat_commits"),
                            format_thousands(total_commits),
                            t.chart[0],
                        ),
                        (
                            crate::i18n::t(lang, "stat_authors"),
                            format_thousands(contributors_len),
                            t.chart[1],
                        ),
                        (
                            crate::i18n::t(lang, "stat_branches"),
                            format_thousands(data.summary.total_branches),
                            t.chart[2],
                        ),
                        (
                            crate::i18n::t(lang, "stat_files"),
                            format_thousands(data.summary.total_files),
                            t.chart[3],
                        ),
                    ];
                    for (col, (label, value, color)) in columns.iter_mut().zip(metrics.iter()) {
                        col.vertical_centered(|ui| {
                            kpi_frame.show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.set_min_height(64.0);
                                ui.vertical_centered(|ui| {
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new(*label).color(t.text_faint).size(10.0),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(value.as_str())
                                            .color(*color)
                                            .size(28.0)
                                            .strong(),
                                    );
                                });
                            });
                        });
                    }
                });

                ui.add_space(20.0);

                if !data.stash_list.is_empty() {
                    widget_frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "stashes"))
                                .strong()
                                .size(14.0)
                                .color(t.text),
                        );
                        ui.add_space(8.0);

                        for stash in &data.stash_list {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("[{}]", stash.ref_name))
                                        .monospace()
                                        .color(t.accent),
                                );

                                let msg = if stash.message.chars().count() > 50 {
                                    stash.message.chars().take(47).collect::<String>() + "..."
                                } else {
                                    stash.message.clone()
                                };
                                ui.label(&msg).on_hover_text(&stash.message);

                                ui.label(
                                    egui::RichText::new(format!(
                                        "({}, {})",
                                        stash.author, stash.date_relative
                                    ))
                                    .color(t.text_dim)
                                    .size(11.0),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "drop_stash",
                                                ))
                                                .color(t.error),
                                            )
                                            .clicked()
                                        {
                                            self.confirm_drop_stash =
                                                Some((repo_idx, stash.ref_name.clone()));
                                        }
                                        ui.add_space(4.0);
                                        if ui
                                            .button(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "apply_stash",
                                                ))
                                                .color(t.success),
                                            )
                                            .clicked()
                                        {
                                            // Worker thread; toast + reload on completion
                                            self.jobs.push(super::Job::StashOp {
                                                repo_idx,
                                                repo_path: repo_path.to_path_buf(),
                                                stash_ref: stash.ref_name.clone(),
                                                action: super::StashAction::Apply,
                                                ctx: ui.ctx().clone(),
                                            });
                                        }
                                        ui.add_space(4.0);
                                        if ui
                                            .button(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "view_diff",
                                                ))
                                                .color(t.accent),
                                            )
                                            .clicked()
                                        {
                                            self.open_diff_single(
                                                repo_idx,
                                                stash.ref_name.clone(),
                                                ui.ctx().clone(),
                                            );
                                            self.selected_repo_index = None;
                                        }
                                    },
                                );
                            });
                            ui.add_space(4.0);
                        }
                    });
                    ui.add_space(20.0);
                }

                widget_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if let Some((name, content)) = &data.readme {
                        ui.label(
                            egui::RichText::new(format!("📖 {}", name))
                                .strong()
                                .size(14.0)
                                .color(t.text),
                        );
                        ui.add_space(10.0);
                        egui::ScrollArea::vertical()
                            .id_salt("readme_scroll")
                            .max_height((panel_h - 120.0).max(400.0))
                            .show(ui, |ui| {
                                egui_commonmark::CommonMarkViewer::new().show(
                                    ui,
                                    &mut self.commonmark_cache,
                                    content,
                                );
                            });
                    } else {
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "readme_not_found"))
                                .color(t.text_dim),
                        );
                    }
                });
            });
    }

    fn draw_activity_tab(
        &mut self,
        ui: &mut egui::Ui,
        data: &RepoData,
        repo_path: &std::path::Path,
    ) {
        let t = self.theme;
        let lang = self.prefs.language;
        let widget_frame = egui::Frame::NONE
            .fill(t.bg_elevated)
            .corner_radius(4.0)
            .inner_margin(16.0);

        egui::ScrollArea::vertical()
            .id_salt("activity_tab_scroll")
            .show(ui, |ui| {
                // Period selector: 30 days renders straight from RepoData,
                // other periods fetch an override on a worker thread
                let periods: [(&str, usize); 5] = [
                    (crate::i18n::t(lang, "30日"), 30),
                    (crate::i18n::t(lang, "90日"), 90),
                    (crate::i18n::t(lang, "180日"), 180),
                    (crate::i18n::t(lang, "past_year"), 365),
                    (crate::i18n::t(lang, "all_history"), 0),
                ];
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "period"))
                            .color(t.text_dim)
                            .size(12.0),
                    );
                    for (label, days) in periods {
                        let sel = self.activity_period_days == days;
                        if ui
                            .selectable_label(sel, label)
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                            && !sel
                        {
                            self.activity_period_days = days;
                            self.activity_override = None;
                            if days == 30 {
                                super::bump_seq(&self.seqs.activity); // drop in-flight fetches
                                self.activity_loading = false;
                            } else {
                                self.request_activity(
                                    repo_path.to_path_buf(),
                                    days,
                                    ui.ctx().clone(),
                                );
                            }
                        }
                    }
                    if self.activity_loading {
                        ui.spinner();
                    }
                });
                ui.add_space(8.0);

                // While a fetch is in flight the default-period data stays
                // visible (stale-while-revalidate, same as the dashboards)
                let activity = if self.activity_period_days == 30 {
                    &data.activity
                } else {
                    self.activity_override.as_ref().unwrap_or(&data.activity)
                };
                let period_label = match self.activity_period_days {
                    0 => crate::i18n::t(lang, "all_history").to_string(),
                    365 => crate::i18n::t(lang, "past_year").to_string(),
                    d => match lang {
                        crate::config::Language::English => format!("Past {} days", d),
                        crate::config::Language::Japanese => format!("過去{}日間", d),
                    },
                };

                widget_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let chart_title = match lang {
                        crate::config::Language::English => {
                            format!("Commit Activity ({})", period_label)
                        }
                        crate::config::Language::Japanese => {
                            format!("コミットアクティビティ（{}）", period_label)
                        }
                    };
                    self.draw_custom_bar_chart(
                        ui,
                        &chart_title,
                        &activity.daily.dates,
                        &activity.daily.counts,
                    );
                });

                ui.add_space(20.0);

                ui.columns(2, |cols| {
                    let labels_hourly: Vec<String> = (0..24)
                        .map(|h| match lang {
                            crate::config::Language::English => format!("{:02}:00", h),
                            crate::config::Language::Japanese => format!("{:02}時", h),
                        })
                        .collect();
                    widget_frame.show(&mut cols[0], |ui| {
                        ui.set_width(ui.available_width());
                        let hourly_title = match lang {
                            crate::config::Language::English => {
                                format!("⏰ Hourly Commits ({})", period_label)
                            }
                            crate::config::Language::Japanese => {
                                format!("⏰ 時間帯別コミット数（{}）", period_label)
                            }
                        };
                        self.draw_custom_bar_chart(
                            ui,
                            &hourly_title,
                            &labels_hourly,
                            &activity.hourly,
                        );
                    });

                    let labels_weekly: Vec<String> = match lang {
                        crate::config::Language::English => {
                            vec!["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                                .into_iter()
                                .map(String::from)
                                .collect()
                        }
                        crate::config::Language::Japanese => {
                            vec!["日", "月", "火", "水", "木", "金", "土"]
                                .into_iter()
                                .map(String::from)
                                .collect()
                        }
                    };
                    widget_frame.show(&mut cols[1], |ui| {
                        ui.set_width(ui.available_width());
                        let weekly_title = match lang {
                            crate::config::Language::English => {
                                format!("Commits by Day of Week ({})", period_label)
                            }
                            crate::config::Language::Japanese => {
                                format!("曜日別コミット数（{}）", period_label)
                            }
                        };
                        self.draw_custom_bar_chart(
                            ui,
                            &weekly_title,
                            &labels_weekly,
                            &activity.weekly,
                        );
                    });
                });
            });
    }

    fn draw_contributors_tab(
        &mut self,
        ui: &mut egui::Ui,
        contributors: &[crate::git::Contributor],
    ) {
        let t = self.theme;
        let widget_frame = egui::Frame::NONE
            .fill(t.bg_elevated)
            .corner_radius(4.0)
            .inner_margin(16.0);

        egui::ScrollArea::vertical()
            .id_salt("contributors_tab_scroll")
            .show(ui, |ui| {
                let lang = self.prefs.language;
                widget_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "contributor_ranking"))
                                .strong()
                                .size(14.0)
                                .color(t.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(
                                &mut self.active_filter,
                                crate::i18n::t(lang, "active_members_only"),
                            )
                            .on_hover_text(crate::i18n::t(lang, "active_filter_tooltip"));
                        });
                    });
                    ui.add_space(10.0);

                    let chart_colors = [
                        t.chart[0], t.chart[1], t.chart[2], t.chart[3], t.chart[4], t.chart[5],
                        t.chart[6], t.chart[7],
                    ];

                    let total_w = ui.available_width();
                    let chart_col_w = 180.0;
                    // Donut: top 7 + "Other". OSS clones can have thousands of
                    // contributors; one slice each was unreadable and painted
                    // thousands of polygons per frame.
                    let mut contrib_slices: Vec<(&str, f32, Color32)> = contributors
                        .iter()
                        .take(7)
                        .enumerate()
                        .map(|(i, c)| {
                            (
                                c.name.as_str(),
                                c.percentage as f32,
                                chart_colors[i % chart_colors.len()],
                            )
                        })
                        .collect();
                    if contributors.len() > 7 {
                        let rest: f32 = contributors[7..].iter().map(|c| c.percentage as f32).sum();
                        contrib_slices.push((
                            crate::i18n::t(lang, "slice_other"),
                            rest,
                            t.text_faint,
                        ));
                    }

                    // Table: top 100 rows by default; laying out thousands of
                    // grid rows every frame froze the tab on large repos
                    let visible_count = if self.contributors_show_all {
                        contributors.len()
                    } else {
                        contributors.len().min(100)
                    };

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(total_w - chart_col_w - 8.0);
                            let mut open_reg_dialog = None;
                            let mut open_alias_dialog = None;
                            let has_members = !self.members.is_empty();
                            egui::Grid::new("contributor_grid")
                                .num_columns(7)
                                .spacing([12.0, 10.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("");
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "rank"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "canonical_name"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "commits"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "share"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "active"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "action_col"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.end_row();

                                    for (rank, c) in
                                        contributors.iter().take(visible_count).enumerate()
                                    {
                                        let color = chart_colors[rank % chart_colors.len()];
                                        ui.horizontal(|ui| {
                                            let (rect, _) = ui.allocate_exact_size(
                                                egui::vec2(8.0, 8.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(rect, 1.0, color);
                                        });
                                        ui.label((rank + 1).to_string());
                                        let name_text = if c.is_member {
                                            egui::RichText::new(&c.name).strong().color(t.accent)
                                        } else {
                                            egui::RichText::new(&c.name).color(t.text)
                                        };
                                        let email_hover = match lang {
                                            crate::config::Language::English => {
                                                format!("Email: {}", c.email)
                                            }
                                            crate::config::Language::Japanese => {
                                                format!("メール: {}", c.email)
                                            }
                                        };
                                        ui.label(name_text).on_hover_text(email_hover);
                                        ui.label(c.commit_count.to_string());
                                        ui.horizontal(|ui| {
                                            ui.label(format!("{:.1}%", c.percentage));
                                            let (rect, _) = ui.allocate_exact_size(
                                                egui::vec2(60.0, 6.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(rect, 2.0, t.bg_sidebar);
                                            let fill = egui::Rect::from_min_max(
                                                rect.min,
                                                egui::pos2(
                                                    rect.min.x
                                                        + 60.0 * (c.percentage as f32 / 100.0),
                                                    rect.max.y,
                                                ),
                                            );
                                            ui.painter().rect_filled(fill, 2.0, t.accent);
                                        });
                                        let active_text = if c.is_member {
                                            if c.is_active {
                                                egui::RichText::new(crate::i18n::t(lang, "active"))
                                                    .color(t.success)
                                            } else {
                                                egui::RichText::new(crate::i18n::t(
                                                    lang, "inactive",
                                                ))
                                                .color(t.error)
                                            }
                                        } else {
                                            egui::RichText::new(crate::i18n::t(
                                                lang,
                                                "unregistered",
                                            ))
                                            .color(t.text_dim)
                                        };
                                        ui.label(active_text);

                                        if !c.is_member {
                                            ui.horizontal(|ui| {
                                                if super::widgets::icon_button(
                                                    ui,
                                                    "user-plus",
                                                    crate::i18n::t(lang, "btn_add_member"),
                                                    &t,
                                                )
                                                .clicked()
                                                {
                                                    open_reg_dialog =
                                                        Some((c.name.clone(), c.email.clone()));
                                                }
                                                if has_members {
                                                    ui.add_space(4.0);
                                                    if super::widgets::icon_button(
                                                        ui,
                                                        "user-link",
                                                        crate::i18n::t(lang, "btn_add_alias"),
                                                        &t,
                                                    )
                                                    .clicked()
                                                    {
                                                        open_alias_dialog =
                                                            Some((c.name.clone(), c.email.clone()));
                                                    }
                                                }
                                            });
                                        } else {
                                            ui.label("");
                                        }

                                        ui.end_row();
                                    }
                                });

                            if let Some((name, email)) = open_reg_dialog {
                                self.register_member_dialog =
                                    Some(crate::app::RegisterMemberDialogState {
                                        contributor_name: name.clone(),
                                        contributor_email: email.clone(),
                                        canonical_name: name,
                                        aliases_input: email,
                                        error_msg: None,
                                    });
                            }
                            if let Some((name, email)) = open_alias_dialog {
                                self.add_alias_dialog = Some(crate::app::AddAliasDialogState {
                                    contributor_name: name,
                                    contributor_email: email,
                                    selected_member_idx: 0,
                                    filter_query: String::new(),
                                    error_msg: None,
                                });
                            }
                            if visible_count < contributors.len() {
                                ui.add_space(6.0);
                                let remaining = contributors.len() - visible_count;
                                let show_all_text = match lang {
                                    crate::config::Language::English => {
                                        format!("Show all ({} remaining)", remaining)
                                    }
                                    crate::config::Language::Japanese => {
                                        format!("すべて表示（残り {} 名）", remaining)
                                    }
                                };
                                if ui
                                    .button(show_all_text)
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.contributors_show_all = true;
                                }
                            }
                        });
                        ui.add_space(8.0);
                        ui.vertical_centered(|ui| {
                            self.draw_pie_chart(ui, &contrib_slices);
                        });
                    });
                });
            });
    }

    fn draw_branches_tab(
        &mut self,
        ui: &mut egui::Ui,
        data: &RepoData,
        repo_idx: usize,
        repo_path: &std::path::Path,
    ) {
        let t = self.theme;
        let lang = self.prefs.language;
        let widget_frame = egui::Frame::NONE
            .fill(t.bg_elevated)
            .corner_radius(4.0)
            .inner_margin(16.0);

        if self.selected_commit_hash.is_some() {
            egui::Panel::bottom("commit_detail_panel")
                .resizable(true)
                .default_size(200.0)
                .min_size(100.0)
                .show(ui, |ui| {
                    widget_frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height(ui.available_height());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "commit_details"))
                                    .strong()
                                    .size(14.0)
                                    .color(t.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if super::widgets::icon_button(
                                        ui,
                                        "close",
                                        crate::i18n::t(lang, "close_details_hint"),
                                        &t,
                                    )
                                    .clicked()
                                    {
                                        self.clear_commit_detail();
                                    }
                                },
                            );
                        });
                        ui.add_space(8.0);

                        if let Some(hash) = self.selected_commit_hash.clone() {
                            ui.horizontal(|ui| {
                                if ui
                                    .button(crate::i18n::t(lang, "view_commit_diff"))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::t(lang, "view_commit_diff_hint"))
                                    .clicked()
                                {
                                    self.open_diff_single(repo_idx, hash, ui.ctx().clone());
                                }
                            });
                            ui.add_space(6.0);
                            if self.commit_detail_loading {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(
                                            lang,
                                            "fetching_commit_details",
                                        ))
                                        .color(t.text_dim),
                                    );
                                });
                            } else {
                                let scroll_h = (ui.available_height() - 8.0).max(40.0);
                                ui.columns(2, |cols| {
                                    if let Some(ref stat) = self.selected_commit_stat {
                                        egui::ScrollArea::vertical()
                                            .id_salt("commit_detail_stat_scroll")
                                            .max_height(scroll_h)
                                            .show(&mut cols[0], |ui| {
                                                let mut stat_clone = stat.clone();
                                                ui.add(
                                                    egui::TextEdit::multiline(&mut stat_clone)
                                                        .font(egui::TextStyle::Monospace)
                                                        .code_editor()
                                                        .desired_width(f32::INFINITY),
                                                );
                                            });
                                    }
                                    let file_count =
                                        self.selected_commit_files.as_ref().map(Vec::len);
                                    if let Some(file_count) = file_count {
                                        let ui = &mut cols[1];
                                        let changed_files_title = match lang {
                                            crate::config::Language::English => {
                                                format!("Changed Files ({file_count})")
                                            }
                                            crate::config::Language::Japanese => {
                                                format!("変更ファイル ({file_count})")
                                            }
                                        };
                                        ui.label(
                                            egui::RichText::new(changed_files_title)
                                                .strong()
                                                .color(t.text_dim)
                                                .size(11.0),
                                        );
                                        ui.add_space(2.0);
                                        // Tree view shared with the diff view's file
                                        // list (take/put-back avoids re-borrowing
                                        // self inside the render closure)
                                        if let Some(tree) = self.commit_files_tree.take() {
                                            let mut clicked: Option<String> = None;
                                            egui::ScrollArea::both()
                                                .id_salt("commit_detail_files_scroll")
                                                .max_height(scroll_h - 22.0)
                                                .show(ui, |ui| {
                                                    for node in &tree {
                                                        super::diff::draw_tree_node(
                                                            ui,
                                                            node,
                                                            4.0,
                                                            &t,
                                                            lang,
                                                            &mut self.commit_tree_collapsed,
                                                            None,
                                                            &mut clicked,
                                                        );
                                                    }
                                                });
                                            self.commit_files_tree = Some(tree);
                                            // Clicking a file opens its diff directly
                                            if let Some(file) = clicked
                                                && let Some(hash) =
                                                    self.selected_commit_hash.clone()
                                            {
                                                self.open_diff_single(
                                                    repo_idx,
                                                    hash,
                                                    ui.ctx().clone(),
                                                );
                                                self.load_file_diff(file, ui.ctx().clone());
                                            }
                                        } else if file_count == 0 {
                                            ui.label(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "no_changes",
                                                ))
                                                .color(t.text_dim)
                                                .size(11.0),
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    });
                });
        }

        egui::Panel::left("branch_list_panel")
            .resizable(true)
            .default_size(260.0)
            .min_size(180.0)
            // Match CentralPanel's 8px margins (the side-panel default is 2px
            // top/bottom, which left this card's edges offset from the commit graph)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(8)))
            .show(ui, |ui| {
                widget_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "branches_header"))
                            .strong()
                            .size(14.0)
                            .color(t.text),
                    );
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "tab_branches"))
                            .color(t.text_dim)
                            .size(10.0),
                    );
                    ui.add_space(8.0);

                    let selected_b = self
                        .selected_branch
                        .get_or_insert_with(|| data.summary.current_branch.clone())
                        .clone();

                    // Fetch on a worker thread: a synchronous git call here froze
                    // the UI on every repo open (especially slow on Windows)
                    if self.branch_oneline_log.is_none() && !self.branch_log_loading {
                        self.request_branch_log(
                            repo_path.to_path_buf(),
                            selected_b.clone(),
                            ui.ctx().clone(),
                        );
                    }

                    // Quick filter: repositories tracking many remotes easily
                    // exceed a hundred branches
                    ui.add(
                        egui::TextEdit::singleline(&mut self.branch_filter)
                            .hint_text(crate::i18n::t(lang, "filter_branches_hint"))
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(4.0);
                    let branch_q = self.branch_filter.trim().to_lowercase();

                    // Vertical 50/50 split: branches scroll in the top half,
                    // tags in the bottom half. The tag header + compare
                    // controls sit outside the tag scroll so they stay fixed.
                    let half_h = ((ui.available_height() - 70.0) / 2.0).max(60.0);
                    let mut clicked_branch = None;
                    egui::ScrollArea::vertical()
                        .id_salt("branch_half_scroll")
                        .max_height(half_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                for (idx, b) in data.branches.iter().enumerate() {
                                    if !branch_q.is_empty()
                                        && !b.name.to_lowercase().contains(&branch_q)
                                    {
                                        continue;
                                    }
                                    let is_selected = selected_b == b.name;

                                    let row_h = 24.0;
                                    let (rect, _temp_resp) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), row_h),
                                        egui::Sense::hover(),
                                    );
                                    // Offscreen rows keep scroll geometry but skip
                                    // interact + painting (same as the commit graph)
                                    if !ui.is_rect_visible(rect) {
                                        continue;
                                    }

                                    let row_id = ui.make_persistent_id(("branch_row", idx));
                                    let resp = ui
                                        .interact(rect, row_id, egui::Sense::click())
                                        .on_hover_cursor(egui::CursorIcon::PointingHand);

                                    if is_selected {
                                        ui.painter().rect_filled(rect, 4.0, t.bg_active);
                                    } else if resp.hovered() {
                                        ui.painter().rect_filled(rect, 4.0, t.bg_hover);
                                    }

                                    if resp.clicked() {
                                        clicked_branch = Some(b.name.clone());
                                    }

                                    // Painter-based row content: child labels would
                                    // swallow hover/click and break full-row selection
                                    let painter = ui.painter_at(rect);
                                    let cy = rect.center().y;

                                    let dot_color = branch_dot_color(&b.name, &t);
                                    let final_dot_color = if b.is_remote {
                                        Color32::from_rgba_unmultiplied(
                                            dot_color.r(),
                                            dot_color.g(),
                                            dot_color.b(),
                                            128,
                                        )
                                    } else {
                                        dot_color
                                    };
                                    let dot_c = egui::pos2(rect.left() + 10.0, cy);
                                    if b.is_remote {
                                        painter.circle_stroke(
                                            dot_c,
                                            3.0,
                                            egui::Stroke::new(1.5, final_dot_color),
                                        );
                                    } else {
                                        painter.circle_filled(dot_c, 4.0, final_dot_color);
                                    }

                                    // Date (right-aligned, painted first to know its width)
                                    let date_galley = painter.layout_no_wrap(
                                        b.date.clone(),
                                        egui::FontId::proportional(10.0),
                                        if is_selected {
                                            t.on_selection
                                        } else {
                                            t.text_dim
                                        },
                                    );
                                    let date_x = rect.right() - 6.0 - date_galley.rect.width();
                                    painter.galley(
                                        egui::pos2(date_x, cy - date_galley.rect.height() / 2.0),
                                        date_galley,
                                        t.text_dim,
                                    );

                                    // Branch name, clipped so it never overlaps the date
                                    let name_color = if is_selected {
                                        t.on_selection
                                    } else if b.is_remote {
                                        t.text_dim
                                    } else {
                                        t.accent
                                    };
                                    let name_clip = egui::Rect::from_min_max(
                                        egui::pos2(rect.left() + 20.0, rect.top()),
                                        egui::pos2(date_x - 6.0, rect.bottom()),
                                    );
                                    ui.painter_at(name_clip).text(
                                        egui::pos2(rect.left() + 20.0, cy),
                                        egui::Align2::LEFT_CENTER,
                                        &b.name,
                                        egui::FontId::proportional(13.0),
                                        name_color,
                                    );

                                    let hover_text = match lang {
                                        crate::config::Language::English => format!(
                                            "Last updated: {}\nAuthor: {}\nMessage: {}",
                                            b.date, b.author, b.message
                                        ),
                                        crate::config::Language::Japanese => format!(
                                            "最終更新日: {}\n更新者: {}\nメッセージ: {}",
                                            b.date, b.author, b.message
                                        ),
                                    };
                                    resp.on_hover_text(hover_text);
                                }
                            });
                        });

                    if let Some(br) = clicked_branch {
                        self.selected_branch = Some(br.clone());
                        self.clear_commit_detail();
                        self.graph_diff_base = None;
                        self.graph_diff_target = None;
                        self.graph_log_limit = 100;
                        self.branch_oneline_log = None;
                        self.request_branch_log(repo_path.to_path_buf(), br, ui.ctx().clone());
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Bottom half: fixed tag header + compare controls, scrolling list
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "tags_header"))
                            .strong()
                            .color(t.text_dim)
                            .size(11.0),
                    );
                    ui.add_space(4.0);

                    if data.tags.is_empty() {
                        ui.label(egui::RichText::new(crate::i18n::t(lang, "no_tags")).color(t.text_dim).size(11.0));
                    } else {
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "tag_select_hint"))
                                .color(t.text_dim)
                                .size(10.0),
                        );
                        if compare_controls(
                            ui,
                            &t,
                            lang,
                            &mut self.tag_diff_base,
                            &mut self.tag_diff_target,
                        ) && let (Some(base), Some(target)) =
                            (self.tag_diff_base.clone(), self.tag_diff_target.clone())
                        {
                            self.open_diff_range(repo_idx, base, target, ui.ctx().clone());
                        }
                        ui.add_space(4.0);

                        // Virtualized rows (show_rows): release-heavy repos can
                        // carry hundreds of tags, and the previous Grid laid out
                        // every row each frame. Long tag names are truncated
                        // (full text on hover) so they can't blow up the panel.
                        let tag_row_h = 22.0;
                        let marker_w = 14.0;
                        egui::ScrollArea::vertical()
                            .id_salt("tag_half_scroll")
                            .auto_shrink([false, false])
                            .show_rows(ui, tag_row_h, data.tags.len(), |ui, range| {
                                for tag in &data.tags[range] {
                                    let (rect, resp) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), tag_row_h),
                                        egui::Sense::click(),
                                    );
                                    let tag_hover = match lang {
                                        crate::config::Language::English => format!(
                                            "Tag: {}\nCreated: {}\nMessage: {}\n● Left: select base  ● Right: select target",
                                            tag.name, tag.date, tag.message
                                        ),
                                        crate::config::Language::Japanese => format!(
                                            "タグ名: {}\n作成日: {}\nメッセージ: {}\n●左: base選択  ●右: target選択",
                                            tag.name, tag.date, tag.message
                                        ),
                                    };
                                    let resp = resp
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text(tag_hover);

                                    // Marker click zones (same UX as the commit graph)
                                    if resp.clicked() {
                                        let cx = ui
                                            .input(|inp| {
                                                inp.pointer.interact_pos().map(|p| p.x)
                                            })
                                            .unwrap_or(f32::MAX);
                                        if cx < rect.left() + marker_w {
                                            if self.tag_diff_base.as_deref()
                                                == Some(tag.name.as_str())
                                            {
                                                self.tag_diff_base = None;
                                            } else {
                                                self.tag_diff_base = Some(tag.name.clone());
                                            }
                                        } else if cx < rect.left() + marker_w * 2.0 {
                                            if self.tag_diff_target.as_deref()
                                                == Some(tag.name.as_str())
                                            {
                                                self.tag_diff_target = None;
                                            } else {
                                                self.tag_diff_target = Some(tag.name.clone());
                                            }
                                        }
                                    }

                                    let is_base = self.tag_diff_base.as_deref()
                                        == Some(tag.name.as_str());
                                    let is_tgt = self.tag_diff_target.as_deref()
                                        == Some(tag.name.as_str());
                                    let painter = ui.painter_at(rect);
                                    let cy = rect.center().y;
                                    let bc = egui::pos2(rect.left() + marker_w / 2.0, cy);
                                    let tc = egui::pos2(rect.left() + marker_w * 1.5, cy);
                                    painter.circle_stroke(
                                        bc,
                                        4.5,
                                        egui::Stroke::new(
                                            1.5,
                                            if is_base { t.accent } else { t.border },
                                        ),
                                    );
                                    if is_base {
                                        painter.circle_filled(bc, 2.5, t.accent);
                                    }
                                    painter.circle_stroke(
                                        tc,
                                        4.5,
                                        egui::Stroke::new(
                                            1.5,
                                            if is_tgt { t.success } else { t.border },
                                        ),
                                    );
                                    if is_tgt {
                                        painter.circle_filled(tc, 2.5, t.success);
                                    }

                                    // Date (right-aligned, painted first to know its width)
                                    let date_galley = painter.layout_no_wrap(
                                        tag.date.clone(),
                                        egui::FontId::proportional(10.0),
                                        t.text_dim,
                                    );
                                    let date_x = rect.right() - 4.0 - date_galley.rect.width();
                                    painter.galley(
                                        egui::pos2(date_x, cy - date_galley.rect.height() / 2.0),
                                        date_galley,
                                        t.text_dim,
                                    );

                                    // Tag name, truncated and clipped before the date
                                    let name_clip = egui::Rect::from_min_max(
                                        egui::pos2(rect.left() + marker_w * 2.0 + 6.0, rect.top()),
                                        egui::pos2(date_x - 6.0, rect.bottom()),
                                    );
                                    ui.painter_at(name_clip).text(
                                        egui::pos2(rect.left() + marker_w * 2.0 + 6.0, cy),
                                        egui::Align2::LEFT_CENTER,
                                        short(&tag.name, 24),
                                        egui::FontId::proportional(12.0),
                                        t.accent,
                                    );
                                }
                            });
                    }
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            widget_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(ui.available_height());
                let title = if let Some(ref br) = self.selected_branch {
                    match lang {
                        crate::config::Language::English => format!("Commit Graph ({})", br),
                        crate::config::Language::Japanese => format!("コミットグラフ ({})", br),
                    }
                } else {
                    crate::i18n::t(lang, "commit_graph").to_string()
                };
                ui.label(egui::RichText::new(title).strong().size(14.0).color(t.text));
                ui.label(
                    egui::RichText::new(crate::i18n::t(lang, "commit_click_hint"))
                        .color(t.text_dim)
                        .size(10.0),
                );
                if compare_controls(
                    ui,
                    &t,
                    lang,
                    &mut self.graph_diff_base,
                    &mut self.graph_diff_target,
                ) && let (Some(base), Some(target)) =
                    (self.graph_diff_base.clone(), self.graph_diff_target.clone())
                {
                    self.open_diff_range(repo_idx, base, target, ui.ctx().clone());
                }
                ui.add_space(8.0);

                let mut clicked_hash: Option<String> = None;
                let mut clicked_base: Option<String> = None;
                let mut clicked_target: Option<String> = None;
                let mut load_more = false;
                let marker_w: f32 = 14.0;
                let markers_total: f32 = marker_w * 2.0;
                egui::ScrollArea::both()
                    .id_salt("branch_graph_scroll")
                    .show(ui, |ui| {
                        if let Some(ref oneline) = self.branch_oneline_log {
                            ui.vertical(|ui| {
                                for line in oneline.lines() {
                                    let row_h = 22.0;
                                    // Allocate first (keeps scroll geometry), then skip
                                    // ANSI parsing and painting for offscreen rows —
                                    // parsing every line every frame burned CPU on
                                    // large graphs. Clicks on non-commit rows are
                                    // no-ops (hash is None), so click sense is safe.
                                    let (rect, row_resp) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), row_h),
                                        egui::Sense::click(),
                                    );
                                    if !ui.is_rect_visible(rect) {
                                        continue;
                                    }

                                    let segments = parse_ansi_line(line, &t);

                                    let mut row_commit_hash = None;
                                    for seg in &segments {
                                        let trimmed = seg.text.trim();
                                        if looks_like_short_hash(trimmed) {
                                            row_commit_hash = Some(trimmed.to_string());
                                            break;
                                        }
                                        for word in trimmed.split_whitespace() {
                                            let clean_word = word
                                                .trim_matches(|c: char| !c.is_ascii_alphanumeric());
                                            if looks_like_short_hash(clean_word) {
                                                row_commit_hash = Some(clean_word.to_string());
                                                break;
                                            }
                                        }
                                        if row_commit_hash.is_some() {
                                            break;
                                        }
                                    }

                                    // Use click sense directly; painter-based rendering avoids
                                    // child_ui labels consuming hover and blocking click detection
                                    let is_clickable = row_commit_hash.is_some();
                                    let row_resp = if is_clickable {
                                        row_resp.on_hover_cursor(egui::CursorIcon::PointingHand)
                                    } else {
                                        row_resp
                                    };

                                    // Determine action from pointer position on click
                                    if row_resp.clicked() {
                                        let cx = ui
                                            .input(|i| i.pointer.interact_pos().map(|p| p.x))
                                            .unwrap_or(f32::MAX);
                                        if cx < rect.left() + marker_w {
                                            clicked_base = row_commit_hash.clone();
                                        } else if cx < rect.left() + markers_total {
                                            clicked_target = row_commit_hash.clone();
                                        } else {
                                            clicked_hash = row_commit_hash.clone();
                                        }
                                    }

                                    let is_row_selected = row_commit_hash
                                        .as_ref()
                                        .map(|h| self.selected_commit_hash.as_deref() == Some(h))
                                        .unwrap_or(false);

                                    // Row background on hover/select
                                    if is_clickable && (row_resp.hovered() || is_row_selected) {
                                        let bg = if is_row_selected {
                                            t.bg_active
                                        } else {
                                            t.bg_hover
                                        };
                                        ui.painter().rect_filled(rect, 0.0, bg);
                                    }

                                    // Marker circles
                                    let base_marker_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(marker_w, row_h),
                                    );
                                    let tgt_marker_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.left() + marker_w, rect.top()),
                                        egui::vec2(marker_w, row_h),
                                    );
                                    if let Some(ref hash) = row_commit_hash {
                                        let is_base =
                                            self.graph_diff_base.as_deref() == Some(hash.as_str());
                                        let is_tgt = self.graph_diff_target.as_deref()
                                            == Some(hash.as_str());
                                        let painter = ui.painter();
                                        let bc = base_marker_rect.center();
                                        let tc = tgt_marker_rect.center();
                                        painter.circle_stroke(
                                            bc,
                                            4.5,
                                            egui::Stroke::new(
                                                1.5,
                                                if is_base { t.accent } else { t.border },
                                            ),
                                        );
                                        if is_base {
                                            painter.circle_filled(bc, 2.5, t.accent);
                                        }
                                        painter.circle_stroke(
                                            tc,
                                            4.5,
                                            egui::Stroke::new(
                                                1.5,
                                                if is_tgt { t.success } else { t.border },
                                            ),
                                        );
                                        if is_tgt {
                                            painter.circle_filled(tc, 2.5, t.success);
                                        }
                                    }

                                    // Render text segments using LayoutJob (no child_ui to avoid interaction blocking)
                                    // Build a single LayoutJob and paint with painter
                                    // (avoids child_ui labels consuming hover events)
                                    let text_x_start = rect.left() + markers_total;
                                    let cy = rect.center().y;
                                    let text_rect = egui::Rect::from_min_size(
                                        egui::pos2(text_x_start, rect.top()),
                                        egui::vec2(rect.width() - markers_total, row_h),
                                    );
                                    let mut job = egui::text::LayoutJob::default();
                                    let mut seen_hash_lj = false;
                                    for seg in &segments {
                                        let trimmed = seg.text.trim();
                                        let is_hash = looks_like_short_hash(trimmed);
                                        if is_hash {
                                            seen_hash_lj = true;
                                        }
                                        let display_text = if !seen_hash_lj {
                                            replace_graph_chars(&seg.text)
                                        } else {
                                            seg.text.clone()
                                        };
                                        let color = if is_hash {
                                            if is_row_selected {
                                                t.on_selection
                                            } else {
                                                t.accent
                                            }
                                        } else if let Some(col) = seg.color {
                                            if is_row_selected && col == t.text {
                                                t.on_selection
                                            } else {
                                                col
                                            }
                                        } else {
                                            if is_row_selected {
                                                t.on_selection
                                            } else {
                                                t.text
                                            }
                                        };
                                        // Single regular-weight monospace, sized to match
                                        // the branch/tag lists. The CJK bold variant
                                        // looked smeared at this size; hashes are already
                                        // set apart by color + underline.
                                        let font_id = egui::FontId::monospace(12.0);
                                        let underline = if is_hash {
                                            egui::Stroke::new(1.0, color)
                                        } else {
                                            egui::Stroke::NONE
                                        };
                                        job.append(
                                            &display_text,
                                            0.0,
                                            egui::text::TextFormat {
                                                font_id,
                                                color,
                                                underline,
                                                ..Default::default()
                                            },
                                        );
                                    }
                                    if !job.text.is_empty() {
                                        let galley = ui.fonts_mut(|f| f.layout_job(job));
                                        ui.painter_at(text_rect).galley(
                                            egui::pos2(
                                                text_x_start,
                                                cy - galley.rect.height() / 2.0,
                                            ),
                                            galley,
                                            t.text,
                                        );
                                    }
                                } // end for (idx, line) loop

                                // Paging: the log is fetched with -n graph_log_limit,
                                // so offer to extend when we likely hit that cap
                                if oneline.lines().count() >= self.graph_log_limit {
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(markers_total);
                                        if self.branch_log_loading {
                                            ui.spinner();
                                            ui.label(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "loading_commits",
                                                ))
                                                .color(t.text_dim)
                                                .size(11.0),
                                            );
                                        } else if ui
                                            .small_button(crate::i18n::t(lang, "load_more_commits"))
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            load_more = true;
                                        }
                                    });
                                    ui.add_space(4.0);
                                }
                            });
                        } else if self.branch_log_loading {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    egui::RichText::new(crate::i18n::t(
                                        lang,
                                        "fetching_commit_graph",
                                    ))
                                    .color(t.text_dim),
                                );
                            });
                        } else {
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "loading_commits"))
                                    .color(t.text_dim),
                            );
                        }
                    });

                if load_more && let Some(branch) = self.selected_branch.clone() {
                    self.graph_log_limit += 200;
                    self.request_branch_log(repo_path.to_path_buf(), branch, ui.ctx().clone());
                }
                if let Some(h) = clicked_hash {
                    self.selected_commit_hash = Some(h.clone());
                    self.request_commit_detail(repo_path.to_path_buf(), h, ui.ctx().clone());
                }
                if let Some(h) = clicked_base {
                    self.graph_diff_base = Some(h);
                }
                if let Some(h) = clicked_target {
                    self.graph_diff_target = Some(h);
                }
            });
        });
    }

    fn draw_files_tab(&mut self, ui: &mut egui::Ui, data: &RepoData) {
        let t = self.theme;
        let lang = self.prefs.language;
        let widget_frame = egui::Frame::NONE
            .fill(t.bg_elevated)
            .corner_radius(4.0)
            .inner_margin(16.0);

        egui::ScrollArea::vertical()
            .id_salt("files_tab_scroll")
            .show(ui, |ui| {
                widget_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "file_structure"))
                            .strong()
                            .size(14.0)
                            .color(t.text),
                    );
                    ui.add_space(10.0);

                    let chart_colors = [
                        t.chart[0], t.chart[1], t.chart[2], t.chart[3], t.chart[4], t.chart[5],
                        t.chart[6], t.chart[7],
                    ];

                    let total_w = ui.available_width();
                    let chart_col_w = 180.0;
                    let ext_slices: Vec<(&str, f32, Color32)> = data
                        .files
                        .extensions
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            (
                                e.ext.as_str(),
                                e.percentage as f32,
                                chart_colors[i % chart_colors.len()],
                            )
                        })
                        .collect();

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_max_width(total_w - chart_col_w - 8.0);
                            egui::Grid::new("extension_grid")
                                .num_columns(4)
                                .spacing([12.0, 8.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("");
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "extension"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "file_count"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(lang, "share"))
                                            .strong()
                                            .color(t.text),
                                    );
                                    ui.end_row();

                                    for (idx, ext) in data.files.extensions.iter().enumerate() {
                                        let color = chart_colors[idx % chart_colors.len()];
                                        ui.horizontal(|ui| {
                                            let (rect, _) = ui.allocate_exact_size(
                                                egui::vec2(8.0, 8.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(rect, 1.0, color);
                                        });
                                        ui.label(ext.ext.to_uppercase());
                                        let count_str = match lang {
                                            crate::config::Language::English => {
                                                format!("{} items", ext.count)
                                            }
                                            crate::config::Language::Japanese => {
                                                format!("{} 件", ext.count)
                                            }
                                        };
                                        ui.label(count_str);
                                        ui.label(format!("{:.1} %", ext.percentage));
                                        ui.end_row();
                                    }
                                });
                        });
                        ui.add_space(8.0);
                        ui.vertical_centered(|ui| {
                            self.draw_pie_chart(ui, &ext_slices);
                        });
                    });
                });

                ui.add_space(20.0);

                widget_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "top_large_files"))
                            .strong()
                            .size(14.0)
                            .color(t.text),
                    );
                    ui.add_space(10.0);
                    // No inner ScrollArea here: nesting one used to trap the
                    // bottom rows in a 250px viewport that the outer tab
                    // scroll could not reach. 10 rows lay out flat instead.
                    egui::Grid::new("largest_files_grid")
                        .num_columns(3)
                        .spacing([16.0, 10.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "rank"))
                                    .strong()
                                    .color(t.text),
                            );
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "file_path"))
                                    .strong()
                                    .color(t.text),
                            );
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "file_size"))
                                    .strong()
                                    .color(t.text),
                            );
                            ui.end_row();

                            for (rank, file) in data.files.largest_files.iter().enumerate() {
                                ui.label((rank + 1).to_string());
                                ui.label(&file.path).on_hover_text(&file.path);
                                ui.label(&file.size_formatted);
                                ui.end_row();
                            }
                        });
                });
            });
    }

    pub(super) fn render_register_member_modal(&mut self, ctx: &egui::Context) {
        let lang = self.prefs.language;
        let t = self.theme;
        if let Some(mut state) = self.register_member_dialog.clone() {
            let mut close = false;
            let mut save = false;

            egui::Window::new(crate::i18n::t(lang, "modal_add_member_title"))
                .collapsible(false)
                .resizable(false)
                .fixed_size(egui::vec2(420.0, 0.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {} ({})",
                            crate::i18n::t(lang, "contributor_info"),
                            state.contributor_name,
                            state.contributor_email
                        ))
                        .size(11.0)
                        .color(t.text_dim),
                    );
                    ui.add_space(10.0);

                    egui::Grid::new("reg_member_modal_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "canonical_name"))
                                    .color(t.text_dim),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut state.canonical_name)
                                    .desired_width(260.0),
                            );
                            ui.end_row();

                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "alias_label"))
                                    .color(t.text_dim),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut state.aliases_input)
                                    .desired_width(260.0),
                            );
                            ui.end_row();
                        });

                    if let Some(err) = &state.error_msg {
                        ui.add_space(6.0);
                        ui.colored_label(t.error, err);
                    }

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(crate::i18n::t(lang, "save_btn"))
                                    .fill(t.success)
                                    .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            if state.canonical_name.trim().is_empty() {
                                state.error_msg = Some(
                                    crate::i18n::t(lang, "err_enter_canonical_name").to_string(),
                                );
                            } else {
                                save = true;
                            }
                        }

                        if ui.button(crate::i18n::t(lang, "cancel_btn")).clicked() {
                            close = true;
                        }
                    });
                });

            if save {
                let aliases: Vec<String> = state
                    .aliases_input
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let new_member = crate::config::Member {
                    canonical_name: state.canonical_name.trim().to_string(),
                    aliases,
                    is_active: true,
                };
                self.members.push(new_member);
                let _ = crate::config::save_members(&self.members);
                self.session_refreshed.clear();
                if let Some(idx) = self.selected_repo_index {
                    self.load_repo_async(idx, ctx.clone());
                }
                self.push_toast(
                    crate::i18n::t(lang, "member_registered_toast").to_string(),
                    crate::app::ToastKind::Success,
                );
                close = true;
            }

            if close {
                self.register_member_dialog = None;
            } else {
                self.register_member_dialog = Some(state);
            }
        }
    }

    pub(super) fn render_add_alias_modal(&mut self, ctx: &egui::Context) {
        let lang = self.prefs.language;
        let t = self.theme;
        if self.members.is_empty() {
            self.add_alias_dialog = None;
            return;
        }
        if let Some(mut state) = self.add_alias_dialog.clone() {
            let mut close = false;
            let mut save = false;

            if state.selected_member_idx >= self.members.len() {
                state.selected_member_idx = 0;
            }

            egui::Window::new(crate::i18n::t(lang, "modal_add_alias_title"))
                .collapsible(false)
                .resizable(false)
                .fixed_size(egui::vec2(420.0, 0.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {} ({})",
                            crate::i18n::t(lang, "contributor_info"),
                            state.contributor_name,
                            state.contributor_email
                        ))
                        .size(11.0)
                        .color(t.text_dim),
                    );
                    ui.add_space(10.0);

                    egui::Grid::new("add_alias_modal_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "select_target_member"))
                                    .color(t.text_dim),
                            );

                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(
                                        &self.members[state.selected_member_idx]
                                            .canonical_name,
                                    )
                                    .strong()
                                    .color(t.text),
                                );
                                ui.add_space(4.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.filter_query)
                                        .hint_text(crate::i18n::t(
                                            lang,
                                            "filter_placeholder",
                                        ))
                                        .desired_width(180.0),
                                );
                                ui.add_space(4.0);
                                egui::ScrollArea::vertical()
                                    .max_height(160.0)
                                    .show(ui, |ui| {
                                        let q = state.filter_query.to_lowercase();
                                        for (idx, member) in
                                            self.members.iter().enumerate()
                                        {
                                            if !q.is_empty()
                                                && !member
                                                    .canonical_name
                                                    .to_lowercase()
                                                    .contains(&q)
                                            {
                                                continue;
                                            }
                                            let selected =
                                                state.selected_member_idx == idx;
                                            if ui
                                                .selectable_label(
                                                    selected,
                                                    &member.canonical_name,
                                                )
                                                .clicked()
                                            {
                                                state.selected_member_idx = idx;
                                            }
                                        }
                                    });
                            });
                            ui.end_row();
                        });

                    if let Some(err) = &state.error_msg {
                        ui.add_space(6.0);
                        ui.colored_label(t.error, err);
                    }

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(crate::i18n::t(lang, "save_btn"))
                                    .fill(t.success)
                                    .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            save = true;
                        }

                        if ui.button(crate::i18n::t(lang, "cancel_btn")).clicked() {
                            close = true;
                        }
                    });
                });

            if save {
                let member = &mut self.members[state.selected_member_idx];
                let name = state.contributor_name.trim().to_string();
                let email = state.contributor_email.trim().to_string();

                if !name.is_empty()
                    && !member.canonical_name.eq_ignore_ascii_case(&name)
                    && !member.aliases.iter().any(|a| a.eq_ignore_ascii_case(&name))
                {
                    member.aliases.push(name);
                }

                if !email.is_empty()
                    && !member
                        .aliases
                        .iter()
                        .any(|a| a.eq_ignore_ascii_case(&email))
                {
                    member.aliases.push(email);
                }

                let _ = crate::config::save_members(&self.members);
                self.session_refreshed.clear();
                if let Some(idx) = self.selected_repo_index {
                    self.load_repo_async(idx, ctx.clone());
                }
                self.push_toast(
                    crate::i18n::t(lang, "alias_added_toast").to_string(),
                    crate::app::ToastKind::Success,
                );
                close = true;
            }

            if close {
                self.add_alias_dialog = None;
            } else {
                self.add_alias_dialog = Some(state);
            }
        }
    }
}

struct TextSegment {
    text: String,
    color: Option<egui::Color32>,
    #[allow(dead_code)] // parsed from ANSI but no longer used for rendering
    bold: bool,
}

fn ansi_256_color(idx: u8) -> egui::Color32 {
    match idx {
        0 => egui::Color32::from_rgb(0, 0, 0),
        1 => egui::Color32::from_rgb(128, 0, 0),
        2 => egui::Color32::from_rgb(0, 128, 0),
        3 => egui::Color32::from_rgb(128, 128, 0),
        4 => egui::Color32::from_rgb(0, 0, 128),
        5 => egui::Color32::from_rgb(128, 0, 128),
        6 => egui::Color32::from_rgb(0, 128, 128),
        7 => egui::Color32::from_rgb(192, 192, 192),
        8 => egui::Color32::from_rgb(128, 128, 128),
        9 => egui::Color32::from_rgb(255, 0, 0),
        10 => egui::Color32::from_rgb(0, 255, 0),
        11 => egui::Color32::from_rgb(255, 255, 0),
        12 => egui::Color32::from_rgb(0, 0, 255),
        13 => egui::Color32::from_rgb(255, 0, 255),
        14 => egui::Color32::from_rgb(0, 255, 255),
        15 => egui::Color32::from_rgb(255, 255, 255),
        16..=231 => {
            let val = idx - 16;
            let r = (val / 36) * 51;
            let g = ((val % 36) / 6) * 51;
            let b = (val % 6) * 51;
            egui::Color32::from_rgb(r, g, b)
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            egui::Color32::from_rgb(gray, gray, gray)
        }
    }
}

fn replace_graph_chars(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '*' => '●',
            '|' => '│',
            '/' => '╱',
            '\\' => '╲',
            '_' => '─',
            other => other,
        })
        .collect()
}

fn parse_ansi_line(line: &str, theme: &crate::theme::Theme) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut current_color = None;
    let mut current_bold = false;

    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            if !current_text.is_empty() {
                segments.push(TextSegment {
                    text: current_text.clone(),
                    color: current_color,
                    bold: current_bold,
                });
                current_text.clear();
            }

            chars.next(); // consume '['
            let mut code = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_ascii_digit() || nc == ';' {
                    if let Some(c) = chars.next() {
                        code.push(c);
                    }
                } else if nc == 'm' {
                    chars.next(); // consume 'm'
                    break;
                } else {
                    chars.next();
                }
            }

            if code.is_empty() || code == "0" {
                current_color = None;
                current_bold = false;
            } else {
                let parts: Vec<&str> = code.split(';').collect();
                let mut i = 0;
                while i < parts.len() {
                    match parts[i] {
                        "0" => {
                            current_color = None;
                            current_bold = false;
                        }
                        "1" => {
                            current_bold = true;
                        }
                        "30" => current_color = Some(egui::Color32::from_rgb(40, 44, 52)),
                        "31" => current_color = Some(theme.error),
                        "32" => current_color = Some(theme.success),
                        "33" => current_color = Some(theme.warning),
                        "34" => current_color = Some(theme.accent),
                        "35" => current_color = Some(egui::Color32::from_rgb(198, 120, 221)),
                        "36" => current_color = Some(theme.info),
                        "37" => current_color = Some(theme.text),
                        "90" => current_color = Some(theme.text_dim),
                        "91" => current_color = Some(theme.error),
                        "92" => current_color = Some(theme.success),
                        "93" => current_color = Some(theme.warning),
                        "94" => current_color = Some(theme.accent),
                        "95" => current_color = Some(egui::Color32::from_rgb(224, 108, 117)),
                        "96" => current_color = Some(theme.info),
                        "97" => current_color = Some(theme.text),
                        "38" if i + 2 < parts.len() && parts[i + 1] == "5" => {
                            if let Ok(color_idx) = parts[i + 2].parse::<u8>() {
                                current_color = Some(ansi_256_color(color_idx));
                            }
                            i += 2;
                        }
                        "38" if i + 4 < parts.len() && parts[i + 1] == "2" => {
                            if let (Ok(r), Ok(g), Ok(b)) = (
                                parts[i + 2].parse::<u8>(),
                                parts[i + 3].parse::<u8>(),
                                parts[i + 4].parse::<u8>(),
                            ) {
                                current_color = Some(egui::Color32::from_rgb(r, g, b));
                            }
                            i += 4;
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
        } else {
            current_text.push(c);
        }
    }

    if !current_text.is_empty() {
        segments.push(TextSegment {
            text: current_text,
            color: current_color,
            bold: current_bold,
        });
    }

    segments
}

fn branch_dot_color(name: &str, t: &crate::theme::Theme) -> Color32 {
    let mut h: u32 = 0;
    for b in name.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    t.chart[(h as usize) % t.chart.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn test_format_thousands() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1000), "1,000");
        assert_eq!(format_thousands(1234567), "1,234,567");
    }

    fn get_test_theme() -> Theme {
        Theme {
            is_dark: true,
            bg: Color32::BLACK,
            bg_sidebar: Color32::BLACK,
            bg_elevated: Color32::BLACK,
            bg_hover: Color32::BLACK,
            bg_active: Color32::BLACK,
            border: Color32::BLACK,
            text: Color32::WHITE,
            text_dim: Color32::GRAY,
            text_faint: Color32::GRAY,
            accent: Color32::BLUE,
            success: Color32::GREEN,
            warning: Color32::YELLOW,
            error: Color32::RED,
            info: Color32::BLUE,
            added: Color32::GREEN,
            removed: Color32::RED,
            added_bg: Color32::GREEN,
            removed_bg: Color32::RED,
            chart: [Color32::BLACK; 8],
            selection_bg: Color32::BLUE,
            on_selection: Color32::WHITE,
            syntax_keyword: Color32::BLUE,
            syntax_type: Color32::BLUE,
            syntax_string: Color32::GREEN,
            syntax_comment: Color32::GRAY,
            syntax_number: Color32::YELLOW,
            syntax_punctuation: Color32::GRAY,
        }
    }

    #[test]
    fn test_parse_ansi_line_plain() {
        let theme = get_test_theme();
        let segments = parse_ansi_line("hello world", &theme);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "hello world");
        assert_eq!(segments[0].color, None);
        assert!(!segments[0].bold);
    }

    #[test]
    fn test_parse_ansi_line_colored() {
        let theme = get_test_theme();
        // \x1b[31mRedText\x1b[0mNormal
        let segments = parse_ansi_line("\x1b[31mRedText\x1b[0mNormal", &theme);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "RedText");
        assert_eq!(segments[0].color, Some(theme.error));
        assert!(!segments[0].bold);

        assert_eq!(segments[1].text, "Normal");
        assert_eq!(segments[1].color, None);
        assert!(!segments[1].bold);
    }

    #[test]
    fn test_parse_ansi_line_bold() {
        let theme = get_test_theme();
        // \x1b[1mBoldText\x1b[0m
        let segments = parse_ansi_line("\x1b[1mBoldText\x1b[0m", &theme);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "BoldText");
        assert_eq!(segments[0].color, None);
        assert!(segments[0].bold);
    }
}
