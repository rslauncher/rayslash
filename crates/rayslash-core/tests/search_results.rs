mod fixtures;

use std::path::{Path, PathBuf};

use fixtures::{app, project};
use rayslash_core::search;

#[test]
fn placeholder_results_are_available() {
    assert_eq!(search::placeholder_results().len(), 3);
}

#[test]
fn project_results_fuzzy_match_partial_non_contiguous_queries() {
    let projects = vec![
        project("/tmp/rayslash", "rayslash"),
        project("/tmp/other", "Other"),
    ];

    let results = search::project_results(&projects, "RS");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "rayslash");
    assert_eq!(results[0].project_path(), Some(Path::new("/tmp/rayslash")));
}

#[test]
fn project_results_rank_better_matches_before_weaker_matches() {
    let projects = vec![
        project("/tmp/x-ray-sidecar", "x-ray-sidecar"),
        project("/tmp/rayslash", "rayslash"),
    ];

    let results = search::project_results(&projects, "ray");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "rayslash");
    assert_eq!(results[1].title, "x-ray-sidecar");
}

#[test]
fn project_results_show_all_projects_for_empty_query_in_sorted_order() {
    let projects = vec![project("/tmp/zeta", "zeta"), project("/tmp/alpha", "Alpha")];

    let results = search::project_results(&projects, "");

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
    let projects = vec![project("/tmp/rayslash", "rayslash")];

    let results = search::project_results(&projects, "zzz");

    assert!(results.is_empty());
}

#[test]
fn project_results_preserve_placeholders_when_no_projects_exist() {
    let results = search::project_results(&[], "anything");

    assert_eq!(results, search::placeholder_results());
}

#[test]
fn current_result_types_have_stable_ids() {
    let apps = vec![app("editor.desktop", "Editor")];
    let projects = vec![project(PathBuf::from("/tmp/rayslash"), "rayslash")];

    let app_result = search::mixed_results(&[], &apps, "editor")
        .into_iter()
        .next()
        .expect("app result");
    let project_result = search::project_results(&projects, "ray")
        .into_iter()
        .next()
        .expect("project result");
    let calculator_result = search::mixed_results(&[], &[], "2 + 2")
        .into_iter()
        .next()
        .expect("calculator result");
    let no_results = search::mixed_results(&projects, &[], "zzz")
        .into_iter()
        .next()
        .expect("no results row");

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
