use std::{cell::RefCell, path::Path, rc::Rc, time::Instant};

use rayslash_core::{apps, config, projects, ranking, search};
use slint::VecModel;

use crate::{
    AppChoiceItem, AppWindow, ResultItem,
    opener_visual::{app_icon_count, set_alternate_opener_visual, to_app_choice_items},
    result_items::{IconImageCache, to_result_items},
    settings::set_settings_properties,
};

pub(crate) fn profile_enabled() -> bool {
    std::env::var_os("RAYSLASH_PROFILE").is_some_and(|value| value != "0")
}

pub(crate) fn profile_stage(enabled: bool, label: &str, started: Instant) {
    if enabled {
        eprintln!("[rayslash profile] {label}: {:.2?}", started.elapsed());
    }
}

pub(crate) fn load_runtime_ranking_state() -> ranking::RankingState {
    match ranking::load_ranking_state() {
        Ok(state) => state,
        Err(error) => {
            eprintln!("{error}; using empty ranking state");
            ranking::RankingState::default()
        }
    }
}

pub(crate) fn search_results(
    config: &config::Config,
    ranking_state: &ranking::RankingState,
    projects: &[projects::Project],
    apps: &[apps::DesktopApp],
    query: &str,
) -> Vec<search::SearchResult> {
    let ranking = config.ranking.learn_from_usage.then_some(ranking_state);
    let mut results = search::mixed_results_with_ranking(
        projects,
        apps,
        &config.aliases,
        query,
        &config.providers,
        ranking,
    );
    results.truncate(config.appearance.max_results);
    results
}

pub(crate) fn refresh_desktop_apps(
    apps_state: &Rc<RefCell<Vec<apps::DesktopApp>>>,
    choices_model: &Rc<VecModel<AppChoiceItem>>,
    icon_cache: &Rc<RefCell<IconImageCache>>,
    profile: bool,
    label: &str,
) {
    let stage_started = Instant::now();
    let discovered_apps = apps::discover_desktop_apps();
    let app_count = discovered_apps.len();
    let icon_count = app_icon_count(&discovered_apps);

    icon_cache.borrow_mut().clear();
    choices_model.set_vec(to_app_choice_items(
        &discovered_apps,
        &mut icon_cache.borrow_mut(),
    ));
    *apps_state.borrow_mut() = discovered_apps;

    profile_stage(
        profile,
        &format!("{label} app refresh ({app_count} apps, {icon_count} icons)"),
        stage_started,
    );
}

pub(crate) struct ResultRefreshContext<'a> {
    pub config: &'a config::Config,
    pub ranking_state: &'a ranking::RankingState,
    pub projects: &'a [projects::Project],
    pub apps: &'a [apps::DesktopApp],
    pub current_results: &'a Rc<RefCell<Vec<search::SearchResult>>>,
    pub results_model: &'a Rc<VecModel<ResultItem>>,
    pub icon_cache: &'a Rc<RefCell<IconImageCache>>,
}

pub(crate) enum ResultSelection {
    Exact(i32),
    QueryDefault,
}

pub(crate) fn refresh_result_view(
    ui: &AppWindow,
    context: ResultRefreshContext<'_>,
    query: &str,
    selection: ResultSelection,
) -> usize {
    let results = search_results(
        context.config,
        context.ranking_state,
        context.projects,
        context.apps,
        query,
    );
    let count = results.len();

    context.results_model.set_vec(to_result_items(
        &results,
        &mut context.icon_cache.borrow_mut(),
    ));
    *context.current_results.borrow_mut() = results;

    ui.set_result_count(count as i32);
    ui.set_selected_index(match selection {
        ResultSelection::Exact(index) => index,
        ResultSelection::QueryDefault => selected_index_for_query(query, count as i32),
    });
    ui.invoke_reset_result_scroll();

    count
}

pub(crate) fn refresh_settings_dependent_ui(
    ui: &AppWindow,
    config: &config::Config,
    projects: &[projects::Project],
    apps: &[apps::DesktopApp],
    ranking_state: &ranking::RankingState,
    icon_cache: &Rc<RefCell<IconImageCache>>,
    socket_path: &Path,
) {
    ui.set_alternate_folder_opener_enabled(config.actions.alternate_folder_opener_enabled);
    set_alternate_opener_visual(
        ui,
        &config.actions.alternate_folder_opener_command,
        apps,
        &mut icon_cache.borrow_mut(),
    );
    set_settings_properties(
        ui,
        config,
        socket_path,
        projects.len(),
        apps.len(),
        app_icon_count(apps),
        ranking_state.entries.len(),
    );
}

pub(crate) fn selected_index_for_query(query: &str, result_count: i32) -> i32 {
    if query.trim().is_empty() || result_count <= 0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn selected_index_requires_non_empty_query_with_results() {
        assert_eq!(selected_index_for_query("", 3), -1);
        assert_eq!(selected_index_for_query("   ", 3), -1);
        assert_eq!(selected_index_for_query("code", 0), -1);
        assert_eq!(selected_index_for_query("code", 3), 0);
    }

    #[test]
    fn search_results_respect_configured_max_results() {
        let config = config::Config {
            folder_sources: Vec::new(),
            aliases: Vec::new(),
            providers: config::ProviderConfig::default(),
            actions: config::ActionConfig::default(),
            appearance: config::AppearanceConfig {
                max_results: 1,
                ..config::AppearanceConfig::default()
            },
            ranking: config::RankingConfig::default(),
        };
        let ranking_state = ranking::RankingState::default();
        let projects = vec![
            projects::Project {
                name: "alpha".to_owned(),
                path: PathBuf::from("/tmp/alpha"),
            },
            projects::Project {
                name: "beta".to_owned(),
                path: PathBuf::from("/tmp/beta"),
            },
        ];

        let results = search_results(&config, &ranking_state, &projects, &[], "");

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_results_ignore_ranking_when_learning_is_disabled() {
        let config = config::Config {
            folder_sources: Vec::new(),
            aliases: Vec::new(),
            providers: config::ProviderConfig::default(),
            actions: config::ActionConfig::default(),
            appearance: config::AppearanceConfig::default(),
            ranking: config::RankingConfig {
                learn_from_usage: false,
            },
        };
        let mut ranking_state = ranking::RankingState::default();
        ranking_state.record_launch_at(
            "folder:/tmp/alpine",
            "al",
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1),
        );
        let projects = vec![
            projects::Project {
                name: "Alpha".to_owned(),
                path: PathBuf::from("/tmp/alpha"),
            },
            projects::Project {
                name: "Alpine".to_owned(),
                path: PathBuf::from("/tmp/alpine"),
            },
        ];

        let results = search_results(&config, &ranking_state, &projects, &[], "al");

        assert_eq!(results[0].title, "Alpha");
    }
}
