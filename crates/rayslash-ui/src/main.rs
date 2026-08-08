mod activation;
mod app_updates;
mod cli;
mod ipc;
mod module_settings;
mod opener_visual;
mod persistence;
mod result_items;
mod runtime_state;
mod settings;
mod settings_callbacks;
mod telemetry;
mod window_state;

use std::{
    cell::{Cell, RefCell},
    env, io,
    path::PathBuf,
    process::ExitCode,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use activation::{ActivationCallbackContext, register_activation_callback};
use app_updates::{AppUpdateContext, register_app_updates};
use module_settings::{
    ModuleSettingsCallbackContext, installed_modules_load_diagnostic, load_runtime_modules,
    module_items, register_module_settings_callback, save_modules_diagnostic,
};
use notify::{RecursiveMode, Watcher};
use opener_visual::accent_color_for_icon;
use rayslash_core::{
    app_state, apps, config,
    diagnostics::{OperationalDiagnostic, OperationalDiagnosticCode, Telemetry},
    modules, projects,
    providers::ProviderExecutionHint,
    ranking, web_search,
};
use result_items::{
    IconImageCache, to_result_items, to_result_items_without_images, update_result_items_model,
};
use runtime_state::{
    ResultRefreshContext, ResultSelection, SearchResultSet, apply_desktop_apps,
    effective_search_query, load_runtime_app_state, load_runtime_ranking_state,
    merge_module_results_with_config, profile_enabled, profile_stage,
    query_execution_hint_with_config, refresh_result_view, refresh_settings_dependent_ui,
    search_result_set, should_preserve_pending_module_results, sync_app_install_state,
};
use settings_callbacks::{
    SettingsCallbackContext, register_settings_callbacks, set_ephemeral_status,
};
use slint::{
    ComponentHandle, Model, Timer, VecModel,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use telemetry::DiagnosticsTelemetry;
use window_state::{
    handle_ipc_request, hide_launcher, should_start_resident_after_send_error, visible_flag,
};

slint::include_modules!();

pub(crate) const DEFAULT_STATUS_TEXT: &str = "";
const DESKTOP_APP_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const BACKGROUND_LOCAL_SEARCH_THRESHOLD: usize = 100_000;

struct ModuleSearchJob {
    generation: u64,
    query: String,
    config: config::Config,
    ranking_state: ranking::RankingState,
    module_config: modules::ModulesConfig,
    local_results: SearchResultSet,
    debounce: Duration,
    started: Instant,
}

struct LocalSearchJob {
    generation: u64,
    query: String,
    config: config::Config,
    ranking_state: ranking::RankingState,
    app_state: app_state::AppInstallState,
    module_config: modules::ModulesConfig,
    projects: Arc<Vec<projects::Project>>,
    apps: Arc<Vec<apps::DesktopApp>>,
    debounce: Duration,
    started: Instant,
}

type RemoteSearchResult = (u64, String, SearchResultSet, Instant);

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
        cli::CliCommand::Version => {
            println!("rayslash {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
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

    let restart_requested = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = run_gui(listener, socket_path.clone(), restart_requested.clone());
    if let Err(error) = std::fs::remove_file(&socket_path)
        && error.kind() != io::ErrorKind::NotFound
    {
        eprintln!(
            "failed to remove rayslash IPC socket at {}: {error}",
            socket_path.display()
        );
    }

    if restart_requested.load(Ordering::Acquire) {
        restart_after_update()
            .map_err(|error| format!("update installed, but restart failed: {error}"))?;
    }

    result.map_err(|error| format!("failed to run rayslash UI: {error}"))
}

fn run_gui(
    listener: std::os::unix::net::UnixListener,
    socket_path: PathBuf,
    restart_requested: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), slint::PlatformError> {
    let profile = profile_enabled();
    let startup_started = Instant::now();

    let stage_started = Instant::now();
    slint::BackendSelector::new().select()?;
    slint::set_xdg_app_id(rayslash_core::APP_ID)?;
    profile_stage(profile, "backend select and app ID", stage_started);

    let stage_started = Instant::now();
    let ui = AppWindow::new()?;
    profile_stage(profile, "ui construct", stage_started);

    let is_visible = visible_flag(true);
    let suppress_next_focus_hide = Rc::new(Cell::new(false));

    let stage_started = Instant::now();
    let existing_config = config::config_file().is_some_and(|path| path.is_file());
    let (mut config, settings_save_blocked, config_load_failure) = match config::load_config() {
        Ok(config) => (config, false, None),
        Err(error) => {
            let diagnostic = match &error {
                config::ConfigError::Read { source, .. } => OperationalDiagnostic::from_io(
                    OperationalDiagnosticCode::MainConfigurationLoad,
                    source,
                ),
                config::ConfigError::Parse { .. } => {
                    OperationalDiagnostic::new(OperationalDiagnosticCode::MainConfigurationLoad)
                }
            };
            eprintln!("{error}; using default config");
            (config::Config::default(), true, Some(diagnostic))
        }
    };
    let mut runtime_modules =
        load_runtime_modules(&config.providers, !settings_save_blocked, existing_config);
    let mut module_config_failure = runtime_modules.diagnostic.take();
    let installed_modules = modules::load_installed_modules();
    let installed_modules_failure = installed_modules
        .as_ref()
        .err()
        .map(installed_modules_load_diagnostic);
    if !runtime_modules.writes_blocked
        && let Ok(installed) = installed_modules
        && runtime_modules.config.reconcile_installed(&installed)
        && let Err(error) = modules::save_modules_config(&runtime_modules.config)
    {
        module_config_failure = Some(save_modules_diagnostic(&error));
        eprintln!("{error}; module writes are disabled until restart");
        runtime_modules.writes_blocked = true;
    }
    runtime_modules
        .config
        .apply_to_provider_config(&mut config.providers);
    let module_writes_blocked = runtime_modules.writes_blocked;
    let module_migration_pending = runtime_modules.migration_pending;
    let module_state = Rc::new(RefCell::new(runtime_modules.config));
    let module_catalog = Rc::new(RefCell::new(
        modules::load_cached_registry()
            .map(|registry| registry.index.modules)
            .unwrap_or_default(),
    ));
    let config_state = Rc::new(RefCell::new(config));
    let diagnostics =
        DiagnosticsTelemetry::new(config_state.borrow().diagnostics.send_anonymous_diagnostics);
    if let Some(diagnostic) = config_load_failure {
        diagnostics.operational_failure(diagnostic);
    }
    if let Some(diagnostic) = module_config_failure {
        diagnostics.operational_failure(diagnostic);
    }
    if let Some(diagnostic) = installed_modules_failure {
        diagnostics.operational_failure(diagnostic);
    }
    let favicon_searches = config_state.borrow().web_searches.clone();
    thread::spawn(move || {
        for search in &favicon_searches {
            let _ = web_search::fetch_and_cache_favicon(search);
        }
    });
    profile_stage(profile, "config load", stage_started);

    let stage_started = Instant::now();
    let ranking_state = Rc::new(RefCell::new(load_runtime_ranking_state(
        diagnostics.as_ref(),
    )));
    profile_stage(
        profile,
        &format!(
            "ranking state load ({} entries)",
            ranking_state.borrow().entries.len()
        ),
        stage_started,
    );

    let stage_started = Instant::now();
    let app_install_state = Rc::new(RefCell::new(load_runtime_app_state(diagnostics.as_ref())));
    profile_stage(
        profile,
        &format!(
            "app state load ({} new apps)",
            app_install_state.borrow().new_app_ids.len()
        ),
        stage_started,
    );

    let stage_started = Instant::now();
    let projects = Rc::new(RefCell::new(Arc::new(projects::scan_project_roots(
        &config_state.borrow().folder_sources,
    ))));
    let pending_project_refresh = Arc::new(Mutex::new(None));
    profile_stage(
        profile,
        &format!("project scan ({} projects)", projects.borrow().len()),
        stage_started,
    );

    let stage_started = Instant::now();
    let cached_scan = apps::load_cached_desktop_scan();
    let loaded_cached_apps = cached_scan.is_some();
    let initial_scan = cached_scan.unwrap_or_else(|| {
        let scan = apps::discover_and_cache_desktop_apps_with_diagnostics();
        diagnostics.application_scan_completed(&scan.statistics);
        scan
    });
    if loaded_cached_apps {
        diagnostics.record_cached_scan(initial_scan.statistics.clone());
    }
    let initial_apps = initial_scan.apps;
    let apps = Rc::new(RefCell::new(Arc::new(initial_apps)));
    let pending_app_refresh = Arc::new(Mutex::new(None));
    if loaded_cached_apps {
        thread::spawn({
            let weak = ui.as_weak();
            let pending_app_refresh = pending_app_refresh.clone();
            let diagnostics = diagnostics.clone();
            move || {
                if apps::desktop_apps_cache_is_current() {
                    return;
                }
                let scan = apps::discover_and_cache_desktop_apps_with_diagnostics();
                diagnostics.application_scan_completed(&scan.statistics);
                *pending_app_refresh
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(scan);
                let _ = weak.upgrade_in_event_loop(|ui| ui.invoke_apply_desktop_refresh());
            }
        });
    }
    let last_desktop_app_refresh = Rc::new(RefCell::new(Instant::now()));
    sync_app_install_state(&app_install_state, &apps.borrow(), diagnostics.clone());
    profile_stage(
        profile,
        &format!(
            "app catalog {} ({} apps)",
            if loaded_cached_apps {
                "cache load"
            } else {
                "discovery"
            },
            apps.borrow().len()
        ),
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
    let results_model = Rc::new(VecModel::from(to_result_items_without_images(
        &current_results.borrow(),
        &mut icon_cache.borrow_mut(),
    )));
    profile_stage(profile, "initial result item build", stage_started);
    profile_stage(
        profile,
        "startup initial result model ready",
        startup_started,
    );

    let remote_search_generation = Arc::new(AtomicU64::new(0));
    let (remote_result_tx, remote_result_rx) = mpsc::channel::<RemoteSearchResult>();
    let pending_remote_result = Arc::new(Mutex::new(Option::<RemoteSearchResult>::None));
    let pending_remote_result_for_ui = pending_remote_result.clone();
    ui.on_apply_remote_results({
        let weak = ui.as_weak();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        let remote_search_generation = remote_search_generation.clone();
        move || {
            let Some((generation, query, result_set, query_started)) = pending_remote_result_for_ui
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            else {
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
            let previous_count = current_results.borrow().len();
            update_result_items_model(
                &results_model,
                to_result_items(&results, &mut icon_cache.borrow_mut()),
            );
            *current_results.borrow_mut() = results;
            ui.set_result_count(count as i32);
            ui.set_result_tip_text(result_set.result_tip.into());
            ui.set_selected_index(runtime_state::selected_index_for_query(
                &query,
                count as i32,
            ));
            if previous_count != count {
                ui.invoke_reset_result_scroll();
            }
            if matches!(ui.get_status_text().as_str(), "Looking up…" | "Searching…") {
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
            }
            profile_stage(
                profile,
                &format!("remote query {query:?} end to end"),
                query_started,
            );
        }
    });
    thread::spawn({
        let weak = ui.as_weak();
        let pending_remote_result = pending_remote_result.clone();
        move || {
            while let Ok(mut result) = remote_result_rx.recv() {
                while let Ok(newer) = remote_result_rx.try_recv() {
                    result = newer;
                }
                *pending_remote_result
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(result);
                if weak
                    .upgrade_in_event_loop(|ui| ui.invoke_apply_remote_results())
                    .is_err()
                {
                    break;
                }
            }
        }
    });
    let (module_search_tx, module_search_rx) = mpsc::channel::<ModuleSearchJob>();
    let (local_search_tx, local_search_rx) = mpsc::channel::<LocalSearchJob>();
    thread::spawn({
        let module_search_tx = module_search_tx.clone();
        let remote_result_tx = remote_result_tx.clone();
        let remote_search_generation = remote_search_generation.clone();
        move || {
            while let Ok(mut job) = local_search_rx.recv() {
                while let Ok(newer) = local_search_rx.try_recv() {
                    job = newer;
                }
                if remote_search_generation.load(Ordering::Acquire) != job.generation {
                    continue;
                }
                let local_results = runtime_state::local_search_result_set(
                    &job.config,
                    &job.ranking_state,
                    &job.app_state,
                    &job.projects,
                    &job.apps,
                    &job.query,
                );
                if remote_search_generation.load(Ordering::Acquire) != job.generation {
                    continue;
                }
                if job.query.trim().is_empty() {
                    let _ = remote_result_tx.send((
                        job.generation,
                        job.query,
                        local_results,
                        job.started,
                    ));
                } else {
                    let _ = module_search_tx.send(ModuleSearchJob {
                        generation: job.generation,
                        query: job.query,
                        config: job.config,
                        ranking_state: job.ranking_state,
                        module_config: job.module_config,
                        local_results,
                        debounce: job.debounce,
                        started: job.started,
                    });
                }
            }
        }
    });
    thread::spawn({
        let remote_result_tx = remote_result_tx.clone();
        let remote_search_generation = remote_search_generation.clone();
        let diagnostics = diagnostics.clone();
        move || {
            while let Ok(mut job) = module_search_rx.recv() {
                loop {
                    match module_search_rx.recv_timeout(job.debounce) {
                        Ok(newer) => job = newer,
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                if remote_search_generation.load(Ordering::Acquire) != job.generation {
                    continue;
                }
                let result_set = merge_module_results_with_config(
                    &job.config,
                    &job.ranking_state,
                    &job.module_config,
                    job.local_results,
                    &job.query,
                    Some(diagnostics.as_ref()),
                );
                if remote_search_generation.load(Ordering::Acquire) == job.generation {
                    let _ =
                        remote_result_tx.send((job.generation, job.query, result_set, job.started));
                }
            }
        }
    });

    ui.set_result_count(current_results.borrow().len() as i32);
    ui.set_result_tip_text(initial_result_tip.into());
    ui.set_results(results_model.clone().into());
    ui.set_selected_index(-1);

    let module_model = Rc::new(VecModel::from(module_items(
        &module_state.borrow(),
        &module_catalog.borrow(),
    )));
    ui.set_settings_modules(module_model.clone().into());
    ui.set_settings_module_update_count(module_update_count(&module_model));
    if cfg!(debug_assertions)
        && std::env::var_os("RAYSLASH_PREVIEW_UPDATES").as_deref()
            == Some(std::ffi::OsStr::new("1"))
    {
        ui.set_settings_module_update_count(3);
    }
    if module_migration_pending {
        ui.set_status_text(
            "Optional providers were migrated without downloading code. Open Settings → Modules and choose Restore for each module you want."
                .into(),
        );
    }

    let pending_registry_refresh = Arc::new(Mutex::new(
        Option::<Result<modules::RegistryRefresh, String>>::None,
    ));
    let pending_registry_refresh_for_ui = pending_registry_refresh.clone();
    ui.on_apply_registry_refresh({
        let weak = ui.as_weak();
        let module_catalog = module_catalog.clone();
        let module_state = module_state.clone();
        let module_model = module_model.clone();
        let config_state = config_state.clone();
        move || match pending_registry_refresh_for_ui
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            Some(Ok(registry)) => {
                *module_catalog.borrow_mut() = registry.index.modules;
                if let Some(ui) = weak.upgrade() {
                    let previous_updates = ui.get_settings_module_update_count();
                    ui.invoke_settings_module_sort_requested(ui.get_settings_module_sort_order());
                    let updates = module_update_count(&module_model);
                    ui.set_settings_module_update_count(updates);
                    if updates > previous_updates
                        && config_state.borrow().updates.notify_module_updates
                    {
                        set_ephemeral_status(
                            &ui,
                            &format!(
                                "{updates} module update{} available.",
                                if updates == 1 { " is" } else { "s are" }
                            ),
                        );
                    }
                    if registry.from_cache {
                        ui.set_status_text("Using the last verified module catalog.".into());
                    }
                } else {
                    module_model.set_vec(module_items(
                        &module_state.borrow(),
                        &module_catalog.borrow(),
                    ));
                }
            }
            Some(Err(error)) => {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text(format!("Could not refresh module catalog: {error}").into());
                }
            }
            None => {}
        }
    });
    thread::spawn({
        let weak = ui.as_weak();
        let diagnostics = diagnostics.clone();
        move || {
            let refresh = modules::refresh_registry_if_stale(Duration::from_secs(6 * 60 * 60));
            if refresh.is_err() {
                diagnostics.operational_failure(OperationalDiagnostic::new(
                    OperationalDiagnosticCode::ModuleRegistryRefresh,
                ));
            }
            *pending_registry_refresh
                .lock()
                .unwrap_or_else(|error| error.into_inner()) =
                Some(refresh.map_err(|error| error.to_string()));
            if weak
                .upgrade_in_event_loop(|ui| ui.invoke_apply_registry_refresh())
                .is_err()
            {
                diagnostics.operational_failure(OperationalDiagnostic::new(
                    OperationalDiagnosticCode::WindowUiDispatch,
                ));
            }
        }
    });

    let alternate_opener_choices = Rc::new(VecModel::from(Vec::<AppChoiceItem>::new()));
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
    ui.set_settings_diagnostics_summary(diagnostics.local_summary().into());
    register_app_updates(
        &ui,
        AppUpdateContext {
            config_state: config_state.clone(),
            diagnostics: diagnostics.clone(),
            restart_requested,
        },
    );
    if cfg!(debug_assertions)
        && std::env::var_os("RAYSLASH_PREVIEW_SETTINGS").as_deref()
            == Some(std::ffi::OsStr::new("info"))
    {
        ui.set_settings_open(true);
        ui.set_settings_section("info".into());
    }
    ui.invoke_focus_search();

    if profile && std::env::var_os("RAYSLASH_PROFILE_FRAME").is_some_and(|value| value != "0") {
        let first_frame_rendered = Rc::new(Cell::new(false));
        let marker = first_frame_rendered.clone();
        if let Err(error) = ui.window().set_rendering_notifier(move |state, _| {
            if matches!(state, slint::RenderingState::AfterRendering) && !marker.replace(true) {
                profile_stage(
                    true,
                    "startup first frame rendered/submitted",
                    startup_started,
                );
            }
        }) {
            eprintln!("[rayslash profile] frame-render telemetry unavailable: {error}");
        }
    }

    let first_redraw_profiled = Rc::new(Cell::new(false));
    ui.window().on_winit_window_event({
        let weak = ui.as_weak();
        let is_visible = is_visible.clone();
        let suppress_next_focus_hide = suppress_next_focus_hide.clone();
        let first_redraw_profiled = first_redraw_profiled.clone();
        let diagnostics = diagnostics.clone();
        move |_, event| {
            if matches!(&event, winit::event::WindowEvent::RedrawRequested)
                && !first_redraw_profiled.replace(true)
            {
                profile_stage(profile, "startup first redraw requested", startup_started);
            }
            if matches!(&event, winit::event::WindowEvent::Focused(false)) {
                if weak.upgrade().is_some_and(|ui| {
                    ui.get_settings_web_search_editor_open() || ui.get_settings_alias_editor_open()
                }) {
                    return EventResult::Propagate;
                }
                if suppress_next_focus_hide.replace(false) {
                    return EventResult::Propagate;
                }

                let is_visible = is_visible.clone();
                let diagnostics = diagnostics.clone();
                let dispatch_diagnostics = diagnostics.clone();
                if let Err(error) = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_control_held(false);
                    hide_launcher(&ui, is_visible.as_ref(), dispatch_diagnostics.as_ref());
                }) {
                    diagnostics.operational_failure(OperationalDiagnostic::new(
                        OperationalDiagnosticCode::WindowUiDispatch,
                    ));
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
        let module_state = module_state.clone();
        move |keyword| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };

            let trigger = {
                let config = config_state.borrow();
                let installed = modules::load_installed_modules()
                    .ok()
                    .is_some_and(|state| state.modules.contains_key(modules::WEB_SEARCH_MODULE_ID));
                if !installed
                    || module_state
                        .borrow()
                        .is_enabled(modules::WEB_SEARCH_MODULE_ID)
                        != Some(true)
                {
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
        let diagnostics = diagnostics.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                hide_launcher(&ui, is_visible.as_ref(), diagnostics.as_ref());
            }
        }
    });

    let last_effective_query = Rc::new(RefCell::new(String::new()));
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
        let module_state = module_state.clone();
        let remote_search_generation = remote_search_generation.clone();
        let module_search_tx = module_search_tx.clone();
        let local_search_tx = local_search_tx.clone();
        let last_effective_query = last_effective_query.clone();
        move |query| {
            let stage_started = Instant::now();

            if let Some(ui) = weak.upgrade() {
                let effective_query =
                    effective_search_query(query.as_str(), ui.get_active_search_keyword().as_str());
                let previous_results = current_results.borrow().clone();
                let previous_result_tip = ui.get_result_tip_text().to_string();
                let preserve_previous = should_preserve_pending_module_results(
                    &last_effective_query.borrow(),
                    &effective_query,
                    &previous_results,
                );
                *last_effective_query.borrow_mut() = effective_query.clone();
                let execution_hint = query_execution_hint_with_config(
                    &config_state.borrow(),
                    &module_state.borrow(),
                    &effective_query,
                );
                let generation = remote_search_generation.fetch_add(1, Ordering::AcqRel) + 1;
                let debounce = match execution_hint {
                    ProviderExecutionHint::DebouncedNetwork { debounce_ms } => {
                        Duration::from_millis(debounce_ms)
                    }
                    ProviderExecutionHint::Local => Duration::ZERO,
                };
                if projects.borrow().len() + apps.borrow().len()
                    >= BACKGROUND_LOCAL_SEARCH_THRESHOLD
                {
                    ui.set_status_text("Searching…".into());
                    let _ = local_search_tx.send(LocalSearchJob {
                        generation,
                        query: effective_query,
                        config: config_state.borrow().clone(),
                        ranking_state: ranking_state.borrow().clone(),
                        app_state: app_install_state.borrow().clone(),
                        module_config: module_state.borrow().clone(),
                        projects: projects.borrow().clone(),
                        apps: apps.borrow().clone(),
                        debounce,
                        started: stage_started,
                    });
                    profile_stage(
                        profile,
                        &format!("query {:?} queued for background search", query.as_str()),
                        stage_started,
                    );
                    return;
                }
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
                let local_results = SearchResultSet {
                    results: current_results.borrow().clone(),
                    result_tip: ui.get_result_tip_text().to_string(),
                };
                if preserve_previous {
                    let previous_count = previous_results.len();
                    update_result_items_model(
                        &results_model,
                        to_result_items(&previous_results, &mut icon_cache.borrow_mut()),
                    );
                    *current_results.borrow_mut() = previous_results;
                    ui.set_result_count(previous_count as i32);
                    ui.set_result_tip_text(previous_result_tip.into());
                    ui.set_selected_index(runtime_state::selected_index_for_query(
                        &effective_query,
                        previous_count as i32,
                    ));
                }
                let debounce = match execution_hint {
                    ProviderExecutionHint::DebouncedNetwork { debounce_ms } => {
                        ui.set_status_text("Looking up…".into());
                        Duration::from_millis(debounce_ms)
                    }
                    ProviderExecutionHint::Local => {
                        ui.set_status_text(DEFAULT_STATUS_TEXT.into());
                        Duration::ZERO
                    }
                };
                if !effective_query.trim().is_empty() {
                    let _ = module_search_tx.send(ModuleSearchJob {
                        generation,
                        query: effective_query,
                        config: config_state.borrow().clone(),
                        ranking_state: ranking_state.borrow().clone(),
                        module_config: module_state.borrow().clone(),
                        local_results,
                        debounce,
                        started: stage_started,
                    });
                }
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
            telemetry: diagnostics.clone(),
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
            diagnostics: diagnostics.clone(),
            project_watch_tx: spawn_project_watcher(
                ui.as_weak(),
                config_state.borrow().folder_sources.clone(),
                pending_project_refresh.clone(),
                diagnostics.clone(),
            ),
            settings_save_blocked,
            profile,
        },
    );

    ui.on_apply_project_refresh({
        let weak = ui.as_weak();
        let pending_project_refresh = pending_project_refresh.clone();
        let projects = projects.clone();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let app_install_state = app_install_state.clone();
        let apps = apps.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        let socket_path = socket_path.clone();
        move || {
            let refreshed = pending_project_refresh
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take();
            let Some(refreshed) = refreshed else {
                return;
            };
            *projects.borrow_mut() = Arc::new(refreshed);
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
                let query = effective_search_query(
                    ui.get_query_text().as_str(),
                    ui.get_active_search_keyword().as_str(),
                );
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
                    &query,
                    ResultSelection::QueryDefault,
                );
            }
        }
    });

    {
        let pending_app_refresh = pending_app_refresh.clone();
        let pending_app_refresh_for_watcher = pending_app_refresh.clone();
        ui.on_apply_desktop_refresh({
            let weak = ui.as_weak();
            let apps = apps.clone();
            let app_install_state = app_install_state.clone();
            let alternate_opener_choices = alternate_opener_choices.clone();
            let icon_cache = icon_cache.clone();
            let config_state = config_state.clone();
            let ranking_state = ranking_state.clone();
            let projects = projects.clone();
            let current_results = current_results.clone();
            let results_model = results_model.clone();
            let last_desktop_app_refresh = last_desktop_app_refresh.clone();
            let diagnostics = diagnostics.clone();
            move || {
                let discovered_scan = pending_app_refresh
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .take();
                let Some(discovered_scan) = discovered_scan else {
                    return;
                };
                if let Some(ui) = weak.upgrade() {
                    ui.set_settings_diagnostics_summary(diagnostics.local_summary().into());
                }
                if discovered_scan.apps.as_slice() != apps.borrow().as_slice() {
                    apply_desktop_apps(
                        &apps,
                        &app_install_state,
                        &alternate_opener_choices,
                        &icon_cache,
                        discovered_scan.apps,
                        (profile, "background reconciliation", Instant::now()),
                        diagnostics.clone(),
                    );
                    *last_desktop_app_refresh.borrow_mut() = Instant::now();
                    if let Some(ui) = weak.upgrade() {
                        let effective_query = effective_search_query(
                            ui.get_query_text().as_str(),
                            ui.get_active_search_keyword().as_str(),
                        );
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
                            &effective_query,
                            ResultSelection::QueryDefault,
                        );
                    }
                }
            }
        });
        spawn_desktop_app_watcher(
            ui.as_weak(),
            pending_app_refresh_for_watcher,
            diagnostics.clone(),
        );
    }

    register_module_settings_callback(
        &ui,
        ModuleSettingsCallbackContext {
            module_state: module_state.clone(),
            module_catalog: module_catalog.clone(),
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
            diagnostics: diagnostics.clone(),
            profile,
        },
    );

    let weak = ui.as_weak();
    let ipc_visibility = is_visible.clone();
    let ipc_diagnostics = diagnostics.clone();
    ipc::start_server(listener, diagnostics.clone(), move |request| {
        let request_started = Instant::now();
        let ipc_visibility = ipc_visibility.clone();
        let diagnostics = ipc_diagnostics.clone();
        if let Err(error) = weak.upgrade_in_event_loop(move |ui| {
            handle_ipc_request(&ui, ipc_visibility.as_ref(), request, diagnostics.as_ref());
            profile_stage(
                profile,
                &format!("IPC {request:?} queued-to-handled"),
                request_started,
            );
        }) {
            ipc_diagnostics.operational_failure(OperationalDiagnostic::new(
                OperationalDiagnosticCode::IpcUiDispatch,
            ));
            eprintln!("failed to queue rayslash IPC request on UI event loop: {error}");
        }
    });

    profile_stage(profile, "startup callbacks and IPC ready", startup_started);
    let show_started = Instant::now();
    if let Err(error) = ui.show() {
        diagnostics.operational_failure(OperationalDiagnostic::new(
            OperationalDiagnosticCode::WindowShow,
        ));
        return Err(error);
    }
    #[cfg(debug_assertions)]
    if let Some(snapshot_path) = std::env::var_os("RAYSLASH_PREVIEW_SNAPSHOT") {
        let weak = ui.as_weak();
        Timer::single_shot(Duration::from_secs(1), move || {
            if let Some(ui) = weak.upgrade() {
                if std::env::var_os("RAYSLASH_PREVIEW_SETTINGS").as_deref()
                    == Some(std::ffi::OsStr::new("info"))
                {
                    ui.set_settings_open(true);
                    ui.set_settings_section("info".into());
                }
                if std::env::var_os("RAYSLASH_PREVIEW_UPDATES").as_deref()
                    == Some(std::ffi::OsStr::new("1"))
                {
                    ui.set_settings_module_update_count(3);
                    ui.set_status_text("".into());
                }
                if let Ok(query) = std::env::var("RAYSLASH_PREVIEW_QUERY") {
                    ui.set_query_text(query.clone().into());
                    ui.invoke_query_changed(query.into());
                }
                ui.window().request_redraw();
                let weak = ui.as_weak();
                Timer::single_shot(Duration::from_millis(500), move || {
                    if let Some(ui) = weak.upgrade() {
                        match ui.window().take_snapshot() {
                            Ok(pixels) => {
                                if let Err(error) = image::save_buffer(
                                    &snapshot_path,
                                    pixels.as_bytes(),
                                    pixels.width(),
                                    pixels.height(),
                                    image::ColorType::Rgba8,
                                ) {
                                    eprintln!("could not save UI preview snapshot: {error}");
                                }
                            }
                            Err(error) => {
                                eprintln!("could not capture UI preview snapshot: {error}")
                            }
                        }
                        let _ = slint::quit_event_loop();
                    }
                });
            }
        });
    }
    profile_stage(profile, "startup show call", show_started);
    profile_stage(profile, "startup ready for event loop", startup_started);
    Timer::single_shot(Duration::from_millis(500), {
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move || {
            results_model.set_vec(to_result_items(
                &current_results.borrow(),
                &mut icon_cache.borrow_mut(),
            ));
        }
    });
    slint::run_event_loop_until_quit()
}

fn module_update_count(model: &VecModel<crate::ModuleItem>) -> i32 {
    model.iter().filter(|item| item.update_available).count() as i32
}

fn restart_after_update() -> io::Result<()> {
    if std::env::var_os("FLATPAK_ID").is_some() {
        std::process::Command::new("flatpak-spawn")
            .args(["--host", "flatpak", "run", rayslash_core::APP_ID])
            .spawn()?;
    } else {
        std::process::Command::new(std::env::current_exe()?).spawn()?;
    }
    Ok(())
}

fn spawn_desktop_app_watcher(
    weak: slint::Weak<AppWindow>,
    pending_app_refresh: Arc<Mutex<Option<apps::ApplicationScan>>>,
    diagnostics: Arc<DiagnosticsTelemetry>,
) {
    thread::spawn(move || {
        let (event_tx, event_rx) = mpsc::channel();
        let watcher_diagnostics = diagnostics.clone();
        let Ok(mut watcher) =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
                Ok(event) if filesystem_event_requires_refresh(&event) => {
                    let _ = event_tx.send(());
                }
                Ok(_) => {}
                Err(_) => watcher_diagnostics.operational_failure(OperationalDiagnostic::new(
                    OperationalDiagnosticCode::DesktopWatcherWatch,
                )),
            })
        else {
            diagnostics.operational_failure(OperationalDiagnostic::new(
                OperationalDiagnosticCode::DesktopWatcherInitialize,
            ));
            return;
        };
        let mut watched = false;
        for directory in apps::desktop_application_dirs() {
            if directory.is_dir() {
                if watcher.watch(&directory, RecursiveMode::Recursive).is_ok() {
                    watched = true;
                } else {
                    diagnostics.operational_failure(OperationalDiagnostic::new(
                        OperationalDiagnosticCode::DesktopWatcherWatch,
                    ));
                }
            }
        }
        if !watched {
            return;
        }
        while event_rx.recv().is_ok() {
            while event_rx.recv_timeout(Duration::from_millis(250)).is_ok() {}
            let discovered = apps::discover_and_cache_desktop_apps_with_diagnostics();
            diagnostics.application_scan_completed(&discovered.statistics);
            *pending_app_refresh
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(discovered);
            if weak
                .upgrade_in_event_loop(|ui| ui.invoke_apply_desktop_refresh())
                .is_err()
            {
                diagnostics.operational_failure(OperationalDiagnostic::new(
                    OperationalDiagnosticCode::WindowUiDispatch,
                ));
                return;
            }
        }
    });
}

fn spawn_project_watcher(
    weak: slint::Weak<AppWindow>,
    initial_roots: Vec<PathBuf>,
    pending_project_refresh: Arc<Mutex<Option<Vec<projects::Project>>>>,
    diagnostics: Arc<DiagnosticsTelemetry>,
) -> mpsc::Sender<Vec<PathBuf>> {
    let (roots_tx, roots_rx) = mpsc::channel::<Vec<PathBuf>>();
    thread::spawn(move || {
        let (event_tx, event_rx) = mpsc::channel();
        let watcher_diagnostics = diagnostics.clone();
        let mut watcher =
            match notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                match event {
                    Ok(event) if filesystem_event_requires_refresh(&event) => {
                        let _ = event_tx.send(());
                    }
                    Ok(_) => {}
                    Err(_) => watcher_diagnostics.operational_failure(OperationalDiagnostic::new(
                        OperationalDiagnosticCode::ProjectWatcherWatch,
                    )),
                }
            }) {
                Ok(watcher) => Some(watcher),
                Err(_) => {
                    diagnostics.operational_failure(OperationalDiagnostic::new(
                        OperationalDiagnosticCode::ProjectWatcherInitialize,
                    ));
                    None
                }
            };
        let mut roots = initial_roots;
        let mut watched_roots = Vec::new();
        let mut last_scan = projects::scan_project_roots(&roots);
        let mut fallback_scan_at = Instant::now();
        configure_project_watches(
            watcher.as_mut(),
            &mut watched_roots,
            &roots,
            diagnostics.as_ref(),
        );

        loop {
            let mut roots_changed = false;
            while let Ok(updated) = roots_rx.try_recv() {
                roots = updated;
                roots_changed = true;
            }
            if roots_changed {
                configure_project_watches(
                    watcher.as_mut(),
                    &mut watched_roots,
                    &roots,
                    diagnostics.as_ref(),
                );
            }

            let filesystem_changed = event_rx.recv_timeout(Duration::from_secs(1)).is_ok();
            if filesystem_changed {
                while event_rx.recv_timeout(Duration::from_millis(250)).is_ok() {}
            }
            let fallback_due = fallback_scan_at.elapsed() >= Duration::from_secs(10);
            if !roots_changed && !filesystem_changed && !fallback_due {
                continue;
            }
            fallback_scan_at = Instant::now();
            let refreshed = projects::scan_project_roots(&roots);
            if refreshed == last_scan {
                continue;
            }
            last_scan = refreshed.clone();
            *pending_project_refresh
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(refreshed);
            if weak
                .upgrade_in_event_loop(|ui| ui.invoke_apply_project_refresh())
                .is_err()
            {
                diagnostics.operational_failure(OperationalDiagnostic::new(
                    OperationalDiagnosticCode::WindowUiDispatch,
                ));
                return;
            }
        }
    });
    roots_tx
}

fn configure_project_watches(
    watcher: Option<&mut notify::RecommendedWatcher>,
    watched_roots: &mut Vec<PathBuf>,
    roots: &[PathBuf],
    telemetry: &dyn Telemetry,
) {
    let Some(watcher) = watcher else {
        watched_roots.clear();
        return;
    };
    for root in watched_roots.drain(..) {
        let _ = watcher.unwatch(&root);
    }
    for root in roots {
        if root.is_dir() {
            if watcher.watch(root, RecursiveMode::NonRecursive).is_ok() {
                watched_roots.push(root.clone());
            } else {
                telemetry.operational_failure(OperationalDiagnostic::new(
                    OperationalDiagnosticCode::ProjectWatcherWatch,
                ));
            }
        }
    }
}

fn filesystem_event_requires_refresh(event: &notify::Event) -> bool {
    !matches!(event.kind, notify::EventKind::Access(_))
}

#[cfg(test)]
mod watcher_tests {
    use notify::event::{AccessKind, AccessMode, CreateKind};

    use super::*;

    #[test]
    fn file_access_events_do_not_trigger_catalog_rescans() {
        let event =
            notify::Event::new(notify::EventKind::Access(AccessKind::Open(AccessMode::Any)));

        assert!(!filesystem_event_requires_refresh(&event));
    }

    #[test]
    fn filesystem_changes_still_trigger_catalog_rescans() {
        let event = notify::Event::new(notify::EventKind::Create(CreateKind::Folder));

        assert!(filesystem_event_requires_refresh(&event));
    }
}
