use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use slint::winit_030::{CustomApplicationHandler, EventResult, winit};

/// Keep Slint's native windows pending while the resident launcher is hidden.
/// A Wayland top-level can appear in the taskbar even before its first frame.
pub(crate) struct NativeWindowHandler {
    is_visible: Arc<AtomicBool>,
}

impl NativeWindowHandler {
    pub(crate) fn new(is_visible: Arc<AtomicBool>) -> Self {
        Self { is_visible }
    }
}

impl CustomApplicationHandler for NativeWindowHandler {
    fn resumed(&mut self, _: &winit::event_loop::ActiveEventLoop) -> EventResult {
        if self.is_visible.load(Ordering::Acquire) {
            EventResult::Propagate
        } else {
            EventResult::PreventDefault
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) -> EventResult {
        if self.is_visible.load(Ordering::Acquire) {
            return EventResult::Propagate;
        }

        // Slint normally schedules its timers after creating pending windows.
        // Preserve timer wakeups while skipping that native window creation.
        if let Some(duration) = slint::platform::duration_until_next_timer_update() {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(duration));
        }
        EventResult::PreventDefault
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppWindow;
    use slint::{
        ComponentHandle, Timer,
        winit_030::{SlintEvent, WinitWindowAccessor},
    };
    use std::time::Duration;
    use winit::platform::wayland::EventLoopBuilderExtWayland;

    #[test]
    #[ignore = "requires a running Wayland compositor"]
    fn background_has_no_native_window_and_timers_still_show_hide_and_show() {
        let visible = Arc::new(AtomicBool::new(false));
        let mut builder = winit::event_loop::EventLoop::<SlintEvent>::with_user_event();
        builder.with_any_thread(true).with_wayland();
        slint::BackendSelector::new()
            .with_winit_event_loop_builder(builder)
            .with_winit_custom_application_handler(NativeWindowHandler::new(visible.clone()))
            .select()
            .unwrap();
        slint::set_xdg_app_id("dev.rayan6ms.rayslash.background-test").unwrap();
        let ui = AppWindow::new().unwrap();
        ui.invoke_prepare_background_window();
        let weak = ui.as_weak();
        Timer::single_shot(Duration::from_millis(100), move || {
            let ui = weak.unwrap();
            assert!(!ui.window().has_winit_window());
            ui.show().unwrap();
            visible.store(true, Ordering::Release);
            let weak = ui.as_weak();
            Timer::single_shot(Duration::from_millis(100), move || {
                let ui = weak.unwrap();
                assert!(ui.window().has_winit_window());
                let size = ui.window().size().to_logical(ui.window().scale_factor());
                assert_eq!((size.width, size.height), (660.0, 440.0));
                ui.hide().unwrap();
                visible.store(false, Ordering::Release);
                assert!(!ui.window().has_winit_window());
                let weak = ui.as_weak();
                Timer::single_shot(Duration::from_millis(100), move || {
                    let ui = weak.unwrap();
                    assert!(!ui.window().has_winit_window());
                    ui.show().unwrap();
                    visible.store(true, Ordering::Release);
                    let weak = ui.as_weak();
                    Timer::single_shot(Duration::from_millis(100), move || {
                        let ui = weak.unwrap();
                        assert!(ui.window().has_winit_window());
                        ui.hide().unwrap();
                        visible.store(false, Ordering::Release);
                        slint::quit_event_loop().unwrap();
                    });
                });
            });
        });
        slint::run_event_loop_until_quit().unwrap();
    }
}
