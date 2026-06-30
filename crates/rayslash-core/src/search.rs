use std::path::{Path, PathBuf};

use crate::actions::CommandSpec;
use crate::apps::DesktopApp;
use crate::calc;
use crate::config::ProviderConfig;
use crate::projects::Project;
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub icon: SearchResultIcon,
    pub kind: SearchResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultIcon {
    Placeholder,
    Calculator,
    App { path: Option<PathBuf> },
    ProjectFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultKind {
    Placeholder,
    NoResults { query: String },
    Calculator { expression: String, result: String },
    CalculatorError { expression: String, message: String },
    App { id: String, command: CommandSpec },
    Project { path: PathBuf },
}

impl SearchResult {
    pub fn project_path(&self) -> Option<&Path> {
        match &self.kind {
            SearchResultKind::Placeholder => None,
            SearchResultKind::NoResults { .. } => None,
            SearchResultKind::Calculator { .. } => None,
            SearchResultKind::CalculatorError { .. } => None,
            SearchResultKind::App { .. } => None,
            SearchResultKind::Project { path } => Some(path),
        }
    }

    pub fn app_command(&self) -> Option<&CommandSpec> {
        match &self.kind {
            SearchResultKind::App { command, .. } => Some(command),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::Calculator { .. }
            | SearchResultKind::CalculatorError { .. }
            | SearchResultKind::Project { .. } => None,
        }
    }

    pub fn calculator_result(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::Calculator { result, .. } => Some(result),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::CalculatorError { .. }
            | SearchResultKind::App { .. }
            | SearchResultKind::Project { .. } => None,
        }
    }

    pub fn calculator_error_message(&self) -> Option<&str> {
        match &self.kind {
            SearchResultKind::CalculatorError { message, .. } => Some(message),
            SearchResultKind::Placeholder
            | SearchResultKind::NoResults { .. }
            | SearchResultKind::Calculator { .. }
            | SearchResultKind::App { .. }
            | SearchResultKind::Project { .. } => None,
        }
    }

    pub fn is_no_results(&self) -> bool {
        matches!(self.kind, SearchResultKind::NoResults { .. })
    }
}

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

    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );
    let mut config = Config::DEFAULT;
    config.prefer_prefix = true;
    let mut matcher = Matcher::new(config);
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

pub fn mixed_results(projects: &[Project], apps: &[DesktopApp], query: &str) -> Vec<SearchResult> {
    mixed_results_with_providers(projects, apps, query, &ProviderConfig::default())
}

pub fn mixed_results_with_providers(
    projects: &[Project],
    apps: &[DesktopApp],
    query: &str,
    providers: &ProviderConfig,
) -> Vec<SearchResult> {
    let query = query.trim();
    let calculation = providers
        .calculator
        .then(|| calc::calculate(query).map(calculator_result))
        .flatten();

    if !providers.apps && !providers.folders && !providers.calculator {
        return vec![disabled_providers_result()];
    }

    let enabled_projects = providers.folders.then_some(projects).unwrap_or(&[]);
    let enabled_apps = providers.apps.then_some(apps).unwrap_or(&[]);

    if enabled_projects.is_empty() && enabled_apps.is_empty() {
        return calculation
            .map(|result| vec![result])
            .unwrap_or_else(placeholder_results);
    }

    if query.is_empty() {
        let mut results = enabled_projects
            .iter()
            .map(project_result)
            .chain(enabled_apps.iter().map(app_result))
            .collect::<Vec<_>>();
        results.sort_by(search_result_order);
        return results;
    }

    let pattern = fuzzy_pattern(query);
    let mut matcher = fuzzy_matcher();
    let mut char_buf = Vec::new();

    let mut matches = Vec::new();

    for project in enabled_projects {
        let haystack = Utf32Str::new(&project.name, &mut char_buf);
        if let Some(score) = pattern.score(haystack, &mut matcher) {
            matches.push((project_result(project), score));
        }
    }

    for app in enabled_apps {
        let haystack = Utf32Str::new(&app.name, &mut char_buf);
        if let Some(score) = pattern.score(haystack, &mut matcher) {
            matches.push((app_result(app), score));
        }
    }

    matches.sort_by(|(a, a_score), (b, b_score)| {
        b_score.cmp(a_score).then_with(|| search_result_order(a, b))
    });

    let mut results = matches
        .into_iter()
        .map(|(result, _score)| result)
        .collect::<Vec<_>>();

    if let Some(calculation) = calculation {
        results.insert(0, calculation);
    }

    if results.is_empty() {
        results.push(no_results(query));
    }

    results
}

fn project_result(project: &Project) -> SearchResult {
    let subtitle = dirs::home_dir()
        .map(|home| display_path_for_home(&project.path, &home))
        .unwrap_or_else(|| project.path.display().to_string());

    project_result_with_subtitle(project, subtitle)
}

fn project_result_with_subtitle(project: &Project, subtitle: String) -> SearchResult {
    SearchResult {
        title: project.name.clone(),
        subtitle,
        icon: SearchResultIcon::ProjectFolder,
        kind: SearchResultKind::Project {
            path: project.path.clone(),
        },
    }
}

fn app_result(app: &DesktopApp) -> SearchResult {
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

fn calculator_result(calculation: calc::Calculation) -> SearchResult {
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

fn no_results(query: &str) -> SearchResult {
    SearchResult {
        title: "No results".to_owned(),
        subtitle: format!("No apps, projects, or calculations match \"{query}\""),
        icon: SearchResultIcon::Placeholder,
        kind: SearchResultKind::NoResults {
            query: query.to_owned(),
        },
    }
}

fn disabled_providers_result() -> SearchResult {
    SearchResult {
        title: "No providers enabled".to_owned(),
        subtitle: "Enable Apps, Folders, or Calculator in Settings".to_owned(),
        icon: SearchResultIcon::Placeholder,
        kind: SearchResultKind::Placeholder,
    }
}

fn app_subtitle(app: &DesktopApp) -> String {
    app.comment
        .as_ref()
        .or(app.generic_name.as_ref())
        .map(|detail| format!("Application - {detail}"))
        .unwrap_or_else(|| "Application".to_owned())
}

pub fn display_path(path: &Path) -> String {
    dirs::home_dir()
        .map(|home| display_path_for_home(path, &home))
        .unwrap_or_else(|| path.display().to_string())
}

fn display_path_for_home(path: &Path, home: &Path) -> String {
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

fn project_order(a: &Project, b: &Project) -> std::cmp::Ordering {
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then_with(|| a.path.cmp(&b.path))
}

fn search_result_order(a: &SearchResult, b: &SearchResult) -> std::cmp::Ordering {
    a.title
        .to_lowercase()
        .cmp(&b.title.to_lowercase())
        .then_with(|| result_type_order(&a.kind).cmp(&result_type_order(&b.kind)))
        .then_with(|| a.subtitle.cmp(&b.subtitle))
}

fn result_type_order(kind: &SearchResultKind) -> u8 {
    match kind {
        SearchResultKind::Calculator { .. } => 0,
        SearchResultKind::CalculatorError { .. } => 0,
        SearchResultKind::App { .. } => 1,
        SearchResultKind::Project { .. } => 2,
        SearchResultKind::Placeholder | SearchResultKind::NoResults { .. } => 3,
    }
}

fn fuzzy_pattern(query: &str) -> Pattern {
    Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    )
}

fn fuzzy_matcher() -> Matcher {
    let mut config = Config::DEFAULT;
    config.prefer_prefix = true;
    Matcher::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_results_are_available() {
        assert_eq!(placeholder_results().len(), 3);
    }

    #[test]
    fn project_results_fuzzy_match_partial_non_contiguous_queries() {
        let projects = vec![
            Project {
                name: "rayslash".to_owned(),
                path: PathBuf::from("/tmp/rayslash"),
            },
            Project {
                name: "Other".to_owned(),
                path: PathBuf::from("/tmp/other"),
            },
        ];

        let results = project_results(&projects, "RS");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "rayslash");
        assert_eq!(results[0].project_path(), Some(Path::new("/tmp/rayslash")));
    }

    #[test]
    fn project_results_rank_better_matches_before_weaker_matches() {
        let projects = vec![
            Project {
                name: "x-ray-sidecar".to_owned(),
                path: PathBuf::from("/tmp/x-ray-sidecar"),
            },
            Project {
                name: "rayslash".to_owned(),
                path: PathBuf::from("/tmp/rayslash"),
            },
        ];

        let results = project_results(&projects, "ray");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "rayslash");
        assert_eq!(results[1].title, "x-ray-sidecar");
    }

    #[test]
    fn project_results_show_all_projects_for_empty_query_in_sorted_order() {
        let projects = vec![
            Project {
                name: "zeta".to_owned(),
                path: PathBuf::from("/tmp/zeta"),
            },
            Project {
                name: "Alpha".to_owned(),
                path: PathBuf::from("/tmp/alpha"),
            },
        ];

        let results = project_results(&projects, "");

        assert_eq!(
            results
                .iter()
                .map(|result| &result.title)
                .collect::<Vec<_>>(),
            vec!["Alpha", "zeta"]
        );
    }

    #[test]
    fn project_results_return_empty_list_when_query_does_not_match() {
        let projects = vec![Project {
            name: "rayslash".to_owned(),
            path: PathBuf::from("/tmp/rayslash"),
        }];

        let results = project_results(&projects, "zzz");

        assert!(results.is_empty());
    }

    #[test]
    fn project_results_preserve_placeholders_when_no_projects_exist() {
        let results = project_results(&[], "anything");

        assert_eq!(results, placeholder_results());
    }

    #[test]
    fn display_path_shortens_paths_under_home() {
        let home = PathBuf::from("/home/example");
        let path = home.join("Documents/Projects/rayslash");

        assert_eq!(
            display_path_for_home(&path, &home),
            "~/Documents/Projects/rayslash"
        );
    }

    #[test]
    fn display_path_shortens_home_itself() {
        let home = PathBuf::from("/home/example");

        assert_eq!(display_path_for_home(&home, &home), "~");
    }

    #[test]
    fn display_path_keeps_paths_outside_home_unchanged() {
        let home = PathBuf::from("/home/example");
        let path = PathBuf::from("/tmp/rayslash");

        assert_eq!(display_path_for_home(&path, &home), "/tmp/rayslash");
    }

    #[test]
    fn project_results_use_shortened_subtitle_without_changing_launch_path() {
        let home = PathBuf::from("/home/example");
        let path = home.join("Projects/rayslash");
        let project = Project {
            name: "rayslash".to_owned(),
            path: path.clone(),
        };

        let result = project_result_with_subtitle(&project, display_path_for_home(&path, &home));

        assert_eq!(result.subtitle, "~/Projects/rayslash");
        assert_eq!(result.project_path(), Some(path.as_path()));
        assert_eq!(result.icon, SearchResultIcon::ProjectFolder);
    }

    #[test]
    fn mixed_results_show_apps_and_projects_for_empty_query() {
        let app_icon = PathBuf::from("/tmp/calculator.svg");
        let projects = vec![Project {
            name: "rayslash".to_owned(),
            path: PathBuf::from("/tmp/rayslash"),
        }];
        let apps = vec![DesktopApp {
            id: "calculator.desktop".to_owned(),
            name: "Calculator".to_owned(),
            generic_name: Some("Calculator".to_owned()),
            comment: Some("Perform arithmetic".to_owned()),
            exec: "calculator".to_owned(),
            icon: None,
            icon_path: Some(app_icon.clone()),
            command: CommandSpec {
                program: "calculator".into(),
                args: Vec::new(),
            },
            desktop_file: PathBuf::from("/tmp/calculator.desktop"),
        }];

        let results = mixed_results(&projects, &apps, "");

        assert_eq!(
            results
                .iter()
                .map(|result| result.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Calculator", "rayslash"]
        );
        assert_eq!(results[0].subtitle, "Application - Perform arithmetic");
        assert!(results[0].app_command().is_some());
        assert_eq!(
            results[0].icon,
            SearchResultIcon::App {
                path: Some(app_icon)
            }
        );
        assert_eq!(results[1].project_path(), Some(Path::new("/tmp/rayslash")));
        assert_eq!(results[1].icon, SearchResultIcon::ProjectFolder);
    }

    #[test]
    fn mixed_results_rank_apps_and_projects_by_fuzzy_score() {
        let projects = vec![Project {
            name: "x-ray-sidecar".to_owned(),
            path: PathBuf::from("/tmp/x-ray-sidecar"),
        }];
        let apps = vec![DesktopApp {
            id: "rayslash.desktop".to_owned(),
            name: "Rayslash".to_owned(),
            generic_name: None,
            comment: None,
            exec: "rayslash".to_owned(),
            icon: None,
            icon_path: None,
            command: CommandSpec {
                program: "rayslash".into(),
                args: Vec::new(),
            },
            desktop_file: PathBuf::from("/tmp/rayslash.desktop"),
        }];

        let results = mixed_results(&projects, &apps, "ray");

        assert_eq!(
            results
                .iter()
                .map(|result| result.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Rayslash", "x-ray-sidecar"]
        );
    }

    #[test]
    fn mixed_results_rank_calculator_result_first_for_valid_expression() {
        let projects = vec![Project {
            name: "2-plus-2-notes".to_owned(),
            path: PathBuf::from("/tmp/2-plus-2-notes"),
        }];
        let apps = vec![DesktopApp {
            id: "calculator.desktop".to_owned(),
            name: "Calculator".to_owned(),
            generic_name: Some("Calculator".to_owned()),
            comment: Some("Perform arithmetic".to_owned()),
            exec: "calculator".to_owned(),
            icon: None,
            icon_path: None,
            command: CommandSpec {
                program: "calculator".into(),
                args: Vec::new(),
            },
            desktop_file: PathBuf::from("/tmp/calculator.desktop"),
        }];

        let results = mixed_results(&projects, &apps, "2 + 2");

        assert_eq!(results[0].title, "4");
        assert_eq!(results[0].subtitle, "Calculate: 2 + 2");
        assert_eq!(results[0].calculator_result(), Some("4"));
        assert_eq!(results[0].icon, SearchResultIcon::Calculator);
    }

    #[test]
    fn mixed_results_respect_disabled_provider_settings() {
        let projects = vec![Project {
            name: "rayslash".to_owned(),
            path: PathBuf::from("/tmp/rayslash"),
        }];
        let apps = vec![DesktopApp {
            id: "rayslash.desktop".to_owned(),
            name: "Rayslash".to_owned(),
            generic_name: None,
            comment: None,
            exec: "rayslash".to_owned(),
            icon: None,
            icon_path: None,
            command: CommandSpec {
                program: "rayslash".into(),
                args: Vec::new(),
            },
            desktop_file: PathBuf::from("/tmp/rayslash.desktop"),
        }];
        let providers = ProviderConfig {
            apps: false,
            folders: true,
            calculator: false,
        };

        let results = mixed_results_with_providers(&projects, &apps, "ray", &providers);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "rayslash");
        assert!(results[0].project_path().is_some());
    }

    #[test]
    fn mixed_results_hide_calculator_rows_when_calculator_provider_is_disabled() {
        let providers = ProviderConfig {
            apps: true,
            folders: true,
            calculator: false,
        };

        let results = mixed_results_with_providers(&[], &[], "2 + 2", &providers);

        assert_eq!(results, placeholder_results());
    }

    #[test]
    fn mixed_results_show_disabled_provider_row_when_all_providers_are_off() {
        let providers = ProviderConfig {
            apps: false,
            folders: false,
            calculator: false,
        };

        let results = mixed_results_with_providers(&[], &[], "2 + 2", &providers);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "No providers enabled");
        assert_eq!(
            results[0].subtitle,
            "Enable Apps, Folders, or Calculator in Settings"
        );
    }

    #[test]
    fn mixed_results_show_superscript_exponent_calculator_results() {
        let results = mixed_results(&[], &[], "10²");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "100");
        assert_eq!(results[0].subtitle, "Calculate: 10²");
        assert_eq!(results[0].calculator_result(), Some("100"));
        assert_eq!(results[0].icon, SearchResultIcon::Calculator);
    }

    #[test]
    fn mixed_results_show_linear_equation_results() {
        let results = mixed_results(&[], &[], "x + 10 / 2 = 8");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "x = 3");
        assert_eq!(results[0].subtitle, "Calculate: x + 10 / 2 = 8");
        assert_eq!(results[0].calculator_result(), Some("x = 3"));
        assert_eq!(results[0].icon, SearchResultIcon::Calculator);
    }

    #[test]
    fn mixed_results_show_calculator_error_rows() {
        let apps = vec![DesktopApp {
            id: "calculator.desktop".to_owned(),
            name: "Calculator".to_owned(),
            generic_name: Some("Calculator".to_owned()),
            comment: Some("Perform arithmetic".to_owned()),
            exec: "calculator".to_owned(),
            icon: None,
            icon_path: None,
            command: CommandSpec {
                program: "calculator".into(),
                args: Vec::new(),
            },
            desktop_file: PathBuf::from("/tmp/calculator.desktop"),
        }];

        let results = mixed_results(&[], &apps, "10 / 0");

        assert_eq!(results[0].title, "Division by zero is not possible.");
        assert_eq!(results[0].subtitle, "Calculate: 10 / 0");
        assert_eq!(
            results[0].calculator_error_message(),
            Some("Division by zero is not possible.")
        );
        assert_eq!(results[0].icon, SearchResultIcon::Calculator);
    }

    #[test]
    fn mixed_results_do_not_treat_normal_queries_as_calculator_expressions() {
        let apps = vec![DesktopApp {
            id: "calculator.desktop".to_owned(),
            name: "Calculator".to_owned(),
            generic_name: Some("Calculator".to_owned()),
            comment: Some("Perform arithmetic".to_owned()),
            exec: "calculator".to_owned(),
            icon: None,
            icon_path: None,
            command: CommandSpec {
                program: "calculator".into(),
                args: Vec::new(),
            },
            desktop_file: PathBuf::from("/tmp/calculator.desktop"),
        }];

        let results = mixed_results(&[], &apps, "calculator");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Calculator");
        assert!(results[0].calculator_result().is_none());
        assert!(results[0].app_command().is_some());
    }

    #[test]
    fn mixed_results_use_placeholders_only_when_no_real_results_exist() {
        assert_eq!(mixed_results(&[], &[], "anything"), placeholder_results());
    }

    #[test]
    fn mixed_results_show_no_results_message_for_unmatched_real_indexes() {
        let projects = vec![Project {
            name: "rayslash".to_owned(),
            path: PathBuf::from("/tmp/rayslash"),
        }];

        let results = mixed_results(&projects, &[], "zzz");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "No results");
        assert_eq!(
            results[0].subtitle,
            "No apps, projects, or calculations match \"zzz\""
        );
        assert_eq!(
            results[0].kind,
            SearchResultKind::NoResults {
                query: "zzz".to_owned()
            }
        );
        assert!(results[0].is_no_results());
    }
}
