mod fixtures;

use fixtures::{app, project, ranking_with_launches};
use rayslash_core::{config::ProviderConfig, ranking::RankingState, search};

#[test]
fn mixed_search_orders_apps_projects_and_calculator_with_fixture_data() {
    let projects = vec![
        project("/tmp/rayslash", "rayslash"),
        project("/tmp/x-ray-sidecar", "x-ray-sidecar"),
    ];
    let apps = vec![
        app("calculator.desktop", "Calculator"),
        app("rayslash.desktop", "Rayslash"),
    ];

    let empty_results = search::mixed_results(&projects, &apps, "");

    assert_eq!(
        empty_results
            .iter()
            .map(|result| result.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Calculator", "Rayslash", "rayslash", "x-ray-sidecar"]
    );

    let fuzzy_results = search::mixed_results(&projects, &apps, "ray");

    assert_eq!(
        fuzzy_results
            .iter()
            .map(|result| result.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Rayslash", "rayslash", "x-ray-sidecar"]
    );

    let calculator_results = search::mixed_results(&projects, &apps, "2 + 2");

    assert_eq!(calculator_results[0].title, "4");
    assert_eq!(calculator_results[0].calculator_result(), Some("4"));
}

#[test]
fn learned_ranking_integration_respects_provider_toggles_and_calculator_precedence() {
    let apps = vec![
        app("alpha.desktop", "Alpha"),
        app("alpine.desktop", "Alpine"),
    ];
    let ranking = ranking_with_launches("app:alpine.desktop", "al", 3);

    let learned = search::mixed_results_with_ranking(
        &[],
        &apps,
        "al",
        &ProviderConfig::default(),
        Some(&ranking),
    );

    assert_eq!(learned[0].title, "Alpine");

    let providers = ProviderConfig {
        apps: false,
        folders: true,
        calculator: true,
    };
    let projects = vec![project("/tmp/alpha-project", "Alpha Project")];
    let hidden =
        search::mixed_results_with_ranking(&projects, &apps, "al", &providers, Some(&ranking));

    assert_eq!(hidden[0].title, "Alpha Project");

    let calculator_first = search::mixed_results_with_ranking(
        &[],
        &apps,
        "2 + 2",
        &ProviderConfig::default(),
        Some(&ranking),
    );

    assert_eq!(calculator_first[0].title, "4");
}

#[test]
fn mixed_search_provider_and_empty_index_rows_respect_provider_toggles() {
    let providers = ProviderConfig {
        apps: false,
        folders: true,
        calculator: false,
    };
    let projects = vec![project("/tmp/rayslash", "rayslash")];
    let apps = vec![app("rayslash.desktop", "Rayslash")];

    let folder_only = search::mixed_results_with_providers(&projects, &apps, "ray", &providers);

    assert_eq!(folder_only.len(), 1);
    assert_eq!(folder_only[0].title, "rayslash");
    assert!(folder_only[0].project_path().is_some());

    let disabled_calculator = search::mixed_results_with_providers(
        &[],
        &[],
        "2 + 2",
        &ProviderConfig {
            apps: true,
            folders: true,
            calculator: false,
        },
    );

    assert_eq!(
        disabled_calculator
            .iter()
            .map(|result| result.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Open applications", "Find folders"]
    );

    let calculator_only = search::mixed_results_with_providers(
        &[],
        &[],
        "not math",
        &ProviderConfig {
            apps: false,
            folders: false,
            calculator: true,
        },
    );

    assert_eq!(calculator_only.len(), 1);
    assert_eq!(calculator_only[0].title, "Calculate");

    let no_providers = search::mixed_results_with_providers(
        &[],
        &[],
        "2 + 2",
        &ProviderConfig {
            apps: false,
            folders: false,
            calculator: false,
        },
    );

    assert_eq!(no_providers[0].title, "No providers enabled");
}

#[test]
fn mixed_search_distinguishes_calculator_errors_normal_queries_placeholders_and_no_results() {
    let apps = vec![app("calculator.desktop", "Calculator")];

    let error = search::mixed_results(&[], &apps, "10 / 0");

    assert_eq!(error[0].title, "Division by zero is not possible.");
    assert_eq!(
        error[0].calculator_error_message(),
        Some("Division by zero is not possible.")
    );

    let normal_query = search::mixed_results(&[], &apps, "calculator");

    assert_eq!(normal_query[0].title, "Calculator");
    assert!(normal_query[0].app_command().is_some());
    assert!(normal_query[0].calculator_result().is_none());

    assert_eq!(
        search::mixed_results(&[], &[], "anything"),
        search::placeholder_results()
    );

    let no_results = search::mixed_results(&[project("/tmp/rayslash", "rayslash")], &[], "zzz");

    assert_eq!(no_results[0].title, "No results");
    assert_eq!(
        no_results[0].subtitle,
        "No apps, folders, or calculations match \"zzz\""
    );
    assert!(no_results[0].is_no_results());
}

#[test]
fn mixed_search_no_results_wording_uses_enabled_provider_names() {
    let providers = ProviderConfig {
        apps: false,
        folders: true,
        calculator: false,
    };
    let projects = vec![project("/tmp/rayslash", "rayslash")];

    let no_results = search::mixed_results_with_providers(&projects, &[], "zzz", &providers);

    assert_eq!(no_results[0].title, "No results");
    assert_eq!(no_results[0].subtitle, "No folders match \"zzz\"");
}

#[test]
fn learned_ranking_integration_keeps_strong_textual_matches_above_weaker_history() {
    let projects = vec![project("/tmp/x-ray-sidecar", "x-ray-sidecar")];
    let apps = vec![app("rayslash.desktop", "Rayslash")];
    let mut ranking = RankingState::default();
    for second in 1..=10 {
        ranking.record_launch_at(
            "folder:/tmp/x-ray-sidecar",
            "ray",
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(second),
        );
    }

    let results = search::mixed_results_with_ranking(
        &projects,
        &apps,
        "ray",
        &ProviderConfig::default(),
        Some(&ranking),
    );

    assert_eq!(
        results
            .iter()
            .map(|result| result.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Rayslash", "x-ray-sidecar"]
    );
}
