use crate::config::{self, Member, Repository};
use crate::git;
use egui::Color32;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

const SIDEBAR_PADDING: f32 = 15.0;

fn draw_indented_separator(ui: &mut egui::Ui, indent: f32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.add_space(indent);
        let w = ui.available_width() - indent;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
        ui.painter().line_segment(
            [
                egui::pos2(rect.left(), rect.center().y),
                egui::pos2(rect.right(), rect.center().y),
            ],
            egui::Stroke::new(1.0, color),
        );
    });
}

mod dashboard;
mod diff;
mod home;
mod settings;
mod widgets;

/// Load a repository's cached analysis from disk (best effort: any parse
/// failure — e.g. schema change — just means a cache miss).
fn load_repo_disk_cache(repo_path: &std::path::Path) -> Option<RepoData> {
    let content = std::fs::read_to_string(config::repo_cache_path(repo_path)).ok()?;
    serde_json::from_str(&content).ok()
}

/// Persist a repository's analysis so the next launch can render instantly
/// (stale-while-revalidate: cached data shows first, a refresh replaces it).
fn save_repo_disk_cache(repo_path: &std::path::Path, data: &RepoData) {
    let _ = std::fs::create_dir_all(config::cache_dir());
    if let Ok(json) = serde_json::to_string(data) {
        let _ = config::write_atomic(&config::repo_cache_path(repo_path), &json);
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct RepoData {
    pub summary: git::Summary,
    pub contributors: Vec<git::Contributor>,
    pub branches: Vec<git::BranchInfo>,
    pub tags: Vec<git::TagInfo>,
    pub activity: git::Activity,
    pub files: git::FilesReport,
    pub readme: Option<(String, String)>,
    pub recent_commits: Vec<git::CommitSummary>,
    pub tech_info: git::TechInfo,
    pub stash_list: Vec<git::StashEntry>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub started_at: std::time::Instant,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PullState {
    Pulling,
    Success(String),
    Error(String),
}

enum AsyncMessage {
    RepoLoaded {
        repo_index: usize,
        data: Box<Result<RepoData, String>>,
        // Non-fatal analysis failures (the affected sections show defaults)
        warnings: Vec<String>,
    },
    PullFinished {
        path: PathBuf,
        result: Result<String, String>,
    },
    FetchFinished {
        path: PathBuf,
        result: Result<String, String>,
    },
    ChangedFilesLoaded {
        seq: u64,
        result: Result<Vec<git::ChangedFile>, String>,
    },
    FileDiffLoaded {
        seq: u64,
        result: Result<git::FileDiff, String>,
    },
    FileBlameLoaded {
        seq: u64,
        result: Result<Vec<git::BlameEntry>, String>,
    },
    BranchLogLoaded {
        seq: u64,
        result: Result<String, String>,
    },
    CommitDetailLoaded {
        seq: u64,
        stat: Result<String, String>,
        files: Result<Vec<git::ChangedFile>, String>,
    },
    ActivityLoaded {
        seq: u64,
        result: Result<git::Activity, String>,
    },
    DiffCommitsLoaded {
        seq: u64,
        result: Result<Vec<git::CommitSummary>, String>,
    },
    StashOpFinished {
        repo_idx: usize,
        action: StashAction,
        result: Result<(), String>,
    },
    WorkerPanicked {
        detail: String,
    },
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct RegisterMemberDialogState {
    pub contributor_name: String,
    pub contributor_email: String,
    pub canonical_name: String,
    pub aliases_input: String,
    pub error_msg: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct AddAliasDialogState {
    pub contributor_name: String,
    pub contributor_email: String,
    pub selected_member_idx: usize,
    pub filter_query: String,
    pub error_msg: Option<String>,
}

/// Stash operations executed on a worker thread (sync git on the UI thread
/// froze the app, especially on SSH repositories).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum StashAction {
    Apply,
    Drop,
}

pub(crate) enum Job {
    LoadRepo {
        index: usize,
        path: PathBuf,
        members: Vec<Member>,
        ctx: egui::Context,
    },
    PullRepo {
        path: PathBuf,
        lang: crate::config::Language,
        ctx: egui::Context,
    },
    FetchRepo {
        path: PathBuf,
        lang: crate::config::Language,
        ctx: egui::Context,
    },
    LoadChangedFiles {
        seq: u64,
        repo_path: PathBuf,
        base: Option<String>,
        target: String,
        three_dot: bool,
        ctx: egui::Context,
    },
    LoadFileDiff {
        seq: u64,
        repo_path: PathBuf,
        base: Option<String>,
        target: String,
        file: String,
        ignore_whitespace: bool,
        full: bool,
        three_dot: bool,
        ctx: egui::Context,
    },
    LoadFileBlame {
        seq: u64,
        repo_path: PathBuf,
        blame_ref: String,
        file: String,
        ctx: egui::Context,
    },
    LoadBranchLog {
        seq: u64,
        repo_path: PathBuf,
        branch: String,
        limit: usize,
        ctx: egui::Context,
    },
    LoadCommitDetail {
        seq: u64,
        repo_path: PathBuf,
        hash: String,
        ctx: egui::Context,
    },
    LoadActivity {
        seq: u64,
        repo_path: PathBuf,
        days: usize,
        ctx: egui::Context,
    },
    LoadDiffCommits {
        seq: u64,
        repo_path: PathBuf,
        ctx: egui::Context,
    },
    StashOp {
        repo_idx: usize,
        repo_path: PathBuf,
        stash_ref: String,
        action: StashAction,
        ctx: egui::Context,
    },
}

impl Job {
    /// Background jobs (whole-repo analysis, bulk pull/fetch) may wait behind
    /// interactive ones; everything the user is actively looking at goes first.
    fn is_background(&self) -> bool {
        matches!(
            self,
            Job::LoadRepo { .. } | Job::PullRepo { .. } | Job::FetchRepo { .. }
        )
    }

    /// The egui context carried by every job, for requesting a repaint when
    /// the job itself could not run to completion (e.g. it panicked).
    fn ctx(&self) -> &egui::Context {
        match self {
            Job::LoadRepo { ctx, .. }
            | Job::PullRepo { ctx, .. }
            | Job::FetchRepo { ctx, .. }
            | Job::LoadChangedFiles { ctx, .. }
            | Job::LoadFileDiff { ctx, .. }
            | Job::LoadFileBlame { ctx, .. }
            | Job::LoadBranchLog { ctx, .. }
            | Job::LoadCommitDetail { ctx, .. }
            | Job::LoadActivity { ctx, .. }
            | Job::LoadDiffCommits { ctx, .. }
            | Job::StashOp { ctx, .. } => ctx,
        }
    }

    /// True when a newer request of the same class has been issued since this
    /// job was queued — workers drop such jobs before running git at all
    /// (the receive-side seq check alone still executed every queued request).
    fn is_stale(&self, seqs: &JobSeqs) -> bool {
        match self {
            Job::LoadChangedFiles { seq, .. } => *seq != cur_seq(&seqs.files),
            Job::LoadFileDiff { seq, .. } => *seq != cur_seq(&seqs.content),
            Job::LoadFileBlame { seq, .. } => *seq != cur_seq(&seqs.blame),
            Job::LoadBranchLog { seq, .. } => *seq != cur_seq(&seqs.branch_log),
            Job::LoadCommitDetail { seq, .. } => *seq != cur_seq(&seqs.commit_detail),
            Job::LoadActivity { seq, .. } => *seq != cur_seq(&seqs.activity),
            Job::LoadDiffCommits { seq, .. } => *seq != cur_seq(&seqs.diff_commits),
            _ => false,
        }
    }
}

/// Latest request generation per job class, shared between the app (which
/// bumps on every new request) and the workers (which skip superseded jobs).
#[derive(Default)]
pub(crate) struct JobSeqs {
    pub(crate) files: std::sync::atomic::AtomicU64,
    pub(crate) content: std::sync::atomic::AtomicU64,
    pub(crate) blame: std::sync::atomic::AtomicU64,
    pub(crate) branch_log: std::sync::atomic::AtomicU64,
    pub(crate) commit_detail: std::sync::atomic::AtomicU64,
    pub(crate) activity: std::sync::atomic::AtomicU64,
    pub(crate) diff_commits: std::sync::atomic::AtomicU64,
}

/// Bump a request generation and return the new value to tag the job with.
pub(crate) fn bump_seq(seq: &std::sync::atomic::AtomicU64) -> u64 {
    seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

/// Current generation of a job class (for receive-side staleness checks).
pub(crate) fn cur_seq(seq: &std::sync::atomic::AtomicU64) -> u64 {
    seq.load(std::sync::atomic::Ordering::Relaxed)
}

/// Two-tier job queue: a bulk pull or startup analysis used to fill the single
/// FIFO channel, making clicks (diff, commit detail …) wait minutes behind it.
/// Interactive jobs now always pop first. Mutex+Condvar keeps this free of
/// extra dependencies; poisoned locks are recovered like the old worker loop.
pub(crate) struct JobQueue {
    inner: std::sync::Mutex<(
        std::collections::VecDeque<Job>,
        std::collections::VecDeque<Job>,
    )>,
    cv: std::sync::Condvar,
}

impl JobQueue {
    fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new((
                std::collections::VecDeque::new(),
                std::collections::VecDeque::new(),
            )),
            cv: std::sync::Condvar::new(),
        }
    }

    pub(crate) fn push(&self, job: Job) {
        let mut q = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if job.is_background() {
            q.1.push_back(job);
        } else {
            q.0.push_back(job);
        }
        self.cv.notify_one();
    }

    fn pop(&self) -> Job {
        let mut q = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        loop {
            if let Some(job) = q.0.pop_front() {
                return job;
            }
            if let Some(job) = q.1.pop_front() {
                return job;
            }
            q = match self.cv.wait(q) {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum DiffMode {
    Single,
    Range,
}

/// Intra-line highlight ranges for one diff row: (left side, right side)
pub(crate) type EmphRanges = (Vec<(usize, usize)>, Vec<(usize, usize)>);

/// Execute one worker job. Runs on a worker thread; panics are caught by
/// the caller and surfaced as an error toast.
fn handle_job(job: Job, tx: &std::sync::mpsc::Sender<AsyncMessage>) {
    match job {
        Job::LoadRepo {
            index,
            path,
            members,
            ctx,
        } => {
            // Secondary analyses degrade to defaults + a warning instead of
            // turning the whole dashboard into an error; only the summary
            // (repo identity / current branch) is essential.
            let mut warnings: Vec<String> = Vec::new();
            macro_rules! soft {
                ($name:expr, $call:expr, $default:expr) => {
                    match $call {
                        Ok(v) => v,
                        Err(e) => {
                            warnings.push(format!("{}: {}", $name, e.trim()));
                            $default
                        }
                    }
                };
            }
            let data = match git::get_summary(&path, &members) {
                Err(e) => Err(e),
                Ok(summary) => {
                    let contributors = soft!(
                        "contributors",
                        git::get_contributors(&path, &members),
                        Vec::new()
                    );
                    let branches = soft!("branches", git::get_branches(&path), Vec::new());
                    let tags = soft!("tags", git::get_tags(&path), Vec::new());
                    let activity = soft!(
                        "activity",
                        git::get_activity(&path, 30),
                        git::Activity::default()
                    );
                    let files = soft!(
                        "file structure",
                        git::get_files_report(&path),
                        git::FilesReport::default()
                    );
                    let readme = git::find_readme(&path);
                    let recent_commits =
                        soft!("recent commits", git::get_recent_commits(&path), Vec::new());
                    let tech_info = git::detect_tech_info(&path);
                    let stash_list = soft!("stashes", git::get_stash_list(&path), Vec::new());
                    Ok(RepoData {
                        summary,
                        contributors,
                        branches,
                        tags,
                        activity,
                        files,
                        readme,
                        recent_commits,
                        tech_info,
                        stash_list,
                    })
                }
            };
            // Persist fresh results from the worker thread so the
            // next launch renders without re-running git analysis
            if let Ok(ref d) = data {
                save_repo_disk_cache(&path, d);
            }
            let _ = tx.send(AsyncMessage::RepoLoaded {
                repo_index: index,
                data: Box::new(data),
                warnings,
            });
            ctx.request_repaint();
        }
        Job::PullRepo { path, lang, ctx } => {
            let res = git::pull_repository(&path, lang);
            let _ = tx.send(AsyncMessage::PullFinished { path, result: res });
            ctx.request_repaint();
        }
        Job::FetchRepo { path, lang, ctx } => {
            let res = git::fetch_repository(&path, lang);
            let _ = tx.send(AsyncMessage::FetchFinished { path, result: res });
            ctx.request_repaint();
        }
        Job::LoadChangedFiles {
            seq,
            repo_path,
            base,
            target,
            three_dot,
            ctx,
        } => {
            let result = git::get_changed_files(&repo_path, base.as_deref(), &target, three_dot);
            let _ = tx.send(AsyncMessage::ChangedFilesLoaded { seq, result });
            ctx.request_repaint();
        }
        Job::LoadFileDiff {
            seq,
            repo_path,
            base,
            target,
            file,
            ignore_whitespace,
            full,
            three_dot,
            ctx,
        } => {
            let result = git::get_file_diff(
                &repo_path,
                base.as_deref(),
                &target,
                &file,
                ignore_whitespace,
                full,
                three_dot,
            );
            let _ = tx.send(AsyncMessage::FileDiffLoaded { seq, result });
            ctx.request_repaint();
        }
        Job::LoadFileBlame {
            seq,
            repo_path,
            blame_ref,
            file,
            ctx,
        } => {
            let result = git::get_file_blame(&repo_path, &blame_ref, &file);
            let _ = tx.send(AsyncMessage::FileBlameLoaded { seq, result });
            ctx.request_repaint();
        }
        Job::LoadBranchLog {
            seq,
            repo_path,
            branch,
            limit,
            ctx,
        } => {
            let result = git::get_branch_oneline_log(&repo_path, &branch, limit);
            let _ = tx.send(AsyncMessage::BranchLogLoaded { seq, result });
            ctx.request_repaint();
        }
        Job::LoadCommitDetail {
            seq,
            repo_path,
            hash,
            ctx,
        } => {
            let stat = git::get_commit_show(&repo_path, &hash);
            let files = git::get_changed_files(&repo_path, None, &hash, false);
            let _ = tx.send(AsyncMessage::CommitDetailLoaded { seq, stat, files });
            ctx.request_repaint();
        }
        Job::LoadActivity {
            seq,
            repo_path,
            days,
            ctx,
        } => {
            let result = git::get_activity(&repo_path, days);
            let _ = tx.send(AsyncMessage::ActivityLoaded { seq, result });
            ctx.request_repaint();
        }
        Job::LoadDiffCommits {
            seq,
            repo_path,
            ctx,
        } => {
            let result = git::get_commits_for_diff(&repo_path);
            let _ = tx.send(AsyncMessage::DiffCommitsLoaded { seq, result });
            ctx.request_repaint();
        }
        Job::StashOp {
            repo_idx,
            repo_path,
            stash_ref,
            action,
            ctx,
        } => {
            let result = match action {
                StashAction::Apply => git::apply_stash(&repo_path, &stash_ref).map(|_| ()),
                StashAction::Drop => git::drop_stash(&repo_path, &stash_ref).map(|_| ()),
            };
            let _ = tx.send(AsyncMessage::StashOpFinished {
                repo_idx,
                action,
                result,
            });
            ctx.request_repaint();
        }
    }
}

pub struct GitDashboardApp {
    // Application state data
    repositories: Vec<Repository>,
    members: Vec<Member>,
    selected_repo_index: Option<usize>,
    viewing_settings: bool,
    active_filter: bool,
    // Contributors table caps at 100 rows until the user opts into all
    pub(crate) contributors_show_all: bool,

    // Async communication channels
    rx: Receiver<AsyncMessage>,
    jobs: std::sync::Arc<JobQueue>,

    // Cache and status
    repo_cache: HashMap<usize, Result<RepoData, String>>,
    // Bumped on every repo_cache / repositories mutation; keys the memoized
    // home list so it is not rebuilt (clone + sort) every frame
    repo_cache_gen: u64,
    home_items_cache: Option<(String, usize, u64, Vec<home::RepoFilterSortInput>)>,
    loading_repos: HashSet<usize>,
    // Repos already (re)analyzed this session: disk-cached repos refresh
    // lazily on first visit instead of all at once on startup
    session_refreshed: HashSet<usize>,
    pull_statuses: HashMap<PathBuf, PullState>,

    // UI input buffers (settings screen)
    new_repo_name: String,
    new_repo_path: String,
    repo_add_error: Option<String>,

    new_member_name: String,
    new_member_aliases: String,
    new_member_active: bool,
    member_add_error: Option<String>,

    // Branch-specific commit display state
    selected_branch: Option<String>,
    // Quick filter for the branch list (repos tracking many remotes)
    pub(crate) branch_filter: String,
    selected_commit_hash: Option<String>,
    selected_commit_stat: Option<String>,
    // Files changed by the selected commit, fetched together with the stat on
    // a worker thread (a sync git call here would freeze the UI; seq-guarded).
    // The tree is built once on arrival (never per frame) from the file list.
    selected_commit_files: Option<Vec<git::ChangedFile>>,
    commit_files_tree: Option<Vec<diff::DiffTreeNode>>,
    commit_tree_collapsed: std::collections::HashSet<String>,
    commit_detail_loading: bool,
    // Activity-tab period selection: 30 days renders straight from RepoData;
    // other periods are fetched on demand (seq-guarded) into the override
    activity_period_days: usize,
    activity_override: Option<git::Activity>,
    activity_loading: bool,
    branch_oneline_log: Option<String>,
    // Commit graph paging: fetched async via Job::LoadBranchLog (seq drops stale results)
    pub(crate) graph_log_limit: usize,
    pub(crate) branch_log_loading: bool,

    // Tag comparison markers (same interaction as the commit graph):
    // ●left picks the base tag, ●right picks the target tag
    pub(crate) tag_diff_base: Option<String>,
    pub(crate) tag_diff_target: Option<String>,

    // Settings tab index (0=repos, 1=members)
    settings_tab: usize,

    // Inline edit and delete confirmation state
    editing_repo_idx: Option<usize>,
    editing_repo_name: String,
    editing_repo_path: String,
    editing_repo_error: Option<String>,

    editing_member_idx: Option<usize>,
    editing_member_name: String,
    editing_member_aliases: String,
    editing_member_error: Option<String>,

    confirm_delete_repo_idx: Option<usize>,
    confirm_delete_member_idx: Option<usize>,

    // Home screen search/sort and sidebar filter state
    repo_search_query: String,
    repo_sort_by: usize, // 0=name asc, 1=name desc, 2=last-updated newest, 3=last-updated oldest
    sidebar_search_query: String,

    // Settings save feedback display (2 seconds)
    pub(crate) repo_added_at: Option<std::time::Instant>,
    pub(crate) member_added_at: Option<std::time::Instant>,

    // Diff view state
    pub(crate) viewing_diff: bool,
    pub(crate) diff_repo_idx: Option<usize>,
    pub(crate) diff_mode: DiffMode,
    pub(crate) diff_base: Option<String>,
    pub(crate) diff_target: Option<String>,
    // Range mode: compare from the merge-base (base...target, GitHub style)
    pub(crate) diff_three_dot: bool,
    // Commit list for the pickers (fetched async; a sync git call here froze
    // the UI when opening the diff view on SSH repositories)
    pub(crate) diff_commit_list: Vec<git::CommitSummary>,
    pub(crate) diff_commits_loading: bool,
    pub(crate) diff_changed_files: Option<Result<Vec<git::ChangedFile>, String>>,
    pub(crate) diff_loading_files: bool,
    pub(crate) diff_selected_file: Option<String>,
    // Arc: the diff view clones this every frame — deep-cloning thousands of
    // tokenized rows at 60fps was the single largest per-frame allocation
    diff_file_content: Option<Result<std::sync::Arc<git::FileDiff>, String>>,
    diff_loading_content: bool,
    // Widest diff row in monospace columns; computed once per file load
    pub(crate) diff_max_cols: usize,
    pub(crate) diff_show_blame: bool,
    pub(crate) diff_files_collapsed: bool,
    pub(crate) diff_fullscreen: bool,
    pub(crate) diff_hscroll: f32,
    pub(crate) diff_blame_content: Option<Result<Vec<git::BlameEntry>, String>>,
    pub(crate) diff_loading_blame: bool,
    // Request generation counters per job class: results tagged with an older
    // seq are dropped on receive, and workers also skip superseded jobs before
    // running git at all (the atomics are shared with the worker threads).
    seqs: std::sync::Arc<JobSeqs>,
    // Search text shared by the base/target ref-picker popups (one open at a time)
    pub(crate) diff_picker_search: String,
    pub(crate) diff_picker_focus: bool,
    // Per-frame computation caches (immediate-mode UI re-runs draw code at
    // 60fps, so anything O(rows) must be memoized):
    // file tree of the changed-files panel; invalidated when the list reloads
    pub(crate) diff_tree_cache: Option<Vec<diff::DiffTreeNode>>,
    // search hits keyed by (query, content seq)
    pub(crate) diff_search_matches_cache: Option<(String, u64, Vec<usize>)>,
    // intra-line word-diff ranges keyed by row index; cleared per file load
    pub(crate) diff_emph_cache: HashMap<usize, EmphRanges>,
    pub(crate) diff_selected_line_idx: Option<usize>,
    pub(crate) diff_ignore_whitespace: bool,
    pub(crate) diff_full_file: bool,
    pub(crate) diff_search_query: String,
    pub(crate) diff_search_visible: bool,
    pub(crate) diff_search_active_index: usize,
    pub(crate) diff_search_scroll_to: Option<usize>,
    pub(crate) diff_search_focus_input: bool,
    pub(crate) diff_hunk_pos: usize,

    // Graph-based diff selection (T49)
    pub(crate) graph_diff_base: Option<String>,
    pub(crate) graph_diff_target: Option<String>,

    // Theme and layout settings
    pub(crate) theme: crate::theme::Theme,
    pub(crate) themes: Vec<(String, crate::theme::Theme)>,
    pub(crate) prefs: crate::config::Preferences,
    // The concrete theme name last applied via apply_visuals. Always a real
    // name from `themes` — never "System" itself, which ensure_theme_applied
    // resolves through crate::theme::resolve_system_theme_name each frame.
    // Guards against re-applying (and thus repainting) every frame; see
    // aero-grep's doc/design.md pitfall #4 for why that matters.
    applied_theme_name: String,
    pub(crate) dashboard_tab: usize,
    pub(crate) diff_collapsed_dirs: std::collections::HashSet<String>,

    // Toast notifications and stash drop confirmation dialog
    pub(crate) toasts: Vec<Toast>,
    pub(crate) confirm_drop_stash: Option<(usize, String)>,
    #[allow(dead_code)]
    pub(crate) register_member_dialog: Option<RegisterMemberDialogState>,
    #[allow(dead_code)]
    pub(crate) add_alias_dialog: Option<AddAliasDialogState>,
    pub(crate) commonmark_cache: egui_commonmark::CommonMarkCache,

    // Command palette (Ctrl/Cmd+K), design-system.md §6
    pub(crate) command_palette_open: bool,
    pub(crate) command_palette_query: String,
    pub(crate) command_palette_selected: usize,

    app_icon: Option<egui::TextureHandle>,
}

#[derive(Clone)]
enum PaletteAction {
    OpenRepo(usize),
    OpenSettings,
    ToggleSidebar,
    Pull,
    SetTheme(String),
}

impl GitDashboardApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Set up Japanese (CJK) fonts
        crate::theme::setup_fonts(&cc.egui_ctx);

        // Load configuration
        let prefs = config::load_preferences();
        let themes = crate::theme::load_themes();
        let resolved_theme_name = if prefs.theme == crate::theme::SYSTEM_THEME_NAME {
            crate::theme::resolve_system_theme_name(&cc.egui_ctx).to_string()
        } else {
            prefs.theme.clone()
        };
        let theme = themes
            .iter()
            .find(|(name, _)| name == &resolved_theme_name)
            .map(|(_, t)| *t)
            .unwrap_or_else(|| themes[0].1);

        // Apply the selected theme
        crate::theme::apply_visuals(&cc.egui_ctx, &theme);

        let repositories = config::load_repositories();
        let members = config::load_members();

        let (tx, rx) = channel();
        let jobs = std::sync::Arc::new(JobQueue::new());
        let seqs = std::sync::Arc::new(JobSeqs::default());

        // Determine worker count from logical CPU count (fallback 4, cap 8)
        let num_workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);

        // Spawn worker threads (they live for the whole process; exiting the
        // app tears them down with it, same as the old closed-channel break)
        for _ in 0..num_workers {
            let jobs = std::sync::Arc::clone(&jobs);
            let seqs = std::sync::Arc::clone(&seqs);
            let tx = tx.clone();
            std::thread::spawn(move || {
                loop {
                    let job = jobs.pop();

                    // A newer request of the same class supersedes this one:
                    // drop it before spending a git invocation on it
                    if job.is_stale(&seqs) {
                        continue;
                    }

                    // Convert panics into an error toast instead of letting
                    // them silently kill this worker thread (the pool would
                    // shrink permanently with no feedback otherwise)
                    let panic_ctx = job.ctx().clone();
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        handle_job(job, &tx);
                    }));
                    if let Err(payload) = outcome {
                        let detail = payload
                            .downcast_ref::<&str>()
                            .map(|s| s.to_string())
                            .or_else(|| payload.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| {
                                crate::i18n::t(prefs.language, "unknown_error").to_string()
                            });
                        let _ = tx.send(AsyncMessage::WorkerPanicked { detail });
                        panic_ctx.request_repaint();
                    }
                }
            });
        }

        // Load the app icon texture once at startup so the sidebar header can
        // render the real icon instead of the 'G' placeholder.
        let icon_bytes = include_bytes!("../../assets/icon-64.rgba");
        let icon_image = egui::ColorImage::from_rgba_unmultiplied([64, 64], icon_bytes);
        let app_icon_handle =
            cc.egui_ctx
                .load_texture("app-icon", icon_image, egui::TextureOptions::LINEAR);

        // Initialize application state
        let mut app = Self {
            repositories,
            members,
            selected_repo_index: None,
            viewing_settings: false,
            active_filter: false,
            contributors_show_all: false,
            rx,
            jobs,
            repo_cache: HashMap::new(),
            repo_cache_gen: 0,
            home_items_cache: None,
            loading_repos: HashSet::new(),
            session_refreshed: HashSet::new(),
            pull_statuses: HashMap::new(),
            new_repo_name: String::new(),
            new_repo_path: String::new(),
            repo_add_error: None,
            new_member_name: String::new(),
            new_member_aliases: String::new(),
            new_member_active: true,
            member_add_error: None,
            selected_branch: None,
            branch_filter: String::new(),
            graph_log_limit: 100,
            branch_log_loading: false,
            tag_diff_base: None,
            tag_diff_target: None,
            selected_commit_hash: None,
            selected_commit_stat: None,
            selected_commit_files: None,
            commit_files_tree: None,
            commit_tree_collapsed: std::collections::HashSet::new(),
            commit_detail_loading: false,
            activity_period_days: 30,
            activity_override: None,
            activity_loading: false,
            branch_oneline_log: None,
            settings_tab: 0,
            editing_repo_idx: None,
            editing_repo_name: String::new(),
            editing_repo_path: String::new(),
            editing_repo_error: None,
            editing_member_idx: None,
            editing_member_name: String::new(),
            editing_member_aliases: String::new(),
            editing_member_error: None,
            confirm_delete_repo_idx: None,
            confirm_delete_member_idx: None,
            repo_search_query: String::new(),
            repo_sort_by: 2,
            sidebar_search_query: String::new(),
            repo_added_at: None,
            member_added_at: None,
            viewing_diff: false,
            diff_repo_idx: None,
            diff_mode: DiffMode::Single,
            diff_base: None,
            diff_target: None,
            diff_three_dot: false,
            diff_commit_list: Vec::new(),
            diff_commits_loading: false,
            diff_changed_files: None,
            diff_loading_files: false,
            diff_selected_file: None,
            diff_file_content: None,
            diff_loading_content: false,
            diff_max_cols: 0,
            diff_show_blame: prefs.diff_show_blame,
            diff_files_collapsed: false,
            diff_fullscreen: false,
            diff_hscroll: 0.0,
            diff_blame_content: None,
            diff_loading_blame: false,
            seqs,
            diff_picker_search: String::new(),
            diff_picker_focus: false,
            diff_tree_cache: None,
            diff_search_matches_cache: None,
            diff_emph_cache: HashMap::new(),
            diff_selected_line_idx: None,
            diff_ignore_whitespace: prefs.diff_ignore_whitespace,
            diff_full_file: prefs.diff_full_file,
            diff_search_query: String::new(),
            diff_search_visible: false,
            diff_search_active_index: 0,
            diff_search_scroll_to: None,
            diff_search_focus_input: false,
            diff_hunk_pos: 0,

            graph_diff_base: None,
            graph_diff_target: None,

            // Theme and layout settings
            theme,
            themes,
            prefs,
            applied_theme_name: resolved_theme_name,
            dashboard_tab: 0,
            diff_collapsed_dirs: std::collections::HashSet::new(),

            toasts: Vec::new(),
            confirm_drop_stash: None,
            register_member_dialog: None,
            add_alias_dialog: None,
            commonmark_cache: egui_commonmark::CommonMarkCache::default(),

            command_palette_open: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,

            app_icon: Some(app_icon_handle),
        };

        // Pre-populate from the disk cache so home/dashboards render instantly.
        // Only repos WITHOUT a cache are analyzed up front; cached ones refresh
        // lazily (in the background) the first time they are opened, so startup
        // no longer fires dozens of git invocations with many registered repos.
        for idx in 0..app.repositories.len() {
            if let Some(data) = load_repo_disk_cache(&app.repositories[idx].path) {
                app.repo_cache.insert(idx, Ok(data));
            } else {
                app.load_repo_async(idx, cc.egui_ctx.clone());
            }
        }

        app
    }

    fn select_repository(&mut self, index: usize, ctx: egui::Context) {
        if index < self.repositories.len() {
            self.selected_repo_index = Some(index);
            self.viewing_settings = false;
            self.viewing_diff = false;
            self.selected_branch = None;
            self.branch_filter.clear();
            self.contributors_show_all = false;
            self.clear_commit_detail();
            self.activity_period_days = 30;
            self.activity_override = None;
            self.activity_loading = false;
            bump_seq(&self.seqs.activity); // drop any in-flight activity fetch
            self.branch_oneline_log = None;
            self.graph_log_limit = 100;
            bump_seq(&self.seqs.branch_log); // drop any in-flight graph fetch
            self.branch_log_loading = false;
            self.graph_diff_base = None;
            self.graph_diff_target = None;
            self.tag_diff_base = None;
            self.tag_diff_target = None;

            // Not cached: analyze now. Cached from disk: refresh in the
            // background once per session (stale data stays on screen).
            if !self.repo_cache.contains_key(&index) || !self.session_refreshed.contains(&index) {
                self.load_repo_async(index, ctx);
            }
        }
    }

    /// Command palette (Ctrl/Cmd+K) contents: core actions only (MVP scope,
    /// not an exhaustive index of every action in the app — see task_006).
    fn palette_commands(&self) -> Vec<(String, PaletteAction)> {
        let lang = self.prefs.language;
        let mut cmds = Vec::new();
        cmds.push((
            crate::i18n::t(lang, "palette_open_settings").to_string(),
            PaletteAction::OpenSettings,
        ));
        cmds.push((
            if self.prefs.sidebar_collapsed {
                crate::i18n::t(lang, "palette_expand_sidebar").to_string()
            } else {
                crate::i18n::t(lang, "palette_collapse_sidebar").to_string()
            },
            PaletteAction::ToggleSidebar,
        ));
        if self.selected_repo_index.is_some() {
            cmds.push((
                crate::i18n::t(lang, "palette_pull_selected").to_string(),
                PaletteAction::Pull,
            ));
        }
        for (idx, repo) in self.repositories.iter().enumerate() {
            let label = match lang {
                crate::config::Language::English => format!("Open repository: {}", repo.name),
                crate::config::Language::Japanese => format!("リポジトリを開く: {}", repo.name),
            };
            cmds.push((label, PaletteAction::OpenRepo(idx)));
        }
        cmds.push((
            crate::i18n::t(lang, "palette_theme_system").to_string(),
            PaletteAction::SetTheme(crate::theme::SYSTEM_THEME_NAME.to_string()),
        ));
        for (name, _) in self.themes.iter() {
            let label = match lang {
                crate::config::Language::English => format!("Theme: {}", name),
                crate::config::Language::Japanese => format!("テーマ: {}", name),
            };
            cmds.push((label, PaletteAction::SetTheme(name.clone())));
        }
        cmds
    }

    fn execute_palette_action(&mut self, action: &PaletteAction, ctx: &egui::Context) {
        match action {
            PaletteAction::OpenRepo(idx) => {
                self.select_repository(*idx, ctx.clone());
            }
            PaletteAction::OpenSettings => {
                self.viewing_settings = true;
                self.selected_repo_index = None;
                self.viewing_diff = false;
            }
            PaletteAction::ToggleSidebar => {
                self.prefs.sidebar_collapsed = !self.prefs.sidebar_collapsed;
                let _ = crate::config::save_preferences(&self.prefs);
            }
            PaletteAction::Pull => {
                if let Some(idx) = self.selected_repo_index {
                    let path = self.repositories[idx].path.clone();
                    self.pull_statuses.insert(path.clone(), PullState::Pulling);
                    self.jobs.push(Job::PullRepo {
                        path,
                        lang: self.prefs.language,
                        ctx: ctx.clone(),
                    });
                }
            }
            PaletteAction::SetTheme(name) => {
                self.prefs.theme = name.clone();
                self.ensure_theme_applied(ctx);
                let _ = crate::config::save_preferences(&self.prefs);
            }
        }
    }

    /// Re-resolve and (only if it actually changed) apply the current theme.
    /// Must be called every frame so a "System" selection keeps following the
    /// OS's light/dark setting — but the change check means apply_visuals
    /// itself only runs when the resolved theme differs from what's already
    /// applied, not unconditionally (an unconditional call here would
    /// request a repaint every frame; see aero-grep's doc/design.md #4).
    fn ensure_theme_applied(&mut self, ctx: &egui::Context) {
        let target_name = if self.prefs.theme == crate::theme::SYSTEM_THEME_NAME {
            crate::theme::resolve_system_theme_name(ctx)
        } else {
            self.prefs.theme.as_str()
        };
        if target_name != self.applied_theme_name {
            if let Some((_, theme)) = self.themes.iter().find(|(n, _)| n == target_name) {
                self.theme = *theme;
                crate::theme::apply_visuals(ctx, &self.theme);
            }
            self.applied_theme_name = target_name.to_string();
        }
    }

    /// Close the commit-detail panel and invalidate any in-flight fetch
    pub(crate) fn clear_commit_detail(&mut self) {
        self.selected_commit_hash = None;
        self.selected_commit_stat = None;
        self.selected_commit_files = None;
        self.commit_files_tree = None;
        self.commit_tree_collapsed.clear();
        self.commit_detail_loading = false;
        bump_seq(&self.seqs.commit_detail); // drop in-flight results
    }

    /// Fetch activity for a non-default period on a worker thread
    pub(crate) fn request_activity(&mut self, repo_path: PathBuf, days: usize, ctx: egui::Context) {
        let seq = bump_seq(&self.seqs.activity);
        self.activity_loading = true;
        self.jobs.push(Job::LoadActivity {
            seq,
            repo_path,
            days,
            ctx,
        });
    }

    /// Fetch stat + changed files for the selected commit on a worker thread
    /// (synchronous git here froze the UI, especially on SSH repositories)
    pub(crate) fn request_commit_detail(
        &mut self,
        repo_path: PathBuf,
        hash: String,
        ctx: egui::Context,
    ) {
        let seq = bump_seq(&self.seqs.commit_detail);
        self.commit_detail_loading = true;
        self.selected_commit_stat = None;
        self.selected_commit_files = None;
        self.commit_files_tree = None;
        self.commit_tree_collapsed.clear();
        self.jobs.push(Job::LoadCommitDetail {
            seq,
            repo_path,
            hash,
            ctx,
        });
    }

    /// Fetch the commit graph for a branch in the background (current
    /// graph_log_limit lines). Stale results are dropped via seq.
    pub(crate) fn request_branch_log(
        &mut self,
        repo_path: PathBuf,
        branch: String,
        ctx: egui::Context,
    ) {
        let seq = bump_seq(&self.seqs.branch_log);
        self.branch_log_loading = true;
        self.jobs.push(Job::LoadBranchLog {
            seq,
            repo_path,
            branch,
            limit: self.graph_log_limit,
            ctx,
        });
    }

    fn load_repo_async(&mut self, index: usize, ctx: egui::Context) {
        self.loading_repos.insert(index);
        self.session_refreshed.insert(index);
        let path = self.repositories[index].path.clone();
        let members = self.members.clone();

        self.jobs.push(Job::LoadRepo {
            index,
            path,
            members,
            ctx,
        });
    }

    pub(crate) fn push_toast(&mut self, message: impl Into<String>, kind: ToastKind) {
        self.toasts.push(Toast {
            message: message.into(),
            kind,
            started_at: std::time::Instant::now(),
        });
    }

    // Process incoming async messages from worker threads
    fn handle_async_messages(&mut self, ctx: &egui::Context) {
        let lang = self.prefs.language;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AsyncMessage::RepoLoaded {
                    repo_index,
                    data,
                    warnings,
                } => {
                    match (*data, self.repo_cache.get(&repo_index)) {
                        // A failed refresh must not wipe usable cached data
                        // (e.g. transient git error while stale results are shown)
                        (Err(err), Some(Ok(_))) => {
                            let msg = match lang {
                                crate::config::Language::English => format!("Update failed: {err}"),
                                crate::config::Language::Japanese => format!("更新失敗: {err}"),
                            };
                            self.push_toast(msg, ToastKind::Error);
                        }
                        (result, _) => {
                            self.repo_cache.insert(repo_index, result);
                            self.repo_cache_gen += 1;
                        }
                    }
                    if !warnings.is_empty() {
                        let more = if warnings.len() > 1 {
                            match lang {
                                crate::config::Language::English => {
                                    format!(" ({} more)", warnings.len() - 1)
                                }
                                crate::config::Language::Japanese => {
                                    format!("（他{}件）", warnings.len() - 1)
                                }
                            }
                        } else {
                            String::new()
                        };
                        let msg = match lang {
                            crate::config::Language::English => {
                                format!("Partial analysis failed: {}{}", warnings[0], more)
                            }
                            crate::config::Language::Japanese => {
                                format!("一部の解析に失敗: {}{}", warnings[0], more)
                            }
                        };
                        self.push_toast(msg, ToastKind::Error);
                    }
                    self.loading_repos.remove(&repo_index);
                }
                AsyncMessage::PullFinished { path, result } => {
                    let is_ok = result.is_ok();
                    if let Err(ref err) = result {
                        let msg = match lang {
                            crate::config::Language::English => format!("Pull failed: {err}"),
                            crate::config::Language::Japanese => format!("Pull 失敗: {err}"),
                        };
                        self.push_toast(msg, ToastKind::Error);
                    }
                    let state = match result {
                        Ok(msg) => PullState::Success(msg),
                        Err(err) => PullState::Error(err),
                    };
                    self.pull_statuses.insert(path.clone(), state);

                    if is_ok
                        && let Some(idx) = self.repositories.iter().position(|r| r.path == path)
                    {
                        // Keep stale data visible; the reload replaces it when done
                        self.load_repo_async(idx, ctx.clone());
                    }
                }
                AsyncMessage::FetchFinished { path, result } => {
                    let is_ok = result.is_ok();
                    if let Err(ref err) = result {
                        let msg = match lang {
                            crate::config::Language::English => format!("Fetch failed: {err}"),
                            crate::config::Language::Japanese => format!("Fetch 失敗: {err}"),
                        };
                        self.push_toast(msg, ToastKind::Error);
                    }
                    let state = match result {
                        Ok(msg) => PullState::Success(msg),
                        Err(err) => PullState::Error(err),
                    };
                    self.pull_statuses.insert(path.clone(), state);

                    if is_ok
                        && let Some(idx) = self.repositories.iter().position(|r| r.path == path)
                    {
                        // Keep stale data visible; the reload replaces it when done
                        self.load_repo_async(idx, ctx.clone());
                    }
                }
                AsyncMessage::ChangedFilesLoaded { seq, result } => {
                    if seq == cur_seq(&self.seqs.files) {
                        self.diff_changed_files = Some(result);
                        self.diff_tree_cache = None;
                        self.diff_loading_files = false;
                    }
                }
                AsyncMessage::FileDiffLoaded { seq, result } => {
                    if seq == cur_seq(&self.seqs.content) {
                        // Widest row in monospace columns (ASCII=1, CJK=2),
                        // computed once here instead of every frame
                        self.diff_max_cols = result.as_ref().ok().map_or(0, |fd| {
                            fd.rows
                                .iter()
                                .map(|row| {
                                    let l = row.left_text.as_deref().map_or(0, diff::str_cols);
                                    let r = row.right_text.as_deref().map_or(0, diff::str_cols);
                                    l.max(r)
                                })
                                .max()
                                .unwrap_or(0)
                        });
                        self.diff_file_content = Some(result.map(std::sync::Arc::new));
                        self.diff_loading_content = false;
                        self.diff_hunk_pos = 0;
                        self.diff_selected_line_idx = None;
                        self.diff_emph_cache.clear();
                    }
                }
                AsyncMessage::FileBlameLoaded { seq, result } => {
                    if seq == cur_seq(&self.seqs.blame) {
                        self.diff_blame_content = Some(result);
                        self.diff_loading_blame = false;
                    }
                }
                AsyncMessage::CommitDetailLoaded { seq, stat, files } => {
                    if seq == cur_seq(&self.seqs.commit_detail) {
                        match stat {
                            Ok(s) => self.selected_commit_stat = Some(s),
                            Err(err) => {
                                let msg = match lang {
                                    crate::config::Language::English => {
                                        format!("Failed to fetch commit details: {err}")
                                    }
                                    crate::config::Language::Japanese => {
                                        format!("コミット詳細取得失敗: {err}")
                                    }
                                };
                                self.push_toast(msg, ToastKind::Error);
                            }
                        }
                        // A files error is non-fatal: the message text still
                        // shows. The tree is built once here, not per frame.
                        self.commit_files_tree =
                            files.as_ref().ok().filter(|f| !f.is_empty()).map(|f| {
                                let mut tree = diff::build_diff_tree(f);
                                diff::compress_diff_tree(&mut tree);
                                diff::collapse_top_dirs_if_large(
                                    &tree,
                                    f.len(),
                                    &mut self.commit_tree_collapsed,
                                );
                                tree
                            });
                        self.selected_commit_files = files.ok();
                        self.commit_detail_loading = false;
                    }
                }
                AsyncMessage::ActivityLoaded { seq, result } => {
                    if seq == cur_seq(&self.seqs.activity) {
                        match result {
                            Ok(activity) => self.activity_override = Some(activity),
                            Err(err) => {
                                let msg = match lang {
                                    crate::config::Language::English => {
                                        format!("Failed to fetch activity: {err}")
                                    }
                                    crate::config::Language::Japanese => {
                                        format!("アクティビティ取得失敗: {err}")
                                    }
                                };
                                self.push_toast(msg, ToastKind::Error);
                            }
                        }
                        self.activity_loading = false;
                    }
                }
                AsyncMessage::DiffCommitsLoaded { seq, result } => {
                    if seq == cur_seq(&self.seqs.diff_commits) {
                        match result {
                            Ok(list) => self.diff_commit_list = list,
                            Err(err) => {
                                let msg = match lang {
                                    crate::config::Language::English => {
                                        format!("Failed to fetch commit list: {err}")
                                    }
                                    crate::config::Language::Japanese => {
                                        format!("コミット一覧の取得失敗: {err}")
                                    }
                                };
                                self.push_toast(msg, ToastKind::Error);
                            }
                        }
                        self.diff_commits_loading = false;
                    }
                }
                AsyncMessage::StashOpFinished {
                    repo_idx,
                    action,
                    result,
                } => {
                    match (action, &result) {
                        (StashAction::Apply, Ok(())) => {
                            self.push_toast(
                                crate::i18n::t(lang, "toast_stash_applied"),
                                ToastKind::Success,
                            );
                        }
                        (StashAction::Drop, Ok(())) => {
                            self.push_toast(
                                crate::i18n::t(lang, "toast_stash_dropped"),
                                ToastKind::Success,
                            );
                        }
                        (StashAction::Apply, Err(e)) => {
                            let msg = match lang {
                                crate::config::Language::English => {
                                    format!("Failed to apply stash: {e}")
                                }
                                crate::config::Language::Japanese => {
                                    format!("スタッシュ適用失敗: {e}")
                                }
                            };
                            self.push_toast(msg, ToastKind::Error);
                        }
                        (StashAction::Drop, Err(e)) => {
                            let msg = match lang {
                                crate::config::Language::English => {
                                    format!("Failed to drop stash: {e}")
                                }
                                crate::config::Language::Japanese => {
                                    format!("スタッシュ削除失敗: {e}")
                                }
                            };
                            self.push_toast(msg, ToastKind::Error);
                        }
                    }
                    // Reload even on failure (apply may leave partial changes);
                    // stale data stays visible while the refresh runs
                    if repo_idx < self.repositories.len() {
                        self.load_repo_async(repo_idx, ctx.clone());
                    }
                }
                AsyncMessage::WorkerPanicked { detail } => {
                    let msg = match lang {
                        crate::config::Language::English => {
                            format!("Internal error occurred (continuing execution): {detail}")
                        }
                        crate::config::Language::Japanese => {
                            format!("内部エラーが発生しました（処理は継続します）: {detail}")
                        }
                    };
                    self.push_toast(msg, ToastKind::Error);
                }
                AsyncMessage::BranchLogLoaded { seq, result } => {
                    if seq == cur_seq(&self.seqs.branch_log) {
                        match result {
                            Ok(log) => self.branch_oneline_log = Some(log),
                            Err(err) => {
                                // Keep whatever is displayed; just report the failure
                                let msg = match lang {
                                    crate::config::Language::English => {
                                        format!("Failed to fetch commit graph: {err}")
                                    }
                                    crate::config::Language::Japanese => {
                                        format!("コミットグラフ取得失敗: {err}")
                                    }
                                };
                                self.push_toast(msg, ToastKind::Error);
                            }
                        }
                        self.branch_log_loading = false;
                    }
                }
            }
        }
    }

    /// Open the diff view for a single commit (called from commit log/graph hash clicks)
    pub(crate) fn open_diff_single(
        &mut self,
        repo_idx: usize,
        target_hash: String,
        ctx: egui::Context,
    ) {
        self.open_diff(repo_idx, None, target_hash, ctx);
    }

    /// Open the diff view for a base..target range (commits or tags)
    pub(crate) fn open_diff_range(
        &mut self,
        repo_idx: usize,
        base: String,
        target: String,
        ctx: egui::Context,
    ) {
        self.open_diff(repo_idx, Some(base), target, ctx);
    }

    fn open_diff(
        &mut self,
        repo_idx: usize,
        base: Option<String>,
        target: String,
        ctx: egui::Context,
    ) {
        let repo_path = self.repositories[repo_idx].path.clone();
        let repo_changed = self.diff_repo_idx != Some(repo_idx);

        self.viewing_diff = true;
        self.diff_repo_idx = Some(repo_idx);
        self.diff_mode = if base.is_some() {
            DiffMode::Range
        } else {
            DiffMode::Single
        };
        self.diff_base = base.clone();
        self.diff_target = Some(target.clone());
        self.diff_changed_files = None;
        self.diff_tree_cache = None;
        self.diff_selected_file = None;
        self.diff_file_content = None;
        self.diff_loading_files = true;
        self.diff_loading_content = false;
        let files_seq = bump_seq(&self.seqs.files);
        bump_seq(&self.seqs.content); // invalidate any in-flight file diff
        bump_seq(&self.seqs.blame);

        if let Some(b) = base.clone() {
            self.record_recent_compare(&repo_path, &b, &target);
        }

        // Commit list for the pickers, fetched on a worker thread (skip when
        // already populated for this repository)
        if self.diff_commit_list.is_empty() || repo_changed {
            self.diff_commit_list = Vec::new();
            let commits_seq = bump_seq(&self.seqs.diff_commits);
            self.diff_commits_loading = true;
            self.jobs.push(Job::LoadDiffCommits {
                seq: commits_seq,
                repo_path: repo_path.clone(),
                ctx: ctx.clone(),
            });
        }

        self.jobs.push(Job::LoadChangedFiles {
            seq: files_seq,
            repo_path,
            base,
            target,
            three_dot: self.effective_three_dot(),
            ctx,
        });
    }

    /// merge-base (three-dot) comparison applies only in range mode
    fn effective_three_dot(&self) -> bool {
        self.diff_mode == DiffMode::Range && self.diff_three_dot
    }

    /// Remember a range comparison (newest first, deduped, capped at 5 per
    /// repository / 50 overall) so it can be re-applied from the history menu.
    fn record_recent_compare(&mut self, repo_path: &std::path::Path, base: &str, target: &str) {
        if target == "WORKING_TREE" {
            return;
        }
        let rp = repo_path.to_string_lossy().to_string();
        let already_first = self
            .prefs
            .recent_compares
            .first()
            .is_some_and(|r| r.repo_path == rp && r.base == base && r.target == target);
        if already_first {
            return; // unchanged → skip the prefs.json write
        }
        self.prefs
            .recent_compares
            .retain(|r| !(r.repo_path == rp && r.base == base && r.target == target));
        self.prefs.recent_compares.insert(
            0,
            config::RecentCompare {
                repo_path: rp.clone(),
                base: base.to_string(),
                target: target.to_string(),
            },
        );
        let mut kept_for_repo = 0;
        self.prefs.recent_compares.retain(|r| {
            if r.repo_path == rp {
                kept_for_repo += 1;
                kept_for_repo <= 5
            } else {
                true
            }
        });
        self.prefs.recent_compares.truncate(50);
        let _ = config::save_preferences(&self.prefs);
    }

    /// Reload the changed-files list for the current diff view
    pub(crate) fn reload_diff_files(&mut self, ctx: egui::Context) {
        self.diff_collapsed_dirs.clear();
        let (Some(repo_idx), Some(target)) = (self.diff_repo_idx, self.diff_target.clone()) else {
            return;
        };
        let repo_path = self.repositories[repo_idx].path.clone();
        let base = if self.diff_mode == DiffMode::Range {
            self.diff_base.clone()
        } else {
            None
        };
        if let Some(b) = base.clone() {
            self.record_recent_compare(&repo_path, &b, &target);
        }
        self.diff_changed_files = None;
        self.diff_tree_cache = None;
        self.diff_selected_file = None;
        self.diff_file_content = None;
        self.diff_loading_files = true;
        self.diff_loading_content = false;
        let files_seq = bump_seq(&self.seqs.files);
        bump_seq(&self.seqs.content); // invalidate any in-flight file diff
        bump_seq(&self.seqs.blame);
        self.jobs.push(Job::LoadChangedFiles {
            seq: files_seq,
            repo_path,
            base,
            target,
            three_dot: self.effective_three_dot(),
            ctx,
        });
    }

    fn load_file_diff(&mut self, file: String, ctx: egui::Context) {
        let (Some(repo_idx), Some(target)) = (self.diff_repo_idx, self.diff_target.clone()) else {
            return;
        };
        let repo_path = self.repositories[repo_idx].path.clone();
        let base = if self.diff_mode == DiffMode::Range {
            self.diff_base.clone()
        } else {
            None
        };
        self.diff_selected_file = Some(file.clone());
        self.diff_file_content = None;
        self.diff_loading_content = true;
        let seq = bump_seq(&self.seqs.content);
        self.jobs.push(Job::LoadFileDiff {
            seq,
            repo_path,
            base,
            target,
            file,
            ignore_whitespace: self.diff_ignore_whitespace,
            full: self.diff_full_file,
            three_dot: self.effective_three_dot(),
            ctx: ctx.clone(),
        });
        if self.diff_show_blame {
            self.load_file_blame(ctx);
        }
    }

    pub(crate) fn load_file_blame(&mut self, ctx: egui::Context) {
        let (Some(repo_idx), Some(target), Some(file)) = (
            self.diff_repo_idx,
            self.diff_target.clone(),
            self.diff_selected_file.clone(),
        ) else {
            return;
        };
        let repo_path = self.repositories[repo_idx].path.clone();
        let blame_ref = if target == "WORKING_TREE" {
            "HEAD".to_string()
        } else if self.diff_mode == DiffMode::Range {
            self.diff_base
                .clone()
                .unwrap_or_else(|| format!("{}^", target))
        } else {
            format!("{}^", target)
        };

        self.diff_blame_content = None;
        self.diff_loading_blame = true;
        let seq = bump_seq(&self.seqs.blame);
        self.jobs.push(Job::LoadFileBlame {
            seq,
            repo_path,
            blame_ref,
            file,
            ctx,
        });
    }

    fn run_bulk_pull(&mut self, ctx: egui::Context) {
        for (idx, repo) in self.repositories.iter().enumerate() {
            if let Some(Ok(data)) = self.repo_cache.get(&idx)
                && !data.summary.has_remote
            {
                continue;
            }
            let path = repo.path.clone();
            self.pull_statuses.insert(path.clone(), PullState::Pulling);

            self.jobs.push(Job::PullRepo {
                path,
                lang: self.prefs.language,
                ctx: ctx.clone(),
            });
        }
    }

    fn run_bulk_fetch(&mut self, ctx: egui::Context) {
        for (idx, repo) in self.repositories.iter().enumerate() {
            if let Some(Ok(data)) = self.repo_cache.get(&idx)
                && !data.summary.has_remote
            {
                continue;
            }
            let path = repo.path.clone();
            self.pull_statuses.insert(path.clone(), PullState::Pulling);

            self.jobs.push(Job::FetchRepo {
                path,
                lang: self.prefs.language,
                ctx: ctx.clone(),
            });
        }
    }

    pub(crate) fn open_in_editor(&self, path: &std::path::Path) -> Option<String> {
        let lang = self.prefs.language;
        let editor = &self.prefs.editor_command;
        if git::parse_ssh_repo(path).is_some() {
            return Some(crate::i18n::t(lang, "editor_err_ssh").to_string());
        }
        if editor.trim().is_empty() {
            return Some(crate::i18n::t(lang, "editor_err_not_set").to_string());
        }
        let parts: Vec<&str> = editor.split_whitespace().collect();
        if parts.is_empty() {
            return Some(crate::i18n::t(lang, "editor_err_empty").to_string());
        }
        // quiet_command: editor shims like "code" are .cmd scripts on Windows
        // and would flash a console window otherwise
        let mut cmd = git::quiet_command(parts[0]);
        for arg in &parts[1..] {
            cmd.arg(arg);
        }
        cmd.arg(path);
        if let Err(e) = cmd.spawn() {
            let msg = match lang {
                crate::config::Language::English => {
                    format!("Failed to launch editor: {e} (Command: {editor})")
                }
                crate::config::Language::Japanese => {
                    format!("エディタ起動失敗: {e} (コマンド: {editor})")
                }
            };
            return Some(msg);
        }
        None
    }
}

impl eframe::App for GitDashboardApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe 0.34 requires `ui()` (the given Ui has no margin/background);
        // the existing body builds its own panels straight off `ctx`, so we
        // just recover it and keep the untouched logic below.
        let ctx = &ui.ctx().clone();
        self.handle_async_messages(ctx);
        self.ensure_theme_applied(ctx);
        let t = self.theme;
        let lang = self.prefs.language;

        // Command palette (Ctrl/Cmd+K): a global shortcut, reachable even
        // while some other text field has focus — matching orapli
        // design-system.md §6.
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::K)) {
            self.command_palette_open = !self.command_palette_open;
            self.command_palette_query.clear();
            self.command_palette_selected = 0;
        }

        // Esc: step back through screen stack (ZED/VSCode style)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.command_palette_open {
                self.command_palette_open = false;
            } else if self.diff_search_visible {
                self.diff_search_visible = false;
            } else if self.diff_selected_line_idx.is_some() {
                self.diff_selected_line_idx = None;
            } else if self.viewing_diff {
                self.viewing_diff = false;
            } else if self.viewing_settings {
                self.viewing_settings = false;
            } else if self.selected_repo_index.is_some() {
                self.selected_repo_index = None;
            }
        }

        // Skip shortcuts when a text field has focus
        if !ctx.egui_wants_keyboard_input() {
            // Dashboard tab switching: Ctrl/Cmd + 1..5
            if self.selected_repo_index.is_some()
                && !self.viewing_diff
                && !self.viewing_settings
                && let Some(tab) = ctx.input(|i| {
                    if !i.modifiers.command {
                        return None;
                    }
                    if i.key_pressed(egui::Key::Num1) {
                        Some(0usize)
                    } else if i.key_pressed(egui::Key::Num2) {
                        Some(1)
                    } else if i.key_pressed(egui::Key::Num3) {
                        Some(2)
                    } else if i.key_pressed(egui::Key::Num4) {
                        Some(3)
                    } else if i.key_pressed(egui::Key::Num5) {
                        Some(4)
                    } else {
                        None
                    }
                })
            {
                self.dashboard_tab = tab;
            }

            // Diff hunk navigation: n=next hunk, p=prev hunk
            if self.viewing_diff {
                let (nav_next, nav_prev) =
                    ctx.input(|i| (i.key_pressed(egui::Key::N), i.key_pressed(egui::Key::P)));
                if nav_next || nav_prev {
                    let hunk_starts: Vec<usize> = if let Some(Ok(ref fd)) = self.diff_file_content {
                        fd.rows
                            .iter()
                            .enumerate()
                            .filter(|(i, row)| {
                                !matches!(row.kind, git::DiffRowKind::Context)
                                    && (*i == 0
                                        || matches!(fd.rows[i - 1].kind, git::DiffRowKind::Context))
                            })
                            .map(|(i, _)| i)
                            .collect()
                    } else {
                        Vec::new()
                    };
                    if !hunk_starts.is_empty() {
                        let cur = self.diff_hunk_pos;
                        let target = if nav_next {
                            hunk_starts
                                .iter()
                                .find(|&&h| h > cur)
                                .copied()
                                .unwrap_or(hunk_starts[0])
                        } else {
                            hunk_starts
                                .iter()
                                .rev()
                                .find(|&&h| h < cur)
                                .copied()
                                .unwrap_or(*hunk_starts.last().unwrap())
                        };
                        self.diff_hunk_pos = target;
                        self.diff_search_scroll_to = Some(target);
                    }
                }

                // [ / ]: previous / next changed file (wraps around)
                let (file_prev, file_next) = ctx.input(|i| {
                    (
                        i.key_pressed(egui::Key::OpenBracket),
                        i.key_pressed(egui::Key::CloseBracket),
                    )
                });
                if (file_prev || file_next)
                    && let Some(Ok(files)) = self.diff_changed_files.as_ref()
                    && !files.is_empty()
                {
                    let cur = self
                        .diff_selected_file
                        .as_deref()
                        .and_then(|p| files.iter().position(|f| f.path == p));
                    let idx = match (cur, file_next) {
                        (Some(i), true) => (i + 1) % files.len(),
                        (Some(i), false) => (i + files.len() - 1) % files.len(),
                        (None, _) => 0,
                    };
                    let path = files[idx].path.clone();
                    self.diff_full_file = false;
                    self.load_file_diff(path, ctx.clone());
                }
            }
        }

        // Left sidebar panel. Collapsed and expanded states use different panel
        // ids: egui remembers each panel's width by id, so sharing one id let the
        // collapsed 52px clobber the remembered expanded width (and prefs with it).
        let sidebar_panel = if self.prefs.sidebar_collapsed {
            egui::Panel::left("sidebar_collapsed")
                .resizable(false)
                .exact_size(52.0)
        } else {
            egui::Panel::left("sidebar")
                .resizable(true)
                .min_size(180.0)
                .default_size(self.prefs.sidebar_width.clamp(180.0, 400.0))
                .max_size(400.0)
        };
        sidebar_panel
            .frame(egui::Frame::NONE.fill(t.bg_sidebar))
            .show(ui, |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(t.bg_sidebar))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.add_space(20.0);
                            // Logo area
                            if self.prefs.sidebar_collapsed {
                                ui.horizontal(|ui| {
                                    ui.add_space(15.0); // center 22px icon in 52px rail
                                    if widgets::icon_button(
                                        ui,
                                        "expand",
                                        crate::i18n::t(lang, "palette_expand_sidebar"),
                                        &t,
                                    )
                                    .clicked()
                                    {
                                        self.prefs.sidebar_collapsed = false;
                                        let _ = crate::config::save_preferences(&self.prefs);
                                    }
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.add_space(15.0);
                                    let (rect, resp) = ui.allocate_exact_size(
                                        egui::vec2(24.0, 24.0),
                                        egui::Sense::click(),
                                    );
                                    let resp = resp
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text(crate::i18n::t(lang, "back_to_home"));
                                    if let Some(ref icon) = self.app_icon {
                                        ui.painter().image(
                                            icon.id(),
                                            rect,
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().rect_filled(rect, 4.0, t.accent);
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "G",
                                            egui::FontId::proportional(14.0),
                                            Color32::WHITE,
                                        );
                                    }
                                    if resp.clicked() {
                                        self.selected_repo_index = None;
                                        self.viewing_settings = false;
                                        self.viewing_diff = false;
                                    }
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new("Git Dashboard")
                                            .strong()
                                            .size(16.0)
                                            .color(t.text),
                                    );

                                    // Collapse toggle (« at the header's right edge)
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(SIDEBAR_PADDING);
                                            if widgets::icon_button(
                                                ui,
                                                "collapse",
                                                crate::i18n::t(lang, "palette_collapse_sidebar"),
                                                &t,
                                            )
                                            .clicked()
                                            {
                                                self.prefs.sidebar_collapsed = true;
                                                let _ =
                                                    crate::config::save_preferences(&self.prefs);
                                            }
                                        },
                                    );
                                });
                            }

                            ui.add_space(20.0);
                            draw_indented_separator(ui, SIDEBAR_PADDING, t.border);
                            ui.add_space(10.0);

                            // Home (all repos) button
                            if self.prefs.sidebar_collapsed {
                                ui.horizontal(|ui| {
                                    ui.add_space(12.0);
                                    let is_home_selected = self.selected_repo_index.is_none()
                                        && !self.viewing_settings
                                        && !self.viewing_diff;
                                    if draw_selectable_icon(
                                        ui,
                                        is_home_selected,
                                        &t,
                                        draw_home_icon,
                                    )
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::t(lang, "home"))
                                    .clicked()
                                    {
                                        self.selected_repo_index = None;
                                        self.viewing_settings = false;
                                        self.viewing_diff = false;
                                    }
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.add_space(SIDEBAR_PADDING);
                                    let is_home_selected = self.selected_repo_index.is_none()
                                        && !self.viewing_settings
                                        && !self.viewing_diff;
                                    if ui
                                        .selectable_label(
                                            is_home_selected,
                                            crate::i18n::t(lang, "home"),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        self.selected_repo_index = None;
                                        self.viewing_settings = false;
                                        self.viewing_diff = false;
                                    }
                                });
                            }

                            ui.add_space(SIDEBAR_PADDING);
                            if !self.prefs.sidebar_collapsed {
                                ui.horizontal(|ui| {
                                    ui.add_space(SIDEBAR_PADDING);
                                    ui.label(
                                        egui::RichText::new("REPOSITORIES")
                                            .size(10.0)
                                            .color(t.text_faint),
                                    );
                                });
                                ui.add_space(3.0);

                                // Quick filter and sort cycle for sidebar repo list
                                ui.horizontal(|ui| {
                                    ui.add_space(SIDEBAR_PADDING);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.sidebar_search_query)
                                            .hint_text(crate::i18n::t(lang, "filter_placeholder"))
                                            .desired_width(158.0),
                                    );
                                    egui::ComboBox::from_id_salt("sidebar_sort_combobox")
                                        .selected_text("⇅")
                                        .width(28.0)
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
                                });
                                ui.add_space(8.0);
                            }

                            // Repository list (full-row click support). The settings section
                            // lives in a separate bottom panel, so this can scroll freely.
                            let mut clicked_repo_idx = None;
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                let sorted_items = {
                                    let input: Vec<home::RepoFilterSortInput> = self
                                        .repositories
                                        .iter()
                                        .enumerate()
                                        .map(|(idx, repo)| {
                                            let last_commit_date =
                                                if let Some(Ok(data)) = self.repo_cache.get(&idx) {
                                                    data.recent_commits
                                                        .first()
                                                        .map(|c| c.date.clone())
                                                        .unwrap_or_default()
                                                } else {
                                                    String::new()
                                                };
                                            home::RepoFilterSortInput {
                                                index: idx,
                                                name: repo.name.clone(),
                                                path: repo.path.to_string_lossy().to_string(),
                                                last_commit_date,
                                                language: String::new(),
                                                framework: String::new(),
                                            }
                                        })
                                        .collect();
                                    home::filter_and_sort_repositories(
                                        input,
                                        &self.sidebar_search_query,
                                        self.repo_sort_by,
                                    )
                                };

                                for item in &sorted_items {
                                    let idx = item.index;
                                    let name = &item.name;
                                    let path = std::path::PathBuf::from(&item.path);

                                    let is_selected = self.selected_repo_index == Some(idx)
                                        && !self.viewing_settings
                                        && !self.viewing_diff;

                                    let cache_entry = self.repo_cache.get(&idx);
                                    let (behind, has_remote) = if let Some(Ok(data)) = cache_entry {
                                        (data.summary.behind, data.summary.has_remote)
                                    } else {
                                        (0, true)
                                    };

                                    let dot_color = match self.pull_statuses.get(&path) {
                                        Some(PullState::Pulling) => t.accent,
                                        Some(PullState::Success(_)) => t.success,
                                        Some(PullState::Error(_)) => t.error,
                                        _ => {
                                            if !has_remote {
                                                t.text_faint
                                            } else if behind > 0 {
                                                t.warning
                                            } else {
                                                t.text_dim
                                            }
                                        }
                                    };

                                    let available_w = ui.available_width();
                                    let (rect, resp) = ui.allocate_exact_size(
                                        egui::vec2(available_w, 22.0),
                                        egui::Sense::click(),
                                    );
                                    let mut resp =
                                        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                                    if self.prefs.sidebar_collapsed {
                                        if !has_remote {
                                            let tip = match lang {
                                                crate::config::Language::English => {
                                                    format!("{} (Local only)", name)
                                                }
                                                crate::config::Language::Japanese => {
                                                    format!("{} (ローカルのみ)", name)
                                                }
                                            };
                                            resp = resp.on_hover_text(tip);
                                        } else if behind > 0 {
                                            let tip = match lang {
                                                crate::config::Language::English => format!(
                                                    "{} (Updates needed: {} items)",
                                                    name, behind
                                                ),
                                                crate::config::Language::Japanese => {
                                                    format!("{} (要更新: {}件)", name, behind)
                                                }
                                            };
                                            resp = resp.on_hover_text(tip);
                                        } else {
                                            resp = resp.on_hover_text(name.as_str());
                                        }
                                    }

                                    // Drawing bounds (full row when collapsed, indented when expanded)
                                    let draw_rect = if self.prefs.sidebar_collapsed {
                                        rect
                                    } else {
                                        egui::Rect::from_min_max(
                                            egui::pos2(rect.left() + 15.0, rect.top()),
                                            egui::pos2(rect.right() - 15.0, rect.bottom()),
                                        )
                                    };

                                    // Background (rounding on indented area; flat when collapsed)
                                    let bg = if is_selected {
                                        t.bg_active
                                    } else if resp.hovered() {
                                        t.bg_hover
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    if bg != Color32::TRANSPARENT {
                                        let rounding = if self.prefs.sidebar_collapsed {
                                            0.0
                                        } else {
                                            4.0
                                        };
                                        ui.painter().rect_filled(draw_rect, rounding, bg);
                                    }
                                    // Left-edge accent bar on selection (left edge of indented area)
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_max(
                                                draw_rect.left_top(),
                                                egui::pos2(
                                                    draw_rect.left() + 2.0,
                                                    draw_rect.bottom(),
                                                ),
                                            ),
                                            0.0,
                                            t.accent,
                                        );
                                    }

                                    let painter = ui.painter_at(rect);
                                    let cy = rect.center().y;
                                    if self.prefs.sidebar_collapsed {
                                        // Show only the status dot when collapsed
                                        painter.circle_filled(
                                            egui::pos2(rect.left() + 26.0, cy),
                                            4.0,
                                            dot_color,
                                        );
                                    } else {
                                        // Status dot (relative to indented area)
                                        painter.circle_filled(
                                            egui::pos2(draw_rect.left() + 10.0, cy),
                                            3.0,
                                            dot_color,
                                        );
                                        // Repository name
                                        let text_color =
                                            if is_selected { t.text } else { t.text_dim };
                                        painter.text(
                                            egui::pos2(draw_rect.left() + 22.0, cy),
                                            egui::Align2::LEFT_CENTER,
                                            name.as_str(),
                                            egui::FontId::proportional(12.0),
                                            text_color,
                                        );

                                        // "behind" badge (relative to indented area)
                                        if behind > 0 {
                                            let badge_text = behind.to_string();
                                            let badge_galley = painter.layout_no_wrap(
                                                badge_text,
                                                egui::FontId::proportional(9.0),
                                                t.warning,
                                            );
                                            let badge_rect = egui::Rect::from_min_max(
                                                egui::pos2(
                                                    draw_rect.right()
                                                        - badge_galley.size().x
                                                        - 12.0,
                                                    cy - 6.0,
                                                ),
                                                egui::pos2(draw_rect.right() - 6.0, cy + 6.0),
                                            );
                                            painter.rect_filled(
                                                badge_rect,
                                                6.0,
                                                Color32::from_rgba_unmultiplied(
                                                    t.warning.r(),
                                                    t.warning.g(),
                                                    t.warning.b(),
                                                    25,
                                                ),
                                            );
                                            painter.galley(
                                                egui::pos2(badge_rect.left() + 4.0, cy - 5.0),
                                                badge_galley,
                                                t.text,
                                            );
                                        }
                                    }

                                    if resp.clicked() {
                                        clicked_repo_idx = Some(idx);
                                    }
                                }
                            });
                            if let Some(idx) = clicked_repo_idx {
                                self.select_repository(idx, ctx.clone());
                            }

                            // Persist sidebar width to preferences — only after the
                            // drag ends, otherwise prefs.json is written to disk on
                            // every frame while the user is resizing
                            let actual_width = ui.min_rect().right();
                            let pointer_down = ui.input(|i| i.pointer.any_down());
                            if !self.prefs.sidebar_collapsed
                                && !pointer_down
                                && (actual_width - self.prefs.sidebar_width).abs() > 1.0
                            {
                                self.prefs.sidebar_width = actual_width;
                                let _ = crate::config::save_preferences(&self.prefs);
                            }
                        });
                    });
            });

        // Right-side main content area
        let central_frame = egui::Frame::NONE.fill(t.bg).inner_margin(0.0);

        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ui, |ui| {
                // Top bar (breadcrumb + theme switcher)
                ui.horizontal(|ui| {
                    let style = ui.style_mut();
                    style
                        .text_styles
                        .insert(egui::TextStyle::Body, egui::FontId::proportional(12.0));
                    style
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(12.0));

                    ui.set_height(44.0);
                    ui.add_space(16.0);

                    // Left: breadcrumb navigation (each segment navigates on click)
                    ui.horizontal(|ui| {
                        // Home segment (click to go home)
                        let home_resp = ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(crate::i18n::t(lang, "home"))
                                        .size(12.0)
                                        .color(t.text_dim),
                                )
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if home_resp.clicked() {
                            self.selected_repo_index = None;
                            self.viewing_settings = false;
                            self.viewing_diff = false;
                        }
                        if let Some(idx) = self.selected_repo_index {
                            ui.label(egui::RichText::new(" /").size(11.0).color(t.text_faint));
                            let name = self.repositories[idx].name.clone();
                            // Repo name (click returns to dashboard when in diff view)
                            let crumb_active = self.viewing_diff;
                            let name_color = if crumb_active { t.text_dim } else { t.text };
                            let repo_resp = ui
                                .add(
                                    egui::Label::new(
                                        egui::RichText::new(&name).size(12.0).color(name_color),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if repo_resp.clicked() {
                                self.viewing_settings = false;
                                self.viewing_diff = false;
                            }
                            if self.viewing_diff {
                                ui.label(egui::RichText::new(" /").size(11.0).color(t.text_faint));
                                ui.label(
                                    egui::RichText::new(crate::i18n::t(lang, "diff"))
                                        .size(12.0)
                                        .color(t.text),
                                );
                            }
                        } else if self.viewing_settings {
                            ui.label(egui::RichText::new(" /").size(11.0).color(t.text_faint));
                            ui.label(
                                egui::RichText::new(crate::i18n::t(lang, "settings"))
                                    .size(12.0)
                                    .color(t.text),
                            );
                        }
                    });

                    // Right: theme switcher ComboBox
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);

                        // Settings entry point, per orapli design-system.md §6
                        // ("Settings via ⚙ icon (top right)"): always visible here,
                        // regardless of sidebar collapse state.
                        let is_settings_selected = self.viewing_settings;
                        if draw_selectable_icon(ui, is_settings_selected, &t, draw_settings_icon)
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(crate::i18n::t(lang, "settings"))
                            .clicked()
                        {
                            self.viewing_settings = true;
                            self.selected_repo_index = None;
                            self.viewing_diff = false;
                        }
                        ui.add_space(12.0);

                        let current_theme_name = self.prefs.theme.clone();
                        let mut selected_theme_name: Option<String> = None;
                        egui::ComboBox::from_id_salt("theme_dropdown")
                            .selected_text(&current_theme_name)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        current_theme_name == crate::theme::SYSTEM_THEME_NAME,
                                        crate::theme::SYSTEM_THEME_NAME,
                                    )
                                    .on_hover_text(crate::i18n::t(lang, "follow_os_theme_tooltip"))
                                    .clicked()
                                {
                                    selected_theme_name =
                                        Some(crate::theme::SYSTEM_THEME_NAME.to_string());
                                }
                                ui.separator();
                                for (name, _) in self.themes.iter() {
                                    if ui
                                        .selectable_label(name == &current_theme_name, name)
                                        .clicked()
                                    {
                                        selected_theme_name = Some(name.clone());
                                    }
                                }
                            });

                        if let Some(name) = selected_theme_name {
                            self.prefs.theme = name;
                            self.ensure_theme_applied(ctx);
                            let _ = crate::config::save_preferences(&self.prefs);
                        }

                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "theme_label"))
                                .size(12.0)
                                .color(t.text_dim),
                        );
                    });
                });

                ui.separator();

                // Main content area (inner margin 24.0)
                egui::Frame::NONE.inner_margin(24.0).show(ui, |ui| {
                    if self.viewing_diff {
                        self.draw_diff_view(ui, ctx);
                    } else if self.viewing_settings {
                        self.draw_settings_view(ui);
                    } else if let Some(idx) = self.selected_repo_index {
                        // Cached data (including the disk cache from a previous
                        // session) renders immediately; a background refresh may
                        // still be running and will replace it when done.
                        let cache_entry = self.repo_cache.get(&idx).cloned();
                        match cache_entry {
                            Some(Ok(data)) => {
                                self.draw_dashboard_view(ui, &data, idx);
                            }
                            _ if self.loading_repos.contains(&idx) => {
                                ui.centered_and_justified(|ui| {
                                    ui.spinner();
                                    ui.label(crate::i18n::t(lang, "analyzing_repo_data"));
                                });
                            }
                            Some(Err(err)) => {
                                let mut retry = false;
                                ui.centered_and_justified(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(crate::i18n::t(
                                                lang,
                                                "analysis_error",
                                            ))
                                            .color(t.error)
                                            .size(18.0),
                                        );
                                        ui.add_space(10.0);
                                        ui.label(&err);
                                        ui.add_space(10.0);
                                        if ui
                                            .button(crate::i18n::t(lang, "retry_btn"))
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            retry = true;
                                        }
                                    });
                                });
                                if retry {
                                    self.load_repo_async(idx, ui.ctx().clone());
                                }
                            }
                            None => {
                                ui.centered_and_justified(|ui| {
                                    ui.label(crate::i18n::t(lang, "loading"));
                                });
                            }
                        }
                    } else {
                        // Render the home view (all repos list)
                        self.draw_home_view(ui);
                    }
                });
            });

        // Toast notifications (Info=3s, Success=3s, Error=6s; manual close also available)
        {
            let now = std::time::Instant::now();
            self.toasts.retain(|toast| {
                let ttl = match toast.kind {
                    ToastKind::Error => 6.0,
                    _ => 3.0,
                };
                now.duration_since(toast.started_at).as_secs_f32() < ttl
            });
        }
        if !self.toasts.is_empty() {
            let mut close_idx: Option<usize> = None;
            egui::Window::new("__toasts__")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    ui.set_max_width(360.0);
                    for (i, toast) in self.toasts.iter().enumerate() {
                        let (badge_color, border_color) = match toast.kind {
                            ToastKind::Info => (t.accent, t.accent),
                            ToastKind::Success => (t.success, t.success),
                            ToastKind::Error => (t.error, t.error),
                        };
                        let frame = egui::Frame::NONE
                            .fill(t.bg_elevated)
                            .stroke(egui::Stroke::new(1.0, border_color))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(8, 6));
                        let mut close_this = false;
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("●").color(badge_color));
                                ui.label(egui::RichText::new(&toast.message).color(t.text));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .button(egui::RichText::new("×").color(t.text_dim))
                                            .clicked()
                                        {
                                            close_this = true;
                                        }
                                    },
                                );
                            });
                        });
                        if close_this {
                            close_idx = Some(i);
                        }
                        ui.add_space(4.0);
                    }
                });
            if let Some(i) = close_idx {
                self.toasts.remove(i);
            }
            // TTL expiry only needs ~10fps, not a full-speed repaint loop
            // for the whole duration a toast is visible
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // Stash drop confirmation dialog
        if let Some((confirm_repo_idx, ref_name)) = &self.confirm_drop_stash.clone() {
            let mut close = false;
            let mut confirmed = false;
            egui::Window::new(crate::i18n::t(lang, "drop_stash_window_title"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ui, |ui| {
                    let confirm_msg = match lang {
                        crate::config::Language::English => {
                            format!("Drop stash '{ref_name}'?\nThis action cannot be undone.")
                        }
                        crate::config::Language::Japanese => format!(
                            "スタッシュ '{ref_name}' を削除しますか？\nこの操作は取り消せません。"
                        ),
                    };
                    ui.label(confirm_msg);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::t(lang, "cancel")).clicked() {
                            close = true;
                        }
                        if ui
                            .button(
                                egui::RichText::new(crate::i18n::t(lang, "delete_action"))
                                    .color(t.error),
                            )
                            .clicked()
                        {
                            confirmed = true;
                            close = true;
                        }
                    });
                });
            if confirmed {
                // Runs on a worker thread; the toast + reload happen on completion
                self.jobs.push(Job::StashOp {
                    repo_idx: *confirm_repo_idx,
                    repo_path: self.repositories[*confirm_repo_idx].path.clone(),
                    stash_ref: ref_name.clone(),
                    action: StashAction::Drop,
                    ctx: ctx.clone(),
                });
            }
            if close {
                self.confirm_drop_stash = None;
            }
        }

        // Command palette (Ctrl/Cmd+K)
        if self.command_palette_open {
            let commands = self.palette_commands();
            let query = self.command_palette_query.to_lowercase();
            let filtered: Vec<&(String, PaletteAction)> = commands
                .iter()
                .filter(|(label, _)| label.to_lowercase().contains(&query))
                .collect();
            if self.command_palette_selected >= filtered.len() {
                self.command_palette_selected = filtered.len().saturating_sub(1);
            }

            let up = ctx.input(|i| i.key_pressed(egui::Key::ArrowUp));
            let down = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown));
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            if down && !filtered.is_empty() {
                self.command_palette_selected =
                    (self.command_palette_selected + 1) % filtered.len();
            }
            if up && !filtered.is_empty() {
                self.command_palette_selected = self
                    .command_palette_selected
                    .checked_sub(1)
                    .unwrap_or(filtered.len() - 1);
            }

            let mut run_action: Option<PaletteAction> = None;
            let mut close = false;

            egui::Window::new(crate::i18n::t(self.prefs.language, "command_palette"))
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .fixed_size(egui::vec2(480.0, 0.0))
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
                .frame(
                    egui::Frame::NONE
                        .fill(t.bg_elevated)
                        .stroke(egui::Stroke::new(1.0, t.border))
                        .corner_radius(6.0)
                        .inner_margin(8.0),
                )
                .show(ui, |ui| {
                    let query_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.command_palette_query)
                            .hint_text(crate::i18n::t(self.prefs.language, "search_command_hint"))
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Heading),
                    );
                    if query_resp.changed() {
                        self.command_palette_selected = 0;
                    }
                    query_resp.request_focus();

                    ui.add_space(6.0);
                    ui.separator();

                    if filtered.is_empty() {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(crate::i18n::t(lang, "no_matching_commands"))
                                .color(t.text_faint),
                        );
                        ui.add_space(8.0);
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .show(ui, |ui| {
                                for (i, (label, action)) in filtered.iter().enumerate() {
                                    let selected = i == self.command_palette_selected;
                                    let resp = ui.selectable_label(selected, label.as_str());
                                    if selected {
                                        resp.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    if resp.clicked() {
                                        run_action = Some((*action).clone());
                                        close = true;
                                    }
                                }
                            });
                    }
                });

            if enter && !filtered.is_empty() {
                run_action = Some(filtered[self.command_palette_selected].1.clone());
                close = true;
            }
            if let Some(action) = run_action {
                self.execute_palette_action(&action, ctx);
            }
            if close {
                self.command_palette_open = false;
            }
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        // eframe handles viewport persistence internally when save is called.
    }
}

fn draw_selectable_icon<F>(
    ui: &mut egui::Ui,
    selected: bool,
    theme: &crate::theme::Theme,
    draw_icon: F,
) -> egui::Response
where
    F: FnOnce(&mut egui::Ui, egui::Color32, f32) -> egui::Response,
{
    let size = 28.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());

    // Background highlight matching egui standard
    let bg_fill = if selected {
        theme.bg_active
    } else if response.hovered() {
        theme.bg_hover
    } else {
        egui::Color32::TRANSPARENT
    };

    if bg_fill != egui::Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 4.0, bg_fill);
    }

    let icon_color = if selected { theme.text } else { theme.text_dim };

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
    ));
    draw_icon(&mut child_ui, icon_color, 16.0);

    response
}

fn draw_home_icon(ui: &mut egui::Ui, color: egui::Color32, size: f32) -> egui::Response {
    // hover() only, not click(): this is a paint-only helper invoked through
    // draw_selectable_icon's child Ui — its own allocate_exact_size(..click())
    // already owns the click sense for this rect. A second click-sensing
    // allocation here shadows it under egui 0.35's single-winner hit test
    // (each screen position resolves to exactly one `hits.click` widget),
    // which made the outer response's `.clicked()`/`.hovered()` never fire.
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        widgets::paint_icon_glyph(ui.painter(), rect, color, widgets::icons::HOME);
    }
    response
}

fn draw_settings_icon(ui: &mut egui::Ui, color: egui::Color32, size: f32) -> egui::Response {
    // hover() only — see draw_home_icon for why (avoids shadowing the outer
    // draw_selectable_icon response's click sense under egui 0.35).
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        widgets::paint_icon_glyph(ui.painter(), rect, color, widgets::icons::GEAR);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_queue_interactive_first() {
        let q = JobQueue::new();
        let ctx = egui::Context::default();
        // Background job enqueued first…
        q.push(Job::LoadRepo {
            index: 0,
            path: PathBuf::new(),
            members: Vec::new(),
            ctx: ctx.clone(),
        });
        // …but the interactive one must pop first
        q.push(Job::LoadBranchLog {
            seq: 1,
            repo_path: PathBuf::new(),
            branch: "main".to_string(),
            limit: 10,
            ctx: ctx.clone(),
        });
        assert!(!q.pop().is_background());
        assert!(q.pop().is_background());
    }
}
