use super::app::{App, FocusPane, RepoTab, Screen};
use super::colors::Palette;
use crate::git::DiffRowKind;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs, Wrap,
};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    let pal = Palette::mocha();
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(pal.bg).fg(pal.text)),
        area,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    draw_title(frame, app, chunks[0], pal);
    match app.screen {
        Screen::Home => draw_home(frame, app, chunks[1], pal),
        Screen::Repo => draw_repo(frame, app, chunks[1], pal),
        Screen::Diff => draw_diff(frame, app, chunks[1], pal),
        Screen::Settings => draw_settings(frame, app, chunks[1], pal),
        Screen::Help => draw_help(frame, app, chunks[1], pal),
        Screen::Log => draw_log(frame, app, chunks[1], pal),
    }
    draw_footer(frame, app, chunks[2], pal);

    if app.is_adding_repo() {
        draw_prompt(
            frame,
            area,
            &app.tt(
                "Add repository  (~/path or /abs/path)",
                "リポジトリ追加  (~/path または 絶対パス)",
            ),
            app.input_buf(),
            pal,
        );
    }
    if let Some(msg) = app.confirm_message() {
        draw_prompt(frame, area, &msg, "", pal);
    }
}

fn draw_title(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let title = match app.screen {
        Screen::Home => app.t("app_title"),
        Screen::Repo => app
            .repo_index
            .and_then(|i| app.repos.get(i))
            .map(|r| r.name.clone())
            .unwrap_or_else(|| app.t("dashboard")),
        Screen::Diff => app
            .diff
            .as_ref()
            .map(|d| d.title.clone())
            .unwrap_or_else(|| app.t("diff")),
        Screen::Settings => app.t("settings"),
        Screen::Help => app.tt("Keyboard shortcuts", "キーボードショートカット"),
        Screen::Log => app
            .log
            .as_ref()
            .map(|l| l.title.clone())
            .unwrap_or_else(|| app.tt("Log", "ログ")),
    };
    let bar = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(pal.bg)
                .bg(pal.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  git-dashboard-tui", Style::default().fg(pal.muted)),
    ]));
    frame.render_widget(bar, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);
    frame.render_widget(
        Paragraph::new(app.footer_keys()).style(Style::default().fg(pal.subtext).bg(pal.surface)),
        rows[0],
    );
    let (msg, color) = if let Some(e) = app.error.as_deref() {
        (e, pal.red)
    } else if !app.status.is_empty() {
        (app.status.as_str(), pal.yellow)
    } else if app.is_filtering() {
        (app.input_buf(), pal.accent)
    } else {
        ("", pal.muted)
    };
    frame.render_widget(
        Paragraph::new(msg).style(Style::default().fg(color).bg(pal.surface)),
        rows[1],
    );
}

fn draw_home(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    if app.repos.is_empty() {
        let body = vec![
            Line::from(""),
            Line::from(Span::styled(
                app.tt("No repositories registered.", "リポジトリがありません。"),
                Style::default().fg(pal.text).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                app.tt(
                    "Press a  and enter a path  (e.g. ~/work/my-repo)",
                    "a を押してパスを入力  (例: ~/work/my-repo)",
                ),
                Style::default().fg(pal.subtext),
            )),
        ];
        frame.render_widget(
            Paragraph::new(body).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(pal.border))
                    .title(app.t("repositories")),
            ),
            area,
        );
        return;
    }

    let indices = app.filtered_home();
    let header = Row::new(vec![
        app.tt("Name", "名前"),
        app.tt("Branch", "ブランチ"),
        app.tt("Sync", "同期"),
        app.tt("Dirty", "未コミット"),
        app.tt("Updated", "更新"),
        app.tt("Path", "パス"),
    ])
    .style(
        Style::default()
            .fg(pal.subtext)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = indices
        .iter()
        .map(|&i| {
            let repo = &app.repos[i];
            let (branch, sync, dirty, updated) = match app.home_rows.get(&i) {
                Some(r) => (
                    r.branch.clone(),
                    format!("↑{} ↓{}", r.ahead, r.behind),
                    r.dirty.to_string(),
                    r.last_commit.clone(),
                ),
                None => (
                    "…".to_string(),
                    "…".to_string(),
                    "…".to_string(),
                    "…".to_string(),
                ),
            };
            Row::new(vec![
                repo.name.clone(),
                branch,
                sync,
                dirty,
                updated,
                repo.path.display().to_string(),
            ])
        })
        .collect();

    let filter = if app.is_filtering() || !app.home_filter.is_empty() {
        format!(" / {}", app.home_filter)
    } else {
        String::new()
    };
    let title = if indices.is_empty() {
        format!(
            "{} (0/{}){filter}  {}",
            app.t("repositories"),
            app.repos.len(),
            app.tt("no matches — esc to clear", "一致なし — esc で解除")
        )
    } else {
        format!(
            "{} ({}/{}){filter}",
            app.t("repositories"),
            indices.len(),
            app.repos.len()
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Length(16),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(18),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(pal.border))
            .title(title)
            .title_style(Style::default().fg(pal.accent)),
    )
    .row_highlight_style(
        Style::default()
            .bg(pal.overlay)
            .fg(pal.text)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▸ ")
    .column_spacing(1);

    let mut state = TableState::default();
    if !indices.is_empty() {
        state.select(Some(app.home_selected.min(indices.len() - 1)));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_repo(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let tabs = RepoTab::all();
    let labels: Vec<Line> = tabs
        .iter()
        .map(|t| {
            let s = match t {
                RepoTab::Status => app.tt("1 Status", "1 状態"),
                RepoTab::Commits => app.tt("2 Commits", "2 コミット"),
                RepoTab::Branches => format!("3 {}", app.t("tab_branches")),
                RepoTab::Tags => app.tt("4 Tags", "4 タグ"),
                RepoTab::Stash => format!("5 {}", app.tt("Stash", "Stash")),
            };
            Line::from(s)
        })
        .collect();
    let selected = tabs.iter().position(|t| *t == app.repo_tab).unwrap_or(0);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    let tabs_w = Tabs::new(labels)
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(pal.accent)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().fg(pal.subtext))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(pal.border)),
        );
    frame.render_widget(tabs_w, chunks[0]);

    if app.repo_loading && app.repo_data.is_none() {
        frame.render_widget(
            Paragraph::new(app.t("analyzing_repo_data"))
                .style(Style::default().fg(pal.yellow))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(pal.border)),
                ),
            chunks[1],
        );
        return;
    }

    match app.repo_tab {
        RepoTab::Status => draw_status(frame, app, chunks[1], pal),
        RepoTab::Commits => draw_commits(frame, app, chunks[1], pal),
        RepoTab::Branches => draw_branches(frame, app, chunks[1], pal),
        RepoTab::Tags => draw_tags(frame, app, chunks[1], pal),
        RepoTab::Stash => draw_stash(frame, app, chunks[1], pal),
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let s = &data.summary;
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Length(8), Constraint::Min(1)])
        .split(area);

    let kpis = vec![
        Line::from(vec![
            Span::styled("branch  ", Style::default().fg(pal.muted)),
            Span::styled(
                s.current_branch.clone(),
                Style::default()
                    .fg(pal.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("ahead/behind  ", Style::default().fg(pal.muted)),
            Span::styled(
                format!("↑{} ↓{}", s.ahead, s.behind),
                Style::default().fg(pal.yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("commits  ", Style::default().fg(pal.muted)),
            Span::raw(s.total_commits.to_string()),
            Span::styled("   contributors  ", Style::default().fg(pal.muted)),
            Span::raw(s.total_contributors.to_string()),
            Span::styled("   branches  ", Style::default().fg(pal.muted)),
            Span::raw(s.total_branches.to_string()),
        ]),
        Line::from(vec![
            Span::styled("dirty  ", Style::default().fg(pal.muted)),
            Span::styled(
                s.uncommitted_changes.to_string(),
                if s.uncommitted_changes > 0 {
                    Style::default().fg(pal.red)
                } else {
                    Style::default().fg(pal.green)
                },
            ),
        ]),
        Line::from(Span::styled(
            s.repo_path.clone(),
            Style::default().fg(pal.muted),
        )),
    ];
    frame.render_widget(
        Paragraph::new(kpis).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.tt("Overview  (2 = full commit log)", "概要  (2 = コミット履歴)"))
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        split[0],
    );

    let recent: Vec<Line> = data
        .commits
        .iter()
        .take(6)
        .map(|c| {
            Line::from(format!(
                "{}  {}  {}",
                c.hash,
                c.date,
                truncate(&c.message, 48)
            ))
        })
        .collect();
    let recent_body = if recent.is_empty() {
        vec![Line::styled(
            app.tt("No recent commits.", "最近のコミットはありません。"),
            Style::default().fg(pal.muted),
        )]
    } else {
        recent
    };
    frame.render_widget(
        Paragraph::new(recent_body).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.tt("Recent commits", "最近のコミット"))
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        split[1],
    );

    render_path_list(
        frame,
        app,
        split[2],
        pal,
        format!(
            "{} ({})",
            app.t("uncommitted_changes_working"),
            data.working_files.len()
        ),
        app.tt("No uncommitted files. Press 2 for commits.", "未コミットなし。2 でコミット履歴。"),
    );
}

fn draw_commits(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let vis = app.visible_indices();
    let items: Vec<ListItem> = vis
        .iter()
        .map(|&i| {
            let c = &data.commits[i];
            ListItem::new(format!(
                "{}  {}  {:<16}  {}",
                c.hash,
                c.date,
                truncate(&c.author, 16),
                c.message
            ))
        })
        .collect();
    let title = format!(
        "{} ({}/{})",
        app.tt("Commits", "コミット"),
        vis.len(),
        data.commits.len()
    );
    render_items(
        frame,
        area,
        pal,
        items,
        app.list_selected,
        title,
        app.list_error(),
        &app.tt("No commits to show.", "表示するコミットがありません。"),
    );
}

fn draw_branches(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let vis = app.visible_indices();
    let items: Vec<ListItem> = vis
        .iter()
        .map(|&i| {
            let b = &data.branches[i];
            let kind = if b.is_remote { "remote" } else { "local " };
            ListItem::new(format!(
                "{kind}  {:<32}  {}  {}",
                truncate(&b.name, 32),
                b.date,
                b.message
            ))
            .style(if b.is_remote {
                Style::default().fg(pal.subtext)
            } else {
                Style::default().fg(pal.accent)
            })
        })
        .collect();
    render_items(
        frame,
        area,
        pal,
        items,
        app.list_selected,
        format!("{} ({})", app.t("tab_branches"), vis.len()),
        app.list_error(),
        &app.tt("No branches.", "ブランチがありません。"),
    );
}

fn draw_tags(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let vis = app.visible_indices();
    let items: Vec<ListItem> = vis
        .iter()
        .map(|&i| {
            let t = &data.tags[i];
            let mark = if app.tag_base.as_deref() == Some(t.name.as_str()) {
                "B"
            } else if app.tag_target.as_deref() == Some(t.name.as_str()) {
                "T"
            } else {
                " "
            };
            ListItem::new(format!(
                "[{mark}] {:<24}  {}  {}  {}",
                t.name, t.hash, t.date, t.message
            ))
        })
        .collect();
    let title = match (&app.tag_base, &app.tag_target) {
        (Some(b), Some(t)) => format!("Tags  {b}...{t}  (enter to diff)"),
        (Some(b), None) => format!("Tags  base={b}  (space to pick target)"),
        _ => format!("Tags ({})  (space to mark base/target)", vis.len()),
    };
    render_items(
        frame,
        area,
        pal,
        items,
        app.list_selected,
        title,
        app.list_error(),
        &app.tt("No tags.", "タグがありません。"),
    );
}

fn draw_stash(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let vis = app.visible_indices();
    let items: Vec<ListItem> = vis
        .iter()
        .map(|&i| {
            let s = &data.stashes[i];
            ListItem::new(format!(
                "{}  {}  {}  {}",
                s.ref_name, s.date_relative, s.author, s.message
            ))
        })
        .collect();
    render_items(
        frame,
        area,
        pal,
        items,
        app.list_selected,
        format!("Stash ({})", vis.len()),
        app.list_error(),
        &app.tt("No stashes.", "stash はありません。"),
    );
}

fn draw_diff(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(diff) = app.diff.as_ref() else {
        return;
    };
    let has_hunks = !diff.hunks.is_empty();
    let split = if has_hunks {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(26),
                Constraint::Percentage(24),
                Constraint::Percentage(50),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
            .split(area)
    };

    let file_items: Vec<ListItem> = diff
        .files
        .iter()
        .map(|f| {
            ListItem::new(format!("[{}] {}", f.status, f.path)).style(status_style(&f.status, pal))
        })
        .collect();
    let file_title = format!(
        "{} {} ({})",
        if app.focus == FocusPane::List {
            "▸"
        } else {
            " "
        },
        app.t("changed_files_header"),
        diff.files.len()
    );
    let no_changes = app.t("no_changes");
    render_items(
        frame,
        split[0],
        pal,
        file_items,
        diff.file_idx,
        file_title,
        None,
        no_changes.trim(),
    );

    let content_area = if has_hunks {
        let hunk_items: Vec<ListItem> = diff
            .hunks
            .iter()
            .map(|h| ListItem::new(h.label.clone()))
            .collect();
        let hunk_title = format!(
            "{} hunks ({}/{})",
            if app.focus == FocusPane::Hunks {
                "▸"
            } else {
                " "
            },
            if diff.hunks.is_empty() {
                0
            } else {
                diff.hunk_idx + 1
            },
            diff.hunks.len()
        );
        render_items(
            frame,
            split[1],
            pal,
            hunk_items,
            diff.hunk_idx,
            hunk_title,
            None,
            &app.tt("No hunks.", "hunk なし"),
        );
        split[2]
    } else {
        split[1]
    };

    let content_border = if app.focus == FocusPane::Content {
        Style::default().fg(pal.accent)
    } else {
        Style::default().fg(pal.border)
    };
    let hunk_range = diff.hunks.get(diff.hunk_idx).map(|h| (h.start, h.end));
    let body: Vec<Line> = if let Some(err) = &diff.error {
        vec![Line::styled(err.clone(), Style::default().fg(pal.red))]
    } else if diff.loading {
        vec![Line::styled(
            app.t("fetching_diff"),
            Style::default().fg(pal.yellow),
        )]
    } else if diff.files.is_empty() {
        vec![Line::styled(
            app.t("no_changes"),
            Style::default().fg(pal.muted),
        )]
    } else {
        let start = if diff.lines.is_empty() {
            0
        } else {
            diff.scroll.min(diff.lines.len() - 1)
        };
        diff.lines
            .iter()
            .enumerate()
            .skip(start)
            .map(|(idx, l)| {
                let mut style = match l.kind {
                    DiffRowKind::Added => Style::default().fg(pal.green),
                    DiffRowKind::Removed => Style::default().fg(pal.red),
                    DiffRowKind::Modified => Style::default().fg(pal.yellow),
                    DiffRowKind::Context => Style::default().fg(pal.text),
                };
                if let Some((hs, he)) = hunk_range
                    && idx >= hs
                    && idx < he
                {
                    style = style.bg(pal.overlay);
                }
                Line::styled(l.text.clone(), style)
            })
            .collect()
    };
    let header = diff
        .header
        .as_deref()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("");
    let title = format!(
        "{} {}  {}",
        if app.focus == FocusPane::Content {
            "▸"
        } else {
            " "
        },
        app.t("diff"),
        truncate(header, 40)
    );
    frame.render_widget(
        Paragraph::new(body).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(content_border)
                .title_style(Style::default().fg(pal.accent)),
        ),
        content_area,
    );
}

fn draw_settings(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let lang = match app.lang() {
        crate::config::Language::English => "English",
        crate::config::Language::Japanese => "日本語",
    };
    let header = vec![
        Line::from(vec![
            Span::styled(app.t("display_language"), Style::default().fg(pal.muted)),
            Span::raw("  "),
            Span::styled(lang, Style::default().fg(pal.accent)),
            Span::styled("  (l)", Style::default().fg(pal.muted)),
        ]),
        Line::from(vec![
            Span::styled(
                app.t("config_file_location"),
                Style::default().fg(pal.muted),
            ),
            Span::raw("  "),
            Span::raw(crate::config::get_config_dir().display().to_string()),
        ]),
        Line::from(""),
    ];
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(1)])
        .split(area);
    frame.render_widget(
        Paragraph::new(header).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("appearance"))
                .border_style(Style::default().fg(pal.border)),
        ),
        split[0],
    );
    let items: Vec<ListItem> = app
        .repos
        .iter()
        .map(|r| ListItem::new(format!("{}  {}", r.name, r.path.display())))
        .collect();
    render_items(
        frame,
        split[1],
        pal,
        items,
        app.settings_selected,
        app.t("tab_repos"),
        None,
        &app.tt("No repositories. Press a to add.", "リポジトリなし。a で追加。"),
    );
}

fn draw_log(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(log) = app.log.as_ref() else {
        return;
    };
    let lines: Vec<Line> = log
        .body
        .lines()
        .skip(log.scroll)
        .map(|l| Line::from(l.to_string()))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{}  (esc back)", log.title))
                .border_style(Style::default().fg(pal.accent))
                .title_style(Style::default().fg(pal.accent)),
        ),
        area,
    );
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let lines = vec![
        Line::from(Span::styled(
            app.tt("Global", "全体"),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from("  q / Ctrl+C     quit"),
        Line::from("  Esc / h        back one screen"),
        Line::from("  ?              toggle this help (returns here)"),
        Line::from("  /              filter current list"),
        Line::from("  g / G          first / last"),
        Line::from(""),
        Line::from(Span::styled(
            app.t("repositories"),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from("  j k            move    enter open    a/d add/delete"),
        Line::from("  p / f          pull / fetch    r reload    s settings"),
        Line::from(""),
        Line::from(Span::styled(
            app.tt("Repository", "リポジトリ"),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from("  1 Status  2 Commits  3 Branches  4 Tags  5 Stash"),
        Line::from("  enter     commit/file/stash diff, or branch log"),
        Line::from("  space     mark tag base/target, then enter to compare"),
        Line::from("  r         reload without leaving the tab"),
        Line::from(""),
        Line::from(Span::styled(
            app.t("diff"),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab / h l      files ↔ hunks ↔ diff"),
        Line::from("  n / p          next/prev hunk (wraps; highlights current)"),
        Line::from("  [ / ]          previous/next changed file"),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(pal.border)),
        ),
        area,
    );
}

fn draw_prompt(frame: &mut Frame, area: Rect, title: &str, value: &str, pal: Palette) {
    let w = area.width.clamp(20, 80);
    let h = 5;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect::new(x, y, w, h);
    frame.render_widget(Clear, rect);
    let body = if value.is_empty() {
        String::new()
    } else {
        format!("{value}█")
    };
    frame.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(pal.accent))
                .style(Style::default().bg(pal.surface).fg(pal.text)),
        ),
        rect,
    );
}

fn render_path_list(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    pal: Palette,
    title: String,
    empty: String,
) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let vis = app.visible_indices();
    let items: Vec<ListItem> = vis
        .iter()
        .map(|&i| {
            let f = &data.working_files[i];
            ListItem::new(format!(
                "[{}] {:+}/-{}  {}",
                f.status, f.additions, f.deletions, f.path
            ))
            .style(status_style(&f.status, pal))
        })
        .collect();
    render_items(
        frame,
        area,
        pal,
        items,
        app.list_selected,
        title,
        app.list_error(),
        &empty,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_items(
    frame: &mut Frame,
    area: Rect,
    pal: Palette,
    items: Vec<ListItem>,
    selected: usize,
    title: String,
    error: Option<&str>,
    empty: &str,
) {
    if let Some(err) = error {
        frame.render_widget(
            Paragraph::new(err).style(Style::default().fg(pal.red)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(pal.red)),
            ),
            area,
        );
        return;
    }
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(empty)
                .style(Style::default().fg(pal.muted))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .border_style(Style::default().fg(pal.border))
                        .title_style(Style::default().fg(pal.accent)),
                ),
            area,
        );
        return;
    }
    let n = items.len();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        )
        .highlight_style(
            Style::default()
                .bg(pal.overlay)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    let mut state = ListState::default();
    state.select(Some(selected.min(n.saturating_sub(1))));
    frame.render_stateful_widget(list, area, &mut state);
}

fn status_style(status: &str, pal: Palette) -> Style {
    match status {
        "A" => Style::default().fg(pal.green),
        "D" => Style::default().fg(pal.red),
        "M" => Style::default().fg(pal.yellow),
        "R" => Style::default().fg(pal.accent),
        _ => Style::default().fg(pal.subtext),
    }
}

fn truncate(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return format!("{s:<max$}");
    }
    let take = max.saturating_sub(1);
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
}
