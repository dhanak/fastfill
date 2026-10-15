//! Blocking recovery work and platform paths stay off the UI thread.

use super::store::Store;
use super::{Command, Event};
use crate::app::Message;
use iced::Task;
use iced::futures::{StreamExt, channel::mpsc};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc as commands};
use std::time::{Duration, Instant};

const INTERVAL: Duration = Duration::from_secs(10);

pub(crate) fn start() -> Task<Message> {
    let stream = iced::futures::stream::once(async {
        let (sender, receiver) = mpsc::unbounded();
        let events = sender.clone();
        let worker = std::thread::Builder::new()
            .name("fastfill-recovery".into())
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || worker(&directory()?, &events, INTERVAL),
                    ))
                    .unwrap_or_else(|_| Err("Recovery worker failed.".into()));
                if let Err(error) = result {
                    let _ = events.unbounded_send(Event::Failed(error));
                }
            });
        if let Err(error) = worker {
            let _ = sender.unbounded_send(Event::Failed(error.to_string()));
        }
        receiver
    })
    .flatten();
    Task::run(stream, Message::Recovery)
}

fn directory() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("FASTFILL_RECOVERY_DIR") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(
                "FASTFILL_RECOVERY_DIR must be an absolute path.".into()
            );
        }
        return Ok(path);
    }
    #[cfg(target_os = "windows")]
    return std::env::var_os("LOCALAPPDATA")
        .map(|path| PathBuf::from(path).join("FastFill/Recovery"))
        .ok_or_else(|| "LOCALAPPDATA is not set.".into());
    #[cfg(target_os = "macos")]
    return std::env::var_os("HOME")
        .map(|path| {
            PathBuf::from(path)
                .join("Library/Application Support/FastFill/Recovery")
        })
        .ok_or_else(|| "HOME is not set.".into());
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|path| PathBuf::from(path).join(".local/state"))
            })
            .ok_or("HOME and XDG_STATE_HOME are not set.")?;
        Ok(state.join("fastfill/recovery"))
    }
}

pub(super) fn worker(
    directory: &Path,
    events: &mpsc::UnboundedSender<Event>,
    interval: Duration,
) -> Result<(), String> {
    let mut store = Store::open(directory)?;
    let (sender, receiver) = commands::channel();
    if events
        .unbounded_send(Event::Ready(sender, store.candidate()))
        .is_err()
    {
        return Ok(());
    }
    let mut next_tick = Instant::now() + interval;
    loop {
        if Instant::now() >= next_tick {
            if events.unbounded_send(Event::Tick).is_err() {
                return Ok(());
            }
            next_tick = Instant::now() + interval;
        }
        let event = match receiver
            .recv_timeout(next_tick.saturating_duration_since(Instant::now()))
        {
            Ok(Command::Write(stamp, snapshot)) => {
                Event::Written(stamp, store.write(snapshot))
            }
            Ok(Command::Clear) => Event::Cleared(store.clear()),
            Ok(Command::Read(id)) => {
                Event::Restored(id, store.read(id).map(Arc::new))
            }
            Ok(Command::Adopt(id)) => {
                store.adopt(id)?;
                continue;
            }
            Ok(Command::Discard(id)) => {
                let result = store.discard(id).map(|()| store.candidate());
                Event::Discarded(result)
            }
            Ok(Command::Defer) => {
                store.defer();
                continue;
            }
            Ok(Command::Close(id)) => {
                let result = store.clear();
                let done = result.is_ok();
                let _ = events.unbounded_send(Event::Closed(id, result));
                if done {
                    return Ok(());
                }
                continue;
            }
            Err(commands::RecvTimeoutError::Timeout) => continue,
            Err(commands::RecvTimeoutError::Disconnected) => return Ok(()),
        };
        if events.unbounded_send(event).is_err() {
            return Ok(());
        }
    }
}
