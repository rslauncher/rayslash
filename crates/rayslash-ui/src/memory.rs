use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use slint::{ComponentHandle, Model, Timer, VecModel};

use crate::{
    AppChoiceItem, AppWindow, ResultItem, result_items::IconImageCache,
    runtime_state::profile_stage,
};

const HIDDEN_IDLE_DELAY: Duration = Duration::from_secs(30);

pub(crate) fn register(
    ui: &AppWindow,
    is_visible: Arc<AtomicBool>,
    results: Rc<VecModel<ResultItem>>,
    choices: Rc<VecModel<AppChoiceItem>>,
    cache: Rc<RefCell<IconImageCache>>,
    profile: bool,
) {
    register_with_delay(
        ui,
        is_visible,
        results,
        choices,
        cache,
        profile,
        HIDDEN_IDLE_DELAY,
    );
}

fn register_with_delay(
    ui: &AppWindow,
    is_visible: Arc<AtomicBool>,
    results: Rc<VecModel<ResultItem>>,
    choices: Rc<VecModel<AppChoiceItem>>,
    cache: Rc<RefCell<IconImageCache>>,
    profile: bool,
    delay: Duration,
) {
    let timer = Rc::new(Timer::default());
    ui.on_launcher_visibility_changed({
        let weak = ui.as_weak();
        let timer = timer.clone();
        move || {
            timer.stop();
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_launcher_visible() {
                return;
            }
            let weak = ui.as_weak();
            let is_visible = is_visible.clone();
            let results = results.clone();
            let choices = choices.clone();
            let cache = cache.clone();
            timer.start(slint::TimerMode::SingleShot, delay, move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                // Visibility transitions restart/cancel the one-shot timer;
                // recheck both flags before touching models or the allocator.
                if ui.get_launcher_visible() || is_visible.load(Ordering::Acquire) {
                    return;
                }
                let started = Instant::now();
                release_images(&ui, &results, &choices, &cache);
                trim_allocator();
                profile_stage(
                    profile,
                    "hidden idle image release and allocator trim",
                    started,
                );
            });
        }
    });
    // Background startup is hidden before a visibility property ever changes.
    ui.invoke_launcher_visibility_changed();
}

fn release_images(
    ui: &AppWindow,
    results: &VecModel<ResultItem>,
    choices: &VecModel<AppChoiceItem>,
    cache: &RefCell<IconImageCache>,
) {
    cache.borrow_mut().clear();
    for index in 0..results.row_count() {
        if let Some(mut row) = results.row_data(index)
            && row.has_icon
        {
            row.icon = Default::default();
            row.has_icon = false;
            results.set_row_data(index, row);
        }
    }
    // These choices are already constructed lazily on opening Settings.
    choices.set_vec(Vec::new());
    let searches = ui.get_settings_web_searches();
    let mut rows = Vec::with_capacity(searches.row_count());
    for index in 0..searches.row_count() {
        if let Some(mut row) = searches.row_data(index) {
            row.icon = Default::default();
            row.has_icon = false;
            rows.push(row);
        }
    }
    ui.set_settings_web_searches(Rc::new(VecModel::from(rows)).into());
    ui.set_alternate_folder_opener_icon(Default::default());
    ui.set_alternate_folder_opener_has_icon(false);
    // show_launcher resets/rebuilds results and settings before showing the
    // window, so released images are restored before the next visible frame.
}

fn trim_allocator() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> std::ffi::c_int;
        }
        // SAFETY: glibc's thread-safe allocator API only returns unused pages;
        // it neither invalidates live allocations nor changes allocator limits.
        unsafe {
            malloc_trim(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WebSearchItem, native_window::NativeWindowHandler};
    use slint::{
        Image, SharedPixelBuffer,
        winit_030::{SlintEvent, WinitWindowAccessor, winit},
    };
    use std::cell::Cell;
    use winit::platform::wayland::EventLoopBuilderExtWayland;

    #[test]
    #[ignore = "requires a running Wayland compositor"]
    fn hidden_cleanup_preserves_data_and_reopening_cancels_the_old_deadline() {
        let visible = Arc::new(AtomicBool::new(true));
        let mut builder = winit::event_loop::EventLoop::<SlintEvent>::with_user_event();
        builder.with_any_thread(true).with_wayland();
        crate::renderer::configure_backend(slint::BackendSelector::new())
            .with_winit_event_loop_builder(builder)
            .with_winit_custom_application_handler(NativeWindowHandler::new(visible.clone()))
            .select()
            .unwrap();
        let ui = AppWindow::new().unwrap();
        let image = Image::from_rgba8(SharedPixelBuffer::new(16, 16));
        let row = ResultItem {
            title: "Keep result data".into(),
            icon: image.clone(),
            has_icon: true,
            ..Default::default()
        };
        let results = Rc::new(VecModel::from(vec![row.clone()]));
        let choices = Rc::new(VecModel::from(vec![AppChoiceItem {
            name: "Keep picker".into(),
            ..Default::default()
        }]));
        ui.set_results(results.clone().into());
        ui.set_settings_web_searches(
            Rc::new(VecModel::from(vec![WebSearchItem {
                name: "Keep web search".into(),
                url: "https://example.com/?q=%s".into(),
                icon: image,
                has_icon: true,
                ..Default::default()
            }]))
            .into(),
        );
        ui.set_query_text("keep query".into());
        ui.set_settings_aliases_text("keep unsaved text".into());
        register_with_delay(
            &ui,
            visible.clone(),
            results.clone(),
            choices.clone(),
            Rc::default(),
            false,
            Duration::from_millis(40),
        );
        ui.show().unwrap();
        ui.set_launcher_visible(true);
        let timer = Timer::default();
        let stage = Cell::new(0);
        let weak = ui.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(100),
            move || {
                let ui = weak.unwrap();
                match stage.get() {
                    0 => {
                        assert!(results.row_data(0).unwrap().has_icon);
                        ui.hide().unwrap();
                        visible.store(false, Ordering::Release);
                        ui.set_launcher_visible(false);
                    }
                    1 => {
                        assert!(!ui.window().has_winit_window());
                        let result = results.row_data(0).unwrap();
                        assert_eq!(result.title, "Keep result data");
                        assert!(!result.has_icon);
                        assert_eq!(choices.row_count(), 0);
                        let search = ui.get_settings_web_searches().row_data(0).unwrap();
                        assert_eq!(search.name, "Keep web search");
                        assert_eq!(search.url, "https://example.com/?q=%s");
                        assert!(!search.has_icon);
                        assert_eq!(ui.get_query_text(), "keep query");
                        assert_eq!(ui.get_settings_aliases_text(), "keep unsaved text");
                        results.set_row_data(0, row.clone());
                        ui.show().unwrap();
                        visible.store(true, Ordering::Release);
                        ui.set_launcher_visible(true);
                    }
                    2 => {
                        ui.hide().unwrap();
                        visible.store(false, Ordering::Release);
                        ui.set_launcher_visible(false);
                        let weak = ui.as_weak();
                        let visible = visible.clone();
                        Timer::single_shot(Duration::from_millis(15), move || {
                            let ui = weak.unwrap();
                            ui.show().unwrap();
                            visible.store(true, Ordering::Release);
                            ui.set_launcher_visible(true);
                        });
                    }
                    _ => {
                        assert!(visible.load(Ordering::Acquire));
                        assert!(results.row_data(0).unwrap().has_icon);
                        ui.hide().unwrap();
                        slint::quit_event_loop().unwrap();
                    }
                }
                stage.set(stage.get() + 1);
            },
        );
        slint::run_event_loop_until_quit().unwrap();
    }
}
