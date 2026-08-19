use super::app::{App, FocusPane, RepoTab, Screen};
use super::colors::Palette;
use crate::git::DiffRowKind;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Row, Table, Tabs, Wrap};
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
            Constraint::Length(1),
        ])
        .split(area);

    draw_title(frame, app, chunks[0], pal);
    match app.screen {
        Screen::Home => draw_home(frame, app, chunks[1], pal),
        Screen::Repo => draw_repo(frame, app, chunks[1], pal),
        Screen::Diff => draw_diff(frame, app, chunks[1], pal),
        Screen::Settings => draw_settings(frame, app, chunks[1], pal),
        Screen::Help => draw_help(frame, app, chunks[1], pal),
    }
    draw_footer(frame, app, chunks[2], pal);

    if app.is_adding_repo() {
        draw_prompt(
            frame,
            area,
            &app.tt("Add repository", "リポジトリを追加"),
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
    let keys = match app.screen {
        Screen::Home if app.is_filtering() => app.tt(
            "type to filter  enter apply  esc clear",
            "入力で絞り込み  enter 確定  esc 解除",
        ),
        Screen::Home => app.tt(
            "j/k move  enter open  / filter  a add  d delete  p pull  f fetch  s settings  ? help  q quit",
            "j/k 移動  enter 開く  / 絞込  a 追加  d 削除  p pull  f fetch  s 設定  ? ヘルプ  q 終了",
        ),
        Screen::Repo => app.tt(
            "tab/[ ] tabs  j/k  enter diff  p pull  f fetch  space tag-mark  a apply-stash  esc back",
            "tab/[ ] タブ  j/k  enter diff  p pull  f fetch  space タグ選択  a stash適用  esc 戻る",
        ),
        Screen::Diff => app.tt(
            "tab files/diff  j/k  n/p hunk  esc back",
            "tab ファイル/diff  j/k  n/p hunk  esc 戻る",
        ),
        Screen::Settings => app.tt(
            "j/k  a add  d delete  l language  esc back",
            "j/k  a 追加  d 削除  l 言語  esc 戻る",
        ),
        Screen::Help => "esc / q / ?".to_string(),
    };
    let status_msg = if app.status.is_empty() {
        None
    } else {
        Some((app.status.as_str(), pal.yellow))
    };
    let msg = app
        .error
        .as_deref()
        .map(|e| (e, pal.red))
        .or(status_msg)
        .unwrap_or((keys.as_str(), pal.subtext));
    frame.render_widget(
        Paragraph::new(msg.0).style(Style::default().fg(msg.1).bg(pal.surface)),
        area,
    );
}

fn draw_home(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let indices = app.filtered_home();
    let header = Row::new(vec![
        app.tt("Name", "名前"),
        app.t("tab_branches").replace("Branches", "Branch"),
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
                Some(r) => {
                    let sync = format!("↑{} ↓{}", r.ahead, r.behind);
                    (
                        r.branch.clone(),
                        sync,
                        r.dirty.to_string(),
                        r.last_commit.clone(),
                    )
                }
                None => (
                    "…".to_string(),
                    "…".to_string(),
                    "…".to_string(),
                    "…".to_string(),
                ),
            };
            let mut row = Row::new(vec![
                repo.name.clone(),
                branch,
                sync,
                dirty,
                updated,
                repo.path.display().to_string(),
            ]);
            if Some(&i) == indices.get(app.home_selected) {
                row = row.style(
                    Style::default()
                        .bg(pal.overlay)
                        .fg(pal.text)
                        .add_modifier(Modifier::BOLD),
                );
            }
            row
        })
        .collect();

    let filter = if app.is_filtering() || !app.home_filter.is_empty() {
        format!(" / {}", app.home_filter)
    } else {
        String::new()
    };
    let title = format!(
        "{} ({}/{}){filter}",
        app.t("repositories"),
        indices.len(),
        app.repos.len()
    );

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
    .row_highlight_style(Style::default())
    .column_spacing(1);

    frame.render_widget(table, area);
}

fn draw_repo(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let tabs = RepoTab::all();
    let labels: Vec<Line> = tabs
        .iter()
        .map(|t| {
            let s = match t {
                RepoTab::Status => app.tt("Status", "状態"),
                RepoTab::Commits => app.t("commits"),
                RepoTab::Branches => app.t("tab_branches"),
                RepoTab::Tags => app.tt("Tags", "タグ"),
                RepoTab::Stash => app.t("stashes"),
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
                .block(Block::default().borders(Borders::ALL).border_style(
                    Style::default().fg(pal.border),
                )),
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
        .constraints([Constraint::Length(8), Constraint::Min(1)])
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
            Span::styled(format!("↑{} ↓{}", s.ahead, s.behind), Style::default().fg(pal.yellow)),
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
                .title(app.tt("Overview", "概要"))
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        split[0],
    );

    let items: Vec<ListItem> = data
        .working_files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let style = if i == app.list_selected {
                Style::default().bg(pal.overlay).fg(pal.text)
            } else {
                status_style(&f.status, pal)
            };
            ListItem::new(format!("[{}] {:+}/-{}  {}", f.status, f.additions, f.deletions, f.path))
                .style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("uncommitted_changes_working"))
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        split[1],
    );
}

fn draw_commits(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let items: Vec<ListItem> = data
        .commits
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let style = if i == app.list_selected {
                Style::default().bg(pal.overlay).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(
                "{}  {}  {:<16}  {}",
                c.hash,
                c.date,
                truncate(&c.author, 16),
                c.message
            ))
            .style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("commits"))
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        area,
    );
}

fn draw_branches(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let items: Vec<ListItem> = data
        .branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let kind = if b.is_remote { "remote" } else { "local " };
            let style = if i == app.list_selected {
                Style::default().bg(pal.overlay)
            } else if !b.is_remote {
                Style::default().fg(pal.accent)
            } else {
                Style::default().fg(pal.subtext)
            };
            ListItem::new(format!(
                "{kind}  {:<32}  {}  {}",
                truncate(&b.name, 32),
                b.date,
                b.message
            ))
            .style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("tab_branches"))
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        area,
    );
}

fn draw_tags(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let items: Vec<ListItem> = data
        .tags
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mark = if app.tag_base.as_deref() == Some(t.name.as_str()) {
                "B"
            } else if app.tag_target.as_deref() == Some(t.name.as_str()) {
                "T"
            } else {
                " "
            };
            let style = if i == app.list_selected {
                Style::default().bg(pal.overlay)
            } else {
                Style::default()
            };
            ListItem::new(format!(
                "[{mark}] {:<24}  {}  {}  {}",
                t.name, t.hash, t.date, t.message
            ))
            .style(style)
        })
        .collect();
    let title = match (&app.tag_base, &app.tag_target) {
        (Some(b), Some(t)) => format!("Tags  {b}...{t}  (enter to diff)"),
        (Some(b), None) => format!("Tags  base={b}  (space to pick target)"),
        _ => "Tags  (space to mark base/target)".to_string(),
    };
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        area,
    );
}

fn draw_stash(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(data) = app.repo_data.as_ref() else {
        return;
    };
    let items: Vec<ListItem> = data
        .stashes
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == app.list_selected {
                Style::default().bg(pal.overlay)
            } else {
                Style::default()
            };
            ListItem::new(format!(
                "{}  {}  {}  {}",
                s.ref_name, s.date_relative, s.author, s.message
            ))
            .style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("stashes"))
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        area,
    );
}

fn draw_diff(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let Some(diff) = app.diff.as_ref() else {
        return;
    };
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);

    let file_block_style = if app.focus == FocusPane::List {
        Style::default().fg(pal.accent)
    } else {
        Style::default().fg(pal.border)
    };
    let items: Vec<ListItem> = diff
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let style = if i == diff.file_idx {
                Style::default().bg(pal.overlay).add_modifier(Modifier::BOLD)
            } else {
                status_style(&f.status, pal)
            };
            ListItem::new(format!("[{}] {}", f.status, f.path)).style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("changed_files_header"))
                .border_style(file_block_style)
                .title_style(Style::default().fg(pal.accent)),
        ),
        split[0],
    );

    let content_border = if app.focus == FocusPane::Content {
        Style::default().fg(pal.accent)
    } else {
        Style::default().fg(pal.border)
    };
    let body: Vec<Line> = if let Some(err) = &diff.error {
        vec![Line::styled(err.clone(), Style::default().fg(pal.red))]
    } else if diff.loading {
        vec![Line::styled(
            app.t("fetching_diff"),
            Style::default().fg(pal.yellow),
        )]
    } else {
        let start = diff.scroll.min(diff.lines.len().saturating_sub(1));
        diff.lines
            .iter()
            .skip(start)
            .map(|l| {
                let style = match l.kind {
                    DiffRowKind::Added => Style::default().fg(pal.green),
                    DiffRowKind::Removed => Style::default().fg(pal.red),
                    DiffRowKind::Modified => Style::default().fg(pal.yellow),
                    DiffRowKind::Context => Style::default().fg(pal.text),
                };
                Line::styled(l.text.clone(), style)
            })
            .collect()
    };
    let mut lines = Vec::new();
    if let Some(h) = &diff.header {
        for row in h.lines() {
            lines.push(Line::styled(row.to_string(), Style::default().fg(pal.subtext)));
        }
        lines.push(Line::from(""));
    }
    lines.extend(body);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("diff"))
                    .border_style(content_border)
                    .title_style(Style::default().fg(pal.accent)),
            ),
        split[1],
    );
}

fn draw_settings(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let lang = match app.lang() {
        crate::config::Language::English => "English",
        crate::config::Language::Japanese => "日本語",
    };
    let header = vec![
        Line::from(vec![
            Span::styled(
                app.t("display_language"),
                Style::default().fg(pal.muted),
            ),
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
        .enumerate()
        .map(|(i, r)| {
            let style = if i == app.settings_selected {
                Style::default().bg(pal.overlay)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}  {}", r.name, r.path.display())).style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("tab_repos"))
                .border_style(Style::default().fg(pal.border))
                .title_style(Style::default().fg(pal.accent)),
        ),
        split[1],
    );
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect, pal: Palette) {
    let lines = vec![
        Line::from(Span::styled(
            app.tt("Global", "全体"),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from("  q / Ctrl+C     quit"),
        Line::from("  ?              toggle this help"),
        Line::from("  Esc            back / cancel"),
        Line::from(""),
        Line::from(Span::styled(
            app.t("repositories"),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from("  j k / arrows   move"),
        Line::from("  enter          open repository"),
        Line::from("  /              filter"),
        Line::from("  a / d          add / delete repository"),
        Line::from("  p / f          pull / fetch"),
        Line::from("  s              settings"),
        Line::from(""),
        Line::from(Span::styled(
            app.t("dashboard"),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab [ ] 1-5    switch Status / Commits / Branches / Tags / Stash"),
        Line::from("  enter          open diff (commit or working tree)"),
        Line::from("  space          mark tag as base/target, enter to compare"),
        Line::from("  a / d          apply / drop stash"),
        Line::from(""),
        Line::from(Span::styled(
            app.t("diff"),
            Style::default().fg(pal.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab            files ↔ diff"),
        Line::from("  n / p          next / previous hunk"),
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
