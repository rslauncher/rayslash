//! A single replaceable pending job, with no queue of superseded jobs.
//!
//! Sending never waits for the consumer to finish its current job. Superseded
//! jobs are dropped immediately, while the newest job is always delivered.

use std::{
    sync::{Arc, Condvar, Mutex, mpsc},
    time::{Duration, Instant},
};

pub(crate) struct Sender<T> {
    shared: Arc<Shared<T>>,
}

pub(crate) struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

struct Shared<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
}

struct State<T> {
    pending: Option<T>,
    senders: usize,
    receiver_alive: bool,
}

pub(crate) fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            pending: None,
            senders: 1,
            receiver_alive: true,
        }),
        ready: Condvar::new(),
    });
    (
        Sender {
            shared: shared.clone(),
        },
        Receiver { shared },
    )
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .senders += 1;
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<T> Sender<T> {
    pub(crate) fn send(&self, value: T) -> Result<(), mpsc::SendError<T>> {
        self.send_if(value, |_, _| true)
    }

    pub(crate) fn send_if(
        &self,
        value: T,
        replace: impl FnOnce(&T, &T) -> bool,
    ) -> Result<(), mpsc::SendError<T>> {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !state.receiver_alive {
            return Err(mpsc::SendError(value));
        }
        if state
            .pending
            .as_ref()
            .is_some_and(|previous| !replace(previous, &value))
        {
            return Ok(());
        }
        let previous = state.pending.replace(value);
        self.shared.ready.notify_one();
        drop(state);
        drop(previous);
        Ok(())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.senders -= 1;
        if state.senders == 0 {
            self.shared.ready.notify_one();
        }
    }
}

impl<T> Receiver<T> {
    pub(crate) fn recv(&self) -> Result<T, mpsc::RecvError> {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        loop {
            if let Some(value) = state.pending.take() {
                return Ok(value);
            }
            if state.senders == 0 {
                return Err(mpsc::RecvError);
            }
            state = self
                .shared
                .ready
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    pub(crate) fn recv_timeout(&self, timeout: Duration) -> Result<T, mpsc::RecvTimeoutError> {
        let started = Instant::now();
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        loop {
            if let Some(value) = state.pending.take() {
                return Ok(value);
            }
            if state.senders == 0 {
                return Err(mpsc::RecvTimeoutError::Disconnected);
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(mpsc::RecvTimeoutError::Timeout);
            }
            (state, _) = self
                .shared
                .ready
                .wait_timeout(state, remaining)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.receiver_alive = false;
        let pending = state.pending.take();
        drop(state);
        drop(pending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        thread,
    };

    #[test]
    fn blocked_consumer_retains_only_latest_pending_job() {
        struct Job(usize, Arc<AtomicUsize>);
        impl Drop for Job {
            fn drop(&mut self) {
                self.1.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let live = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = channel();
        for index in 0..10_000 {
            live.fetch_add(1, Ordering::SeqCst);
            assert!(sender.send(Job(index, live.clone())).is_ok());
            assert_eq!(live.load(Ordering::SeqCst), 1);
        }
        let job = receiver.recv().unwrap();
        assert_eq!(job.0, 9_999);
        drop(job);
        assert_eq!(live.load(Ordering::SeqCst), 0);
        assert_eq!(
            receiver.recv_timeout(Duration::ZERO).err(),
            Some(mpsc::RecvTimeoutError::Timeout)
        );
    }

    #[test]
    fn running_job_finishes_before_newest_replacement() {
        let (sender, receiver) = channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        sender.send(0).unwrap();
        let worker = thread::spawn(move || {
            assert_eq!(receiver.recv().unwrap(), 0);
            started_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
            assert_eq!(receiver.recv().unwrap(), 999);
            assert!(receiver.recv().is_err());
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for value in 1..1_000 {
            sender.send(value).unwrap();
        }
        drop(sender);
        finish_tx.send(()).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn disconnect_wakes_receiver_and_rejects_future_sends() {
        let (sender, receiver) = channel::<usize>();
        let clone = sender.clone();
        drop(sender);
        clone.send(42).unwrap();
        drop(clone);
        assert_eq!(receiver.recv_timeout(Duration::ZERO).unwrap(), 42);
        assert_eq!(
            receiver.recv_timeout(Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );

        let (sender, receiver) = channel();
        drop(receiver);
        assert_eq!(sender.send(7), Err(mpsc::SendError(7)));
    }

    #[test]
    fn concurrent_producer_does_not_lose_final_value_or_deadlock() {
        let (sender, receiver) = channel();
        let worker = thread::spawn(move || {
            let mut last = None;
            while let Ok(value) = receiver.recv() {
                assert!(last.is_none_or(|previous| previous < value));
                last = Some(value);
            }
            last
        });
        for value in 0..10_000 {
            sender.send(value).unwrap();
        }
        drop(sender);
        assert_eq!(worker.join().unwrap(), Some(9_999));
    }

    #[test]
    fn delayed_old_producer_cannot_replace_a_newer_pending_generation() {
        let (sender, receiver) = channel();
        sender.send_if(2, |old, new| old <= new).unwrap();
        sender.send_if(1, |old, new| old <= new).unwrap();
        assert_eq!(receiver.recv().unwrap(), 2);
    }

    #[test]
    fn dropping_receiver_releases_pending_payload_even_with_live_sender() {
        let (sender, receiver) = channel();
        let payload = Arc::new(());
        sender.send(payload.clone()).unwrap();
        assert_eq!(Arc::strong_count(&payload), 2);
        drop(receiver);
        assert_eq!(Arc::strong_count(&payload), 1);
        assert!(sender.send(payload.clone()).is_err());
    }
}
