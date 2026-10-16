//! Small concurrency helpers shared by the services.

use std::sync::{Mutex, MutexGuard};

use tauri::async_runtime::JoinHandle;

/// Locks a mutex, recovering from poisoning: a panic while holding one of
/// our locks leaves plain data behind, which is still fine to use.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// At most one background job per service: starting a new one (e.g. for the
/// next track) aborts the previous, so rapid skipping never piles up work.
#[derive(Default)]
pub struct Inflight(Mutex<Option<JoinHandle<()>>>);

impl Inflight {
    /// Stores `task` (or nothing) and aborts the job it replaces.
    pub fn replace(&self, task: Option<JoinHandle<()>>) {
        if let Some(old) = std::mem::replace(&mut *lock(&self.0), task) {
            old.abort();
        }
    }

    #[cfg(test)]
    fn is_running(&self) -> bool {
        lock(&self.0)
            .as_ref()
            .is_some_and(|t| !t.inner().is_finished())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_aborts_the_previous_job() {
        let inflight = Inflight::default();
        let mut handles = Vec::new();
        for _ in 0..10 {
            let task = tauri::async_runtime::spawn(async {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            });
            handles.push(task.inner().abort_handle());
            inflight.replace(Some(task));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        let running = handles.iter().filter(|h| !h.is_finished()).count();
        assert_eq!(running, 1, "only the newest job may still run");
        assert!(inflight.is_running());
        inflight.replace(None);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(handles.iter().all(tokio::task::AbortHandle::is_finished));
    }
}
