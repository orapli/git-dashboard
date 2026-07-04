use crate::app::GitDashboardApp;
use crate::config::{self, Language, Member, Repository};
use crate::git;
use std::path::PathBuf;

/// Reveal a directory in the OS file manager (Finder / Explorer / xdg-open).
fn open_in_file_manager(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = git::quiet_command("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = git::quiet_command("explorer").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = git::quiet_command("xdg-open").arg(path).spawn();
}

/// Fixed-width label column + content, for consistent two-column alignment
/// across settings sections — same shape as aero-grep's `settings_row()`
/// (app.rs). Sections migrate to this helper incrementally, one at a time.
fn settings_row(
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
    label: impl AsRef<str>,
    content: impl FnOnce(&mut egui::Ui),
) {
    let label = label.as_ref();
    ui.horizontal(|ui| {
        let row_h = ui.spacing().interact_size.y;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(152.0, row_h), egui::Sense::hover());
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.0),
            theme.text_dim,
        );
        content(ui);
    });
    ui.add_space(4.0);
}

impl GitDashboardApp {
    // Settings and member management screen
    pub(super) fn draw_settings_view(&mut self, ui: &mut egui::Ui) {
        let t = self.theme;
        let lang = self.prefs.language;
        let card_frame = egui::Frame::NONE
            .fill(t.bg_elevated)
            .corner_radius(4.0)
            .inner_margin(16.0);

        // Header
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new(crate::i18n::t(lang, "settings"))
                    .strong()
                    .color(t.text),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(crate::i18n::t(lang, "open_config_folder"))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(crate::i18n::t(lang, "open_config_folder_tooltip"))
                    .clicked()
                {
                    open_in_file_manager(&config::get_config_dir());
                }
            });
        });
        ui.add_space(12.0);

        // Tab bar: segmented selectable buttons
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
            let tab_labels = [
                crate::i18n::t(lang, "tab_repos"),
                crate::i18n::t(lang, "tab_members"),
                crate::i18n::t(lang, "tab_appearance"),
                crate::i18n::t(lang, "about"),
            ];
            for (i, label) in tab_labels.iter().enumerate() {
                let is_active = self.settings_tab == i;
                let color = if is_active { t.accent } else { t.text_dim };
                if ui
                    .add(egui::Button::selectable(
                        is_active,
                        egui::RichText::new(label.clone()).size(12.0).color(color),
                    ))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.settings_tab = i;
                }
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(16.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.settings_tab == 0 {
                // Repository management
                ui.label(
                    egui::RichText::new(crate::i18n::t(lang, "add_repo"))
                        .strong()
                        .size(13.0)
                        .color(t.text),
                );
                ui.add_space(8.0);

                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut r1 = None;
                    let mut r2 = None;
                    egui::Grid::new("repo_form_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "display_name"))
                                    .color(t.text_dim),
                            );
                            r1 = Some(
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_repo_name)
                                        .desired_width(f32::INFINITY)
                                        .hint_text(crate::i18n::t(lang, "display_name_hint")),
                                ),
                            );
                            ui.end_row();

                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "path_label"))
                                    .color(t.text_dim),
                            );
                            r2 = Some(
                                ui.horizontal(|ui| {
                                    let resp = ui
                                        .add(
                                            egui::TextEdit::singleline(&mut self.new_repo_path)
                                                .desired_width(ui.available_width() - 80.0)
                                                .hint_text(crate::i18n::t(lang, "path_hint")),
                                        )
                                        .on_hover_text(crate::i18n::t(lang, "remote_repo_hint"));
                                    let pick = ui
                                        .button(crate::i18n::t(lang, "browse_btn"))
                                        .on_hover_text(crate::i18n::t(lang, "browse_folder_hint"));
                                    if pick.clicked()
                                        && let Some(paths) = rfd::FileDialog::new().pick_folders()
                                    {
                                        if paths.len() == 1 {
                                            // Single pick: fill the form as before
                                            let path = &paths[0];
                                            self.new_repo_path = path.to_string_lossy().to_string();
                                            if self.new_repo_name.trim().is_empty()
                                                && let Some(folder_name) = path.file_name()
                                            {
                                                self.new_repo_name =
                                                    folder_name.to_string_lossy().to_string();
                                            }
                                        } else if !paths.is_empty() {
                                            // Bulk pick: register every valid repo directly,
                                            // defaulting each name to its folder name
                                            let mut added = 0usize;
                                            let mut skipped: Vec<String> = Vec::new();
                                            for path in paths {
                                                let name = path
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().to_string())
                                                    .unwrap_or_else(|| {
                                                        path.to_string_lossy().to_string()
                                                    });
                                                if self.repositories.iter().any(|r| r.path == path)
                                                {
                                                    skipped.push(format!(
                                                        "{name} {}",
                                                        crate::i18n::t(lang, "already_registered")
                                                    ));
                                                } else if !git::is_git_repo(&path) {
                                                    skipped.push(format!(
                                                        "{name} {}",
                                                        crate::i18n::t(lang, "not_a_git_repo")
                                                    ));
                                                } else {
                                                    self.repo_cache_gen += 1;
                                                    self.repositories
                                                        .push(Repository { name, path });
                                                    added += 1;
                                                }
                                            }
                                            if added > 0 {
                                                let _ =
                                                    config::save_repositories(&self.repositories);
                                                self.repo_added_at =
                                                    Some(std::time::Instant::now());
                                                let msg = match lang {
                                                    crate::config::Language::English => {
                                                        format!("Added {} repository(ies)", added)
                                                    }
                                                    crate::config::Language::Japanese => format!(
                                                        "{}件のリポジトリを追加しました",
                                                        added
                                                    ),
                                                };
                                                self.push_toast(msg, super::ToastKind::Success);
                                            }
                                            if !skipped.is_empty() {
                                                let msg = match lang {
                                                    crate::config::Language::English => {
                                                        format!("Skipped: {}", skipped.join(", "))
                                                    }
                                                    crate::config::Language::Japanese => {
                                                        format!("スキップ: {}", skipped.join(", "))
                                                    }
                                                };
                                                self.push_toast(msg, super::ToastKind::Error);
                                            }
                                        }
                                    }
                                    resp
                                })
                                .inner,
                            );
                            ui.end_row();
                        });

                    ui.add_space(8.0);

                    if let Some(err) = &self.repo_add_error {
                        ui.colored_label(t.error, err);
                        ui.add_space(4.0);
                    }
                    // Save success feedback (shown for 2 seconds)
                    if let Some(t_instant) = self.repo_added_at {
                        if t_instant.elapsed().as_secs_f32() < 2.0 {
                            ui.colored_label(t.success, crate::i18n::t(lang, "added_success"));
                            ui.add_space(4.0);
                            ui.ctx().request_repaint();
                        } else {
                            self.repo_added_at = None;
                        }
                    }

                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let mut should_add_repo = false;
                    let r1_lost = r1.as_ref().map(|r| r.lost_focus()).unwrap_or(false);
                    let r2_lost = r2.as_ref().map(|r| r.lost_focus()).unwrap_or(false);
                    if (r1_lost || r2_lost) && enter_pressed {
                        should_add_repo = true;
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(crate::i18n::t(lang, "add_repo_btn"))
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(t.accent)
                            .stroke(egui::Stroke::NONE),
                        )
                        .clicked()
                    {
                        should_add_repo = true;
                    }

                    if should_add_repo {
                        let path = PathBuf::from(&self.new_repo_path);
                        if self.new_repo_name.trim().is_empty()
                            || self.new_repo_path.trim().is_empty()
                        {
                            self.repo_add_error =
                                Some(crate::i18n::t(lang, "err_enter_name_and_path").to_string());
                        } else if !git::is_git_repo(&path) {
                            self.repo_add_error = Some(if git::parse_ssh_repo(&path).is_some() {
                                crate::i18n::t(lang, "err_ssh_unreachable").to_string()
                            } else {
                                crate::i18n::t(lang, "err_invalid_git_repo").to_string()
                            });
                        } else {
                            self.repo_cache_gen += 1;
                            self.repositories.push(Repository {
                                name: self.new_repo_name.trim().to_string(),
                                path: path.clone(),
                            });
                            let _ = config::save_repositories(&self.repositories);
                            self.new_repo_name.clear();
                            self.new_repo_path.clear();
                            self.repo_add_error = None;
                            self.repo_added_at = Some(std::time::Instant::now());
                        }
                    }
                });

                ui.add_space(20.0);
                let repo_count_title = match lang {
                    crate::config::Language::English => format!(
                        "Registered Repositories ({} total)",
                        self.repositories.len()
                    ),
                    crate::config::Language::Japanese => {
                        format!("登録済みリポジトリ  ({}件)", self.repositories.len())
                    }
                };
                ui.label(
                    egui::RichText::new(repo_count_title)
                        .strong()
                        .size(13.0)
                        .color(t.text),
                );
                ui.add_space(8.0);

                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if self.repositories.is_empty() {
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "no_registered_repos"))
                                .color(t.text_faint),
                        );
                    } else {
                        let mut to_remove = None;
                        let mut save_edit_repo = None;
                        for (idx, repo) in self.repositories.iter().enumerate() {
                            if self.editing_repo_idx == Some(idx) {
                                let mut r_edit1 = None;
                                let mut r_edit2 = None;
                                ui.vertical(|ui| {
                                    egui::Grid::new(format!("edit_repo_grid_{}", idx))
                                        .num_columns(2)
                                        .spacing([12.0, 8.0])
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "display_name",
                                                ))
                                                .color(t.text_dim),
                                            );
                                            r_edit1 = Some(
                                                ui.add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.editing_repo_name,
                                                    )
                                                    .desired_width(f32::INFINITY),
                                                ),
                                            );
                                            ui.end_row();

                                            ui.label(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "path_label",
                                                ))
                                                .color(t.text_dim),
                                            );
                                            r_edit2 = Some(
                                                ui.horizontal(|ui| {
                                                    let resp = ui.add(
                                                        egui::TextEdit::singleline(
                                                            &mut self.editing_repo_path,
                                                        )
                                                        .desired_width(ui.available_width() - 80.0),
                                                    );
                                                    if ui
                                                        .button(crate::i18n::t(lang, "browse_btn"))
                                                        .clicked()
                                                        && let Some(path) =
                                                            rfd::FileDialog::new().pick_folder()
                                                    {
                                                        self.editing_repo_path =
                                                            path.to_string_lossy().to_string();
                                                        if self.editing_repo_name.trim().is_empty()
                                                            && let Some(folder_name) =
                                                                path.file_name()
                                                        {
                                                            self.editing_repo_name = folder_name
                                                                .to_string_lossy()
                                                                .to_string();
                                                        }
                                                    }
                                                    resp
                                                })
                                                .inner,
                                            );
                                            ui.end_row();
                                        });

                                    if let Some(err) = &self.editing_repo_error {
                                        ui.add_space(4.0);
                                        ui.colored_label(t.error, err);
                                    }

                                    ui.add_space(8.0);

                                    let enter_pressed =
                                        ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    let mut should_save_edit = false;
                                    let r_edit1_lost =
                                        r_edit1.as_ref().map(|r| r.lost_focus()).unwrap_or(false);
                                    let r_edit2_lost =
                                        r_edit2.as_ref().map(|r| r.lost_focus()).unwrap_or(false);
                                    if (r_edit1_lost || r_edit2_lost) && enter_pressed {
                                        should_save_edit = true;
                                    }

                                    ui.horizontal(|ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(crate::i18n::t(lang, "save_btn"))
                                                    .fill(t.success)
                                                    .stroke(egui::Stroke::NONE),
                                            )
                                            .clicked()
                                        {
                                            should_save_edit = true;
                                        }

                                        if should_save_edit {
                                            let path = PathBuf::from(&self.editing_repo_path);
                                            if self.editing_repo_name.trim().is_empty()
                                                || self.editing_repo_path.trim().is_empty()
                                            {
                                                self.editing_repo_error = Some(
                                                    crate::i18n::t(lang, "err_enter_name_and_path")
                                                        .to_string(),
                                                );
                                            } else if !git::is_git_repo(&path) {
                                                self.editing_repo_error = Some(
                                                    crate::i18n::t(lang, "err_invalid_git_repo")
                                                        .to_string(),
                                                );
                                            } else {
                                                save_edit_repo = Some((
                                                    idx,
                                                    self.editing_repo_name.trim().to_string(),
                                                    path,
                                                ));
                                            }
                                        }
                                        if ui.button(crate::i18n::t(lang, "cancel_btn")).clicked() {
                                            self.editing_repo_idx = None;
                                            self.editing_repo_error = None;
                                        }
                                    });
                                });
                            } else if self.confirm_delete_repo_idx == Some(idx) {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(
                                            lang,
                                            "confirm_delete_repo",
                                        ))
                                        .color(t.error)
                                        .strong(),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button(crate::i18n::t(lang, "cancel_btn"))
                                                .clicked()
                                            {
                                                self.confirm_delete_repo_idx = None;
                                            }
                                            ui.add_space(8.0);
                                            if ui
                                                .add(
                                                    egui::Button::new(crate::i18n::t(
                                                        lang,
                                                        "yes_delete",
                                                    ))
                                                    .fill(t.error)
                                                    .stroke(egui::Stroke::NONE),
                                                )
                                                .clicked()
                                            {
                                                to_remove = Some(idx);
                                            }
                                        },
                                    );
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(&repo.name).strong().color(t.text),
                                        );
                                        ui.label(
                                            egui::RichText::new(repo.path.to_string_lossy())
                                                .size(10.0)
                                                .color(t.text_faint),
                                        );
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if super::widgets::icon_button(
                                                ui,
                                                "trash",
                                                crate::i18n::t(lang, "delete_action"),
                                                &t,
                                            )
                                            .clicked()
                                            {
                                                self.confirm_delete_repo_idx = Some(idx);
                                            }
                                            ui.add_space(4.0);
                                            if super::widgets::icon_button(
                                                ui,
                                                "pencil",
                                                crate::i18n::t(lang, "edit_action"),
                                                &t,
                                            )
                                            .clicked()
                                            {
                                                self.editing_repo_idx = Some(idx);
                                                self.editing_repo_name = repo.name.clone();
                                                self.editing_repo_path =
                                                    repo.path.to_string_lossy().to_string();
                                                self.editing_repo_error = None;
                                            }
                                        },
                                    );
                                });
                            }
                            if idx + 1 < self.repositories.len() {
                                ui.add(egui::Separator::default().spacing(8.0));
                            }
                        }

                        if let Some(idx) = to_remove {
                            self.repositories.remove(idx);
                            let _ = config::save_repositories(&self.repositories);
                            self.selected_repo_index = None;
                            self.repo_cache.clear();
                            self.repo_cache_gen += 1;
                            self.confirm_delete_repo_idx = None;
                        }

                        if let Some((idx, name, path)) = save_edit_repo {
                            self.repositories[idx].name = name;
                            self.repositories[idx].path = path;
                            let _ = config::save_repositories(&self.repositories);
                            self.editing_repo_idx = None;
                            self.editing_repo_error = None;
                            self.repo_cache.remove(&idx);
                            self.repo_cache_gen += 1;
                            if self.selected_repo_index == Some(idx) {
                                self.load_repo_async(idx, ui.ctx().clone());
                            }
                        }
                    }
                });
            } else if self.settings_tab == 1 {
                // Member management
                ui.label(
                    egui::RichText::new(crate::i18n::t(lang, "add_member"))
                        .strong()
                        .size(13.0)
                        .color(t.text),
                );
                ui.add_space(8.0);

                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut r1 = None;
                    let mut r2 = None;
                    egui::Grid::new("member_form_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "canonical_name"))
                                    .color(t.text_dim),
                            );
                            r1 = Some(
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_member_name)
                                        .desired_width(f32::INFINITY)
                                        .hint_text(crate::i18n::t(lang, "member_name_hint")),
                                ),
                            );
                            ui.end_row();

                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "alias_label"))
                                    .color(t.text_dim),
                            );
                            r2 = Some(
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_member_aliases)
                                        .desired_width(f32::INFINITY)
                                        .hint_text(crate::i18n::t(lang, "member_alias_hint")),
                                ),
                            );
                            ui.end_row();

                            ui.label("");
                            ui.checkbox(
                                &mut self.new_member_active,
                                crate::i18n::t(lang, "active_member_checkbox"),
                            );
                            ui.end_row();
                        });

                    ui.add_space(8.0);

                    if let Some(err) = &self.member_add_error {
                        ui.colored_label(t.error, err);
                        ui.add_space(4.0);
                    }
                    // Save success feedback (shown for 2 seconds)
                    if let Some(t_instant) = self.member_added_at {
                        if t_instant.elapsed().as_secs_f32() < 2.0 {
                            ui.colored_label(t.success, crate::i18n::t(lang, "added_success"));
                            ui.add_space(4.0);
                            ui.ctx().request_repaint();
                        } else {
                            self.member_added_at = None;
                        }
                    }

                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let mut should_add_member = false;
                    let r1_lost = r1.as_ref().map(|r| r.lost_focus()).unwrap_or(false);
                    let r2_lost = r2.as_ref().map(|r| r.lost_focus()).unwrap_or(false);
                    if (r1_lost || r2_lost) && enter_pressed {
                        should_add_member = true;
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(crate::i18n::t(lang, "add_member_btn"))
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(t.accent)
                            .stroke(egui::Stroke::NONE),
                        )
                        .clicked()
                    {
                        should_add_member = true;
                    }

                    if should_add_member {
                        if self.new_member_name.trim().is_empty() {
                            self.member_add_error =
                                Some(crate::i18n::t(lang, "err_enter_canonical_name").to_string());
                        } else {
                            let aliases = self
                                .new_member_aliases
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();

                            self.members.push(Member {
                                canonical_name: self.new_member_name.trim().to_string(),
                                aliases,
                                is_active: self.new_member_active,
                            });
                            let _ = config::save_members(&self.members);
                            self.new_member_name.clear();
                            self.new_member_aliases.clear();
                            self.new_member_active = true;
                            self.member_add_error = None;
                            self.member_added_at = Some(std::time::Instant::now());
                            self.session_refreshed.clear();
                            if let Some(idx) = self.selected_repo_index {
                                self.load_repo_async(idx, ui.ctx().clone());
                            }
                        }
                    }
                });

                ui.add_space(20.0);
                let member_count_title = match lang {
                    crate::config::Language::English => {
                        format!("Registered Members ({} total)", self.members.len())
                    }
                    crate::config::Language::Japanese => {
                        format!("登録済みメンバー  ({}人)", self.members.len())
                    }
                };
                ui.label(
                    egui::RichText::new(member_count_title)
                        .strong()
                        .size(13.0)
                        .color(t.text),
                );
                ui.add_space(8.0);

                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if self.members.is_empty() {
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "no_registered_members"))
                                .color(t.text_faint),
                        );
                    } else {
                        let mut to_remove_member = None;
                        let mut status_changed = false;
                        let mut save_edit_member = None;

                        let sorted_indices: Vec<usize> = {
                            let mut v: Vec<usize> = (0..self.members.len()).collect();
                            v.sort_by(|&a, &b| {
                                self.members[a]
                                    .canonical_name
                                    .to_lowercase()
                                    .cmp(&self.members[b].canonical_name.to_lowercase())
                            });
                            v
                        };
                        let members_len = sorted_indices.len();
                        for (pos, idx) in sorted_indices.iter().copied().enumerate() {
                            if self.editing_member_idx == Some(idx) {
                                let mut r_edit1 = None;
                                let mut r_edit2 = None;
                                ui.vertical(|ui| {
                                    egui::Grid::new(format!("edit_member_grid_{}", idx))
                                        .num_columns(2)
                                        .spacing([12.0, 8.0])
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "canonical_name",
                                                ))
                                                .color(t.text_dim),
                                            );
                                            r_edit1 = Some(
                                                ui.add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.editing_member_name,
                                                    )
                                                    .desired_width(f32::INFINITY),
                                                ),
                                            );
                                            ui.end_row();

                                            ui.label(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "alias_label",
                                                ))
                                                .color(t.text_dim),
                                            );
                                            r_edit2 = Some(
                                                ui.add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.editing_member_aliases,
                                                    )
                                                    .desired_width(f32::INFINITY),
                                                ),
                                            );
                                            ui.end_row();
                                        });

                                    if let Some(err) = &self.editing_member_error {
                                        ui.add_space(4.0);
                                        ui.colored_label(t.error, err);
                                    }

                                    ui.add_space(8.0);

                                    let enter_pressed =
                                        ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    let mut should_save_edit = false;
                                    let r_edit1_lost =
                                        r_edit1.as_ref().map(|r| r.lost_focus()).unwrap_or(false);
                                    let r_edit2_lost =
                                        r_edit2.as_ref().map(|r| r.lost_focus()).unwrap_or(false);
                                    if (r_edit1_lost || r_edit2_lost) && enter_pressed {
                                        should_save_edit = true;
                                    }

                                    ui.horizontal(|ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(crate::i18n::t(lang, "save_btn"))
                                                    .fill(t.success)
                                                    .stroke(egui::Stroke::NONE),
                                            )
                                            .clicked()
                                        {
                                            should_save_edit = true;
                                        }

                                        if should_save_edit {
                                            if self.editing_member_name.trim().is_empty() {
                                                self.editing_member_error = Some(
                                                    crate::i18n::t(
                                                        lang,
                                                        "err_enter_canonical_name",
                                                    )
                                                    .to_string(),
                                                );
                                            } else {
                                                let aliases = self
                                                    .editing_member_aliases
                                                    .split(',')
                                                    .map(|s| s.trim().to_string())
                                                    .filter(|s| !s.is_empty())
                                                    .collect();
                                                save_edit_member = Some((
                                                    idx,
                                                    self.editing_member_name.trim().to_string(),
                                                    aliases,
                                                ));
                                            }
                                        }
                                        if ui.button(crate::i18n::t(lang, "cancel_btn")).clicked() {
                                            self.editing_member_idx = None;
                                            self.editing_member_error = None;
                                        }
                                    });
                                });
                            } else if self.confirm_delete_member_idx == Some(idx) {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t(
                                            lang,
                                            "confirm_delete_member",
                                        ))
                                        .color(t.error)
                                        .strong(),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .button(crate::i18n::t(lang, "cancel_btn"))
                                                .clicked()
                                            {
                                                self.confirm_delete_member_idx = None;
                                            }
                                            ui.add_space(8.0);
                                            if ui
                                                .add(
                                                    egui::Button::new(crate::i18n::t(
                                                        lang,
                                                        "yes_delete",
                                                    ))
                                                    .fill(t.error)
                                                    .stroke(egui::Stroke::NONE),
                                                )
                                                .clicked()
                                            {
                                                to_remove_member = Some(idx);
                                            }
                                        },
                                    );
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(&self.members[idx].canonical_name)
                                                .strong()
                                                .color(t.text),
                                        );
                                        if self.members[idx].aliases.is_empty() {
                                            ui.label(
                                                egui::RichText::new(crate::i18n::t(
                                                    lang,
                                                    "no_aliases",
                                                ))
                                                .size(10.0)
                                                .color(t.text_faint),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new(
                                                    self.members[idx].aliases.join(", "),
                                                )
                                                .size(10.0)
                                                .color(t.text_faint),
                                            );
                                        }
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if super::widgets::icon_button(
                                                ui,
                                                "trash",
                                                crate::i18n::t(lang, "delete_action"),
                                                &t,
                                            )
                                            .clicked()
                                            {
                                                self.confirm_delete_member_idx = Some(idx);
                                            }
                                            ui.add_space(4.0);
                                            if super::widgets::icon_button(
                                                ui,
                                                "pencil",
                                                crate::i18n::t(lang, "edit_action"),
                                                &t,
                                            )
                                            .clicked()
                                            {
                                                self.editing_member_idx = Some(idx);
                                                self.editing_member_name =
                                                    self.members[idx].canonical_name.clone();
                                                self.editing_member_aliases =
                                                    self.members[idx].aliases.join(", ");
                                                self.editing_member_error = None;
                                            }
                                            ui.add_space(8.0);
                                            if ui
                                                .checkbox(
                                                    &mut self.members[idx].is_active,
                                                    crate::i18n::t(lang, "is_active_checkbox"),
                                                )
                                                .changed()
                                            {
                                                status_changed = true;
                                            }
                                        },
                                    );
                                });
                            }
                            if pos + 1 < members_len {
                                ui.add(egui::Separator::default().spacing(8.0));
                            }
                        }

                        if let Some(idx) = to_remove_member {
                            self.members.remove(idx);
                            let _ = config::save_members(&self.members);
                            self.session_refreshed.clear();
                            self.confirm_delete_member_idx = None;
                            if let Some(s_idx) = self.selected_repo_index {
                                self.load_repo_async(s_idx, ui.ctx().clone());
                            }
                        } else if status_changed {
                            let _ = config::save_members(&self.members);
                            self.session_refreshed.clear();
                            if let Some(s_idx) = self.selected_repo_index {
                                self.load_repo_async(s_idx, ui.ctx().clone());
                            }
                        }

                        if let Some((idx, name, aliases)) = save_edit_member {
                            self.members[idx].canonical_name = name;
                            self.members[idx].aliases = aliases;
                            let _ = config::save_members(&self.members);
                            self.editing_member_idx = None;
                            self.editing_member_error = None;
                            self.session_refreshed.clear();
                            if let Some(s_idx) = self.selected_repo_index {
                                self.load_repo_async(s_idx, ui.ctx().clone());
                            }
                        }
                    }
                });
            } else if self.settings_tab == 2 {
                // Language settings
                ui.label(
                    egui::RichText::new(crate::i18n::t(lang, "display_language"))
                        .strong()
                        .size(13.0)
                        .color(t.text),
                );
                ui.add_space(8.0);
                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "select_display_language"))
                                .color(t.text_dim),
                        );
                        ui.add_space(12.0);
                        let mut selected_lang = self.prefs.language;
                        ui.horizontal(|ui| {
                            for l in Language::all() {
                                if ui.radio(selected_lang == *l, l.label()).clicked() {
                                    selected_lang = *l;
                                }
                                ui.add_space(12.0);
                            }
                        });
                        if selected_lang != self.prefs.language {
                            self.prefs.language = selected_lang;
                            let _ = crate::config::save_preferences(&self.prefs);
                        }
                    });
                });
                ui.add_space(20.0);

                // Appearance settings
                ui.label(
                    egui::RichText::new(crate::i18n::t(lang, "theme_settings"))
                        .strong()
                        .size(13.0)
                        .color(t.text),
                );
                ui.add_space(8.0);

                let mut selected_theme_name: Option<String> = None;
                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "theme_card_desc"))
                                .color(t.text_dim),
                        );
                        ui.add_space(16.0);

                        // "System" follows the OS light/dark setting — no
                        // fixed swatches to preview since it resolves
                        // dynamically (see theme::resolve_system_theme_name).
                        ui.horizontal(|ui| {
                            let is_system = self.prefs.theme == crate::theme::SYSTEM_THEME_NAME;
                            let resp = ui.radio(is_system, crate::theme::SYSTEM_THEME_NAME);
                            if resp.clicked() {
                                selected_theme_name =
                                    Some(crate::theme::SYSTEM_THEME_NAME.to_string());
                            }
                            ui.colored_label(t.text_faint, crate::i18n::t(lang, "os_sync_label"));
                        });
                        ui.add(egui::Separator::default().spacing(12.0));

                        for (idx, (name, theme_obj)) in self.themes.iter().enumerate() {
                            let is_current = name == &self.prefs.theme;
                            ui.horizontal(|ui| {
                                let resp = ui.radio(is_current, name);
                                if resp.clicked() {
                                    selected_theme_name = Some(name.clone());
                                }

                                if theme_obj.is_dark {
                                    ui.colored_label(
                                        t.text_faint,
                                        crate::i18n::t(lang, "dark_label"),
                                    );
                                } else {
                                    ui.colored_label(
                                        t.text_faint,
                                        crate::i18n::t(lang, "light_label"),
                                    );
                                }

                                ui.add_space(8.0);

                                // Colour preview swatches
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    let colors = [
                                        (crate::i18n::t(lang, "swatch_bg"), theme_obj.bg),
                                        (
                                            crate::i18n::t(lang, "swatch_sidebar"),
                                            theme_obj.bg_sidebar,
                                        ),
                                        (
                                            crate::i18n::t(lang, "swatch_card"),
                                            theme_obj.bg_elevated,
                                        ),
                                        (crate::i18n::t(lang, "swatch_text"), theme_obj.text),
                                        (crate::i18n::t(lang, "swatch_accent"), theme_obj.accent),
                                        (crate::i18n::t(lang, "swatch_success"), theme_obj.success),
                                        (crate::i18n::t(lang, "swatch_error"), theme_obj.error),
                                    ];
                                    for (_label, color) in colors {
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(12.0, 12.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(rect, 2.0, color);
                                        ui.painter().rect_stroke(
                                            rect,
                                            2.0,
                                            egui::Stroke::new(1.0, theme_obj.border),
                                            egui::StrokeKind::Middle,
                                        );
                                    }
                                });
                            });
                            if idx + 1 < self.themes.len() {
                                ui.add(egui::Separator::default().spacing(12.0));
                            }
                        }
                    });
                });

                if let Some(name) = selected_theme_name {
                    self.prefs.theme = name;
                    self.ensure_theme_applied(ui.ctx());
                    let _ = crate::config::save_preferences(&self.prefs);
                }

                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new(crate::i18n::t(lang, "editor_settings"))
                        .strong()
                        .size(13.0)
                        .color(t.text),
                );
                ui.add_space(8.0);

                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "editor_settings_desc"))
                                .color(t.text_dim),
                        );
                        ui.add_space(12.0);

                        settings_row(ui, &t, crate::i18n::t(lang, "editor_command"), |ui| {
                            let mut cmd = self.prefs.editor_command.clone();
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut cmd)
                                    .hint_text(crate::i18n::t(lang, "editor_hint"))
                                    .desired_width(240.0),
                            );
                            if resp.changed() {
                                self.prefs.editor_command = cmd;
                                let _ = crate::config::save_preferences(&self.prefs);
                            }
                        });

                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "editor_note"))
                                .size(11.0)
                                .color(t.text_faint),
                        );
                    });
                });
            } else if self.settings_tab == 3 {
                // ── About ──
                card_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new("Git Dashboard")
                            .strong()
                            .size(18.0)
                            .color(t.text),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .size(13.0)
                            .color(t.accent),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "app_description"))
                            .size(12.0)
                            .color(t.text_dim),
                    );
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "tech_stack"))
                            .strong()
                            .size(12.0)
                            .color(t.text),
                    );
                    ui.add_space(6.0);
                    for item in &[
                        "Rust (edition 2024)",
                        "egui 0.26 / eframe",
                        "serde_json / chrono / regex",
                    ] {
                        ui.label(
                            egui::RichText::new(format!("• {item}"))
                                .size(12.0)
                                .color(t.text_dim),
                        );
                    }
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "keyboard_shortcuts"))
                            .strong()
                            .size(12.0)
                            .color(t.text),
                    );
                    ui.add_space(6.0);
                    let shortcuts = [
                        ("Esc", crate::i18n::t(lang, "shortcut_esc")),
                        ("Ctrl/Cmd + 1〜5", crate::i18n::t(lang, "shortcut_tabs")),
                        ("n / p", crate::i18n::t(lang, "shortcut_hunks")),
                        ("Ctrl/Cmd + F", crate::i18n::t(lang, "shortcut_search")),
                    ];
                    egui::Grid::new("shortcut_grid")
                        .num_columns(2)
                        .spacing([20.0, 6.0])
                        .show(ui, |ui| {
                            for (key, desc) in &shortcuts {
                                ui.label(
                                    egui::RichText::new(key.to_string())
                                        .monospace()
                                        .color(t.accent),
                                );
                                ui.label(
                                    egui::RichText::new(desc.to_string())
                                        .size(12.0)
                                        .color(t.text_dim),
                                );
                                ui.end_row();
                            }
                        });
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(crate::i18n::t(lang, "config_file_location"))
                            .strong()
                            .size(12.0)
                            .color(t.text),
                    );
                    ui.add_space(6.0);
                    let config_dir = crate::config::get_config_dir();
                    ui.label(
                        egui::RichText::new(config_dir.to_string_lossy().as_ref())
                            .monospace()
                            .size(11.0)
                            .color(t.text_dim),
                    );
                });
            }
        });
    }
}
