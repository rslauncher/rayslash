mod fixtures;

use fixtures::{app, project, ranking_with_launches};
use rayslash_core::{
    config::{AliasConfig, AliasKind, ProviderConfig, WebSearchConfig},
    ranking::RankingState,
    search,
    utility_actions::{SystemActionKind, UtilityAction},
};

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
        &[],
        "al",
        &ProviderConfig::default(),
        Some(&ranking),
    );

    assert_eq!(learned[0].title, "Alpine");

    let providers = ProviderConfig {
        apps: false,
        folders: true,
        calculator: true,
        aliases: true,
        ..ProviderConfig::default()
    };
    let projects = vec![project("/tmp/alpha-project", "Alpha Project")];
    let hidden =
        search::mixed_results_with_ranking(&projects, &apps, &[], "al", &providers, Some(&ranking));

    assert_eq!(hidden[0].title, "Alpha Project");

    let calculator_first = search::mixed_results_with_ranking(
        &[],
        &apps,
        &[],
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
        aliases: true,
        ..ProviderConfig::default()
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
            aliases: true,
            web_search: false,
            unit_conversion: false,
            currency_conversion: false,
            time_lookup: false,
            utility_actions: false,
        },
    );

    assert_eq!(
        disabled_calculator
            .iter()
            .map(|result| result.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Open applications", "Find folders", "Use aliases"]
    );

    let calculator_only = search::mixed_results_with_providers(
        &[],
        &[],
        "not math",
        &ProviderConfig {
            apps: false,
            folders: false,
            calculator: true,
            aliases: false,
            web_search: false,
            unit_conversion: false,
            currency_conversion: false,
            time_lookup: false,
            utility_actions: false,
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
            aliases: false,
            web_search: false,
            unit_conversion: false,
            currency_conversion: false,
            time_lookup: false,
            utility_actions: false,
        },
    );

    assert_eq!(no_providers[0].title, "No providers enabled");
}

#[test]
fn mixed_search_matches_alias_names_and_queries_when_provider_enabled() {
    let aliases = vec![
        AliasConfig {
            name: "GitHub".to_owned(),
            query: "gh".to_owned(),
            target: "https://github.com".to_owned(),
            kind: Some(AliasKind::Url),
        },
        AliasConfig {
            name: "Project notes".to_owned(),
            query: "notes".to_owned(),
            target: "~/Documents/notes.md".to_owned(),
            kind: Some(AliasKind::File),
        },
    ];

    let by_query = search::mixed_results_with_aliases(&[], &[], &aliases, "gh");

    assert_eq!(by_query[0].title, "GitHub");
    assert!(by_query[0].alias().is_some());
    assert_eq!(by_query[0].stable_id(), Some("alias:gh".to_owned()));

    let disabled = search::mixed_results_with_ranking(
        &[],
        &[],
        &aliases,
        "gh",
        &ProviderConfig {
            apps: false,
            folders: false,
            calculator: false,
            aliases: false,
            web_search: false,
            unit_conversion: false,
            currency_conversion: false,
            time_lookup: false,
            utility_actions: false,
        },
        None,
    );

    assert_eq!(disabled[0].title, "No providers enabled");
}

#[test]
fn mixed_search_supports_configured_web_search_templates() {
    let templates = vec![WebSearchConfig {
        name: "DuckDuckGo".to_owned(),
        keyword: "ddg".to_owned(),
        url: "https://duckduckgo.com/?q=%s".to_owned(),
        enabled: true,
    }];
    let providers = ProviderConfig {
        apps: false,
        folders: false,
        calculator: false,
        aliases: false,
        web_search: true,
        unit_conversion: false,
        currency_conversion: false,
        time_lookup: false,
        utility_actions: false,
    };

    let results = search::mixed_results_with_ranking_and_web_searches(
        &[],
        &[],
        &[],
        &templates,
        "ddg rust slint",
        &providers,
        None,
    );

    assert_eq!(results[0].title, "Search DuckDuckGo for rust slint");
    assert_eq!(
        results[0].web_search_url(),
        Some("https://duckduckgo.com/?q=rust%20slint")
    );
}

#[test]
fn mixed_search_default_browser_search_requires_search_command() {
    let apps = vec![app("rayslash.desktop", "Rayslash")];
    let providers = ProviderConfig {
        apps: true,
        folders: false,
        calculator: false,
        aliases: false,
        web_search: true,
        unit_conversion: false,
        currency_conversion: false,
        time_lookup: false,
        utility_actions: false,
    };
    let web_searches = vec![WebSearchConfig {
        name: "Web Search".to_owned(),
        keyword: "search".to_owned(),
        url: "https://www.google.com/search?q=%s".to_owned(),
        enabled: true,
    }];

    let results = search::mixed_results_with_ranking_and_web_searches(
        &[],
        &apps,
        &[],
        &web_searches,
        "manhattan",
        &providers,
        None,
    );

    assert_eq!(results[0].title, "No matches for manhattan");

    let results = search::mixed_results_with_ranking_and_web_searches(
        &[],
        &apps,
        &[],
        &web_searches,
        "search manhattan",
        &providers,
        None,
    );

    assert_eq!(results[0].title, "Search Web Search for manhattan");
    assert_eq!(
        results[0].web_search_url(),
        Some("https://www.google.com/search?q=manhattan")
    );
}

#[test]
fn mixed_search_web_search_rows_suppress_regular_matches() {
    let apps = vec![app("firefox.desktop", "Firefox")];
    let providers = ProviderConfig {
        apps: true,
        folders: false,
        calculator: true,
        aliases: false,
        web_search: true,
        unit_conversion: false,
        currency_conversion: false,
        time_lookup: false,
        utility_actions: false,
    };
    let web_searches = vec![WebSearchConfig {
        name: "Web Search".to_owned(),
        keyword: "search".to_owned(),
        url: "https://www.google.com/search?q=%s".to_owned(),
        enabled: true,
    }];

    let results = search::mixed_results_with_ranking_and_web_searches(
        &[],
        &apps,
        &[],
        &web_searches,
        "search firefox",
        &providers,
        None,
    );

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Search Web Search for firefox");
}

#[test]
fn mixed_search_supports_local_unit_conversion() {
    let providers = ProviderConfig {
        apps: false,
        folders: false,
        calculator: false,
        aliases: false,
        web_search: false,
        unit_conversion: true,
        currency_conversion: false,
        time_lookup: false,
        utility_actions: false,
    };

    let results = search::mixed_results_with_providers(&[], &[], "10 km to mi", &providers);

    assert_eq!(results[0].title, "6.2137 mi");
    assert_eq!(results[0].unit_conversion_result(), Some("6.2137 mi"));
}

#[test]
fn mixed_search_unit_conversions_suppress_calculator_errors() {
    let providers = ProviderConfig {
        apps: false,
        folders: false,
        calculator: true,
        aliases: false,
        web_search: false,
        unit_conversion: true,
        currency_conversion: false,
        time_lookup: false,
        utility_actions: false,
    };

    let compact = search::mixed_results_with_providers(&[], &[], "10c to k", &providers);
    let named =
        search::mixed_results_with_providers(&[], &[], "10 celsius to fahrenheit", &providers);
    let compact_length = search::mixed_results_with_providers(&[], &[], "10mi to km", &providers);
    let reverse = search::mixed_results_with_providers(&[], &[], "10f to celsius", &providers);

    assert_eq!(compact[0].title, "283.15 K");
    assert_eq!(compact.len(), 1);
    assert_eq!(named[0].title, "50 °F");
    assert_eq!(compact_length[0].title, "16.0934 km");
    assert_eq!(compact_length.len(), 1);
    assert_eq!(reverse[0].title, "-12.22 °C");
    assert_eq!(reverse.len(), 1);
}

#[test]
fn mixed_search_supports_power_and_timer_actions() {
    let reboot = search::mixed_results(&[], &[], "reboot in 10");
    let timer = search::mixed_results(&[], &[], "timer feed the cat 10min");
    let timer_error = search::mixed_results(&[], &[], "timer feed 2 cats 10min");

    assert_eq!(reboot[0].title, "Reboot in 10 seconds");
    let action = reboot[0].utility_action().expect("utility action");
    assert!(matches!(
        action,
        UtilityAction::System(action) if action.kind == SystemActionKind::Reboot
    ));

    let lock = search::mixed_results(&[], &[], "lock now");
    assert_eq!(lock[0].title, "Lock now");
    let action = lock[0].utility_action().expect("utility action");
    assert!(matches!(
        action,
        UtilityAction::System(action) if action.kind == SystemActionKind::Lock
    ));

    assert_eq!(timer[0].title, "Remind in 10 minutes: feed the cat");
    assert!(timer[0].utility_action().is_some());

    assert_eq!(
        timer_error[0].utility_action_error_message(),
        Some("More than one time found. Quote the message if needed.")
    );
}

#[test]
fn partial_system_actions_are_search_items_alongside_apps() {
    let apps = vec![app("shutdown-helper.desktop", "Shutdown Helper")];
    let results = search::mixed_results(&[], &apps, "shutdow");

    assert!(results.iter().any(|result| {
        matches!(
            result.utility_action(),
            Some(UtilityAction::System(action)) if action.kind == SystemActionKind::Shutdown
        )
    }));
    assert!(
        results
            .iter()
            .any(|result| result.title == "Shutdown Helper")
    );
}

#[test]
fn mixed_search_supports_currency_conversion_rows_without_network_for_same_currency() {
    let providers = ProviderConfig {
        apps: false,
        folders: false,
        calculator: false,
        aliases: false,
        web_search: false,
        unit_conversion: false,
        currency_conversion: true,
        time_lookup: false,
        utility_actions: false,
    };

    let results = search::mixed_results_with_providers(&[], &[], "10 usd to usd", &providers);

    assert_eq!(results[0].title, "10 USD");
    assert_eq!(results[0].currency_conversion_result(), Some("10 USD"));
}

#[test]
fn mixed_search_matches_app_keywords_and_localized_names() {
    let mut settings = app("settings.desktop", "Settings");
    settings.localized_names = vec!["Configuracoes".to_owned()];
    settings.keywords = vec!["preferences".to_owned(), "display".to_owned()];
    let apps = vec![settings];

    let by_keyword = search::mixed_results(&[], &apps, "display");
    let by_localized_name = search::mixed_results(&[], &apps, "config");

    assert_eq!(by_keyword[0].title, "Settings");
    assert_eq!(by_localized_name[0].title, "Settings");
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

    let no_results = search::mixed_results_with_providers(
        &[project("/tmp/rayslash", "rayslash")],
        &[],
        "zzz",
        &ProviderConfig {
            apps: true,
            folders: true,
            calculator: true,
            aliases: true,
            web_search: false,
            unit_conversion: true,
            currency_conversion: true,
            time_lookup: true,
            utility_actions: true,
        },
    );

    assert_eq!(no_results[0].title, "No matches for zzz");
    assert_eq!(no_results[0].subtitle, "No matches");
    assert!(no_results[0].is_no_results());
}

#[test]
fn mixed_search_no_results_wording_stays_short() {
    let providers = ProviderConfig {
        apps: false,
        folders: true,
        calculator: false,
        aliases: false,
        web_search: false,
        ..ProviderConfig::default()
    };
    let projects = vec![project("/tmp/rayslash", "rayslash")];

    let no_results = search::mixed_results_with_providers(&projects, &[], "zzz", &providers);

    assert_eq!(no_results[0].title, "No matches for zzz");
    assert_eq!(no_results[0].subtitle, "No matches");
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
        &[],
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
