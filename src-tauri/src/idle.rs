//! System-wide idle detection for the screensaver mode.
//!
//! Linux: see `idle/linux.rs` (Wayland idle notifications, D-Bus fallbacks).
//! Windows: the time since the last keyboard/mouse input, polled.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

#[cfg(target_os = "linux")]
mod linux;

/// Shared with the monitor thread; 0 disables idle detection.
pub struct IdleMonitor {
    timeout_ms: Arc<AtomicU32>,
    idle: watch::Receiver<bool>,
}

impl IdleMonitor {
    pub fn start(timeout: Duration) -> Self {
        let timeout_ms = Arc::new(AtomicU32::new(duration_ms(timeout)));
        let (tx, idle) = watch::channel(false);
        let shared = Arc::clone(&timeout_ms);
        let spawned = std::thread::Builder::new()
            .name("sahne-idle".into())
            .spawn(move || run(&shared, &tx));
        if let Err(e) = spawned {
            tracing::warn!("could not start idle monitor: {e}");
        }
        Self { timeout_ms, idle }
    }

    pub fn set_timeout(&self, timeout: Option<Duration>) {
        self.timeout_ms
            .store(timeout.map_or(0, duration_ms), Ordering::Relaxed);
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.idle.clone()
    }
}

fn duration_ms(d: Duration) -> u32 {
    u32::try_from(d.as_millis()).unwrap_or(u32::MAX).max(1)
}

#[cfg(target_os = "linux")]
fn run(timeout_ms: &AtomicU32, tx: &watch::Sender<bool>) {
    if let Err(e) = linux::run_wayland(timeout_ms, tx) {
        tracing::info!("Wayland idle notifications unavailable ({e}); polling D-Bus instead");
        tauri::async_runtime::block_on(linux::poll_dbus(timeout_ms, tx));
    }
}

/// Windows: compares the time since the last input with the timeout.
#[cfg(windows)]
fn run(timeout_ms: &AtomicU32, tx: &watch::Sender<bool>) {
    use ::windows::Win32::System::SystemInformation::GetTickCount;
    use ::windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    loop {
        let mut info = LASTINPUTINFO {
            cbSize: u32::try_from(std::mem::size_of::<LASTINPUTINFO>()).unwrap_or(8),
            dwTime: 0,
        };
        // SAFETY: `info` is a valid, correctly sized LASTINPUTINFO.
        if unsafe { GetLastInputInfo(&mut info) }.as_bool() {
            // Both are 32-bit millisecond tick counts; wrapping_sub handles rollover.
            // SAFETY: GetTickCount has no preconditions.
            let idle_ms = unsafe { GetTickCount() }.wrapping_sub(info.dwTime);
            let wanted = timeout_ms.load(Ordering::Relaxed);
            let idle = wanted > 0 && idle_ms >= wanted;
            if *tx.borrow() != idle {
                tx.send_replace(idle);
            }
        }
        if tx.is_closed() {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_fit_protocol_range() {
        assert_eq!(duration_ms(Duration::from_secs(180)), 180_000);
        assert_eq!(duration_ms(Duration::ZERO), 1);
        assert_eq!(duration_ms(Duration::from_secs(u64::MAX)), u32::MAX);
    }
}
