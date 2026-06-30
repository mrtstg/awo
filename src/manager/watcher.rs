use anyhow::Result;
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

pub enum WatcherEvent {
    FileChanged(Vec<PathBuf>),
    InitialScanComplete(usize),
}

pub struct FileWatcher {
    watch_patterns: Vec<String>,
    event_sender: Sender<WatcherEvent>,
    cancel_token: CancellationToken,
}

impl FileWatcher {
    pub fn new(
        watch_patterns: Vec<String>,
        event_sender: Sender<WatcherEvent>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            watch_patterns,
            event_sender,
            cancel_token,
        }
    }

    pub async fn start_watching(self) -> Result<()> {
        if self.watch_patterns.is_empty() {
            return Ok(());
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx)?;

        let mut watched_paths = Vec::new();
        for pattern in &self.watch_patterns {
            match glob::glob(pattern) {
                Ok(paths) => {
                    for path in paths.filter_map(Result::ok) {
                        // resursive mode is ignored on files
                        // if file is not accessible watcher returns error, but works
                        // so we ignore it
                        let _ = watcher.watch(&path, RecursiveMode::Recursive);
                        watched_paths.push(path);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to expand glob pattern '{}': {:?}",
                        pattern, e
                    );
                }
            }
        }

        if !watched_paths.is_empty() {
            self.event_sender
                .send(WatcherEvent::InitialScanComplete(watched_paths.len()))
                .await?;
        } else {
            return Ok(());
        }

        let event_sender = self.event_sender;
        let cancel_token = self.cancel_token.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                if cancel_token.is_cancelled() {
                    break;
                }

                // Use a timeout so we can check cancellation periodically
                match rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(Ok(event)) => {
                        let is_metatdata = matches!(
                            event.kind,
                            EventKind::Modify(notify::event::ModifyKind::Metadata(_))
                        );
                        if (event.kind.is_modify() && !is_metatdata)
                            || event.kind.is_create()
                            || event.kind.is_remove()
                        {
                            let paths: Vec<PathBuf> = event.paths.clone();

                            if let Err(_) =
                                event_sender.blocking_send(WatcherEvent::FileChanged(paths))
                            {
                                break;
                            }
                        }
                    }
                    Ok(Err(error)) => {
                        eprintln!("Watcher error: {:?}", error);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        self.cancel_token.cancelled().await;
        drop(watcher);
        Ok(())
    }
}
