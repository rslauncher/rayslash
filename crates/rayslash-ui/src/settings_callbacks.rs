use std::{cell::Cell, cell::RefCell, path::PathBuf, rc::Rc};

use rayslash_core::{apps, config, projects, ranking, search};
use slint::{ComponentHandle, VecModel};

use crate::{
    AppWindow, DEFAULT_STATUS_TEXT, ResultItem,
    opener_visual::{app_icon_count, set_alternate_opener_visual},
    result_items::{IconImageCache, to_result_items},
    runtime_state::{search_results, selected_index_for_query},
    settings::{
        first_existing_folder_source, parse_folder_sources_text, parse_max_results,
        set_settings_properties,
    },
};

pub(crate) struct SettingsCallbackContext {
    pub config_state: Rc<RefCell<config::Config>>,
    pub ranking_state: Rc<RefCell<ranking::RankingState>>,
    pub projects: Rc<RefCell<Vec<projects::Project>>>,
    pub apps: Rc<Vec<apps::DesktopApp>>,
    pub current_results: Rc<RefCell<Vec<search::SearchResult>>>,
    pub results_model: Rc<VecModel<ResultItem>>,
    pub icon_cache: Rc<RefCell<IconImageCache>>,
    pub socket_path: PathBuf,
    pub suppress_next_focus_hide: Rc<Cell<bool>>,
}

pub(crate) fn register_settings_callbacks(ui: &AppWindow, context: SettingsCallbackContext) {
    let SettingsCallbackContext {
        config_state,
        ranking_state,
        projects,
        apps,
        current_results,
        results_model,
        icon_cache,
        socket_path,
        suppress_next_focus_hide,
    } = context;

    ui.on_settings_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let ranking_state = ranking_state.clone();
        let socket_path = socket_path.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_settings_open() {
                    ui.set_settings_open(false);
                    ui.invoke_focus_search();
                    return;
                }

                set_settings_properties(
                    &ui,
                    &config_state.borrow(),
                    &socket_path,
                    projects.borrow().len(),
                    apps.len(),
                    app_icon_count(&apps),
                    ranking_state.borrow().entries.len(),
                );
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
                ui.set_settings_open(true);
                ui.invoke_focus_settings();
            }
        }
    });

    ui.on_settings_cancel_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let ranking_state = ranking_state.clone();
        let socket_path = socket_path.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                set_settings_properties(
                    &ui,
                    &config_state.borrow(),
                    &socket_path,
                    projects.borrow().len(),
                    apps.len(),
                    app_icon_count(&apps),
                    ranking_state.borrow().entries.len(),
                );
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
                ui.set_settings_open(false);
                ui.invoke_focus_search();
            }
        }
    });

    ui.on_settings_save_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move |folder_sources_text,
              editor_command,
              apps_enabled,
              folders_enabled,
              calculator_enabled,
              alternate_folder_opener_enabled,
              learn_from_usage,
              max_results_text| {
            let editor_command = editor_command.trim();
            if alternate_folder_opener_enabled && editor_command.is_empty() {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text("Alternate folder opener cannot be empty.".into());
                }
                return;
            }

            let Some(max_results) = parse_max_results(max_results_text.as_str()) else {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text("Max results must be a positive number.".into());
                }
                return;
            };

            let config_to_save = config::Config {
                folder_sources: parse_folder_sources_text(folder_sources_text.as_str()),
                providers: config::ProviderConfig {
                    apps: apps_enabled,
                    folders: folders_enabled,
                    calculator: calculator_enabled,
                },
                actions: config::ActionConfig {
                    alternate_folder_opener_enabled,
                    alternate_folder_opener_command: editor_command.to_owned(),
                },
                appearance: config::AppearanceConfig { max_results },
                ranking: config::RankingConfig { learn_from_usage },
            };

            if let Err(error) = config::save_config(&config_to_save) {
                eprintln!("{error}");
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text(format!("Could not save settings: {error}").into());
                }
                return;
            }

            let runtime_config = config_to_save.normalized();
            *config_state.borrow_mut() = runtime_config;
            let updated_projects =
                projects::scan_project_roots(&config_state.borrow().folder_sources);
            *projects.borrow_mut() = updated_projects;

            if let Some(ui) = weak.upgrade() {
                let query = ui.get_query_text();
                let results = search_results(
                    &config_state.borrow(),
                    &ranking_state.borrow(),
                    &projects.borrow(),
                    &apps,
                    query.as_str(),
                );
                let count = results.len() as i32;

                results_model.set_vec(to_result_items(&results, &mut icon_cache.borrow_mut()));
                *current_results.borrow_mut() = results;

                ui.set_result_count(count);
                ui.set_selected_index(selected_index_for_query(query.as_str(), count));
                ui.set_alternate_folder_opener_enabled(
                    config_state
                        .borrow()
                        .actions
                        .alternate_folder_opener_enabled,
                );
                set_alternate_opener_visual(
                    &ui,
                    &config_state
                        .borrow()
                        .actions
                        .alternate_folder_opener_command,
                    &apps,
                    &mut icon_cache.borrow_mut(),
                );
                set_settings_properties(
                    &ui,
                    &config_state.borrow(),
                    &socket_path,
                    projects.borrow().len(),
                    apps.len(),
                    app_icon_count(&apps),
                    ranking_state.borrow().entries.len(),
                );
                ui.set_status_text("Settings saved.".into());
                ui.invoke_reset_result_scroll();
            }
        }
    });

    ui.on_settings_choose_alternate_opener_requested({
        let weak = ui.as_weak();
        move |command| {
            if let Some(ui) = weak.upgrade() {
                ui.set_settings_alternate_folder_opener_command(command.clone());
                ui.set_status_text(format!("Selected alternate opener: {command}").into());
            }
        }
    });

    ui.on_settings_browse_folder_requested({
        let weak = ui.as_weak();
        move |current_sources| {
            suppress_next_focus_hide.set(true);
            let initial_dir = first_existing_folder_source(current_sources.as_str())
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("/"));
            let selected = rfd::FileDialog::new()
                .set_title("Choose folder source")
                .set_directory(initial_dir)
                .pick_folder();

            if let (Some(ui), Some(folder)) = (weak.upgrade(), selected) {
                ui.set_settings_folder_sources(search::display_path(&folder).into());
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
                ui.set_settings_open(true);
                ui.invoke_settings_save_requested(
                    ui.get_settings_folder_sources(),
                    ui.get_settings_alternate_folder_opener_command(),
                    ui.get_settings_provider_apps(),
                    ui.get_settings_provider_folders(),
                    ui.get_settings_provider_calculator(),
                    ui.get_settings_alternate_folder_opener_enabled(),
                    ui.get_settings_ranking_learn_from_usage(),
                    ui.get_settings_max_results(),
                );
            }
        }
    });

    ui.on_settings_clear_ranking_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move || {
            if let Err(error) = ranking::clear_ranking_state() {
                eprintln!("{error}");
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text(format!("Could not clear ranking history: {error}").into());
                }
                return;
            }

            *ranking_state.borrow_mut() = ranking::RankingState::default();

            if let Some(ui) = weak.upgrade() {
                let query = ui.get_query_text();
                let results = search_results(
                    &config_state.borrow(),
                    &ranking_state.borrow(),
                    &projects.borrow(),
                    &apps,
                    query.as_str(),
                );
                let count = results.len() as i32;

                results_model.set_vec(to_result_items(&results, &mut icon_cache.borrow_mut()));
                *current_results.borrow_mut() = results;

                ui.set_result_count(count);
                ui.set_selected_index(selected_index_for_query(query.as_str(), count));
                set_settings_properties(
                    &ui,
                    &config_state.borrow(),
                    &socket_path,
                    projects.borrow().len(),
                    apps.len(),
                    app_icon_count(&apps),
                    ranking_state.borrow().entries.len(),
                );
                ui.set_status_text("Ranking history cleared.".into());
                ui.invoke_reset_result_scroll();
            }
        }
    });
}
