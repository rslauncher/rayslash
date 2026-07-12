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
            let Some(result) = result else {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text("No result selected.".into());
                }
                return;
            };

            match result.provider_action() {
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
                ProviderAction::OpenFolder(path) => {
                    let alternate = use_alternate_opener
                        && config_state
                            .borrow()
                            .actions
                            .alternate_folder_opener_enabled;
                    let outcome = if alternate {
                        actions::open_project_in_editor(
                            &path,
                            &config_state
                                .borrow()
                                .actions
                                .alternate_folder_opener_command,
                        )
                    } else {
                        actions::open_project_folder(&path)
                    };
                    finish_launch(
                        &weak,
                        outcome.map(|_| ()),
                        &result,
                        LaunchState {
                            config: &config_state,
                            ranking: &ranking_state,
                            projects: &projects,
                            apps: &apps,
                            visible: &is_visible,
                        },
                    );
                }
                ProviderAction::LaunchApp {
                    id,
                    command,
                    desktop_file,
                    dbus_activatable,
                    startup_wm_class,
                } => {
                    let outcome = actions::activate_app(
                        &id,
                        &result.title,
                        &command,
                        &desktop_file,
                        dbus_activatable,
                        startup_wm_class.as_deref(),
                    )
                    .map(|_| ());
                    if outcome.is_ok() {
                        mark_app_selected(&app_install_state, &result);
                    }
                    finish_launch(
                        &weak,
                        outcome,
                        &result,
                        LaunchState {
                            config: &config_state,
                            ranking: &ranking_state,
                            projects: &projects,
                            apps: &apps,
                            visible: &is_visible,
                        },
                    );
                }
                ProviderAction::Module(action) => {
                    activate_module(&weak, &result, action, &is_visible)
                }
            }
        }
    });
}

fn activate_module(
    weak: &slint::Weak<AppWindow>,
    result: &search::SearchResult,
    action: search::ModuleAction,
    is_visible: &Arc<AtomicBool>,
) {
    match &action {
        search::ModuleAction::CopyText(text) => match copy_to_clipboard(text) {
            Ok(()) => {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text(format!("Copied result: {text}").into());
                    hide_launcher(&ui, is_visible.as_ref());
                }
            }
            Err(error) => {
                eprintln!("failed to copy module result: {error}");
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text("Could not copy module result.".into());
                }
            }
        },
        search::ModuleAction::ShowMessage(message) => {
            if let Some(ui) = weak.upgrade() {
                ui.set_status_text(message.clone().into());
            }
        }
        search::ModuleAction::None => {
            if let Some(ui) = weak.upgrade() {
                ui.set_status_text(format!("Preview only: {}", result.title).into());
            }
        }
        _ => match actions::run_module_action(&action) {
            Ok(()) => {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text(format!("Activated {}", result.title).into());
                    hide_launcher(&ui, is_visible.as_ref());
                }
            }
            Err(error) => {
                eprintln!("failed to activate module result {}: {error}", result.title);
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_text(format!("Could not activate {}", result.title).into());
                }
            }
        },
    }
}

struct LaunchState<'a> {
    config: &'a Rc<RefCell<config::Config>>,
    ranking: &'a Rc<RefCell<ranking::RankingState>>,
    projects: &'a Rc<RefCell<Vec<projects::Project>>>,
    apps: &'a Rc<RefCell<Vec<apps::DesktopApp>>>,
    visible: &'a Arc<AtomicBool>,
}

fn finish_launch(
    weak: &slint::Weak<AppWindow>,
    outcome: std::io::Result<()>,
    result: &search::SearchResult,
    state: LaunchState<'_>,
) {
    match outcome {
        Ok(()) => {
            if let Some(ui) = weak.upgrade() {
                record_learned_launch(
                    &state.config.borrow(),
                    state.ranking,
                    &state.projects.borrow(),
                    &state.apps.borrow(),
                    result,
                    ui.get_query_text().as_str(),
                );
                ui.set_status_text(format!("Opening {}", result.title).into());
                hide_launcher(&ui, state.visible.as_ref());
            }
        }
        Err(error) => {
            eprintln!("failed to activate {}: {error}", result.title);
            if let Some(ui) = weak.upgrade() {
                ui.set_status_text(format!("Could not open {}", result.title).into());
            }
        }
    }
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

fn copy_to_clipboard(text: &str) -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(text.to_owned())
}
