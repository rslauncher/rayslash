mod fixtures;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use fixtures::TempDir;
use rayslash_core::modules::{
    ModulesConfig, RegistryModule, RegistryVersion, install_registry_version,
    load_installed_modules, query_installed_modules, refresh_registry, remove_installed_module,
};
use semver::Version;

const MODULE_API_V1: Version = Version::new(1, 0, 0);

fn latest_compatible_version(module: &RegistryModule) -> &RegistryVersion {
    module
        .versions
        .iter()
        .filter(|version| !version.yanked && version.api_version.matches(&MODULE_API_V1))
        .max_by(|left, right| left.version.cmp(&right.version))
        .expect("official module has a compatible, non-yanked API v1 release")
}

#[test]
#[ignore = "live signed registry, release assets, services, and module-host probe"]
fn every_official_module_installs_and_runs_through_the_production_path() {
    let root = TempDir::new("rayslash-module-runtime");
    unsafe {
        std::env::set_var("XDG_DATA_HOME", root.join("data"));
        std::env::set_var("XDG_STATE_HOME", root.join("state"));
        std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
        std::env::set_var("XDG_CACHE_HOME", root.join("cache"));
    }

    let registry = refresh_registry().expect("refresh and verify production registry");
    let official = registry
        .index
        .modules
        .iter()
        .filter(|module| module.official)
        .collect::<Vec<_>>();
    assert_eq!(
        official.len(),
        7,
        "production registry official module count"
    );

    let mut config = ModulesConfig::empty();
    for module in &official {
        let version = latest_compatible_version(module);
        let installed = install_registry_version(module, version)
            .unwrap_or_else(|error| panic!("install {}: {error}", module.id));
        assert_eq!(installed.version, version.version);
        config.set_installed(&module.id, &installed.version.to_string(), false);
    }

    let mut settings = BTreeMap::new();
    settings.insert(
        "rayslash.aliases".into(),
        serde_json::json!({
            "aliases": [{
                "name": "Documentation",
                "query": "docs",
                "target": "https://example.com",
                "kind": "url"
            }]
        })
        .to_string(),
    );

    let cases = [
        ("rayslash.calculator", "2x+4=10", "x = 3"),
        ("rayslash.units", "10 km to mi", "6.21371192 mi"),
        ("rayslash.currency", "25 usd to usd", "25 USD"),
        (
            "rayslash.time",
            "time in São Paulo",
            " in São Paulo, Brazil",
        ),
        ("rayslash.web-search", "search rust", "Search Web for rust"),
        ("rayslash.timers", "timer 10min take a break", "Timer"),
        ("rayslash.aliases", "docs", "Documentation"),
    ];
    for (module_id, query, expected_title) in cases {
        config
            .set_installed_enabled(module_id, true)
            .expect("installed config entry");
        let batch = query_installed_modules(query, 10, &config, &settings);
        assert!(batch.errors.is_empty(), "{module_id}: {:?}", batch.errors);
        let result = batch
            .results
            .first()
            .unwrap_or_else(|| panic!("{module_id} returned no result for {query:?}"));
        if module_id == "rayslash.time" {
            assert!(
                result.title.contains(expected_title),
                "unexpected {module_id} title: {}",
                result.title
            );
        } else {
            assert_eq!(result.title, expected_title, "{module_id}");
        }
        config
            .set_installed_enabled(module_id, false)
            .expect("installed config entry");
    }

    config
        .set_installed_enabled("rayslash.calculator", true)
        .expect("calculator installed config entry");
    let started = Instant::now();
    let second = query_installed_modules("6 * 7", 10, &config, &settings);
    assert!(second.errors.is_empty(), "{:?}", second.errors);
    assert_eq!(second.results[0].title, "42");
    assert!(started.elapsed() < Duration::from_millis(250));

    for module in official {
        assert!(
            remove_installed_module(&module.id, true)
                .unwrap_or_else(|error| panic!("remove {}: {error}", module.id)),
            "{} was installed",
            module.id
        );
    }
    assert!(load_installed_modules().unwrap().modules.is_empty());
}
