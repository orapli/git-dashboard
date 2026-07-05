use crate::app::{DiffMode, GitDashboardApp, PullState};
use egui::Color32;

#[derive(Clone, Debug)]
pub(super) struct RepoFilterSortInput {
    pub index: usize,
    pub name: String,
    pub path: String,
    pub last_commit_date: String,
    pub language: String,
    pub framework: String,
}

pub(crate) fn filter_and_sort_repositories(
    mut items: Vec<RepoFilterSortInput>,
    search_query: &str,
    sort_by: usize,
) -> Vec<RepoFilterSortInput> {
    let q = search_query.trim().to_lowercase();
    if !q.is_empty() {
        items.retain(|item| {
            item.name.to_lowercase().contains(&q)
                || item.path.to_lowercase().contains(&q)
                || item.language.to_lowercase().contains(&q)
                || item.framework.to_lowercase().contains(&q)
        });
    }

    match sort_by {
        0 => {
            items.sort_by_key(|item| item.name.to_lowercase());
        }
        1 => {
            items.sort_by_key(|item| std::cmp::Reverse(item.name.to_lowercase()));
        }
        2 => {
            items.sort_by(|a, b| {
                if a.last_commit_date.is_empty() && !b.last_commit_date.is_empty() {
                    std::cmp::Ordering::Greater
                } else if !a.last_commit_date.is_empty() && b.last_commit_date.is_empty() {
                    std::cmp::Ordering::Less
                } else {
                    b.last_commit_date.cmp(&a.last_commit_date)
                }
            });
        }
        3 => {
            items.sort_by(|a, b| {
                if a.last_commit_date.is_empty() && !b.last_commit_date.is_empty() {
                    std::cmp::Ordering::Greater
                } else if !a.last_commit_date.is_empty() && b.last_commit_date.is_empty() {
                    std::cmp::Ordering::Less
                } else {
                    a.last_commit_date.cmp(&b.last_commit_date)
                }
            });
        }
        _ => {}
    }
    items
}

impl GitDashboardApp {
    pub(super) fn draw_home_view(&mut self, ui: &mut egui::Ui) {
        let t = self.theme;
        let lang = self.prefs.language;
        ui.vertical(|ui| {
            ui.heading(
                egui::RichText::new(crate::i18n::t(lang, "repositories"))
                    .strong()
                    .color(t.text),
            );
            ui.add_space(8.0);

            // Overall loading progress (hidden once all repos are loaded)
            let total_repos = self.repositories.len();
            let loaded_repos = self.repo_cache.len();
            if loaded_repos < total_repos {
                ui.horizontal(|ui| {
                    ui.spinner();
                    let progress_fmt = match lang {
                        crate::config::Language::English => format!(
                            "Analysis in progress: {} of {} completed",
                            loaded_repos, total_repos
                        ),
                        crate::config::Language::Japanese => format!(
                            "分析進行中: {} 件中 {} 件の分析が完了",
                            total_repos, loaded_repos
                        ),
                    };
                    ui.label(egui::RichText::new(progress_fmt).color(t.accent).size(11.5));

                    let progress = loaded_repos as f32 / total_repos as f32;
                    ui.add(egui::ProgressBar::new(progress).show_percentage());
                });
            }

            ui.add_space(15.0);
            ui.separator();
            ui.add_space(15.0);

            // Search and sort controls
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t(lang, "search_label"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.repo_search_query)
                        .hint_text(crate::i18n::t(lang, "search_repo_hint"))
                        .desired_width(240.0),
                );

                // Host filter
                {
                    let distinct_hosts: Vec<String> = self
                        .repositories
                        .iter()
                        .filter_map(|r| r.host.clone())
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    if !distinct_hosts.is_empty() {
                        ui.add_space(20.0);
                        let all_label = crate::i18n::t(lang, "all_hosts");
                        let selected_text = self
                            .home_selected_host
                            .as_deref()
                            .unwrap_or(&all_label)
                            .to_string();
                        egui::ComboBox::from_id_salt("home_host_combobox")
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                let label = crate::i18n::t(lang, "all_hosts");
                                if ui
                                    .selectable_label(self.home_selected_host.is_none(), &label)
                                    .clicked()
                                {
                                    self.home_selected_host = None;
                                }
                                for host in &distinct_hosts {
                                    if ui
                                        .selectable_label(
                                            self.home_selected_host.as_deref()
                                                == Some(host.as_str()),
                                            host,
                                        )
                                        .clicked()
                                    {
                                        self.home_selected_host = Some(host.clone());
                                    }
                                }
                            });
                    }
                }

                ui.add_space(20.0);

                ui.label(crate::i18n::t(lang, "sort_label"));
                egui::ComboBox::from_id_salt("repo_sort_combobox")
                    .selected_text(match self.repo_sort_by {
                        0 => crate::i18n::t(lang, "sort_name_asc"),
                        1 => crate::i18n::t(lang, "sort_name_desc"),
                        2 => crate::i18n::t(lang, "sort_updated_desc"),
                        3 => crate::i18n::t(lang, "sort_updated_asc"),
                        _ => crate::i18n::t(lang, "select_option"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.repo_sort_by,
                            0,
                            crate::i18n::t(lang, "sort_name_asc"),
                        );
                        ui.selectable_value(
                            &mut self.repo_sort_by,
                            1,
                            crate::i18n::t(lang, "sort_name_desc"),
                        );
                        ui.selectable_value(
                            &mut self.repo_sort_by,
                            2,
                            crate::i18n::t(lang, "sort_updated_desc"),
                        );
                        ui.selectable_value(
                            &mut self.repo_sort_by,
                            3,
                            crate::i18n::t(lang, "sort_updated_asc"),
                        );
                    });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if super::widgets::icon_button(
                        ui,
                        "refresh",
                        crate::i18n::t(lang, "reanalyze_all_hint"),
                        &t,
                    )
                    .clicked()
                    {
                        self.repo_cache.clear();
                        self.repo_cache_gen += 1;
                        let ctx = ui.ctx().clone();
                        for idx in 0..self.repositories.len() {
                            self.load_repo_async(idx, ctx.clone());
                        }
                    }
                    ui.add_space(4.0);
                    if super::widgets::icon_button(
                        ui,
                        "fetch",
                        crate::i18n::t(lang, "fetch_check_hint"),
                        &t,
                    )
                    .clicked()
                    {
                        self.run_bulk_fetch(ui.ctx().clone());
                    }
                    ui.add_space(4.0);
                    if super::widgets::icon_button(
                        ui,
                        "download",
                        crate::i18n::t(lang, "pull_all_hint"),
                        &t,
                    )
                    .clicked()
                    {
                        self.run_bulk_pull(ui.ctx().clone());
                    }
                });
            });
            ui.add_space(15.0);

            // Minimal ZED-style widget frame
            let widget_frame = egui::Frame::NONE
                .fill(t.bg_elevated)
                .corner_radius(4.0)
                .inner_margin(16.0);

            let mut clicked_idx = None;
            let mut clicked_uncommitted_idx = None;
            let mut to_clear_statuses = Vec::new();
            let mut pending_toast_errors: Vec<String> = Vec::new();
            let mut refresh_repo_idx: Option<usize> = None;

            // Memoized on (query, sort, cache generation): rebuilding the
            // list (a clone of every repo's fields + a sort) every frame
            // violated the per-frame-computation rule with many repos
            let cache_gen = self.repo_cache_gen;
            let items = match self.home_items_cache.take() {
                Some((q, h, s, g, items))
                    if q == self.repo_search_query
                        && h == self.home_selected_host
                        && s == self.repo_sort_by
                        && g == cache_gen =>
                {
                    items
                }
                _ => {
                    let input_items: Vec<RepoFilterSortInput> = self
                        .repositories
                        .iter()
                        .enumerate()
                        .filter(|(_, repo)| match &self.home_selected_host {
                            None => true,
                            Some(h) => repo.host.as_deref() == Some(h.as_str()),
                        })
                        .map(|(idx, repo)| {
                            let (last_commit_date, language, framework) =
                                if let Some(Ok(data)) = self.repo_cache.get(&idx) {
                                    let date = data
                                        .recent_commits
                                        .first()
                                        .map(|c| c.date.clone())
                                        .unwrap_or_default();
                                    (
                                        date,
                                        data.tech_info.language.clone(),
                                        data.tech_info.framework.clone(),
                                    )
                                } else {
                                    (String::new(), String::new(), String::new())
                                };

                            RepoFilterSortInput {
                                index: idx,
                                name: repo.name.clone(),
                                path: repo.path.to_string_lossy().to_string(),
                                last_commit_date,
                                language,
                                framework,
                            }
                        })
                        .collect();

                    filter_and_sort_repositories(
                        input_items,
                        &self.repo_search_query,
                        self.repo_sort_by,
                    )
                }
            };

            widget_frame.show(ui, |ui| {
                if self.repositories.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "no_repos_title"))
                                .strong()
                                .size(14.0)
                                .color(t.text_dim),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "no_repos_sub"))
                                .color(t.text_dim),
                        );
                        ui.add_space(40.0);
                    });
                } else if items.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "no_matching_repos_title"))
                                .strong()
                                .size(14.0)
                                .color(t.text_dim),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "no_matching_repos_sub"))
                                .color(t.text_dim),
                        );
                        ui.add_space(40.0);
                    });
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.set_max_width(ui.available_width() - 12.0);

                        for item in &items {
                            let idx = item.index;
                            let repo = &self.repositories[idx];

                            let available_w = ui.available_width();
                            let row_h = 56.0;
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(available_w, row_h),
                                egui::Sense::click(),
                            );
                            // Offscreen rows keep scroll geometry but skip the
                            // multiple galley layouts + painting per row
                            if !ui.is_rect_visible(rect) {
                                continue;
                            }
                            let resp = resp
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(crate::i18n::t(lang, "click_to_view_details"));

                            // Background (on hover)
                            let bg = if resp.hovered() {
                                t.bg_hover
                            } else {
                                Color32::TRANSPARENT
                            };
                            if bg != Color32::TRANSPARENT {
                                ui.painter().rect_filled(rect, 4.0, bg);
                            }

                            // Bottom separator line
                            ui.painter().line_segment(
                                [rect.left_bottom(), rect.right_bottom()],
                                egui::Stroke::new(1.0, t.border),
                            );

                            let painter = ui.painter_at(rect);
                            let cy = rect.center().y;
                            let left = rect.left();

                            // 1. Status dot
                            let dot_color = match self.pull_statuses.get(&repo.path) {
                                Some(PullState::Pulling) => t.accent,
                                Some(PullState::Success(_)) => t.success,
                                Some(PullState::Error(_)) => t.error,
                                _ => t.text_faint,
                            };
                            painter.circle_filled(egui::pos2(left + 16.0, cy), 4.0, dot_color);

                            // 2. Retrieve display data
                            let cache_entry = self.repo_cache.get(&idx);
                            let mut branch = "-".to_string();
                            let mut lang_str = "-".to_string();
                            let mut fw_str = "-".to_string();
                            let commit_msg;
                            let mut commit_date = "-".to_string();
                            let mut is_loading = false;
                            let mut is_error = false;
                            let mut err_msg = String::new();

                            match cache_entry {
                                Some(Ok(data)) => {
                                    branch = data.summary.current_branch.clone();

                                    lang_str = if data.tech_info.language == "-" {
                                        "-".to_string()
                                    } else if data.tech_info.lang_version == "-" {
                                        data.tech_info.language.clone()
                                    } else {
                                        format!(
                                            "{} ({})",
                                            data.tech_info.language, data.tech_info.lang_version
                                        )
                                    };

                                    fw_str = if data.tech_info.framework == "-" {
                                        "-".to_string()
                                    } else if data.tech_info.framework_version == "-" {
                                        data.tech_info.framework.clone()
                                    } else {
                                        format!(
                                            "{} ({})",
                                            data.tech_info.framework,
                                            data.tech_info.framework_version
                                        )
                                    };

                                    if let Some(c) = data.recent_commits.first() {
                                        commit_msg = c.message.clone();
                                        commit_date = c.date.clone();
                                    } else {
                                        commit_msg = crate::i18n::t(lang, "no_commits").to_string();
                                        commit_date = crate::i18n::t(lang, "unknown").to_string();
                                    }
                                }
                                Some(Err(err)) => {
                                    is_error = true;
                                    err_msg = err.clone();
                                    commit_msg = match lang {
                                        crate::config::Language::English => {
                                            format!("Analysis failed: {}", err)
                                        }
                                        crate::config::Language::Japanese => {
                                            format!("分析失敗: {}", err)
                                        }
                                    };
                                }
                                None => {
                                    is_loading = true;
                                    commit_msg = crate::i18n::t(lang, "analyzing").to_string();
                                }
                            }

                            // 3. Text rendering
                            let repo_name_text = &repo.name;
                            let repo_name_font = egui::FontId::proportional(14.0);
                            let metadata_font = egui::FontId::proportional(11.0);

                            let line1_y = rect.top() + 16.0;
                            let line2_y = rect.top() + 36.0;

                            // Repository name
                            let name_color = if is_error {
                                t.error
                            } else if is_loading {
                                t.text_dim
                            } else if resp.hovered() {
                                t.accent
                            } else {
                                t.text
                            };
                            let name_galley = painter.layout_no_wrap(
                                repo_name_text.to_string(),
                                repo_name_font,
                                name_color,
                            );
                            let name_w = name_galley.size().x;
                            painter.galley(
                                egui::pos2(left + 36.0, line1_y - 2.0),
                                name_galley,
                                t.text,
                            );

                            // Sync status badge
                            let sync_badge = if let Some(Ok(data)) = cache_entry {
                                if !data.summary.has_remote {
                                    Some((
                                        crate::i18n::t(lang, "local_only").to_string(),
                                        t.text_faint,
                                    ))
                                } else if !data.summary.has_upstream {
                                    Some((
                                        crate::i18n::t(lang, "no_upstream").to_string(),
                                        t.text_faint,
                                    ))
                                } else if data.summary.behind > 0 {
                                    Some((format!("↓{}", data.summary.behind), t.warning))
                                } else if data.summary.ahead > 0 {
                                    Some((format!("↑{}", data.summary.ahead), t.text_dim))
                                } else {
                                    Some((
                                        crate::i18n::t(lang, "up_to_date").to_string(),
                                        t.success,
                                    ))
                                }
                            } else {
                                None
                            };

                            let uncommitted_count = if let Some(Ok(data)) = cache_entry {
                                data.summary.uncommitted_changes
                            } else {
                                0
                            };

                            let mut badge_w = 0.0;
                            if let Some((text, color)) = &sync_badge {
                                let badge_galley = painter.layout_no_wrap(
                                    text.clone(),
                                    egui::FontId::proportional(11.0),
                                    *color,
                                );
                                let badge_rect = egui::Rect::from_min_max(
                                    egui::pos2(left + 36.0 + name_w + 8.0, line1_y - 8.0),
                                    egui::pos2(
                                        left + 36.0 + name_w + 8.0 + badge_galley.size().x + 8.0,
                                        line1_y + 8.0,
                                    ),
                                );
                                painter.rect_filled(
                                    badge_rect,
                                    2.0,
                                    Color32::from_rgba_unmultiplied(
                                        color.r(),
                                        color.g(),
                                        color.b(),
                                        20,
                                    ),
                                );
                                painter.rect_stroke(
                                    badge_rect,
                                    2.0,
                                    egui::Stroke::new(
                                        1.0,
                                        Color32::from_rgba_unmultiplied(
                                            color.r(),
                                            color.g(),
                                            color.b(),
                                            40,
                                        ),
                                    ),
                                    egui::StrokeKind::Middle,
                                );
                                painter.galley(
                                    egui::pos2(badge_rect.left() + 4.0, line1_y - 7.5),
                                    badge_galley,
                                    t.text,
                                );
                                badge_w = badge_rect.width() + 8.0;
                            }

                            let mut is_uncommitted_clicked = false;
                            if uncommitted_count > 0 {
                                let text = match lang {
                                    crate::config::Language::English => {
                                        format!("{} changes", uncommitted_count)
                                    }
                                    crate::config::Language::Japanese => {
                                        format!("変更 {}件", uncommitted_count)
                                    }
                                };
                                let color = t.warning;
                                let badge_galley = painter.layout_no_wrap(
                                    text,
                                    egui::FontId::proportional(11.0),
                                    color,
                                );
                                let badge_rect = egui::Rect::from_min_max(
                                    egui::pos2(left + 36.0 + name_w + 8.0 + badge_w, line1_y - 8.0),
                                    egui::pos2(
                                        left + 36.0
                                            + name_w
                                            + 8.0
                                            + badge_w
                                            + badge_galley.size().x
                                            + 8.0,
                                        line1_y + 8.0,
                                    ),
                                );

                                let badge_resp =
                                    ui.interact(
                                        badge_rect,
                                        ui.make_persistent_id(("uncommitted_badge", idx)),
                                        egui::Sense::click(),
                                    )
                                    .on_hover_text(
                                        crate::i18n::t(lang, "show_uncommitted_diff_hint"),
                                    );
                                let badge_hovered = badge_resp.hovered();

                                let bg_alpha = if badge_hovered { 40 } else { 20 };
                                painter.rect_filled(
                                    badge_rect,
                                    2.0,
                                    Color32::from_rgba_unmultiplied(
                                        color.r(),
                                        color.g(),
                                        color.b(),
                                        bg_alpha,
                                    ),
                                );
                                painter.rect_stroke(
                                    badge_rect,
                                    2.0,
                                    egui::Stroke::new(
                                        1.0,
                                        Color32::from_rgba_unmultiplied(
                                            color.r(),
                                            color.g(),
                                            color.b(),
                                            60,
                                        ),
                                    ),
                                    egui::StrokeKind::Middle,
                                );
                                painter.galley(
                                    egui::pos2(badge_rect.left() + 4.0, line1_y - 7.5),
                                    badge_galley,
                                    t.text,
                                );
                                badge_w += badge_rect.width() + 8.0;

                                if badge_resp.clicked() {
                                    clicked_uncommitted_idx = Some(idx);
                                    is_uncommitted_clicked = true;
                                }
                            }

                            // Latest commit message (placed to the right of name and badge)
                            let max_commit_w = available_w - name_w - badge_w - 80.0;
                            if max_commit_w > 50.0 {
                                let commit_text = if is_loading {
                                    crate::i18n::t(lang, "loading_analysis_data").to_string()
                                } else {
                                    commit_msg
                                };
                                // Simple character-count truncation
                                let commit_display = if commit_text.chars().count() > 60 {
                                    commit_text.chars().take(57).collect::<String>() + "..."
                                } else {
                                    commit_text
                                };
                                let commit_galley = painter.layout_no_wrap(
                                    commit_display,
                                    egui::FontId::proportional(12.0),
                                    t.text_faint,
                                );
                                painter.galley(
                                    egui::pos2(left + 36.0 + name_w + 8.0 + badge_w, line1_y - 1.0),
                                    commit_galley,
                                    t.text,
                                );
                            }

                            // Metadata sub-row (language · framework · branch · last updated)
                            let mut metadata_parts = Vec::new();
                            if lang_str != "-" {
                                metadata_parts.push(lang_str);
                            }
                            if fw_str != "-" {
                                metadata_parts.push(fw_str);
                            }
                            if branch != "-" {
                                metadata_parts.push(format!("branch: {}", branch));
                            }
                            if commit_date != "-" {
                                metadata_parts.push(commit_date);
                            }

                            let metadata_text = if is_error {
                                match lang {
                                    crate::config::Language::English => {
                                        format!("Error: {}", err_msg)
                                    }
                                    crate::config::Language::Japanese => {
                                        format!("エラー: {}", err_msg)
                                    }
                                }
                            } else if is_loading {
                                crate::i18n::t(lang, "loading").to_string()
                            } else if metadata_parts.is_empty() {
                                crate::i18n::t(lang, "no_metadata").to_string()
                            } else {
                                metadata_parts.join("  ·  ")
                            };

                            let metadata_galley =
                                painter.layout_no_wrap(metadata_text, metadata_font, t.text_dim);
                            painter.galley(
                                egui::pos2(left + 36.0, line2_y - 2.0),
                                metadata_galley,
                                t.text,
                            );

                            // Right-side action buttons (refresh / open in editor / status clear)
                            let has_status_clear = self.pull_statuses.contains_key(&repo.path);
                            let editor_btn_right = if has_status_clear {
                                rect.right() - 44.0
                            } else {
                                rect.right() - 16.0
                            };
                            let editor_btn_rect = egui::Rect::from_min_max(
                                egui::pos2(editor_btn_right - 22.0, cy - 11.0),
                                egui::pos2(editor_btn_right, cy + 11.0),
                            );
                            let refresh_btn_rect = egui::Rect::from_min_max(
                                egui::pos2(editor_btn_rect.left() - 26.0, cy - 11.0),
                                egui::pos2(editor_btn_rect.left() - 4.0, cy + 11.0),
                            );

                            let mut is_editor_clicked = false;
                            let mut is_refresh_clicked = false;

                            // Refresh button (visible only on row hover to reduce visual noise)
                            // Use interact (not allocate_rect) so the layout cursor isn't pulled
                            // back to the row's vertical center, which would overlap the next row.
                            let refresh_resp = ui
                                .interact(
                                    refresh_btn_rect,
                                    ui.make_persistent_id(("refresh_btn", idx)),
                                    egui::Sense::click(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(crate::i18n::t(lang, "reanalyze"));
                            let row_hovered = resp.hovered();
                            if refresh_resp.hovered() {
                                painter.rect_filled(refresh_btn_rect, 4.0, t.bg_hover);
                            }
                            let refresh_icon_color = if refresh_resp.hovered() {
                                t.accent
                            } else if row_hovered {
                                t.text_dim
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            if refresh_icon_color != egui::Color32::TRANSPARENT {
                                super::widgets::draw_icon_refresh_pub(
                                    &painter,
                                    refresh_btn_rect.shrink(3.0),
                                    refresh_icon_color,
                                );
                            }
                            if refresh_resp.clicked() && !is_loading {
                                refresh_repo_idx = Some(idx);
                                is_refresh_clicked = true;
                            }

                            let editor_resp = ui
                                .interact(
                                    editor_btn_rect,
                                    ui.make_persistent_id(("editor_btn", idx)),
                                    egui::Sense::click(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(crate::i18n::t(lang, "open_in_editor_hint"));
                            if editor_resp.hovered() {
                                painter.rect_filled(editor_btn_rect, 4.0, t.bg_hover);
                            }
                            let editor_icon_color = if editor_resp.hovered() {
                                t.accent
                            } else {
                                t.text_dim
                            };
                            super::widgets::draw_icon_editor_pub(
                                &painter,
                                editor_btn_rect.shrink(3.0),
                                editor_icon_color,
                            );
                            if editor_resp.clicked() {
                                if let Some(err) = self.open_in_editor(&repo.path) {
                                    pending_toast_errors.push(err);
                                }
                                is_editor_clicked = true;
                            }

                            // Right-edge × button (status clear)
                            let is_button_clicked = if self.pull_statuses.contains_key(&repo.path) {
                                let btn_rect = egui::Rect::from_min_max(
                                    egui::pos2(rect.right() - 36.0, cy - 10.0),
                                    egui::pos2(rect.right() - 16.0, cy + 10.0),
                                );
                                let btn_resp = ui
                                    .interact(
                                        btn_rect,
                                        ui.make_persistent_id(("status_clear", idx)),
                                        egui::Sense::click(),
                                    )
                                    .on_hover_text(crate::i18n::t(lang, "clear_status_hint"));

                                let btn_hovered = btn_resp.hovered();
                                if btn_hovered {
                                    painter.rect_filled(btn_rect, 2.0, t.bg_hover);
                                }
                                let icon_color = if btn_hovered { t.accent } else { t.text_dim };
                                super::widgets::draw_icon_close(
                                    &painter,
                                    btn_rect.shrink(3.0),
                                    icon_color,
                                );

                                if btn_resp.clicked() {
                                    to_clear_statuses.push(repo.path.clone());
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            if resp.clicked()
                                && !is_button_clicked
                                && !is_editor_clicked
                                && !is_refresh_clicked
                                && !is_uncommitted_clicked
                            {
                                clicked_idx = Some(idx);
                            }
                        }
                    });
                }
            });

            if let Some(idx) = clicked_uncommitted_idx {
                self.diff_repo_idx = Some(idx);
                self.diff_mode = DiffMode::Single;
                self.diff_base = None;
                self.diff_target = Some("WORKING_TREE".to_string());
                self.viewing_diff = true;
                self.reload_diff_files(ui.ctx().clone());
                self.selected_repo_index = None;
            }

            // Put the (possibly rebuilt) list back for the next frame
            self.home_items_cache = Some((
                self.repo_search_query.clone(),
                self.home_selected_host.clone(),
                self.repo_sort_by,
                cache_gen,
                items,
            ));

            if let Some(idx) = clicked_idx {
                self.select_repository(idx, ui.ctx().clone());
            }

            for path in to_clear_statuses {
                self.pull_statuses.remove(&path);
            }
            for err in pending_toast_errors {
                self.push_toast(err, super::ToastKind::Error);
            }
            if let Some(idx) = refresh_repo_idx {
                // Keep stale data visible; the reload replaces it when done
                self.load_repo_async(idx, ui.ctx().clone());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_and_sort_repositories() {
        let items = vec![
            RepoFilterSortInput {
                index: 0,
                name: "rust-project".to_string(),
                path: "/path/to/rust".to_string(),
                last_commit_date: "2026-06-01 12:00".to_string(),
                language: "Rust".to_string(),
                framework: "egui".to_string(),
            },
            RepoFilterSortInput {
                index: 1,
                name: "rails-project".to_string(),
                path: "/path/to/rails".to_string(),
                last_commit_date: "2026-06-05 10:00".to_string(),
                language: "Ruby".to_string(),
                framework: "Rails".to_string(),
            },
            RepoFilterSortInput {
                index: 2,
                name: "django-project".to_string(),
                path: "/path/to/django".to_string(),
                last_commit_date: "2026-06-03 14:00".to_string(),
                language: "Python".to_string(),
                framework: "Django".to_string(),
            },
        ];

        let filtered = filter_and_sort_repositories(items.clone(), "ruby", 0);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "rails-project");

        let sorted_name_asc = filter_and_sort_repositories(items.clone(), "", 0);
        assert_eq!(sorted_name_asc[0].name, "django-project");
        assert_eq!(sorted_name_asc[1].name, "rails-project");
        assert_eq!(sorted_name_asc[2].name, "rust-project");

        let sorted_name_desc = filter_and_sort_repositories(items.clone(), "", 1);
        assert_eq!(sorted_name_desc[0].name, "rust-project");
        assert_eq!(sorted_name_desc[1].name, "rails-project");
        assert_eq!(sorted_name_desc[2].name, "django-project");

        let sorted_date_desc = filter_and_sort_repositories(items.clone(), "", 2);
        assert_eq!(sorted_date_desc[0].name, "rails-project");
        assert_eq!(sorted_date_desc[1].name, "django-project");
        assert_eq!(sorted_date_desc[2].name, "rust-project");

        let sorted_date_asc = filter_and_sort_repositories(items.clone(), "", 3);
        assert_eq!(sorted_date_asc[0].name, "rust-project");
        assert_eq!(sorted_date_asc[1].name, "django-project");
        assert_eq!(sorted_date_asc[2].name, "rails-project");
    }
}
