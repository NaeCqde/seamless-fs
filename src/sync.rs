//! Keeps the relay up to date: scans the files when something changed, and withdraws this server when it stops.

use crate::{
    confine::InsideRoot,
    relay::Relay,
    scan::{is_hidden_path, scan},
};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{sync::Arc, time::Duration};
use tokio::{
    signal,
    sync::mpsc,
    time::{interval, MissedTickBehavior},
};

/// Changes are collected for this long before the files are scanned and sent again.
const QUIET_TIME: Duration = Duration::from_secs(10);

async fn announce(relay: &Relay, root: &InsideRoot, first: bool) {
    let root = root.clone();
    // the scan reads the whole tree: not on the thread that serves
    match tokio::task::spawn_blocking(move || scan(&root)).await {
        Ok(files) => relay.announce(first, &files).await,
        Err(e) => log::error!("scan failed: {e}"),
    }
}

/// Runs until the program is asked to stop (Ctrl-C, or SIGTERM on Unix), then withdraws this server from the relay.
pub async fn run(relay: Arc<Relay>, root: InsideRoot) {
    relay.withdraw().await; // forget what an earlier run left behind
    announce(&relay, &root, true).await;

    // one pending event is enough: the scan reads everything anyway
    let (tx, mut rx) = mpsc::channel::<()>(1);
    let watched = root.root().to_owned();
    let filter_root = watched.clone();
    let watcher = RecommendedWatcher::new(
        move |event: notify::Result<Event>| {
            let Ok(event) = event else { return };
            let changes = matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            );
            if changes && event.paths.iter().any(|p| !is_hidden_path(&filter_root, p)) {
                let _ = tx.try_send(());
            }
        },
        Config::default(),
    );
    // without a watcher the files are not sent again, but the server still serves
    let _watcher = watcher
        .and_then(|mut w| w.watch(&watched, RecursiveMode::Recursive).map(|_| w))
        .map_err(|e| log::error!("cannot watch {}: {e}", watched.display()))
        .ok();

    let mut ticker = interval(QUIET_TIME);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut changed = false;
    let stop = shutdown_signal();
    tokio::pin!(stop);

    loop {
        tokio::select! {
            Some(()) = rx.recv() => changed = true,
            _ = ticker.tick() => {
                if std::mem::take(&mut changed) {
                    announce(&relay, &root, false).await;
                }
            }
            _ = &mut stop => break,
        }
    }
    relay.withdraw().await;
}

/// Ctrl-C, and SIGTERM (what `systemctl stop` and `docker stop` send).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = signal::ctrl_c().await;
    }
}
