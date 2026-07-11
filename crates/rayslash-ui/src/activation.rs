use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
};

use rayslash_core::{
    actions, app_state, apps, config, projects, providers::ProviderAction, ranking, search,
};
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
                Some(result) => match result.provider_action() {
                    ProviderAction::None => {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(format!("Preview only: {}", result.title).into());
                        }
                    }
                    ProviderAction::Dismiss => {
                        if let Some(ui) = weak.upgrade() {
                            hide_launcher(&ui, is_visible.as_ref());
                        }
                    }
                    ProviderAction::CopyText(text) => match copy_to_clipboard(&text) {
                        Ok(()) => {
                            if let Some(ui) = weak.upgrade() {
                                ui.set_status_text(format!("Copied result: {text}").into());
                                hide_launcher(&ui, is_visible.as_ref());
                            }
                        }
                        Err(error) => {
                            eprintln!("failed to copy result: {error}");
                            if let Some(ui) = weak.upgrade() {
                                ui.set_status_text(format!("Could not copy result: {text}").into());
                            }
                        }
                    },
                    ProviderAction::ShowMessage(message) => {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_status_text(message.into());
                        }
                    }
                    ProviderAction::RunUtility(action) => {
                        match actions::run_utility_action(&action) {
                            Ok(()) => {
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
                                        format!("Scheduled {}", result.title).into(),
                                    );
                                    hide_launcher(&ui, is_visible.as_ref());
                                }
                            }
                            Err(error) => {
                                eprintln!("failed to run utility action {}: {error}", result.title);
                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        format!("Could not run {}", result.title).into(),
                                    );
                                }
                            }
                        }
                    }
                    ProviderAction::OpenUrl(url) => match actions::open_url(&url) {
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
                    },
                    ProviderAction::OpenDefaultWebSearch(query) => {
                        match actions::open_default_web_search(&query, &apps.borrow()) {
                            Ok(_child) => {
                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(format!("Opening {}", result.title).into());
                                    hide_launcher(&ui, is_visible.as_ref());
                                }
                            }
                            Err(error) => {
                                eprintln!("failed to open default web search {query}: {error}");
                                if let Some(ui) = weak.upgrade() {
                                    ui.set_status_text(
                                        "Could not open browser search. Is a default browser set?"
                                            .into(),
                                    );
                                }
                            }
                        }
                    }
                    ProviderAction::OpenFolder(path) => {
                        let display_path = search::display_path(&path);
                        if use_alternate_opener
                            && config_state
                                .borrow()
                                .actions
                                .alternate_folder_opener_enabled
                        {
                            let opener_command = config_state
                                .borrow()
                                .actions
                                .alternate_folder_opener_command
                                .clone();
                            match actions::open_project_in_editor(&path, &opener_command) {
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
                                                result.title, opener_command
                                            )
                                            .into(),
                                        );
                                        hide_launcher(&ui, is_visible.as_ref());
                                    }
                                }
                                Err(error) => {
                                    eprintln!(
                                        "failed to open folder with `{} {}`: {error}",
                                        opener_command,
                                        path.display()
                                    );
                                    if let Some(ui) = weak.upgrade() {
                                        ui.set_status_text(
                                            format!(
                                                "Could not open {}. Is `{}` on PATH?",
                                                display_path, opener_command
                                            )
                                            .into(),
                                        );
                                    }
                                }
                            }
                        } else {
                            match actions::open_project_folder(&path) {
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
                                            format!("Opening folder {display_path}").into(),
                                        );
                                        hide_launcher(&ui, is_visible.as_ref());
                                    }
                                }
                                Err(error) => {
                                    eprintln!(
                                        "failed to open folder with `xdg-open {}`: {error}",
                                        path.display()
                                    );
                                    if let Some(ui) = weak.upgrade() {
                                        ui.set_status_text(
                                            format!(
                                                "Could not open folder {display_path}. Is `xdg-open` on PATH?"
                                            )
                                            .into(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ProviderAction::LaunchApp {
                        id,
                        command,
                        desktop_file,
                        dbus_activatable,
                        startup_wm_class,
                    } => match actions::activate_app(
                        &id,
                        &result.title,
                        &command,
                        &desktop_file,
                        dbus_activatable,
                        startup_wm_class.as_deref(),
                    ) {
                        Ok(outcome) => {
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
                                let verb = match outcome {
                                    actions::LaunchOutcome::FocusedExisting => "Showing",
                                    actions::LaunchOutcome::Completed
                                    | actions::LaunchOutcome::Spawned(_) => "Launching",
                                };
                                ui.set_status_text(format!("{verb} {}", result.title).into());
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
                    },
                    ProviderAction::LaunchAlias(alias) => match actions::launch_alias(&alias) {
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
                    },
                },
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
        .chain([
            "system-action:Reboot".to_owned(),
            "system-action:Shutdown".to_owned(),
            "system-action:Logout".to_owned(),
            "system-action:Lock".to_owned(),
        ])
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
