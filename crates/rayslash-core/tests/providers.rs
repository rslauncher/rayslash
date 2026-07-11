mod fixtures;

use std::collections::BTreeSet;

use fixtures::app;
use rayslash_core::{
    config::ProviderConfig as LegacyProviderConfig,
    providers::{
        Provider, ProviderAction, ProviderContext, ProviderExecutionHint, ProviderHealth,
        ProviderId, builtin_provider_catalog, builtin_providers, query_execution_hint,
    },
    search,
};

fn provider(id: &ProviderId) -> &'static dyn Provider {
    *builtin_providers()
        .iter()
        .find(|provider| &provider.metadata().id == id)
        .expect("built-in provider")
}

fn empty_context<'a>(query: &'a str, config: &'a LegacyProviderConfig) -> ProviderContext<'a> {
    ProviderContext {
        query,
        projects: &[],
        apps: &[],
        aliases: &[],
        web_searches: &[],
        legacy_config: config,
        ranking: None,
    }
}

#[test]
fn built_in_catalog_has_unique_stable_ids_and_official_module_mappings() {
    let catalog = builtin_provider_catalog();
    let ids = catalog
        .iter()
        .map(|metadata| metadata.id.as_str())
        .collect::<BTreeSet<_>>();

    assert_eq!(catalog.len(), 9);
    assert_eq!(ids.len(), catalog.len());
    assert_eq!(ProviderId::CORE_APPS.as_str(), "rayslash.core.apps");
    assert_eq!(ProviderId::CORE_FOLDERS.as_str(), "rayslash.core.folders");

    let official_ids = [
        "rayslash.calculator",
        "rayslash.units",
        "rayslash.currency",
        "rayslash.time",
        "rayslash.web-search",
        "rayslash.timers",
        "rayslash.aliases",
    ];
    for id in official_ids {
        let metadata = catalog
            .iter()
            .find(|metadata| metadata.id.as_str() == id)
            .expect("official provider metadata");
        assert_eq!(metadata.module_id.as_ref(), Some(&metadata.id));
    }

    assert_eq!(provider(&ProviderId::CORE_APPS).metadata().module_id, None);
    assert_eq!(
        provider(&ProviderId::CORE_FOLDERS).metadata().module_id,
        None
    );
}

#[test]
fn metadata_exposes_permissions_and_config_drives_diagnostics() {
    let config = LegacyProviderConfig {
        currency_conversion: false,
        ..LegacyProviderConfig::default()
    };
    let context = empty_context("10 USD to EUR", &config);
    let currency = provider(&ProviderId::CURRENCY);

    assert!(currency.metadata().permissions.network);
    assert!(currency.metadata().permissions.clipboard);
    assert!(!currency.config(&context).enabled);
    assert_eq!(
        currency.diagnostics(&context).health,
        ProviderHealth::Disabled
    );

    let calculator = provider(&ProviderId::CALCULATOR);
    assert!(!calculator.metadata().permissions.network);
    assert_eq!(
        calculator.diagnostics(&context).health,
        ProviderHealth::Ready
    );
}

#[test]
fn provider_results_own_typed_actions_and_search_results_report_their_provider() {
    let config = LegacyProviderConfig::default();
    let context = empty_context("2 + 2", &config);
    let output = provider(&ProviderId::CALCULATOR).run(&context);
    let provider_result = output.results.first().expect("calculator provider result");

    assert_eq!(provider_result.provider_id, ProviderId::CALCULATOR);
    assert_eq!(
        provider_result.action,
        ProviderAction::CopyText("4".to_owned())
    );
    assert_eq!(provider_result.result.provider_id(), ProviderId::CALCULATOR);
    assert_eq!(
        provider_result.result.provider_action(),
        ProviderAction::CopyText("4".to_owned())
    );

    let apps = [app("editor.desktop", "Editor")];
    let app_context = ProviderContext {
        query: "editor",
        projects: &[],
        apps: &apps,
        aliases: &[],
        web_searches: &[],
        legacy_config: &config,
        ranking: None,
    };
    let app_result = provider(&ProviderId::CORE_APPS)
        .run(&app_context)
        .results
        .into_iter()
        .next()
        .expect("app provider result");
    assert_eq!(app_result.provider_id, ProviderId::CORE_APPS);
    assert!(matches!(
        app_result.action,
        ProviderAction::LaunchApp { .. }
    ));
}

#[test]
fn execution_hints_follow_enabled_network_providers_without_running_them() {
    let config = LegacyProviderConfig::default();
    assert_eq!(
        query_execution_hint(&empty_context("10 USD to EUR", &config)),
        ProviderExecutionHint::DebouncedNetwork { debounce_ms: 450 }
    );
    assert_eq!(
        search::query_execution_hint("time in Lisbon", &config),
        ProviderExecutionHint::DebouncedNetwork { debounce_ms: 450 }
    );
    assert_eq!(
        search::query_execution_hint("10 USD to USD", &config),
        ProviderExecutionHint::Local
    );

    let mut disabled = config;
    disabled.currency_conversion = false;
    disabled.time_lookup = false;
    assert_eq!(
        search::query_execution_hint("10 USD to EUR", &disabled),
        ProviderExecutionHint::Local
    );
    assert_eq!(
        search::query_execution_hint("time in Lisbon", &disabled),
        ProviderExecutionHint::Local
    );
}

#[test]
fn disabling_timers_removes_exact_and_fuzzy_utility_results() {
    let apps = [app("shutdown-helper.desktop", "Shutdown Helper")];
    let config = LegacyProviderConfig {
        utility_actions: false,
        ..LegacyProviderConfig::default()
    };

    let exact = search::mixed_results_with_providers(&[], &apps, "reboot now", &config);
    let fuzzy = search::mixed_results_with_providers(&[], &apps, "shutdow", &config);

    assert!(exact.iter().all(|result| result.utility_action().is_none()));
    assert!(fuzzy.iter().all(|result| result.utility_action().is_none()));
    assert_eq!(fuzzy[0].title, "Shutdown Helper");
}

#[test]
fn utility_only_configuration_has_an_honest_empty_query_placeholder() {
    let config = LegacyProviderConfig {
        apps: false,
        folders: false,
        calculator: false,
        aliases: false,
        web_search: false,
        unit_conversion: false,
        currency_conversion: false,
        time_lookup: false,
        utility_actions: true,
    };

    let results = search::mixed_results_with_providers(&[], &[], "", &config);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Use timers and system actions");
}
