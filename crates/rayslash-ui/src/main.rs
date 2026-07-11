mod activation;
mod cli;
mod ipc;
mod module_settings;
mod opener_visual;
mod result_items;
mod runtime_state;
mod settings;
mod settings_callbacks;
mod window_state;

use std::{
    cell::{Cell, RefCell},
    env, io,
    path::PathBuf,
    process::ExitCode,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use activation::{ActivationCallbackContext, register_activation_callback};
use module_settings::{
    ModuleSettingsCallbackContext, load_runtime_modules, module_items,
    register_module_settings_callback,
};
use opener_visual::{accent_color_for_icon, to_app_choice_items};
use rayslash_core::{apps, config, projects, providers::ProviderExecutionHint, web_search};
use result_items::{IconImageCache, to_result_items};
use runtime_state::{
    ResultRefreshContext, ResultSelection, SearchResultSet, effective_search_query,
    load_runtime_app_state, load_runtime_ranking_state, profile_enabled, profile_stage,
    refresh_result_view, refresh_settings_dependent_ui, search_result_set, sync_app_install_state,
};
use settings_callbacks::{SettingsCallbackContext, register_settings_callbacks};
use slint::{
    ComponentHandle, Timer, TimerMode, VecModel,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use window_state::{
    handle_ipc_request, hide_launcher, should_start_resident_after_send_error, visible_flag,
};

slint::include_modules!();

pub(crate) const DEFAULT_STATUS_TEXT: &str = "";
const DESKTOP_APP_REFRESH_INTERVAL: Duration = Duration::from_secs(10);

fn main() -> ExitCode {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "rayslash".to_string());
    let args = args.collect::<Vec<_>>();
    let command = match cli::parse_args(&args) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{}", cli::usage(&program));
            if !error.args().is_empty() {
                eprintln!("Unknown arguments: {}", error.args().join(" "));
            }
            return ExitCode::FAILURE;
        }
    };

    let request = match command {
        cli::CliCommand::Run => ipc::IpcRequest::Show,
        cli::CliCommand::Toggle => ipc::IpcRequest::Toggle,
    };
    let socket_path = ipc::socket_path();

    match ipc::send_request(&socket_path, request) {
        Ok(()) => return ExitCode::SUCCESS,
        Err(error) if should_start_resident_after_send_error(&error) => {}
        Err(error) => {
            eprintln!(
                "failed to contact rayslash at {}: {error}; starting a resident instance",
                socket_path.display()
            );
        }
    }

    match run_resident(socket_path, request) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run_resident(socket_path: std::path::PathBuf, request: ipc::IpcRequest) -> Result<(), String> {
    let listener = match ipc::bind_server_socket(&socket_path) {
        Ok(listener) => listener,
        Err(ipc::BindSocketError::AlreadyRunning) => {
            ipc::send_request(&socket_path, request).map_err(|error| {
                format!(
                    "another rayslash instance is running, but IPC request to {} failed: {error}",
                    socket_path.display()
                )
            })?;
            return Ok(());
        }
        Err(ipc::BindSocketError::Io(error)) => {
            return Err(format!(
                "failed to bind rayslash IPC socket at {}: {error}",
                socket_path.display()
            ));
        }
    };

    let result = run_gui(listener, socket_path.clone());
    if let Err(error) = std::fs::remove_file(&socket_path)
        && error.kind() != io::ErrorKind::NotFound
    {
        eprintln!(
            "failed to remove rayslash IPC socket at {}: {error}",
            socket_path.display()
        );
    }

    result.map_err(|error| format!("failed to run rayslash UI: {error}"))
}

fn run_gui(
    listener: std::os::unix::net::UnixListener,
    socket_path: PathBuf,
) -> Result<(), slint::PlatformError> {
    let profile = profile_enabled();
    let startup_started = Instant::now();

    slint::BackendSelector::new().select()?;
    slint::set_xdg_app_id(rayslash_core::APP_ID)?;

    let stage_started = Instant::now();
    let ui = AppWindow::new()?;
    profile_stage(profile, "ui construct", stage_started);

    let is_visible = visible_flag(true);
    let suppress_next_focus_hide = Rc::new(Cell::new(false));

    let stage_started = Instant::now();
    let (mut config, settings_save_blocked) = match config::load_config() {
        Ok(config) => (config, false),
        Err(error) => {
            eprintln!("{error}; using default config");
            (config::Config::default(), true)
        }
    };
    let runtime_modules = load_runtime_modules(&config.providers, !settings_save_blocked);
    runtime_modules
        .config
        .apply_to_provider_config(&mut config.providers);
    let module_writes_blocked = runtime_modules.writes_blocked;
    let module_state = Rc::new(RefCell::new(runtime_modules.config));
    let config_state = Rc::new(RefCell::new(config));
    let favicon_searches = config_state.borrow().web_searches.clone();
    thread::spawn(move || {
        for search in &favicon_searches {
            let _ = web_search::fetch_and_cache_favicon(search);
        }
    });
    profile_stage(profile, "config load", stage_started);

    let stage_started = Instant::now();
    let ranking_state = Rc::new(RefCell::new(load_runtime_ranking_state()));
    profile_stage(
        profile,
        &format!(
            "ranking state load ({} entries)",
            ranking_state.borrow().entries.len()
        ),
        stage_started,
    );

    let stage_started = Instant::now();
    let app_install_state = Rc::new(RefCell::new(load_runtime_app_state()));
    profile_stage(
        profile,
        &format!(
            "app state load ({} new apps)",
            app_install_state.borrow().new_app_ids.len()
        ),
        stage_started,
    );

    let stage_started = Instant::now();
    let projects = Rc::new(RefCell::new(projects::scan_project_roots(
        &config_state.borrow().folder_sources,
    )));
    profile_stage(
        profile,
        &format!("project scan ({} projects)", projects.borrow().len()),
        stage_started,
    );

    let stage_started = Instant::now();
    let apps = Rc::new(RefCell::new(apps::discover_desktop_apps()));
    let last_desktop_app_refresh = Rc::new(RefCell::new(Instant::now()));
    sync_app_install_state(&app_install_state, &apps.borrow());
    profile_stage(
        profile,
        &format!("app discovery ({} apps)", apps.borrow().len()),
        stage_started,
    );

    let stage_started = Instant::now();
    let initial_result_set = search_result_set(
        &config_state.borrow(),
        &ranking_state.borrow(),
        &app_install_state.borrow(),
        &projects.borrow(),
        &apps.borrow(),
        "",
    );
    let initial_result_tip = initial_result_set.result_tip.clone();
    let current_results = Rc::new(RefCell::new(initial_result_set.results));
    profile_stage(
        profile,
        &format!(
            "initial search ({} results)",
            current_results.borrow().len()
        ),
        stage_started,
    );

    let icon_cache = Rc::new(RefCell::new(IconImageCache::new()));
    let stage_started = Instant::now();
    let results_model = Rc::new(VecModel::from(to_result_items(
        &current_results.borrow(),
        &mut icon_cache.borrow_mut(),
    )));
    profile_stage(profile, "initial result item build", stage_started);
    profile_stage(profile, "startup before event loop", startup_started);

    let remote_search_generation = Arc::new(AtomicU64::new(0));
    let (remote_result_tx, remote_result_rx) = mpsc::channel::<(u64, String, SearchResultSet)>();
    let remote_result_timer = Timer::default();
    remote_result_timer.start(TimerMode::Repeated, Duration::from_millis(24), {
        let weak = ui.as_weak();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        let remote_search_generation = remote_search_generation.clone();
        move || {
            let mut newest = None;
            while let Ok(result) = remote_result_rx.try_recv() {
                newest = Some(result);
            }
            let Some((generation, query, result_set)) = newest else {
                return;
            };
            if remote_search_generation.load(Ordering::Relaxed) != generation {
                return;
            }
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let results = result_set.results;
            let count = results.len();
            results_model.set_vec(to_result_items(&results, &mut icon_cache.borrow_mut()));
            *current_results.borrow_mut() = results;
            ui.set_result_count(count as i32);
            ui.set_result_tip_text(result_set.result_tip.into());
            ui.set_selected_index(runtime_state::selected_index_for_query(
                &query,
                count as i32,
            ));
            ui.invoke_reset_result_scroll();
            if ui.get_status_text().as_str() == "Looking up…" {
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
            }
        }
    });

    ui.set_result_count(current_results.borrow().len() as i32);
    ui.set_result_tip_text(initial_result_tip.into());
    ui.set_results(results_model.clone().into());
    ui.set_selected_index(-1);

    let module_model = Rc::new(VecModel::from(module_items(&module_state.borrow())));
    ui.set_settings_modules(module_model.clone().into());

    let alternate_opener_choices = Rc::new(VecModel::from(to_app_choice_items(
        &apps.borrow(),
        &mut icon_cache.borrow_mut(),
    )));
    ui.set_alternate_opener_choices(alternate_opener_choices.clone().into());
    refresh_settings_dependent_ui(
        &ui,
        &config_state.borrow(),
        &projects.borrow(),
        &apps.borrow(),
        &ranking_state.borrow(),
        &icon_cache,
        &socket_path,
    );
    ui.invoke_focus_search();

    ui.window().on_winit_window_event({
        let weak = ui.as_weak();
        let is_visible = is_visible.clone();
        let suppress_next_focus_hide = suppress_next_focus_hide.clone();
        move |_, event| {
            if matches!(event, winit::event::WindowEvent::Focused(false)) {
                if suppress_next_focus_hide.replace(false) {
                    return EventResult::Propagate;
                }

                let is_visible = is_visible.clone();
                if let Err(error) = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_control_held(false);
                    hide_launcher(&ui, is_visible.as_ref());
                }) {
                    eprintln!("failed to queue rayslash focus-lost hide on UI event loop: {error}");
                }
            }

            EventResult::Propagate
        }
    });

    ui.on_reset_requested({
        let weak = ui.as_weak();
        let projects = projects.clone();
        let apps = apps.clone();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let app_install_state = app_install_state.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_query_text("".into());
                ui.set_active_search_keyword("".into());
                ui.set_active_search_name("".into());
                ui.set_active_search_has_accent(false);
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
                ui.set_settings_open(false);
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
                    "",
                    ResultSelection::Exact(-1),
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
            }
        }
    });

    ui.on_search_keyword_trigger_requested({
        let weak = ui.as_weak();
        let projects = projects.clone();
        let apps = apps.clone();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let app_install_state = app_install_state.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move |keyword| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };

            let trigger = {
                let config = config_state.borrow();
                if !config.providers.web_search {
                    None
                } else {
                    web_search::trigger_from_input(&config.web_searches, keyword.as_str()).map(
                        |template| {
                            let favicon = web_search::cached_favicon_path(template);
                            (
                                template.keyword.clone(),
                                template.name.clone(),
                                !template
                                    .keyword
                                    .eq_ignore_ascii_case(web_search::DEFAULT_SEARCH_KEYWORD)
                                    && favicon.is_some(),
                                accent_color_for_icon(&template.keyword, favicon.as_deref()),
                            )
                        },
                    )
                }
            };

            let Some((keyword, name, has_accent, accent)) = trigger else {
                return false;
            };

            ui.set_active_search_keyword(keyword.into());
            ui.set_active_search_name(name.into());
            ui.set_active_search_has_accent(has_accent);
            ui.set_active_search_accent(accent);
            ui.set_query_text("".into());
            ui.set_status_text(DEFAULT_STATUS_TEXT.into());
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
                "",
                ResultSelection::Exact(-1),
            );

            true
        }
    });

    ui.on_search_keyword_cleared({
        let weak = ui.as_weak();
        let projects = projects.clone();
        let apps = apps.clone();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let app_install_state = app_install_state.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_active_search_keyword("".into());
                ui.set_active_search_name("".into());
                ui.set_active_search_has_accent(false);
                let query = ui.get_query_text();
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
                    query.as_str(),
                    ResultSelection::QueryDefault,
                );
            }
        }
    });

    ui.on_close_requested({
        let weak = ui.as_weak();
        let is_visible = is_visible.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                hide_launcher(&ui, is_visible.as_ref());
            }
        }
    });

    ui.on_query_changed({
        let weak = ui.as_weak();
        let projects = projects.clone();
        let apps = apps.clone();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let app_install_state = app_install_state.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        let remote_search_generation = remote_search_generation.clone();
        let remote_result_tx = remote_result_tx.clone();
        move |query| {
            let stage_started = Instant::now();

            if let Some(ui) = weak.upgrade() {
                let effective_query =
                    effective_search_query(query.as_str(), ui.get_active_search_keyword().as_str());
                let execution_hint = rayslash_core::search::query_execution_hint(
                    &effective_query,
                    &config_state.borrow().providers,
                );

                if let ProviderExecutionHint::DebouncedNetwork { debounce_ms } = execution_hint {
                    let generation = remote_search_generation.fetch_add(1, Ordering::Relaxed) + 1;
                    let expected_generation = remote_search_generation.clone();
                    let config = config_state.borrow().clone();
                    let ranking = ranking_state.borrow().clone();
                    let app_install = app_install_state.borrow().clone();
                    let projects_snapshot = projects.borrow().clone();
                    let apps_snapshot = apps.borrow().clone();
                    let effective_query = effective_query.clone();
                    let remote_result_tx = remote_result_tx.clone();

                    ui.set_status_text("Looking up…".into());
                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(debounce_ms));
                        if expected_generation.load(Ordering::Relaxed) != generation {
                            return;
                        }

                        let result_set = search_result_set(
                            &config,
                            &ranking,
                            &app_install,
                            &projects_snapshot,
                            &apps_snapshot,
                            &effective_query,
                        );
                        if expected_generation.load(Ordering::Relaxed) != generation {
                            return;
                        }

                        let _ = remote_result_tx.send((generation, effective_query, result_set));
                    });
                    return;
                }

                remote_search_generation.fetch_add(1, Ordering::Relaxed);
                let count = refresh_result_view(
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
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
                profile_stage(
                    profile,
                    &format!("query {:?} ({} results)", query.as_str(), count),
                    stage_started,
                );
            }
        }
    });

    register_activation_callback(
        &ui,
        ActivationCallbackContext {
            current_results: current_results.clone(),
            config_state: config_state.clone(),
            app_install_state: app_install_state.clone(),
            ranking_state: ranking_state.clone(),
            projects: projects.clone(),
            apps: apps.clone(),
            is_visible: is_visible.clone(),
        },
    );

    register_settings_callbacks(
        &ui,
        SettingsCallbackContext {
            config_state: config_state.clone(),
            app_install_state: app_install_state.clone(),
            ranking_state: ranking_state.clone(),
            projects: projects.clone(),
            apps: apps.clone(),
            alternate_opener_choices: alternate_opener_choices.clone(),
            current_results: current_results.clone(),
            results_model: results_model.clone(),
            icon_cache: icon_cache.clone(),
            socket_path: socket_path.clone(),
            suppress_next_focus_hide: suppress_next_focus_hide.clone(),
            last_desktop_app_refresh: last_desktop_app_refresh.clone(),
            settings_save_blocked,
            profile,
        },
    );

    register_module_settings_callback(
        &ui,
        ModuleSettingsCallbackContext {
            module_state: module_state.clone(),
            module_model: module_model.clone(),
            module_writes_blocked,
            config_state: config_state.clone(),
            app_install_state: app_install_state.clone(),
            ranking_state: ranking_state.clone(),
            projects: projects.clone(),
            apps: apps.clone(),
            current_results: current_results.clone(),
            results_model: results_model.clone(),
            icon_cache: icon_cache.clone(),
            socket_path: socket_path.clone(),
            remote_search_generation: remote_search_generation.clone(),
            remote_result_tx: remote_result_tx.clone(),
            profile,
        },
    );

    let weak = ui.as_weak();
    let ipc_visibility = is_visible.clone();
    ipc::start_server(listener, move |request| {
        let ipc_visibility = ipc_visibility.clone();
        if let Err(error) = weak.upgrade_in_event_loop(move |ui| {
            handle_ipc_request(&ui, ipc_visibility.as_ref(), request);
        }) {
            eprintln!("failed to queue rayslash IPC request on UI event loop: {error}");
        }
    });

    ui.run()
}
