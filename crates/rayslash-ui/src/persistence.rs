use std::sync::{Arc, OnceLock, mpsc};

use rayslash_core::{
    app_state,
    diagnostics::{OperationalDiagnostic, OperationalDiagnosticCode, Telemetry},
    ranking,
};

enum StateWrite {
    Apps(app_state::AppInstallState, Arc<dyn Telemetry>),
    Ranking(ranking::RankingState, Arc<dyn Telemetry>),
}

impl StateWrite {
    fn telemetry(self) -> Arc<dyn Telemetry> {
        match self {
            Self::Apps(_, telemetry) | Self::Ranking(_, telemetry) => telemetry,
        }
    }
}

fn sender() -> &'static mpsc::Sender<StateWrite> {
    static SENDER: OnceLock<mpsc::Sender<StateWrite>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut write) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() {
                    write = merge(write, newer);
                }
                match write {
                    StateWrite::Apps(state, telemetry) => {
                        if let Err(error) = app_state::save_app_state(&state) {
                            telemetry.operational_failure(app_state_save_diagnostic(&error));
                            eprintln!("{error}");
                        }
                    }
                    StateWrite::Ranking(state, telemetry) => {
                        if let Err(error) = ranking::save_ranking_state(&state) {
                            telemetry.operational_failure(ranking_state_save_diagnostic(&error));
                            eprintln!("{error}");
                        }
                    }
                }
            }
        });
        sender
    })
}

fn merge(current: StateWrite, newer: StateWrite) -> StateWrite {
    match (&current, &newer) {
        (StateWrite::Apps(_, _), StateWrite::Apps(_, _))
        | (StateWrite::Ranking(_, _), StateWrite::Ranking(_, _)) => newer,
        _ => {
            // Different files cannot replace one another. Queue the older write back
            // behind the newer one; ordering between independent state files is irrelevant.
            let _ = sender().send(current);
            newer
        }
    }
}

pub(crate) fn save_app_state(state: app_state::AppInstallState, telemetry: Arc<dyn Telemetry>) {
    if let Err(error) = sender().send(StateWrite::Apps(state, telemetry)) {
        let telemetry = error.0.telemetry();
        telemetry.operational_failure(OperationalDiagnostic::new(
            OperationalDiagnosticCode::ApplicationStateSave,
        ));
    }
}

pub(crate) fn save_ranking_state(state: ranking::RankingState, telemetry: Arc<dyn Telemetry>) {
    if let Err(error) = sender().send(StateWrite::Ranking(state, telemetry)) {
        let telemetry = error.0.telemetry();
        telemetry.operational_failure(OperationalDiagnostic::new(
            OperationalDiagnosticCode::RankingStateSave,
        ));
    }
}

fn app_state_save_diagnostic(error: &app_state::SaveAppStateError) -> OperationalDiagnostic {
    match error {
        app_state::SaveAppStateError::CreateDir { source, .. }
        | app_state::SaveAppStateError::Write { source, .. } => {
            OperationalDiagnostic::from_io(OperationalDiagnosticCode::ApplicationStateSave, source)
        }
        app_state::SaveAppStateError::Serialize { .. } => {
            OperationalDiagnostic::new(OperationalDiagnosticCode::ApplicationStateSave)
        }
    }
}

fn ranking_state_save_diagnostic(error: &ranking::SaveRankingStateError) -> OperationalDiagnostic {
    match error {
        ranking::SaveRankingStateError::CreateDir { source, .. }
        | ranking::SaveRankingStateError::Write { source, .. } => {
            OperationalDiagnostic::from_io(OperationalDiagnosticCode::RankingStateSave, source)
        }
        ranking::SaveRankingStateError::Serialize { .. } => {
            OperationalDiagnostic::new(OperationalDiagnosticCode::RankingStateSave)
        }
    }
}
