use std::sync::{OnceLock, mpsc};

use rayslash_core::{app_state, ranking};

enum StateWrite {
    Apps(app_state::AppInstallState),
    Ranking(ranking::RankingState),
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
                    StateWrite::Apps(state) => {
                        if let Err(error) = app_state::save_app_state(&state) {
                            eprintln!("{error}");
                        }
                    }
                    StateWrite::Ranking(state) => {
                        if let Err(error) = ranking::save_ranking_state(&state) {
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
        (StateWrite::Apps(_), StateWrite::Apps(_))
        | (StateWrite::Ranking(_), StateWrite::Ranking(_)) => newer,
        _ => {
            // Different files cannot replace one another. Queue the older write back
            // behind the newer one; ordering between independent state files is irrelevant.
            let _ = sender().send(current);
            newer
        }
    }
}

pub(crate) fn save_app_state(state: app_state::AppInstallState) {
    let _ = sender().send(StateWrite::Apps(state));
}

pub(crate) fn save_ranking_state(state: ranking::RankingState) {
    let _ = sender().send(StateWrite::Ranking(state));
}
