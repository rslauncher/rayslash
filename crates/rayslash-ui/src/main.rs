mod activation;
mod cli;
mod ipc;
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
    time::Instant,
};

use activation::register_activation_callback;
use opener_visual::{app_icon_count, set_alternate_opener_visual, to_app_choice_items};
use rayslash_core::{apps, config, projects};
use result_items::{IconImageCache, to_result_items};
use runtime_state::{
    load_runtime_ranking_state, profile_enabled, profile_stage, search_results,
    selected_index_for_query,
};
use settings::set_settings_properties;
use settings_callbacks::{SettingsCallbackContext, register_settings_callbacks};
use slint::{
    ComponentHandle, VecModel,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
use window_state::{
    handle_ipc_request, hide_launcher, should_start_resident_after_send_error, visible_flag,
};

slint::include_modules!();

pub(crate) const DEFAULT_STATUS_TEXT: &str = "";

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
    let config = config::load_config().unwrap_or_else(|error| {
        eprintln!("{error}; using default config");
        config::Config::default()
    });
    let config_state = Rc::new(RefCell::new(config));
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
    profile_stage(
        profile,
        &format!("app discovery ({} apps)", apps.borrow().len()),
        stage_started,
    );

    let stage_started = Instant::now();
    let current_results = Rc::new(RefCell::new(search_results(
        &config_state.borrow(),
        &ranking_state.borrow(),
        &projects.borrow(),
        &apps.borrow(),
        "",
    )));
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

    ui.set_result_count(current_results.borrow().len() as i32);
    ui.set_results(results_model.clone().into());
    ui.set_selected_index(-1);
    ui.set_alternate_folder_opener_enabled(
        config_state
            .borrow()
            .actions
            .alternate_folder_opener_enabled,
    );
    set_settings_properties(
        &ui,
        &config_state.borrow(),
        &socket_path,
        projects.borrow().len(),
        apps.borrow().len(),
        app_icon_count(&apps.borrow()),
        ranking_state.borrow().entries.len(),
    );

    let alternate_opener_choices = Rc::new(VecModel::from(to_app_choice_items(
        &apps.borrow(),
        &mut icon_cache.borrow_mut(),
    )));
    ui.set_alternate_opener_choices(alternate_opener_choices.clone().into());
    set_alternate_opener_visual(
        &ui,
        &config_state
            .borrow()
            .actions
            .alternate_folder_opener_command,
        &apps.borrow(),
        &mut icon_cache.borrow_mut(),
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
        let alternate_opener_choices = alternate_opener_choices.clone();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move || {
            refresh_desktop_apps(&apps, &alternate_opener_choices, &icon_cache);
            let results = search_results(
                &config_state.borrow(),
                &ranking_state.borrow(),
                &projects.borrow(),
                &apps.borrow(),
                "",
            );
            let count = results.len() as i32;

            results_model.set_vec(to_result_items(&results, &mut icon_cache.borrow_mut()));
            *current_results.borrow_mut() = results;

            if let Some(ui) = weak.upgrade() {
                ui.set_query_text("".into());
                ui.set_result_count(count);
                ui.set_selected_index(-1);
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
                ui.set_settings_open(false);
                ui.invoke_reset_result_scroll();
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
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move |query| {
            let stage_started = Instant::now();
            let results = search_results(
                &config_state.borrow(),
                &ranking_state.borrow(),
                &projects.borrow(),
                &apps.borrow(),
                query.as_str(),
            );
            let count = results.len() as i32;

            results_model.set_vec(to_result_items(&results, &mut icon_cache.borrow_mut()));
            *current_results.borrow_mut() = results;
            profile_stage(
                profile,
                &format!("query {:?} ({} results)", query.as_str(), count),
                stage_started,
            );

            if let Some(ui) = weak.upgrade() {
                ui.set_result_count(count);
                ui.set_selected_index(selected_index_for_query(query.as_str(), count));
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
                ui.invoke_reset_result_scroll();
            }
        }
    });

    register_activation_callback(
        &ui,
        current_results.clone(),
        config_state.clone(),
        ranking_state.clone(),
        is_visible.clone(),
    );

    register_settings_callbacks(
        &ui,
        SettingsCallbackContext {
            config_state: config_state.clone(),
            ranking_state: ranking_state.clone(),
            projects: projects.clone(),
            apps: apps.clone(),
            alternate_opener_choices: alternate_opener_choices.clone(),
            current_results: current_results.clone(),
            results_model: results_model.clone(),
            icon_cache: icon_cache.clone(),
            socket_path: socket_path.clone(),
            suppress_next_focus_hide: suppress_next_focus_hide.clone(),
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

fn refresh_desktop_apps(
    apps_state: &Rc<RefCell<Vec<apps::DesktopApp>>>,
    choices_model: &Rc<VecModel<AppChoiceItem>>,
    icon_cache: &Rc<RefCell<IconImageCache>>,
) {
    let discovered_apps = apps::discover_desktop_apps();
    choices_model.set_vec(to_app_choice_items(
        &discovered_apps,
        &mut icon_cache.borrow_mut(),
    ));
    *apps_state.borrow_mut() = discovered_apps;
}
