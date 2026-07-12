use rayslash_core::modules;

#[test]
#[ignore = "live registry probe; the signature fixture covers offline correctness"]
fn live_registry_refresh_verifies_and_caches() {
    let refresh = modules::refresh_registry().expect("refresh signed registry");
    assert_eq!(refresh.root.key_id, "registry-2026-01");
    assert!(!refresh.from_cache);

    let cached = modules::load_cached_registry().expect("load verified cache");
    assert_eq!(cached.root, refresh.root);
    assert_eq!(cached.index, refresh.index);
    assert!(cached.from_cache);
}
