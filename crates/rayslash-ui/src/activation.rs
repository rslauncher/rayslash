use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
};

use rayslash_core::{actions, config, ranking, search};
use slint::ComponentHandle;

use crate::{AppWindow, window_state::hide_launcher};

pub(crate) fn register_activation_callback(
    ui: &AppWindow,
    current_results: Rc<RefCell<Vec<search::SearchResult>>>,
    config_state: Rc<RefCell<config::Config>>,
    ranking_state: Rc<RefCell<ranking::RankingState>>,
    is_visible: Arc<AtomicBool>,
) {
    ui.on_activate_selected_result({
        let weak = ui.as_weak();
        move |index, use_alternate_opener| {
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
