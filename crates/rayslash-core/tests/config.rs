mod fixtures;

use std::{fs, path::PathBuf};

use fixtures::TempDir;
use rayslash_core::config::{
    self, ActionConfig, AppearanceConfig, Config, ProviderConfig, RankingConfig,
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

[providers]
apps = false
folders = true
calculator = false

[actions]
alternate_folder_opener_enabled = false
alternate_folder_opener_command = "codium"

[appearance]
max_results = 20

[ranking]
learn_from_usage = false
"#,
        )
        .expect("write config");

    let config = config::load_config_from_path(&path).expect("load config");

    assert_eq!(config.folder_sources, vec![PathBuf::from("/tmp/alpha")]);
    assert_eq!(
        config.providers,
        ProviderConfig {
            apps: false,
            folders: true,
            calculator: false,
        }
    );
    assert!(!config.actions.alternate_folder_opener_enabled);
    assert_eq!(config.actions.alternate_folder_opener_command, "codium");
    assert_eq!(config.appearance, AppearanceConfig { max_results: 20 });
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
        }
    );
    assert_eq!(
        config.actions.alternate_folder_opener_command,
        "xdg-terminal-exec"
    );
    assert!(config.actions.alternate_folder_opener_enabled);
    assert_eq!(config.appearance.max_results, 50);
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
fn config_can_be_saved_and_loaded_from_toml() {
    let dir = TempDir::new("rayslash-config-save-test");
    let path = dir.join("nested/config.toml");
    let config = Config {
        folder_sources: vec![PathBuf::from("~/Documents")],
        providers: ProviderConfig {
            apps: true,
            folders: false,
            calculator: true,
        },
        actions: ActionConfig {
            alternate_folder_opener_enabled: false,
            alternate_folder_opener_command: "codium".to_owned(),
        },
        appearance: AppearanceConfig { max_results: 25 },
        ranking: RankingConfig {
            learn_from_usage: false,
        },
    };

    config::save_config_to_path(&path, &config).expect("save config");
    let saved = fs::read_to_string(&path).expect("read saved config");
    let loaded = config::load_config_from_path(&path).expect("load saved config");
    let home = dirs::home_dir().expect("home dir");

    assert!(saved.contains("folder_sources"));
    assert!(saved.contains("folders = false"));
    assert!(saved.contains("alternate_folder_opener_enabled = false"));
    assert!(saved.contains("alternate_folder_opener_command = \"codium\""));
    assert!(saved.contains("learn_from_usage = false"));
    assert_eq!(loaded.folder_sources, vec![home.join("Documents")]);
    assert_eq!(loaded.providers, config.providers);
    assert_eq!(loaded.actions, config.actions);
    assert_eq!(loaded.appearance, config.appearance);
    assert_eq!(loaded.ranking, config.ranking);
}
