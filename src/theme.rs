use egui::Color32;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub is_dark: bool,
    pub bg: Color32,          // main panel background
    pub bg_sidebar: Color32,  // sidebar / secondary background
    pub bg_elevated: Color32, // card / elevated surface
    pub bg_hover: Color32,
    pub bg_active: Color32, // selected state
    pub border: Color32,
    pub text: Color32,       // primary text
    pub text_dim: Color32,   // secondary / muted text
    pub text_faint: Color32, // even weaker text
    pub accent: Color32,     // link / primary accent
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub info: Color32,
    pub added: Color32,        // diff + text
    pub removed: Color32,      // diff - text
    pub added_bg: Color32,     // diff + bg
    pub removed_bg: Color32,   // diff - bg
    pub chart: [Color32; 8],   // donut/bar chart series colours
    pub selection_bg: Color32, // selection background
    pub on_selection: Color32, // text colour on selection
    pub syntax_keyword: Color32,
    pub syntax_type: Color32,
    pub syntax_string: Color32,
    pub syntax_comment: Color32,
    pub syntax_number: Color32,
    pub syntax_punctuation: Color32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThemeDef {
    pub is_dark: bool,
    #[serde(rename = "bg_base", alias = "bg")]
    pub bg: String,
    #[serde(rename = "bg_mantle", alias = "bg_sidebar")]
    pub bg_sidebar: String,
    #[serde(rename = "bg_surface0", alias = "bg_elevated")]
    pub bg_elevated: String,
    #[serde(rename = "bg_surface1", alias = "bg_hover")]
    pub bg_hover: String,
    pub bg_active: String,
    pub border: String,
    pub text: String,
    #[serde(rename = "subtext", alias = "text_dim")]
    pub text_dim: String,
    #[serde(rename = "muted", alias = "text_faint")]
    pub text_faint: String,
    pub accent: String,
    #[serde(rename = "green", alias = "success")]
    pub success: String,
    #[serde(rename = "yellow", alias = "warning")]
    pub warning: String,
    #[serde(rename = "red", alias = "error")]
    pub error: String,
    pub info: String,
    pub added: String,
    pub removed: String,
    pub added_bg: String,
    pub removed_bg: String,
    pub chart: Vec<String>,
    #[serde(default)]
    pub selection_bg: Option<String>,
    #[serde(default)]
    pub on_selection: Option<String>,
    #[serde(default)]
    pub syntax_keyword: Option<String>,
    #[serde(default)]
    pub syntax_type: Option<String>,
    #[serde(default)]
    pub syntax_string: Option<String>,
    #[serde(default)]
    pub syntax_comment: Option<String>,
    #[serde(default)]
    pub syntax_number: Option<String>,
    #[serde(default)]
    pub syntax_punctuation: Option<String>,
}

fn parse_hex(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        Color32::from_rgb(r, g, b)
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
        Color32::from_rgba_unmultiplied(r, g, b, a)
    } else {
        Color32::TRANSPARENT
    }
}

impl ThemeDef {
    pub fn to_theme(&self) -> Theme {
        let mut chart = [Color32::TRANSPARENT; 8];
        let fallback_chart = [
            Color32::from_rgb(99, 102, 241),
            Color32::from_rgb(6, 182, 212),
            Color32::from_rgb(16, 185, 129),
            Color32::from_rgb(245, 158, 11),
            Color32::from_rgb(236, 72, 153),
            Color32::from_rgb(139, 92, 246),
            Color32::from_rgb(244, 63, 94),
            Color32::from_rgb(168, 85, 247),
        ];
        for (i, item) in chart.iter_mut().enumerate() {
            if i < self.chart.len() {
                *item = parse_hex(&self.chart[i]);
            } else {
                *item = fallback_chart[i];
            }
        }

        Theme {
            is_dark: self.is_dark,
            bg: parse_hex(&self.bg),
            bg_sidebar: parse_hex(&self.bg_sidebar),
            bg_elevated: parse_hex(&self.bg_elevated),
            bg_hover: parse_hex(&self.bg_hover),
            bg_active: parse_hex(&self.bg_active),
            border: parse_hex(&self.border),
            text: parse_hex(&self.text),
            text_dim: parse_hex(&self.text_dim),
            text_faint: parse_hex(&self.text_faint),
            accent: parse_hex(&self.accent),
            success: parse_hex(&self.success),
            warning: parse_hex(&self.warning),
            error: parse_hex(&self.error),
            info: parse_hex(&self.info),
            added: parse_hex(&self.added),
            removed: parse_hex(&self.removed),
            added_bg: parse_hex(&self.added_bg),
            removed_bg: parse_hex(&self.removed_bg),
            chart,
            selection_bg: parse_hex(self.selection_bg.as_deref().unwrap_or(&self.accent)),
            on_selection: parse_hex(self.on_selection.as_deref().unwrap_or(if self.is_dark {
                "#ffffff"
            } else {
                "#18181b"
            })),
            syntax_keyword: parse_hex(self.syntax_keyword.as_deref().unwrap_or(&self.accent)),
            syntax_type: parse_hex(self.syntax_type.as_deref().unwrap_or(&self.info)),
            syntax_string: parse_hex(self.syntax_string.as_deref().unwrap_or(&self.success)),
            syntax_comment: parse_hex(self.syntax_comment.as_deref().unwrap_or(&self.text_faint)),
            syntax_number: parse_hex(self.syntax_number.as_deref().unwrap_or(&self.warning)),
            syntax_punctuation: parse_hex(
                self.syntax_punctuation.as_deref().unwrap_or(&self.text_dim),
            ),
        }
    }
}

// Bump this whenever built-in theme colors change: themes.json on disk keeps
// the old values until a version bump force-overwrites the built-in entries.
pub const CURRENT_THEME_VERSION: u32 = 7;

// Built-in themes dropped from the lineup; purged from themes.json on migration.
const RETIRED_THEMES: &[&str] = &[
    "Solarized Dark",
    "Solarized Light",
    "Gruvbox Dark",
    "Gruvbox Light",
];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThemesFile {
    pub version: u32,
    pub themes: BTreeMap<String, ThemeDef>,
}

pub fn get_default_themes() -> Vec<(String, ThemeDef)> {
    vec![
        (
            "One Dark".to_string(),
            ThemeDef {
                is_dark: true,
                bg: "#1b1c1f".to_string(),
                bg_sidebar: "#18191b".to_string(),
                bg_elevated: "#212328".to_string(),
                bg_hover: "#2d3139".to_string(),
                bg_active: "#3a3f4b".to_string(),
                border: "#2d3139".to_string(),
                text: "#e3e6ed".to_string(),
                text_dim: "#9da5b4".to_string(),
                text_faint: "#6b7280".to_string(),
                accent: "#528bff".to_string(),
                success: "#10b981".to_string(),
                warning: "#f59e0b".to_string(),
                error: "#f43f5e".to_string(),
                info: "#06b6d4".to_string(),
                added: "#86efac".to_string(),
                removed: "#fca5a5".to_string(),
                added_bg: "#10502878".to_string(),
                removed_bg: "#5a141478".to_string(),
                chart: vec![
                    "#6366f1".to_string(),
                    "#06b6d4".to_string(),
                    "#10b981".to_string(),
                    "#f59e0b".to_string(),
                    "#ec4899".to_string(),
                    "#8b5cf6".to_string(),
                    "#f43f5e".to_string(),
                    "#a855f7".to_string(),
                ],
                selection_bg: Some("#528bff".to_string()),
                on_selection: Some("#ffffff".to_string()),
                syntax_keyword: Some("#c678dd".to_string()),
                syntax_type: Some("#e5c07b".to_string()),
                syntax_string: Some("#98c379".to_string()),
                syntax_comment: Some("#7f848e".to_string()),
                syntax_number: Some("#d19a66".to_string()),
                syntax_punctuation: Some("#abb2bf".to_string()),
            },
        ),
        (
            "One Light".to_string(),
            ThemeDef {
                is_dark: false,
                bg: "#f3f3f3".to_string(),
                bg_sidebar: "#f0f0f0".to_string(),
                bg_elevated: "#ffffff".to_string(),
                bg_hover: "#e4e4e7".to_string(),
                bg_active: "#d4d4d8".to_string(),
                border: "#e4e4e7".to_string(),
                text: "#18181b".to_string(),
                text_dim: "#71717a".to_string(),
                text_faint: "#a1a1aa".to_string(),
                accent: "#0969da".to_string(),
                success: "#1a7f37".to_string(),
                warning: "#9a6700".to_string(),
                error: "#cf222e".to_string(),
                info: "#0969da".to_string(),
                added: "#1a7f37".to_string(),
                removed: "#cf222e".to_string(),
                added_bg: "#c8e9d0".to_string(),
                removed_bg: "#f9d4d0".to_string(),
                chart: vec![
                    "#4f46e5".to_string(),
                    "#0891b2".to_string(),
                    "#16a34a".to_string(),
                    "#ea580c".to_string(),
                    "#db2777".to_string(),
                    "#7c3aed".to_string(),
                    "#e11d48".to_string(),
                    "#9333ea".to_string(),
                ],
                selection_bg: Some("#cfe2ff".to_string()),
                on_selection: Some("#031633".to_string()),
                syntax_keyword: Some("#a626a4".to_string()),
                syntax_type: Some("#c18401".to_string()),
                syntax_string: Some("#50a14f".to_string()),
                syntax_comment: Some("#a0a1a7".to_string()),
                syntax_number: Some("#986801".to_string()),
                syntax_punctuation: Some("#383a42".to_string()),
            },
        ),
        (
            "Ayu Mirage".to_string(),
            ThemeDef {
                is_dark: true,
                bg: "#1f2430".to_string(),
                bg_sidebar: "#171b24".to_string(),
                bg_elevated: "#232834".to_string(),
                bg_hover: "#2b3242".to_string(),
                bg_active: "#343d4f".to_string(),
                border: "#1a1f29".to_string(),
                text: "#cbccc6".to_string(),
                text_dim: "#707a8c".to_string(),
                text_faint: "#5c6773".to_string(),
                accent: "#ffcc66".to_string(),
                success: "#a6cc70".to_string(),
                warning: "#ffae57".to_string(),
                error: "#f07178".to_string(),
                info: "#5ccfe6".to_string(),
                added: "#a6cc70".to_string(),
                removed: "#f07178".to_string(),
                added_bg: "#2b3a2f78".to_string(),
                removed_bg: "#422b3178".to_string(),
                chart: vec![
                    "#ffae57".to_string(),
                    "#5ccfe6".to_string(),
                    "#a6cc70".to_string(),
                    "#ffcc66".to_string(),
                    "#f07178".to_string(),
                    "#95e6cb".to_string(),
                    "#e6b673".to_string(),
                    "#ff7733".to_string(),
                ],
                selection_bg: Some("#ffcc66".to_string()),
                on_selection: Some("#1f2430".to_string()),
                syntax_keyword: Some("#ff7733".to_string()),
                syntax_type: Some("#ffcc66".to_string()),
                syntax_string: Some("#bae67c".to_string()),
                syntax_comment: Some("#5c6773".to_string()),
                syntax_number: Some("#f29718".to_string()),
                syntax_punctuation: Some("#cbccc6".to_string()),
            },
        ),
        (
            "Ayu Light".to_string(),
            ThemeDef {
                is_dark: false,
                bg: "#f0f2f5".to_string(),
                bg_sidebar: "#f3f4f6".to_string(),
                bg_elevated: "#ffffff".to_string(),
                bg_hover: "#e5e7eb".to_string(),
                bg_active: "#d1d5db".to_string(),
                border: "#e5e7eb".to_string(),
                text: "#5c6773".to_string(),
                text_dim: "#828c9a".to_string(),
                text_faint: "#acb3bf".to_string(),
                accent: "#ff9922".to_string(),
                success: "#6699cc".to_string(),
                warning: "#ffaa33".to_string(),
                error: "#ff3333".to_string(),
                info: "#55b4d4".to_string(),
                added: "#1a7f37".to_string(),
                removed: "#cf222e".to_string(),
                added_bg: "#c8e9d0".to_string(),
                removed_bg: "#f9d4d0".to_string(),
                chart: vec![
                    "#ffaa33".to_string(),
                    "#55b4d4".to_string(),
                    "#6699cc".to_string(),
                    "#ff9922".to_string(),
                    "#ff3333".to_string(),
                    "#86b300".to_string(),
                    "#f07178".to_string(),
                    "#e28905".to_string(),
                ],
                selection_bg: Some("#ff9922".to_string()),
                on_selection: Some("#f8f9fa".to_string()),
                syntax_keyword: Some("#f07178".to_string()),
                syntax_type: Some("#e28905".to_string()),
                syntax_string: Some("#86b300".to_string()),
                syntax_comment: Some("#abb2bf".to_string()),
                syntax_number: Some("#ff9922".to_string()),
                syntax_punctuation: Some("#5c6773".to_string()),
            },
        ),
        (
            "Nord".to_string(),
            ThemeDef {
                is_dark: true,
                bg: "#2e3440".to_string(),
                bg_sidebar: "#242933".to_string(),
                bg_elevated: "#3b4252".to_string(),
                bg_hover: "#434c5e".to_string(),
                bg_active: "#4c566a".to_string(),
                border: "#3b4252".to_string(),
                text: "#d8dee9".to_string(),
                text_dim: "#e5e9f0".to_string(),
                text_faint: "#4c566a".to_string(),
                accent: "#88c0d0".to_string(),
                success: "#a3be8c".to_string(),
                warning: "#ebcb8b".to_string(),
                error: "#bf616a".to_string(),
                info: "#81a1c1".to_string(),
                added: "#a3be8c".to_string(),
                removed: "#bf616a".to_string(),
                added_bg: "#3d4b3d78".to_string(),
                removed_bg: "#4c323578".to_string(),
                chart: vec![
                    "#88c0d0".to_string(),
                    "#81a1c1".to_string(),
                    "#a3be8c".to_string(),
                    "#ebcb8b".to_string(),
                    "#b48ead".to_string(),
                    "#8fbcbb".to_string(),
                    "#bf616a".to_string(),
                    "#5e81ac".to_string(),
                ],
                selection_bg: Some("#88c0d0".to_string()),
                on_selection: Some("#2e3440".to_string()),
                syntax_keyword: Some("#81a1c1".to_string()),
                syntax_type: Some("#8fbcbb".to_string()),
                syntax_string: Some("#a3be8c".to_string()),
                syntax_comment: Some("#4c566a".to_string()),
                syntax_number: Some("#b48ead".to_string()),
                syntax_punctuation: Some("#d8dee9".to_string()),
            },
        ),
        (
            "Catppuccin Mocha".to_string(),
            ThemeDef {
                is_dark: true,
                bg: "#1e1e2e".to_string(),
                bg_sidebar: "#181825".to_string(),
                bg_elevated: "#313244".to_string(),
                bg_hover: "#45475a".to_string(),
                bg_active: "#6c7086".to_string(),
                border: "#45475a".to_string(),
                text: "#cdd6f4".to_string(),
                text_dim: "#a6adc8".to_string(),
                text_faint: "#585b70".to_string(),
                accent: "#89b4fa".to_string(),
                success: "#a6e3a1".to_string(),
                warning: "#f9e2af".to_string(),
                error: "#f38ba8".to_string(),
                info: "#89b4fa".to_string(),
                added: "#a6e3a1".to_string(),
                removed: "#f38ba8".to_string(),
                added_bg: "#1f3a2a78".to_string(),
                removed_bg: "#3a1f2a78".to_string(),
                chart: vec![
                    "#89b4fa".to_string(),
                    "#94e2d5".to_string(),
                    "#a6e3a1".to_string(),
                    "#f9e2af".to_string(),
                    "#fab387".to_string(),
                    "#f5c2e7".to_string(),
                    "#f38ba8".to_string(),
                    "#cba6f7".to_string(),
                ],
                selection_bg: Some("#89b4fa".to_string()),
                on_selection: Some("#1e1e2e".to_string()),
                syntax_keyword: Some("#cba6f7".to_string()),
                syntax_type: Some("#f9e2af".to_string()),
                syntax_string: Some("#a6e3a1".to_string()),
                syntax_comment: Some("#585b70".to_string()),
                syntax_number: Some("#fab387".to_string()),
                syntax_punctuation: Some("#a6adc8".to_string()),
            },
        ),
        (
            "Catppuccin Latte".to_string(),
            ThemeDef {
                is_dark: false,
                bg: "#eff1f5".to_string(),
                bg_sidebar: "#e6e9ef".to_string(),
                bg_elevated: "#ffffff".to_string(),
                bg_hover: "#ccd0da".to_string(),
                bg_active: "#bcc0cc".to_string(),
                border: "#ccd0da".to_string(),
                text: "#4c4f69".to_string(),
                text_dim: "#5c5f77".to_string(),
                text_faint: "#787c94".to_string(),
                accent: "#124bc8".to_string(),
                success: "#40a02b".to_string(),
                warning: "#df8e1d".to_string(),
                error: "#d20f39".to_string(),
                info: "#124bc8".to_string(),
                added: "#40a02b".to_string(),
                removed: "#d20f39".to_string(),
                added_bg: "#e2f1df".to_string(),
                removed_bg: "#f8dbe1".to_string(),
                chart: vec![
                    "#1e66f5".to_string(),
                    "#179299".to_string(),
                    "#40a02b".to_string(),
                    "#df8e1d".to_string(),
                    "#fe640b".to_string(),
                    "#ea76cb".to_string(),
                    "#d20f39".to_string(),
                    "#8839ef".to_string(),
                ],
                selection_bg: Some("#124bc8".to_string()),
                on_selection: Some("#ffffff".to_string()),
                syntax_keyword: Some("#8839ef".to_string()),
                syntax_type: Some("#df8e1d".to_string()),
                syntax_string: Some("#40a02b".to_string()),
                syntax_comment: Some("#787c94".to_string()),
                syntax_number: Some("#fe640b".to_string()),
                syntax_punctuation: Some("#5c5f77".to_string()),
            },
        ),
        (
            "Catppuccin High Contrast".to_string(),
            ThemeDef {
                is_dark: true,
                bg: "#000000".to_string(),
                bg_sidebar: "#0a0a0f".to_string(),
                bg_elevated: "#191923".to_string(),
                bg_hover: "#2a2a3a".to_string(),
                bg_active: "#505064".to_string(),
                border: "#2a2a3a".to_string(),
                text: "#ffffff".to_string(),
                text_dim: "#dcdceb".to_string(),
                text_faint: "#8282a0".to_string(),
                accent: "#64c8ff".to_string(),
                success: "#50ff78".to_string(),
                warning: "#ffeb50".to_string(),
                error: "#ff5a6e".to_string(),
                info: "#64c8ff".to_string(),
                added: "#50ff78".to_string(),
                removed: "#ff5a6e".to_string(),
                added_bg: "#0f3a1e78".to_string(),
                removed_bg: "#3a0f1a78".to_string(),
                chart: vec![
                    "#64c8ff".to_string(),
                    "#50fff0".to_string(),
                    "#50ff78".to_string(),
                    "#ffeb50".to_string(),
                    "#ff8c42".to_string(),
                    "#ff64c8".to_string(),
                    "#ff5a6e".to_string(),
                    "#c864ff".to_string(),
                ],
                selection_bg: Some("#64c8ff".to_string()),
                on_selection: Some("#000000".to_string()),
                syntax_keyword: Some("#c864ff".to_string()),
                syntax_type: Some("#ffeb50".to_string()),
                syntax_string: Some("#50ff78".to_string()),
                syntax_comment: Some("#8282a0".to_string()),
                syntax_number: Some("#ff8c42".to_string()),
                syntax_punctuation: Some("#dcdceb".to_string()),
            },
        ),
    ]
}

/// The pseudo-theme name meaning "follow the OS light/dark setting" — never
/// stored in `themes.json`/`self.themes`, only ever as `prefs.theme`. Resolve
/// it with [`resolve_system_theme_name`] before looking up an actual [`Theme`].
pub const SYSTEM_THEME_NAME: &str = "System";

/// Resolve [`SYSTEM_THEME_NAME`] to a concrete built-in theme name by
/// following the OS's reported light/dark preference (same approach as
/// aero-grep's `Pal::from_theme`). Falls back to the dark variant when the
/// platform doesn't report a preference (e.g. this crate's own test/CI
/// environments, or window managers that don't set it).
pub fn resolve_system_theme_name(ctx: &egui::Context) -> &'static str {
    let is_light = ctx.input(|i| i.raw.system_theme == Some(egui::Theme::Light));
    if is_light {
        "Catppuccin Latte"
    } else {
        "Catppuccin Mocha"
    }
}

pub fn load_themes() -> Vec<(String, Theme)> {
    let config_dir = crate::config::get_config_dir();
    let path = config_dir.join("themes.json");

    let mut themes_file = if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if let Ok(file) = serde_json::from_str::<ThemesFile>(&content) {
            file
        } else if let Ok(map) = serde_json::from_str::<BTreeMap<String, ThemeDef>>(&content) {
            // Upgrade from version 1 (old schema)
            ThemesFile {
                version: 1,
                themes: map,
            }
        } else {
            // Fallback when parsing fails
            let defaults = get_default_themes();
            ThemesFile {
                version: CURRENT_THEME_VERSION,
                themes: defaults.into_iter().collect(),
            }
        }
    } else {
        let defaults = get_default_themes();
        ThemesFile {
            version: CURRENT_THEME_VERSION,
            themes: defaults.into_iter().collect(),
        }
    };

    let mut dirty = false;

    // Version migration
    if themes_file.version < CURRENT_THEME_VERSION {
        // Force-overwrite all built-in themes to pick up any fixes
        let defaults = get_default_themes();
        for (name, def) in defaults {
            themes_file.themes.insert(name, def);
        }
        for name in RETIRED_THEMES {
            themes_file.themes.remove(*name);
        }
        themes_file.version = CURRENT_THEME_VERSION;
        dirty = true;
    } else {
        // Merge new built-in themes without destroying user-added ones
        let defaults = get_default_themes();
        for (name, def) in defaults {
            if let std::collections::btree_map::Entry::Vacant(e) = themes_file.themes.entry(name) {
                e.insert(def);
                dirty = true;
            }
        }
    }

    if (dirty || !path.exists())
        && let Ok(content) = serde_json::to_string_pretty(&themes_file)
    {
        let _ = crate::config::write_atomic(&path, &content);
    }

    let mut result: Vec<(String, Theme)> = themes_file
        .themes
        .into_iter()
        .map(|(name, def)| (name, def.to_theme()))
        .collect();

    // Ensure "One Dark" is first in the list
    if let Some(pos) = result.iter().position(|(name, _)| name == "One Dark") {
        let one_dark = result.remove(pos);
        result.insert(0, one_dark);
    }

    result
}

// Load and configure Japanese (CJK) fonts
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Embedded icon font (VS Code Codicons, MIT license) — same file aero-grep
    // uses. Glyphs are rendered as text via icons::glyph()/icon_rt(), never
    // hand-drawn with Painter line segments (see src/app/widgets.rs).
    fonts.font_data.insert(
        "Icons".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/codicon.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name("Icons".into()),
        vec!["Icons".to_owned()],
    );

    // Candidate paths for system Japanese fonts
    let font_paths = [
        // macOS (JP)
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W6.ttc",
        // macOS (CN/Fallback)
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/AquaKana.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        // Windows (JP)
        "C:\\Windows\\Fonts\\yugothm.ttc",
        "C:\\Windows\\Fonts\\meiryo.ttc",
        "C:\\Windows\\Fonts\\msgothic.ttc",
        // Windows (CN)
        "C:\\Windows\\Fonts\\msyh.ttc",
    ];

    let mut font_data = None;
    for path in &font_paths {
        if std::path::Path::new(path).exists()
            && let Ok(data) = std::fs::read(path)
        {
            font_data = Some(data);
            break;
        }
    }

    // Candidate paths for bold Japanese fonts
    let bold_font_paths = [
        "/System/Library/Fonts/ヒラギノ角ゴシック W6.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W8.ttc",
        "C:\\Windows\\Fonts\\yugothb.ttc",
        "C:\\Windows\\Fonts\\meiryob.ttc",
    ];

    let mut bold_font_data = None;
    for path in &bold_font_paths {
        if std::path::Path::new(path).exists()
            && let Ok(data) = std::fs::read(path)
        {
            bold_font_data = Some(data);
            break;
        }
    }

    if let Some(data) = font_data {
        let tweak = egui::FontTweak {
            y_offset_factor: 0.08,
            ..Default::default()
        };
        fonts.font_data.insert(
            "japanese_font".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(data.clone()).tweak(tweak.clone())),
        );

        let bold_data = bold_font_data.unwrap_or(data);
        fonts.font_data.insert(
            "japanese_bold_font".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bold_data).tweak(tweak)),
        );

        if let Some(vec) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            vec.insert(0, "japanese_font".to_owned());
        }
        // Monospace: keep the built-in monospace font FIRST so ASCII (code,
        // commit graph, hashes) renders crisp; the Japanese font is only a
        // fallback for CJK glyphs. Putting it first rendered all monospace
        // text in a proportional CJK font and looked blurry at small sizes.
        if let Some(vec) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            vec.push("japanese_font".to_owned());
        }

        // Define the bold monospace family (based on the regular monospace family,
        // with japanese_bold_font replacing japanese_font)
        let bold_mono: Vec<String> = fonts
            .families
            .get(&egui::FontFamily::Monospace)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|f| {
                if f == "japanese_font" {
                    "japanese_bold_font".to_owned()
                } else {
                    f
                }
            })
            .collect();
        fonts
            .families
            .insert(egui::FontFamily::Name("monospace_bold".into()), bold_mono);
    }

    // Always apply — not just when a Japanese font was found. This used to
    // live inside the `if let Some(data) = font_data` block above, so on any
    // system with none of the hardcoded CJK font paths (e.g. Linux without
    // those exact fonts installed) `set_fonts` was never called at all: the
    // embedded Icons font registered above silently never reached the
    // context, and egui panics the moment anything renders a Codicon glyph.
    ctx.set_fonts(fonts);
}

pub fn apply_visuals(ctx: &egui::Context, theme: &Theme) {
    let mut visuals = if theme.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = theme.bg_sidebar;
    visuals.widgets.noninteractive.bg_fill = theme.bg;
    visuals.widgets.noninteractive.weak_bg_fill = theme.bg_elevated;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme.border);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, theme.text);

    // Inactive state (normal button). bg_hover (not bg_elevated) as the fill so
    // buttons stand out from elevated cards — on light themes both were white.
    visuals.widgets.inactive.bg_fill = theme.bg_hover;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, theme.border);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, theme.text);

    // Hover state
    visuals.widgets.hovered.bg_fill = theme.bg_active;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, theme.text);

    // Active (pressed) state
    visuals.widgets.active.bg_fill = theme.bg_active;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, theme.text);

    // Selection highlight
    visuals.selection.bg_fill = theme.selection_bg;

    // Unify rounding to a flat 4.0 (ZED-style flat corners)
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);

    ctx.set_visuals(visuals);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_theme_json(use_old_keys: bool) -> String {
        if use_old_keys {
            r##"{
                "is_dark": true,
                "bg": "#1b1c1f",
                "bg_sidebar": "#18191b",
                "bg_elevated": "#212328",
                "bg_hover": "#2d3139",
                "bg_active": "#3a3f4b",
                "border": "#2d3139",
                "text": "#e3e6ed",
                "text_dim": "#9da5b4",
                "text_faint": "#6b7280",
                "accent": "#528bff",
                "success": "#10b981",
                "warning": "#f59e0b",
                "error": "#f43f5e",
                "info": "#06b6d4",
                "added": "#86efac",
                "removed": "#fca5a5",
                "added_bg": "#10502878",
                "removed_bg": "#5a141478",
                "chart": ["#6366f1","#06b6d4","#10b981","#f59e0b","#ec4899","#8b5cf6","#f43f5e","#a855f7"]
            }"##
            .to_string()
        } else {
            r##"{
                "is_dark": true,
                "bg_base": "#1b1c1f",
                "bg_mantle": "#18191b",
                "bg_surface0": "#212328",
                "bg_surface1": "#2d3139",
                "bg_active": "#3a3f4b",
                "border": "#2d3139",
                "text": "#e3e6ed",
                "subtext": "#9da5b4",
                "muted": "#6b7280",
                "accent": "#528bff",
                "green": "#10b981",
                "yellow": "#f59e0b",
                "red": "#f43f5e",
                "info": "#06b6d4",
                "added": "#86efac",
                "removed": "#fca5a5",
                "added_bg": "#10502878",
                "removed_bg": "#5a141478",
                "chart": ["#6366f1","#06b6d4","#10b981","#f59e0b","#ec4899","#8b5cf6","#f43f5e","#a855f7"]
            }"##
            .to_string()
        }
    }

    #[test]
    fn old_keys_parse_via_alias() {
        let json = minimal_theme_json(true);
        let def: ThemeDef = serde_json::from_str(&json).expect("old-key JSON must parse");
        assert_eq!(def.bg, "#1b1c1f");
        assert_eq!(def.bg_sidebar, "#18191b");
        assert_eq!(def.bg_elevated, "#212328");
        assert_eq!(def.bg_hover, "#2d3139");
        assert_eq!(def.text_dim, "#9da5b4");
        assert_eq!(def.text_faint, "#6b7280");
        assert_eq!(def.warning, "#f59e0b");
        assert_eq!(def.success, "#10b981");
        assert_eq!(def.error, "#f43f5e");
    }

    #[test]
    fn new_keys_roundtrip() {
        let json = minimal_theme_json(false);
        let def: ThemeDef = serde_json::from_str(&json).expect("new-key JSON must parse");
        let serialized = serde_json::to_string(&def).expect("must serialize");
        let def2: ThemeDef = serde_json::from_str(&serialized).expect("re-parse must succeed");
        assert_eq!(def.bg, def2.bg);
        assert_eq!(def.bg_sidebar, def2.bg_sidebar);
        assert_eq!(def.bg_elevated, def2.bg_elevated);
        assert_eq!(def.bg_hover, def2.bg_hover);
        assert_eq!(def.text_dim, def2.text_dim);
        assert_eq!(def.text_faint, def2.text_faint);
        assert_eq!(def.warning, def2.warning);
        assert_eq!(def.success, def2.success);
        assert_eq!(def.error, def2.error);
        // Verify the serialized form uses new keys
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(v.get("bg_base").is_some(), "serialized key must be bg_base");
        assert!(v.get("bg_mantle").is_some());
        assert!(v.get("bg_surface0").is_some());
        assert!(v.get("bg_surface1").is_some());
        assert!(v.get("subtext").is_some());
        assert!(v.get("muted").is_some());
        assert!(v.get("yellow").is_some());
        assert!(v.get("green").is_some());
        assert!(v.get("red").is_some());
        assert!(v.get("bg").is_none(), "old key bg must not appear");
    }
}
