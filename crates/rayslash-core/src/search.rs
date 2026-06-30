use std::path::{Path, PathBuf};

use crate::actions::CommandSpec;
use crate::apps::DesktopApp;
use crate::projects::Project;
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: SearchResultKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchResultKind {
    Placeholder,
    App { id: String, command: CommandSpec },
    Project { path: PathBuf },
}

impl SearchResult {
    pub fn project_path(&self) -> Option<&Path> {
        match &self.kind {
            SearchResultKind::Placeholder => None,
            SearchResultKind::App { .. } => None,
            SearchResultKind::Project { path } => Some(path),
        }
    }

    pub fn app_command(&self) -> Option<&CommandSpec> {
        match &self.kind {
            SearchResultKind::App { command, .. } => Some(command),
            SearchResultKind::Placeholder | SearchResultKind::Project { .. } => None,
        }
    }
}

pub fn placeholder_results() -> Vec<SearchResult> {
    vec![
        SearchResult {
            title: "Open applications".to_owned(),
            subtitle: "Desktop app search will land in Phase 3".to_owned(),
            kind: SearchResultKind::Placeholder,
        },
        SearchResult {
            title: "Find projects".to_owned(),
            subtitle: "Project folder search will land in Phase 2".to_owned(),
            kind: SearchResultKind::Placeholder,
        },
        SearchResult {
            title: "Calculate".to_owned(),
            subtitle: "Calculator support will land in Phase 4".to_owned(),
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
    if projects.is_empty() && apps.is_empty() {
        return placeholder_results();
    }

    let query = query.trim();

    if query.is_empty() {
        let mut results = projects
            .iter()
            .map(project_result)
            .chain(apps.iter().map(app_result))
            .collect::<Vec<_>>();
        results.sort_by(search_result_order);
        return results;
    }

    let pattern = fuzzy_pattern(query);
    let mut matcher = fuzzy_matcher();
    let mut char_buf = Vec::new();

    let mut matches = Vec::new();

    for project in projects {
        let haystack = Utf32Str::new(&project.name, &mut char_buf);
        if let Some(score) = pattern.score(haystack, &mut matcher) {
            matches.push((project_result(project), score));
        }
    }

    for app in apps {
        let haystack = Utf32Str::new(&app.name, &mut char_buf);
        if let Some(score) = pattern.score(haystack, &mut matcher) {
            matches.push((app_result(app), score));
        }
    }

    matches.sort_by(|(a, a_score), (b, b_score)| {
        b_score.cmp(a_score).then_with(|| search_result_order(a, b))
    });

    matches.into_iter().map(|(result, _score)| result).collect()
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
        kind: SearchResultKind::Project {
            path: project.path.clone(),
        },
    }
}

fn app_result(app: &DesktopApp) -> SearchResult {
    SearchResult {
        title: app.name.clone(),
        subtitle: app_subtitle(app),
        kind: SearchResultKind::App {
            id: app.id.clone(),
            command: app.command.clone(),
        },
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
        SearchResultKind::App { .. } => 0,
        SearchResultKind::Project { .. } => 1,
        SearchResultKind::Placeholder => 2,
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
    }

    #[test]
    fn mixed_results_show_apps_and_projects_for_empty_query() {
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
        assert_eq!(results[1].project_path(), Some(Path::new("/tmp/rayslash")));
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
    fn mixed_results_use_placeholders_only_when_no_real_results_exist() {
        assert_eq!(mixed_results(&[], &[], "anything"), placeholder_results());
    }
}
