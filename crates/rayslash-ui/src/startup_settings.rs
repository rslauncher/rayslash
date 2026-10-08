use rayslash_core::autostart;
use slint::ComponentHandle;

use crate::{AppWindow, settings_callbacks::set_ephemeral_status};

pub(crate) fn register_callbacks(ui: &AppWindow) {
    ui.on_settings_startup_refresh_requested({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                update(&ui, None);
            }
        }
    });
    ui.on_settings_start_at_login_requested({
        let weak = ui.as_weak();
        move |enabled| {
            if let Some(ui) = weak.upgrade() {
                update(&ui, Some(enabled));
            }
        }
    });
    ui.invoke_settings_startup_refresh_requested();
}

fn update(ui: &AppWindow, requested: Option<bool>) {
    if ui.get_settings_startup_busy() {
        return;
    }
    ui.set_settings_startup_busy(true);
    let previous = requested.map_or_else(|| ui.get_settings_start_at_login(), |enabled| !enabled);
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let result = if let Some(enabled) = requested {
            autostart::set_enabled(enabled).map(|()| enabled)
        } else {
            autostart::enabled()
        };
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.set_settings_startup_busy(false);
            match result {
                Ok(enabled) => {
                    ui.set_settings_start_at_login(enabled);
                    ui.set_settings_startup_error("".into());
                    if requested.is_some() {
                        let message = if enabled {
                            "Start at login enabled."
                        } else {
                            "Start at login disabled."
                        };
                        set_ephemeral_status(&ui, message);
                    }
                }
                Err(error) => {
                    ui.set_settings_start_at_login(previous);
                    let action = if requested.is_some() { "change" } else { "read" };
                    let message = format!(
                        "Could not {action} login startup: {error}. Try again or check your desktop's Startup Applications settings."
                    );
                    ui.set_settings_startup_error(message.clone().into());
                    if requested.is_some() {
                        ui.set_status_text(message.into());
                    }
                }
            }
        });
    });
}
