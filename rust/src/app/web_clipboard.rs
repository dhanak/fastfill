//! Async browser clipboard; discard replies after the editor/caret changes.

use super::{FastFill, Message};
use iced::Task;
use iced::widget::text_editor::{self, Action, Edit};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

#[wasm_bindgen(module = "/web/clipboard.mjs")]
extern "C" {
    #[wasm_bindgen(catch, js_name = readClipboard)]
    async fn read_clipboard() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = writeClipboard)]
    async fn write_clipboard(text: &str) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    Copy,
    Cut,
    Paste,
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Request(Kind),
    Finished {
        epoch: u64,
        before: String,
        cursor: text_editor::Cursor,
        result: Result<Option<Action>, String>,
    },
}

pub(super) fn update(state: &mut FastFill, event: Event) -> Task<Message> {
    if !state.annotations().has_text_edit() {
        return Task::none();
    }
    match event {
        Event::Request(kind) => {
            let epoch = state.input_epoch;
            let before = state.text_content.text();
            let cursor = state.text_content.cursor();
            let selection = state.text_content.selection();
            if !matches!(kind, Kind::Paste) && selection.is_none() {
                return Task::none();
            }
            Task::perform(
                async move {
                    let result = match kind {
                        Kind::Paste => read_clipboard().await.map(|text| {
                            Some(Action::Edit(Edit::Paste(
                                std::sync::Arc::new(
                                    text.as_string().unwrap_or_default(),
                                ),
                            )))
                        }),
                        Kind::Copy | Kind::Cut => {
                            write_clipboard(&selection.unwrap()).await.map(
                                |_| {
                                    matches!(kind, Kind::Cut)
                                        .then_some(Action::Edit(Edit::Delete))
                                },
                            )
                        }
                    };
                    result.map_err(|error| {
                        format!(
                            "Clipboard: {}",
                            error.as_string().unwrap_or_else(|| {
                                "Access denied by browser.".into()
                            })
                        )
                    })
                },
                move |result| {
                    Message::BrowserClipboard(Event::Finished {
                        epoch,
                        before: before.clone(),
                        cursor,
                        result,
                    })
                },
            )
        }
        Event::Finished {
            epoch,
            before,
            cursor,
            result,
        } => {
            if state.input_epoch != epoch
                || state.text_content.text() != before
                || state.text_content.cursor() != cursor
            {
                return Task::none();
            }
            match result {
                Ok(Some(action)) => state.edit_text(action),
                Ok(None) => {}
                Err(error) => {
                    state.status = error;
                    state.status_is_error = true;
                }
            }
            Task::none()
        }
    }
}
