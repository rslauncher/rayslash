use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use rayslash_core::{config, search};

use crate::{AliasItem, AppWindow, WebSearchItem};
use slint::{Image, VecModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SettingsConfigError {
    EmptyAlternateFolderOpener,
    InvalidMaxResults,
    InvalidTheme,
    InvalidDensity,
    InvalidAliases(String),
    InvalidWebSearches(String),
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
    ui.set_settings_aliases_text(aliases_text(&config.aliases).into());
    ui.set_settings_web_searches_text(web_searches_text(&config.web_searches).into());
    ui.set_settings_aliases(Rc::new(VecModel::from(alias_items(&config.aliases))).into());
    ui.set_settings_web_searches(
        Rc::new(VecModel::from(web_search_items(&config.web_searches))).into(),
    );
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
    ui.set_settings_provider_utility_actions(config.providers.utility_actions);
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

fn alias_items(aliases: &[config::AliasConfig]) -> Vec<AliasItem> {
    aliases
        .iter()
        .map(|alias| AliasItem {
            name: alias.name.clone().into(),
            keyword: alias.query.clone().into(),
            kind: alias.kind.map(alias_kind_label).unwrap_or("").into(),
            target: alias.target.clone().into(),
        })
        .collect()
}

pub(crate) fn web_search_items(searches: &[config::WebSearchConfig]) -> Vec<WebSearchItem> {
    searches
        .iter()
        .map(|search| {
            let icon = rayslash_core::web_search::cached_favicon_path(search)
                .and_then(|path| Image::load_from_path(&path).ok());
            WebSearchItem {
                name: search.name.clone().into(),
                keyword: search.keyword.clone().into(),
                url: search.url.clone().into(),
                enabled: search.enabled,
                has_icon: icon.is_some(),
                icon: icon.unwrap_or_default(),
            }
        })
        .collect()
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
    utility_actions_enabled: bool,
    alternate_folder_opener_enabled: bool,
    learn_from_usage: bool,
    theme: &str,
    density: &str,
    max_results_text: &str,
    show_tooltips: bool,
    aliases_text: &str,
    web_searches_text: &str,
) -> Result<config::Config, SettingsConfigError> {
    let alternate_folder_opener_command = alternate_folder_opener_command.trim();
    if alternate_folder_opener_enabled && alternate_folder_opener_command.is_empty() {
        return Err(SettingsConfigError::EmptyAlternateFolderOpener);
    }

    let max_results =
        parse_max_results(max_results_text).ok_or(SettingsConfigError::InvalidMaxResults)?;
    let theme = parse_theme(theme).ok_or(SettingsConfigError::InvalidTheme)?;
    let density = parse_density(density).ok_or(SettingsConfigError::InvalidDensity)?;
    let aliases = parse_aliases_text(aliases_text).map_err(SettingsConfigError::InvalidAliases)?;
    let web_searches = parse_web_searches_text(web_searches_text)
        .map_err(SettingsConfigError::InvalidWebSearches)?;

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
            utility_actions: utility_actions_enabled,
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

pub(crate) fn parse_aliases_text(text: &str) -> Result<Vec<config::AliasConfig>, String> {
    let mut aliases = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts = line.splitn(4, '|').map(str::trim).collect::<Vec<_>>();
        let (name, query, kind, target) = match parts.as_slice() {
            [name, query, target] => (*name, *query, None, *target),
            [name, query, kind, target] => (
                *name,
                *query,
                Some(
                    parse_alias_kind(kind)
                        .ok_or_else(|| format!("alias line {} has an unknown kind", index + 1))?,
                ),
                *target,
            ),
            _ => {
                return Err(format!(
                    "alias line {} must be name | keyword | target or name | keyword | kind | target",
                    index + 1
                ));
            }
        };

        if name.is_empty() || query.is_empty() || target.is_empty() {
            return Err(format!(
                "alias line {} has an empty required field",
                index + 1
            ));
        }

        aliases.push(config::AliasConfig {
            name: name.to_owned(),
            query: query.to_owned(),
            target: target.to_owned(),
            kind,
        });
    }

    Ok(aliases)
}

pub(crate) fn parse_web_searches_text(text: &str) -> Result<Vec<config::WebSearchConfig>, String> {
    let mut searches = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts = line.splitn(4, '|').map(str::trim).collect::<Vec<_>>();
        let [enabled, name, keyword, url] = parts.as_slice() else {
            return Err(format!(
                "search line {} must be on/off | name | keyword | url",
                index + 1
            ));
        };

        let enabled = parse_enabled_flag(enabled)
            .ok_or_else(|| format!("search line {} must start with on or off", index + 1))?;
        let url = url.replace("{query}", "%s");
        searches.push(config::WebSearchConfig {
            name: (*name).to_owned(),
            keyword: (*keyword).to_owned(),
            url,
            enabled,
        });
    }

    Ok(searches)
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

pub(crate) fn parse_alias_kind(text: &str) -> Option<config::AliasKind> {
    match text.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "url" => Some(config::AliasKind::Url),
        "file" => Some(config::AliasKind::File),
        "folder" => Some(config::AliasKind::Folder),
        "command" => Some(config::AliasKind::Command),
        _ => None,
    }
}

fn parse_enabled_flag(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "on" | "enabled" | "true" | "yes" => Some(true),
        "off" | "disabled" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn folder_sources_text(sources: &[PathBuf]) -> String {
    sources
        .iter()
        .map(|path| search::display_path(path))
        .collect::<Vec<_>>()
        .join("; ")
}

fn aliases_text(aliases: &[config::AliasConfig]) -> String {
    aliases
        .iter()
        .map(|alias| {
            if let Some(kind) = alias.kind {
                format!(
                    "{} | {} | {} | {}",
                    alias.name,
                    alias.query,
                    alias_kind_label(kind),
                    alias.target
                )
            } else {
                format!("{} | {} | {}", alias.name, alias.query, alias.target)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn web_searches_text(searches: &[config::WebSearchConfig]) -> String {
    searches
        .iter()
        .map(|search| {
            format!(
                "{} | {} | {} | {}",
                if search.enabled { "on" } else { "off" },
                search.name,
                search.keyword,
                search.url
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn alias_kind_label(kind: config::AliasKind) -> &'static str {
    match kind {
        config::AliasKind::Url => "url",
        config::AliasKind::File => "file",
        config::AliasKind::Folder => "folder",
        config::AliasKind::Command => "command",
    }
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
            false,
            true,
            false,
            "dim",
            "compact",
            "25",
            false,
            "GitHub | gh | url | https://github.com",
            "on | DuckDuckGo | ddg | https://duckduckgo.com/?q=%s",
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
        assert!(!config.providers.utility_actions);
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
                true,
                "dark",
                "comfortable",
                "50",
                true,
                "",
                ""
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
                true,
                "dark",
                "comfortable",
                "0",
                true,
                "",
                ""
            ),
            Err(SettingsConfigError::InvalidMaxResults)
        );
    }

    #[test]
    fn aliases_text_round_trips_alias_rows() {
        let aliases = vec![
            config::AliasConfig {
                name: "GitHub".to_owned(),
                query: "gh".to_owned(),
                target: "https://github.com".to_owned(),
                kind: Some(config::AliasKind::Url),
            },
            config::AliasConfig {
                name: "Timer".to_owned(),
                query: "timer".to_owned(),
                target: "gnome-clocks --timer".to_owned(),
                kind: Some(config::AliasKind::Command),
            },
        ];
        let text = aliases_text(&aliases);

        assert_eq!(parse_aliases_text(&text), Ok(aliases));
    }

    #[test]
    fn web_searches_text_round_trips_enabled_rows() {
        let searches = vec![
            config::WebSearchConfig {
                name: "YouTube".to_owned(),
                keyword: "yt".to_owned(),
                url: "https://www.youtube.com/results?search_query=%s".to_owned(),
                enabled: true,
            },
            config::WebSearchConfig {
                name: "Docs".to_owned(),
                keyword: "docs".to_owned(),
                url: "https://example.com/search?q=%s".to_owned(),
                enabled: false,
            },
        ];
        let text = web_searches_text(&searches);

        assert_eq!(parse_web_searches_text(&text), Ok(searches));
    }

    #[test]
    fn web_searches_text_preserves_incomplete_rows_as_drafts() {
        let parsed = parse_web_searches_text(
            "on | YouTube |  | https://www.youtube.com/results?search_query=%s\n\
             on | Broken | br | https://example.com/search",
        )
        .expect("draft rows remain parseable");

        assert_eq!(parsed.len(), 2);
        assert!(!rayslash_core::web_search::is_valid_template(&parsed[0]));
        assert!(!rayslash_core::web_search::is_valid_template(&parsed[1]));
    }
}
