use std::path::{Path, PathBuf};

use rayslash_core::{config, search};

use crate::AppWindow;

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
}
