mod matcher;
mod providers;
mod result;

use crate::apps::DesktopApp;
use crate::calc;
use crate::config::ProviderConfig;
use crate::projects::Project;
use crate::ranking::RankingState;

use matcher::{boosted_score, fuzzy_matcher, fuzzy_pattern, search_result_order};
use nucleo_matcher::Utf32Str;
use providers::{
    app_result, calculator_result, disabled_providers_result, no_results, project_result,
};
pub use providers::{display_path, placeholder_results, project_results};
#[cfg(test)]
use providers::{display_path_for_home, project_result_with_subtitle};
pub use result::{SearchResult, SearchResultIcon, SearchResultKind};

pub fn mixed_results(projects: &[Project], apps: &[DesktopApp], query: &str) -> Vec<SearchResult> {
    mixed_results_with_providers(projects, apps, query, &ProviderConfig::default())
}

pub fn mixed_results_with_providers(
    projects: &[Project],
    apps: &[DesktopApp],
    query: &str,
    providers: &ProviderConfig,
) -> Vec<SearchResult> {
    mixed_results_with_ranking(projects, apps, query, providers, None)
}

pub fn mixed_results_with_ranking(
    projects: &[Project],
    apps: &[DesktopApp],
    query: &str,
    providers: &ProviderConfig,
    ranking: Option<&RankingState>,
) -> Vec<SearchResult> {
    let query = query.trim();
    let calculation = providers
        .calculator
        .then(|| calc::calculate(query).map(calculator_result))
        .flatten();

    if !providers.apps && !providers.folders && !providers.calculator {
        return vec![disabled_providers_result()];
    }

    let enabled_projects = if providers.folders { projects } else { &[] };
    let enabled_apps = if providers.apps { apps } else { &[] };

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
            let result = project_result(project);
            let boosted_score = boosted_score(&result, score, query, ranking);
            matches.push((result, score, boosted_score));
        }
    }

    for app in enabled_apps {
        let haystack = Utf32Str::new(&app.name, &mut char_buf);
        if let Some(score) = pattern.score(haystack, &mut matcher) {
            let result = app_result(app);
            let boosted_score = boosted_score(&result, score, query, ranking);
            matches.push((result, score, boosted_score));
        }
    }

    matches.sort_by(
        |(a, a_score, a_boosted_score), (b, b_score, b_boosted_score)| {
            b_boosted_score
                .cmp(a_boosted_score)
                .then_with(|| b_score.cmp(a_score))
                .then_with(|| search_result_order(a, b))
        },
    );

    let mut results = matches
        .into_iter()
        .map(|(result, _score, _boosted_score)| result)
        .collect::<Vec<_>>();

    if let Some(calculation) = calculation {
        results.insert(0, calculation);
    }

    if results.is_empty() {
        results.push(no_results(query));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::CommandSpec;
    use std::path::{Path, PathBuf};

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
    fn current_result_types_have_stable_ids() {
        let app = DesktopApp {
            id: "editor.desktop".to_owned(),
            name: "Editor".to_owned(),
            generic_name: None,
            comment: None,
            exec: "editor".to_owned(),
            icon: None,
            icon_path: None,
            command: CommandSpec {
                program: "editor".into(),
                args: Vec::new(),
            },
            desktop_file: PathBuf::from("/tmp/editor.desktop"),
        };
        let project = Project {
            name: "rayslash".to_owned(),
            path: PathBuf::from("/tmp/rayslash"),
        };

        let app_result = app_result(&app);
        let project_result = project_result_with_subtitle(&project, "/tmp/rayslash".to_owned());
        let calculator_result = mixed_results(&[], &[], "2 + 2")
            .into_iter()
            .next()
            .expect("calculator result");
        let no_results = no_results("zzz");

        assert_eq!(
            app_result.stable_id(),
            Some("app:editor.desktop".to_owned())
        );
        assert_eq!(
            app_result.learning_id(),
            Some("app:editor.desktop".to_owned())
        );
        assert_eq!(
            project_result.stable_id(),
            Some("folder:/tmp/rayslash".to_owned())
        );
        assert_eq!(
            project_result.learning_id(),
            Some("folder:/tmp/rayslash".to_owned())
        );
        assert_eq!(
            calculator_result.stable_id(),
            Some("calculator:2 + 2".to_owned())
        );
        assert_eq!(calculator_result.learning_id(), None);
        assert_eq!(no_results.stable_id(), Some("no-results:zzz".to_owned()));
        assert_eq!(no_results.learning_id(), None);
    }
}
