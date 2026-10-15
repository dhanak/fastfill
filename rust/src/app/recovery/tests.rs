use super::io::worker;
use super::store::Store;
use super::*;
use crate::app::controller::update;
use crate::app::project::LeaveAction;
use crate::app::tests::{annotated_page, paste_text};
use crate::app::{PagePoint, Pointer, Tool};
use iced::futures::{StreamExt, channel::mpsc};
use std::time::{Duration, Instant, SystemTime};

fn connected() -> (FastFill, commands::Receiver<Command>) {
    let mut state = annotated_page();
    let (sender, receiver) = commands::channel();
    state.recovery.sender = Some(sender);
    state.recovery.project = Some(state.project.header.id);
    (state, receiver)
}

fn tick(state: &mut FastFill) {
    let _ = update(state, Message::Recovery(Event::Tick));
}

fn complete_write(
    state: &mut FastFill,
    receiver: &commands::Receiver<Command>,
) -> Stamp {
    let Command::Write(stamp, snapshot) = receiver.try_recv().unwrap() else {
        panic!("expected write");
    };
    assert_eq!(snapshot.project.pages[0].annotations.len(), 1);
    let _ = update(
        state,
        Message::Recovery(Event::Written(stamp.clone(), Ok(()))),
    );
    stamp
}

#[test]
fn ticks_snapshot_only_changed_committed_work_without_saving_the_project() {
    let (mut state, receiver) = connected();
    let id = state.project.header.id;
    let identity = state.pages.identity();
    let status = state.status.clone();
    state.project.operation = Operation::Opening(1);
    tick(&mut state);
    assert!(receiver.try_recv().is_err());
    state.project.operation = Operation::Idle;
    tick(&mut state);
    complete_write(&mut state, &receiver);
    assert_eq!(state.project.header.id, id);
    assert_eq!(state.pages.identity(), identity);
    assert_eq!(state.status, status);
    assert!(state.project.is_dirty(&state.pages));
    assert!(state.project.path.is_none());
    assert!(state.annotations().can_undo());
    tick(&mut state);
    assert!(receiver.try_recv().is_err());
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(300.0, 200.0), 1.0),
    );
    paste_text(&mut state, "Not accepted yet");
    tick(&mut state);
    assert!(state.annotations().has_text_edit());
    assert!(receiver.try_recv().is_err());
    let _ = update(&mut state, Message::CancelInteraction);
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(300.0, 200.0), 1.0),
    );
    tick(&mut state);
    assert!(state.annotations().active_pointer().is_some());
    assert!(receiver.try_recv().is_err());
}

#[test]
fn new_project_clears_after_inflight_write_and_ignores_its_late_completion() {
    let (mut state, receiver) = connected();
    tick(&mut state);
    let Command::Write(stamp, _) = receiver.try_recv().unwrap() else {
        panic!("expected write");
    };
    let _ = update(&mut state, Message::NewProject);
    let _ = update(&mut state, Message::DiscardProject);
    assert!(matches!(receiver.try_recv(), Ok(Command::Clear)));
    let id = state.project.header.id;
    let _ =
        update(&mut state, Message::Recovery(Event::Written(stamp, Ok(()))));
    tick(&mut state);
    assert!(receiver.try_recv().is_err());
    assert_eq!(state.project.header.id, id);
    assert!(!state.project.is_dirty(&state.pages));
}

#[test]
fn successful_save_clears_recovery_but_canceled_save_keeps_it() {
    let (mut state, receiver) = connected();
    tick(&mut state);
    complete_write(&mut state, &receiver);
    let _ = update(&mut state, Message::SaveProject);
    let Operation::Saving { id, .. } = state.project.operation else {
        panic!("expected save");
    };
    let _ = update(&mut state, Message::ProjectSaved(id, Ok(None)));
    assert!(receiver.try_recv().is_err());
    let _ = update(&mut state, Message::SaveProject);
    let Operation::Saving { id, .. } = state.project.operation else {
        panic!("expected save");
    };
    let receipt = super::super::project_io::SaveReceipt {
        path: Some("original.docscan".into()),
        name: "original.docscan".into(),
    };
    let _ = update(&mut state, Message::ProjectSaved(id, Ok(Some(receipt))));
    assert!(matches!(receiver.try_recv(), Ok(Command::Clear)));
    assert!(!state.project.is_dirty(&state.pages));
    tick(&mut state);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn write_and_cleanup_errors_are_visible_and_retry_on_next_tick() {
    let (mut state, receiver) = connected();
    tick(&mut state);
    let Command::Write(stamp, _) = receiver.try_recv().unwrap() else {
        panic!("expected write");
    };
    let _ = update(
        &mut state,
        Message::Recovery(Event::Written(stamp, Err("Disk full".into()))),
    );
    assert!(state.recovery.error.as_ref().unwrap().contains("Disk full"));
    tick(&mut state);
    complete_write(&mut state, &receiver);
    assert!(state.recovery.error.is_none());
    let _ = update(&mut state, Message::NewProject);
    let _ = update(&mut state, Message::DiscardProject);
    assert!(matches!(receiver.try_recv(), Ok(Command::Clear)));
    let _ = update(
        &mut state,
        Message::Recovery(Event::Cleared(Err("Permission denied".into()))),
    );
    assert!(receiver.try_recv().is_err()); // No runaway retry loop.
    tick(&mut state);
    assert!(matches!(receiver.try_recv(), Ok(Command::Clear)));
}

#[test]
fn startup_restore_keeps_backup_and_opens_unsaved_copy_without_original_path() {
    let (mut state, receiver) = connected();
    state.project.operation = Operation::Recovering;
    let id = Uuid::new_v4();
    state.recovery.candidate = Some(Candidate {
        id,
        modified: SystemTime::now(),
    });
    let original = state.project.header.id;
    let _ = update(&mut state, Message::NewProject);
    assert_eq!(state.project.header.id, original);
    let _ = update(&mut state, Message::RestoreRecovery);
    assert!(
        matches!(receiver.try_recv(), Ok(Command::Read(value)) if value == id)
    );
    let mut snapshot = annotated_page().project_snapshot().unwrap();
    let mut second = snapshot.project.pages[0].clone();
    second.id = Uuid::new_v4();
    snapshot.project.pages.push(second);
    snapshot.images.push(None);
    let archive = super::super::project_io::encode_snapshot(snapshot).unwrap();
    let opened = super::super::project_io::prepare_opened(
        archive,
        None,
        "Recovered project.docscan".into(),
    )
    .unwrap();
    let _ = update(
        &mut state,
        Message::Recovery(Event::Restored(id, Ok(Arc::new(opened)))),
    );
    assert!(
        matches!(receiver.try_recv(), Ok(Command::Adopt(value)) if value == id)
    );
    assert!(state.project.is_idle());
    assert!(state.project.path.is_none());
    assert!(state.project.is_dirty(&state.pages));
    assert_eq!(state.pages.len(), 2);
    assert_eq!(state.annotations().len(), 1);
    tick(&mut state);
    assert!(receiver.try_recv().is_err()); // Keep existing recovery until edit.
    let _ = update(&mut state, Message::NewProject);
    let _ = update(&mut state, Message::DiscardProject);
    assert!(matches!(receiver.try_recv(), Ok(Command::Clear)));
}

#[test]
fn failed_restore_can_be_deferred_without_deleting_the_backup() {
    let (mut state, receiver) = connected();
    state.project.operation = Operation::Recovering;
    let id = Uuid::new_v4();
    state.recovery.candidate = Some(Candidate {
        id,
        modified: SystemTime::now(),
    });
    let _ = update(
        &mut state,
        Message::Recovery(Event::Restored(id, Err("Invalid ZIP".into()))),
    );
    assert!(matches!(state.project.operation, Operation::Recovering));
    assert!(state.recovery.candidate.is_some());
    assert!(state.recovery.error.is_some());
    let _ = update(&mut state, Message::DeferRecovery);
    assert!(matches!(receiver.try_recv(), Ok(Command::Defer)));
    assert!(state.project.is_idle());
}

#[test]
fn late_restore_cannot_cancel_a_pending_close() {
    let (mut state, receiver) = connected();
    state.project.operation = Operation::Recovering;
    let id = Uuid::new_v4();
    state.recovery.candidate = Some(Candidate {
        id,
        modified: SystemTime::now(),
    });
    let _ = update(&mut state, Message::RestoreRecovery);
    assert!(matches!(receiver.try_recv(), Ok(Command::Read(_))));
    let window = iced::window::Id::unique();
    let _ = update(&mut state, Message::CloseRequested(window));
    assert!(matches!(receiver.try_recv(), Ok(Command::Close(_))));
    let archive = super::super::project_io::encode_snapshot(
        annotated_page().project_snapshot().unwrap(),
    )
    .unwrap();
    let opened = super::super::project_io::prepare_opened(
        archive,
        None,
        "Recovered project.docscan".into(),
    )
    .unwrap();
    let _ = update(
        &mut state,
        Message::Recovery(Event::Restored(id, Ok(Arc::new(opened)))),
    );
    assert!(matches!(
        state.project.operation,
        Operation::ClosingRecovery
    ));
    assert!(receiver.try_recv().is_err());
}

#[test]
fn normal_close_waits_for_cleanup_acknowledgment_and_failure_keeps_window() {
    let (mut state, receiver) = connected();
    let window = iced::window::Id::unique();
    state.project.operation = Operation::Confirm(LeaveAction::Close(window));
    let _ = update(&mut state, Message::DiscardProject);
    assert!(
        matches!(receiver.try_recv(), Ok(Command::Close(id)) if id == window)
    );
    assert!(matches!(
        state.project.operation,
        Operation::ClosingRecovery
    ));
    let _ = update(
        &mut state,
        Message::Recovery(Event::Closed(window, Err("Access denied".into()))),
    );
    assert!(state.project.is_idle());
    assert!(state.recovery.error.is_some());
    assert!(state.project.is_dirty(&state.pages));
}

#[test]
fn startup_failure_warns_but_does_not_block_explicit_saves() {
    let mut state = annotated_page();
    state.project.operation = Operation::Recovering;
    let _ = update(
        &mut state,
        Message::Recovery(Event::Failed(
            "Recovery directory unavailable".into(),
        )),
    );
    assert!(state.project.is_idle());
    assert!(state.recovery.error.is_some());
    let _ = update(&mut state, Message::SaveProject);
    assert!(matches!(state.project.operation, Operation::Saving { .. }));
}

#[test]
fn worker_orders_write_clear_and_close_without_touching_explicit_file() {
    let directory = std::env::temp_dir()
        .join(format!("fastfill-recovery-worker-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let explicit = directory.join("original.docscan");
    std::fs::write(&explicit, b"Never replace this").unwrap();
    let (events, mut receiver) = mpsc::unbounded();
    let worker_directory = directory.clone();
    let worker = std::thread::spawn(move || {
        worker(&worker_directory, &events, Duration::from_secs(10))
    });
    iced::futures::executor::block_on(async {
        let Event::Ready(sender, candidate) = receiver.next().await.unwrap()
        else {
            panic!("expected ready");
        };
        assert!(candidate.is_none());
        let state = annotated_page();
        let stamp = (state.project.header.id, state.pages.identity());
        sender
            .send(Command::Write(stamp, state.project_snapshot().unwrap()))
            .unwrap();
        sender.send(Command::Clear).unwrap();
        let id = iced::window::Id::unique();
        sender.send(Command::Close(id)).unwrap();
        assert!(matches!(
            receiver.next().await,
            Some(Event::Written(_, Ok(())))
        ));
        assert!(matches!(
            receiver.next().await,
            Some(Event::Cleared(Ok(())))
        ));
        assert!(matches!(
            receiver.next().await,
            Some(Event::Closed(_, Ok(())))
        ));
    });
    worker.join().unwrap().unwrap();
    let store = Store::open(&directory).unwrap();
    assert!(store.candidate().is_none());
    assert_eq!(std::fs::read(&explicit).unwrap(), b"Never replace this");
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn worker_ticks_are_not_starved_by_other_commands() {
    let directory = std::env::temp_dir()
        .join(format!("fastfill-recovery-timer-test-{}", Uuid::new_v4()));
    let (events, mut receiver) = mpsc::unbounded();
    let worker_directory = directory.clone();
    let worker = std::thread::spawn(move || {
        worker(&worker_directory, &events, Duration::from_millis(20))
    });
    let ready = iced::futures::executor::block_on(receiver.next()).unwrap();
    let Event::Ready(sender, _) = ready else {
        panic!("expected ready");
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut ticked = false;
    while Instant::now() < deadline {
        sender.send(Command::Defer).unwrap();
        if matches!(receiver.try_recv(), Ok(Event::Tick)) {
            ticked = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    sender
        .send(Command::Close(iced::window::Id::unique()))
        .unwrap();
    worker.join().unwrap().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    assert!(ticked, "commands must not reset the recovery deadline");
}
