use crate::config::{Language, Member};
use chrono::{Datelike, Duration, Local, TimeZone, Timelike, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Summary {
    pub repo_name: String,
    pub repo_path: String,
    pub current_branch: String,
    pub total_commits: usize,
    pub total_contributors: usize,
    pub total_branches: usize,
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub total_size_formatted: String,
    pub has_upstream: bool,
    pub ahead: usize,
    pub behind: usize,
    pub has_remote: bool,
    pub uncommitted_changes: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Contributor {
    pub name: String,
    pub email: String,
    pub commit_count: usize,
    pub percentage: f64,
    pub first_commit: String,
    pub last_commit: String,
    pub is_member: bool,
    pub is_active: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    pub author: String,
    pub date: String,
    pub date_unix: i64,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DailyActivity {
    pub dates: Vec<String>,
    pub counts: Vec<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Activity {
    pub daily: DailyActivity,
    pub hourly: Vec<usize>,
    pub weekly: Vec<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileExtInfo {
    pub ext: String,
    pub count: usize,
    pub percentage: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileInfo {
    pub path: String,
    pub size_bytes: u64,
    pub size_formatted: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FilesReport {
    pub extensions: Vec<FileExtInfo>,
    pub largest_files: Vec<FileInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StashEntry {
    pub ref_name: String,
    pub date_relative: String,
    pub author: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BlameEntry {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub summary: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommitSummary {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

fn check_safe_ref(r: &str) -> Result<(), String> {
    if r.starts_with('-') {
        return Err(format!("Git ref cannot start with '-': {}", r));
    }
    Ok(())
}

fn check_safe_ref_opt(r: Option<&str>) -> Result<(), String> {
    if let Some(s) = r {
        check_safe_ref(s)?;
    }
    Ok(())
}

/// Create a Command that never flashes a console window on Windows.
/// GUI-subsystem apps otherwise spawn a visible console for every child
/// process, which makes the screen flicker on each git invocation.
pub fn quiet_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Split an `ssh://user@host/abs/path` repository locator into (host, remote path).
/// Local paths return None. SSH config aliases work as the host part.
pub fn parse_ssh_repo(repo_path: &Path) -> Option<(String, String)> {
    let s = repo_path.to_string_lossy();
    let rest = s.strip_prefix("ssh://")?;
    let (host, path) = rest.split_once('/')?;
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some((
        host.to_string(),
        format!("/{}", path.trim_start_matches('/')),
    ))
}

/// Quote a string for the remote POSIX shell (ssh joins argv with spaces and
/// hands the result to a shell, so pretty-format strings etc. must be quoted).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build the git invocation for a repository: plain `git` with current_dir for
/// local paths, or `ssh <host> git -C <path> ...` for `ssh://` locators.
fn git_command_for(repo_path: &Path, args: &[&str]) -> Command {
    if let Some((host, remote_path)) = parse_ssh_repo(repo_path) {
        let mut cmd = quiet_command("ssh");
        cmd.arg("-o")
            .arg("BatchMode=yes") // never prompt for passwords (key auth only)
            .arg("-o")
            .arg("ConnectTimeout=8")
            .arg(host)
            .arg("git")
            .arg("-C")
            .arg(shell_quote(&remote_path));
        for a in args {
            cmd.arg(shell_quote(a));
        }
        cmd
    } else {
        let mut cmd = quiet_command("git");
        cmd.args(args)
            .current_dir(repo_path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            // Never let a credential helper pop an interactive dialog from a
            // worker thread (Git Credential Manager on Windows ignores
            // GIT_TERMINAL_PROMPT); failing fast beats hanging forever
            .env("GCM_INTERACTIVE", "never")
            .env("GIT_ASKPASS", "echo");
        cmd
    }
}

/// Hard timeout for analysis git invocations. ssh has ConnectTimeout, but an
/// established-yet-stalled session, a credential helper, or a hung network
/// filesystem would otherwise block a worker thread forever — a few of those
/// exhaust the whole pool and the app stops loading anything.
const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// pull/fetch transfer real data and get a more generous limit.
const GIT_NETWORK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Run a command with a hard timeout, killing the process on expiry.
/// stdout/stderr are drained on dedicated threads — waiting for exit before
/// reading would deadlock once a pipe buffer (64 KiB) fills on large output.
fn run_with_timeout(
    mut cmd: Command,
    timeout: std::time::Duration,
) -> Result<std::process::Output, String> {
    use std::io::Read;
    use std::process::Stdio;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("コマンドの起動に失敗しました: {e}"))?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let out_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut s) = stdout_pipe {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut s) = stderr_pipe {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });

    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out_thread.join();
                    let _ = err_thread.join();
                    return Err(format!(
                        "コマンドが{}秒でタイムアウトしました（リモートまたはファイルシステムが応答していません）",
                        timeout.as_secs()
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("child process wait failed: {e}"));
            }
        }
    };

    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

// Helper to run git commands
fn run_git_cmd(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = run_with_timeout(git_command_for(repo_path, args), GIT_TIMEOUT)?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Sentinel delimiting per-command output when several git commands share one
/// SSH connection. Distinctive enough not to collide with structured metadata
/// output (arbitrary file content is never batched).
const BATCH_SEP: &str = "___GITDASH_CMD_END_3f9a2c___";

/// Run several git commands against a repository, returning one result per
/// command in order. For **SSH** repositories all commands share a single ssh
/// connection: each `ssh` invocation is a full TCP + crypto handshake, and one
/// repository analysis otherwise opens a dozen of them (the dominant cost on
/// remote repos, and the only multiplexing option that works on Windows where
/// OpenSSH ControlMaster is unsupported). Local repositories have no handshake
/// cost, so they simply run sequentially.
///
/// A per-command non-zero exit maps to `Err(output)`, mirroring `run_git_cmd`.
fn run_git_batch(repo_path: &Path, commands: &[&[&str]]) -> Vec<Result<String, String>> {
    let Some((host, remote_path)) = parse_ssh_repo(repo_path) else {
        // Local: no connection to amortize; behave exactly like N run_git_cmd
        return commands.iter().map(|c| run_git_cmd(repo_path, c)).collect();
    };

    // Remote shell script: each command's merged stdout+stderr, then a sentinel
    // line carrying its exit code. printf's `\n` guarantees the sentinel starts
    // on its own line even when a command's output has no trailing newline.
    let qpath = shell_quote(&remote_path);
    let mut script = String::new();
    for cmd in commands {
        script.push_str("git -C ");
        script.push_str(&qpath);
        for a in *cmd {
            script.push(' ');
            script.push_str(&shell_quote(a));
        }
        script.push_str(" 2>&1; printf '\\n%s%d\\n' '");
        script.push_str(BATCH_SEP);
        script.push_str("' \"$?\"\n");
    }

    let mut ssh = quiet_command("ssh");
    ssh.arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=8")
        .arg(&host)
        .arg(&script);

    let timeout = (GIT_TIMEOUT * commands.len().max(1) as u32).min(GIT_NETWORK_TIMEOUT);
    match run_with_timeout(ssh, timeout) {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            split_batch_output(&stdout, BATCH_SEP, commands.len(), stderr.trim())
        }
        Err(e) => commands.iter().map(|_| Err(e.clone())).collect(),
    }
}

/// Split sentinel-delimited batch stdout into per-command results. Each command
/// emitted its output followed by a line `<sep><exit-code>`. Segments missing
/// their sentinel (e.g. the connection dropped mid-stream) become `Err`.
fn split_batch_output(
    stdout: &str,
    sep: &str,
    expected: usize,
    conn_err: &str,
) -> Vec<Result<String, String>> {
    let mut results = Vec::with_capacity(expected);
    let mut buf = String::new();
    for line in stdout.lines() {
        if let Some(code_str) = line.strip_prefix(sep) {
            let code: i32 = code_str.trim().parse().unwrap_or(-1);
            // Drop the trailing newline(s) printf/line-joining introduced;
            // callers trim or split, so exact trailing whitespace is moot.
            let content = buf.trim_end_matches('\n').to_string();
            buf.clear();
            if code == 0 {
                results.push(Ok(content));
            } else {
                results.push(Err(content));
            }
        } else {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    while results.len() < expected {
        results.push(Err(if conn_err.is_empty() {
            "SSH接続に失敗しました".to_string()
        } else {
            conn_err.to_string()
        }));
    }
    results.truncate(expected);
    results
}

/// Pure parser for sync status, shared by `get_sync_status` (one command at a
/// time) and `get_summary` (batched). `counts_out` is the tab-separated
/// `rev-list --left-right --count HEAD...@{u}` output ("ahead\tbehind").
fn parse_sync_status(has_upstream: bool, counts_out: &str) -> SyncStatus {
    if !has_upstream {
        return SyncStatus {
            has_upstream: false,
            ahead: 0,
            behind: 0,
        };
    }
    let parts: Vec<&str> = counts_out.trim().split('\t').collect();
    let (ahead, behind) = if parts.len() == 2 {
        (
            parts[0].parse::<usize>().unwrap_or(0),
            parts[1].parse::<usize>().unwrap_or(0),
        )
    } else {
        (0, 0)
    };
    SyncStatus {
        has_upstream: true,
        ahead,
        behind,
    }
}

// Check if a path is a git repository
pub fn is_git_repo(repo_path: &Path) -> bool {
    run_git_cmd(repo_path, &["rev-parse", "--is-inside-work-tree"]).is_ok()
}

// Git pull repository
pub fn pull_repository(repo_path: &Path, lang: Language) -> Result<String, String> {
    // 1. Check whether a remote is configured
    let remote_output = run_git_cmd(repo_path, &["remote"]).unwrap_or_default();
    if remote_output.trim().is_empty() {
        return Err(crate::i18n::t(lang, "git_no_remote_pull").to_string());
    }

    // 2. Check that an upstream tracking branch is configured
    if run_git_cmd(repo_path, &["rev-parse", "--abbrev-ref", "@{u}"]).is_err() {
        return Err(crate::i18n::t(lang, "git_no_upstream").to_string());
    }

    let output = run_with_timeout(git_command_for(repo_path, &["pull"]), GIT_NETWORK_TIMEOUT)
        .map_err(|e| format!("git pull: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        if stdout.trim().is_empty() {
            Ok(crate::i18n::t(lang, "git_already_up_to_date").to_string())
        } else {
            Ok(stdout.trim().to_string())
        }
    } else {
        Err(if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        })
    }
}

pub struct SyncStatus {
    pub has_upstream: bool,
    pub ahead: usize,
    pub behind: usize,
}

// Git fetch repository
pub fn fetch_repository(repo_path: &Path, lang: Language) -> Result<String, String> {
    // 1. Check whether a remote is configured
    let remote_output = run_git_cmd(repo_path, &["remote"]).unwrap_or_default();
    if remote_output.trim().is_empty() {
        return Err(crate::i18n::t(lang, "git_no_remote_fetch").to_string());
    }

    let output = run_with_timeout(git_command_for(repo_path, &["fetch"]), GIT_NETWORK_TIMEOUT)
        .map_err(|e| format!("git fetch: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(crate::i18n::t(lang, "git_fetch_success").to_string())
    } else {
        Err(if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        })
    }
}

// Helper to format file sizes
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// Get repository name
fn get_repo_name(repo_path: &Path) -> String {
    if let Ok(url) = run_git_cmd(repo_path, &["config", "--get", "remote.origin.url"]) {
        let url = url.trim();
        if !url.is_empty()
            && let Some(pos) = url.rfind('/')
        {
            let name = &url[pos + 1..];
            let name = name.trim_end_matches(".git");
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }

    // SSH locators (ssh://…) are not local filesystem paths, so canonicalize
    // would fail; derive the name from the remote path's last segment instead.
    if let Some((_, remote_path)) = parse_ssh_repo(repo_path) {
        return repo_name_from_ssh_path(&remote_path)
            .unwrap_or_else(|| "不明なリポジトリ".to_string());
    }

    repo_path
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "不明なリポジトリ".to_string())
}

/// Last path segment of an SSH remote path, used as the repository name
/// (`/home/dev/myrepo` → `myrepo`, `/srv/app.git` → `app`).
fn repo_name_from_ssh_path(remote_path: &str) -> Option<String> {
    let name = remote_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".git");
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

// Format Unix timestamp to local time string
fn format_timestamp(ts: i64) -> String {
    if ts == 0 {
        return "不明".to_string();
    }
    if let Some(dt) = Utc.timestamp_opt(ts, 0).single() {
        dt.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    } else {
        "不明".to_string()
    }
}

pub fn get_summary(repo_path: &Path, members: &[Member]) -> Result<Summary, String> {
    let repo_name = get_repo_name(repo_path);
    // SSH locators (ssh://…) have no local path to canonicalize — doing so
    // failed with "os error 123" on Windows and, since get_summary is the one
    // mandatory analysis, turned the whole dashboard into an error. Use the
    // locator string as-is for those.
    let repo_path_str = if parse_ssh_repo(repo_path).is_some() {
        repo_path.to_string_lossy().to_string()
    } else {
        repo_path
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string()
    };

    // All summary metadata in one batch: over SSH this is a single connection
    // instead of ~10 separate handshakes (the dominant cost on remote repos).
    let cmds: [&[&str]; 9] = [
        &["branch", "--show-current"],
        &["rev-list", "--count", "HEAD"],
        &["branch", "-a"],
        &["ls-files"],
        &["remote"],
        &["status", "--porcelain=v1"],
        &["rev-parse", "--abbrev-ref", "@{u}"],
        &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
        &["log", "--format=%an|||%ae|||%at"],
    ];
    let r = run_git_batch(repo_path, &cmds);
    // "" on a failed command mirrors the previous per-call unwrap_or_default
    let out = |i: usize| -> &str {
        r.get(i)
            .and_then(|x| x.as_ref().ok())
            .map(String::as_str)
            .unwrap_or("")
    };

    let current_branch = out(0).trim().to_string();
    let total_commits: usize = out(1).trim().parse().unwrap_or(0);
    let total_branches = out(2)
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.contains("->"))
        .count();

    let mut total_files = 0;
    let mut total_size_bytes = 0;
    for file_path in out(3).lines() {
        let file_path = file_path.trim();
        if file_path.is_empty() {
            continue;
        }
        total_files += 1;
        // Local files only; for SSH repos these paths do not resolve locally,
        // so the byte total stays 0 (size reporting is documented as N/A there)
        let full_path = repo_path.join(file_path);
        if let Ok(meta) = std::fs::metadata(&full_path) {
            total_size_bytes += meta.len();
        }
    }

    let has_remote = !out(4).trim().is_empty();
    let uncommitted_changes = out(5).lines().filter(|l| !l.trim().is_empty()).count();
    let has_upstream = !out(6).trim().is_empty();
    let sync_status = parse_sync_status(has_upstream, out(7));
    let total_contributors = process_contributor_log(out(8), members).len();

    Ok(Summary {
        repo_name,
        repo_path: repo_path_str,
        current_branch,
        total_commits,
        total_contributors,
        total_branches,
        total_files,
        total_size_bytes,
        total_size_formatted: format_size(total_size_bytes),
        has_upstream: sync_status.has_upstream,
        ahead: sync_status.ahead,
        behind: sync_status.behind,
        has_remote,
        uncommitted_changes,
    })
}

pub fn process_contributor_log(log_data: &str, members: &[Member]) -> Vec<Contributor> {
    let mut stats: HashMap<String, (usize, i64, i64, std::collections::HashSet<String>)> =
        HashMap::new();
    let mut total_commits = 0;

    for line in log_data.lines() {
        let parts: Vec<&str> = line.split("|||").collect();
        if parts.len() < 3 {
            continue;
        }
        let raw_name = parts[0].trim().to_string();
        let raw_email = parts[1].trim().to_string();
        let timestamp: i64 = parts[2].trim().parse().unwrap_or(0);

        let mut resolved_name = raw_name.clone();

        for m in members {
            let matches_canonical = m.canonical_name.eq_ignore_ascii_case(&raw_name);
            let matches_alias = m.aliases.iter().any(|alias| {
                alias.eq_ignore_ascii_case(&raw_name) || alias.eq_ignore_ascii_case(&raw_email)
            });

            if matches_canonical || matches_alias {
                resolved_name = m.canonical_name.clone();
                break;
            }
        }

        let entry = stats.entry(resolved_name).or_insert((
            0,
            timestamp,
            timestamp,
            std::collections::HashSet::new(),
        ));
        entry.0 += 1;
        if timestamp < entry.1 {
            entry.1 = timestamp;
        }
        if timestamp > entry.2 {
            entry.2 = timestamp;
        }
        if !raw_email.is_empty() {
            entry.3.insert(raw_email);
        }
        total_commits += 1;
    }

    let mut contributors = Vec::new();
    for (name, (count, first, last, emails)) in stats {
        let pct = if total_commits > 0 {
            (count as f64 / total_commits as f64) * 100.0
        } else {
            0.0
        };

        let first_date = format_timestamp(first);
        let last_date = format_timestamp(last);

        let mut is_member = false;
        let mut is_active = false;
        for m in members {
            if m.canonical_name == name {
                is_member = true;
                is_active = m.is_active;
                break;
            }
        }

        let email_list = emails.into_iter().collect::<Vec<_>>().join(", ");

        contributors.push(Contributor {
            name,
            email: email_list,
            commit_count: count,
            percentage: pct,
            first_commit: first_date,
            last_commit: last_date,
            is_member,
            is_active,
        });
    }

    contributors.sort_by_key(|b| std::cmp::Reverse(b.commit_count));
    contributors
}

pub fn get_contributors(repo_path: &Path, members: &[Member]) -> Result<Vec<Contributor>, String> {
    let log_data = match run_git_cmd(repo_path, &["log", "--format=%an|||%ae|||%at"]) {
        Ok(data) => data,
        Err(_) => return Ok(Vec::new()), // no commits yet
    };

    Ok(process_contributor_log(&log_data, members))
}

pub fn get_branches(repo_path: &Path) -> Result<Vec<BranchInfo>, String> {
    let output = match run_git_cmd(
        repo_path,
        &[
            "branch",
            "-a",
            "--format=%(refname:short)|||%(committerdate:unix)|||%(authorname)|||%(subject)",
        ],
    ) {
        Ok(data) => data,
        Err(_) => return Ok(Vec::new()),
    };

    let mut branches = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split("|||").collect();
        if parts.len() < 4 {
            continue;
        }
        let refname = parts[0].trim().to_string();
        if refname.is_empty() || refname == "HEAD" || refname.contains("->") {
            continue;
        }

        if seen.contains(&refname) {
            continue;
        }
        seen.insert(refname.clone());

        let is_remote = refname.starts_with("remotes/") || refname.starts_with("origin/");
        let clean_name = refname.trim_start_matches("remotes/").to_string();

        let date_unix: i64 = parts[1].trim().parse().unwrap_or(0);
        let author = parts[2].trim().to_string();
        let message = parts[3].trim().to_string();

        let date_str = format_timestamp(date_unix);

        branches.push(BranchInfo {
            name: clean_name,
            is_remote,
            author,
            date: date_str,
            date_unix,
            message,
        });
    }

    branches.sort_by_key(|b| std::cmp::Reverse(b.date_unix));

    Ok(branches)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TagInfo {
    pub name: String,
    pub date: String,
    pub message: String,
    /// Short hash of the commit the tag points to (annotated tags are
    /// dereferenced). serde(default): older disk caches lack this field —
    /// such tags just miss their badge until the next refresh.
    #[serde(default)]
    pub hash: String,
}

pub fn get_tags(repo_path: &Path) -> Result<Vec<TagInfo>, String> {
    let output = match run_git_cmd(
        repo_path,
        &[
            "tag",
            "-l",
            "--sort=-creatordate",
            "--format=%(refname:short)|||%(creatordate:short)|||%(subject)|||%(committerdate:short)|||%(objectname:short)|||%(*objectname:short)",
        ],
    ) {
        Ok(data) => data,
        Err(_) => return Ok(Vec::new()),
    };

    let mut tags = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split("|||").collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[0].trim().to_string();
        if name.is_empty() {
            continue;
        }

        let creatordate = parts[1].trim().to_string();
        let subject = parts[2].trim().to_string();
        let committerdate = parts[3].trim().to_string();
        // %(*objectname) is the dereferenced commit of an annotated tag and is
        // empty for lightweight tags, where %(objectname) already is the commit
        let hash = parts
            .get(5)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .or_else(|| parts.get(4).map(|s| s.trim()))
            .unwrap_or("")
            .to_string();

        let date = if !creatordate.is_empty() {
            creatordate
        } else if !committerdate.is_empty() {
            committerdate
        } else {
            "不明".to_string()
        };

        let message = if !subject.is_empty() {
            subject
        } else {
            if let Ok(commit_msg) = run_git_cmd(repo_path, &["log", "-1", "--format=%s", &name]) {
                let msg = commit_msg.trim().to_string();
                if msg.is_empty() {
                    "メッセージなし".to_string()
                } else {
                    msg
                }
            } else {
                "No message".to_string()
            }
        };

        tags.push(TagInfo {
            name,
            date,
            message,
            hash,
        });
    }

    Ok(tags)
}

/// Pure bucketing core of get_activity, separated for testability.
/// `days == 0` means the entire history (window inferred from the oldest
/// commit). Bucket size scales with the window so the bar chart stays
/// readable: daily up to ~6 weeks, weekly up to ~7 months, then coarser.
/// The hourly/weekly histograms count only commits inside the same window.
fn build_activity(timestamps: &[i64], days: usize, today: chrono::NaiveDate) -> Activity {
    let mut hourly = vec![0; 24];
    let mut weekly = vec![0; 7];

    let span_days: i64 = if days > 0 {
        days as i64
    } else {
        timestamps
            .iter()
            .filter_map(|ts| Utc.timestamp_opt(*ts, 0).single())
            .map(|dt| {
                today
                    .signed_duration_since(dt.with_timezone(&Local).date_naive())
                    .num_days()
            })
            .max()
            .unwrap_or(0)
            + 1
    };
    let span_days = span_days.max(1);

    let bucket_days: i64 = if span_days <= 45 {
        1
    } else if span_days <= 217 {
        7
    } else {
        // ceil div by hand: i64::div_ceil is unstable (values are positive)
        (span_days + 30) / 31
    };
    let num_buckets = (((span_days + bucket_days - 1) / bucket_days).max(1)) as usize;

    // Bucket i (0 = newest) covers [i*bucket, (i+1)*bucket) days ago; the
    // label is the bucket's oldest day, rendered oldest-first
    let mut counts = vec![0usize; num_buckets];
    let mut dates = Vec::with_capacity(num_buckets);
    for i in (0..num_buckets as i64).rev() {
        let start = today - Duration::days((i + 1) * bucket_days - 1);
        let label = if span_days > 366 {
            start.format("%y/%m").to_string()
        } else {
            start.format("%m/%d").to_string()
        };
        dates.push(label);
    }

    for &ts in timestamps {
        let Some(dt) = Utc.timestamp_opt(ts, 0).single() else {
            continue;
        };
        let local_dt = dt.with_timezone(&Local);
        let days_ago = today
            .signed_duration_since(local_dt.date_naive())
            .num_days();
        // git log --since filters by committer date but %at is the author
        // timestamp, so out-of-window stragglers are dropped here too
        if days_ago < 0 || days_ago >= span_days {
            continue;
        }

        let hour = local_dt.hour() as usize;
        if hour < 24 {
            hourly[hour] += 1;
        }
        let weekday = local_dt.weekday().num_days_from_sunday() as usize;
        if weekday < 7 {
            weekly[weekday] += 1;
        }
        let b = (days_ago / bucket_days) as usize;
        if b < num_buckets {
            counts[num_buckets - 1 - b] += 1;
        }
    }

    Activity {
        daily: DailyActivity { dates, counts },
        hourly,
        weekly,
    }
}

/// Commit activity over the last `days` days (0 = entire history).
pub fn get_activity(repo_path: &Path, days: usize) -> Result<Activity, String> {
    let since_arg = format!("--since={days} days ago");
    let mut args = vec!["log", "--format=%at"];
    if days > 0 {
        args.push(&since_arg);
    }
    let log_data = match run_git_cmd(repo_path, &args) {
        Ok(data) => data,
        Err(_) => {
            return Ok(Activity {
                daily: DailyActivity {
                    dates: vec![],
                    counts: vec![],
                },
                hourly: vec![0; 24],
                weekly: vec![0; 7],
            });
        }
    };

    let timestamps: Vec<i64> = log_data
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect();
    Ok(build_activity(&timestamps, days, Local::now().date_naive()))
}

pub fn get_files_report(repo_path: &Path) -> Result<FilesReport, String> {
    let output = match run_git_cmd(repo_path, &["ls-files"]) {
        Ok(data) => data,
        Err(_) => {
            return Ok(FilesReport {
                extensions: vec![],
                largest_files: vec![],
            });
        }
    };

    let mut ext_map = HashMap::new();
    let mut all_files = Vec::new();
    let mut total_files_count = 0;

    for file_path in output.lines() {
        let file_path = file_path.trim();
        if file_path.is_empty() {
            continue;
        }

        let full_path = repo_path.join(file_path);
        let size_bytes = match std::fs::metadata(&full_path) {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };

        total_files_count += 1;

        let path_obj = Path::new(file_path);
        let ext = path_obj
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("other")
            .to_lowercase();

        *ext_map.entry(ext).or_insert(0) += 1;

        all_files.push(FileInfo {
            path: file_path.to_string(),
            size_bytes,
            size_formatted: format_size(size_bytes),
        });
    }

    let mut extensions: Vec<FileExtInfo> = ext_map
        .into_iter()
        .map(|(ext, count)| {
            let percentage = if total_files_count > 0 {
                (count as f64 / total_files_count as f64) * 100.0
            } else {
                0.0
            };
            FileExtInfo {
                ext,
                count,
                percentage,
            }
        })
        .collect();
    extensions.sort_by_key(|b| std::cmp::Reverse(b.count));

    all_files.sort_by_key(|b| std::cmp::Reverse(b.size_bytes));
    let largest_files = all_files.into_iter().take(10).collect();

    Ok(FilesReport {
        extensions,
        largest_files,
    })
}

// Find project README file and read its content
/// Guard the Markdown renderer (and the disk cache) against pathological
/// README sizes; 300k chars is far beyond any README meant for humans.
fn cap_readme(mut content: String) -> String {
    const MAX_CHARS: usize = 300_000;
    if content.chars().count() > MAX_CHARS {
        content = content.chars().take(MAX_CHARS).collect();
        content.push_str("\n\n... (truncated due to large file size)");
    }
    content
}

pub fn find_readme(repo_path: &Path) -> Option<(String, String)> {
    const CANDIDATES: [&str; 5] = [
        "README.md",
        "README.txt",
        "README.rst",
        "readme.md",
        "Readme.md",
    ];

    // Remote (ssh://) repositories have no local files: read via `git show`
    if parse_ssh_repo(repo_path).is_some() {
        for name in CANDIDATES {
            if let Ok(content) = run_git_cmd(repo_path, &["show", &format!("HEAD:{name}")]) {
                return Some((name.to_string(), cap_readme(content)));
            }
        }
        return None;
    }

    for name in CANDIDATES {
        let p = repo_path.join(name);
        if let Ok(content) = std::fs::read_to_string(&p) {
            return Some((name.to_string(), cap_readme(content)));
        }
    }
    None
}

pub fn parse_commit_log(log_output: &str) -> Vec<CommitSummary> {
    let mut commits = Vec::new();
    for line in log_output.lines() {
        let parts: Vec<&str> = line.split("|||").collect();
        if parts.len() < 4 {
            continue;
        }
        commits.push(CommitSummary {
            hash: parts[0].trim().to_string(),
            author: parts[1].trim().to_string(),
            date: parts[2].trim().to_string(),
            message: parts[3].trim().to_string(),
        });
    }
    commits
}

fn build_diff_args<'a>(range: &'a [String], extra: &[&'a str], paths: &[&'a str]) -> Vec<&'a str> {
    let mut args = Vec::with_capacity(3 + extra.len() + range.len() + 1 + paths.len());
    args.push("diff");
    args.extend_from_slice(extra);
    for r in range {
        args.push(r.as_str());
    }
    args.push("--");
    args.extend_from_slice(paths);
    args
}

// Get recent commits list
pub fn get_recent_commits(repo_path: &Path) -> Result<Vec<CommitSummary>, String> {
    let output = run_git_cmd(
        repo_path,
        &[
            "log",
            "-n",
            "30",
            "--format=%h|||%an|||%ad|||%s",
            "--date=format:%Y-%m-%d %H:%M",
        ],
    )?;
    Ok(parse_commit_log(&output))
}

use serde::Deserialize;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TechInfo {
    pub language: String,
    pub lang_version: String,
    pub framework: String,
    pub framework_version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VersionFileRule {
    pub file: String,
    pub regex: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TechRule {
    pub name: String,
    pub language: String,
    pub lang_version_files: Vec<VersionFileRule>,
    pub framework: String,
    pub framework_version_files: Vec<VersionFileRule>,
}

fn get_default_rules() -> Vec<TechRule> {
    vec![
        TechRule {
            name: "Ruby on Rails".to_string(),
            language: "Ruby".to_string(),
            lang_version_files: vec![
                VersionFileRule {
                    file: ".ruby-version".to_string(),
                    regex: r"(?:ruby-)?([0-9.]+)".to_string(),
                },
                VersionFileRule {
                    file: "Gemfile.lock".to_string(),
                    regex: r"RUBY VERSION\s+ruby\s+([0-9.]+)".to_string(),
                },
            ],
            framework: "Rails".to_string(),
            framework_version_files: vec![VersionFileRule {
                file: "Gemfile.lock".to_string(),
                regex: r"\srails\s+\(([0-9.]+)\)".to_string(),
            }],
        },
        TechRule {
            name: "Node.js".to_string(),
            language: "JavaScript/TypeScript".to_string(),
            lang_version_files: vec![
                VersionFileRule {
                    file: ".nvmrc".to_string(),
                    regex: r"^v?([0-9.]+)".to_string(),
                },
                VersionFileRule {
                    file: "package.json".to_string(),
                    regex: r#""node"\s*:\s*"[^"0-9]*([0-9.]+)"#.to_string(),
                },
            ],
            framework: "Next/React/Express".to_string(),
            framework_version_files: vec![
                VersionFileRule {
                    file: "package.json".to_string(),
                    regex: r#""next"\s*:\s*"[^"0-9]*([0-9.]+)"#.to_string(),
                },
                VersionFileRule {
                    file: "package.json".to_string(),
                    regex: r#""express"\s*:\s*"[^"0-9]*([0-9.]+)"#.to_string(),
                },
                VersionFileRule {
                    file: "package.json".to_string(),
                    regex: r#""react"\s*:\s*"[^"0-9]*([0-9.]+)"#.to_string(),
                },
            ],
        },
        TechRule {
            name: "Django".to_string(),
            language: "Python".to_string(),
            lang_version_files: vec![
                VersionFileRule {
                    file: ".python-version".to_string(),
                    regex: r"^([0-9.]+)".to_string(),
                },
                VersionFileRule {
                    file: "runtime.txt".to_string(),
                    regex: r"python-([0-9.]+)".to_string(),
                },
            ],
            framework: "Django".to_string(),
            framework_version_files: vec![
                VersionFileRule {
                    file: "requirements.txt".to_string(),
                    regex: r"(?:[Dd]jango|Django)==([0-9.]+)".to_string(),
                },
                VersionFileRule {
                    file: "Pipfile".to_string(),
                    regex: r#"(?:django|Django)\s*=\s*"[^"0-9]*([0-9.]+)"#.to_string(),
                },
            ],
        },
    ]
}

pub fn detect_tech_info(repo_path: &Path) -> TechInfo {
    let config_dir = crate::config::get_config_dir();
    let rules_path = config_dir.join("tech_rules.json");
    let rules: Vec<TechRule> = if rules_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&rules_path) {
            serde_json::from_str(&content).unwrap_or_else(|_| get_default_rules())
        } else {
            get_default_rules()
        }
    } else {
        let r = get_default_rules();
        if let Ok(content) = serde_json::to_string_pretty(&r) {
            let _ = crate::config::write_atomic(&rules_path, &content);
        }
        r
    };

    detect_tech_info_with_rules(repo_path, &rules)
}

pub fn detect_tech_info_with_rules(repo_path: &Path, rules: &[TechRule]) -> TechInfo {
    let mut tech_info = TechInfo {
        language: "-".to_string(),
        lang_version: "-".to_string(),
        framework: "-".to_string(),
        framework_version: "-".to_string(),
    };

    for rule in rules {
        let mut lang_version = None;
        let mut framework_version = None;
        let mut matched_lang = false;
        let mut matched_fw = false;

        // Check language version files
        for vf in &rule.lang_version_files {
            let file_path = repo_path.join(&vf.file);
            if file_path.exists() {
                matched_lang = true;
                if let Ok(content) = std::fs::read_to_string(&file_path)
                    && let Ok(re) = regex::Regex::new(&vf.regex)
                    && let Some(caps) = re.captures(&content)
                    && let Some(m) = caps.get(1)
                {
                    lang_version = Some(m.as_str().to_string());
                    break;
                }
            }
        }

        // Check framework version files
        for vf in &rule.framework_version_files {
            let file_path = repo_path.join(&vf.file);
            if file_path.exists() {
                matched_fw = true;
                if let Ok(content) = std::fs::read_to_string(&file_path)
                    && let Ok(re) = regex::Regex::new(&vf.regex)
                    && let Some(caps) = re.captures(&content)
                    && let Some(m) = caps.get(1)
                {
                    framework_version = Some(m.as_str().to_string());
                    break;
                }
            }
        }

        if matched_lang || matched_fw {
            tech_info.language = rule.language.clone();
            tech_info.lang_version = lang_version.unwrap_or_else(|| "-".to_string());
            tech_info.framework = rule.framework.clone();
            tech_info.framework_version = framework_version.unwrap_or_else(|| "-".to_string());
            break;
        }
    }

    tech_info
}

// ─── Diff view data types ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ChangedFile {
    pub status: String, // "M" | "A" | "D" | "R" | "?"
    pub path: String,   // display path (new path for renames)
    pub old_path: Option<String>,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiffRowKind {
    Context,
    Added,
    Removed,
    Modified, // left=removed, right=added
}

#[derive(Clone, Debug)]
pub struct DiffRow {
    pub kind: DiffRowKind,
    pub left_no: Option<usize>,
    pub left_text: Option<String>,
    pub left_tokens: Option<Vec<crate::syntax::Token>>,
    pub right_no: Option<usize>,
    pub right_text: Option<String>,
    pub right_tokens: Option<Vec<crate::syntax::Token>>,
}

#[derive(Clone, Debug)]
pub struct FileDiff {
    pub path: String,
    pub is_binary: bool,
    pub truncated: bool,
    pub rows: Vec<DiffRow>,
}

/// Commits available in the diff selector (up to 200).
pub fn get_commits_for_diff(repo_path: &Path) -> Result<Vec<CommitSummary>, String> {
    let output = run_git_cmd(
        repo_path,
        &[
            "log",
            "-n",
            "200",
            "--format=%h|||%an|||%ad|||%s",
            "--date=format:%Y-%m-%d %H:%M",
        ],
    )?;
    Ok(parse_commit_log(&output))
}

fn diff_range(base: Option<&str>, target: &str, three_dot: bool) -> Vec<String> {
    match base {
        // GitHub-compare style: changes on target since the merge-base
        Some(b) if three_dot => vec![format!("{b}...{target}")],
        Some(b) => vec![format!("{b}..{target}")],
        // Single-commit mode: diff against parent (first commit has no parent → treated as empty list)
        None => vec![format!("{target}^..{target}")],
    }
}

pub fn parse_changed_files(name_status: &str, numstat: &str) -> Vec<ChangedFile> {
    // Build numstat map: path -> (additions, deletions)
    let mut num_map: HashMap<String, (usize, usize)> = HashMap::new();
    for line in numstat.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let add = parts[0].trim().parse::<usize>().unwrap_or(0);
        let del = parts[1].trim().parse::<usize>().unwrap_or(0);
        // For renames git numstat writes "old => new" or just new path
        let path = parts[2].trim().to_string();
        num_map.insert(path, (add, del));
    }

    let mut files = Vec::new();
    for line in name_status.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.is_empty() {
            continue;
        }
        let status_raw = parts[0].trim();
        let status = if status_raw.starts_with('R') {
            "R".to_string()
        } else {
            status_raw.chars().next().unwrap_or('?').to_string()
        };

        let (path, old_path) = if status == "R" && parts.len() >= 3 {
            (
                parts[2].trim().to_string(),
                Some(parts[1].trim().to_string()),
            )
        } else if parts.len() >= 2 {
            (parts[1].trim().to_string(), None)
        } else {
            continue;
        };

        let (additions, deletions) = num_map.get(&path).copied().unwrap_or((0, 0));
        files.push(ChangedFile {
            status,
            path,
            old_path,
            additions,
            deletions,
        });
    }
    files
}

/// Returns the list of files changed between base (or parent) and target.
pub fn get_changed_files(
    repo_path: &Path,
    base: Option<&str>,
    target: &str,
    three_dot: bool,
) -> Result<Vec<ChangedFile>, String> {
    check_safe_ref_opt(base)?;
    check_safe_ref(target)?;

    let range = if target == "WORKING_TREE" {
        vec!["HEAD".to_string()]
    } else {
        diff_range(base, target, three_dot)
    };

    let ns_args = build_diff_args(&range, &["--name-status", "--find-renames"], &[]);
    let name_status = run_git_cmd(repo_path, &ns_args).unwrap_or_default();

    let num_args = build_diff_args(&range, &["--numstat", "--find-renames"], &[]);
    let numstat = run_git_cmd(repo_path, &num_args).unwrap_or_default();

    Ok(parse_changed_files(&name_status, &numstat))
}

/// Char-level similarity in [0, 1] between two lines: 2*LCS/(len_a+len_b).
/// Very long lines fall back to a cheap char-frequency ratio to stay fast.
fn line_similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.trim().chars().collect();
    let b: Vec<char> = b.trim().chars().collect();
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let (n, m) = (a.len(), b.len());
    let denom = (n + m) as f32;
    if n * m <= 40_000 {
        // LCS length with two rolling rows (O(min) memory)
        let mut prev = vec![0u32; m + 1];
        let mut cur = vec![0u32; m + 1];
        for i in 1..=n {
            for j in 1..=m {
                cur[j] = if a[i - 1] == b[j - 1] {
                    prev[j - 1] + 1
                } else {
                    prev[j].max(cur[j - 1])
                };
            }
            std::mem::swap(&mut prev, &mut cur);
        }
        2.0 * prev[m] as f32 / denom
    } else {
        let mut counts: HashMap<char, u32> = HashMap::new();
        for &c in &a {
            *counts.entry(c).or_insert(0) += 1;
        }
        let mut common = 0u32;
        for &c in &b {
            if let Some(k) = counts.get_mut(&c)
                && *k > 0
            {
                *k -= 1;
                common += 1;
            }
        }
        2.0 * common as f32 / denom
    }
}

/// Positionally paired +/- lines are shown side-by-side as Modified only when
/// at least half their characters match; below this they read better as a
/// plain removal plus addition (no forced intra-line comparison).
const PAIR_SIMILARITY_THRESHOLD: f32 = 0.5;

/// Parses a unified diff string into split-view DiffRows.
/// This is a pure function so it can be unit-tested.
pub fn parse_unified_diff(diff_text: &str) -> Vec<DiffRow> {
    let mut rows: Vec<DiffRow> = Vec::new();
    let mut left_no: usize = 0;
    let mut right_no: usize = 0;

    // Buffers for a consecutive block of +/- lines inside one hunk
    let mut removed_buf: Vec<String> = Vec::new();
    let mut added_buf: Vec<String> = Vec::new();
    let mut removed_start: usize = 0;
    let mut added_start: usize = 0;

    let flush = |rows: &mut Vec<DiffRow>,
                 removed: &mut Vec<String>,
                 added: &mut Vec<String>,
                 r_start: usize,
                 a_start: usize| {
        let pairs = removed.len().max(added.len());
        // Dissimilar pairs are buffered so consecutive ones come out as a block
        // of removals followed by a block of additions, not a zigzag.
        let mut pend_rem: Vec<DiffRow> = Vec::new();
        let mut pend_add: Vec<DiffRow> = Vec::new();
        for i in 0..pairs {
            let l = removed.get(i).cloned();
            let r = added.get(i).cloned();
            let ln = if i < removed.len() {
                Some(r_start + i)
            } else {
                None
            };
            let rn = if i < added.len() {
                Some(a_start + i)
            } else {
                None
            };
            let paired = match (&l, &r) {
                (Some(l), Some(r)) => line_similarity(l, r) >= PAIR_SIMILARITY_THRESHOLD,
                _ => false,
            };
            if paired {
                rows.append(&mut pend_rem);
                rows.append(&mut pend_add);
                rows.push(DiffRow {
                    kind: DiffRowKind::Modified,
                    left_no: ln,
                    left_text: l,
                    left_tokens: None,
                    right_no: rn,
                    right_text: r,
                    right_tokens: None,
                });
            } else {
                if l.is_some() {
                    pend_rem.push(DiffRow {
                        kind: DiffRowKind::Removed,
                        left_no: ln,
                        left_text: l,
                        left_tokens: None,
                        right_no: None,
                        right_text: None,
                        right_tokens: None,
                    });
                }
                if r.is_some() {
                    pend_add.push(DiffRow {
                        kind: DiffRowKind::Added,
                        left_no: None,
                        left_text: None,
                        left_tokens: None,
                        right_no: rn,
                        right_text: r,
                        right_tokens: None,
                    });
                }
            }
        }
        rows.append(&mut pend_rem);
        rows.append(&mut pend_add);
        removed.clear();
        added.clear();
    };

    for line in diff_text.lines() {
        if line.starts_with("@@") {
            // Flush pending +/- block before starting a new hunk
            flush(
                &mut rows,
                &mut removed_buf,
                &mut added_buf,
                removed_start,
                added_start,
            );

            // Parse @@ -a,b +c,d @@
            let parse_hunk = || -> Option<(usize, usize)> {
                let s = line.trim_start_matches('@').trim_start_matches(' ');
                let mut parts = s.split_whitespace();
                let old = parts.next()?;
                let new = parts.next()?;
                let old_start = old
                    .trim_start_matches('-')
                    .split(',')
                    .next()?
                    .parse::<usize>()
                    .ok()?;
                let new_start = new
                    .trim_start_matches('+')
                    .split(',')
                    .next()?
                    .parse::<usize>()
                    .ok()?;
                Some((old_start, new_start))
            };
            if let Some((ls, rs)) = parse_hunk() {
                left_no = ls;
                right_no = rs;
            }
            continue;
        }

        // Skip diff header lines
        if line.starts_with("diff ")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
        {
            flush(
                &mut rows,
                &mut removed_buf,
                &mut added_buf,
                removed_start,
                added_start,
            );
            continue;
        }

        // "\ No newline at end of file" is metadata, not content; counting it
        // as a context line would shift every line number after it.
        if line.starts_with('\\') {
            continue;
        }

        if let Some(stripped) = line.strip_prefix('-') {
            if removed_buf.is_empty() {
                removed_start = left_no;
            }
            removed_buf.push(stripped.to_string());
            left_no += 1;
        } else if let Some(stripped) = line.strip_prefix('+') {
            if added_buf.is_empty() {
                added_start = right_no;
            }
            added_buf.push(stripped.to_string());
            right_no += 1;
        } else {
            // Context line — flush pending block first
            flush(
                &mut rows,
                &mut removed_buf,
                &mut added_buf,
                removed_start,
                added_start,
            );
            let text = if let Some(stripped) = line.strip_prefix(' ') {
                stripped.to_string()
            } else {
                line.to_string()
            };
            rows.push(DiffRow {
                kind: DiffRowKind::Context,
                left_no: Some(left_no),
                left_text: Some(text.clone()),
                left_tokens: None,
                right_no: Some(right_no),
                right_text: Some(text),
                right_tokens: None,
            });
            left_no += 1;
            right_no += 1;
        }
    }
    flush(
        &mut rows,
        &mut removed_buf,
        &mut added_buf,
        removed_start,
        added_start,
    );
    rows
}

pub const MAX_DIFF_LINES_PUB: usize = 2000;
const MAX_DIFF_LINES: usize = MAX_DIFF_LINES_PUB;

/// Returns the split-view diff for a single file.
pub fn get_file_diff(
    repo_path: &Path,
    base: Option<&str>,
    target: &str,
    file_path: &str,
    ignore_whitespace: bool,
    full: bool,
    three_dot: bool,
) -> Result<FileDiff, String> {
    check_safe_ref_opt(base)?;
    check_safe_ref(target)?;

    let mut extra = vec!["--color=never"];
    if ignore_whitespace {
        extra.push("-w");
    }
    if full {
        extra.push("--unified=999999");
    }

    let range = if target == "WORKING_TREE" {
        vec!["HEAD".to_string()]
    } else {
        diff_range(base, target, three_dot)
    };

    let args = build_diff_args(&range, &extra, &[file_path]);
    let raw = run_git_cmd(repo_path, &args)?;

    // Match only git's own marker line, not file content that happens to
    // contain the phrase (content lines are prefixed with +/-/space).
    if raw.lines().any(|l| l.starts_with("Binary files")) {
        return Ok(FileDiff {
            path: file_path.to_string(),
            is_binary: true,
            truncated: false,
            rows: vec![],
        });
    }

    let mut rows = parse_unified_diff(&raw);
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    for row in &mut rows {
        if let Some(ref text) = row.left_text {
            row.left_tokens = Some(crate::syntax::tokenize(text, ext));
        }
        if let Some(ref text) = row.right_text {
            row.right_tokens = Some(crate::syntax::tokenize(text, ext));
        }
    }
    let limit = if full { 5000 } else { MAX_DIFF_LINES };
    let truncated = rows.len() > limit;
    if truncated {
        rows.truncate(limit);
    }
    Ok(FileDiff {
        path: file_path.to_string(),
        is_binary: false,
        truncated,
        rows,
    })
}

// Commit header + message only (-s suppresses the diffstat: the detail
// panel renders the changed files as a styled tree instead)
pub fn get_commit_show(repo_path: &Path, hash: &str) -> Result<String, String> {
    check_safe_ref(hash)?;
    run_git_cmd(repo_path, &["show", "-s", hash])
}

// Get branch commits in oneline format with graph and colors (git log --graph --oneline --decorate --color=always)
pub fn get_branch_oneline_log(
    repo_path: &Path,
    branch_name: &str,
    limit: usize,
) -> Result<String, String> {
    check_safe_ref(branch_name)?;
    let limit_s = limit.to_string();
    run_git_cmd(
        repo_path,
        &[
            "log",
            branch_name,
            "-n",
            &limit_s,
            "--graph",
            "--decorate",
            "--color=always",
            "--pretty=format:%C(auto)%h%C(reset)%C(auto)%d%C(reset) %s %C(dim)(%cr, %an)%C(reset)",
        ],
    )
}

pub fn get_stash_list(repo_path: &Path) -> Result<Vec<StashEntry>, String> {
    let output = run_git_cmd(repo_path, &["stash", "list", "--format=%gd|%cr|%an|%s"])?;
    let mut stashes = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() == 4 {
            stashes.push(StashEntry {
                ref_name: parts[0].trim().to_string(),
                date_relative: parts[1].trim().to_string(),
                author: parts[2].trim().to_string(),
                message: parts[3].trim().to_string(),
            });
        }
    }
    Ok(stashes)
}

pub fn apply_stash(repo_path: &Path, ref_name: &str) -> Result<String, String> {
    check_safe_ref(ref_name)?;
    run_git_cmd(repo_path, &["stash", "apply", ref_name])
}

#[allow(dead_code)]
pub fn pop_stash(repo_path: &Path, ref_name: &str) -> Result<String, String> {
    check_safe_ref(ref_name)?;
    run_git_cmd(repo_path, &["stash", "pop", ref_name])
}

pub fn drop_stash(repo_path: &Path, ref_name: &str) -> Result<String, String> {
    check_safe_ref(ref_name)?;
    run_git_cmd(repo_path, &["stash", "drop", ref_name])
}

pub fn get_file_blame(
    repo_path: &Path,
    blame_ref: &str,
    file_path: &str,
) -> Result<Vec<BlameEntry>, String> {
    check_safe_ref(blame_ref)?;
    let args = vec!["blame", "--line-porcelain", blame_ref, "--", file_path];
    let output_res = run_git_cmd(repo_path, &args);

    let output = match output_res {
        Ok(out) => out,
        Err(err) => {
            if let Some(fallback_ref) = blame_ref.strip_suffix('^') {
                check_safe_ref(fallback_ref)?;
                let fallback_args =
                    vec!["blame", "--line-porcelain", fallback_ref, "--", file_path];
                run_git_cmd(repo_path, &fallback_args)?
            } else {
                return Err(err);
            }
        }
    };

    #[derive(Clone)]
    struct CommitInfo {
        author: String,
        date: String,
        summary: String,
    }

    let mut commit_cache: HashMap<String, CommitInfo> = HashMap::new();
    let mut entries = Vec::new();

    let mut current_hash = String::new();
    let mut temp_author = String::new();
    let mut temp_date = String::new();
    let mut temp_summary = String::new();

    for line in output.lines() {
        if line.starts_with('\t') {
            let info = commit_cache
                .entry(current_hash.clone())
                .or_insert_with(|| CommitInfo {
                    author: temp_author.clone(),
                    date: temp_date.clone(),
                    summary: temp_summary.clone(),
                });

            entries.push(BlameEntry {
                hash: current_hash.clone(),
                author: info.author.clone(),
                date: info.date.clone(),
                summary: info.summary.clone(),
            });
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "author" => {
                if parts.len() > 1 {
                    temp_author = parts[1].to_string();
                }
            }
            "author-time" => {
                if let Some(datetime) = parts
                    .get(1)
                    .and_then(|s| s.parse::<i64>().ok())
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                {
                    temp_date = datetime.format("%Y-%m-%d").to_string();
                }
            }
            "summary" => {
                if parts.len() > 1 {
                    temp_summary = parts[1].to_string();
                }
            }
            _ => {
                if parts[0].len() == 40 && parts[0].chars().all(|c| c.is_ascii_hexdigit()) {
                    current_hash = parts[0].to_string();
                }
            }
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Member;
    use std::fs;

    /// Timestamp for noon local time `days_ago` days before `today`
    /// (noon avoids DST-boundary surprises in date conversions).
    fn ts_days_ago(today: chrono::NaiveDate, days_ago: i64) -> i64 {
        let date = today - Duration::days(days_ago);
        Local
            .with_ymd_and_hms(date.year(), date.month(), date.day(), 12, 0, 0)
            .unwrap()
            .timestamp()
    }

    #[test]
    #[cfg(unix)]
    fn test_run_with_timeout_large_output_no_deadlock() {
        // 200 KB exceeds the 64 KiB pipe buffer: the reader threads must
        // drain it or the child would block before exiting
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("head -c 200000 /dev/zero");
        let out = run_with_timeout(cmd, std::time::Duration::from_secs(10)).unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout.len(), 200_000);
    }

    #[test]
    #[cfg(unix)]
    fn test_run_with_timeout_kills_hung_process() {
        let mut cmd = Command::new("sleep");
        cmd.arg("5");
        let start = std::time::Instant::now();
        let res = run_with_timeout(cmd, std::time::Duration::from_millis(200));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("タイムアウト"));
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
    }

    #[test]
    fn test_build_activity_30_days_daily_buckets() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let timestamps = vec![
            ts_days_ago(today, 0),  // today
            ts_days_ago(today, 0),  // today (second commit)
            ts_days_ago(today, 29), // oldest day inside the window
            ts_days_ago(today, 31), // outside → dropped everywhere
        ];
        let a = build_activity(&timestamps, 30, today);
        assert_eq!(a.daily.counts.len(), 30);
        assert_eq!(a.daily.dates.len(), 30);
        assert_eq!(*a.daily.counts.last().unwrap(), 2); // newest bucket
        assert_eq!(a.daily.counts[0], 1); // oldest bucket
        assert_eq!(a.daily.counts.iter().sum::<usize>(), 3);
        // hourly/weekly are windowed too: the 31-days-ago commit is excluded
        assert_eq!(a.hourly.iter().sum::<usize>(), 3);
        assert_eq!(a.weekly.iter().sum::<usize>(), 3);
        assert_eq!(a.hourly[12], 3); // all test commits are at noon
        assert_eq!(a.daily.dates[0], "05/12"); // 29 days before 06/10
        assert_eq!(a.daily.dates[29], "06/10");
    }

    #[test]
    fn test_build_activity_90_days_weekly_buckets() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let timestamps = vec![
            ts_days_ago(today, 0),  // newest bucket (0-6 days ago)
            ts_days_ago(today, 8),  // second bucket (7-13 days ago)
            ts_days_ago(today, 89), // last bucket
        ];
        let a = build_activity(&timestamps, 90, today);
        assert_eq!(a.daily.counts.len(), 13); // ceil(90 / 7)
        assert_eq!(*a.daily.counts.last().unwrap(), 1);
        assert_eq!(a.daily.counts[11], 1);
        assert_eq!(a.daily.counts[0], 1);
        assert_eq!(a.daily.counts.iter().sum::<usize>(), 3);
    }

    #[test]
    fn test_build_activity_all_history_span_from_oldest() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let timestamps = vec![
            ts_days_ago(today, 0),
            ts_days_ago(today, 700), // ~2 years back → coarse buckets
        ];
        let a = build_activity(&timestamps, 0, today);
        // span 701 days → bucket = ceil(701/31) = 23 days → 31 buckets
        assert_eq!(a.daily.counts.len(), 31);
        assert_eq!(a.daily.counts.iter().sum::<usize>(), 2);
        assert_eq!(a.daily.counts[0], 1);
        assert_eq!(*a.daily.counts.last().unwrap(), 1);
        // labels switch to year/month form for spans beyond a year
        assert!(a.daily.dates[0].contains('/'));
        assert_eq!(a.daily.dates[0].len(), 5); // "yy/mm"
    }

    #[test]
    fn test_build_activity_empty() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 6, 10).unwrap();
        let a = build_activity(&[], 0, today);
        assert_eq!(a.daily.counts.len(), 1);
        assert_eq!(a.daily.counts[0], 0);
        assert_eq!(a.hourly.iter().sum::<usize>(), 0);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024 - 1), "1024.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 15 / 10), "1.50 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_size(1024 * 1024 * 1024 * 25 / 10), "2.50 GB");
    }

    #[test]
    fn test_process_contributor_log() {
        let log_data = "\
田中|||tanaka@example.com|||1620000000
Tanaka|||tanaka@example.com|||1620000100
Bob|||bob@example.com|||1620000200
Alice|||alice@example.com|||1620000300
";
        let members = vec![
            Member {
                canonical_name: "田中".to_string(),
                aliases: vec!["Tanaka".to_string(), "tanaka@example.com".to_string()],
                is_active: true,
            },
            Member {
                canonical_name: "Alice".to_string(),
                aliases: vec![],
                is_active: false,
            },
        ];

        let result = process_contributor_log(log_data, &members);

        // Output should be sorted by commit count descending
        // Total commits: 4
        // Tanaka (田中 + Tanaka alias): 2 commits (50%)
        // Bob: 1 commit (25%)
        // Alice: 1 commit (25%)
        assert_eq!(result.len(), 3);

        // 1st: Tanaka
        assert_eq!(result[0].name, "田中");
        assert_eq!(result[0].commit_count, 2);
        assert_eq!(result[0].percentage, 50.0);
        assert!(result[0].is_member);
        assert!(result[0].is_active);

        // 2nd/3rd: Bob or Alice (both have 1 commit, Bob is active = false? No, Bob is not in members so is_member = false, is_active = false)
        // Alice is a member, but is_active = false.
        let alice_info = result.iter().find(|c| c.name == "Alice").unwrap();
        assert_eq!(alice_info.commit_count, 1);
        assert_eq!(alice_info.percentage, 25.0);
        assert!(alice_info.is_member);
        assert!(!alice_info.is_active);

        let bob_info = result.iter().find(|c| c.name == "Bob").unwrap();
        assert_eq!(bob_info.commit_count, 1);
        assert_eq!(bob_info.percentage, 25.0);
        assert!(!bob_info.is_member);
        assert!(!bob_info.is_active);
    }

    #[test]
    fn test_detect_tech_info_rails() {
        let temp_dir = std::env::temp_dir().join("test_git_dashboard_rails");
        let _ = fs::create_dir_all(&temp_dir);

        let gemfile_lock_content = "\
GEM
  remote: https://rubygems.org/
  specs:
    rails (7.0.5)
      actioncable (= 7.0.5)

PLATFORMS
  ruby

DEPENDENCIES
  rails (~> 7.0.5)

RUBY VERSION
   ruby 3.2.2p53
";
        fs::write(temp_dir.join("Gemfile.lock"), gemfile_lock_content).unwrap();
        fs::write(temp_dir.join(".ruby-version"), "ruby-3.2.1").unwrap();

        let rules = get_default_rules();
        let info = detect_tech_info_with_rules(&temp_dir, &rules);

        assert_eq!(info.language, "Ruby");
        assert_eq!(info.lang_version, "3.2.1");
        assert_eq!(info.framework, "Rails");
        assert_eq!(info.framework_version, "7.0.5");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_detect_tech_info_node() {
        let temp_dir = std::env::temp_dir().join("test_git_dashboard_node");
        let _ = fs::create_dir_all(&temp_dir);

        let package_json_content = r#"{
  "name": "test-node",
  "dependencies": {
    "next": "^13.4.19",
    "react": "18.2.0"
  },
  "engines": {
    "node": ">=18.16.0"
  }
}"#;
        fs::write(temp_dir.join("package.json"), package_json_content).unwrap();
        fs::write(temp_dir.join(".nvmrc"), "v16.14.0").unwrap();

        let rules = get_default_rules();
        let info = detect_tech_info_with_rules(&temp_dir, &rules);

        assert_eq!(info.language, "JavaScript/TypeScript");
        assert_eq!(info.lang_version, "16.14.0");
        assert_eq!(info.framework, "Next/React/Express");
        assert_eq!(info.framework_version, "13.4.19");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_detect_tech_info_django() {
        let temp_dir = std::env::temp_dir().join("test_git_dashboard_django");
        let _ = fs::create_dir_all(&temp_dir);

        fs::write(
            temp_dir.join("requirements.txt"),
            "Django==4.2.1\nrequests>=2.28.0",
        )
        .unwrap();
        fs::write(temp_dir.join(".python-version"), "3.10.2\n").unwrap();

        let rules = get_default_rules();
        let info = detect_tech_info_with_rules(&temp_dir, &rules);

        assert_eq!(info.language, "Python");
        assert_eq!(info.lang_version, "3.10.2");
        assert_eq!(info.framework, "Django");
        assert_eq!(info.framework_version, "4.2.1");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_unified_diff_additions_only() {
        let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,2 +1,4 @@
 fn main() {
+    let x = 1;
+    let y = 2;
 }
";
        let rows = parse_unified_diff(diff);
        let added: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == DiffRowKind::Added)
            .collect();
        assert_eq!(added.len(), 2);
        assert_eq!(added[0].right_text.as_deref(), Some("    let x = 1;"));
        assert!(added[0].left_text.is_none());
    }

    #[test]
    fn test_parse_unified_diff_modified_pairs() {
        let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,3 @@
 fn main() {
-    let x = 1;
+    let x = 42;
 }
";
        let rows = parse_unified_diff(diff);
        let modified: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == DiffRowKind::Modified)
            .collect();
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].left_text.as_deref(), Some("    let x = 1;"));
        assert_eq!(modified[0].right_text.as_deref(), Some("    let x = 42;"));
    }

    #[test]
    fn test_parse_unified_diff_removals_only() {
        let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,4 +1,2 @@
 fn main() {
-    let x = 1;
-    let y = 2;
 }
";
        let rows = parse_unified_diff(diff);
        let removed: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == DiffRowKind::Removed)
            .collect();
        assert_eq!(removed.len(), 2);
        assert!(removed[0].right_text.is_none());
    }

    #[test]
    fn test_parse_unified_diff_context() {
        let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,3 @@
 fn a() {}
 fn b() {}
 fn c() {}
";
        let rows = parse_unified_diff(diff);
        assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].left_no, Some(1));
        assert_eq!(rows[0].right_no, Some(1));
    }

    #[test]
    fn test_parse_unified_diff_multiple_hunks() {
        let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,3 @@
-line1
+line1 changed
 line2
 line3
@@ -10,3 +10,3 @@
 line10
-line11
+line11 changed
 line12
";
        let rows = parse_unified_diff(diff);
        let modified: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == DiffRowKind::Modified)
            .collect();
        assert_eq!(modified.len(), 2);
        assert_eq!(modified[0].left_no, Some(1));
        assert_eq!(modified[1].left_no, Some(11));
    }

    #[test]
    fn test_parse_unified_diff_dissimilar_pair_splits() {
        // A removed and an added line with little in common must not be
        // forced into a side-by-side Modified pair.
        let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,2 +1,2 @@
-completely different old content
+zzz qqq xxx
 ctx
";
        let rows = parse_unified_diff(diff);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].kind, DiffRowKind::Removed);
        assert_eq!(rows[0].left_no, Some(1));
        assert_eq!(rows[0].right_no, None);
        assert_eq!(rows[1].kind, DiffRowKind::Added);
        assert_eq!(rows[1].left_no, None);
        assert_eq!(rows[1].right_no, Some(1));
        assert_eq!(rows[2].kind, DiffRowKind::Context);
    }

    #[test]
    fn test_parse_unified_diff_similar_pair_modified() {
        // Small in-line edits keep the side-by-side Modified pairing
        let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,1 +1,1 @@
-let value = compute(1);
+let value = compute(2);
";
        let rows = parse_unified_diff(diff);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, DiffRowKind::Modified);
        assert_eq!(rows[0].left_no, Some(1));
        assert_eq!(rows[0].right_no, Some(1));
    }

    #[test]
    fn test_line_similarity() {
        assert!(line_similarity("let x = foo;", "let x = bar;") >= 0.5);
        assert!(line_similarity("abcdef", "xyzqwp") < 0.5);
        assert_eq!(line_similarity("", ""), 1.0);
        assert_eq!(line_similarity("abc", ""), 0.0);
    }

    #[test]
    fn test_parse_ssh_repo() {
        use std::path::PathBuf;
        let p = PathBuf::from("ssh://dev@vm01/home/dev/myrepo");
        assert_eq!(
            parse_ssh_repo(&p),
            Some(("dev@vm01".to_string(), "/home/dev/myrepo".to_string()))
        );
        // SSH config alias without user
        let p = PathBuf::from("ssh://devbox/srv/app");
        assert_eq!(
            parse_ssh_repo(&p),
            Some(("devbox".to_string(), "/srv/app".to_string()))
        );
        // Local paths are not SSH locators
        assert_eq!(parse_ssh_repo(Path::new("/Users/me/repo")), None);
        assert_eq!(parse_ssh_repo(Path::new("C:\\work\\repo")), None);
        // Malformed
        assert_eq!(parse_ssh_repo(Path::new("ssh://hostonly")), None);
    }

    #[test]
    fn test_split_batch_output() {
        let sep = "__SEP__";
        // Three commands: ok / failed (non-zero) / ok, with multiline output
        let raw = format!("main\n{sep}0\nerror: no upstream\n{sep}128\na\nb\nc\n{sep}0\n");
        let r = split_batch_output(&raw, sep, 3, "");
        assert_eq!(r[0].as_deref(), Ok("main"));
        assert_eq!(r[1].as_ref().unwrap_err(), "error: no upstream");
        assert_eq!(r[2].as_deref(), Ok("a\nb\nc"));
    }

    #[test]
    fn test_split_batch_output_empty_command_output() {
        let sep = "__SEP__";
        // A command that printed nothing still yields an Ok("")
        let raw = format!("{sep}0\nx\n{sep}0\n");
        let r = split_batch_output(&raw, sep, 2, "");
        assert_eq!(r[0].as_deref(), Ok(""));
        assert_eq!(r[1].as_deref(), Ok("x"));
    }

    #[test]
    fn test_split_batch_output_missing_sentinels_padded() {
        let sep = "__SEP__";
        // Connection dropped after the first command: rest become Err
        let raw = format!("first\n{sep}0\npartial");
        let r = split_batch_output(&raw, sep, 3, "connection closed");
        assert_eq!(r[0].as_deref(), Ok("first"));
        assert_eq!(r[1].as_ref().unwrap_err(), "connection closed");
        assert_eq!(r[2].as_ref().unwrap_err(), "connection closed");
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn test_split_batch_output_total_failure() {
        let r = split_batch_output("", "__SEP__", 2, "ssh: could not resolve host");
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|x| x.is_err()));
    }

    #[test]
    fn test_parse_sync_status() {
        let s = parse_sync_status(true, "3\t5");
        assert!(s.has_upstream);
        assert_eq!(s.ahead, 3);
        assert_eq!(s.behind, 5);

        let none = parse_sync_status(false, "");
        assert!(!none.has_upstream);
        assert_eq!((none.ahead, none.behind), (0, 0));

        // Malformed counts degrade to zero
        let bad = parse_sync_status(true, "garbage");
        assert_eq!((bad.ahead, bad.behind), (0, 0));
    }

    #[test]
    fn test_repo_name_from_ssh_path() {
        // get_summary / get_repo_name route ssh:// locators through this
        // instead of canonicalize (which fails with "os error 123" on Windows)
        assert_eq!(
            repo_name_from_ssh_path("/home/dev/myrepo"),
            Some("myrepo".to_string())
        );
        assert_eq!(
            repo_name_from_ssh_path("/srv/app.git"),
            Some("app".to_string())
        );
        assert_eq!(
            repo_name_from_ssh_path("/srv/app/"),
            Some("app".to_string())
        );
        assert_eq!(repo_name_from_ssh_path("/"), None);
        assert_eq!(repo_name_from_ssh_path(""), None);
    }

    #[test]
    fn test_shell_quote() {
        assert_eq!(shell_quote("log"), "'log'");
        assert_eq!(
            shell_quote("--pretty=format:%C(auto)%h %s"),
            "'--pretty=format:%C(auto)%h %s'"
        );
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn test_parse_unified_diff_no_newline_marker() {
        let diff = "\
--- a/foo.rs
+++ b/foo.rs
@@ -1,2 +1,2 @@
 line1
-old value
\\ No newline at end of file
+new value
\\ No newline at end of file
";
        let rows = parse_unified_diff(diff);
        // The "\ No newline" markers must not become rows or shift numbering
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, DiffRowKind::Context);
        assert_eq!(rows[1].kind, DiffRowKind::Modified);
        assert_eq!(rows[1].left_no, Some(2));
        assert_eq!(rows[1].right_no, Some(2));
    }

    #[test]
    fn test_get_summary_no_upstream() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("git_test_summary_no_upstream");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let output = Command::new("git")
            .arg("init")
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        assert!(output.status.success());

        // End-to-end through the batched metadata path (local repos run the
        // batch as N sequential git commands). A fresh repo has no upstream.
        let summary = get_summary(&temp_dir, &[]).unwrap();
        assert!(!summary.has_upstream);
        assert_eq!(summary.ahead, 0);
        assert_eq!(summary.behind, 0);
        assert!(!summary.has_remote);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_get_stash_list() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("git_test_stash_list");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        Command::new("git")
            .arg("init")
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&temp_dir)
            .output()
            .unwrap();

        // Need at least one commit to stash
        fs::write(temp_dir.join("file.txt"), "hello").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial commit"])
            .current_dir(&temp_dir)
            .output()
            .unwrap();

        // Check empty stash list
        let stashes = get_stash_list(&temp_dir).unwrap();
        assert!(stashes.is_empty());

        // Make change and stash it
        fs::write(temp_dir.join("file.txt"), "hello world").unwrap();
        Command::new("git")
            .args(["stash", "push", "-m", "my stashed changes"])
            .current_dir(&temp_dir)
            .output()
            .unwrap();

        let stashes = get_stash_list(&temp_dir).unwrap();
        assert_eq!(stashes.len(), 1);
        assert_eq!(stashes[0].ref_name, "stash@{0}");
        assert!(stashes[0].message.contains("my stashed changes"));
        assert_eq!(stashes[0].author, "Test User");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_get_file_blame() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("git_test_file_blame");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        Command::new("git")
            .arg("init")
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Blame Author"])
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "blame@example.com"])
            .current_dir(&temp_dir)
            .output()
            .unwrap();

        fs::write(temp_dir.join("file.txt"), "line 1\nline 2\n").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&temp_dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "first commit"])
            .current_dir(&temp_dir)
            .output()
            .unwrap();

        let blame = get_file_blame(&temp_dir, "HEAD", "file.txt").unwrap();
        assert_eq!(blame.len(), 2);
        assert_eq!(blame[0].author, "Blame Author");
        assert_eq!(blame[0].summary, "first commit");
        assert_eq!(blame[1].author, "Blame Author");
        assert_eq!(blame[1].summary, "first commit");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_commit_log() {
        let log = "\
a1b2c3d|||Alice|||2023-01-01 12:00|||First commit
e5f6g7h|||Bob|||2023-01-02 15:30|||Second commit
";
        let commits = parse_commit_log(log);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].hash, "a1b2c3d");
        assert_eq!(commits[0].author, "Alice");
        assert_eq!(commits[0].date, "2023-01-01 12:00");
        assert_eq!(commits[0].message, "First commit");

        assert_eq!(commits[1].hash, "e5f6g7h");
        assert_eq!(commits[1].author, "Bob");
        assert_eq!(commits[1].date, "2023-01-02 15:30");
        assert_eq!(commits[1].message, "Second commit");
    }

    #[test]
    fn test_parse_changed_files() {
        let name_status = "\
M\tsrc/main.rs
A\tsrc/git.rs
R098\told_name.rs\tnew_name.rs
D\tdeleted.rs
";
        let numstat = "\
10\t5\tsrc/main.rs
100\t0\tsrc/git.rs
20\t20\tnew_name.rs
0\t30\tdeleted.rs
";
        let files = parse_changed_files(name_status, numstat);
        assert_eq!(files.len(), 4);

        assert_eq!(files[0].status, "M");
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!(files[0].old_path, None);
        assert_eq!(files[0].additions, 10);
        assert_eq!(files[0].deletions, 5);

        assert_eq!(files[1].status, "A");
        assert_eq!(files[1].path, "src/git.rs");
        assert_eq!(files[1].old_path, None);
        assert_eq!(files[1].additions, 100);
        assert_eq!(files[1].deletions, 0);

        assert_eq!(files[2].status, "R");
        assert_eq!(files[2].path, "new_name.rs");
        assert_eq!(files[2].old_path, Some("old_name.rs".to_string()));
        assert_eq!(files[2].additions, 20);
        assert_eq!(files[2].deletions, 20);

        assert_eq!(files[3].status, "D");
        assert_eq!(files[3].path, "deleted.rs");
        assert_eq!(files[3].old_path, None);
        assert_eq!(files[3].additions, 0);
        assert_eq!(files[3].deletions, 30);
    }

    #[test]
    fn test_diff_range() {
        let range1 = diff_range(Some("v1.0.0"), "v2.0.0", false);
        assert_eq!(range1, vec!["v1.0.0..v2.0.0".to_string()]);

        let range2 = diff_range(None, "v2.0.0", false);
        assert_eq!(range2, vec!["v2.0.0^..v2.0.0".to_string()]);

        // merge-base (GitHub compare) form
        let range3 = diff_range(Some("main"), "feature", true);
        assert_eq!(range3, vec!["main...feature".to_string()]);
        // single-commit mode ignores the three-dot flag
        let range4 = diff_range(None, "v2.0.0", true);
        assert_eq!(range4, vec!["v2.0.0^..v2.0.0".to_string()]);
    }
}
