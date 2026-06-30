mod cli;
mod ipc;

use std::{
    cell::RefCell,
    collections::HashMap,
    env, io,
    path::PathBuf,
    process::ExitCode,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use rayslash_core::{actions, apps, config, projects, search};
use slint::{Image, VecModel};

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

    let result = run_gui(listener);
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

fn run_gui(listener: std::os::unix::net::UnixListener) -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    slint::set_xdg_app_id(rayslash_core::APP_ID)?;

    let is_visible = Arc::new(AtomicBool::new(true));

    let config = config::load_config().unwrap_or_else(|error| {
        eprintln!("{error}; using default config");
        config::Config::default()
    });
    let projects = Rc::new(projects::scan_project_roots(&config.project_roots));
    let apps = Rc::new(apps::discover_desktop_apps());
    let current_results = Rc::new(RefCell::new(search::mixed_results(&projects, &apps, "")));
    let icon_cache = Rc::new(RefCell::new(HashMap::new()));
    let results_model = Rc::new(VecModel::from(to_result_items(
        &current_results.borrow(),
        &mut icon_cache.borrow_mut(),
    )));

    ui.set_result_count(current_results.borrow().len() as i32);
    ui.set_results(results_model.clone().into());
    ui.set_selected_index(-1);
    ui.invoke_focus_search();

    ui.on_reset_requested({
        let weak = ui.as_weak();
        let projects = projects.clone();
        let apps = apps.clone();
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move || {
            let results = search::mixed_results(&projects, &apps, "");
            let count = results.len() as i32;

            results_model.set_vec(to_result_items(&results, &mut icon_cache.borrow_mut()));
            *current_results.borrow_mut() = results;

            if let Some(ui) = weak.upgrade() {
                ui.set_query_text("".into());
                ui.set_result_count(count);
                ui.set_selected_index(-1);
                ui.set_status_text(DEFAULT_STATUS_TEXT.into());
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
        let current_results = current_results.clone();
        let results_model = results_model.clone();
        let icon_cache = icon_cache.clone();
        move |query| {
            let results = search::mixed_results(&projects, &apps, query.as_str());
            let count = results.len() as i32;

            results_model.set_vec(to_result_items(&results, &mut icon_cache.borrow_mut()));
            *current_results.borrow_mut() = results;

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
        let is_visible = is_visible.clone();
        move |index, open_in_vscode| {
            let result = usize::try_from(index)
                .ok()
                .and_then(|index| current_results.borrow().get(index).cloned());

            match result {
                Some(result) => {
                    if let Some(calculator_result) = result.calculator_result() {
                        println!("calculator result: {}", calculator_result);

                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(format!("Result: {}", calculator_result).into());
                        }
                    } else if let Some(path) = result.project_path() {
                        let display_path = search::display_path(path);

                        if open_in_vscode {
                            match actions::open_project_in_vscode(path) {
                                Ok(_child) => {
                                    println!("Opening project in VS Code: {}", path.display());

                                    if let Some(ui) = weak.upgrade() {
                                        ui.set_status_text(
                                            format!("Opening {} in VS Code", result.title).into(),
                                        );
                                        hide_launcher(&ui, is_visible.as_ref());
                                    }
                                }
                                Err(error) => {
                                    eprintln!(
                                        "failed to open project in VS Code with `code {}`: {error}",
                                        path.display()
                                    );

                                    if let Some(ui) = weak.upgrade() {
                                        ui.set_status_text(
                                            format!(
                                                "Could not open {} in VS Code. Is `code` on PATH?",
                                                display_path
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

    match ui.show() {
        Ok(()) => {
            is_visible.store(true, Ordering::SeqCst);
            ui.invoke_focus_search();
        }
        Err(error) => eprintln!("failed to show rayslash window: {error}"),
    }
}

fn hide_launcher(ui: &AppWindow, is_visible: &AtomicBool) {
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

fn selected_index_for_query(query: &str, result_count: i32) -> i32 {
    if query.trim().is_empty() || result_count <= 0 {
        -1
    } else {
        0
    }
}

type IconImageCache = HashMap<PathBuf, Option<Image>>;

fn to_result_items(
    results: &[search::SearchResult],
    icon_cache: &mut IconImageCache,
) -> Vec<ResultItem> {
    results
        .iter()
        .map(|result| {
            let icon = result_icon(&result.icon, icon_cache);

            ResultItem {
                title: result.title.clone().into(),
                subtitle: result.subtitle.clone().into(),
                icon: icon.image,
                has_icon: icon.has_image,
                icon_kind: icon.kind.into(),
                icon_text: icon.text.into(),
            }
        })
        .collect()
}

struct RowIcon {
    image: Image,
    has_image: bool,
    kind: &'static str,
    text: &'static str,
}

fn result_icon(icon: &search::SearchResultIcon, icon_cache: &mut IconImageCache) -> RowIcon {
    match icon {
        search::SearchResultIcon::App { path: Some(path) } => {
            if let Some(image) = load_icon_image(path, icon_cache) {
                RowIcon {
                    image,
                    has_image: true,
                    kind: "app",
                    text: "",
                }
            } else {
                fallback_icon("app", "")
            }
        }
        search::SearchResultIcon::App { path: None } => fallback_icon("app", ""),
        search::SearchResultIcon::Calculator => fallback_icon("calculator", ""),
        search::SearchResultIcon::ProjectFolder => fallback_icon("folder", ""),
        search::SearchResultIcon::Placeholder => fallback_icon("placeholder", ""),
    }
}

fn fallback_icon(kind: &'static str, text: &'static str) -> RowIcon {
    RowIcon {
        image: Image::default(),
        has_image: false,
        kind,
        text,
    }
}

fn load_icon_image(path: &PathBuf, icon_cache: &mut IconImageCache) -> Option<Image> {
    if let Some(cached) = icon_cache.get(path) {
        return cached.clone();
    }

    let image = Image::load_from_path(path).ok();
    icon_cache.insert(path.clone(), image.clone());
    image
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
}
