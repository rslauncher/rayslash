mod fixtures;

use fixtures::TempDir;
use rayslash_core::app_state;

#[test]
fn app_state_marks_only_apps_discovered_after_initial_baseline_as_new() {
    let mut state = app_state::AppInstallState::default();

    assert!(state.mark_discovered_app_ids(["editor.desktop".to_owned()]));
    assert!(!state.is_new_app("editor.desktop"));

    assert!(
        state.mark_discovered_app_ids(["editor.desktop".to_owned(), "browser.desktop".to_owned(),])
    );
    assert!(!state.is_new_app("editor.desktop"));
    assert!(state.is_new_app("browser.desktop"));

    assert!(state.mark_app_selected("browser.desktop"));
    assert!(!state.is_new_app("browser.desktop"));
}

#[test]
fn app_state_saves_and_loads_from_toml() {
    let dir = TempDir::new("rayslash-app-state-test");
    let path = dir.join("apps.toml");
    let mut state = app_state::AppInstallState::default();
    state.mark_discovered_app_ids(["editor.desktop".to_owned()]);
    state.mark_discovered_app_ids(["editor.desktop".to_owned(), "browser.desktop".to_owned()]);

    app_state::save_app_state_to_path(&path, &state).expect("save app state");
    let loaded = app_state::load_app_state_from_path(&path).expect("load app state");

    assert_eq!(loaded, state);
    assert!(loaded.is_new_app("browser.desktop"));
}
