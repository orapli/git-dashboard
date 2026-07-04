use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Write to a sibling temp file then rename, so a crash mid-write
/// never leaves a truncated/corrupt JSON behind. The temp name is unique
/// per call because worker threads may save the same file concurrently.
pub(crate) fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp.{}.{}", std::process::id(), n));
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Repository {
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub host: Option<String>,
}

/// Extract the SSH host string from an `ssh://user@host/path` locator.
/// Returns None for local paths.
pub fn repo_host(path: &Path) -> Option<String> {
    crate::git::parse_ssh_repo(path).map(|(h, _)| h)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Member {
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub is_active: bool,
}

pub fn get_config_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "git-dashboard", "git-dashboard") {
        let path = proj_dirs.config_dir();
        if !path.exists() {
            let _ = fs::create_dir_all(path);

            // Migrate existing local files if they exist in current directory
            for filename in &["config.json", "members.json", "tech_rules.json"] {
                let local = Path::new(filename);
                if local.exists() {
                    let dest = path.join(filename);
                    let _ = fs::copy(local, dest);
                }
            }
        }
        path.to_path_buf()
    } else {
        PathBuf::from(".")
    }
}

/// Directory for cached analysis results (one JSON per repository).
pub fn cache_dir() -> PathBuf {
    get_config_dir().join("cache")
}

/// Cache file path for a repository, keyed by a hash of its absolute path.
pub fn repo_cache_path(repo_path: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    repo_path.hash(&mut h);
    cache_dir().join(format!("repo-{:016x}.json", h.finish()))
}

pub fn load_repositories() -> Vec<Repository> {
    let config_dir = get_config_dir();
    let path = config_dir.join("config.json");
    if path.exists()
        && let Ok(content) = fs::read_to_string(&path)
        && let Ok(repos) = serde_json::from_str(&content)
    {
        return repos;
    }

    // Fresh install: start with no repositories (the user registers their own)
    Vec::new()
}

pub fn save_repositories(repos: &[Repository]) -> Result<(), String> {
    let config_dir = get_config_dir();
    let path = config_dir.join("config.json");
    let content =
        serde_json::to_string_pretty(repos).map_err(|e| format!("Serialization error: {}", e))?;
    write_atomic(&path, &content).map_err(|e| format!("Failed to write config file: {}", e))
}

pub fn load_members() -> Vec<Member> {
    let config_dir = get_config_dir();
    let path = config_dir.join("members.json");
    if path.exists()
        && let Ok(content) = fs::read_to_string(&path)
        && let Ok(members) = serde_json::from_str(&content)
    {
        return members;
    }

    // Fresh install: start with no members (name merging is opt-in)
    Vec::new()
}

pub fn save_members(members: &[Member]) -> Result<(), String> {
    let config_dir = get_config_dir();
    let path = config_dir.join("members.json");
    let content =
        serde_json::to_string_pretty(members).map_err(|e| format!("Serialization error: {}", e))?;
    write_atomic(&path, &content).map_err(|e| format!("Failed to write members config file: {}", e))
}

/// One remembered base→target comparison (per repository, newest first).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RecentCompare {
    pub repo_path: String,
    pub base: String,
    pub target: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    Japanese,
}

impl Language {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Japanese => "日本語",
        }
    }

    #[allow(dead_code)]
    pub fn all() -> &'static [Language] {
        &[Language::English, Language::Japanese]
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Preferences {
    pub theme: String,
    pub language: Language,
    pub sidebar_collapsed: bool,
    pub sidebar_width: f32,
    pub editor_command: String,
    // Diff view toggles, persisted across sessions
    pub diff_ignore_whitespace: bool,
    pub diff_full_file: bool,
    pub diff_show_blame: bool,
    // Recent range comparisons (capped per repo and overall)
    pub recent_compares: Vec<RecentCompare>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "Catppuccin Mocha".to_string(),
            language: Language::English,
            sidebar_collapsed: false,
            sidebar_width: 260.0,
            editor_command: "code".to_string(),
            diff_ignore_whitespace: false,
            diff_full_file: false,
            diff_show_blame: false,
            recent_compares: Vec::new(),
        }
    }
}

pub fn load_preferences() -> Preferences {
    let config_dir = get_config_dir();
    let path = config_dir.join("prefs.json");
    if path.exists()
        && let Ok(content) = fs::read_to_string(&path)
        && let Ok(prefs) = serde_json::from_str(&content)
    {
        return prefs;
    }

    let default_prefs = Preferences::default();
    let _ = save_preferences(&default_prefs);
    default_prefs
}

pub fn save_preferences(prefs: &Preferences) -> Result<(), String> {
    let config_dir = get_config_dir();
    let path = config_dir.join("prefs.json");
    let content =
        serde_json::to_string_pretty(prefs).map_err(|e| format!("Serialization error: {}", e))?;
    write_atomic(&path, &content).map_err(|e| format!("Failed to write config file: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_serde_roundtrip() {
        let prefs = Preferences {
            language: Language::Japanese,
            ..Preferences::default()
        };
        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("\"language\":\"Japanese\""));
        let decoded: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.language, Language::Japanese);
    }

    #[test]
    fn test_language_default_fallback() {
        // Without language key, serde_json should fall back to Default (English)
        let json = r#"{"theme":"Catppuccin Mocha"}"#;
        let decoded: Preferences = serde_json::from_str(json).unwrap();
        assert_eq!(decoded.language, Language::English);
    }
}
