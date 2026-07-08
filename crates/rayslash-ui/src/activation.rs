use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
};

use rayslash_core::{actions, app_state, apps, config, projects, ranking, search};
use slint::ComponentHandle;

use crate::{AppWindow, window_state::hide_launcher};

pub(crate) struct ActivationCallbackContext {
    pub current_results: Rc<RefCell<Vec<search::SearchResult>>>,
    pub config_state: Rc<RefCell<config::Config>>,
    pub app_install_state: Rc<RefCell<app_state::AppInstallState>>,
    pub ranking_state: Rc<RefCell<ranking::RankingState>>,
    pub projects: Rc<RefCell<Vec<projects::Project>>>,
    pub apps: Rc<RefCell<Vec<apps::DesktopApp>>>,
    pub is_visible: Arc<AtomicBool>,
}

pub(crate) fn register_activation_callback(ui: &AppWindow, context: ActivationCallbackContext) {
    let ActivationCallbackContext {
        current_results,
        config_state,
        app_install_state,
        ranking_state,
        projects,
        apps,
        is_visible,
    } = context;

    ui.on_activate_selected_result({
        let weak = ui.as_weak();
        move |index, use_alternate_opener| {
            let result = usize::try_from(index)
                .ok()
                .and_then(|index| current_results.borrow().get(index).cloned());

            match result {
                Some(result) => {
                    if let Some(copyable_result) = result
                        .calculator_result()
                        .or_else(|| result.unit_conversion_result())
                        .or_else(|| result.currency_conversion_result())
                        .or_else(|| result.time_lookup_result())
                    {
                        match copy_to_clipboard(copyable_result) {
                            Ok(()) => {
                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        format!("Copied result: {}", copyable_result).into(),
                                    );
                                    hide_launcher(&ui, is_visible.as_ref());
                                }
                            }
                            Err(error) => {
                                eprintln!("failed to copy result: {error}");

                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        format!("Could not copy result: {}", copyable_result)
                                            .into(),
                                    );
                                }
                            }
                        }
                    } else if let Some(calculator_error) = result.calculator_error_message() {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(calculator_error.into());
                        }
                    } else if let Some(currency_error) = result.currency_error_message() {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(currency_error.into());
                        }
                    } else if let Some(time_lookup_error) = result.time_lookup_error_message() {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(time_lookup_error.into());
                        }
                    } else if result.is_no_results() {
                        if let Some(ui) = weak.upgrade() {
                            hide_launcher(&ui, is_visible.as_ref());
                        }
                    } else if let Some(url) = result.web_search_url() {
                        match actions::open_url(url) {
                            Ok(_child) => {
                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(format!("Opening {}", result.title).into());
                                    hide_launcher(&ui, is_visible.as_ref());
                                }
                            }
                            Err(error) => {
                                eprintln!("failed to open web search {}: {error}", result.title);

                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        "Could not open web search. Is `xdg-open` on PATH?".into(),
                                    );
                                }
                            }
                        }
                    } else if let Some(path) = result.project_path() {
                        let display_path = search::display_path(path);

                        if use_alternate_opener
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
                                    if let Some(ui) = weak.upgrade() {
                                        let query = ui.get_query_text();
                                        record_learned_launch(
                                            &config_state.borrow(),
                                            &ranking_state,
                                            &projects.borrow(),
                                            &apps.borrow(),
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
                                    if let Some(ui) = weak.upgrade() {
                                        let query = ui.get_query_text();
                                        record_learned_launch(
                                            &config_state.borrow(),
                                            &ranking_state,
                                            &projects.borrow(),
                                            &apps.borrow(),
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
                                if let Some(ui) = weak.upgrade() {
                                    let query = ui.get_query_text();
                                    record_learned_launch(
                                        &config_state.borrow(),
                                        &ranking_state,
                                        &projects.borrow(),
                                        &apps.borrow(),
                                        &result,
                                        query.as_str(),
                                    );
                                    mark_app_selected(&app_install_state, &result);
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
                    } else if let Some(alias) = result.alias().cloned() {
                        match actions::launch_alias(&alias) {
                            Ok(_child) => {
                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(format!("Opening {}", result.title).into());
                                    hide_launcher(&ui, is_visible.as_ref());
                                }
                            }
                            Err(error) => {
                                eprintln!("failed to launch alias {}: {error}", result.title);

                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        format!("Could not open alias {}", result.title).into(),
                                    );
                                }
                            }
                        }
                    } else if let Some(ui) = weak.upgrade() {
                        ui.set_status_text(format!("Preview only: {}", result.title).into());
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
}

fn mark_app_selected(
    app_install_state: &Rc<RefCell<app_state::AppInstallState>>,
    result: &search::SearchResult,
) {
    let Some(app_id) = result.app_id() else {
        return;
    };

    let changed = app_install_state.borrow_mut().mark_app_selected(app_id);
    if changed && let Err(error) = app_state::save_app_state(&app_install_state.borrow()) {
        eprintln!("{error}");
    }
}

fn record_learned_launch(
    config: &config::Config,
    ranking_state: &Rc<RefCell<ranking::RankingState>>,
    projects: &[projects::Project],
    apps: &[apps::DesktopApp],
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
        state.prune(
            active_learning_ids(projects, apps),
            std::time::SystemTime::now(),
        );
    }

    if let Err(error) = ranking::save_ranking_state(&ranking_state.borrow()) {
        eprintln!("{error}");
    }
}

fn active_learning_ids(projects: &[projects::Project], apps: &[apps::DesktopApp]) -> Vec<String> {
    apps.iter()
        .map(|app| format!("app:{}", app.id))
        .chain(
            projects
                .iter()
                .map(|project| format!("folder:{}", project.path.display())),
        )
        .collect()
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
