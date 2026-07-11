use std::path::Path;

use crate::apps::DesktopApp;
use crate::calc;
use crate::config::{AliasConfig, ProviderConfig};
use crate::currency;
use crate::projects::Project;
use crate::time_lookup;
use crate::units;
use crate::utility_actions::{self, SystemActionKind};
use crate::web_search;
use nucleo_matcher::Utf32Str;

use super::matcher::{fuzzy_matcher, fuzzy_pattern, project_order};
use super::{SearchResult, SearchResultIcon, SearchResultKind};

pub fn placeholder_results() -> Vec<SearchResult> {
    placeholder_results_for_providers(&ProviderConfig::default())
}

pub(crate) fn placeholder_results_for_providers(providers: &ProviderConfig) -> Vec<SearchResult> {
    let mut results = Vec::new();

    if providers.apps {
        results.push(SearchResult {
            title: "Open applications".to_owned(),
            flair: String::new(),
            subtitle: "Desktop app search is available when applications are discovered".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if providers.folders {
        results.push(SearchResult {
            title: "Find folders".to_owned(),
            flair: String::new(),
            subtitle: "Folder search is available when folder sources contain folders".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if providers.calculator {
        results.push(SearchResult {
            title: "Calculate".to_owned(),
            flair: String::new(),
            subtitle: "Type an expression such as 2 + 2".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if providers.aliases {
        results.push(SearchResult {
            title: "Use aliases".to_owned(),
            flair: String::new(),
            subtitle: "Add quick links in config.toml with [[aliases]]".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if providers.web_search {
        results.push(SearchResult {
            title: "Search the web".to_owned(),
            flair: String::new(),
            subtitle: "Type search and a query, or add [[web_searches]] templates".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if providers.unit_conversion {
        results.push(SearchResult {
            title: "Convert units".to_owned(),
            flair: String::new(),
            subtitle: "Type a conversion such as 10 km to mi".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if providers.currency_conversion {
        results.push(SearchResult {
            title: "Convert currency".to_owned(),
            flair: String::new(),
            subtitle: "Type a conversion such as 10 USD to EUR".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if providers.time_lookup {
        results.push(SearchResult {
            title: "Check time".to_owned(),
            flair: String::new(),
            subtitle: "Type a lookup such as time in Argentina".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if results.is_empty() && providers.utility_actions {
        results.push(SearchResult {
            title: "Use timers and system actions".to_owned(),
            flair: String::new(),
            subtitle: "Type a timer, reminder, reboot, shutdown, logout, or lock command"
                .to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        });
    }

    if results.is_empty() {
        results.push(disabled_providers_result());
    }

    results
}

pub fn project_results(projects: &[Project], query: &str) -> Vec<SearchResult> {
    if projects.is_empty() {
        return placeholder_results();
    }

    let query = query.trim();

    if query.is_empty() {
        let mut projects = projects.iter().collect::<Vec<_>>();
        projects.sort_by(|a, b| project_order(a, b));
        return projects.into_iter().map(project_result).collect();
    }

    let pattern = fuzzy_pattern(query);
    let mut matcher = fuzzy_matcher();
    let mut char_buf = Vec::new();

    let mut matches = projects
        .iter()
        .filter_map(|project| {
            let haystack = Utf32Str::new(&project.name, &mut char_buf);
            pattern
                .score(haystack, &mut matcher)
                .map(|score| (project, score))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|(a, a_score), (b, b_score)| {
        b_score.cmp(a_score).then_with(|| project_order(a, b))
    });

    matches
        .into_iter()
        .map(|(project, _score)| project_result(project))
        .collect()
}

pub(crate) fn project_result(project: &Project) -> SearchResult {
    let subtitle = dirs::home_dir()
        .map(|home| display_path_for_home(&project.path, &home))
        .unwrap_or_else(|| project.path.display().to_string());

    project_result_with_subtitle(project, subtitle)
}

pub(super) fn project_result_with_subtitle(project: &Project, subtitle: String) -> SearchResult {
    SearchResult {
        title: project.name.clone(),
        flair: String::new(),
        subtitle,
        icon: SearchResultIcon::ProjectFolder,
        kind: SearchResultKind::Project {
            path: project.path.clone(),
        },
    }
}

pub(crate) fn app_result(app: &DesktopApp) -> SearchResult {
    SearchResult {
        title: app.name.clone(),
        flair: String::new(),
        subtitle: app_subtitle(app),
        icon: SearchResultIcon::App {
            path: app.icon_path.clone(),
        },
        kind: SearchResultKind::App {
            id: app.id.clone(),
            command: app.command.clone(),
            desktop_file: app.desktop_file.clone(),
            dbus_activatable: app.dbus_activatable,
            startup_wm_class: app.startup_wm_class.clone(),
        },
    }
}

pub(crate) fn alias_result(alias: &AliasConfig) -> SearchResult {
    SearchResult {
        title: alias.name.clone(),
        flair: String::new(),
        subtitle: crate::aliases::alias_subtitle(alias),
        icon: SearchResultIcon::Placeholder,
        kind: SearchResultKind::Alias {
            alias: alias.clone(),
        },
    }
}

pub(crate) fn calculator_result(calculation: calc::Calculation) -> SearchResult {
    match calculation {
        calc::Calculation::Value { expression, result } => SearchResult {
            title: result.clone(),
            flair: String::new(),
            subtitle: format!("Calculate: {expression}"),
            icon: SearchResultIcon::Calculator,
            kind: SearchResultKind::Calculator { expression, result },
        },
        calc::Calculation::Error {
            expression,
            message,
        } => SearchResult {
            title: message.clone(),
            flair: String::new(),
            subtitle: format!("Calculate: {expression}"),
            icon: SearchResultIcon::Calculator,
            kind: SearchResultKind::CalculatorError {
                expression,
                message,
            },
        },
    }
}

pub(crate) fn unit_conversion_result(conversion: units::UnitConversion) -> SearchResult {
    SearchResult {
        title: conversion.result.clone(),
        flair: String::new(),
        subtitle: format!("Convert: {}", conversion.expression),
        icon: SearchResultIcon::UnitConversion,
        kind: SearchResultKind::UnitConversion {
            expression: conversion.expression,
            result: conversion.result,
        },
    }
}

pub(crate) fn currency_conversion_result(conversion: currency::CurrencyConversion) -> SearchResult {
    let subtitle = if let Some(date) = conversion.date.as_deref() {
        format!(
            "Currency: {} ({}, {date})",
            conversion.expression, conversion.provider
        )
    } else {
        format!(
            "Currency: {} ({})",
            conversion.expression, conversion.provider
        )
    };

    SearchResult {
        title: conversion.result.clone(),
        flair: String::new(),
        subtitle,
        icon: SearchResultIcon::CurrencyConversion,
        kind: SearchResultKind::CurrencyConversion {
            expression: conversion.expression,
            result: conversion.result,
        },
    }
}

pub(crate) fn currency_error_result(expression: &str, message: String) -> SearchResult {
    SearchResult {
        title: "Currency conversion unavailable.".to_owned(),
        flair: String::new(),
        subtitle: format!("Currency: {expression}"),
        icon: SearchResultIcon::CurrencyConversion,
        kind: SearchResultKind::CurrencyConversionError {
            expression: expression.to_owned(),
            message,
        },
    }
}

pub(crate) fn time_lookup_result(lookup: time_lookup::TimeLookup) -> SearchResult {
    SearchResult {
        title: lookup.result.clone(),
        flair: String::new(),
        subtitle: format!(
            "Time in {} ({}, {})",
            lookup.location, lookup.offset, lookup.timezone
        ),
        icon: SearchResultIcon::TimeLookup,
        kind: SearchResultKind::TimeLookup {
            expression: lookup.expression,
            result: lookup.result,
        },
    }
}

pub(crate) fn time_lookup_error_result(expression: &str, message: String) -> SearchResult {
    SearchResult {
        title: "Time lookup unavailable.".to_owned(),
        flair: String::new(),
        subtitle: format!("Time: {expression}"),
        icon: SearchResultIcon::TimeLookup,
        kind: SearchResultKind::TimeLookupError {
            expression: expression.to_owned(),
            message,
        },
    }
}

pub(crate) fn utility_action_result(action: utility_actions::UtilityAction) -> SearchResult {
    let icon = match &action {
        utility_actions::UtilityAction::System(action) => match action.kind {
            SystemActionKind::Reboot => SearchResultIcon::SystemReboot,
            SystemActionKind::Shutdown => SearchResultIcon::SystemShutdown,
            SystemActionKind::Logout => SearchResultIcon::SystemLogout,
            SystemActionKind::Lock => SearchResultIcon::SystemLock,
        },
        utility_actions::UtilityAction::Timer(_) => SearchResultIcon::Timer,
    };

    SearchResult {
        title: utility_actions::action_title(&action),
        flair: String::new(),
        subtitle: utility_actions::action_subtitle(&action),
        icon,
        kind: SearchResultKind::UtilityAction { action },
    }
}

pub(crate) fn utility_action_error_result(expression: &str, message: String) -> SearchResult {
    SearchResult {
        title: message.clone(),
        flair: String::new(),
        subtitle: format!("Command: {expression}"),
        icon: SearchResultIcon::Timer,
        kind: SearchResultKind::UtilityActionError {
            expression: expression.to_owned(),
            message,
        },
    }
}

pub(crate) fn web_search_result(search: web_search::WebSearch) -> SearchResult {
    let subtitle = if search.host.is_empty() {
        format!("Custom search - {}", search.keyword)
    } else {
        format!("{} - {}", search.host, search.keyword)
    };

    SearchResult {
        title: format!("Search {} for {}", search.name, search.query),
        flair: String::new(),
        subtitle,
        icon: SearchResultIcon::WebSearch {
            label: search.icon_label,
        },
        kind: SearchResultKind::WebSearch {
            name: search.name,
            url: search.url,
        },
    }
}

pub(super) fn no_results(query: &str, _providers: &ProviderConfig) -> SearchResult {
    SearchResult {
        title: format!("No matches for {query}"),
        flair: String::new(),
        subtitle: "No matches".to_owned(),
        icon: SearchResultIcon::Placeholder,
        kind: SearchResultKind::NoResults {
            query: query.to_owned(),
        },
    }
}

pub(super) fn disabled_providers_result() -> SearchResult {
    SearchResult {
        title: "No providers enabled".to_owned(),
        flair: String::new(),
        subtitle: "Enable search providers in Settings".to_owned(),
        icon: SearchResultIcon::Placeholder,
        kind: SearchResultKind::Placeholder,
    }
}

pub fn display_path(path: &Path) -> String {
    dirs::home_dir()
        .map(|home| display_path_for_home(path, &home))
        .unwrap_or_else(|| path.display().to_string())
}

fn app_subtitle(app: &DesktopApp) -> String {
    app.comment
        .as_ref()
        .or(app.generic_name.as_ref())
        .cloned()
        .unwrap_or_else(|| "Application".to_owned())
}

pub(super) fn display_path_for_home(path: &Path, home: &Path) -> String {
    if path == home {
        return "~".to_owned();
    }

    path.strip_prefix(home)
        .ok()
        .and_then(|relative| {
            let relative = relative.to_str()?;
            Some(format!("~/{relative}"))
        })
        .unwrap_or_else(|| path.display().to_string())
}
