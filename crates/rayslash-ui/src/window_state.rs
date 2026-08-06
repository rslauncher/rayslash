use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{AppWindow, ipc};
use rayslash_core::diagnostics::{OperationalDiagnostic, OperationalDiagnosticCode, Telemetry};
use slint::ComponentHandle;

pub(crate) fn handle_ipc_request(
    ui: &AppWindow,
    is_visible: &AtomicBool,
    request: ipc::IpcRequest,
    telemetry: &dyn Telemetry,
) {
    match request {
        ipc::IpcRequest::Show => show_launcher(ui, is_visible, telemetry),
        ipc::IpcRequest::Toggle if is_visible.load(Ordering::SeqCst) => {
            hide_launcher(ui, is_visible, telemetry);
        }
        ipc::IpcRequest::Toggle => show_launcher(ui, is_visible, telemetry),
    }
}

pub(crate) fn show_launcher(ui: &AppWindow, is_visible: &AtomicBool, telemetry: &dyn Telemetry) {
    ui.invoke_reset_requested();
    ui.set_control_held(false);

    match ui.show() {
        Ok(()) => {
            is_visible.store(true, Ordering::SeqCst);
            ui.invoke_focus_search();
        }
        Err(error) => {
            telemetry.operational_failure(OperationalDiagnostic::new(
                OperationalDiagnosticCode::WindowShow,
            ));
            eprintln!("failed to show rayslash window: {error}");
        }
    }
}

pub(crate) fn hide_launcher(ui: &AppWindow, is_visible: &AtomicBool, telemetry: &dyn Telemetry) {
    ui.set_control_held(false);

    if let Err(error) = ui.hide() {
        telemetry.operational_failure(OperationalDiagnostic::new(
            OperationalDiagnosticCode::WindowHide,
        ));
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
