use crate::config::{self, Language, Member, Repository};
use crate::git::{
    self, BranchInfo, ChangedFile, CommitSummary, DiffRowKind, FileDiff, StashEntry, Summary,
    TagInfo,
};
use crate::i18n;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Home,
    Repo,
    Diff,
    Settings,
    Help,
    Log,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepoTab {
    Status,
    Commits,
    Branches,
    Tags,
    Stash,
}

impl RepoTab {
    pub fn all() -> [RepoTab; 5] {
        [
            RepoTab::Status,
            RepoTab::Commits,
            RepoTab::Branches,
            RepoTab::Tags,
            RepoTab::Stash,
        ]
    }

    pub fn next(self) -> Self {
        match self {
            Self::Status => Self::Commits,
            Self::Commits => Self::Branches,
            Self::Branches => Self::Tags,
            Self::Tags => Self::Stash,
            Self::Stash => Self::Status,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Status => Self::Stash,
            Self::Commits => Self::Status,
            Self::Branches => Self::Commits,
            Self::Tags => Self::Branches,
            Self::Stash => Self::Tags,
        }
    }

    fn from_digit(d: char) -> Option<Self> {
        match d {
            '1' => Some(Self::Status),
            '2' => Some(Self::Commits),
            '3' => Some(Self::Branches),
            '4' => Some(Self::Tags),
            '5' => Some(Self::Stash),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusPane {
    List,
    Content,
}

#[derive(Clone, Debug)]
pub struct HomeRow {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: usize,
    pub last_commit: String,
}

#[derive(Clone, Debug)]
pub struct RepoSnapshot {
    pub summary: Summary,
    pub commits: Vec<CommitSummary>,
    pub commits_err: Option<String>,
    pub branches: Vec<BranchInfo>,
    pub branches_err: Option<String>,
    pub tags: Vec<TagInfo>,
    pub tags_err: Option<String>,
    pub stashes: Vec<StashEntry>,
    pub stashes_err: Option<String>,
    pub working_files: Vec<ChangedFile>,
    pub working_err: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LogView {
    pub title: String,
    pub body: String,
    pub scroll: usize,
}

#[derive(Clone, Debug)]
pub struct DiffLine {
    pub kind: DiffRowKind,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct DiffView {
    pub title: String,
    pub target: String,
    pub base: Option<String>,
    pub three_dot: bool,
    pub files: Vec<ChangedFile>,
    pub file_idx: usize,
    pub lines: Vec<DiffLine>,
    pub scroll: usize,
    pub loading: bool,
    pub header: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
enum Confirm {
    DeleteRepo(usize),
    DropStash { repo_idx: usize, stash_ref: String },
}

#[derive(Clone, Copy, Debug)]
enum InputKind {
    Filter,
    AddRepo,
}

enum Job {
    LoadHome {
        generation: u64,
        index: usize,
        path: PathBuf,
        members: Vec<Member>,
    },
    LoadRepo {
        index: usize,
        path: PathBuf,
        members: Vec<Member>,
    },
    LoadDiff {
        seq: u64,
        path: PathBuf,
        base: Option<String>,
        target: String,
        file: String,
        three_dot: bool,
    },
    LoadCommitMeta {
        seq: u64,
        path: PathBuf,
        hash: String,
    },
    LoadFiles {
        seq: u64,
        path: PathBuf,
        base: Option<String>,
        target: String,
        three_dot: bool,
        preselect: Option<String>,
    },
    Pull {
        path: PathBuf,
        lang: Language,
    },
    Fetch {
        path: PathBuf,
        lang: Language,
    },
    StashApply {
        path: PathBuf,
        stash_ref: String,
    },
    StashDrop {
        path: PathBuf,
        stash_ref: String,
    },
    LoadBranchLog {
        path: PathBuf,
        branch: String,
    },
}

enum Msg {
    HomeLoaded {
        generation: u64,
        index: usize,
        row: Result<HomeRow, String>,
    },
    RepoLoaded {
        index: usize,
        data: Box<Result<RepoSnapshot, String>>,
    },
    DiffLoaded {
        seq: u64,
        result: Result<FileDiff, String>,
    },
    CommitMeta {
        seq: u64,
        header: Result<String, String>,
        files: Result<Vec<ChangedFile>, String>,
    },
    FilesLoaded {
        seq: u64,
        files: Result<Vec<ChangedFile>, String>,
        preselect: Option<String>,
    },
    OpDone {
        ok: bool,
        text: String,
    },
    LogLoaded {
        title: String,
        body: Result<String, String>,
    },
}

pub struct App {
    pub repos: Vec<Repository>,
    members: Vec<Member>,
    prefs: config::Preferences,
    pub screen: Screen,
    pub home_filter: String,
    pub home_selected: usize,
    pub home_rows: HashMap<usize, HomeRow>,
    pub repo_tab: RepoTab,
    pub repo_index: Option<usize>,
    pub repo_data: Option<RepoSnapshot>,
    pub repo_loading: bool,
    pub list_selected: usize,
    pub focus: FocusPane,
    pub diff: Option<DiffView>,
    pub settings_selected: usize,
    pub status: String,
    pub error: Option<String>,
    pub should_quit: bool,
    pub tag_base: Option<String>,
    pub tag_target: Option<String>,
    pub list_filter: String,
    pub log: Option<LogView>,
    input: Option<InputKind>,
    input_buf: String,
    confirm: Option<Confirm>,
    help_return: Option<Screen>,
    home_gen: u64,
    job_tx: Sender<Job>,
    msg_rx: Receiver<Msg>,
    diff_seq: u64,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
        spawn_worker(job_rx, msg_tx);

        let repos = config::load_repositories();
        let members = config::load_members();
        let prefs = config::load_preferences();
        let mut app = Self {
            repos,
            members,
            prefs,
            screen: Screen::Home,
            home_filter: String::new(),
            home_selected: 0,
            home_rows: HashMap::new(),
            repo_tab: RepoTab::Commits,
            repo_index: None,
            repo_data: None,
            repo_loading: false,
            list_selected: 0,
            focus: FocusPane::List,
            diff: None,
            settings_selected: 0,
            status: String::new(),
            error: None,
            should_quit: false,
            tag_base: None,
            tag_target: None,
            list_filter: String::new(),
            log: None,
            input: None,
            input_buf: String::new(),
            confirm: None,
            help_return: None,
            home_gen: 0,
            job_tx,
            msg_rx,
            diff_seq: 0,
        };
        app.refresh_home();
        app
    }

    pub fn lang(&self) -> Language {
        self.prefs.language
    }

    pub fn t(&self, key: &str) -> String {
        i18n::t(self.prefs.language, key)
    }

    pub fn tt(&self, en: &str, ja: &str) -> String {
        match self.prefs.language {
            Language::Japanese => ja.to_string(),
            Language::English => en.to_string(),
        }
    }

    pub fn is_filtering(&self) -> bool {
        matches!(self.input, Some(InputKind::Filter))
    }

    pub fn is_adding_repo(&self) -> bool {
        matches!(self.input, Some(InputKind::AddRepo))
    }

    pub fn input_buf(&self) -> &str {
        &self.input_buf
    }

    pub fn confirm_message(&self) -> Option<String> {
        match self.confirm {
            Some(Confirm::DeleteRepo(i)) => {
                let name = self.repos.get(i).map(|r| r.name.as_str()).unwrap_or("?");
                Some(format!("{} ({name}) [y/n]", self.t("confirm_delete_repo")))
            }
            Some(Confirm::DropStash { .. }) => Some(self.tt(
                "Drop this stash? [y/n]",
                "この stash を削除しますか？ [y/n]",
            )),
            None => None,
        }
    }

    pub fn filtered_home(&self) -> Vec<usize> {
        filter_repo_indices(&self.repos, &self.home_filter)
    }

    pub fn current_list_len(&self) -> usize {
        self.visible_indices().len()
    }

    pub fn list_error(&self) -> Option<&str> {
        let data = self.repo_data.as_ref()?;
        match self.repo_tab {
            RepoTab::Status => data.working_err.as_deref(),
            RepoTab::Commits => data.commits_err.as_deref(),
            RepoTab::Branches => data.branches_err.as_deref(),
            RepoTab::Tags => data.tags_err.as_deref(),
            RepoTab::Stash => data.stashes_err.as_deref(),
        }
    }

    pub fn footer_keys(&self) -> String {
        if self.is_filtering() {
            return self.tt(
                "type to filter  enter apply  esc clear",
                "入力で絞り込み  enter 確定  esc 解除",
            );
        }
        match self.screen {
            Screen::Home => self.tt(
                "j/k  enter open  / filter  a add  d delete  p pull  f fetch  s settings  ?  q quit",
                "j/k  enter 開く  / 絞込  a 追加  d 削除  p pull  f fetch  s 設定  ?  q 終了",
            ),
            Screen::Repo => match self.repo_tab {
                RepoTab::Status => self.tt(
                    "1-5 tabs  enter file-diff  2 commits  r reload  esc back  q quit",
                    "1-5 タブ  enter ファイルdiff  2 コミット  r 再読込  esc 戻る  q 終了",
                ),
                RepoTab::Commits => self.tt(
                    "1-5 tabs  enter diff  / filter  g/G top/end  r reload  esc back  q quit",
                    "1-5 タブ  enter diff  / 絞込  g/G 先頭/末尾  r 再読込  esc 戻る  q 終了",
                ),
                RepoTab::Branches => self.tt(
                    "1-5 tabs  enter log  / filter  r reload  esc back  q quit",
                    "1-5 タブ  enter ログ  / 絞込  r 再読込  esc 戻る  q 終了",
                ),
                RepoTab::Tags => self.tt(
                    "1-5 tabs  space mark  enter compare  r reload  esc back  q quit",
                    "1-5 タブ  space 選択  enter 比較  r 再読込  esc 戻る  q 終了",
                ),
                RepoTab::Stash => self.tt(
                    "1-5 tabs  enter diff  a apply  d drop  r reload  esc back  q quit",
                    "1-5 タブ  enter diff  a 適用  d 削除  r 再読込  esc 戻る  q 終了",
                ),
            },
            Screen::Diff => self.tt(
                "tab/h/l panes  j/k  n/p hunk  enter load file  esc back  q quit",
                "tab/h/l ペイン  j/k  n/p hunk  enter ファイル  esc 戻る  q 終了",
            ),
            Screen::Settings => self.tt(
                "j/k  a add  d delete  l language  esc back  q quit",
                "j/k  a 追加  d 削除  l 言語  esc 戻る  q 終了",
            ),
            Screen::Help => self.tt("esc back  q quit", "esc 戻る  q 終了"),
            Screen::Log => self.tt("j/k scroll  esc back  q quit", "j/k スクロール  esc 戻る  q 終了"),
        }
    }

    pub fn drain_messages(&mut self) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            self.apply_msg(msg);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.confirm.is_some() {
            self.handle_confirm(key);
            return;
        }
        if self.input.is_some() {
            self.handle_input(key);
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
            || key.code == KeyCode::Char('q')
        {
            self.should_quit = true;
            return;
        }
        if key.code == KeyCode::Char('?') && self.screen != Screen::Help {
            self.help_return = Some(self.screen);
            self.screen = Screen::Help;
            return;
        }
        if key.code == KeyCode::Esc && self.error.is_some() {
            self.error = None;
            return;
        }
        match self.screen {
            Screen::Home => self.handle_home(key),
            Screen::Repo => self.handle_repo(key),
            Screen::Diff => self.handle_diff(key),
            Screen::Settings => self.handle_settings(key),
            Screen::Log => self.handle_log(key),
            Screen::Help => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Backspace) {
                    self.screen = self.help_return.take().unwrap_or(Screen::Home);
                }
            }
        }
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        let Some(data) = self.repo_data.as_ref() else {
            return Vec::new();
        };
        let q = self.list_filter.trim().to_lowercase();
        let matches = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
        match self.repo_tab {
            RepoTab::Status => data
                .working_files
                .iter()
                .enumerate()
                .filter(|(_, f)| matches(&f.path))
                .map(|(i, _)| i)
                .collect(),
            RepoTab::Commits => data
                .commits
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    matches(&c.hash) || matches(&c.author) || matches(&c.message) || matches(&c.date)
                })
                .map(|(i, _)| i)
                .collect(),
            RepoTab::Branches => data
                .branches
                .iter()
                .enumerate()
                .filter(|(_, b)| matches(&b.name) || matches(&b.message) || matches(&b.author))
                .map(|(i, _)| i)
                .collect(),
            RepoTab::Tags => data
                .tags
                .iter()
                .enumerate()
                .filter(|(_, t)| matches(&t.name) || matches(&t.message) || matches(&t.hash))
                .map(|(i, _)| i)
                .collect(),
            RepoTab::Stash => data
                .stashes
                .iter()
                .enumerate()
                .filter(|(_, s)| matches(&s.ref_name) || matches(&s.message) || matches(&s.author))
                .map(|(i, _)| i)
                .collect(),
        }
    }

    pub fn selected_item_index(&self) -> Option<usize> {
        self.visible_indices().get(self.list_selected).copied()
    }

    fn handle_confirm(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let c = self.confirm.take();
                match c {
                    Some(Confirm::DeleteRepo(i)) => self.delete_repo(i),
                    Some(Confirm::DropStash {
                        repo_idx,
                        stash_ref,
                    }) => self.drop_stash(repo_idx, stash_ref),
                    None => {}
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.confirm = None;
            }
            _ => {}
        }
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                let kind = self.input.take();
                self.input_buf.clear();
                if matches!(kind, Some(InputKind::Filter)) {
                    if self.screen == Screen::Home {
                        self.home_filter.clear();
                        self.home_selected = 0;
                    } else {
                        self.list_filter.clear();
                        self.list_selected = 0;
                    }
                }
            }
            KeyCode::Enter => {
                let kind = self.input.take();
                let buf = std::mem::take(&mut self.input_buf);
                match kind {
                    Some(InputKind::Filter) => {
                        if self.screen == Screen::Home {
                            self.home_filter = buf;
                            self.home_selected = 0;
                        } else {
                            self.list_filter = buf;
                            self.list_selected = 0;
                        }
                    }
                    Some(InputKind::AddRepo) => self.add_repo_from_path(&buf),
                    None => {}
                }
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
                if matches!(self.input, Some(InputKind::Filter)) {
                    if self.screen == Screen::Home {
                        self.home_filter.clone_from(&self.input_buf);
                        self.home_selected = 0;
                    } else {
                        self.list_filter.clone_from(&self.input_buf);
                        self.list_selected = 0;
                    }
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input_buf.push(c);
                if matches!(self.input, Some(InputKind::Filter)) {
                    if self.screen == Screen::Home {
                        self.home_filter.clone_from(&self.input_buf);
                        self.home_selected = 0;
                    } else {
                        self.list_filter.clone_from(&self.input_buf);
                        self.list_selected = 0;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_home(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc if !self.home_filter.is_empty() => {
                self.home_filter.clear();
                self.home_selected = 0;
            }
            KeyCode::Char('s') => {
                self.settings_selected = 0;
                self.screen = Screen::Settings;
            }
            KeyCode::Char('/') => {
                self.input = Some(InputKind::Filter);
                self.input_buf.clone_from(&self.home_filter);
            }
            KeyCode::Char('a') => self.begin_add_repo(),
            KeyCode::Char('d') => {
                if let Some(&idx) = self.filtered_home().get(self.home_selected) {
                    self.confirm = Some(Confirm::DeleteRepo(idx));
                }
            }
            KeyCode::Char('p') => self.pull_selected_home(),
            KeyCode::Char('f') => self.fetch_selected_home(),
            KeyCode::Char('r') => self.refresh_home(),
            KeyCode::Char('g') => self.home_selected = 0,
            KeyCode::Char('G') => {
                let n = self.filtered_home().len();
                self.home_selected = n.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_home(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_home(-1),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.open_selected_repo(),
            _ => {}
        }
    }

    fn handle_repo(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Left => {
                self.screen = Screen::Home;
                self.repo_data = None;
                self.repo_index = None;
                self.list_filter.clear();
            }
            KeyCode::Tab | KeyCode::Char(']') => self.switch_tab(self.repo_tab.next()),
            KeyCode::BackTab | KeyCode::Char('[') => self.switch_tab(self.repo_tab.prev()),
            KeyCode::Char(d) if d.is_ascii_digit() => {
                if let Some(tab) = RepoTab::from_digit(d) {
                    self.switch_tab(tab);
                }
            }
            KeyCode::Char('/') => {
                self.input = Some(InputKind::Filter);
                self.input_buf.clone_from(&self.list_filter);
            }
            KeyCode::Char('r') => {
                if let Some(idx) = self.repo_index {
                    self.reload_repo(idx);
                }
            }
            KeyCode::Char('g') => self.list_selected = 0,
            KeyCode::Char('G') => {
                self.list_selected = self.current_list_len().saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_list(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_list(-1),
            KeyCode::Char('p') => self.pull_current_repo(),
            KeyCode::Char('f') => self.fetch_current_repo(),
            KeyCode::Char(' ') if self.repo_tab == RepoTab::Tags => self.toggle_tag_marker(),
            KeyCode::Char('a') if self.repo_tab == RepoTab::Stash => self.apply_selected_stash(),
            KeyCode::Char('d') if self.repo_tab == RepoTab::Stash => {
                if let (Some(idx), Some(item)) = (self.repo_index, self.selected_item_index())
                    && let Some(data) = self.repo_data.as_ref()
                    && let Some(st) = data.stashes.get(item)
                {
                    self.confirm = Some(Confirm::DropStash {
                        repo_idx: idx,
                        stash_ref: st.ref_name.clone(),
                    });
                }
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.activate_repo_item(),
            _ => {}
        }
    }

    fn handle_diff(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Backspace => {
                self.diff = None;
                self.screen = Screen::Repo;
                self.focus = FocusPane::List;
            }
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right if self.focus == FocusPane::List => {
                self.focus = FocusPane::Content;
            }
            KeyCode::Char('h') | KeyCode::Left if self.focus == FocusPane::Content => {
                self.focus = FocusPane::List;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusPane::List => FocusPane::Content,
                    FocusPane::Content => FocusPane::List,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => self.diff_move(1),
            KeyCode::Up | KeyCode::Char('k') => self.diff_move(-1),
            KeyCode::PageDown => self.diff_move(20),
            KeyCode::Char(' ') if self.focus == FocusPane::Content => self.diff_move(20),
            KeyCode::PageUp => self.diff_move(-20),
            KeyCode::Char('n') => self.next_hunk(1),
            KeyCode::Char('N') | KeyCode::Char('p') => self.next_hunk(-1),
            KeyCode::Enter if self.focus == FocusPane::List => self.load_selected_diff_file(),
            _ => {}
        }
    }

    fn handle_log(&mut self, key: KeyEvent) {
        let Some(log) = self.log.as_mut() else {
            self.screen = Screen::Repo;
            return;
        };
        let max = log.body.lines().count().saturating_sub(1);
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Left => {
                self.log = None;
                self.screen = Screen::Repo;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                log.scroll = (log.scroll + 1).min(max);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                log.scroll = log.scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                log.scroll = (log.scroll + 20).min(max);
            }
            KeyCode::PageUp => {
                log.scroll = log.scroll.saturating_sub(20);
            }
            KeyCode::Char('g') => log.scroll = 0,
            KeyCode::Char('G') => log.scroll = max,
            _ => {}
        }
    }

    fn handle_settings(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => {
                self.screen = Screen::Home;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let n = self.repos.len();
                if n > 0 {
                    self.settings_selected = (self.settings_selected + 1).min(n - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_selected = self.settings_selected.saturating_sub(1);
            }
            KeyCode::Char('a') => self.begin_add_repo(),
            KeyCode::Char('d') => {
                if self.settings_selected < self.repos.len() {
                    self.confirm = Some(Confirm::DeleteRepo(self.settings_selected));
                }
            }
            KeyCode::Char('l') => self.toggle_language(),
            _ => {}
        }
    }

    fn move_home(&mut self, delta: isize) {
        let n = self.filtered_home().len();
        self.home_selected = move_index(self.home_selected, n, delta);
    }

    fn move_list(&mut self, delta: isize) {
        let n = self.current_list_len();
        self.list_selected = move_index(self.list_selected, n, delta);
    }

    fn switch_tab(&mut self, tab: RepoTab) {
        self.repo_tab = tab;
        self.list_selected = 0;
        self.list_filter.clear();
    }

    fn diff_move(&mut self, delta: isize) {
        let mut load = false;
        {
            let Some(diff) = self.diff.as_mut() else {
                return;
            };
            if self.focus == FocusPane::List {
                let old = diff.file_idx;
                diff.file_idx = move_index(diff.file_idx, diff.files.len(), delta);
                load = diff.file_idx != old;
            } else {
                let max = diff.lines.len().saturating_sub(1);
                diff.scroll = (diff.scroll as isize + delta).clamp(0, max as isize) as usize;
            }
        }
        if load {
            self.load_selected_diff_file();
        }
    }

    fn next_hunk(&mut self, dir: isize) {
        let Some(diff) = self.diff.as_mut() else {
            return;
        };
        self.focus = FocusPane::Content;
        let len = diff.lines.len();
        if len == 0 {
            return;
        }
        let start = diff.scroll;
        let mut i = start as isize;
        loop {
            i += dir;
            if i < 0 || i >= len as isize {
                break;
            }
            let u = i as usize;
            if is_hunk_start(&diff.lines, u) {
                diff.scroll = u;
                break;
            }
        }
    }

    fn refresh_home(&mut self) {
        self.status = self.t("analyzing");
        self.home_gen = self.home_gen.saturating_add(1);
        let generation = self.home_gen;
        for (index, repo) in self.repos.iter().enumerate() {
            let _ = self.job_tx.send(Job::LoadHome {
                generation,
                index,
                path: repo.path.clone(),
                members: self.members.clone(),
            });
        }
        if self.repos.is_empty() {
            self.status = self.tt(
                "No repositories. Press a to add one.",
                "リポジトリがありません。a で追加。",
            );
        }
    }

    fn open_selected_repo(&mut self) {
        let Some(&idx) = self.filtered_home().get(self.home_selected) else {
            return;
        };
        self.open_repo(idx);
    }

    fn open_repo(&mut self, idx: usize) {
        let Some(repo) = self.repos.get(idx) else {
            return;
        };
        self.repo_index = Some(idx);
        self.repo_data = None;
        self.repo_loading = true;
        self.repo_tab = RepoTab::Commits;
        self.list_selected = 0;
        self.list_filter.clear();
        self.tag_base = None;
        self.tag_target = None;
        self.screen = Screen::Repo;
        self.status = self.t("analyzing_repo_data");
        let _ = self.job_tx.send(Job::LoadRepo {
            index: idx,
            path: repo.path.clone(),
            members: self.members.clone(),
        });
    }

    fn reload_repo(&mut self, idx: usize) {
        let Some(repo) = self.repos.get(idx) else {
            return;
        };
        self.repo_loading = true;
        self.status = self.t("analyzing_repo_data");
        let _ = self.job_tx.send(Job::LoadRepo {
            index: idx,
            path: repo.path.clone(),
            members: self.members.clone(),
        });
    }

    fn activate_repo_item(&mut self) {
        let Some(idx) = self.repo_index else {
            return;
        };
        let item = self.selected_item_index();
        let data = self.repo_data.as_ref();
        match self.repo_tab {
            RepoTab::Status => {
                let path = item.and_then(|i| data?.working_files.get(i).map(|f| f.path.clone()));
                if let Some(path) = path {
                    self.open_diff(
                        idx,
                        None,
                        "WORKING_TREE".to_string(),
                        false,
                        Some(path),
                        self.tt("Working tree", "作業ツリー"),
                    );
                }
            }
            RepoTab::Commits => {
                let commit = item.and_then(|i| {
                    data?.commits
                        .get(i)
                        .map(|c| (c.hash.clone(), c.message.clone()))
                });
                if let Some((hash, message)) = commit {
                    self.open_commit_diff(idx, hash, message);
                }
            }
            RepoTab::Tags => {
                if let (Some(base), Some(target)) =
                    (self.tag_base.clone(), self.tag_target.clone())
                {
                    self.open_diff(
                        idx,
                        Some(base.clone()),
                        target.clone(),
                        true,
                        None,
                        format!("{base}...{target}"),
                    );
                }
            }
            RepoTab::Branches => {
                let name = item.and_then(|i| data?.branches.get(i).map(|b| b.name.clone()));
                let path = self.repos.get(idx).map(|r| r.path.clone());
                if let (Some(name), Some(path)) = (name, path) {
                    self.status = self.tt("Loading log...", "ログを読み込み中...");
                    let _ = self.job_tx.send(Job::LoadBranchLog {
                        path,
                        branch: name,
                    });
                }
            }
            RepoTab::Stash => {
                let stash = item.and_then(|i| {
                    data?.stashes
                        .get(i)
                        .map(|st| (st.ref_name.clone(), st.message.clone()))
                });
                if let Some((stash_ref, message)) = stash {
                    self.open_commit_diff(idx, stash_ref, message);
                }
            }
        }
    }

    fn open_commit_diff(&mut self, repo_idx: usize, hash: String, message: String) {
        let Some(repo) = self.repos.get(repo_idx) else {
            return;
        };
        self.diff_seq += 1;
        let seq = self.diff_seq;
        self.diff = Some(DiffView {
            title: format!("{hash}  {message}"),
            target: hash.clone(),
            base: None,
            three_dot: false,
            files: Vec::new(),
            file_idx: 0,
            lines: Vec::new(),
            scroll: 0,
            loading: true,
            header: None,
            error: None,
        });
        self.screen = Screen::Diff;
        self.focus = FocusPane::List;
        let _ = self.job_tx.send(Job::LoadCommitMeta {
            seq,
            path: repo.path.clone(),
            hash,
        });
    }

    fn open_diff(
        &mut self,
        repo_idx: usize,
        base: Option<String>,
        target: String,
        three_dot: bool,
        preselect_file: Option<String>,
        title: String,
    ) {
        let Some(repo) = self.repos.get(repo_idx) else {
            return;
        };
        self.diff_seq += 1;
        let seq = self.diff_seq;
        self.diff = Some(DiffView {
            title,
            target: target.clone(),
            base: base.clone(),
            three_dot,
            files: Vec::new(),
            file_idx: 0,
            lines: Vec::new(),
            scroll: 0,
            loading: true,
            header: None,
            error: None,
        });
        self.screen = Screen::Diff;
        self.focus = FocusPane::List;
        let _ = self.job_tx.send(Job::LoadFiles {
            seq,
            path: repo.path.clone(),
            base,
            target,
            three_dot,
            preselect: preselect_file,
        });
    }

    fn load_selected_diff_file(&mut self) {
        let Some(repo_idx) = self.repo_index else {
            return;
        };
        let Some(repo) = self.repos.get(repo_idx) else {
            return;
        };
        let Some(diff) = self.diff.as_mut() else {
            return;
        };
        let Some(file) = diff.files.get(diff.file_idx) else {
            diff.loading = false;
            diff.lines.clear();
            return;
        };
        self.diff_seq += 1;
        let seq = self.diff_seq;
        diff.loading = true;
        diff.scroll = 0;
        let _ = self.job_tx.send(Job::LoadDiff {
            seq,
            path: repo.path.clone(),
            base: diff.base.clone(),
            target: diff.target.clone(),
            file: file.path.clone(),
            three_dot: diff.three_dot,
        });
    }

    fn toggle_tag_marker(&mut self) {
        let Some(data) = self.repo_data.as_ref() else {
            return;
        };
        let Some(i) = self.selected_item_index() else {
            return;
        };
        let Some(tag) = data.tags.get(i) else {
            return;
        };
        let name = tag.name.clone();
        if self.tag_base.as_deref() == Some(name.as_str()) {
            self.tag_base = None;
        } else if self.tag_target.as_deref() == Some(name.as_str()) {
            self.tag_target = None;
        } else if self.tag_base.is_none() {
            self.tag_base = Some(name);
        } else {
            self.tag_target = Some(name);
        }
    }

    fn begin_add_repo(&mut self) {
        self.input = Some(InputKind::AddRepo);
        self.input_buf.clear();
        self.status = self.tt(
            "Enter repository path, then Enter",
            "リポジトリのパスを入力して Enter",
        );
    }

    fn add_repo_from_path(&mut self, path_str: &str) {
        if path_str.trim().is_empty() {
            return;
        }
        let path = expand_user_path(path_str);
        if !git::is_git_repo(&path) {
            self.error = Some(format!(
                "{}: {}",
                self.tt("Not a git repository", "Git リポジトリではありません"),
                path.display()
            ));
            return;
        }
        if self.repos.iter().any(|r| r.path == path) {
            self.error = Some(self.t("already_registered"));
            return;
        }
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| path.display().to_string());
        self.repos.push(Repository {
            name,
            path: path.clone(),
        });
        if let Err(e) = config::save_repositories(&self.repos) {
            self.error = Some(e);
            self.repos.pop();
            return;
        }
        let index = self.repos.len() - 1;
        let _ = self.job_tx.send(Job::LoadHome {
            generation: self.home_gen,
            index,
            path,
            members: self.members.clone(),
        });
        self.status = self.t("added_success");
        if self.screen == Screen::Settings {
            self.settings_selected = index;
        }
    }

    fn delete_repo(&mut self, i: usize) {
        if i >= self.repos.len() {
            return;
        }
        let removed = self.repos.remove(i);
        if let Err(e) = config::save_repositories(&self.repos) {
            self.repos.insert(i, removed);
            self.error = Some(e);
            return;
        }
        self.home_rows.remove(&i);
        let shifted: HashMap<_, _> = self
            .home_rows
            .drain()
            .filter_map(|(k, v)| {
                if k == i {
                    None
                } else if k > i {
                    Some((k - 1, v))
                } else {
                    Some((k, v))
                }
            })
            .collect();
        self.home_rows = shifted;
        self.home_selected = self
            .home_selected
            .min(self.filtered_home().len().saturating_sub(1));
        if self.settings_selected >= self.repos.len() && !self.repos.is_empty() {
            self.settings_selected = self.repos.len() - 1;
        }
        self.status = self.tt("Repository removed.", "リポジトリを削除しました。");
        self.refresh_home();
    }

    fn pull_selected_home(&mut self) {
        if let Some(&idx) = self.filtered_home().get(self.home_selected) {
            self.pull_repo(idx);
        }
    }

    fn fetch_selected_home(&mut self) {
        if let Some(&idx) = self.filtered_home().get(self.home_selected) {
            self.fetch_repo(idx);
        }
    }

    fn pull_current_repo(&mut self) {
        if let Some(idx) = self.repo_index {
            self.pull_repo(idx);
        }
    }

    fn fetch_current_repo(&mut self) {
        if let Some(idx) = self.repo_index {
            self.fetch_repo(idx);
        }
    }

    fn pull_repo(&mut self, idx: usize) {
        let Some(repo) = self.repos.get(idx) else {
            return;
        };
        self.status = self.tt("Pulling...", "Pull しています...");
        let _ = self.job_tx.send(Job::Pull {
            path: repo.path.clone(),
            lang: self.prefs.language,
        });
    }

    fn fetch_repo(&mut self, idx: usize) {
        let Some(repo) = self.repos.get(idx) else {
            return;
        };
        self.status = self.tt("Fetching...", "Fetch しています...");
        let _ = self.job_tx.send(Job::Fetch {
            path: repo.path.clone(),
            lang: self.prefs.language,
        });
    }

    fn apply_selected_stash(&mut self) {
        let Some(idx) = self.repo_index else {
            return;
        };
        let Some(data) = self.repo_data.as_ref() else {
            return;
        };
        let stash_ref = self.selected_item_index().and_then(|i| {
            data.stashes.get(i).map(|st| st.ref_name.clone())
        });
        let Some(stash_ref) = stash_ref else {
            return;
        };
        let Some(repo) = self.repos.get(idx) else {
            return;
        };
        self.status = self.t("apply_stash");
        let _ = self.job_tx.send(Job::StashApply {
            path: repo.path.clone(),
            stash_ref,
        });
    }

    fn drop_stash(&mut self, repo_idx: usize, stash_ref: String) {
        let Some(repo) = self.repos.get(repo_idx) else {
            return;
        };
        let _ = self.job_tx.send(Job::StashDrop {
            path: repo.path.clone(),
            stash_ref,
        });
    }

    fn toggle_language(&mut self) {
        self.prefs.language = match self.prefs.language {
            Language::English => Language::Japanese,
            Language::Japanese => Language::English,
        };
        let _ = config::save_preferences(&self.prefs);
    }

    fn apply_msg(&mut self, msg: Msg) {
        match msg {
            Msg::HomeLoaded {
                generation,
                index,
                row,
            } => {
                if generation != self.home_gen {
                    return;
                }
                match row {
                    Ok(r) => {
                        self.home_rows.insert(index, r);
                        if self.screen == Screen::Home {
                            self.status.clear();
                        }
                    }
                    Err(e) => {
                        self.home_rows.remove(&index);
                        self.error = Some(e);
                    }
                }
            }
            Msg::RepoLoaded { index, data } => {
                if self.repo_index != Some(index) {
                    return;
                }
                self.repo_loading = false;
                match *data {
                    Ok(snap) => {
                        self.status.clear();
                        self.repo_data = Some(snap);
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Msg::DiffLoaded { seq, result } => {
                if seq != self.diff_seq {
                    return;
                }
                let Some(diff) = self.diff.as_mut() else {
                    return;
                };
                diff.loading = false;
                match result {
                    Ok(fd) => {
                        diff.error = None;
                        diff.lines = flatten_diff(&fd);
                    }
                    Err(e) => diff.error = Some(e),
                }
            }
            Msg::CommitMeta { seq, header, files } => {
                if seq != self.diff_seq {
                    return;
                }
                let load = {
                    let Some(diff) = self.diff.as_mut() else {
                        return;
                    };
                    diff.header = header.ok();
                    match files {
                        Ok(f) => {
                            diff.files = f;
                            diff.file_idx = 0;
                            true
                        }
                        Err(e) => {
                            diff.loading = false;
                            diff.error = Some(e);
                            false
                        }
                    }
                };
                if load {
                    self.load_selected_diff_file();
                }
            }
            Msg::FilesLoaded {
                seq,
                files,
                preselect,
            } => {
                if seq != self.diff_seq {
                    return;
                }
                let mut focus_content = false;
                let load = {
                    let Some(diff) = self.diff.as_mut() else {
                        return;
                    };
                    match files {
                        Ok(f) => {
                            diff.file_idx = preselect
                                .as_ref()
                                .and_then(|p| f.iter().position(|x| &x.path == p))
                                .unwrap_or(0);
                            diff.files = f;
                            focus_content = preselect.is_some();
                            true
                        }
                        Err(e) => {
                            diff.loading = false;
                            diff.error = Some(e);
                            false
                        }
                    }
                };
                if focus_content {
                    self.focus = FocusPane::Content;
                }
                if load {
                    self.load_selected_diff_file();
                }
            }
            Msg::OpDone { ok, text } => {
                if ok {
                    self.status = text;
                    if let Some(idx) = self.repo_index {
                        self.reload_repo(idx);
                    } else {
                        self.refresh_home();
                    }
                } else {
                    self.error = Some(text);
                }
            }
            Msg::LogLoaded { title, body } => match body {
                Ok(raw) => {
                    self.status.clear();
                    self.log = Some(LogView {
                        title,
                        body: strip_ansi(&raw),
                        scroll: 0,
                    });
                    self.screen = Screen::Log;
                }
                Err(e) => self.error = Some(e),
            },
        }
    }
}

fn filter_repo_indices(repos: &[Repository], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    repos
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            q.is_empty()
                || r.name.to_lowercase().contains(&q)
                || r.path.to_string_lossy().to_lowercase().contains(&q)
        })
        .map(|(i, _)| i)
        .collect()
}

fn move_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let next = current as isize + delta;
    next.clamp(0, (len - 1) as isize) as usize
}

fn is_hunk_start(lines: &[DiffLine], i: usize) -> bool {
    let is_change = |k: &DiffRowKind| {
        matches!(
            k,
            DiffRowKind::Added | DiffRowKind::Removed | DiffRowKind::Modified
        )
    };
    is_change(&lines[i].kind) && (i == 0 || !is_change(&lines[i - 1].kind))
}

fn flatten_diff(diff: &FileDiff) -> Vec<DiffLine> {
    if diff.is_binary {
        return vec![DiffLine {
            kind: DiffRowKind::Context,
            text: "Binary file".to_string(),
        }];
    }
    let mut out = Vec::new();
    for row in &diff.rows {
        match row.kind {
            DiffRowKind::Context => {
                let t = row
                    .right_text
                    .as_deref()
                    .or(row.left_text.as_deref())
                    .unwrap_or("");
                out.push(DiffLine {
                    kind: DiffRowKind::Context,
                    text: format!(" {t}"),
                });
            }
            DiffRowKind::Removed => {
                out.push(DiffLine {
                    kind: DiffRowKind::Removed,
                    text: format!("-{}", row.left_text.as_deref().unwrap_or("")),
                });
            }
            DiffRowKind::Added => {
                out.push(DiffLine {
                    kind: DiffRowKind::Added,
                    text: format!("+{}", row.right_text.as_deref().unwrap_or("")),
                });
            }
            DiffRowKind::Modified => {
                if let Some(t) = &row.left_text {
                    out.push(DiffLine {
                        kind: DiffRowKind::Removed,
                        text: format!("-{t}"),
                    });
                }
                if let Some(t) = &row.right_text {
                    out.push(DiffLine {
                        kind: DiffRowKind::Added,
                        text: format!("+{t}"),
                    });
                }
            }
        }
    }
    if diff.truncated {
        out.push(DiffLine {
            kind: DiffRowKind::Context,
            text: "… truncated".to_string(),
        });
    }
    out
}

fn spawn_worker(job_rx: Receiver<Job>, msg_tx: Sender<Msg>) {
    thread::spawn(move || {
        while let Ok(job) = job_rx.recv() {
            let msg = match job {
                Job::LoadHome {
                    generation,
                    index,
                    path,
                    members,
                } => {
                    let row = load_home_row(&path, &members);
                    Msg::HomeLoaded {
                        generation,
                        index,
                        row,
                    }
                }
                Job::LoadRepo {
                    index,
                    path,
                    members,
                } => {
                    let data = Box::new(load_repo(&path, &members));
                    Msg::RepoLoaded { index, data }
                }
                Job::LoadDiff {
                    seq,
                    path,
                    base,
                    target,
                    file,
                    three_dot,
                } => {
                    let result = git::get_file_diff(
                        &path,
                        base.as_deref(),
                        &target,
                        &file,
                        false,
                        false,
                        three_dot,
                    );
                    Msg::DiffLoaded { seq, result }
                }
                Job::LoadCommitMeta { seq, path, hash } => {
                    let header = git::get_commit_show(&path, &hash);
                    let files = git::get_changed_files(&path, None, &hash, false);
                    Msg::CommitMeta { seq, header, files }
                }
                Job::LoadFiles {
                    seq,
                    path,
                    base,
                    target,
                    three_dot,
                    preselect,
                } => {
                    let files =
                        git::get_changed_files(&path, base.as_deref(), &target, three_dot);
                    Msg::FilesLoaded {
                        seq,
                        files,
                        preselect,
                    }
                }
                Job::Pull { path, lang } => match git::pull_repository(&path, lang) {
                    Ok(t) => Msg::OpDone { ok: true, text: t },
                    Err(t) => Msg::OpDone { ok: false, text: t },
                },
                Job::Fetch { path, lang } => match git::fetch_repository(&path, lang) {
                    Ok(t) => Msg::OpDone { ok: true, text: t },
                    Err(t) => Msg::OpDone { ok: false, text: t },
                },
                Job::StashApply { path, stash_ref } => match git::apply_stash(&path, &stash_ref) {
                    Ok(t) => Msg::OpDone { ok: true, text: t },
                    Err(t) => Msg::OpDone { ok: false, text: t },
                },
                Job::StashDrop { path, stash_ref } => match git::drop_stash(&path, &stash_ref) {
                    Ok(t) => Msg::OpDone { ok: true, text: t },
                    Err(t) => Msg::OpDone { ok: false, text: t },
                },
                Job::LoadBranchLog { path, branch } => {
                    let body = git::get_branch_oneline_log(&path, &branch, 200);
                    Msg::LogLoaded {
                        title: branch,
                        body,
                    }
                }
            };
            if msg_tx.send(msg).is_err() {
                break;
            }
        }
    });
}

fn load_home_row(path: &std::path::Path, members: &[Member]) -> Result<HomeRow, String> {
    let summary = git::get_summary(path, members)?;
    let last_commit = git::get_recent_commits(path)
        .ok()
        .and_then(|c| c.into_iter().next())
        .map(|c| c.date)
        .unwrap_or_default();
    Ok(HomeRow {
        branch: summary.current_branch,
        ahead: summary.ahead,
        behind: summary.behind,
        dirty: summary.uncommitted_changes,
        last_commit,
    })
}

fn load_repo(path: &std::path::Path, members: &[Member]) -> Result<RepoSnapshot, String> {
    let summary = git::get_summary(path, members)?;
    let (commits, commits_err) = split_list(git::get_commits_for_diff(path));
    let (branches, branches_err) = split_list(git::get_branches(path));
    let (tags, tags_err) = split_list(git::get_tags(path));
    let (stashes, stashes_err) = split_list(git::get_stash_list(path));
    let (working_files, working_err) =
        split_list(git::get_changed_files(path, None, "WORKING_TREE", false));
    Ok(RepoSnapshot {
        summary,
        commits,
        commits_err,
        branches,
        branches_err,
        tags,
        tags_err,
        stashes,
        stashes_err,
        working_files,
        working_err,
    })
}

fn split_list<T>(result: Result<Vec<T>, String>) -> (Vec<T>, Option<String>) {
    match result {
        Ok(v) => (v, None),
        Err(e) => (Vec::new(), Some(e)),
    }
}

fn expand_user_path(raw: &str) -> PathBuf {
    let raw = raw.trim();
    if raw == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home_dir()
            .map(|h| h.join(rest))
            .unwrap_or_else(|| PathBuf::from(raw));
    }
    PathBuf::from(raw)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for x in chars.by_ref() {
                if x.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str, path: &str) -> Repository {
        Repository {
            name: name.to_string(),
            path: PathBuf::from(path),
        }
    }

    #[test]
    fn filter_matches_name_or_path() {
        let repos = vec![
            repo("git-dashboard", "/work/git-dashboard"),
            repo("aero-grep", "/work/search/aero-grep"),
        ];
        assert_eq!(filter_repo_indices(&repos, ""), vec![0, 1]);
        assert_eq!(filter_repo_indices(&repos, "DASH"), vec![0]);
        assert_eq!(filter_repo_indices(&repos, "search"), vec![1]);
        assert!(filter_repo_indices(&repos, "zzz").is_empty());
    }

    #[test]
    fn move_index_clamps() {
        assert_eq!(move_index(0, 0, 1), 0);
        assert_eq!(move_index(0, 3, 1), 1);
        assert_eq!(move_index(2, 3, 1), 2);
        assert_eq!(move_index(0, 3, -1), 0);
        assert_eq!(move_index(1, 3, -1), 0);
    }

    #[test]
    fn tab_cycle_is_circular() {
        let mut t = RepoTab::Status;
        for _ in 0..5 {
            t = t.next();
        }
        assert_eq!(t, RepoTab::Status);
        t = t.prev();
        assert_eq!(t, RepoTab::Stash);
    }

    #[test]
    fn home_keys_move_and_open_settings() {
        let mut app = App::new();
        app.repos = vec![
            repo("alpha", "/tmp/alpha"),
            repo("beta", "/tmp/beta"),
            repo("gamma", "/tmp/gamma"),
        ];
        app.home_selected = 0;
        app.handle_key(KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.home_selected, 1);
        app.handle_key(KeyEvent::from(KeyCode::Char('k')));
        assert_eq!(app.home_selected, 0);
        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(app.screen, Screen::Settings);
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn slash_enters_filter_and_narrows_list() {
        let mut app = App::new();
        app.repos = vec![repo("alpha", "/tmp/alpha"), repo("beta", "/tmp/beta")];
        app.handle_key(KeyEvent::from(KeyCode::Char('/')));
        assert!(app.is_filtering());
        app.handle_key(KeyEvent::from(KeyCode::Char('b')));
        assert_eq!(app.filtered_home(), vec![1]);
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.is_filtering());
        assert_eq!(app.filtered_home(), vec![0, 1]);
    }

    #[test]
    fn hunk_start_detects_change_blocks() {
        let lines = vec![
            DiffLine {
                kind: DiffRowKind::Context,
                text: " a".into(),
            },
            DiffLine {
                kind: DiffRowKind::Removed,
                text: "-b".into(),
            },
            DiffLine {
                kind: DiffRowKind::Added,
                text: "+c".into(),
            },
            DiffLine {
                kind: DiffRowKind::Context,
                text: " d".into(),
            },
            DiffLine {
                kind: DiffRowKind::Added,
                text: "+e".into(),
            },
        ];
        assert!(!is_hunk_start(&lines, 0));
        assert!(is_hunk_start(&lines, 1));
        assert!(!is_hunk_start(&lines, 2));
        assert!(is_hunk_start(&lines, 4));
    }

    #[test]
    fn open_repo_lands_on_commits() {
        let mut app = App::new();
        app.repos = vec![repo("alpha", "/tmp/alpha")];
        app.open_repo(0);
        assert_eq!(app.screen, Screen::Repo);
        assert_eq!(app.repo_tab, RepoTab::Commits);
    }

    #[test]
    fn q_quits_from_repo_and_esc_goes_home() {
        let mut app = App::new();
        app.screen = Screen::Repo;
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Home);
        assert!(!app.should_quit);
        app.screen = Screen::Repo;
        app.handle_key(KeyEvent::from(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn help_returns_to_previous_screen() {
        let mut app = App::new();
        app.screen = Screen::Repo;
        app.handle_key(KeyEvent::from(KeyCode::Char('?')));
        assert_eq!(app.screen, Screen::Help);
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Repo);
    }

    #[test]
    fn expand_tilde_and_strip_ansi() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/x".into());
        assert_eq!(
            expand_user_path("~/work/repo"),
            PathBuf::from(home).join("work/repo")
        );
        assert_eq!(strip_ansi("\u{1b}[32mgreen\u{1b}[0m"), "green");
    }

    #[test]
    fn commit_filter_narrows_visible_indices() {
        let mut app = App::new();
        app.screen = Screen::Repo;
        app.repo_tab = RepoTab::Commits;
        app.repo_data = Some(RepoSnapshot {
            summary: crate::git::Summary {
                repo_name: "x".into(),
                repo_path: "/tmp/x".into(),
                current_branch: "main".into(),
                total_commits: 2,
                total_contributors: 1,
                total_branches: 1,
                total_files: 1,
                total_size_bytes: 0,
                total_size_formatted: "0".into(),
                has_upstream: false,
                ahead: 0,
                behind: 0,
                has_remote: false,
                uncommitted_changes: 0,
            },
            commits: vec![
                CommitSummary {
                    hash: "aaa".into(),
                    author: "Ann".into(),
                    date: "2026-01-01".into(),
                    message: "fix footer".into(),
                },
                CommitSummary {
                    hash: "bbb".into(),
                    author: "Bob".into(),
                    date: "2026-01-02".into(),
                    message: "add tui".into(),
                },
            ],
            commits_err: None,
            branches: vec![],
            branches_err: None,
            tags: vec![],
            tags_err: None,
            stashes: vec![],
            stashes_err: None,
            working_files: vec![],
            working_err: None,
        });
        app.handle_key(KeyEvent::from(KeyCode::Char('/')));
        app.handle_key(KeyEvent::from(KeyCode::Char('t')));
        app.handle_key(KeyEvent::from(KeyCode::Char('u')));
        app.handle_key(KeyEvent::from(KeyCode::Char('i')));
        assert_eq!(app.visible_indices(), vec![1]);
    }
}
