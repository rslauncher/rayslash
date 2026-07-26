use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

use rayslash_core::{
    app_state, apps, config, modules, projects, providers::ProviderExecutionHint, ranking, search,
    web_search,
};
use slint::{Model, VecModel};

use crate::{
    AppChoiceItem, AppWindow, ResultItem,
    opener_visual::{app_icon_count, set_alternate_opener_visual, to_app_choice_items},
    persistence,
    result_items::{IconImageCache, to_result_items},
    settings::set_settings_properties,
};

pub(crate) struct SearchResultSet {
    pub results: Vec<search::SearchResult>,
    pub result_tip: String,
}

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

pub(crate) fn load_runtime_app_state() -> app_state::AppInstallState {
    match app_state::load_app_state() {
        Ok(state) => state,
        Err(error) => {
            eprintln!("{error}; using empty app state");
            app_state::AppInstallState::default()
        }
    }
}

pub(crate) fn search_result_set(
    config: &config::Config,
    ranking_state: &ranking::RankingState,
    app_state: &app_state::AppInstallState,
    projects: &[projects::Project],
    apps: &[apps::DesktopApp],
    query: &str,
) -> SearchResultSet {
    let local = local_search_result_set(config, ranking_state, app_state, projects, apps, query);
    merge_module_results(config, local, query)
}

pub(crate) fn local_search_result_set(
    config: &config::Config,
    ranking_state: &ranking::RankingState,
    app_state: &app_state::AppInstallState,
    projects: &[projects::Project],
    apps: &[apps::DesktopApp],
    query: &str,
) -> SearchResultSet {
    let ranking = config.ranking.learn_from_usage.then_some(ranking_state);
    let mut core_providers = config.providers.clone();
    core_providers.calculator = false;
    core_providers.aliases = false;
    core_providers.web_search = false;
    core_providers.unit_conversion = false;
    core_providers.currency_conversion = false;
    core_providers.time_lookup = false;
    core_providers.utility_actions = false;
    let results = search::mixed_results_with_ranking_and_web_searches(
        projects,
        apps,
        &config.aliases,
        &config.web_searches,
        query,
        &core_providers,
        ranking,
    );
    finalize_results(config, app_state, results)
}

pub(crate) fn merge_module_results(
    config: &config::Config,
    local: SearchResultSet,
    query: &str,
) -> SearchResultSet {
    let module_config = modules::load_modules_config(&config.providers)
        .unwrap_or_else(|_| modules::ModulesConfig::empty());
    merge_module_results_with_config(config, &module_config, local, query)
}

pub(crate) fn merge_module_results_with_config(
    config: &config::Config,
    module_config: &modules::ModulesConfig,
    mut local: SearchResultSet,
    query: &str,
) -> SearchResultSet {
    if !should_query_modules(query) {
        return local;
    }
    let settings = module_settings(config);
    let mut module_results = modules::query_installed_modules(
        query,
        config.appearance.max_results,
        module_config,
        &settings,
    );
    apply_web_search_favicons(&mut module_results.results, &config.web_searches);
    let mut results = std::mem::take(&mut local.results);
    if module_results.exclusive {
        results = module_results.results;
    } else if !module_results.results.is_empty() {
        results = prepend_module_results(module_results.results, results);
    } else if !query.trim().is_empty()
        && let Some(error) = module_results.errors.first()
    {
        results.retain(|result| !result.is_no_results());
        results.push(search::SearchResult {
            title: "Module runtime unavailable".into(),
            flair: "Module".into(),
            subtitle: error.clone(),
            icon: search::SearchResultIcon::Module {
                label: "!".into(),
                path: None,
            },
            kind: search::SearchResultKind::Module {
                module_id: "rayslash.module-runtime".into(),
                result_id: "runtime-error".into(),
                action: search::ModuleAction::ShowMessage(error.clone()),
                score: None,
            },
        });
    }
    let total_results = results.len();
    let max_results = config.appearance.max_results;
    if total_results > max_results {
        results.truncate(max_results);
    }
    SearchResultSet {
        results,
        result_tip: if total_results > max_results {
            format!("Max results: {max_results}")
        } else {
            String::new()
        },
    }
}

fn prepend_module_results(
    module_results: Vec<search::SearchResult>,
    mut local_results: Vec<search::SearchResult>,
) -> Vec<search::SearchResult> {
    local_results.retain(|result| !result.is_no_results());
    let mut leading_modules = Vec::new();
    for module_result in module_results {
        if let Some(index) = local_results
            .iter()
            .rposition(|local| local.title.eq_ignore_ascii_case(&module_result.title))
        {
            local_results.insert(index + 1, module_result);
        } else {
            leading_modules.push(module_result);
        }
    }
    leading_modules.extend(local_results);
    leading_modules
}

fn finalize_results(
    config: &config::Config,
    app_state: &app_state::AppInstallState,
    mut results: Vec<search::SearchResult>,
) -> SearchResultSet {
    apply_new_app_flairs(&mut results, app_state);
    let total_results = results.len();
    let max_results = config.appearance.max_results;
    let result_tip = if total_results > max_results {
        results.truncate(max_results);
        format!("Max results: {max_results}")
    } else {
        String::new()
    };

    SearchResultSet {
        results,
        result_tip,
    }
}

fn should_query_modules(query: &str) -> bool {
    !query.trim().is_empty()
}

fn apply_web_search_favicons(
    results: &mut [search::SearchResult],
    searches: &[config::WebSearchConfig],
) {
    for result in results {
        let Some(keyword) = web_search_result_keyword(result) else {
            continue;
        };
        let Some(path) = searches
            .iter()
            .find(|search| search.keyword.eq_ignore_ascii_case(keyword))
            .and_then(web_search::cached_favicon_path)
        else {
            continue;
        };
        result.icon = search::SearchResultIcon::Module {
            label: String::new(),
            path: Some(path),
        };
    }
}

fn web_search_result_keyword(result: &search::SearchResult) -> Option<&str> {
    let search::SearchResultKind::Module {
        module_id,
        result_id,
        ..
    } = &result.kind
    else {
        return None;
    };
    if module_id != modules::WEB_SEARCH_MODULE_ID {
        return None;
    }
    result_id
        .strip_prefix("web-search:")?
        .split_once(':')
        .map(|(keyword, _)| keyword)
}

pub(crate) fn query_execution_hint_with_config(
    config: &config::Config,
    module_config: &modules::ModulesConfig,
    query: &str,
) -> ProviderExecutionHint {
    modules::installed_module_execution_hint(query, module_config, &module_settings(config))
}

pub(crate) fn module_settings(config: &config::Config) -> BTreeMap<String, String> {
    let mut settings = BTreeMap::new();
    settings.insert(
        modules::WEB_SEARCH_MODULE_ID.to_owned(),
        serde_json::json!({ "searches": config.web_searches }).to_string(),
    );
    settings.insert(
        modules::ALIASES_MODULE_ID.to_owned(),
        serde_json::json!({ "aliases": config.aliases }).to_string(),
    );
    settings
}

pub(crate) fn refresh_desktop_apps(
    apps_state: &Rc<RefCell<Vec<apps::DesktopApp>>>,
    app_install_state: &Rc<RefCell<app_state::AppInstallState>>,
    choices_model: &Rc<VecModel<AppChoiceItem>>,
    icon_cache: &Rc<RefCell<IconImageCache>>,
    profile: bool,
    label: &str,
) {
    let stage_started = Instant::now();
    let discovered_apps = apps::discover_and_cache_desktop_apps();
    apply_desktop_apps(
        apps_state,
        app_install_state,
        choices_model,
        icon_cache,
        discovered_apps,
        (profile, label, stage_started),
    );
}

pub(crate) fn apply_desktop_apps(
    apps_state: &Rc<RefCell<Vec<apps::DesktopApp>>>,
    app_install_state: &Rc<RefCell<app_state::AppInstallState>>,
    choices_model: &Rc<VecModel<AppChoiceItem>>,
    icon_cache: &Rc<RefCell<IconImageCache>>,
    discovered_apps: Vec<apps::DesktopApp>,
    profile_info: (bool, &str, Instant),
) {
    let (profile, label, stage_started) = profile_info;
    let app_count = discovered_apps.len();
    let icon_count = app_icon_count(&discovered_apps);
    sync_app_install_state(app_install_state, &discovered_apps);

    icon_cache.borrow_mut().clear();
    if choices_model.row_count() > 0 {
        choices_model.set_vec(to_app_choice_items(
            &discovered_apps,
            &mut icon_cache.borrow_mut(),
        ));
    }
    *apps_state.borrow_mut() = discovered_apps;

    profile_stage(
        profile,
        &format!("{label} app refresh ({app_count} apps, {icon_count} icons)"),
        stage_started,
    );
}

pub(crate) struct DesktopAppRefreshContext<'a> {
    pub apps_state: &'a Rc<RefCell<Vec<apps::DesktopApp>>>,
    pub app_install_state: &'a Rc<RefCell<app_state::AppInstallState>>,
    pub choices_model: &'a Rc<VecModel<AppChoiceItem>>,
    pub icon_cache: &'a Rc<RefCell<IconImageCache>>,
    pub last_refresh: &'a Rc<RefCell<Instant>>,
    pub profile: bool,
    pub label: &'a str,
}

pub(crate) fn refresh_desktop_apps_if_stale(
    context: DesktopAppRefreshContext<'_>,
    min_interval: Duration,
) {
    if context.last_refresh.borrow().elapsed() < min_interval {
        return;
    }

    refresh_desktop_apps(
        context.apps_state,
        context.app_install_state,
        context.choices_model,
        context.icon_cache,
        context.profile,
        context.label,
    );
    *context.last_refresh.borrow_mut() = Instant::now();
}

pub(crate) struct ResultRefreshContext<'a> {
    pub config: &'a config::Config,
    pub ranking_state: &'a ranking::RankingState,
    pub app_state: &'a app_state::AppInstallState,
    pub projects: &'a [projects::Project],
    pub apps: &'a [apps::DesktopApp],
    pub current_results: &'a Rc<RefCell<Vec<search::SearchResult>>>,
    pub results_model: &'a Rc<VecModel<ResultItem>>,
    pub icon_cache: &'a Rc<RefCell<IconImageCache>>,
    pub profile: bool,
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
    let refresh_started = Instant::now();
    let search_started = Instant::now();
    let result_set = local_search_result_set(
        context.config,
        context.ranking_state,
        context.app_state,
        context.projects,
        context.apps,
        query,
    );
    profile_stage(context.profile, "result refresh search", search_started);

    let results = result_set.results;
    let count = results.len();

    let item_started = Instant::now();
    let result_items = to_result_items(&results, &mut context.icon_cache.borrow_mut());
    profile_stage(
        context.profile,
        "result refresh item conversion",
        item_started,
    );

    let model_started = Instant::now();
    context.results_model.set_vec(result_items);
    profile_stage(context.profile, "result refresh model set", model_started);

    *context.current_results.borrow_mut() = results;

    let ui_started = Instant::now();
    ui.set_result_count(count as i32);
    ui.set_result_tip_text(result_set.result_tip.into());
    ui.set_selected_index(match selection {
        ResultSelection::Exact(index) => index,
        ResultSelection::QueryDefault => selected_index_for_query(query, count as i32),
    });
    ui.invoke_reset_result_scroll();
    profile_stage(context.profile, "result refresh ui properties", ui_started);
    profile_stage(context.profile, "result refresh total", refresh_started);

    count
}

pub(crate) fn effective_search_query(query: &str, active_search_keyword: &str) -> String {
    let query = query.trim();
    let keyword = active_search_keyword.trim();

    if keyword.is_empty() {
        query.to_owned()
    } else if query.is_empty() {
        keyword.to_owned()
    } else {
        format!("{keyword} {query}")
    }
}

pub(crate) fn should_hide_transient_no_results(
    active_search_keyword: &str,
    results: &[search::SearchResult],
) -> bool {
    !active_search_keyword.trim().is_empty()
        && matches!(results, [result] if result.is_no_results())
}

pub(crate) fn sync_app_install_state(
    app_install_state: &Rc<RefCell<app_state::AppInstallState>>,
    apps: &[apps::DesktopApp],
) {
    let changed = app_install_state
        .borrow_mut()
        .mark_discovered_app_ids(apps.iter().map(|app| app.id.clone()));

    if changed {
        persistence::save_app_state(app_install_state.borrow().clone());
    }
}

fn apply_new_app_flairs(
    results: &mut [search::SearchResult],
    app_state: &app_state::AppInstallState,
) {
    for result in results {
        if result
            .app_id()
            .is_some_and(|app_id| app_state.is_new_app(app_id))
        {
            result.flair = "New".to_owned();
        }
    }
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
    let installed_modules = modules::load_installed_modules().unwrap_or_default();
    ui.set_settings_aliases_installed(
        installed_modules
            .modules
            .contains_key(modules::ALIASES_MODULE_ID),
    );
    ui.set_settings_web_search_installed(
        installed_modules
            .modules
            .contains_key(modules::WEB_SEARCH_MODULE_ID),
    );
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
    fn effective_search_query_prepends_active_search_keyword() {
        assert_eq!(effective_search_query("rust slint", ""), "rust slint");
        assert_eq!(effective_search_query("", "yt"), "yt");
        assert_eq!(effective_search_query("rust slint", "yt"), "yt rust slint");
    }

    #[test]
    fn active_web_search_hides_only_the_pending_no_results_placeholder() {
        let no_results = search::SearchResult {
            title: "No results to show".into(),
            flair: String::new(),
            subtitle: String::new(),
            icon: search::SearchResultIcon::Placeholder,
            kind: search::SearchResultKind::NoResults {
                query: "youtube rust".into(),
            },
        };

        assert!(should_hide_transient_no_results(
            "youtube",
            std::slice::from_ref(&no_results)
        ));
        assert!(!should_hide_transient_no_results(
            "",
            std::slice::from_ref(&no_results)
        ));
        assert!(!should_hide_transient_no_results(
            "youtube",
            &[no_results.clone(), no_results]
        ));
    }

    #[test]
    fn routed_module_actions_precede_unrelated_local_matches() {
        let module = search::SearchResult {
            title: "Reboot".into(),
            flair: String::new(),
            subtitle: "Reboot now".into(),
            icon: search::SearchResultIcon::Placeholder,
            kind: search::SearchResultKind::Module {
                module_id: modules::TIMERS_MODULE_ID.into(),
                result_id: "timers:reboot".into(),
                action: search::ModuleAction::None,
                score: None,
            },
        };
        let local = search::SearchResult {
            title: "Discord".into(),
            flair: String::new(),
            subtitle: "Application".into(),
            icon: search::SearchResultIcon::Placeholder,
            kind: search::SearchResultKind::Placeholder,
        };

        let results = prepend_module_results(vec![module], vec![local]);
        assert_eq!(results[0].title, "Reboot");
        assert_eq!(results[1].title, "Discord");
    }

    #[test]
    fn same_named_local_app_can_precede_a_module_action() {
        let action = search::SearchResult {
            title: "Lock".into(),
            flair: String::new(),
            subtitle: "Lock now".into(),
            icon: search::SearchResultIcon::Placeholder,
            kind: search::SearchResultKind::Module {
                module_id: modules::TIMERS_MODULE_ID.into(),
                result_id: "timers:lock".into(),
                action: search::ModuleAction::None,
                score: None,
            },
        };
        let app = search::SearchResult {
            title: "Lock".into(),
            flair: String::new(),
            subtitle: "Application".into(),
            icon: search::SearchResultIcon::Placeholder,
            kind: search::SearchResultKind::Placeholder,
        };

        let results = prepend_module_results(vec![action], vec![app]);
        assert!(matches!(
            results[0].kind,
            search::SearchResultKind::Placeholder
        ));
        assert!(matches!(
            results[1].kind,
            search::SearchResultKind::Module { .. }
        ));
    }

    #[test]
    fn empty_queries_do_not_start_module_hosts() {
        assert!(!should_query_modules(""));
        assert!(!should_query_modules("   "));
        assert!(should_query_modules("youtube rust"));
    }

    #[test]
    fn web_search_module_result_ids_expose_the_engine_keyword() {
        let result = search::SearchResult {
            title: "Search YouTube for rust".into(),
            flair: String::new(),
            subtitle: "https://www.youtube.com/results?search_query=rust".into(),
            icon: search::SearchResultIcon::Module {
                label: "youtube".into(),
                path: None,
            },
            kind: search::SearchResultKind::Module {
                module_id: modules::WEB_SEARCH_MODULE_ID.into(),
                result_id: "web-search:youtube:rust".into(),
                action: search::ModuleAction::None,
                score: None,
            },
        };

        assert_eq!(web_search_result_keyword(&result), Some("youtube"));
    }

    #[test]
    fn search_results_respect_configured_max_results_with_scroll_end_tip() {
        let config = config::Config {
            folder_sources: Vec::new(),
            aliases: Vec::new(),
            web_searches: Vec::new(),
            providers: config::ProviderConfig::default(),
            actions: config::ActionConfig::default(),
            appearance: config::AppearanceConfig {
                max_results: 1,
                ..config::AppearanceConfig::default()
            },
            ranking: config::RankingConfig::default(),
        };
        let ranking_state = ranking::RankingState::default();
        let app_state = app_state::AppInstallState::default();
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

        let result_set = search_result_set(&config, &ranking_state, &app_state, &projects, &[], "");

        assert_eq!(result_set.results.len(), 1);
        assert_eq!(result_set.results[0].title, "alpha");
        assert_eq!(result_set.result_tip, "Max results: 1");
    }

    #[test]
    fn search_results_ignore_ranking_when_learning_is_disabled() {
        let config = config::Config {
            folder_sources: Vec::new(),
            aliases: Vec::new(),
            web_searches: Vec::new(),
            providers: config::ProviderConfig::default(),
            actions: config::ActionConfig::default(),
            appearance: config::AppearanceConfig::default(),
            ranking: config::RankingConfig {
                learn_from_usage: false,
            },
        };
        let app_state = app_state::AppInstallState::default();
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

        let results =
            search_result_set(&config, &ranking_state, &app_state, &projects, &[], "al").results;

        assert_eq!(results[0].title, "Alpha");
    }
}
