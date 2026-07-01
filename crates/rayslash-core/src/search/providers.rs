use std::path::Path;

use crate::apps::DesktopApp;
use crate::calc;
use crate::projects::Project;
use nucleo_matcher::Utf32Str;

use super::matcher::{fuzzy_matcher, fuzzy_pattern, project_order};
use super::{SearchResult, SearchResultIcon, SearchResultKind};

pub fn placeholder_results() -> Vec<SearchResult> {
    vec![
        SearchResult {
            title: "Open applications".to_owned(),
            subtitle: "Desktop app search will land in Phase 3".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        },
        SearchResult {
            title: "Find projects".to_owned(),
            subtitle: "Project folder search will land in Phase 2".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        },
        SearchResult {
            title: "Calculate".to_owned(),
            subtitle: "Type an expression such as 2 + 2".to_owned(),
            icon: SearchResultIcon::Placeholder,
            kind: SearchResultKind::Placeholder,
        },
    ]
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

pub(super) fn project_result(project: &Project) -> SearchResult {
    let subtitle = dirs::home_dir()
        .map(|home| display_path_for_home(&project.path, &home))
        .unwrap_or_else(|| project.path.display().to_string());

    project_result_with_subtitle(project, subtitle)
}

pub(super) fn project_result_with_subtitle(project: &Project, subtitle: String) -> SearchResult {
    SearchResult {
        title: project.name.clone(),
        subtitle,
        icon: SearchResultIcon::ProjectFolder,
        kind: SearchResultKind::Project {
            path: project.path.clone(),
        },
    }
}

pub(super) fn app_result(app: &DesktopApp) -> SearchResult {
    SearchResult {
        title: app.name.clone(),
        subtitle: app_subtitle(app),
        icon: SearchResultIcon::App {
            path: app.icon_path.clone(),
        },
        kind: SearchResultKind::App {
            id: app.id.clone(),
            command: app.command.clone(),
        },
    }
}

pub(super) fn calculator_result(calculation: calc::Calculation) -> SearchResult {
    match calculation {
        calc::Calculation::Value { expression, result } => SearchResult {
            title: result.clone(),
            subtitle: format!("Calculate: {expression}"),
            icon: SearchResultIcon::Calculator,
            kind: SearchResultKind::Calculator { expression, result },
        },
        calc::Calculation::Error {
            expression,
            message,
        } => SearchResult {
            title: message.clone(),
            subtitle: format!("Calculate: {expression}"),
            icon: SearchResultIcon::Calculator,
            kind: SearchResultKind::CalculatorError {
                expression,
                message,
            },
        },
    }
}

pub(super) fn no_results(query: &str) -> SearchResult {
    SearchResult {
        title: "No results".to_owned(),
        subtitle: format!("No apps, projects, or calculations match \"{query}\""),
        icon: SearchResultIcon::Placeholder,
        kind: SearchResultKind::NoResults {
            query: query.to_owned(),
        },
    }
}

pub(super) fn disabled_providers_result() -> SearchResult {
    SearchResult {
        title: "No providers enabled".to_owned(),
        subtitle: "Enable Apps, Folders, or Calculator in Settings".to_owned(),
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
        .map(|detail| format!("Application - {detail}"))
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
