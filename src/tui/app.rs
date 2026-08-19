use crate::config::{self, Language, Member, Repository};
use crate::git::{
    self, BranchInfo, ChangedFile, CommitSummary, Contributor, DiffRowKind, FileDiff, StashEntry,
    Summary, TagInfo,
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
    Contributors,
}

impl RepoTab {
    pub fn all() -> [RepoTab; 6] {
        [
            RepoTab::Status,
            RepoTab::Commits,
            RepoTab::Branches,
            RepoTab::Tags,
            RepoTab::Stash,
            RepoTab::Contributors,
        ]
    }

    pub fn next(self) -> Self {
        match self {
            Self::Status => Self::Commits,
            Self::Commits => Self::Branches,
            Self::Branches => Self::Tags,
            Self::Tags => Self::Stash,
            Self::Stash => Self::Contributors,
            Self::Contributors => Self::Status,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Status => Self::Contributors,
            Self::Commits => Self::Status,
            Self::Branches => Self::Commits,
            Self::Tags => Self::Branches,
            Self::Stash => Self::Tags,
            Self::Contributors => Self::Stash,
        }
    }

    fn from_digit(d: char) -> Option<Self> {
        match d {
            '1' => Some(Self::Status),
            '2' => Some(Self::Commits),
            '3' => Some(Self::Branches),
            '4' => Some(Self::Tags),
            '5' => Some(Self::Stash),
            '6' => Some(Self::Contributors),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusPane {
    List,
    Hunks,
    Content,
}

impl FocusPane {
    fn next(self, has_hunks: bool) -> Self {
        match (self, has_hunks) {
            (Self::List, true) => Self::Hunks,
            (Self::List, false) => Self::Content,
            (Self::Hunks, _) => Self::Content,
            (Self::Content, _) => Self::List,
        }
    }

    fn prev(self, has_hunks: bool) -> Self {
        match (self, has_hunks) {
            (Self::List, _) => Self::Content,
            (Self::Hunks, _) => Self::List,
            (Self::Content, true) => Self::Hunks,
            (Self::Content, false) => Self::List,
        }
    }
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
    pub contributors: Vec<Contributor>,
    pub contributors_err: Option<String>,
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
    pub old_no: Option<usize>,
    pub new_no: Option<usize>,
}

impl Default for DiffLine {
    fn default() -> Self {
        Self {
            kind: DiffRowKind::Context,
            text: String::new(),
            old_no: None,
            new_no: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub start: usize,
    pub end: usize,
    pub label: String,
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
    pub hunks: Vec<Hunk>,
    pub hunk_idx: usize,
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
    AddAlias,
    Rename,
    DiffCommand,
}

#[derive(Clone, Debug)]
pub struct ExternalDiff {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub pipe_git_diff: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct CommitPreview {
    pub hash: String,
    pub header: String,
    pub files: Vec<ChangedFile>,
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
    LoadCommitPreview {
        seq: u64,
        path: PathBuf,
        hash: String,
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
    CommitPreviewLoaded {
        seq: u64,
        hash: String,
        header: Result<String, String>,
        files: Result<Vec<ChangedFile>, String>,
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
    pub commit_base: Option<String>,
    pub commit_target: Option<String>,
    pub commit_preview: Option<CommitPreview>,
    pub list_filter: String,
    pub log: Option<LogView>,
    pub active_only: bool,
    input: Option<InputKind>,
    input_buf: String,
    confirm: Option<Confirm>,
    help_return: Option<Screen>,
    home_gen: u64,
    pending_add_path: Option<PathBuf>,
    rename_idx: Option<usize>,
    pending_external: Option<ExternalDiff>,
    preview_seq: u64,
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
            commit_base: None,
            commit_target: None,
            commit_preview: None,
            list_filter: String::new(),
            log: None,
            active_only: false,
            input: None,
            input_buf: String::new(),
            confirm: None,
            help_return: None,
            home_gen: 0,
            pending_add_path: None,
            rename_idx: None,
            pending_external: None,
            preview_seq: 0,
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
        matches!(
            self.input,
            Some(InputKind::AddRepo | InputKind::AddAlias | InputKind::Rename | InputKind::DiffCommand)
        )
    }

    pub fn prompt_title(&self) -> String {
        match self.input {
            Some(InputKind::AddRepo) => self.tt(
                "Add repository  (~/path or /abs/path)",
                "リポジトリ追加  (~/path または 絶対パス)",
            ),
            Some(InputKind::AddAlias) => self.tt(
                "Display name / alias (Enter to use folder name)",
                "表示名 / 別名 (Enter でフォルダ名)",
            ),
            Some(InputKind::Rename) => self.tt("Rename alias", "別名を変更"),
            Some(InputKind::DiffCommand) => self.tt(
                "Diff tool (empty = builtin, e.g. hunk). Ranges use hash..hash",
                "Diff ツール (空 = 内蔵, 例: hunk). 範囲は hash..hash",
            ),
            _ => String::new(),
        }
    }

    pub fn input_buf(&self) -> &str {
        &self.input_buf
    }

    pub fn take_external(&mut self) -> Option<ExternalDiff> {
        self.pending_external.take()
    }

    pub fn diff_tool_label(&self) -> String {
        let c = self.prefs.diff_command.trim();
        if c.is_empty() {
            self.tt("builtin", "内蔵")
        } else {
            c.to_string()
        }
    }

    pub fn sort_label(&self) -> String {
        match self.prefs.repo_sort {
            0 => self.tt("name ↑", "名前 ↑"),
            1 => self.tt("name ↓", "名前 ↓"),
            2 => self.tt("updated ↓", "更新 ↓"),
            3 => self.tt("updated ↑", "更新 ↑"),
            _ => String::new(),
        }
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
        let mut idx = filter_repo_indices(&self.repos, &self.home_filter);
        sort_repo_indices(&mut idx, &self.repos, &self.home_rows, self.prefs.repo_sort);
        idx
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
            RepoTab::Contributors => data.contributors_err.as_deref(),
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
                "j/k  enter open  / filter  o sort  e alias  a add  d delete  p/f  s settings  q quit",
                "j/k  enter 開く  / 絞込  o ソート  e 別名  a 追加  d 削除  p/f  s 設定  q 終了",
            ),
            Screen::Repo => match self.repo_tab {
                RepoTab::Status => self.tt(
                    "1-6 tabs  enter file-diff  2 commits  r reload  esc back  q quit",
                    "1-6 タブ  enter ファイルdiff  2 コミット  r 再読込  esc 戻る  q 終了",
                ),
                RepoTab::Commits => self.tt(
                    "1-6 tabs  space mark  enter diff/compare  i builtin  / filter  r reload  q quit",
                    "1-6 タブ  space 選択  enter diff/比較  i 内蔵  / 絞込  r 再読込  q 終了",
                ),
                RepoTab::Branches => self.tt(
                    "1-6 tabs  enter log  / filter  r reload  esc back  q quit",
                    "1-6 タブ  enter ログ  / 絞込  r 再読込  esc 戻る  q 終了",
                ),
                RepoTab::Tags => self.tt(
                    "1-6 tabs  space mark  enter compare  r reload  esc back  q quit",
                    "1-6 タブ  space 選択  enter 比較  r 再読込  esc 戻る  q 終了",
                ),
                RepoTab::Stash => self.tt(
                    "1-6 tabs  enter diff  a apply  d drop  r reload  esc back  q quit",
                    "1-6 タブ  enter diff  a 適用  d 削除  r 再読込  esc 戻る  q 終了",
                ),
                RepoTab::Contributors => self.tt(
                    "1-6 tabs  m active-only  / filter  r reload  esc back  q quit",
                    "1-6 タブ  m メンテ中のみ  / 絞込  r 再読込  esc 戻る  q 終了",
                ),
            },
            Screen::Diff => {
                let hunk = self
                    .diff
                    .as_ref()
                    .map(|d| {
                        if d.hunks.is_empty() {
                            "hunk 0/0".to_string()
                        } else {
                            format!("hunk {}/{}", d.hunk_idx + 1, d.hunks.len())
                        }
                    })
                    .unwrap_or_default();
                self.tt(
                    &format!("{hunk}  n/p hunk  tab files/hunks/diff  [ ] file  esc back  q quit"),
                    &format!("{hunk}  n/p hunk  tab ファイル/hunk/diff  [ ] ファイル  esc 戻る  q 終了"),
                )
            }
            Screen::Settings => self.tt(
                "j/k  a add  d delete  e alias  c diff-tool  l language  esc back  q quit",
                "j/k  a 追加  d 削除  e 別名  c diffツール  l 言語  esc 戻る  q 終了",
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
            RepoTab::Contributors => data
                .contributors
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    if self.active_only && !(c.is_member && c.is_active) {
                        return false;
                    }
                    matches(&c.name) || matches(&c.email) || matches(&c.last_commit)
                })
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
                    Some(InputKind::AddAlias) => self.finish_add_repo(buf),
                    Some(InputKind::Rename) => self.rename_selected(buf),
                    Some(InputKind::DiffCommand) => {
                        self.prefs.diff_command = buf.trim().to_string();
                        let _ = config::save_preferences(&self.prefs);
                        self.status = if self.prefs.diff_command.is_empty() {
                            self.tt("Using builtin diff.", "内蔵 diff を使います。")
                        } else {
                            format!(
                                "{} {}",
                                self.tt("Diff tool:", "Diff ツール:"),
                                self.prefs.diff_command
                            )
                        };
                    }
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
            KeyCode::Char('o') => self.cycle_sort(),
            KeyCode::Char('e') => self.begin_rename_home(),
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
            KeyCode::Char('g') => {
                self.list_selected = 0;
                self.after_list_move();
            }
            KeyCode::Char('G') => {
                self.list_selected = self.current_list_len().saturating_sub(1);
                self.after_list_move();
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_list(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_list(-1),
            KeyCode::Char('p') => self.pull_current_repo(),
            KeyCode::Char('f') => self.fetch_current_repo(),
            KeyCode::Char('m') if self.repo_tab == RepoTab::Contributors => {
                self.active_only = !self.active_only;
                self.list_selected = 0;
            }
            KeyCode::Char('i') if self.repo_tab == RepoTab::Commits => {
                self.open_selected_commit(true);
            }
            KeyCode::Char(' ') if self.repo_tab == RepoTab::Tags => self.toggle_tag_marker(),
            KeyCode::Char(' ') if self.repo_tab == RepoTab::Commits => self.toggle_commit_marker(),
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
        let has_hunks = self
            .diff
            .as_ref()
            .is_some_and(|d| !d.hunks.is_empty());
        match key.code {
            KeyCode::Esc | KeyCode::Backspace => {
                self.diff = None;
                self.screen = Screen::Repo;
                self.focus = FocusPane::List;
            }
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => {
                self.focus = self.focus.next(has_hunks);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.focus = self.focus.prev(has_hunks);
            }
            KeyCode::Char('[') => self.diff_change_file(-1),
            KeyCode::Char(']') => self.diff_change_file(1),
            KeyCode::Down | KeyCode::Char('j') => self.diff_move(1),
            KeyCode::Up | KeyCode::Char('k') => self.diff_move(-1),
            KeyCode::PageDown => self.diff_move(20),
            KeyCode::Char(' ') if self.focus == FocusPane::Content => self.diff_move(20),
            KeyCode::PageUp => self.diff_move(-20),
            KeyCode::Char('g') => self.diff_home_end(true),
            KeyCode::Char('G') => self.diff_home_end(false),
            KeyCode::Char('n') => self.next_hunk(1),
            KeyCode::Char('N') | KeyCode::Char('p') => self.next_hunk(-1),
            KeyCode::Enter if self.focus == FocusPane::List => self.load_selected_diff_file(),
            KeyCode::Enter if self.focus == FocusPane::Hunks => self.jump_current_hunk(),
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
            KeyCode::Char('e') => {
                if self.settings_selected < self.repos.len() {
                    self.begin_rename(self.settings_selected);
                }
            }
            KeyCode::Char('c') => {
                self.input = Some(InputKind::DiffCommand);
                self.input_buf.clone_from(&self.prefs.diff_command);
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
        self.after_list_move();
    }

    fn after_list_move(&mut self) {
        if self.repo_tab == RepoTab::Commits {
            self.request_commit_preview();
        }
    }

    fn switch_tab(&mut self, tab: RepoTab) {
        self.repo_tab = tab;
        self.list_selected = 0;
        self.list_filter.clear();
        self.after_list_move();
    }

    fn diff_move(&mut self, delta: isize) {
        let mut load = false;
        {
            let Some(diff) = self.diff.as_mut() else {
                return;
            };
            match self.focus {
                FocusPane::List => {
                    let old = diff.file_idx;
                    diff.file_idx = move_index(diff.file_idx, diff.files.len(), delta);
                    load = diff.file_idx != old;
                }
                FocusPane::Hunks => {
                    if diff.hunks.is_empty() {
                        return;
                    }
                    diff.hunk_idx = move_index(diff.hunk_idx, diff.hunks.len(), delta);
                    diff.scroll = diff.hunks[diff.hunk_idx].start;
                }
                FocusPane::Content => {
                    let max = diff.lines.len().saturating_sub(1);
                    diff.scroll = (diff.scroll as isize + delta).clamp(0, max as isize) as usize;
                    sync_hunk_from_scroll(diff);
                }
            }
        }
        if load {
            self.load_selected_diff_file();
        }
    }

    fn diff_change_file(&mut self, delta: isize) {
        let changed = {
            let Some(diff) = self.diff.as_mut() else {
                return;
            };
            let old = diff.file_idx;
            diff.file_idx = move_index(diff.file_idx, diff.files.len(), delta);
            diff.file_idx != old
        };
        if changed {
            self.load_selected_diff_file();
        }
    }

    fn diff_home_end(&mut self, home: bool) {
        let mut load = false;
        {
            let Some(diff) = self.diff.as_mut() else {
                return;
            };
            match self.focus {
                FocusPane::List => {
                    diff.file_idx = if home {
                        0
                    } else {
                        diff.files.len().saturating_sub(1)
                    };
                    load = true;
                }
                FocusPane::Hunks | FocusPane::Content => {
                    if diff.hunks.is_empty() {
                        diff.scroll = if home {
                            0
                        } else {
                            diff.lines.len().saturating_sub(1)
                        };
                    } else {
                        diff.hunk_idx = if home { 0 } else { diff.hunks.len() - 1 };
                        diff.scroll = diff.hunks[diff.hunk_idx].start;
                    }
                }
            }
        }
        if load {
            self.load_selected_diff_file();
        }
    }

    fn jump_current_hunk(&mut self) {
        let Some(diff) = self.diff.as_mut() else {
            return;
        };
        if let Some(h) = diff.hunks.get(diff.hunk_idx) {
            diff.scroll = h.start;
        }
    }

    fn next_hunk(&mut self, dir: isize) {
        let Some(diff) = self.diff.as_mut() else {
            return;
        };
        let n = diff.hunks.len();
        if n == 0 {
            return;
        }
        let next = (diff.hunk_idx as isize + dir).rem_euclid(n as isize) as usize;
        diff.hunk_idx = next;
        diff.scroll = diff.hunks[next].start;
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
        self.commit_base = None;
        self.commit_target = None;
        self.commit_preview = None;
        self.screen = Screen::Repo;
        if let Some(cached) = load_tui_cache(&repo.path) {
            self.repo_data = Some(cached);
            self.repo_loading = true;
        }
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
                if let Some(path) = path
                    && !self.launch_external(None, "WORKING_TREE")
                {
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
                if let (Some(base), Some(target)) = (
                    self.commit_base.clone(),
                    self.commit_target.clone(),
                ) && !self.launch_external(Some(&base), &target)
                {
                    self.open_diff(
                        idx,
                        Some(base.clone()),
                        target.clone(),
                        true,
                        None,
                        format!("{base}...{target}"),
                    );
                } else if self.commit_base.is_none() || self.commit_target.is_none() {
                    self.open_selected_commit(false);
                }
            }
            RepoTab::Tags => {
                if let (Some(base), Some(target)) =
                    (self.tag_base.clone(), self.tag_target.clone())
                    && !self.launch_external(Some(&base), &target)
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
            RepoTab::Contributors => {}
        }
    }

    fn open_selected_commit(&mut self, force_builtin: bool) {
        let Some(idx) = self.repo_index else {
            return;
        };
        let item = self.selected_item_index();
        let commit = item.and_then(|i| {
            self.repo_data.as_ref()?.commits.get(i).map(|c| {
                (c.hash.clone(), c.message.clone())
            })
        });
        let Some((hash, message)) = commit else {
            return;
        };
        if !force_builtin && self.launch_external(None, &hash) {
            return;
        }
        self.open_commit_diff(idx, hash, message);
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
            hunks: Vec::new(),
            hunk_idx: 0,
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
            hunks: Vec::new(),
            hunk_idx: 0,
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
            diff.hunks.clear();
            diff.hunk_idx = 0;
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

    fn toggle_commit_marker(&mut self) {
        let Some(data) = self.repo_data.as_ref() else {
            return;
        };
        let Some(i) = self.selected_item_index() else {
            return;
        };
        let Some(c) = data.commits.get(i) else {
            return;
        };
        let name = c.hash.clone();
        if self.commit_base.as_deref() == Some(name.as_str()) {
            self.commit_base = None;
        } else if self.commit_target.as_deref() == Some(name.as_str()) {
            self.commit_target = None;
        } else if self.commit_base.is_none() {
            self.commit_base = Some(name);
        } else {
            self.commit_target = Some(name);
        }
    }

    fn cycle_sort(&mut self) {
        self.prefs.repo_sort = (self.prefs.repo_sort + 1) % 4;
        let _ = config::save_preferences(&self.prefs);
        self.home_selected = 0;
        self.status = format!(
            "{} {}",
            self.tt("Sort:", "ソート:"),
            self.sort_label()
        );
    }

    fn begin_rename_home(&mut self) {
        if let Some(&idx) = self.filtered_home().get(self.home_selected) {
            self.begin_rename(idx);
        }
    }

    fn begin_rename(&mut self, idx: usize) {
        let Some(repo) = self.repos.get(idx) else {
            return;
        };
        self.rename_idx = Some(idx);
        self.input = Some(InputKind::Rename);
        self.input_buf.clone_from(&repo.name);
    }

    fn rename_selected(&mut self, name: String) {
        let Some(idx) = self.rename_idx.take() else {
            return;
        };
        let name = name.trim();
        if name.is_empty() || idx >= self.repos.len() {
            return;
        }
        self.repos[idx].name = name.to_string();
        if let Err(e) = config::save_repositories(&self.repos) {
            self.error = Some(e);
        }
    }

    fn finish_add_repo(&mut self, alias: String) {
        let Some(path) = self.pending_add_path.take() else {
            return;
        };
        let default_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| path.display().to_string());
        let name = {
            let t = alias.trim();
            if t.is_empty() {
                default_name
            } else {
                t.to_string()
            }
        };
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

    fn request_commit_preview(&mut self) {
        let Some(idx) = self.repo_index else {
            return;
        };
        let hash = self.selected_item_index().and_then(|i| {
            self.repo_data
                .as_ref()?
                .commits
                .get(i)
                .map(|c| c.hash.clone())
        });
        let Some(hash) = hash else {
            self.commit_preview = None;
            return;
        };
        if self.commit_preview.as_ref().is_some_and(|p| p.hash == hash) {
            return;
        }
        let Some(repo) = self.repos.get(idx) else {
            return;
        };
        self.preview_seq += 1;
        let seq = self.preview_seq;
        let _ = self.job_tx.send(Job::LoadCommitPreview {
            seq,
            path: repo.path.clone(),
            hash,
        });
    }

    fn launch_external(&mut self, base: Option<&str>, target: &str) -> bool {
        let cmd = self.prefs.diff_command.trim();
        if cmd.is_empty() {
            return false;
        }
        let Some(idx) = self.repo_index else {
            return false;
        };
        let Some(repo) = self.repos.get(idx) else {
            return false;
        };
        match resolve_diff_command(cmd, &repo.path, base, target) {
            Ok(ext) => {
                self.pending_external = Some(ext);
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
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
        let default_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| path.display().to_string());
        self.pending_add_path = Some(path);
        self.input = Some(InputKind::AddAlias);
        self.input_buf = default_name;
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
                        if let Some(repo) = self.repos.get(index) {
                            save_tui_cache(&repo.path, &snap);
                        }
                        self.repo_data = Some(snap);
                        if self.repo_tab == RepoTab::Commits {
                            self.request_commit_preview();
                        }
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
                        apply_hunks(diff);
                    }
                    Err(e) => {
                        diff.error = Some(e);
                        diff.lines.clear();
                        diff.hunks.clear();
                        diff.hunk_idx = 0;
                    }
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
            Msg::CommitPreviewLoaded {
                seq,
                hash,
                header,
                files,
            } => {
                if seq != self.preview_seq {
                    return;
                }
                self.commit_preview = Some(CommitPreview {
                    hash,
                    header: header.unwrap_or_default(),
                    files: files.unwrap_or_default(),
                });
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

fn is_change_kind(k: &DiffRowKind) -> bool {
    matches!(
        k,
        DiffRowKind::Added | DiffRowKind::Removed | DiffRowKind::Modified
    )
}

fn is_hunk_start(lines: &[DiffLine], i: usize) -> bool {
    is_change_kind(&lines[i].kind) && (i == 0 || !is_change_kind(&lines[i - 1].kind))
}

fn collect_hunks(lines: &[DiffLine]) -> Vec<Hunk> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if !is_hunk_start(lines, i) {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < lines.len() && is_change_kind(&lines[i].kind) {
            i += 1;
        }
        let end = i;
        let preview = lines[start]
            .text
            .trim()
            .trim_start_matches(['+', '-', ' '])
            .chars()
            .take(36)
            .collect::<String>();
        let line_no = lines[start]
            .new_no
            .or(lines[start].old_no)
            .unwrap_or(0);
        out.push(Hunk {
            start,
            end,
            label: format!("#{:<3} L{:<5} {preview}", out.len() + 1, line_no),
        });
    }
    out
}

fn apply_hunks(diff: &mut DiffView) {
    diff.hunks = collect_hunks(&diff.lines);
    diff.hunk_idx = 0;
    if let Some(h) = diff.hunks.first() {
        diff.scroll = h.start;
    } else {
        diff.scroll = 0;
    }
}

fn sync_hunk_from_scroll(diff: &mut DiffView) {
    if let Some(i) = diff
        .hunks
        .iter()
        .rposition(|h| h.start <= diff.scroll)
    {
        diff.hunk_idx = i;
    }
}

fn flatten_diff(diff: &FileDiff) -> Vec<DiffLine> {
    if diff.is_binary {
        return vec![DiffLine {
            kind: DiffRowKind::Context,
            text: "Binary file".to_string(),
            old_no: None,
            new_no: None,
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
                    old_no: row.left_no,
                    new_no: row.right_no,
                });
            }
            DiffRowKind::Removed => {
                out.push(DiffLine {
                    kind: DiffRowKind::Removed,
                    text: format!("-{}", row.left_text.as_deref().unwrap_or("")),
                    old_no: row.left_no,
                    new_no: None,
                });
            }
            DiffRowKind::Added => {
                out.push(DiffLine {
                    kind: DiffRowKind::Added,
                    text: format!("+{}", row.right_text.as_deref().unwrap_or("")),
                    old_no: None,
                    new_no: row.right_no,
                });
            }
            DiffRowKind::Modified => {
                if let Some(t) = &row.left_text {
                    out.push(DiffLine {
                        kind: DiffRowKind::Removed,
                        text: format!("-{t}"),
                        old_no: row.left_no,
                        new_no: None,
                    });
                }
                if let Some(t) = &row.right_text {
                    out.push(DiffLine {
                        kind: DiffRowKind::Added,
                        text: format!("+{t}"),
                        old_no: None,
                        new_no: row.right_no,
                    });
                }
            }
        }
    }
    if diff.truncated {
        out.push(DiffLine {
            kind: DiffRowKind::Context,
            text: "… truncated".to_string(),
            old_no: None,
            new_no: None,
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
                Job::LoadCommitPreview { seq, path, hash } => {
                    let header = git::get_commit_show(&path, &hash);
                    let files = git::get_changed_files(&path, None, &hash, false);
                    Msg::CommitPreviewLoaded {
                        seq,
                        hash,
                        header,
                        files,
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
    let (contributors, contributors_err) = split_list(git::get_contributors(path, members));
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
        contributors,
        contributors_err,
    })
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TuiCache {
    version: u32,
    summary: Summary,
    commits: Vec<CommitSummary>,
    branches: Vec<BranchInfo>,
    tags: Vec<TagInfo>,
    stashes: Vec<StashEntry>,
    contributors: Vec<Contributor>,
}

fn load_tui_cache(path: &std::path::Path) -> Option<RepoSnapshot> {
    let json = std::fs::read_to_string(config::tui_cache_path(path)).ok()?;
    let cache: TuiCache = serde_json::from_str(&json).ok()?;
    if cache.version != 1 {
        return None;
    }
    Some(RepoSnapshot {
        summary: cache.summary,
        commits: cache.commits,
        commits_err: None,
        branches: cache.branches,
        branches_err: None,
        tags: cache.tags,
        tags_err: None,
        stashes: cache.stashes,
        stashes_err: None,
        working_files: Vec::new(),
        working_err: None,
        contributors: cache.contributors,
        contributors_err: None,
    })
}

fn save_tui_cache(path: &std::path::Path, snap: &RepoSnapshot) {
    let _ = std::fs::create_dir_all(config::cache_dir());
    let cache = TuiCache {
        version: 1,
        summary: snap.summary.clone(),
        commits: snap.commits.clone(),
        branches: snap.branches.clone(),
        tags: snap.tags.clone(),
        stashes: snap.stashes.clone(),
        contributors: snap.contributors.clone(),
    };
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = config::write_atomic(&config::tui_cache_path(path), &json);
    }
}

fn sort_repo_indices(
    idx: &mut [usize],
    repos: &[Repository],
    rows: &HashMap<usize, HomeRow>,
    sort_by: usize,
) {
    idx.sort_by(|&a, &b| {
        let date = |i: usize| rows.get(&i).map(|r| r.last_commit.as_str()).unwrap_or("");
        match sort_by {
            0 => repos[a].name.to_lowercase().cmp(&repos[b].name.to_lowercase()),
            1 => repos[b].name.to_lowercase().cmp(&repos[a].name.to_lowercase()),
            3 => {
                let da = date(a);
                let db = date(b);
                match (da.is_empty(), db.is_empty()) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => da.cmp(db),
                }
            }
            _ => {
                let da = date(a);
                let db = date(b);
                match (da.is_empty(), db.is_empty()) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => db.cmp(da),
                }
            }
        }
    });
}

fn resolve_diff_command(
    template: &str,
    repo: &std::path::Path,
    base: Option<&str>,
    target: &str,
) -> Result<ExternalDiff, String> {
    let t = template.trim();
    if t.is_empty() {
        return Err("diff command is empty".into());
    }
    let first = t.split_whitespace().next().unwrap_or(t);
    let is_hunk = first == "hunk" || first == "hunkdiff";
    if is_hunk && !t.contains('{') {
        let wt = target == "WORKING_TREE";
        if wt {
            return Ok(ExternalDiff {
                program: first.to_string(),
                args: vec!["diff".into()],
                cwd: repo.to_path_buf(),
                pipe_git_diff: None,
            });
        }
        if let Some(b) = base {
            return Ok(ExternalDiff {
                program: first.to_string(),
                args: vec!["diff".into(), format!("{b}..{target}")],
                cwd: repo.to_path_buf(),
                pipe_git_diff: None,
            });
        }
        return Ok(ExternalDiff {
            program: first.to_string(),
            args: vec!["show".into(), target.to_string()],
            cwd: repo.to_path_buf(),
            pipe_git_diff: None,
        });
    }
    let spec = if target == "WORKING_TREE" {
        String::new()
    } else {
        target.to_string()
    };
    let range = match base {
        Some(b) if !spec.is_empty() => format!("{b}..{spec}"),
        _ => spec.clone(),
    };
    let expanded = t
        .replace("{path}", &repo.display().to_string())
        .replace("{repo}", &repo.display().to_string())
        .replace("{range}", &range)
        .replace("{base} {target}", &range)
        .replace("{base}", base.unwrap_or(""))
        .replace("{target}", &spec)
        .replace("{file}", "");
    let mut parts = expanded.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| "diff command has no program".to_string())?
        .to_string();
    let mut args: Vec<String> = parts.map(str::to_string).collect();
    let is_hunk_bin = program == "hunk" || program == "hunkdiff" || program.ends_with("/hunk");
    if is_hunk_bin
        && args.first().is_some_and(|a| a == "show")
        && args.get(1).is_some_and(|a| a.contains(".."))
    {
        args[0] = "diff".into();
    }
    Ok(ExternalDiff {
        program,
        args,
        cwd: repo.to_path_buf(),
        pipe_git_diff: None,
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
        for _ in 0..6 {
            t = t.next();
        }
        assert_eq!(t, RepoTab::Status);
        t = t.prev();
        assert_eq!(t, RepoTab::Contributors);
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
                old_no: Some(1),
                new_no: Some(1),
            },
            DiffLine {
                kind: DiffRowKind::Removed,
                text: "-b".into(),
                old_no: Some(2),
                new_no: None,
            },
            DiffLine {
                kind: DiffRowKind::Added,
                text: "+c".into(),
                old_no: None,
                new_no: Some(2),
            },
            DiffLine {
                kind: DiffRowKind::Context,
                text: " d".into(),
                old_no: Some(3),
                new_no: Some(3),
            },
            DiffLine {
                kind: DiffRowKind::Added,
                text: "+e".into(),
                old_no: None,
                new_no: Some(4),
            },
        ];
        assert!(!is_hunk_start(&lines, 0));
        assert!(is_hunk_start(&lines, 1));
        assert!(!is_hunk_start(&lines, 2));
        assert!(is_hunk_start(&lines, 4));
        let hunks = collect_hunks(&lines);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].start, 1);
        assert_eq!(hunks[0].end, 3);
        assert_eq!(hunks[1].start, 4);
        assert_eq!(hunks[1].end, 5);
        assert!(hunks[0].label.contains("L2"));
    }

    #[test]
    fn next_hunk_wraps_and_syncs_scroll() {
        let mut app = App::new();
        app.screen = Screen::Diff;
        app.diff = Some(DiffView {
            title: "t".into(),
            target: "abc".into(),
            base: None,
            three_dot: false,
            files: vec![],
            file_idx: 0,
            lines: vec![
                DiffLine {
                    kind: DiffRowKind::Context,
                    text: " a".into(),
                    old_no: Some(1),
                    new_no: Some(1),
                },
                DiffLine {
                    kind: DiffRowKind::Added,
                    text: "+b".into(),
                    old_no: None,
                    new_no: Some(2),
                },
                DiffLine {
                    kind: DiffRowKind::Context,
                    text: " c".into(),
                    old_no: Some(3),
                    new_no: Some(3),
                },
                DiffLine {
                    kind: DiffRowKind::Removed,
                    text: "-d".into(),
                    old_no: Some(4),
                    new_no: None,
                },
            ],
            hunks: vec![],
            hunk_idx: 0,
            scroll: 0,
            loading: false,
            header: None,
            error: None,
        });
        if let Some(d) = app.diff.as_mut() {
            apply_hunks(d);
        }
        assert_eq!(app.diff.as_ref().unwrap().hunks.len(), 2);
        assert_eq!(app.diff.as_ref().unwrap().hunk_idx, 0);
        assert_eq!(app.diff.as_ref().unwrap().scroll, 1);
        app.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(app.diff.as_ref().unwrap().hunk_idx, 1);
        assert_eq!(app.diff.as_ref().unwrap().scroll, 3);
        app.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(app.diff.as_ref().unwrap().hunk_idx, 0);
        app.handle_key(KeyEvent::from(KeyCode::Char('p')));
        assert_eq!(app.diff.as_ref().unwrap().hunk_idx, 1);
    }

    #[test]
    fn scrolling_diff_updates_hunk_selection() {
        let mut d = DiffView {
            title: "t".into(),
            target: "abc".into(),
            base: None,
            three_dot: false,
            files: vec![],
            file_idx: 0,
            lines: vec![],
            hunks: vec![
                Hunk {
                    start: 0,
                    end: 2,
                    label: "#1".into(),
                },
                Hunk {
                    start: 5,
                    end: 8,
                    label: "#2".into(),
                },
            ],
            hunk_idx: 0,
            scroll: 0,
            loading: false,
            header: None,
            error: None,
        };
        d.scroll = 6;
        sync_hunk_from_scroll(&mut d);
        assert_eq!(d.hunk_idx, 1);
        d.scroll = 1;
        sync_hunk_from_scroll(&mut d);
        assert_eq!(d.hunk_idx, 0);
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
            contributors: vec![],
            contributors_err: None,
        });
        app.handle_key(KeyEvent::from(KeyCode::Char('/')));
        app.handle_key(KeyEvent::from(KeyCode::Char('t')));
        app.handle_key(KeyEvent::from(KeyCode::Char('u')));
        app.handle_key(KeyEvent::from(KeyCode::Char('i')));
        assert_eq!(app.visible_indices(), vec![1]);
    }

    #[test]
    fn sort_repo_indices_by_name_and_date() {
        let repos = vec![
            repo("zeta", "/z"),
            repo("alpha", "/a"),
        ];
        let mut rows = HashMap::new();
        rows.insert(
            0,
            HomeRow {
                branch: "main".into(),
                ahead: 0,
                behind: 0,
                dirty: 0,
                last_commit: "2026-01-01".into(),
            },
        );
        rows.insert(
            1,
            HomeRow {
                branch: "main".into(),
                ahead: 0,
                behind: 0,
                dirty: 0,
                last_commit: "2026-08-01".into(),
            },
        );
        let mut idx = vec![0, 1];
        sort_repo_indices(&mut idx, &repos, &rows, 0);
        assert_eq!(idx, vec![1, 0]);
        sort_repo_indices(&mut idx, &repos, &rows, 2);
        assert_eq!(idx, vec![1, 0]);
    }

    #[test]
    fn resolve_hunk_command_variants() {
        let repo = PathBuf::from("/tmp/repo");
        let wt = resolve_diff_command("hunk", &repo, None, "WORKING_TREE").unwrap();
        assert_eq!(wt.args, vec!["diff"]);
        let show = resolve_diff_command("hunk", &repo, None, "abc123").unwrap();
        assert_eq!(show.args, vec!["show", "abc123"]);
        let range = resolve_diff_command("hunk", &repo, Some("aaa"), "bbb").unwrap();
        assert_eq!(range.args, vec!["diff", "aaa..bbb"]);
        assert!(range.pipe_git_diff.is_none());
        let templated =
            resolve_diff_command("hunk show {base} {target}", &repo, Some("aaa"), "bbb").unwrap();
        assert_eq!(templated.args, vec!["diff", "aaa..bbb"]);
        assert!(resolve_diff_command("", &repo, None, "abc").is_err());
    }
}
