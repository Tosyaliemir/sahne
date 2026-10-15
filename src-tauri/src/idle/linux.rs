//! Linux idle detection.
//!
//! Wayland: `ext-idle-notify-v1` (KDE, GNOME 47+, wlroots). Version 2's
//! "input idle" ignores idle inhibitors, which matters because media players
//! usually inhibit idleness while music plays. Fallbacks poll D-Bus:
//! GNOME's Mutter IdleMonitor, then `org.freedesktop.ScreenSaver` (X11).

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tokio::sync::watch;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};

const POLL_INTERVAL: Duration = Duration::from_secs(5);

// --- Wayland -------------------------------------------------------------------

#[derive(Default)]
struct State {
    seat: Option<wl_seat::WlSeat>,
    notifier: Option<(ext_idle_notifier_v1::ExtIdleNotifierV1, u32)>,
    idle: Option<bool>,
}

pub(super) fn run_wayland(timeout_ms: &AtomicU32, tx: &watch::Sender<bool>) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let mut queue: EventQueue<State> = conn.new_event_queue();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());
    let mut state = State::default();
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
    let (Some(seat), Some((notifier, version))) = (state.seat.clone(), state.notifier.clone())
    else {
        return Err("compositor lacks ext_idle_notifier_v1".into());
    };
    tracing::info!("idle detection via ext_idle_notifier_v1 v{version}");

    let mut applied_ms: Option<u32> = None;
    let mut notification: Option<ext_idle_notification_v1::ExtIdleNotificationV1> = None;
    loop {
        // (Re)create the notification whenever the timeout setting changes.
        let wanted = timeout_ms.load(Ordering::Relaxed);
        if applied_ms != Some(wanted) {
            applied_ms = Some(wanted);
            if let Some(old) = notification.take() {
                old.destroy();
            }
            state.idle = Some(false);
            tx.send_replace(false);
            if wanted > 0 {
                notification = Some(if version >= 2 {
                    notifier.get_input_idle_notification(wanted, &seat, &qh, ())
                } else {
                    notifier.get_idle_notification(wanted, &seat, &qh, ())
                });
            }
        }

        queue.flush().map_err(|e| e.to_string())?;
        queue
            .dispatch_pending(&mut state)
            .map_err(|e| e.to_string())?;
        if let Some(guard) = queue.prepare_read() {
            // Wake at least once a second to notice setting changes.
            if wait_readable(guard.connection_fd(), 1000) {
                guard.read().map_err(|e| e.to_string())?;
            }
        }
        queue
            .dispatch_pending(&mut state)
            .map_err(|e| e.to_string())?;
        if let Some(idle) = state.idle {
            if *tx.borrow() != idle {
                tracing::debug!("system idle: {idle}");
                tx.send_replace(idle);
            }
        }
        if tx.is_closed() {
            return Ok(());
        }
    }
}

fn wait_readable(fd: std::os::fd::BorrowedFd<'_>, timeout_ms: i32) -> bool {
    use std::os::fd::AsRawFd;
    let mut pfd = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd for the duration of the call.
    let n = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    n > 0 && (pfd.revents & libc::POLLIN) != 0
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_seat" if state.seat.is_none() => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()));
                }
                "ext_idle_notifier_v1" => {
                    let v = version.min(2);
                    state.notifier = Some((registry.bind(name, v, qh, ()), v));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => state.idle = Some(true),
            ext_idle_notification_v1::Event::Resumed => state.idle = Some(false),
            _ => {}
        }
    }
}

wayland_client::delegate_noop!(State: ignore wl_seat::WlSeat);
wayland_client::delegate_noop!(State: ignore ext_idle_notifier_v1::ExtIdleNotifierV1);

// --- D-Bus polling fallback ----------------------------------------------------------

#[zbus::proxy(
    interface = "org.gnome.Mutter.IdleMonitor",
    default_service = "org.gnome.Mutter.IdleMonitor",
    default_path = "/org/gnome/Mutter/IdleMonitor/Core"
)]
trait MutterIdle {
    fn get_idletime(&self) -> zbus::Result<u64>;
}

#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver"
)]
trait ScreenSaver {
    fn get_session_idle_time(&self) -> zbus::Result<u32>;
}

pub(super) async fn poll_dbus(timeout_ms: &AtomicU32, tx: &watch::Sender<bool>) {
    let Ok(conn) = zbus::Connection::session().await else {
        tracing::warn!("no session bus; screensaver mode disabled");
        return;
    };
    let mutter = MutterIdleProxy::new(&conn).await.ok();
    let saver = ScreenSaverProxy::new(&conn).await.ok();
    let mut warned = false;
    loop {
        let idle_ms = match &mutter {
            Some(m) => m.get_idletime().await.ok(),
            None => None,
        };
        let idle_ms = match idle_ms {
            Some(ms) => Some(ms),
            None => match &saver {
                // Reported in seconds by most implementations.
                Some(s) => s
                    .get_session_idle_time()
                    .await
                    .ok()
                    .map(|s| u64::from(s) * 1000),
                None => None,
            },
        };
        match idle_ms {
            Some(ms) => {
                let wanted = u64::from(timeout_ms.load(Ordering::Relaxed));
                let idle = wanted > 0 && ms >= wanted;
                if *tx.borrow() != idle {
                    tx.send_replace(idle);
                }
            }
            None if !warned => {
                tracing::warn!(
                    "no way to read idle time on this desktop; screensaver mode disabled"
                );
                warned = true;
            }
            None => {}
        }
        if tx.is_closed() {
            return;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
