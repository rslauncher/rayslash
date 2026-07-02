use std::path::{Path, PathBuf};

use rayslash_core::{config, search};

use crate::AppWindow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SettingsConfigError {
    EmptyAlternateFolderOpener,
    InvalidMaxResults,
}

pub(crate) fn set_settings_properties(
    ui: &AppWindow,
    config: &config::Config,
    socket_path: &Path,
    project_count: usize,
    app_count: usize,
    icon_count: usize,
    ranking_entry_count: usize,
) {
    ui.set_settings_folder_sources(folder_sources_text(&config.folder_sources).into());
    ui.set_settings_alternate_folder_opener_command(
        config
            .actions
            .alternate_folder_opener_command
            .clone()
            .into(),
    );
    ui.set_settings_provider_apps(config.providers.apps);
    ui.set_settings_provider_folders(config.providers.folders);
    ui.set_settings_provider_calculator(config.providers.calculator);
    ui.set_settings_alternate_folder_opener_enabled(config.actions.alternate_folder_opener_enabled);
    ui.set_settings_ranking_learn_from_usage(config.ranking.learn_from_usage);
    ui.set_settings_max_results(config.appearance.max_results.to_string().into());
    ui.set_settings_config_path(path_option_label(config::config_file()).into());
    ui.set_settings_state_path(path_option_label(config::state_dir()).into());
    ui.set_settings_socket_path(socket_path.display().to_string().into());
    ui.set_settings_project_count(project_count.to_string().into());
    ui.set_settings_app_count(app_count.to_string().into());
    ui.set_settings_icon_count(format!("{icon_count}/{app_count}").into());
    ui.set_settings_ranking_entry_count(ranking_entry_count.to_string().into());
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn config_from_settings_fields(
    folder_sources_text: &str,
    alternate_folder_opener_command: &str,
    apps_enabled: bool,
    folders_enabled: bool,
    calculator_enabled: bool,
    alternate_folder_opener_enabled: bool,
    learn_from_usage: bool,
    max_results_text: &str,
) -> Result<config::Config, SettingsConfigError> {
    let alternate_folder_opener_command = alternate_folder_opener_command.trim();
    if alternate_folder_opener_enabled && alternate_folder_opener_command.is_empty() {
        return Err(SettingsConfigError::EmptyAlternateFolderOpener);
    }

    let max_results =
        parse_max_results(max_results_text).ok_or(SettingsConfigError::InvalidMaxResults)?;

    Ok(config::Config {
        folder_sources: parse_folder_sources_text(folder_sources_text),
        providers: config::ProviderConfig {
            apps: apps_enabled,
            folders: folders_enabled,
            calculator: calculator_enabled,
        },
        actions: config::ActionConfig {
            alternate_folder_opener_enabled,
            alternate_folder_opener_command: alternate_folder_opener_command.to_owned(),
        },
        appearance: config::AppearanceConfig { max_results },
        ranking: config::RankingConfig { learn_from_usage },
    })
}

pub(crate) fn parse_folder_sources_text(text: &str) -> Vec<PathBuf> {
    text.split([';', '\n'])
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

pub(crate) fn first_existing_folder_source(text: &str) -> Option<PathBuf> {
    parse_folder_sources_text(text)
        .into_iter()
        .map(expand_home_for_ui)
        .find(|path| path.is_dir())
}

pub(crate) fn parse_max_results(text: &str) -> Option<usize> {
    let max_results = text.trim().parse().ok()?;
    (max_results > 0).then_some(max_results)
}

fn folder_sources_text(sources: &[PathBuf]) -> String {
    sources
        .iter()
        .map(|path| search::display_path(path))
        .collect::<Vec<_>>()
        .join("; ")
}

fn expand_home_for_ui(path: PathBuf) -> PathBuf {
    let Some(path_str) = path.to_str() else {
        return path;
    };

    if path_str == "~" {
        return dirs::home_dir().unwrap_or(path);
    }

    if let Some(rest) = path_str.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }

    path
}

fn path_option_label(path: Option<PathBuf>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "Unavailable".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_sources_text_uses_semicolon_separated_paths() {
        let sources = vec![PathBuf::from("/tmp/alpha"), PathBuf::from("/tmp/beta")];

        assert_eq!(folder_sources_text(&sources), "/tmp/alpha; /tmp/beta");
    }

    #[test]
    fn parse_folder_sources_text_accepts_semicolons_and_newlines() {
        let roots = parse_folder_sources_text(" ~/Documents ; /tmp/rayslash\n/tmp/other ");

        assert_eq!(
            roots,
            vec![
                PathBuf::from("~/Documents"),
                PathBuf::from("/tmp/rayslash"),
                PathBuf::from("/tmp/other")
            ]
        );
    }

    #[test]
    fn parse_max_results_requires_positive_number() {
        assert_eq!(parse_max_results("25"), Some(25));
        assert_eq!(parse_max_results("0"), None);
        assert_eq!(parse_max_results("abc"), None);
    }

    #[test]
    fn config_from_settings_fields_builds_config() {
        let config = config_from_settings_fields(
            "~/Documents; /tmp/rayslash",
            " code --reuse-window ",
            true,
            false,
            true,
            true,
            false,
            "25",
        )
        .expect("settings config");

        assert_eq!(
            config.folder_sources,
            vec![PathBuf::from("~/Documents"), PathBuf::from("/tmp/rayslash")]
        );
        assert!(config.providers.apps);
        assert!(!config.providers.folders);
        assert!(config.providers.calculator);
        assert!(config.actions.alternate_folder_opener_enabled);
        assert_eq!(
            config.actions.alternate_folder_opener_command,
            "code --reuse-window"
        );
        assert!(!config.ranking.learn_from_usage);
        assert_eq!(config.appearance.max_results, 25);
    }

    #[test]
    fn config_from_settings_fields_validates_user_editable_fields() {
        assert_eq!(
            config_from_settings_fields("", " ", true, true, true, true, true, "50"),
            Err(SettingsConfigError::EmptyAlternateFolderOpener)
        );
        assert_eq!(
            config_from_settings_fields("", "code", true, true, true, true, true, "0"),
            Err(SettingsConfigError::InvalidMaxResults)
        );
    }
}
