//! Native crash recovery, independent of explicit project saves.

mod io;
mod store;

pub(super) use io::start;

use super::project::Operation;
use super::project_io::{OpenedProject, SaveSnapshot};
use super::{FastFill, Message};
use iced::widget::{button, column, container, row, text};
use iced::{Element, Fill, Task};
use std::sync::{Arc, mpsc as commands};
use store::Candidate;
use uuid::Uuid;

type Stamp = (Uuid, Vec<(Uuid, u64)>);

#[derive(Default)]
pub(super) struct Recovery {
    sender: Option<commands::Sender<Command>>,
    candidate: Option<Candidate>,
    busy: bool,
    writing: bool,
    project: Option<Uuid>,
    snapshot: Option<Stamp>,
    needs_clear: bool,
    error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Ready(commands::Sender<Command>, Option<Candidate>),
    Tick,
    Written(Stamp, Result<(), String>),
    Cleared(Result<(), String>),
    Restored(Uuid, Result<Arc<OpenedProject>, String>),
    Discarded(Result<Option<Candidate>, String>),
    Closed(iced::window::Id, Result<(), String>),
    Failed(String),
}

#[derive(Debug)]
pub(crate) enum Command {
    Write(Stamp, SaveSnapshot),
    Clear,
    Read(Uuid),
    Adopt(Uuid),
    Discard(Uuid),
    Defer,
    Close(iced::window::Id),
}

impl Recovery {
    fn send(&mut self, command: Command) -> bool {
        let Some(sender) = &self.sender else {
            return false;
        };
        if sender.send(command).is_ok() {
            return true;
        }
        self.sender = None;
        self.error = Some(
            "Automatic recovery stopped. Save your project explicitly.".into(),
        );
        false
    }
}

// Called after ordinary edits and file operations, not worker completions.
pub(super) fn synchronize(state: &mut FastFill) {
    let project = state.project.header.id;
    let recovery = &mut state.recovery;
    if recovery.project != Some(project)
        || !state.project.is_dirty(&state.pages)
    {
        if recovery.needs_clear && recovery.send(Command::Clear) {
            recovery.needs_clear = false;
        }
        recovery.snapshot = None;
    }
    recovery.project = Some(project);
}

fn checkpoint(state: &mut FastFill) {
    synchronize(state);
    if state.recovery.sender.is_none()
        || state.recovery.writing
        || !state.project.is_idle()
        || !state.project.is_dirty(&state.pages)
        || state.pages.iter().any(|page| {
            page.annotations.active_pointer().is_some()
                || page.annotations.has_text_edit()
        })
    {
        return;
    }
    let stamp = (state.project.header.id, state.pages.identity());
    if state.recovery.snapshot.as_ref() == Some(&stamp) {
        return;
    }
    match state.project_snapshot() {
        Ok(snapshot) => {
            if state.recovery.send(Command::Write(stamp.clone(), snapshot)) {
                state.recovery.writing = true;
                state.recovery.needs_clear = true;
                state.recovery.snapshot = Some(stamp);
            }
        }
        Err(error) => state.recovery.error = Some(error),
    }
}

pub(super) fn close(
    state: &mut FastFill,
    id: iced::window::Id,
) -> Task<Message> {
    if state.recovery.send(Command::Close(id)) {
        state.project.operation = Operation::ClosingRecovery;
        state.status = "Finishing recovery cleanup…".into();
        state.status_is_error = false;
        Task::none()
    } else {
        // If recovery never initialized, there is nothing owned to clean up.
        iced::window::close(id)
    }
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if let Message::Recovery(event) = message {
        return Some(receive(state, event));
    }
    if matches!(state.project.operation, Operation::Recovering) {
        match message {
            Message::CloseRequested(id) => return Some(close(state, *id)),
            Message::RestoreRecovery | Message::DiscardRecovery
                if !state.recovery.busy =>
            {
                if let Some(candidate) = &state.recovery.candidate {
                    let command = if matches!(message, Message::RestoreRecovery)
                    {
                        Command::Read(candidate.id)
                    } else {
                        Command::Discard(candidate.id)
                    };
                    state.recovery.busy = state.recovery.send(command);
                }
            }
            Message::DeferRecovery if !state.recovery.busy => {
                state.recovery.send(Command::Defer);
                state.recovery.candidate = None;
                state.recovery.error = None;
                state.project.operation = Operation::Idle;
            }
            _ => return None,
        }
        return Some(Task::none());
    }
    matches!(
        message,
        Message::RestoreRecovery
            | Message::DiscardRecovery
            | Message::DeferRecovery
    )
    .then(Task::none)
}

fn receive(state: &mut FastFill, event: &Event) -> Task<Message> {
    match event {
        Event::Ready(sender, candidate) => {
            state.recovery.sender = Some(sender.clone());
            state.recovery.project = Some(state.project.header.id);
            state.recovery.candidate.clone_from(candidate);
            if candidate.is_none() {
                state.project.operation = Operation::Idle;
            }
        }
        Event::Tick => checkpoint(state),
        Event::Written(stamp, result) => {
            state.recovery.writing = false;
            match result {
                Ok(()) => state.recovery.error = None,
                Err(error) => {
                    if state.recovery.snapshot.as_ref() == Some(stamp) {
                        state.recovery.snapshot = None;
                    }
                    state.recovery.error = Some(format!(
                        "Recovery snapshot failed: {error}. Save explicitly."
                    ));
                }
            }
        }
        Event::Cleared(result) => match result {
            Ok(()) => state.recovery.error = None,
            Err(error) => {
                state.recovery.needs_clear = true;
                state.recovery.error = Some(format!(
                    "Could not remove old recovery snapshot: {error}"
                ));
            }
        },
        Event::Restored(id, result) => {
            restored(state, *id, result);
        }
        Event::Discarded(result) => {
            if !matches!(state.project.operation, Operation::Recovering) {
                return Task::none();
            }
            state.recovery.busy = false;
            match result {
                Ok(candidate) => {
                    state.recovery.candidate.clone_from(candidate);
                    state.recovery.error = None;
                    if candidate.is_none() {
                        state.project.operation = Operation::Idle;
                    }
                }
                Err(error) => {
                    state.recovery.error = Some(format!(
                        "Could not discard recovery; file kept: {error}"
                    ));
                }
            }
        }
        Event::Closed(id, result) => match result {
            Ok(()) => return iced::window::close(*id),
            Err(error) => {
                state.project.operation = Operation::Idle;
                state.recovery.error = Some(format!(
                    "Could not clean recovery before closing: {error}"
                ));
            }
        },
        Event::Failed(error) => {
            state.recovery.sender = None;
            state.recovery.writing = false;
            if matches!(
                state.project.operation,
                Operation::Recovering | Operation::ClosingRecovery
            ) {
                state.project.operation = Operation::Idle;
            }
            state.recovery.error = Some(format!(
                "Automatic recovery unavailable: {error}. Save explicitly."
            ));
        }
    }
    Task::none()
}

fn restored(
    state: &mut FastFill,
    id: Uuid,
    result: &Result<Arc<OpenedProject>, String>,
) {
    if !matches!(state.project.operation, Operation::Recovering)
        || state.recovery.candidate.as_ref().map(|c| c.id) != Some(id)
    {
        return;
    }
    state.recovery.busy = false;
    let result = result
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|opened| state.restore_project(opened));
    match result {
        Ok(()) => {
            state.recovery.error = None;
            state.recovery.send(Command::Adopt(id));
            state.recovery.candidate = None;
            state.recovery.project = Some(state.project.header.id);
            state.recovery.snapshot =
                Some((state.project.header.id, state.pages.identity()));
            state.recovery.needs_clear = true;
        }
        Err(error) => {
            state.recovery.error =
                Some(format!("Could not restore recovery; file kept: {error}"));
        }
    }
}

pub(super) fn banner(state: &FastFill) -> Option<Element<'_, Message>> {
    let prompt = prompt(state);
    let error: Option<Element<'_, Message>> =
        state.recovery.error.as_ref().map(|error| {
            container(text(error).size(14))
                .padding([6, 10])
                .style(container::danger)
                .into()
        });
    if prompt.is_none() && error.is_none() {
        return None;
    }
    Some(column![].extend(prompt).extend(error).spacing(4).into())
}

fn prompt(state: &FastFill) -> Option<Element<'_, Message>> {
    if !matches!(state.project.operation, Operation::Recovering) {
        return None;
    }
    let content = if let Some(candidate) = &state.recovery.candidate {
        let date = time::OffsetDateTime::from(candidate.modified)
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "unknown time".into());
        let enabled = !state.recovery.busy;
        row![
            text(format!("Unsaved recovery from {date}. Restore a copy?"))
                .width(Fill),
            button("Not now")
                .on_press_maybe(enabled.then_some(Message::DeferRecovery)),
            button("Discard")
                .on_press_maybe(enabled.then_some(Message::DiscardRecovery)),
            button("Restore")
                .on_press_maybe(enabled.then_some(Message::RestoreRecovery)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
    } else {
        row![text("Checking for unsaved recovery…")]
    };
    Some(
        container(content)
            .padding(8)
            .style(container::warning)
            .into(),
    )
}

#[cfg(test)]
mod tests;
