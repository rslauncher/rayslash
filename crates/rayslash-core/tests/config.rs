mod fixtures;

use std::{
    fs,
    path::{Path, PathBuf},
};

use fixtures::TempDir;
use rayslash_core::config::{
    self, ActionConfig, AliasConfig, AliasKind, AppearanceConfig, AppearanceDensity,
    AppearanceTheme, Config, ProviderConfig, RankingConfig, WebSearchConfig,
};

#[test]
fn default_config_has_public_defaults() {
    let config = Config::default();

    assert!(config.folder_sources.len() <= 1);
    assert_eq!(config.providers, ProviderConfig::default());
    assert_eq!(config.actions, ActionConfig::default());
    assert_eq!(config.appearance, AppearanceConfig::default());
}

#[test]
fn config_file_lives_under_rayslash_config_dir() {
    let Some(path) = config::config_file() else {
        return;
    };

    assert!(path.ends_with("rayslash/config.toml"));
}

#[test]
fn missing_config_loads_defaults() {
    let dir = TempDir::new("rayslash-missing-config");
    let path = dir.join("missing.toml");

    let config = config::load_config_from_path(&path).expect("missing config should use defaults");

    assert_eq!(config, Config::default());
}

#[test]
fn config_loads_folder_sources_from_toml() {
    let dir = TempDir::new("rayslash-config-test");
    let path = dir
        .write(
            "config.toml",
            r#"
folder_sources = ["/tmp/alpha", "/tmp/beta"]
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");

    assert_eq!(
        config.folder_sources,
        vec![PathBuf::from("/tmp/alpha"), PathBuf::from("/tmp/beta")]
    );
}

#[test]
fn legacy_project_roots_still_load_as_folder_sources() {
    let dir = TempDir::new("rayslash-config-legacy-roots-test");
    let path = dir
        .write(
            "config.toml",
            r#"
project_roots = ["/tmp/alpha"]
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");

    assert_eq!(config.folder_sources, vec![PathBuf::from("/tmp/alpha")]);
}

#[test]
fn config_loads_provider_action_and_appearance_settings_from_toml() {
    let dir = TempDir::new("rayslash-config-settings-test");
    let path = dir
        .write(
            "config.toml",
            r#"
folder_sources = ["/tmp/alpha"]

[[aliases]]
name = "GitHub"
query = "gh"
target = "https://github.com"
kind = "url"

[[web_searches]]
name = "DuckDuckGo"
query = "ddg"
url_template = "https://duckduckgo.com/?q={query}"

[providers]
apps = false
folders = true
calculator = false
aliases = false
web_search = true
unit_conversion = false
currency_conversion = true

[actions]
alternate_folder_opener_enabled = false
alternate_folder_opener_command = "codium"

[appearance]
max_results = 20
theme = "dim"
density = "compact"
show_tooltips = false

[ranking]
learn_from_usage = false
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");

    assert_eq!(config.folder_sources, vec![PathBuf::from("/tmp/alpha")]);
    assert_eq!(
        config.aliases,
        vec![AliasConfig {
            name: "GitHub".to_owned(),
            query: "gh".to_owned(),
            target: "https://github.com".to_owned(),
            kind: Some(AliasKind::Url),
        }]
    );
    assert_eq!(
        config.web_searches,
        vec![WebSearchConfig {
            name: "DuckDuckGo".to_owned(),
            query: "ddg".to_owned(),
            url_template: "https://duckduckgo.com/?q={query}".to_owned(),
        }]
    );
    assert_eq!(
        config.providers,
        ProviderConfig {
            apps: false,
            folders: true,
            calculator: false,
            aliases: false,
            web_search: true,
            unit_conversion: false,
            currency_conversion: true,
        }
    );
    assert!(!config.actions.alternate_folder_opener_enabled);
    assert_eq!(config.actions.alternate_folder_opener_command, "codium");
    assert_eq!(
        config.appearance,
        AppearanceConfig {
            theme: AppearanceTheme::Dim,
            density: AppearanceDensity::Compact,
            max_results: 20,
            show_tooltips: false,
        }
    );
    assert_eq!(
        config.ranking,
        RankingConfig {
            learn_from_usage: false,
        }
    );
}

#[test]
fn legacy_provider_and_action_fields_still_load() {
    let dir = TempDir::new("rayslash-config-legacy-settings-test");
    let path = dir
        .write(
            "config.toml",
            r#"
[providers]
projects = false

[actions]
project_editor_command = "codium"
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");

    assert!(!config.providers.folders);
    assert_eq!(config.actions.alternate_folder_opener_command, "codium");
}

#[test]
fn missing_nested_config_fields_use_public_defaults() {
    let dir = TempDir::new("rayslash-config-default-settings-test");
    let path = dir
        .write(
            "config.toml",
            r#"
[providers]
apps = false

[actions]
alternate_folder_opener_command = ""

[appearance]
max_results = 0
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");

    assert_eq!(
        config.providers,
        ProviderConfig {
            apps: false,
            folders: true,
            calculator: true,
            aliases: true,
            web_search: false,
            unit_conversion: false,
            currency_conversion: false,
        }
    );
    assert_eq!(
        config.actions.alternate_folder_opener_command,
        "xdg-terminal-exec"
    );
    assert!(config.actions.alternate_folder_opener_enabled);
    assert_eq!(config.appearance.max_results, 36);
    assert!(config.appearance.show_tooltips);
    assert!(config.ranking.learn_from_usage);
}

#[test]
fn config_expands_tilde_folder_sources() {
    let dir = TempDir::new("rayslash-config-tilde-test");
    let path = dir
        .write(
            "config.toml",
            r#"
folder_sources = ["~/Documents"]
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");
    let home = dirs::home_dir().expect("home dir");

    assert_eq!(config.folder_sources, vec![home.join("Documents")]);
}

#[test]
fn config_normalizes_relative_folder_sources_to_absolute_paths() {
    let dir = TempDir::new("rayslash-config-relative-test");
    let path = dir
        .write(
            "config.toml",
            r#"
folder_sources = ["relative/projects"]
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");
    let expected = std::env::current_dir()
        .expect("current dir")
        .join("relative/projects");

    assert_eq!(config.folder_sources, vec![expected]);
}

#[test]
fn config_normalizes_inferred_file_and_folder_alias_targets() {
    let dir = TempDir::new("rayslash-config-alias-normalize-test");
    let path = dir
        .write(
            "config.toml",
            r#"
[[aliases]]
name = "Documents"
query = "docs"
target = "~/Documents"
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");
    let home = dirs::home_dir().expect("home dir");

    assert_eq!(
        config.aliases[0].target,
        home.join("Documents").display().to_string()
    );
}

#[test]
fn config_can_be_saved_and_loaded_from_toml() {
    let dir = TempDir::new("rayslash-config-save-test");
    let path = dir.join("nested/config.toml");
    let config = Config {
        folder_sources: vec![PathBuf::from("~/Documents")],
        aliases: vec![AliasConfig {
            name: "Docs".to_owned(),
            query: "docs".to_owned(),
            target: "~/Documents".to_owned(),
            kind: Some(AliasKind::Folder),
        }],
        web_searches: vec![WebSearchConfig {
            name: "DuckDuckGo".to_owned(),
            query: "ddg".to_owned(),
            url_template: "https://duckduckgo.com/?q={query}".to_owned(),
        }],
        providers: ProviderConfig {
            apps: true,
            folders: false,
            calculator: true,
            aliases: true,
            web_search: true,
            unit_conversion: true,
            currency_conversion: false,
        },
        actions: ActionConfig {
            alternate_folder_opener_enabled: false,
            alternate_folder_opener_command: "codium".to_owned(),
        },
        appearance: AppearanceConfig {
            theme: AppearanceTheme::Dim,
            density: AppearanceDensity::Compact,
            max_results: 25,
            show_tooltips: false,
        },
        ranking: RankingConfig {
            learn_from_usage: false,
        },
    };

    config::save_config_to_path(&path, &config).expect("save config");
    let saved = fs::read_to_string(&path).expect("read saved config");
    let loaded = config::load_config_from_path(&path).expect("load saved config");
    let home = dirs::home_dir().expect("home dir");

    assert!(saved.contains("folder_sources"));
    assert!(saved.contains("[[aliases]]"));
    assert!(saved.contains("[[web_searches]]"));
    assert!(saved.contains("folders = false"));
    assert!(saved.contains("web_search = true"));
    assert!(saved.contains("currency_conversion = false"));
    assert!(saved.contains("show_tooltips = false"));
    assert!(saved.contains("alternate_folder_opener_enabled = false"));
    assert!(saved.contains("alternate_folder_opener_command = \"codium\""));
    assert!(saved.contains("theme = \"dim\""));
    assert!(saved.contains("density = \"compact\""));
    assert!(saved.contains("learn_from_usage = false"));
    assert_eq!(loaded.folder_sources, vec![home.join("Documents")]);
    assert_eq!(
        loaded.aliases[0].target,
        home.join("Documents").display().to_string()
    );
    assert_eq!(loaded.providers, config.providers);
    assert_eq!(loaded.actions, config.actions);
    assert_eq!(loaded.appearance, config.appearance);
    assert_eq!(loaded.ranking, config.ranking);
    assert_no_temp_save_files(path.parent().expect("config parent"));
}

#[test]
fn backup_save_preserves_existing_config_before_rewrite() {
    let dir = TempDir::new("rayslash-config-backup-save-test");
    let path = dir
        .write(
            "config.toml",
            r#"# hand-authored comment
folder_sources = ["/tmp/original"]
"#,
        )
        .expect("write original config");
    let config = Config {
        folder_sources: vec![PathBuf::from("/tmp/updated")],
        ..Config::default()
    };

    config::save_config_to_path_with_backup(&path, &config).expect("save config with backup");

    let backups = backup_files(path.parent().expect("config parent"));
    assert_eq!(backups.len(), 1);
    let backup = fs::read_to_string(&backups[0]).expect("read backup");
    let saved = fs::read_to_string(&path).expect("read saved config");

    assert!(backup.contains("# hand-authored comment"));
    assert!(backup.contains("/tmp/original"));
    assert!(saved.contains("/tmp/updated"));
}

#[test]
fn backup_save_does_not_create_backup_for_missing_config() {
    let dir = TempDir::new("rayslash-config-missing-backup-save-test");
    let path = dir.join("config.toml");
    let config = Config::default();

    config::save_config_to_path_with_backup(&path, &config).expect("save config with backup");

    assert!(backup_files(path.parent().expect("config parent")).is_empty());
}

#[test]
fn saved_config_writes_normalized_folder_sources() {
    let dir = TempDir::new("rayslash-config-normalized-save-test");
    let path = dir.join("config.toml");
    let config = Config {
        folder_sources: vec![PathBuf::from("relative/projects")],
        ..Config::default()
    };

    config::save_config_to_path(&path, &config).expect("save config");
    let saved = fs::read_to_string(&path).expect("read saved config");
    let expected = std::env::current_dir()
        .expect("current dir")
        .join("relative/projects");

    assert!(saved.contains(&expected.display().to_string()));
    assert!(!saved.contains("\"relative/projects\""));
}

fn backup_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .expect("read save directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("config.toml.backup-"))
        })
        .collect()
}

fn assert_no_temp_save_files(dir: &Path) {
    let temp_files = fs::read_dir(dir)
        .expect("read save directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".config.toml.") && name.ends_with(".tmp"))
        })
        .collect::<Vec<_>>();

    assert!(temp_files.is_empty());
}
