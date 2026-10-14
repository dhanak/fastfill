use super::text::measure_text;
use super::{FastFill, ImagePicker, Message, TEXT_EDITOR_ID, page_size};
use crate::picker::{ImportedImage, pick_image};
use fastfill_rust::annotations::{Annotations, DragModifiers, Pointer, Tool};
use fastfill_rust::geometry::Point as PagePoint;
use iced::widget::text_editor;
use iced::{Task, keyboard};

pub(super) fn shortcut_message(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<Message> {
    match key {
        keyboard::Key::Character(value)
            if modifiers.command()
                && modifiers.shift()
                && value.eq_ignore_ascii_case("z") =>
        {
            Some(Message::Redo)
        }
        keyboard::Key::Character(value)
            if modifiers.command() && value.eq_ignore_ascii_case("z") =>
        {
            Some(Message::Undo)
        }
        keyboard::Key::Character(value)
            if modifiers.command() && value.eq_ignore_ascii_case("y") =>
        {
            Some(Message::Redo)
        }
        keyboard::Key::Named(keyboard::key::Named::Delete) => {
            Some(Message::DeleteSelected)
        }
        keyboard::Key::Named(keyboard::key::Named::Escape) => {
            Some(Message::CancelInteraction)
        }
        _ => None,
    }
}

impl FastFill {
    pub(super) fn cancel_interaction(&mut self) {
        self.annotations.cancel();
        self.input_epoch = self.input_epoch.wrapping_add(1);
    }

    fn pick_image(&mut self) -> Task<Message> {
        self.image_picker = ImagePicker::Picking;
        self.status = "Selecting an image…".into();
        self.status_is_error = false;
        Task::perform(pick_image(), Message::ImagePicked)
    }

    fn accept_text(&mut self) {
        self.sync_text();
        if self.annotations.accept_text() {
            self.input_epoch = self.input_epoch.wrapping_add(1);
        }
    }

    fn sync_text(&mut self) {
        let Some((_, _, format, scale)) = self.annotations.text_edit() else {
            return;
        };
        let content = self.text_content.text();
        let size = measure_text(&content, format, scale);
        self.annotations.update_text(content, size);
    }

    fn set_text_format(&mut self, format: super::TextFormat) {
        if self.annotations.has_text_edit() {
            if self.annotations.set_text_format(format) {
                self.sync_text();
            }
        } else if let Some(annotation) =
            self.annotations.selected_text_annotation()
        {
            let size = measure_text(&annotation.text, format, 1.0);
            self.annotations.set_selected_text_format(format, size);
        } else if self.annotations.set_text_format(format) {
            self.sync_text();
        }
    }

    fn start_text_editor(&mut self) -> Task<Message> {
        let Some((content, ..)) = self.annotations.text_edit() else {
            return Task::none();
        };
        self.text_content = text_editor::Content::with_text(content);
        iced::widget::operation::focus(TEXT_EDITOR_ID)
    }

    fn edit_text(&mut self, action: text_editor::Action) {
        self.text_content.perform(action);
        self.sync_text();
    }

    fn finish_interaction(
        &mut self,
        pointer: Pointer,
        point: PagePoint,
        square: bool,
    ) -> Task<Message> {
        self.annotations.finish(pointer, point, square);
        if self.annotations.has_text_edit() {
            self.start_text_editor()
        } else {
            Task::none()
        }
    }

    fn undo(&mut self) {
        if self.image_picker == ImagePicker::Idle && self.annotations.undo() {
            self.input_epoch = self.input_epoch.wrapping_add(1);
        }
    }

    fn redo(&mut self) {
        if self.image_picker == ImagePicker::Idle && self.annotations.redo() {
            self.input_epoch = self.input_epoch.wrapping_add(1);
        }
    }

    fn reset_rotation(&mut self) {
        if self.image_picker == ImagePicker::Idle {
            self.annotations.reset_rotation();
        }
    }

    fn adjust_thickness(&mut self, delta: f32) {
        if let Some(style) = self.annotations.current_style() {
            self.annotations.set_stroke_width(
                (style.stroke_width + delta).clamp(1.0, 20.0),
            );
        }
    }

    fn ask_delete_all(&mut self) {
        self.accept_text();
        if self.image_picker == ImagePicker::Idle
            && self.annotations.active_pointer().is_none()
            && !self.annotations.is_empty()
        {
            self.confirm_delete_all = true;
        }
    }

    fn begin_interaction(
        &mut self,
        pointer: Pointer,
        point: PagePoint,
        scale: f32,
    ) {
        if self.image_picker == ImagePicker::Idle {
            self.annotations.begin_interaction(
                pointer,
                point,
                page_size(self),
                scale,
                self.modifiers,
            );
        }
    }

    fn image_picked(&mut self, result: Result<Option<ImportedImage>, String>) {
        self.image_picker = ImagePicker::Idle;
        match result {
            Ok(Some(image)) => {
                self.cancel_interaction();
                self.status = format!(
                    "Opened {} ({} × {} px)",
                    image.name, image.size.width, image.size.height,
                );
                self.page = Some(image);
                self.navigation.fit();
                let tool = self.annotations.tool();
                self.annotations = Annotations::default();
                self.annotations.set_tool(tool);
                self.status_is_error = false;
            }
            Ok(None) => {
                self.status = "Open canceled.".into();
                self.status_is_error = false;
            }
            Err(error) => {
                self.status = error;
                self.status_is_error = true;
            }
        }
    }
}

pub(super) fn update(state: &mut FastFill, message: Message) -> Task<Message> {
    if handle_delete_confirmation(state, &message)
        || handle_navigation(state, &message)
        || handle_text_format(state, &message)
    {
        return Task::none();
    }
    match message {
        Message::OpenImage => {
            if state.image_picker == ImagePicker::Idle {
                state.accept_text();
                state.cancel_interaction();
                if state.annotations.is_empty() {
                    return state.pick_image();
                }
                state.image_picker = ImagePicker::ConfirmReplace;
            }
        }
        Message::ReplacePage => {
            if state.image_picker == ImagePicker::ConfirmReplace {
                return state.pick_image();
            }
        }
        Message::KeepPage => state.image_picker = ImagePicker::Idle,
        Message::ImagePicked(result) => state.image_picked(result),
        Message::SetTool(tool) => {
            state.accept_text();
            state.cancel_interaction();
            state.annotations.set_tool(tool);
        }
        Message::Undo => state.undo(),
        Message::Redo => state.redo(),
        Message::ResetRotation => state.reset_rotation(),
        Message::DeleteSelected => {
            if state.image_picker == ImagePicker::Idle {
                state.annotations.delete_selected();
            }
        }
        Message::AskDeleteAll => state.ask_delete_all(),
        Message::SendToBack => {
            if state.image_picker == ImagePicker::Idle {
                state.annotations.send_selected_to_back();
            }
        }
        Message::BringToFront => {
            if state.image_picker == ImagePicker::Idle {
                state.annotations.bring_selected_to_front();
            }
        }
        Message::SetColor(color) => {
            state.annotations.set_color(color);
        }
        Message::AdjustThickness(delta) => state.adjust_thickness(delta),
        Message::ToggleFill => {
            if let Some(filled) = state.annotations.current_fill() {
                state.annotations.set_filled(!filled);
            }
        }
        Message::TextAction(action) => state.edit_text(action),
        Message::AcceptText => state.accept_text(),
        Message::DrawPressed(pointer, point, scale) => {
            return draw_pressed(state, pointer, point, scale);
        }
        Message::DrawMoved(pointer, point, square) => {
            state.annotations.drag(pointer, point, square);
        }
        Message::DrawReleased(pointer, point, square) => {
            return state.finish_interaction(pointer, point, square);
        }
        Message::ModifiersChanged(modifiers) => {
            state.modifiers = modifiers;
            state.annotations.constrain_square(modifiers.square);
        }
        Message::CancelInteraction => {
            state.cancel_interaction();
            if state.image_picker == ImagePicker::ConfirmReplace {
                state.image_picker = ImagePicker::Idle;
            }
        }
        Message::FocusLost => {
            state.accept_text();
            state.cancel_interaction();
            state.modifiers = DragModifiers::default();
        }
        Message::ConfirmDeleteAll
        | Message::KeepAll
        | Message::Fit
        | Message::OneToOne(_)
        | Message::Pan(_)
        | Message::ZoomAt { .. }
        | Message::TouchGesture { .. }
        | Message::SetTextTypeface(_)
        | Message::SetTextSize(_)
        | Message::ToggleTextBold
        | Message::ToggleTextItalic
        | Message::ToggleTextUnderline => {}
    }

    Task::none()
}

fn handle_text_format(state: &mut FastFill, message: &Message) -> bool {
    let Some(mut format) = state.annotations.current_text_format() else {
        return false;
    };
    match message {
        Message::SetColor(color) => format.color = *color,
        Message::SetTextTypeface(typeface) => format.typeface = *typeface,
        Message::SetTextSize(font_size) => format.font_size = *font_size,
        Message::ToggleTextBold => format.bold = !format.bold,
        Message::ToggleTextItalic => format.italic = !format.italic,
        Message::ToggleTextUnderline => format.underline = !format.underline,
        _ => return false,
    }
    state.set_text_format(format);
    true
}

fn draw_pressed(
    state: &mut FastFill,
    pointer: Pointer,
    point: PagePoint,
    scale: f32,
) -> Task<Message> {
    if state.image_picker != ImagePicker::Idle {
        return Task::none();
    }
    state.accept_text();
    match state.annotations.tool() {
        Tool::Text => {
            if !state.annotations.begin_text_edit(
                point,
                page_size(state),
                scale,
            ) {
                state.annotations.begin_text(point, page_size(state), scale);
            }
            return state.start_text_editor();
        }
        Tool::Select
            if state.annotations.begin_text_move(
                pointer,
                point,
                page_size(state),
                scale,
            ) => {}
        _ => state.begin_interaction(pointer, point, scale),
    }
    Task::none()
}

fn handle_delete_confirmation(state: &mut FastFill, message: &Message) -> bool {
    if !state.confirm_delete_all {
        return false;
    }
    match message {
        Message::ConfirmDeleteAll => {
            state.confirm_delete_all = false;
            state.annotations.delete_all();
        }
        Message::KeepAll | Message::CancelInteraction => {
            state.confirm_delete_all = false;
        }
        Message::FocusLost => {
            state.confirm_delete_all = false;
            state.cancel_interaction();
            state.modifiers = DragModifiers::default();
        }
        _ => {}
    }
    true
}

fn handle_navigation(state: &mut FastFill, message: &Message) -> bool {
    match message {
        Message::Fit => {
            state.accept_text();
            state.cancel_interaction();
            state.navigation.fit();
        }
        Message::OneToOne(viewport) => {
            state.accept_text();
            state.cancel_interaction();
            state.navigation.one_to_one(page_size(state), *viewport);
        }
        Message::Pan(delta) => {
            state.accept_text();
            state.annotations.cancel();
            state.navigation.pan_by(*delta);
        }
        Message::ZoomAt {
            factor,
            anchor,
            viewport,
        } => {
            state.accept_text();
            state.annotations.cancel();
            state.navigation.zoom_at(
                *factor,
                *anchor,
                page_size(state),
                *viewport,
            );
        }
        Message::TouchGesture { delta, viewport } => {
            state.accept_text();
            state.annotations.cancel();
            state.navigation.zoom_at(
                delta.zoom_factor,
                delta.zoom_anchor,
                page_size(state),
                *viewport,
            );
            state.navigation.pan_by(delta.pan);
        }
        _ => return false,
    }
    true
}
