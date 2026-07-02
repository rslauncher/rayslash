mod fixtures;

use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use fixtures::TempDir;
use rayslash_core::ranking::{self, RankingEntry, RankingState};

#[test]
fn missing_ranking_state_loads_default() {
    let dir = TempDir::new("rayslash-ranking-missing");
    let path = dir.join("ranking.toml");

    let state = ranking::load_ranking_state_from_path(&path).expect("missing state should load");

    assert_eq!(state, RankingState::default());
}

#[test]
fn ranking_state_can_be_saved_and_loaded() {
    let dir = TempDir::new("rayslash-ranking-save");
    let path = dir.join("ranking.toml");
    let mut state = RankingState::default();
    state.record_launch_at("app:org.example.Editor.desktop", "ed", unix_time(100));
    state.record_launch_at("app:org.example.Editor.desktop", "edi", unix_time(200));

    ranking::save_ranking_state_to_path(&path, &state).expect("save ranking state");
    let saved = fs::read_to_string(&path).expect("read saved ranking state");
    let loaded = ranking::load_ranking_state_from_path(&path).expect("load saved ranking state");

    assert!(saved.contains("version = 1"));
    assert!(saved.contains("launch_count = 2"));
    assert_eq!(loaded, state);
    assert_no_temp_save_files(path.parent().expect("ranking parent"));
}

#[test]
fn corrupted_ranking_state_falls_back_to_default() {
    let dir = TempDir::new("rayslash-ranking-corrupt");
    let path = dir
        .write("ranking.toml", "this is not valid toml =")
        .expect("write corrupted state");

    let state = ranking::load_ranking_state_from_path_or_default(&path);

    assert_eq!(state, RankingState::default());
}

#[test]
fn record_launch_tracks_count_time_and_query_prefixes() {
    let mut state = RankingState::default();

    state.record_launch_at("folder:/tmp/rayslash", "Ray", unix_time(123));

    let entry = state
        .entries
        .get("folder:/tmp/rayslash")
        .expect("entry should be recorded");
    assert_eq!(entry.launch_count, 1);
    assert_eq!(entry.last_launched_unix, 123);
    assert_eq!(entry.query_prefixes.get("ra"), Some(&1));
    assert_eq!(entry.query_prefixes.get("ray"), Some(&1));
    assert_eq!(entry.query_prefixes.get("r"), None);
}

#[test]
fn ranking_boost_is_bounded_and_query_sensitive() {
    let mut state = RankingState::default();
    for second in 1..=10 {
        state.record_launch_at("app:editor.desktop", "edi", unix_time(second));
    }

    assert_eq!(state.boost_for("app:editor.desktop", ""), 0);
    assert!(state.boost_for("app:editor.desktop", "ed") > 0);
    assert_eq!(state.boost_for("app:editor.desktop", "ed"), 20);
    assert!(state.boost_for("app:editor.desktop", "other") <= 8);
    assert_eq!(state.boost_for("missing", "ed"), 0);
}

#[test]
fn ranking_prune_removes_missing_and_old_entries() {
    let mut state = RankingState::default();
    let now = 200 + 181 * 24 * 60 * 60;
    state.record_launch_at("app:active.desktop", "ac", unix_time(now));
    state.record_launch_at("app:removed.desktop", "rm", unix_time(now));
    state.record_launch_at("folder:/tmp/old", "ol", unix_time(1));

    state.prune(
        vec![
            "app:active.desktop".to_owned(),
            "folder:/tmp/old".to_owned(),
        ],
        unix_time(now),
    );

    assert!(state.entries.contains_key("app:active.desktop"));
    assert!(!state.entries.contains_key("app:removed.desktop"));
    assert!(!state.entries.contains_key("folder:/tmp/old"));
}

#[test]
fn ranking_prune_caps_query_prefixes_by_count() {
    let mut state = RankingState::default();
    let mut entry = RankingEntry {
        launch_count: 1,
        last_launched_unix: 100,
        query_prefixes: Default::default(),
    };
    for index in 0..80 {
        entry
            .query_prefixes
            .insert(format!("prefix-{index:02}"), index);
    }
    state.entries.insert("app:active.desktop".to_owned(), entry);

    state.prune(vec!["app:active.desktop".to_owned()], unix_time(100));

    let prefixes = &state.entries["app:active.desktop"].query_prefixes;
    assert_eq!(prefixes.len(), 64);
    assert!(!prefixes.contains_key("prefix-00"));
    assert!(prefixes.contains_key("prefix-79"));
}

#[test]
fn ranking_prune_caps_total_entries_by_recency() {
    let mut state = RankingState::default();
    let active_ids = (0..1100)
        .map(|index| {
            let id = format!("app:{index:04}.desktop");
            state.record_launch_at(&id, "ap", unix_time(index));
            id
        })
        .collect::<Vec<_>>();

    state.prune(active_ids, unix_time(1100));

    assert_eq!(state.entries.len(), 1000);
    assert!(!state.entries.contains_key("app:0000.desktop"));
    assert!(state.entries.contains_key("app:1099.desktop"));
}

#[test]
fn clear_ranking_state_removes_existing_file_and_accepts_missing_file() {
    let dir = TempDir::new("rayslash-ranking-clear");
    let path = dir
        .write("ranking.toml", "version = 1")
        .expect("write ranking state");

    ranking::clear_ranking_state_at_path(&path).expect("clear existing state");
    assert!(!path.exists());
    ranking::clear_ranking_state_at_path(&path).expect("clear missing state");
}

fn unix_time(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn assert_no_temp_save_files(dir: &Path) {
    let temp_files = fs::read_dir(dir)
        .expect("read save directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".ranking.toml.") && name.ends_with(".tmp"))
        })
        .collect::<Vec<_>>();

    assert!(temp_files.is_empty());
}
