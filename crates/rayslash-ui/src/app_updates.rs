use std::{
    cell::RefCell,
    env,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use rayslash_core::{
    config,
    diagnostics::{OperationalDiagnostic, OperationalDiagnosticCode, Telemetry},
    updates::{self, AppRelease, UpdateError},
};
use slint::ComponentHandle;

use crate::{AppWindow, settings_callbacks::set_ephemeral_status};

enum UpdateEvent {
    Checked(AppRelease),
    CheckFailed(String),
    Installed,
    InstallFailed(String),
}

pub(crate) struct AppUpdateContext {
    pub config_state: Rc<RefCell<config::Config>>,
    pub diagnostics: Arc<dyn Telemetry>,
    pub restart_requested: Arc<AtomicBool>,
}

pub(crate) fn register_app_updates(ui: &AppWindow, context: AppUpdateContext) {
    let current_version = updates::current_version();
    let current_executable = env::current_exe().unwrap_or_else(|_| PathBuf::from("rayslash"));
    let installation = updates::detect_installation(&current_executable);
    let latest = Arc::new(Mutex::new(None::<AppRelease>));
    let pending = Arc::new(Mutex::new(None::<UpdateEvent>));
    let busy = Arc::new(AtomicBool::new(false));

    ui.set_settings_app_version(current_version.to_string().into());
    ui.set_settings_installation_kind(installation.label().into());

    ui.on_apply_app_update_state({
        let weak = ui.as_weak();
        let latest = latest.clone();
        let pending = pending.clone();
        let busy = busy.clone();
        let config_state = context.config_state.clone();
        let restart_requested = context.restart_requested.clone();
        move || {
            let event = pending
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take();
            busy.store(false, Ordering::Release);
            let Some(event) = event else {
                return;
            };
            let Some(ui) = weak.upgrade() else {
                return;
            };
            ui.set_settings_app_update_busy(false);
            match event {
                UpdateEvent::Checked(release) => {
                    let available = release.is_newer_than(&current_version);
                    ui.set_settings_latest_app_version(release.version.to_string().into());
                    ui.set_settings_app_update_available(available);
                    ui.set_settings_app_update_action(
                        if available {
                            "Update now"
                        } else {
                            "Check again"
                        }
                        .into(),
                    );
                    ui.set_settings_app_update_status(
                        if available {
                            format!("Rayslash {} is ready to install.", release.version)
                        } else {
                            "This is the latest available release.".to_owned()
                        }
                        .into(),
                    );
                    *latest.lock().unwrap_or_else(|error| error.into_inner()) =
                        Some(release.clone());
                    if available && config_state.borrow().updates.notify_app_updates {
                        set_ephemeral_status(
                            &ui,
                            &format!("Rayslash {} is available.", release.version),
                        );
                    }
                }
                UpdateEvent::CheckFailed(message) => {
                    ui.set_settings_latest_app_version("Unavailable".into());
                    ui.set_settings_app_update_available(false);
                    ui.set_settings_app_update_action("Try again".into());
                    ui.set_settings_app_update_status(message.into());
                }
                UpdateEvent::Installed => {
                    ui.set_settings_app_update_available(false);
                    ui.set_settings_app_update_action("Installed".into());
                    ui.set_settings_app_update_status(
                        "Update installed. Restarting Rayslash…".into(),
                    );
                    restart_requested.store(true, Ordering::Release);
                    let _ = slint::quit_event_loop();
                }
                UpdateEvent::InstallFailed(message) => {
                    ui.set_settings_app_update_available(true);
                    ui.set_settings_app_update_action("Try update again".into());
                    ui.set_settings_app_update_status(message.clone().into());
                    set_ephemeral_status(&ui, &message);
                }
            }
        }
    });

    ui.on_settings_check_app_update_requested({
        let weak = ui.as_weak();
        let pending = pending.clone();
        let busy = busy.clone();
        let diagnostics = context.diagnostics.clone();
        move || {
            start_update_check(&weak, &pending, &busy, diagnostics.clone());
        }
    });

    ui.on_settings_install_app_update_requested({
        let weak = ui.as_weak();
        let pending = pending.clone();
        let latest = latest.clone();
        let busy = busy.clone();
        let diagnostics = context.diagnostics.clone();
        let current_executable = current_executable.clone();
        move || {
            if busy.swap(true, Ordering::AcqRel) {
                return;
            }
            let release = latest
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            let Some(release) = release else {
                busy.store(false, Ordering::Release);
                start_update_check(&weak, &pending, &busy, diagnostics.clone());
                return;
            };
            if let Some(ui) = weak.upgrade() {
                ui.set_settings_app_update_busy(true);
                ui.set_settings_app_update_status("Downloading and verifying the update…".into());
            }
            let weak = weak.clone();
            let pending = pending.clone();
            let busy = busy.clone();
            let diagnostics = diagnostics.clone();
            let current_executable = current_executable.clone();
            thread::spawn(move || {
                let result = updates::download_verified_asset(&release, installation)
                    .inspect_err(|error| {
                        diagnostics.operational_failure(update_diagnostic(
                            OperationalDiagnosticCode::AppUpdateDownload,
                            error,
                        ));
                    })
                    .and_then(|downloaded| {
                        updates::install_downloaded_update(
                            installation,
                            &downloaded,
                            &current_executable,
                        )
                        .inspect_err(|error| {
                            diagnostics.operational_failure(update_diagnostic(
                                OperationalDiagnosticCode::AppUpdateInstall,
                                error,
                            ));
                        })
                    });
                *pending.lock().unwrap_or_else(|error| error.into_inner()) = Some(match result {
                    Ok(()) => UpdateEvent::Installed,
                    Err(error) => UpdateEvent::InstallFailed(error.to_string()),
                });
                if weak
                    .upgrade_in_event_loop(|ui| ui.invoke_apply_app_update_state())
                    .is_err()
                {
                    busy.store(false, Ordering::Release);
                }
            });
        }
    });

    if cfg!(debug_assertions)
        && env::var_os("RAYSLASH_PREVIEW_UPDATES").as_deref() == Some("1".as_ref())
    {
        ui.set_settings_latest_app_version("9.9.9".into());
        ui.set_settings_app_update_available(true);
        ui.set_settings_app_update_action("Update now".into());
        ui.set_settings_app_update_status("Rayslash 9.9.9 is ready to install.".into());
    } else {
        start_update_check(&ui.as_weak(), &pending, &busy, context.diagnostics);
    }
}

fn start_update_check(
    weak: &slint::Weak<AppWindow>,
    pending: &Arc<Mutex<Option<UpdateEvent>>>,
    busy: &Arc<AtomicBool>,
    diagnostics: Arc<dyn Telemetry>,
) {
    if busy.swap(true, Ordering::AcqRel) {
        return;
    }
    if let Some(ui) = weak.upgrade() {
        ui.set_settings_app_update_busy(true);
        ui.set_settings_app_update_status("Checking GitHub releases…".into());
    }
    let weak = weak.clone();
    let pending = pending.clone();
    let busy = busy.clone();
    thread::spawn(move || {
        let event = match updates::fetch_latest_release() {
            Ok(release) => UpdateEvent::Checked(release),
            Err(error) => {
                diagnostics.operational_failure(update_diagnostic(
                    OperationalDiagnosticCode::AppUpdateCheck,
                    &error,
                ));
                UpdateEvent::CheckFailed(error.to_string())
            }
        };
        *pending.lock().unwrap_or_else(|error| error.into_inner()) = Some(event);
        if weak
            .upgrade_in_event_loop(|ui| ui.invoke_apply_app_update_state())
            .is_err()
        {
            busy.store(false, Ordering::Release);
        }
    });
}

fn update_diagnostic(
    code: OperationalDiagnosticCode,
    error: &UpdateError,
) -> OperationalDiagnostic {
    match error {
        UpdateError::Io { source, .. } => OperationalDiagnostic::from_io(code, source),
        _ => OperationalDiagnostic::new(code),
    }
}
