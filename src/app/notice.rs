//! Dismissible notifications; deadlines use the native/browser monotonic clock.

use super::{FastFill, Message};
use iced::time::Instant;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct Notice {
    pub(super) message: String,
    pub(super) error: bool,
    shown: Instant,
}

pub(super) fn warning(state: &mut FastFill, message: String) {
    state.notice = Some(Notice {
        message,
        error: false,
        shown: Instant::now(),
    });
}

pub(super) fn synchronize(state: &mut FastFill) {
    synchronize_at(state, Instant::now());
}

fn synchronize_at(state: &mut FastFill, now: Instant) {
    if state.status_is_error && !state.status.is_empty() {
        if state.notice.as_ref().is_none_or(|notice| {
            !notice.error || notice.message != state.status
        }) {
            state.notice = Some(Notice {
                message: state.status.clone(),
                error: true,
                shown: now,
            });
        }
    } else if state.notice.as_ref().is_some_and(|notice| notice.error) {
        state.notice = None;
    }
}

pub(super) fn update(state: &mut FastFill, message: &Message) -> bool {
    match message {
        Message::DismissNotice => dismiss(state),
        Message::NoticeTick(now) => {
            if state.notice.as_ref().is_some_and(|notice| {
                now.saturating_duration_since(notice.shown) >= TIMEOUT
            }) {
                dismiss(state);
            }
        }
        _ => return false,
    }
    true
}

fn dismiss(state: &mut FastFill) {
    if let Some(notice) = state.notice.take()
        && notice.error
        && state.status_is_error
        && state.status == notice.message
    {
        state.status.clear();
        state.status_is_error = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_dismiss_manually_and_timeout_only_the_current_notice() {
        let mut state = FastFill::default();
        let start = Instant::now();
        state.status = "First error".into();
        state.status_is_error = true;
        synchronize_at(&mut state, start);
        state.status = "Newer error".into();
        synchronize_at(&mut state, start + Duration::from_secs(8));
        update(&mut state, &Message::NoticeTick(start + TIMEOUT));
        assert!(state.notice.is_some());
        update(&mut state, &Message::DismissNotice);
        assert!(state.notice.is_none());
        assert!(!state.status_is_error);
        assert_eq!(state.status, "");
        state.status = "Third error".into();
        state.status_is_error = true;
        synchronize_at(&mut state, start);
        update(&mut state, &Message::NoticeTick(start + TIMEOUT));
        assert!(state.notice.is_none());
        assert!(!state.status_is_error);
    }
}
