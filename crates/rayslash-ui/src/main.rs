mod cli;
mod ipc;
mod opener_visual;
mod result_items;
mod settings;

use std::{
    cell::{Cell, RefCell},
    env, io,
    path::PathBuf,
    process::ExitCode,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use opener_visual::{app_icon_count, set_alternate_opener_visual, to_app_choice_items};
use rayslash_core::{actions, apps, config, projects, ranking, search};
use result_items::{IconImageCache, to_result_items};
use settings::{
    first_existing_folder_source, parse_folder_sources_text, parse_max_results,
    set_settings_properties,
};
use slint::{
    ComponentHandle, VecModel,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};

slint::include_modules!();

const DEFAULT_STATUS_TEXT: &str = "";

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

    let is_visible = Arc::new(AtomicBool::new(true));
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
    let apps = Rc::new(apps::discover_desktop_apps());
    profile_stage(
        profile,
        &format!("app discovery ({} apps)", apps.len()),
        stage_started,
    );

    let stage_started = Instant::now();
    let current_results = Rc::new(RefCell::new(search_results(
        &config_state.borrow(),
        &ranking_state.borrow(),
        &projects.borrow(),
        &apps,
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
        apps.len(),
        app_icon_count(&apps),
        ranking_state.borrow().entries.len(),
    );

    let alternate_opener_choices = Rc::new(VecModel::from(to_app_choice_items(
        &apps,
        &mut icon_cache.borrow_mut(),
    )));
    ui.set_alternate_opener_choices(alternate_opener_choices.clone().into());
    set_alternate_opener_visual(
        &ui,
        &config_state
            .borrow()
            .actions
            .alternate_folder_opener_command,
        &apps,
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
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move || {
            let results = search_results(
                &config_state.borrow(),
                &ranking_state.borrow(),
                &projects.borrow(),
                &apps,
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
                &apps,
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

    ui.on_activate_selected_result({
        let weak = ui.as_weak();
        let current_results = current_results.clone();
        let config_state = config_state.clone();
        let ranking_state = ranking_state.clone();
        let is_visible = is_visible.clone();
        move |index, open_in_vscode| {
            let result = usize::try_from(index)
                .ok()
                .and_then(|index| current_results.borrow().get(index).cloned());

            match result {
                Some(result) => {
                    if let Some(calculator_result) = result.calculator_result() {
                        println!("calculator result: {}", calculator_result);

                        match copy_to_clipboard(calculator_result) {
                            Ok(()) => {
                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        format!("Copied result: {}", calculator_result).into(),
                                    );
                                    hide_launcher(&ui, is_visible.as_ref());
                                }
                            }
                            Err(error) => {
                                eprintln!("failed to copy calculator result: {error}");

                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        format!("Could not copy result: {}", calculator_result)
                                            .into(),
                                    );
                                }
                            }
                        }
                    } else if let Some(calculator_error) = result.calculator_error_message() {
                        println!("calculator error: {}", calculator_error);

                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(calculator_error.into());
                        }
                    } else if result.is_no_results() {
                        println!("no results for query");

                        if let Some(ui) = weak.upgrade() {
                            hide_launcher(&ui, is_visible.as_ref());
                        }
                    } else if let Some(path) = result.project_path() {
                        let display_path = search::display_path(path);

                        if open_in_vscode
                            && config_state
                                .borrow()
                                .actions
                                .alternate_folder_opener_enabled
                        {
                            let editor_command = config_state
                                .borrow()
                                .actions
                                .alternate_folder_opener_command
                                .clone();
                            match actions::open_project_in_editor(path, &editor_command) {
                                Ok(_child) => {
                                    println!(
                                        "Opening project with {}: {}",
                                        editor_command,
                                        path.display()
                                    );

                                    if let Some(ui) = weak.upgrade() {
                                        let query = ui.get_query_text();
                                        record_learned_launch(
                                            &config_state.borrow(),
                                            &ranking_state,
                                            &result,
                                            query.as_str(),
                                        );
                                        ui.set_status_text(
                                            format!(
                                                "Opening {} with {}",
                                                result.title, editor_command
                                            )
                                            .into(),
                                        );
                                        hide_launcher(&ui, is_visible.as_ref());
                                    }
                                }
                                Err(error) => {
                                    eprintln!(
                                        "failed to open project with `{} {}`: {error}",
                                        editor_command,
                                        path.display()
                                    );

                                    if let Some(ui) = weak.upgrade() {
                                        ui.set_status_text(
                                            format!(
                                                "Could not open {}. Is `{}` on PATH?",
                                                display_path, editor_command
                                            )
                                            .into(),
                                        );
                                    }
                                }
                            }
                        } else {
                            match actions::open_project_folder(path) {
                                Ok(_child) => {
                                    println!("Opening project folder: {}", path.display());

                                    if let Some(ui) = weak.upgrade() {
                                        let query = ui.get_query_text();
                                        record_learned_launch(
                                            &config_state.borrow(),
                                            &ranking_state,
                                            &result,
                                            query.as_str(),
                                        );
                                        ui.set_status_text(
                                            format!("Opening folder {}", display_path).into(),
                                        );
                                        hide_launcher(&ui, is_visible.as_ref());
                                    }
                                }
                                Err(error) => {
                                    eprintln!(
                                        "failed to open project folder with `xdg-open {}`: {error}",
                                        path.display()
                                    );

                                    if let Some(ui) = weak.upgrade() {
                                        ui.set_status_text(
                                            format!(
                                                "Could not open folder {}. Is `xdg-open` on PATH?",
                                                display_path
                                            )
                                            .into(),
                                        );
                                    }
                                }
                            }
                        }
                    } else if let Some(command) = result.app_command().cloned() {
                        match actions::launch_app(&command) {
                            Ok(_child) => {
                                println!(
                                    "Launching app {} with command: {}",
                                    result.title,
                                    command_display(&command)
                                );

                                if let Some(ui) = weak.upgrade() {
                                    let query = ui.get_query_text();
                                    record_learned_launch(
                                        &config_state.borrow(),
                                        &ranking_state,
                                        &result,
                                        query.as_str(),
                                    );
                                    ui.set_status_text(
                                        format!("Launching {}", result.title).into(),
                                    );
                                    hide_launcher(&ui, is_visible.as_ref());
                                }
                            }
                            Err(error) => {
                                eprintln!(
                                    "failed to launch app {} with command `{}`: {error}",
                                    result.title,
                                    command_display(&command)
                                );

                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        format!(
                                            "Could not launch {}. Is `{}` on PATH?",
                                            result.title,
                                            command.program.to_string_lossy()
                                        )
                                        .into(),
                                    );
                                }
                            }
                        }
                    } else {
                        println!("placeholder activation: {}", result.title);

                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(format!("Preview only: {}", result.title).into());
                        }
                    }
                }
                None => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_text("No result selected.".into());
                    }
                }
            }
        }
    });

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
        let suppress_next_focus_hide = suppress_next_focus_hide.clone();
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
        let socket_path = socket_path.clone();
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

fn handle_ipc_request(ui: &AppWindow, is_visible: &AtomicBool, request: ipc::IpcRequest) {
    match request {
        ipc::IpcRequest::Show => show_launcher(ui, is_visible),
        ipc::IpcRequest::Toggle if is_visible.load(Ordering::SeqCst) => {
            hide_launcher(ui, is_visible);
        }
        ipc::IpcRequest::Toggle => show_launcher(ui, is_visible),
    }
}

fn show_launcher(ui: &AppWindow, is_visible: &AtomicBool) {
    ui.invoke_reset_requested();
    ui.set_control_held(false);

    match ui.show() {
        Ok(()) => {
            is_visible.store(true, Ordering::SeqCst);
            ui.invoke_focus_search();
        }
        Err(error) => eprintln!("failed to show rayslash window: {error}"),
    }
}

fn hide_launcher(ui: &AppWindow, is_visible: &AtomicBool) {
    ui.set_control_held(false);

    if let Err(error) = ui.hide() {
        eprintln!("failed to hide rayslash window: {error}");
    } else {
        is_visible.store(false, Ordering::SeqCst);
    }
}

fn should_start_resident_after_send_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
    )
}

fn profile_enabled() -> bool {
    env::var_os("RAYSLASH_PROFILE").is_some_and(|value| value != "0")
}

fn profile_stage(enabled: bool, label: &str, started: Instant) {
    if enabled {
        eprintln!("[rayslash profile] {label}: {:.2?}", started.elapsed());
    }
}

fn load_runtime_ranking_state() -> ranking::RankingState {
    match ranking::load_ranking_state() {
        Ok(state) => state,
        Err(error) => {
            eprintln!("{error}; using empty ranking state");
            ranking::RankingState::default()
        }
    }
}

fn search_results(
    config: &config::Config,
    ranking_state: &ranking::RankingState,
    projects: &[projects::Project],
    apps: &[apps::DesktopApp],
    query: &str,
) -> Vec<search::SearchResult> {
    let ranking = config.ranking.learn_from_usage.then_some(ranking_state);
    let mut results =
        search::mixed_results_with_ranking(projects, apps, query, &config.providers, ranking);
    results.truncate(config.appearance.max_results);
    results
}

fn record_learned_launch(
    config: &config::Config,
    ranking_state: &Rc<RefCell<ranking::RankingState>>,
    result: &search::SearchResult,
    query: &str,
) {
    if !config.ranking.learn_from_usage {
        return;
    }

    let Some(result_id) = result.learning_id() else {
        return;
    };

    {
        let mut state = ranking_state.borrow_mut();
        state.record_launch(&result_id, query);
    }

    if let Err(error) = ranking::save_ranking_state(&ranking_state.borrow()) {
        eprintln!("{error}");
    }
}

fn command_display(command: &actions::CommandSpec) -> String {
    std::iter::once(command.program.to_string_lossy().into_owned())
        .chain(
            command
                .args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned()),
        )
        .collect::<Vec<_>>()
        .join(" ")
}

fn copy_to_clipboard(text: &str) -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(text.to_owned())
}

fn selected_index_for_query(query: &str, result_count: i32) -> i32 {
    if query.trim().is_empty() || result_count <= 0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
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
            providers: config::ProviderConfig::default(),
            actions: config::ActionConfig::default(),
            appearance: config::AppearanceConfig { max_results: 1 },
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
