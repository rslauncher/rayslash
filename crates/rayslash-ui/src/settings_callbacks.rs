use std::{cell::Cell, cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

use rayslash_core::{app_state, apps, config, projects, ranking, search};
use slint::{ComponentHandle, Timer, VecModel};

use crate::{
    AppChoiceItem, AppWindow, DEFAULT_STATUS_TEXT, DESKTOP_APP_REFRESH_INTERVAL, ResultItem,
    result_items::IconImageCache,
    runtime_state::{
        DesktopAppRefreshContext, ResultRefreshContext, ResultSelection, effective_search_query,
        refresh_desktop_apps_if_stale, refresh_result_view, refresh_settings_dependent_ui,
    },
    settings::{
        SettingsConfigError, config_from_settings_fields, first_existing_folder_source,
        parse_alias_kind, web_search_items,
    },
};

pub(crate) struct SettingsCallbackContext {
    pub config_state: Rc<RefCell<config::Config>>,
    pub app_install_state: Rc<RefCell<app_state::AppInstallState>>,
    pub ranking_state: Rc<RefCell<ranking::RankingState>>,
    pub projects: Rc<RefCell<Vec<projects::Project>>>,
    pub apps: Rc<RefCell<Vec<apps::DesktopApp>>>,
    pub alternate_opener_choices: Rc<VecModel<AppChoiceItem>>,
    pub current_results: Rc<RefCell<Vec<search::SearchResult>>>,
    pub results_model: Rc<VecModel<ResultItem>>,
    pub icon_cache: Rc<RefCell<IconImageCache>>,
    pub socket_path: PathBuf,
    pub suppress_next_focus_hide: Rc<Cell<bool>>,
    pub last_desktop_app_refresh: Rc<RefCell<std::time::Instant>>,
    pub settings_save_blocked: bool,
    pub profile: bool,
}

pub(crate) fn register_settings_callbacks(ui: &AppWindow, context: SettingsCallbackContext) {
    let SettingsCallbackContext {
        config_state,
        app_install_state,
        ranking_state,
        projects,
        apps,
        alternate_opener_choices,
        current_results,
        results_model,
        icon_cache,
        socket_path,
        suppress_next_focus_hide,
        last_desktop_app_refresh,
        settings_save_blocked,
        profile,
    } = context;

    ui.on_settings_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let app_install_state = app_install_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let alternate_opener_choices = alternate_opener_choices.clone();
        let icon_cache = icon_cache.clone();
        let last_desktop_app_refresh = last_desktop_app_refresh.clone();
        let ranking_state = ranking_state.clone();
        let socket_path = socket_path.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_settings_open() {
                    ui.set_settings_open(false);
                    ui.invoke_focus_search();
                    return;
                }

                refresh_desktop_apps_if_stale(
                    DesktopAppRefreshContext {
                        apps_state: &apps,
                        app_install_state: &app_install_state,
                        choices_model: &alternate_opener_choices,
                        icon_cache: &icon_cache,
                        last_refresh: &last_desktop_app_refresh,
                        profile,
                        label: "settings-open",
                    },
                    DESKTOP_APP_REFRESH_INTERVAL,
                );
                refresh_settings_dependent_ui(
                    &ui,
                    &config_state.borrow(),
                    &projects.borrow(),
                    &apps.borrow(),
                    &ranking_state.borrow(),
                    &icon_cache,
                    &socket_path,
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
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                refresh_settings_dependent_ui(
                    &ui,
                    &config_state.borrow(),
                    &projects.borrow(),
                    &apps.borrow(),
                    &ranking_state.borrow(),
                    &icon_cache,
                    &socket_path,
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
        let app_install_state = app_install_state.clone();
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
              aliases_enabled,
              web_search_enabled,
              unit_conversion_enabled,
              currency_conversion_enabled,
              time_lookup_enabled,
              utility_actions_enabled,
              alternate_folder_opener_enabled,
              learn_from_usage,
              theme,
              density,
              max_results_text,
              show_tooltips,
              aliases_text,
              web_searches_text| {
            if settings_save_blocked {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text(
                        "Could not save settings: fix config.toml and restart rayslash.".into(),
                    );
                }
                return;
            }

            let config_to_save = match config_from_settings_fields(
                folder_sources_text.as_str(),
                editor_command.as_str(),
                apps_enabled,
                folders_enabled,
                calculator_enabled,
                aliases_enabled,
                web_search_enabled,
                unit_conversion_enabled,
                currency_conversion_enabled,
                time_lookup_enabled,
                utility_actions_enabled,
                alternate_folder_opener_enabled,
                learn_from_usage,
                theme.as_str(),
                density.as_str(),
                max_results_text.as_str(),
                show_tooltips,
                aliases_text.as_str(),
                web_searches_text.as_str(),
            ) {
                Ok(config) => config,
                Err(SettingsConfigError::EmptyAlternateFolderOpener) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_text("Alternate folder opener cannot be empty.".into());
                    }
                    return;
                }
                Err(SettingsConfigError::InvalidMaxResults) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_text("Max results must be a positive number.".into());
                    }
                    return;
                }
                Err(SettingsConfigError::InvalidTheme) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_text("Theme must be dark, dim, or light.".into());
                    }
                    return;
                }
                Err(SettingsConfigError::InvalidDensity) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_text("Density must be comfortable or compact.".into());
                    }
                    return;
                }
                Err(SettingsConfigError::InvalidAliases(message)) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_text(format!("Could not save aliases: {message}").into());
                    }
                    return;
                }
                Err(SettingsConfigError::InvalidWebSearches(message)) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_text(
                            format!("Could not save search engines: {message}").into(),
                        );
                    }
                    return;
                }
            };

            let runtime_config = config_to_save.clone().normalized();
            if runtime_config == *config_state.borrow() {
                return;
            }

            if let Err(error) = config::save_config_with_backup(&config_to_save) {
                eprintln!("{error}");
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text(format!("Could not save settings: {error}").into());
                }
                return;
            }

            *config_state.borrow_mut() = runtime_config;
            let updated_projects =
                projects::scan_project_roots(&config_state.borrow().folder_sources);
            *projects.borrow_mut() = updated_projects;

            if let Some(ui) = weak.upgrade() {
                let query = ui.get_query_text();
                let effective_query =
                    effective_search_query(query.as_str(), ui.get_active_search_keyword().as_str());
                refresh_result_view(
                    &ui,
                    ResultRefreshContext {
                        config: &config_state.borrow(),
                        ranking_state: &ranking_state.borrow(),
                        app_state: &app_install_state.borrow(),
                        projects: &projects.borrow(),
                        apps: &apps.borrow(),
                        current_results: &current_results,
                        results_model: &results_model,
                        icon_cache: &icon_cache,
                        profile,
                    },
                    effective_query.as_str(),
                    ResultSelection::QueryDefault,
                );
                refresh_settings_dependent_ui(
                    &ui,
                    &config_state.borrow(),
                    &projects.borrow(),
                    &apps.borrow(),
                    &ranking_state.borrow(),
                    &icon_cache,
                    &socket_path,
                );
                set_ephemeral_status(&ui, "Settings saved.");
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

    ui.on_settings_alias_save_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let ranking_state = ranking_state.clone();
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move |index, name, keyword, kind, target| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut next = config_state.borrow().clone();
            let Some(alias) = next.aliases.get_mut(index as usize) else {
                return;
            };
            if name.trim().is_empty() || keyword.trim().is_empty() || target.trim().is_empty() {
                ui.set_status_text("Alias name, keyword, and target are required.".into());
                return;
            }
            alias.name = name.trim().to_owned();
            alias.query = keyword.trim().to_owned();
            alias.target = target.trim().to_owned();
            alias.kind = parse_alias_kind(kind.as_str());
            save_collection_change(
                &ui,
                &config_state,
                next,
                settings_save_blocked,
                "Alias saved.",
                &projects.borrow(),
                &apps.borrow(),
                &ranking_state.borrow(),
                &icon_cache,
                &socket_path,
            );
        }
    });
    ui.on_settings_alias_add_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let ranking_state = ranking_state.clone();
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut next = config_state.borrow().clone();
            next.aliases.push(config::AliasConfig {
                name: "New alias".into(),
                query: "alias".into(),
                target: "https://example.com".into(),
                kind: Some(config::AliasKind::Url),
            });
            save_collection_change(
                &ui,
                &config_state,
                next,
                settings_save_blocked,
                "Alias added.",
                &projects.borrow(),
                &apps.borrow(),
                &ranking_state.borrow(),
                &icon_cache,
                &socket_path,
            );
        }
    });
    ui.on_settings_alias_remove_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let ranking_state = ranking_state.clone();
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move |index| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut next = config_state.borrow().clone();
            if (index as usize) < next.aliases.len() {
                next.aliases.remove(index as usize);
            }
            save_collection_change(
                &ui,
                &config_state,
                next,
                settings_save_blocked,
                "Alias removed.",
                &projects.borrow(),
                &apps.borrow(),
                &ranking_state.borrow(),
                &icon_cache,
                &socket_path,
            );
        }
    });
    ui.on_settings_web_search_save_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let ranking_state = ranking_state.clone();
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move |index, name, keyword, url, enabled| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if name.trim().is_empty() || keyword.trim().is_empty() || !url.contains("%s") {
                ui.set_status_text(
                    "Search name and keyword are required; URL must contain %s.".into(),
                );
                return;
            }
            let mut next = config_state.borrow().clone();
            let updated = config::WebSearchConfig {
                name: name.trim().into(),
                keyword: keyword.trim().into(),
                url: url.trim().into(),
                enabled,
            };
            if let Some(engine) = next.web_searches.get_mut(index as usize) {
                *engine = updated;
            } else if index as usize == next.web_searches.len() {
                next.web_searches.push(updated);
            } else {
                return;
            }
            save_collection_change(
                &ui,
                &config_state,
                next,
                settings_save_blocked,
                "Search engine saved.",
                &projects.borrow(),
                &apps.borrow(),
                &ranking_state.borrow(),
                &icon_cache,
                &socket_path,
            );
        }
    });
    ui.on_settings_web_search_add_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut searches = config_state.borrow().web_searches.clone();
            searches.push(config::WebSearchConfig {
                name: String::new(),
                keyword: String::new(),
                url: String::new(),
                enabled: true,
            });
            ui.set_settings_web_searches(
                Rc::new(VecModel::from(web_search_items(&searches))).into(),
            );
            ui.set_status_text("Fill in the new search engine fields, then save it.".into());
        }
    });
    ui.on_settings_web_search_remove_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let projects = projects.clone();
        let apps = apps.clone();
        let ranking_state = ranking_state.clone();
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move |index| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut next = config_state.borrow().clone();
            if index > 0 && (index as usize) < next.web_searches.len() {
                next.web_searches.remove(index as usize);
            }
            save_collection_change(
                &ui,
                &config_state,
                next,
                settings_save_blocked,
                "Search engine removed.",
                &projects.borrow(),
                &apps.borrow(),
                &ranking_state.borrow(),
                &icon_cache,
                &socket_path,
            );
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
                    ui.get_settings_provider_aliases(),
                    ui.get_settings_provider_web_search(),
                    ui.get_settings_provider_unit_conversion(),
                    ui.get_settings_provider_currency_conversion(),
                    ui.get_settings_provider_time_lookup(),
                    ui.get_settings_provider_utility_actions(),
                    ui.get_settings_alternate_folder_opener_enabled(),
                    ui.get_settings_ranking_learn_from_usage(),
                    ui.get_settings_theme(),
                    ui.get_settings_density(),
                    ui.get_settings_max_results(),
                    ui.get_settings_show_tooltips(),
                    ui.get_settings_aliases_text(),
                    ui.get_settings_web_searches_text(),
                );
            }
        }
    });

    ui.on_settings_clear_ranking_requested({
        let weak = ui.as_weak();
        let config_state = config_state.clone();
        let app_install_state = app_install_state.clone();
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
                let effective_query =
                    effective_search_query(query.as_str(), ui.get_active_search_keyword().as_str());
                refresh_result_view(
                    &ui,
                    ResultRefreshContext {
                        config: &config_state.borrow(),
                        ranking_state: &ranking_state.borrow(),
                        app_state: &app_install_state.borrow(),
                        projects: &projects.borrow(),
                        apps: &apps.borrow(),
                        current_results: &current_results,
                        results_model: &results_model,
                        icon_cache: &icon_cache,
                        profile,
                    },
                    effective_query.as_str(),
                    ResultSelection::QueryDefault,
                );
                refresh_settings_dependent_ui(
                    &ui,
                    &config_state.borrow(),
                    &projects.borrow(),
                    &apps.borrow(),
                    &ranking_state.borrow(),
                    &icon_cache,
                    &socket_path,
                );
                ui.set_status_text("Ranking history cleared.".into());
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn save_collection_change(
    ui: &AppWindow,
    state: &Rc<RefCell<config::Config>>,
    config_to_save: config::Config,
    blocked: bool,
    message: &str,
    projects: &[projects::Project],
    apps: &[apps::DesktopApp],
    ranking: &ranking::RankingState,
    icon_cache: &Rc<RefCell<IconImageCache>>,
    socket_path: &std::path::Path,
) {
    if blocked {
        ui.set_status_text("Could not save settings: fix config.toml and restart rayslash.".into());
        return;
    }
    if let Err(error) = config::save_config_with_backup(&config_to_save) {
        ui.set_status_text(format!("Could not save settings: {error}").into());
        return;
    }
    *state.borrow_mut() = config_to_save.normalized();
    let favicon_searches = state.borrow().web_searches.clone();
    std::thread::spawn(move || {
        for search in &favicon_searches {
            let _ = rayslash_core::web_search::fetch_and_cache_favicon(search);
        }
    });
    refresh_settings_dependent_ui(
        ui,
        &state.borrow(),
        projects,
        apps,
        ranking,
        icon_cache,
        socket_path,
    );
    set_ephemeral_status(ui, message);
}

fn set_ephemeral_status(ui: &AppWindow, message: &str) {
    ui.set_status_text(message.into());

    let expected = message.to_owned();
    let weak = ui.as_weak();
    Timer::single_shot(Duration::from_millis(1800), move || {
        if let Some(ui) = weak.upgrade()
            && ui.get_status_text().as_str() == expected
        {
            ui.set_status_text(DEFAULT_STATUS_TEXT.into());
        }
    });
}
