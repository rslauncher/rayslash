use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{AppWindow, ipc};
use slint::ComponentHandle;

pub(crate) fn handle_ipc_request(
    ui: &AppWindow,
    is_visible: &AtomicBool,
    request: ipc::IpcRequest,
) {
    match request {
        ipc::IpcRequest::Show => show_launcher(ui, is_visible),
        ipc::IpcRequest::Toggle if is_visible.load(Ordering::SeqCst) => {
            hide_launcher(ui, is_visible);
        }
        ipc::IpcRequest::Toggle => show_launcher(ui, is_visible),
    }
}

pub(crate) fn show_launcher(ui: &AppWindow, is_visible: &AtomicBool) {
    ui.invoke_reset_requested();
    ui.set_control_held(false);

    match ui.show() {
        Ok(()) => {
            is_visible.store(true, Ordering::SeqCst);
            ui.invoke_focus_search();
        }
        Err(error) => eprintln!("failed to show rayslash window: {error}"),
    }
}

pub(crate) fn hide_launcher(ui: &AppWindow, is_visible: &AtomicBool) {
    ui.set_control_held(false);

    if let Err(error) = ui.hide() {
        eprintln!("failed to hide rayslash window: {error}");
    } else {
        is_visible.store(false, Ordering::SeqCst);
    }
}

pub(crate) fn should_start_resident_after_send_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
    )
}

pub(crate) fn visible_flag(initially_visible: bool) -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(initially_visible))
}
