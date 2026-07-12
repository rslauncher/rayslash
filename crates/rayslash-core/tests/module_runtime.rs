mod fixtures;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use fixtures::TempDir;
use rayslash_core::modules::{
    PackageKind, PackagePermissions, RegistryModule, RegistryVersion, ReviewStatus,
    install_registry_version, query_installed_modules,
};
use semver::{Version, VersionReq};

#[test]
#[ignore = "live release and locally installed rayslash-module-host probe"]
fn official_calculator_installs_and_runs_through_the_host() {
    let root = TempDir::new("rayslash-module-runtime");
    unsafe {
        std::env::set_var("XDG_DATA_HOME", root.join("data"));
        std::env::set_var("XDG_STATE_HOME", root.join("state"));
        std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
        std::env::set_var("XDG_CACHE_HOME", root.join("cache"));
    }
    let version = RegistryVersion {
        version: Version::new(1, 0, 2),
        api_version: VersionReq::parse("^1.0").unwrap(),
        source_commit: "77d1f7d1f8f1903b3f7b41fd17e03e375e64112b".into(),
        asset_url: "https://github.com/rslauncher/rayslash-module-calculator/releases/download/v1.0.2/rayslash.calculator-1.0.2.tar.zst".into(),
        sha256: "8fc6398816f89ab20c39b61768c912056c8030855243f7e145a1a664fe364b94".into(),
        size: 257746,
        yanked: false,
    };
    let module = RegistryModule {
        id: "rayslash.calculator".into(),
        name: "Calculator".into(),
        description: "Calculate expressions and linear equations.".into(),
        author: "rayslash".into(),
        license: "MIT".into(),
        kind: PackageKind::Wasm,
        permissions: PackagePermissions::default(),
        repository: "https://github.com/rslauncher/rayslash-module-calculator".into(),
        official: true,
        review_status: ReviewStatus::Reviewed,
        github_stars: 0,
        updated_at: "2026-07-12T05:30:00Z".into(),
        versions: vec![version.clone()],
    };
    install_registry_version(&module, &version).expect("verified install");
    let mut config = rayslash_core::modules::ModulesConfig::empty();
    config.set_installed(&module.id, "1.0.2", true);
    let results = query_installed_modules("2x+4=10", 10, &config, &BTreeMap::new());
    assert!(results.errors.is_empty(), "{:?}", results.errors);
    assert_eq!(results.results[0].title, "x = 3");

    let started = Instant::now();
    let second = query_installed_modules("6 * 7", 10, &config, &BTreeMap::new());
    assert_eq!(second.results[0].title, "42");
    assert!(started.elapsed() < Duration::from_millis(250));
}
