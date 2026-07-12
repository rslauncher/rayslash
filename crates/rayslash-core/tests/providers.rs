mod fixtures;

use std::collections::BTreeSet;

use fixtures::app;
use rayslash_core::{
    config::ProviderConfig,
    providers::{
        Provider, ProviderAction, ProviderContext, ProviderId, builtin_provider_catalog,
        builtin_providers,
    },
};

fn provider(id: &ProviderId) -> &'static dyn Provider {
    *builtin_providers()
        .iter()
        .find(|provider| &provider.metadata().id == id)
        .expect("core provider")
}

#[test]
fn only_apps_and_folders_are_built_in() {
    let catalog = builtin_provider_catalog();
    let ids = catalog
        .iter()
        .map(|metadata| metadata.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        BTreeSet::from(["rayslash.core.apps", "rayslash.core.folders"])
    );
    assert!(catalog.iter().all(|metadata| metadata.module_id.is_none()));
}

#[test]
fn core_app_results_keep_typed_launch_actions() {
    let apps = [app("editor.desktop", "Editor")];
    let config = ProviderConfig::default();
    let context = ProviderContext {
        query: "editor",
        projects: &[],
        apps: &apps,
        aliases: &[],
        web_searches: &[],
        legacy_config: &config,
        ranking: None,
    };
    let result = provider(&ProviderId::CORE_APPS)
        .run(&context)
        .results
        .into_iter()
        .next()
        .expect("app result");
    assert_eq!(result.provider_id, ProviderId::CORE_APPS);
    assert!(matches!(result.action, ProviderAction::LaunchApp { .. }));
}
