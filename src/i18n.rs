use crate::config::Language;

/// Translate a key for a given Language. Returns the localized string if found,
/// or the key itself as fallback.
#[allow(dead_code)]
pub fn t(lang: Language, key: &str) -> &str {
    match (lang, key) {
        // Common UI
        (Language::English, "ok") => "OK",
        (Language::Japanese, "ok") => "OK",
        (Language::English, "cancel") => "Cancel",
        (Language::Japanese, "cancel") => "キャンセル",
        (Language::English, "save") => "Save",
        (Language::Japanese, "save") => "保存",
        (Language::English, "close") => "Close",
        (Language::Japanese, "close") => "閉じる",
        (Language::English, "settings") => "Settings",
        (Language::Japanese, "settings") => "設定",
        (Language::English, "language") => "Language",
        (Language::Japanese, "language") => "言語",
        (Language::English, "theme_label") => "Theme:",
        (Language::Japanese, "theme_label") => "テーマ:",
        (Language::English, "english") => "English",
        (Language::Japanese, "english") => "English",
        (Language::English, "japanese") => "Japanese",
        (Language::Japanese, "japanese") => "日本語",

        // Navigation / Tabs
        (Language::English, "dashboard") => "Dashboard",
        (Language::Japanese, "dashboard") => "ダッシュボード",
        (Language::English, "diff") => "Diff",
        (Language::Japanese, "diff") => "差分",
        (Language::English, "general") => "General",
        (Language::Japanese, "general") => "全般",
        (Language::English, "appearance") => "Appearance",
        (Language::Japanese, "appearance") => "外観設定",
        (Language::English, "members") => "Members",
        (Language::Japanese, "members") => "メンバー登録",
        (Language::English, "about") => "About",
        (Language::Japanese, "about") => "情報",

        // Settings View Keys
        (Language::English, "open_config_folder") => "Open Config Folder",
        (Language::Japanese, "open_config_folder") => "設定フォルダを開く",
        (Language::English, "open_config_folder_tooltip") => {
            "Directly edit config.json / members.json / themes.json / tech_rules.json\n(Changes take effect upon app restart)"
        }
        (Language::Japanese, "open_config_folder_tooltip") => {
            "config.json / members.json / themes.json / tech_rules.json を直接編集できます\n（編集内容はアプリ再起動で反映されます）"
        }
        (Language::English, "tab_repos") => "Repositories",
        (Language::Japanese, "tab_repos") => "リポジトリ管理",
        (Language::English, "tab_members") => "Members",
        (Language::Japanese, "tab_members") => "メンバー管理",
        (Language::English, "tab_appearance") => "Appearance",
        (Language::Japanese, "tab_appearance") => "外観設定",
        (Language::English, "display_language") => "Display Language",
        (Language::Japanese, "display_language") => "表示言語",
        (Language::English, "select_display_language") => "Select application display language.",
        (Language::Japanese, "select_display_language") => "アプリの表示言語を選択します。",
        (Language::English, "add_repo") => "Add New Repository",
        (Language::Japanese, "add_repo") => "新規リポジトリの登録",
        (Language::English, "add_member") => "Add New Member",
        (Language::Japanese, "add_member") => "新規メンバーの登録",
        (Language::English, "registered_repos") => "Registered Repositories",
        (Language::Japanese, "registered_repos") => "登録済みリポジトリ",
        (Language::English, "registered_members") => "Registered Members",
        (Language::Japanese, "registered_members") => "登録済みメンバー",
        (Language::English, "theme_settings") => "Appearance & Theme Settings",
        (Language::Japanese, "theme_settings") => "外観とテーマの設定",
        (Language::English, "editor_settings") => "Editor Integration Settings",
        (Language::Japanese, "editor_settings") => "エディタ連携の設定",
        (Language::English, "editor_command") => "Editor Command:",
        (Language::Japanese, "editor_command") => "エディタコマンド:",
        (Language::English, "display_name") => "Display Name",
        (Language::Japanese, "display_name") => "表示名",
        (Language::English, "display_name_hint") => "e.g., my-project",
        (Language::Japanese, "display_name_hint") => "例: my-project",
        (Language::English, "path_label") => "Path",
        (Language::Japanese, "path_label") => "パス",
        (Language::English, "remote_repo_hint") => {
            "You can enter a local path or a remote (SSH) repository URL"
        }
        (Language::Japanese, "remote_repo_hint") => {
            "ローカルパスまたはリモート(SSH)リポジトリのURLを入力できます"
        }
        (Language::English, "path_hint") => {
            "/home/user/repos/my-project or ssh://user@host/path/to/repo"
        }
        (Language::Japanese, "path_hint") => {
            "/home/user/repos/my-project または ssh://user@host/path/to/repo"
        }
        (Language::English, "browse_btn") => "Browse...",
        (Language::Japanese, "browse_btn") => "選択...",
        (Language::English, "browse_folder_hint") => {
            "Select multiple folders (display names will default to folder names)"
        }
        (Language::Japanese, "browse_folder_hint") => {
            "複数フォルダをまとめて選択できます（表示名はフォルダ名になります）"
        }
        (Language::English, "already_registered") => "(Already registered)",
        (Language::Japanese, "already_registered") => "(登録済み)",
        (Language::English, "not_a_git_repo") => "(Not a Git repository)",
        (Language::Japanese, "not_a_git_repo") => "(Gitリポジトリではない)",
        (Language::English, "added_success") => "✓ Added",
        (Language::Japanese, "added_success") => "✓ 追加しました",
        (Language::English, "add_repo_btn") => "+ Add Repository",
        (Language::Japanese, "add_repo_btn") => "+ リポジトリを追加",
        (Language::English, "err_enter_name_and_path") => "Please enter a display name and path.",
        (Language::Japanese, "err_enter_name_and_path") => "表示名とパスを入力してください。",
        (Language::English, "err_ssh_unreachable") => {
            "Cannot reach repository via SSH. Check key authentication, hostname, and remote path."
        }
        (Language::Japanese, "err_ssh_unreachable") => {
            "SSH経由でリポジトリに到達できません。鍵認証・ホスト名・リモートのパスを確認してください。"
        }
        (Language::English, "err_invalid_git_repo") => "Not a valid Git repository directory.",
        (Language::Japanese, "err_invalid_git_repo") => {
            "有効なGitリポジトリのディレクトリではありません。"
        }
        (Language::English, "no_registered_repos") => "No repositories registered",
        (Language::Japanese, "no_registered_repos") => "リポジトリが登録されていません",
        (Language::English, "save_btn") => "Save",
        (Language::Japanese, "save_btn") => "保存",
        (Language::English, "cancel_btn") => "Cancel",
        (Language::Japanese, "cancel_btn") => "キャンセル",
        (Language::English, "confirm_delete_repo") => {
            "Are you sure you want to delete this repository?"
        }
        (Language::Japanese, "confirm_delete_repo") => "本当にこのリポジトリを削除しますか？",
        (Language::English, "yes_delete") => "Yes, Delete",
        (Language::Japanese, "yes_delete") => "はい、削除する",
        (Language::English, "delete_action") => "Delete",
        (Language::Japanese, "delete_action") => "削除",
        (Language::English, "edit_action") => "Edit",
        (Language::Japanese, "edit_action") => "編集",
        (Language::English, "alias_label") => "Aliases",
        (Language::Japanese, "alias_label") => "別名",
        (Language::English, "member_name_hint") => "e.g., Tanaka",
        (Language::Japanese, "member_name_hint") => "例: 田中",
        (Language::English, "member_alias_hint") => "Tanaka, tanaka@example.com (comma-separated)",
        (Language::Japanese, "member_alias_hint") => "Tanaka, tanaka@example.com（カンマ区切り）",
        (Language::English, "active_member_checkbox") => "Active project member",
        (Language::Japanese, "active_member_checkbox") => "プロジェクトに在籍中",
        (Language::English, "add_member_btn") => "+ Add Member",
        (Language::Japanese, "add_member_btn") => "+ メンバーを追加",
        (Language::English, "err_enter_canonical_name") => "Please enter a display name.",
        (Language::Japanese, "err_enter_canonical_name") => "代表名を入力してください。",
        (Language::English, "no_registered_members") => "No members registered",
        (Language::Japanese, "no_registered_members") => "メンバーが登録されていません",
        (Language::English, "confirm_delete_member") => {
            "Are you sure you want to delete this member?"
        }
        (Language::Japanese, "confirm_delete_member") => "本当にこのメンバーを削除しますか？",
        (Language::English, "no_aliases") => "No aliases",
        (Language::Japanese, "no_aliases") => "別名なし",
        (Language::English, "is_active_checkbox") => "Active",
        (Language::Japanese, "is_active_checkbox") => "在籍中",
        (Language::English, "theme_card_desc") => {
            "Select application color theme. Changes apply immediately and are saved to preferences."
        }
        (Language::Japanese, "theme_card_desc") => {
            "アプリのカラーテーマを選択します。クリックすると即座に反映され、設定が保存されます。"
        }
        (Language::English, "os_sync_label") => "[Follow OS]",
        (Language::Japanese, "os_sync_label") => "[OS追従]",
        (Language::English, "dark_label") => "[Dark]",
        (Language::Japanese, "dark_label") => "[ダーク]",
        (Language::English, "light_label") => "[Light]",
        (Language::Japanese, "light_label") => "[ライト]",
        (Language::English, "swatch_bg") => "Background",
        (Language::Japanese, "swatch_bg") => "背景",
        (Language::English, "swatch_sidebar") => "Sidebar",
        (Language::Japanese, "swatch_sidebar") => "サイドバー",
        (Language::English, "swatch_card") => "Card",
        (Language::Japanese, "swatch_card") => "カード",
        (Language::English, "swatch_text") => "Text",
        (Language::Japanese, "swatch_text") => "文字",
        (Language::English, "swatch_accent") => "Accent",
        (Language::Japanese, "swatch_accent") => "アクセント",
        (Language::English, "swatch_success") => "Success",
        (Language::Japanese, "swatch_success") => "成功",
        (Language::English, "swatch_error") => "Error",
        (Language::Japanese, "swatch_error") => "エラー",
        (Language::English, "editor_settings_desc") => {
            "Configure editor command for opening repositories and files."
        }
        (Language::Japanese, "editor_settings_desc") => {
            "リポジトリやファイルを開くためのエディタコマンドを設定します。"
        }
        (Language::English, "editor_hint") => "e.g., code, zed, cursor, nvim, vim",
        (Language::Japanese, "editor_hint") => "例: code, zed, cursor, nvim, vim",
        (Language::English, "editor_note") => {
            "※ The command must be available in your system PATH. Extra arguments can be included (e.g., `code -r`)."
        }
        (Language::Japanese, "editor_note") => {
            "※ コマンドはシステムの PATH に通っている必要があります。引数を含めることも可能です (例: `code -r`)。"
        }
        (Language::English, "tech_stack") => "Tech Stack Rules",
        (Language::Japanese, "tech_stack") => "技術スタック",
        (Language::English, "keyboard_shortcuts") => "Keyboard Shortcuts",
        (Language::Japanese, "keyboard_shortcuts") => "キーボードショートカット",
        (Language::English, "shortcut_esc") => "Back to previous screen / close search & dropdowns",
        (Language::Japanese, "shortcut_esc") => "前の画面へ戻る / 検索・選択を閉じる",
        (Language::English, "shortcut_tabs") => "Switch dashboard view tabs",
        (Language::Japanese, "shortcut_tabs") => "ダッシュボードのタブ切替",
        (Language::English, "shortcut_hunks") => "Jump to next/previous hunk in diff view",
        (Language::Japanese, "shortcut_hunks") => "差分画面でハンク間ジャンプ（次/前）",
        (Language::English, "shortcut_search") => "In-page text search in diff view",
        (Language::Japanese, "shortcut_search") => "差分画面内テキスト検索",
        (Language::English, "config_file_location") => "Config Storage Path",
        (Language::Japanese, "config_file_location") => "設定ファイルの場所",

        // Dashboard View Keys
        (Language::English, "30日") => "30 Days",
        (Language::Japanese, "30日") => "30日",
        (Language::English, "90日") => "90 Days",
        (Language::Japanese, "90日") => "90日",
        (Language::English, "180日") => "180 Days",
        (Language::Japanese, "180日") => "180日",
        (Language::English, "compare_btn") => "Compare",
        (Language::Japanese, "compare_btn") => "比較",
        (Language::English, "compare_hint") => "Show diff from selected base -> target",
        (Language::Japanese, "compare_hint") => "選択した base → target の差分を表示",
        (Language::English, "select_base_target_hint") => "Select base / target with ●",
        (Language::Japanese, "select_base_target_hint") => "●で base / target を選択",
        (Language::English, "clear_selection_hint") => "Clear selection",
        (Language::Japanese, "clear_selection_hint") => "選択をクリア",
        (Language::English, "back_to_home") => "Back to Home",
        (Language::Japanese, "back_to_home") => "ホームへ戻る",
        (Language::English, "home") => "Home",
        (Language::Japanese, "home") => "ホーム",
        (Language::English, "refresh_status") => "Refresh status",
        (Language::Japanese, "refresh_status") => "最新状態に更新",
        (Language::English, "syncing") => "Syncing",
        (Language::Japanese, "syncing") => "更新中",
        (Language::English, "run_pull") => "Run Pull",
        (Language::Japanese, "run_pull") => "Pullを実行",
        (Language::English, "sync_ok") => "✓ Synced",
        (Language::Japanese, "sync_ok") => "✓ 同期済",
        (Language::English, "sync_fail") => "✗ Failed",
        (Language::Japanese, "sync_fail") => "✗ 失敗",
        (Language::English, "sync_error_prefix") => "Sync Error:",
        (Language::Japanese, "sync_error_prefix") => "同期エラー:",
        (Language::English, "tab_overview") => "Overview",
        (Language::Japanese, "tab_overview") => "概要",
        (Language::English, "tab_branches") => "Branches",
        (Language::Japanese, "tab_branches") => "ブランチ",
        (Language::English, "tab_contributors") => "Contributors",
        (Language::Japanese, "tab_contributors") => "貢献者",
        (Language::English, "tab_files") => "Files",
        (Language::Japanese, "tab_files") => "ファイル",
        (Language::English, "tab_activity") => "Activity",
        (Language::Japanese, "tab_activity") => "アクティビティ",
        (Language::English, "stat_commits") => "Commits",
        (Language::Japanese, "stat_commits") => "コミット",
        (Language::English, "stat_authors") => "Contributors",
        (Language::Japanese, "stat_authors") => "開発者",
        (Language::English, "stat_branches") => "Branches",
        (Language::Japanese, "stat_branches") => "ブランチ",
        (Language::English, "stat_files") => "Files",
        (Language::Japanese, "stat_files") => "ファイル",
        (Language::English, "stashes") => "📂 Stashes",
        (Language::Japanese, "stashes") => "📂 スタッシュ",
        (Language::English, "apply_stash") => "Apply",
        (Language::Japanese, "apply_stash") => "適用",
        (Language::English, "drop_stash") => "Drop",
        (Language::Japanese, "drop_stash") => "削除",
        (Language::English, "view_diff") => "Diff",
        (Language::Japanese, "view_diff") => "差分",
        (Language::English, "active_filter_tooltip") => {
            "Only shows members marked as Active in Settings"
        }
        (Language::Japanese, "active_filter_tooltip") => {
            "設定で「在籍中」に設定した登録メンバーのみ表示します"
        }
        (Language::English, "slice_other") => "Other",
        (Language::Japanese, "slice_other") => "その他",
        (Language::English, "close_details_hint") => "Close details",
        (Language::Japanese, "close_details_hint") => "詳細を閉じる",
        (Language::English, "view_commit_diff") => "View Diff",
        (Language::Japanese, "view_commit_diff") => "差分を見る",
        (Language::English, "view_commit_diff_hint") => "View this commit's changes in diff view",
        (Language::Japanese, "view_commit_diff_hint") => "このコミットの変更を差分ビューで表示",
        (Language::English, "fetching_commit_details") => "Fetching commit details...",
        (Language::Japanese, "fetching_commit_details") => "コミット詳細を取得中...",
        (Language::English, "no_changes") => "No changes",
        (Language::Japanese, "no_changes") => "変更なし",
        (Language::English, "branches_header") => "Branches",
        (Language::Japanese, "branches_header") => "ブランチ",
        (Language::English, "filter_branches_hint") => "Filter branches...",
        (Language::Japanese, "filter_branches_hint") => "ブランチを絞り込み...",
        (Language::English, "tags_header") => "Tags",
        (Language::Japanese, "tags_header") => "タグ",
        (Language::English, "no_tags") => "No tags",
        (Language::Japanese, "no_tags") => "タグなし",
        (Language::English, "tag_select_hint") => "● Left: select base  ● Right: select target",
        (Language::Japanese, "tag_select_hint") => "●左: base選択  ●右: target選択",
        (Language::English, "commit_graph") => "Commit Graph",
        (Language::Japanese, "commit_graph") => "コミットグラフ",
        (Language::English, "commit_click_hint") => {
            "Click: view single commit diff  ● Left: select base  ● Right: select target for range diff"
        }
        (Language::Japanese, "commit_click_hint") => {
            "クリック: 1コミットの変更を表示  ●左: base選択  ●右: target選択で範囲差分"
        }
        (Language::English, "loading_commits") => "Loading...",
        (Language::Japanese, "loading_commits") => "読み込み中...",
        (Language::English, "load_more_commits") => "Load more",
        (Language::Japanese, "load_more_commits") => "さらに読み込む",
        (Language::English, "fetching_commit_graph") => "Fetching commit graph...",
        (Language::Japanese, "fetching_commit_graph") => "コミットグラフを取得中...",
        (Language::English, "files_unit") => "items",
        (Language::Japanese, "files_unit") => "件",
        (Language::English, "commit_details") => "Commit Details",
        (Language::Japanese, "commit_details") => "コミット詳細",
        (Language::English, "contributor_ranking") => "Contributor Ranking",
        (Language::Japanese, "contributor_ranking") => "貢献者ランキング",
        (Language::English, "active_members_only") => "Active members only",
        (Language::Japanese, "active_members_only") => "在籍メンバーのみ",
        (Language::English, "readme_not_found") => "README not found",
        (Language::Japanese, "readme_not_found") => "README が見つかりません",
        (Language::English, "period") => "Period:",
        (Language::Japanese, "period") => "期間:",
        (Language::English, "branch_tag_list") => "Branches & Tags",
        (Language::Japanese, "branch_tag_list") => "ブランチ・タグ一覧",
        (Language::English, "file_structure") => "File Structure",
        (Language::Japanese, "file_structure") => "ファイル構成",
        (Language::English, "top_large_files") => "Top 10 Largest Files",
        (Language::Japanese, "top_large_files") => "容量の大きいファイル TOP 10",
        (Language::English, "active") => "Active",
        (Language::Japanese, "active") => "在籍中",
        (Language::English, "inactive") => "Inactive",
        (Language::Japanese, "inactive") => "非在籍",
        (Language::English, "unregistered") => "Unregistered",
        (Language::Japanese, "unregistered") => "未登録",
        (Language::English, "all_history") => "All Time",
        (Language::Japanese, "all_history") => "全期間",
        (Language::English, "past_year") => "Past 1 Year",
        (Language::Japanese, "past_year") => "過去1年間",
        (Language::English, "commits") => "Commits",
        (Language::Japanese, "commits") => "コミット数",
        (Language::English, "share") => "Share",
        (Language::Japanese, "share") => "貢献割合",
        (Language::English, "rank") => "Rank",
        (Language::Japanese, "rank") => "順位",
        (Language::English, "canonical_name") => "Name",
        (Language::Japanese, "canonical_name") => "代表名",
        (Language::English, "file_path") => "File Path",
        (Language::Japanese, "file_path") => "ファイルパス",
        (Language::English, "file_size") => "File Size",
        (Language::Japanese, "file_size") => "ファイルサイズ",
        (Language::English, "file_count") => "Files",
        (Language::Japanese, "file_count") => "ファイル数",
        (Language::English, "extension") => "Extension",
        (Language::Japanese, "extension") => "拡張子",

        // Diff & App Frame Keys
        (Language::English, "search_placeholder") => "Search diff (F3 / Enter)...",
        (Language::Japanese, "search_placeholder") => "差分を検索 (F3 / Enter)...",
        (Language::English, "search_commit_hint") => "Search hash / author / message / tag...",
        (Language::Japanese, "search_commit_hint") => "ハッシュ / 作者 / メッセージ / タグ名で検索",
        (Language::English, "search_diff_hint") => "Search in diff...",
        (Language::Japanese, "search_diff_hint") => "差分内を検索...",
        (Language::English, "search_command_hint") => "Search commands...",
        (Language::Japanese, "search_command_hint") => "コマンドを検索...",
        (Language::English, "command_palette") => "Command Palette",
        (Language::Japanese, "command_palette") => "コマンドパレット",
        (Language::English, "toggle_theme") => "Toggle Theme",
        (Language::Japanese, "toggle_theme") => "テーマ切り替え",

        // App Mod & Widgets Keys
        (Language::English, "app_description") => {
            "A dashboard for visualizing Git repository commits, branches, and contributors"
        }
        (Language::Japanese, "app_description") => {
            "Git リポジトリのコミット・ブランチ・貢献者を可視化するダッシュボード"
        }
        (Language::English, "no_data") => "No data available",
        (Language::Japanese, "no_data") => "データがありません",
        (Language::English, "unknown_error") => "Unknown error",
        (Language::Japanese, "unknown_error") => "不明なエラー",
        (Language::English, "palette_open_settings") => "Open Settings",
        (Language::Japanese, "palette_open_settings") => "設定を開く",
        (Language::English, "palette_expand_sidebar") => "Expand Sidebar",
        (Language::Japanese, "palette_expand_sidebar") => "サイドバーを展開",
        (Language::English, "palette_collapse_sidebar") => "Collapse Sidebar",
        (Language::Japanese, "palette_collapse_sidebar") => "サイドバーを折りたたむ",
        (Language::English, "palette_pull_selected") => "Pull selected repository",
        (Language::Japanese, "palette_pull_selected") => "選択中のリポジトリをPull",
        (Language::English, "palette_theme_system") => "Theme: System (Follow OS)",
        (Language::Japanese, "palette_theme_system") => "テーマ: System (OSに追従)",
        (Language::English, "toast_stash_applied") => "Stash applied.",
        (Language::Japanese, "toast_stash_applied") => "スタッシュを適用しました。",
        (Language::English, "toast_stash_dropped") => "Stash dropped.",
        (Language::Japanese, "toast_stash_dropped") => "スタッシュを削除しました。",
        (Language::English, "editor_err_ssh") => {
            "Remote (SSH) repositories cannot be opened in a local editor."
        }
        (Language::Japanese, "editor_err_ssh") => {
            "リモート(SSH)リポジトリはローカルのエディタでは開けません。"
        }
        (Language::English, "editor_err_not_set") => {
            "Editor command is not configured. Please set it in Settings."
        }
        (Language::Japanese, "editor_err_not_set") => {
            "エディタコマンドが設定されていません。設定画面で設定してください。"
        }
        (Language::English, "editor_err_empty") => "Editor command is empty.",
        (Language::Japanese, "editor_err_empty") => "エディタコマンドが空です。",
        (Language::English, "filter_placeholder") => "Filter...",
        (Language::Japanese, "filter_placeholder") => "フィルター...",
        (Language::English, "follow_os_theme_tooltip") => {
            "Automatically follow OS light/dark theme"
        }
        (Language::Japanese, "follow_os_theme_tooltip") => "OSのライト/ダーク設定に自動追従",
        (Language::English, "analyzing_repo_data") => "Analyzing repository data...",
        (Language::Japanese, "analyzing_repo_data") => "リポジトリデータを分析中...",
        (Language::English, "analysis_error") => "Analysis Error",
        (Language::Japanese, "analysis_error") => "分析エラー",
        (Language::English, "retry_btn") => "Retry",
        (Language::Japanese, "retry_btn") => "再試行",
        (Language::English, "drop_stash_window_title") => "Drop Stash",
        (Language::Japanese, "drop_stash_window_title") => "スタッシュの削除",
        (Language::English, "no_matching_commands") => "No matching commands",
        (Language::Japanese, "no_matching_commands") => "一致するコマンドがありません",

        // Contributor Quick-Registration Keys
        (Language::English, "action_col") => "Actions",
        (Language::Japanese, "action_col") => "操作",
        (Language::English, "btn_add_member") => "Register as new member",
        (Language::Japanese, "btn_add_member") => "新規メンバーとして登録",
        (Language::English, "btn_add_alias") => "Add as alias to existing member",
        (Language::Japanese, "btn_add_alias") => "既存メンバーの別名として追加",
        (Language::English, "modal_add_member_title") => "Register New Member",
        (Language::Japanese, "modal_add_member_title") => "新規メンバー登録",
        (Language::English, "modal_add_alias_title") => "Add Alias to Existing Member",
        (Language::Japanese, "modal_add_alias_title") => "既存メンバーへの別名追加",
        (Language::English, "select_target_member") => "Select Target Member",
        (Language::Japanese, "select_target_member") => "登録先メンバーの選択",
        (Language::English, "contributor_info") => "Contributor Info",
        (Language::Japanese, "contributor_info") => "対象貢献者",
        (Language::English, "alias_added_toast") => "Added alias to member.",
        (Language::Japanese, "alias_added_toast") => "メンバーに別名を追加しました。",
        (Language::English, "member_registered_toast") => "Registered new member.",
        (Language::Japanese, "member_registered_toast") => "新規メンバーを登録しました。",

        // Diff View Keys
        (Language::English, "uncommitted_changes_working") => "Uncommitted Changes (Working Tree)",
        (Language::Japanese, "uncommitted_changes_working") => "未コミットの変更 (作業中)",
        (Language::English, "fetching_commits") => "Fetching commit list...",
        (Language::Japanese, "fetching_commits") => "コミット一覧を取得中...",
        (Language::English, "earlier_commit_tags") => "Tags on earlier commits",
        (Language::Japanese, "earlier_commit_tags") => "これより前のコミットのタグ",
        (Language::English, "back_to_dashboard") => "Back to Dashboard",
        (Language::Japanese, "back_to_dashboard") => "ダッシュボードへ戻る",
        (Language::English, "exit_fullscreen") => "Exit Fullscreen",
        (Language::Japanese, "exit_fullscreen") => "全画面表示を解除",
        (Language::English, "enter_fullscreen") => "Enter Fullscreen",
        (Language::Japanese, "enter_fullscreen") => "差分を全画面で表示",
        (Language::English, "range_compare") => "Range Compare",
        (Language::Japanese, "range_compare") => "範囲比較",
        (Language::English, "single_commit_changes") => "Single Commit Changes",
        (Language::Japanese, "single_commit_changes") => "1コミットの変更",
        (Language::English, "select_placeholder") => "Please select...",
        (Language::Japanese, "select_placeholder") => "選択してください",
        (Language::English, "swap_base_target") => "Swap base and target",
        (Language::Japanese, "swap_base_target") => "base と target を入れ替え",
        (Language::English, "working_tree_hover") => "Uncommitted changes (working copy)",
        (Language::Japanese, "working_tree_hover") => "未コミットの変更 (作業コピー)",
        (Language::English, "working_tree") => "Working Tree",
        (Language::Japanese, "working_tree") => "作業ツリー",
        (Language::English, "re_run_compare") => "Re-run comparison",
        (Language::Japanese, "re_run_compare") => "比較を再実行",
        (Language::English, "merge_base_checkbox") => "From common ancestor",
        (Language::Japanese, "merge_base_checkbox") => "共通祖先から",
        (Language::English, "merge_base_tooltip") => {
            "base...target (GitHub compare style):\nShows changes in target since its common ancestor with base"
        }
        (Language::Japanese, "merge_base_tooltip") => {
            "base...target（GitHub compare方式）:\nbaseとの共通祖先以降の target 側の変更のみを表示します"
        }
        (Language::English, "history_menu") => "History",
        (Language::Japanese, "history_menu") => "履歴",
        (Language::English, "reversed_diff_warning") => {
            "⚠ base is newer than target (reversed comparison)"
        }
        (Language::Japanese, "reversed_diff_warning") => {
            "⚠ base が target より新しいため逆方向の差分です"
        }
        (Language::English, "swap_link") => "Swap",
        (Language::Japanese, "swap_link") => "入れ替える",
        (Language::English, "select_commits_to_compare") => "Please select commits to compare",
        (Language::Japanese, "select_commits_to_compare") => "比較するコミットを選択してください",
        (Language::English, "show_file_list") => "Show file list",
        (Language::Japanese, "show_file_list") => "ファイル一覧を表示",
        (Language::English, "changed_files_header") => "Changed Files",
        (Language::Japanese, "changed_files_header") => "変更ファイル",
        (Language::English, "hide_file_list_tooltip") => "Hide file list for wider diff view",
        (Language::Japanese, "hide_file_list_tooltip") => "ファイル一覧を隠して差分を広く表示",
        (Language::English, "select_file_hint") => "Select a file on the left to view diff",
        (Language::Japanese, "select_file_hint") => "左のファイルを選択すると差分を表示します",
        (Language::English, "fetching_diff") => "Fetching diff...",
        (Language::Japanese, "fetching_diff") => "差分を取得中...",
        (Language::English, "copy_path") => "Copy Path",
        (Language::Japanese, "copy_path") => "パスをコピー",
        (Language::English, "binary_file_no_diff") => "Binary file, cannot display diff",
        (Language::Japanese, "binary_file_no_diff") => "バイナリファイルのため差分を表示できません",
        (Language::English, "copy_btn") => "Copy",
        (Language::Japanese, "copy_btn") => "コピー",
        (Language::English, "copy_path_tooltip") => "Copy file path",
        (Language::Japanese, "copy_path_tooltip") => "ファイルパスをコピー",
        (Language::English, "prev_next_file_hint") => "Use [ / ] keys to move to prev/next file",
        (Language::Japanese, "prev_next_file_hint") => "[ / ] キーで前後のファイルへ移動",
        (Language::English, "full_file_checkbox") => "Full file",
        (Language::Japanese, "full_file_checkbox") => "全体表示",
        (Language::English, "ignore_ws_checkbox") => "Ignore whitespace",
        (Language::Japanese, "ignore_ws_checkbox") => "空白無視",
        (Language::English, "no_diff") => "No diff available",
        (Language::Japanese, "no_diff") => "差分はありません",
        (Language::English, "old_label_prefix") => "Old: ",
        (Language::Japanese, "old_label_prefix") => "旧  ",
        (Language::English, "new_label_prefix") => "New: ",
        (Language::Japanese, "new_label_prefix") => "新  ",
        (Language::English, "copy_old_line") => "Copy old line",
        (Language::Japanese, "copy_old_line") => "旧側の行をコピー",
        (Language::English, "copy_new_line") => "Copy new line",
        (Language::Japanese, "copy_new_line") => "新側の行をコピー",
        (Language::English, "line_detail_header") => "Line Details (Top: Old / Bottom: New)",
        (Language::Japanese, "line_detail_header") => "行詳細 (上: 旧 / 下: 新)",
        (Language::English, "zero_matches") => "0 matches",
        (Language::Japanese, "zero_matches") => "0 件",
        (Language::English, "no_matches_found") => "Not found",
        (Language::Japanese, "no_matches_found") => "見つかりません",
        (Language::English, "prev_match_tooltip") => "Previous match (Shift+Enter)",
        (Language::Japanese, "prev_match_tooltip") => "前のマッチ (Shift+Enter)",
        (Language::English, "next_match_tooltip") => "Next match (Enter)",
        (Language::Japanese, "next_match_tooltip") => "次のマッチ (Enter)",
        (Language::English, "close_search_tooltip") => "Close (Esc)",
        (Language::Japanese, "close_search_tooltip") => "閉じる (Esc)",

        // Home View Keys
        (Language::English, "repositories") => "Repositories",
        (Language::Japanese, "repositories") => "リポジトリ",
        (Language::English, "search_label") => "Search:",
        (Language::Japanese, "search_label") => "検索:",
        (Language::English, "search_repo_hint") => "Search by repository name or path...",
        (Language::Japanese, "search_repo_hint") => "リポジトリ名やパスで検索...",
        (Language::English, "sort_label") => "Sort:",
        (Language::Japanese, "sort_label") => "並び替え:",
        (Language::English, "sort_name_asc") => "Name (A-Z)",
        (Language::Japanese, "sort_name_asc") => "名前 (昇順)",
        (Language::English, "sort_name_desc") => "Name (Z-A)",
        (Language::Japanese, "sort_name_desc") => "名前 (降順)",
        (Language::English, "sort_updated_desc") => "Last Updated (Newest)",
        (Language::Japanese, "sort_updated_desc") => "最終更新 (新しい順)",
        (Language::English, "sort_updated_asc") => "Last Updated (Oldest)",
        (Language::Japanese, "sort_updated_asc") => "最終更新 (古い順)",
        (Language::English, "select_option") => "Select...",
        (Language::Japanese, "select_option") => "選択してください",
        (Language::English, "reanalyze_all_hint") => "Re-analyze all repositories",
        (Language::Japanese, "reanalyze_all_hint") => "全再分析（全リポジトリを再読み込み）",
        (Language::English, "fetch_check_hint") => "Check remote updates (fetch all repositories)",
        (Language::Japanese, "fetch_check_hint") => {
            "更新確認（リモートから最新の状態を取得し、要プル件数を確認）"
        }
        (Language::English, "pull_all_hint") => "Pull all repositories",
        (Language::Japanese, "pull_all_hint") => "全プル（全リポジトリを一括プル）",
        (Language::English, "no_repos_title") => "No repositories registered",
        (Language::Japanese, "no_repos_title") => "登録されているリポジトリがありません",
        (Language::English, "no_repos_sub") => {
            "Add a repository from the Settings button at top right."
        }
        (Language::Japanese, "no_repos_sub") => {
            "右上の「設定」ボタンからリポジトリを追加してください。"
        }
        (Language::English, "no_matching_repos_title") => "No matching repositories found",
        (Language::Japanese, "no_matching_repos_title") => "条件に一致するリポジトリがありません",
        (Language::English, "no_matching_repos_sub") => {
            "Try changing your search terms or filters."
        }
        (Language::Japanese, "no_matching_repos_sub") => {
            "検索キーワードを変えるか、フィルター条件を見直してください。"
        }
        (Language::English, "click_to_view_details") => "Click to view details",
        (Language::Japanese, "click_to_view_details") => "クリックして詳細を表示",
        (Language::English, "no_commits") => "No commits",
        (Language::Japanese, "no_commits") => "コミットなし",
        (Language::English, "unknown") => "Unknown",
        (Language::Japanese, "unknown") => "不明",
        (Language::English, "analyzing") => "Analyzing...",
        (Language::Japanese, "analyzing") => "分析中...",
        (Language::English, "local_only") => "Local only",
        (Language::Japanese, "local_only") => "ローカルのみ",
        (Language::English, "no_upstream") => "No upstream",
        (Language::Japanese, "no_upstream") => "上流なし",
        (Language::English, "up_to_date") => "Up to date",
        (Language::Japanese, "up_to_date") => "最新",
        (Language::English, "show_uncommitted_diff_hint") => "Show uncommitted changes diff",
        (Language::Japanese, "show_uncommitted_diff_hint") => "未コミット変更の差分を表示",
        (Language::English, "loading_analysis_data") => "Loading analysis data...",
        (Language::Japanese, "loading_analysis_data") => "分析データをロード中...",
        (Language::English, "loading") => "Loading...",
        (Language::Japanese, "loading") => "ロード中...",
        (Language::English, "no_metadata") => "No metadata",
        (Language::Japanese, "no_metadata") => "メタデータなし",
        (Language::English, "reanalyze") => "Re-analyze",
        (Language::Japanese, "reanalyze") => "再分析",
        (Language::English, "open_in_editor_hint") => "Open in configured editor",
        (Language::Japanese, "open_in_editor_hint") => "設定されたエディタで開く",
        (Language::English, "clear_status_hint") => "Clear status",
        (Language::Japanese, "clear_status_hint") => "ステータスをクリア",

        // Git operation messages
        (Language::English, "git_no_remote_pull") => {
            "No remote (e.g. origin) configured. Pull is unavailable for local-only repositories."
        }
        (Language::Japanese, "git_no_remote_pull") => {
            "リモート（origin等）が登録されていません。ローカル専用リポジトリのためプルは行えません。"
        }
        (Language::English, "git_no_upstream") => {
            "No upstream tracking branch is set for the current branch."
        }
        (Language::Japanese, "git_no_upstream") => {
            "現在のブランチに上流（upstream）の追跡リモートブランチが設定されていません。"
        }
        (Language::English, "git_already_up_to_date") => "Already up to date.",
        (Language::Japanese, "git_already_up_to_date") => "すでに最新の状態です。",
        (Language::English, "git_no_remote_fetch") => "No remote (e.g. origin) configured.",
        (Language::Japanese, "git_no_remote_fetch") => "リモート（origin等）が登録されていません。",
        (Language::English, "git_fetch_success") => "Sync status updated.",
        (Language::Japanese, "git_fetch_success") => "同期状態を最新化しました。",

        // App title
        (Language::English, "app_title") => "Git Repository Analysis Dashboard",
        (Language::Japanese, "app_title") => "Gitリポジトリ分析ダッシュボード",

        // Default / Fallback
        (_, _) => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i18n_fallback() {
        assert_eq!(t(Language::English, "unknown_key_xyz"), "unknown_key_xyz");
        assert_eq!(t(Language::Japanese, "unknown_key_xyz"), "unknown_key_xyz");
    }

    #[test]
    fn test_i18n_translations() {
        assert_eq!(t(Language::English, "settings"), "Settings");
        assert_eq!(t(Language::Japanese, "settings"), "設定");
        assert_eq!(t(Language::English, "cancel"), "Cancel");
        assert_eq!(t(Language::Japanese, "cancel"), "キャンセル");
    }
}
