use std::path::{Path, PathBuf};

use rayslash_core::{config, search};

use crate::AppWindow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SettingsConfigError {
    EmptyAlternateFolderOpener,
    InvalidMaxResults,
    InvalidTheme,
    InvalidDensity,
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
    ui.set_settings_provider_aliases(config.providers.aliases);
    ui.set_settings_provider_web_search(config.providers.web_search);
    ui.set_settings_provider_unit_conversion(config.providers.unit_conversion);
    ui.set_settings_provider_currency_conversion(config.providers.currency_conversion);
    ui.set_settings_provider_time_lookup(config.providers.time_lookup);
    ui.set_settings_alternate_folder_opener_enabled(config.actions.alternate_folder_opener_enabled);
    ui.set_settings_ranking_learn_from_usage(config.ranking.learn_from_usage);
    ui.set_settings_theme(appearance_theme_label(config.appearance.theme).into());
    ui.set_settings_density(appearance_density_label(config.appearance.density).into());
    ui.set_settings_max_results(config.appearance.max_results.to_string().into());
    ui.set_settings_show_tooltips(config.appearance.show_tooltips);
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
    aliases_enabled: bool,
    web_search_enabled: bool,
    unit_conversion_enabled: bool,
    currency_conversion_enabled: bool,
    time_lookup_enabled: bool,
    alternate_folder_opener_enabled: bool,
    learn_from_usage: bool,
    theme: &str,
    density: &str,
    max_results_text: &str,
    show_tooltips: bool,
    aliases: Vec<config::AliasConfig>,
    web_searches: Vec<config::WebSearchConfig>,
) -> Result<config::Config, SettingsConfigError> {
    let alternate_folder_opener_command = alternate_folder_opener_command.trim();
    if alternate_folder_opener_enabled && alternate_folder_opener_command.is_empty() {
        return Err(SettingsConfigError::EmptyAlternateFolderOpener);
    }

    let max_results =
        parse_max_results(max_results_text).ok_or(SettingsConfigError::InvalidMaxResults)?;
    let theme = parse_theme(theme).ok_or(SettingsConfigError::InvalidTheme)?;
    let density = parse_density(density).ok_or(SettingsConfigError::InvalidDensity)?;

    Ok(config::Config {
        folder_sources: parse_folder_sources_text(folder_sources_text),
        aliases,
        web_searches,
        providers: config::ProviderConfig {
            apps: apps_enabled,
            folders: folders_enabled,
            calculator: calculator_enabled,
            aliases: aliases_enabled,
            web_search: web_search_enabled,
            unit_conversion: unit_conversion_enabled,
            currency_conversion: currency_conversion_enabled,
            time_lookup: time_lookup_enabled,
        },
        actions: config::ActionConfig {
            alternate_folder_opener_enabled,
            alternate_folder_opener_command: alternate_folder_opener_command.to_owned(),
        },
        appearance: config::AppearanceConfig {
            theme,
            density,
            max_results,
            show_tooltips,
        },
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

pub(crate) fn parse_theme(text: &str) -> Option<config::AppearanceTheme> {
    match text.trim().to_ascii_lowercase().as_str() {
        "dark" => Some(config::AppearanceTheme::Dark),
        "dim" => Some(config::AppearanceTheme::Dim),
        "light" => Some(config::AppearanceTheme::Light),
        _ => None,
    }
}

pub(crate) fn parse_density(text: &str) -> Option<config::AppearanceDensity> {
    match text.trim().to_ascii_lowercase().as_str() {
        "compact" => Some(config::AppearanceDensity::Compact),
        "comfortable" => Some(config::AppearanceDensity::Comfortable),
        _ => None,
    }
}

fn appearance_theme_label(theme: config::AppearanceTheme) -> &'static str {
    match theme {
        config::AppearanceTheme::Dark => "dark",
        config::AppearanceTheme::Dim => "dim",
        config::AppearanceTheme::Light => "light",
    }
}

fn appearance_density_label(density: config::AppearanceDensity) -> &'static str {
    match density {
        config::AppearanceDensity::Compact => "compact",
        config::AppearanceDensity::Comfortable => "comfortable",
    }
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
    fn parse_theme_and_density_accept_known_values() {
        assert_eq!(parse_theme("dim"), Some(config::AppearanceTheme::Dim));
        assert_eq!(parse_theme("light"), Some(config::AppearanceTheme::Light));
        assert_eq!(
            parse_density("compact"),
            Some(config::AppearanceDensity::Compact)
        );
        assert_eq!(parse_theme("solarized"), None);
        assert_eq!(parse_density("tiny"), None);
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
            true,
            true,
            false,
            true,
            true,
            false,
            "dim",
            "compact",
            "25",
            false,
            vec![config::AliasConfig {
                name: "GitHub".to_owned(),
                query: "gh".to_owned(),
                target: "https://github.com".to_owned(),
                kind: Some(config::AliasKind::Url),
            }],
            vec![config::WebSearchConfig {
                name: "DuckDuckGo".to_owned(),
                query: "ddg".to_owned(),
                url_template: "https://duckduckgo.com/?q={query}".to_owned(),
            }],
        )
        .expect("settings config");

        assert_eq!(
            config.folder_sources,
            vec![PathBuf::from("~/Documents"), PathBuf::from("/tmp/rayslash")]
        );
        assert!(config.providers.apps);
        assert!(!config.providers.folders);
        assert!(config.providers.calculator);
        assert!(config.providers.aliases);
        assert!(config.providers.web_search);
        assert!(config.providers.unit_conversion);
        assert!(!config.providers.currency_conversion);
        assert!(config.providers.time_lookup);
        assert_eq!(config.aliases.len(), 1);
        assert_eq!(config.web_searches.len(), 1);
        assert!(config.actions.alternate_folder_opener_enabled);
        assert_eq!(
            config.actions.alternate_folder_opener_command,
            "code --reuse-window"
        );
        assert!(!config.ranking.learn_from_usage);
        assert_eq!(
            config.appearance,
            config::AppearanceConfig {
                theme: config::AppearanceTheme::Dim,
                density: config::AppearanceDensity::Compact,
                max_results: 25,
                show_tooltips: false,
            }
        );
    }

    #[test]
    fn config_from_settings_fields_validates_user_editable_fields() {
        assert_eq!(
            config_from_settings_fields(
                "",
                " ",
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                "dark",
                "comfortable",
                "50",
                true,
                Vec::new(),
                Vec::new()
            ),
            Err(SettingsConfigError::EmptyAlternateFolderOpener)
        );
        assert_eq!(
            config_from_settings_fields(
                "",
                "code",
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                true,
                "dark",
                "comfortable",
                "0",
                true,
                Vec::new(),
                Vec::new()
            ),
            Err(SettingsConfigError::InvalidMaxResults)
        );
    }
}
