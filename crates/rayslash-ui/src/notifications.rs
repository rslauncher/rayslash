use crate::AppWindow;

pub(crate) fn register(ui: &AppWindow) {
    ui.on_notification_kind(|message| feedback_kind(message.as_str()).into());
    ui.on_notification_duration_ms(|message| feedback_duration_ms(message.as_str()));
}

pub(crate) fn show_notification(ui: &AppWindow, message: &str) {
    ui.invoke_show_notification(message.into());
}

fn feedback_duration_ms(message: &str) -> i32 {
    let characters = message.chars().count() as i32;
    (1_200 + (characters * 1_000 + 17) / 18).clamp(4_200, 10_000)
}

fn feedback_kind(message: &str) -> &'static str {
    let message = message.to_ascii_lowercase();

    if [
        "could not",
        "failed",
        "cannot",
        "must be",
        "required",
        "invalid",
        "unknown",
        "unavailable",
        "read-only",
    ]
    .iter()
    .any(|needle| message.contains(needle))
    {
        "error"
    } else if [
        "installing",
        "restoring",
        "updating",
        "removing",
        "repairing",
        "confirm",
        "new capabilities",
    ]
    .iter()
    .any(|needle| message.contains(needle))
    {
        "warning"
    } else if [
        "saved",
        "completed",
        "enabled",
        "disabled",
        "installed",
        "restored",
        "updated",
        "removed",
        "cleared",
        "selected",
        "copied",
        "cancelled",
    ]
    .iter()
    .any(|needle| message.contains(needle))
    {
        "success"
    } else {
        "info"
    }
}

#[cfg(test)]
mod tests {
    use super::{feedback_duration_ms, feedback_kind};

    #[test]
    fn notifications_wait_while_hidden_and_repeated_messages_restart_dismissal() {
        std::thread::spawn(|| {
            use slint::platform::{
                Platform, WindowAdapter,
                software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
            };
            use std::{cell::Cell, rc::Rc, time::Duration};
            struct TestPlatform(Rc<Cell<Duration>>);
            impl Platform for TestPlatform {
                fn create_window_adapter(
                    &self,
                ) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                    Ok(MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer))
                }
                fn duration_since_start(&self) -> Duration {
                    self.0.get()
                }
            }
            let clock = Rc::new(Cell::new(Duration::ZERO));
            slint::platform::set_platform(Box::new(TestPlatform(clock.clone()))).unwrap();
            let ui = crate::AppWindow::new().unwrap();
            super::register(&ui);
            let tick = |milliseconds| {
                clock.set(clock.get() + Duration::from_millis(milliseconds));
                slint::platform::update_timers_and_animations();
            };
            ui.invoke_show_notification("Copied result.".into());
            tick(12_000);
            assert_eq!(ui.get_notification_text(), "Copied result.");
            ui.set_launcher_visible(true);
            tick(0);
            tick(3_000);
            ui.invoke_show_notification("Copied result.".into());
            tick(3_000);
            assert_eq!(ui.get_notification_text(), "Copied result.");
            tick(1_300);
            assert!(ui.get_notification_text().is_empty());
            ui.invoke_show_notification("Saved.".into());
            ui.set_settings_open(true);
            tick(100);
            assert_eq!(ui.get_notification_text(), "Saved.");
            ui.set_launcher_visible(false);
            tick(0);
            tick(12_000);
            assert_eq!(ui.get_notification_text(), "Saved.");
            ui.set_launcher_visible(true);
            tick(0);
            tick(4_300);
            assert!(ui.get_notification_text().is_empty());
        })
        .join()
        .unwrap();
    }

    #[test]
    fn feedback_kind_distinguishes_status_intent() {
        assert_eq!(feedback_kind("Calculator enabled."), "success");
        assert_eq!(feedback_kind("Restoring Aliases…"), "warning");
        assert_eq!(feedback_kind("Could not save settings."), "error");
        assert_eq!(feedback_kind("No changes to apply."), "info");
    }

    #[test]
    fn feedback_duration_scales_from_a_readable_minimum() {
        assert_eq!(feedback_duration_ms("Saved."), 4_200);
        assert!(feedback_duration_ms(&"warning ".repeat(15)) > 4_200);
        assert_eq!(feedback_duration_ms(&"very long ".repeat(100)), 10_000);
    }
}
