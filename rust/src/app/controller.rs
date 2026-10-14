use super::{FastFill, ImagePicker, Message, page_size};
use crate::picker::{ImportedImage, pick_image};
use fastfill_rust::annotations::{Annotations, DragModifiers, Pointer};
use fastfill_rust::geometry::Point as PagePoint;
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
    {
        return Task::none();
    }
    match message {
        Message::OpenImage => {
            if state.image_picker == ImagePicker::Idle {
                state.cancel_interaction();
                if state.annotations.shapes().is_empty() {
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
        Message::AskDeleteAll => {
            if state.image_picker == ImagePicker::Idle
                && state.annotations.active_pointer().is_none()
                && !state.annotations.shapes().is_empty()
            {
                state.confirm_delete_all = true;
            }
        }
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
        Message::DrawPressed(pointer, point, scale) => {
            state.begin_interaction(pointer, point, scale);
        }
        Message::DrawMoved(pointer, point, square) => {
            state.annotations.drag(pointer, point, square);
        }
        Message::DrawReleased(pointer, point, square) => {
            state.annotations.finish(pointer, point, square);
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
            state.cancel_interaction();
            state.modifiers = DragModifiers::default();
        }
        Message::ConfirmDeleteAll
        | Message::KeepAll
        | Message::Fit
        | Message::OneToOne(_)
        | Message::Pan(_)
        | Message::ZoomAt { .. }
        | Message::TouchGesture { .. } => {}
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
            state.cancel_interaction();
            state.navigation.fit();
        }
        Message::OneToOne(viewport) => {
            state.cancel_interaction();
            state.navigation.one_to_one(page_size(state), *viewport);
        }
        Message::Pan(delta) => {
            state.annotations.cancel();
            state.navigation.pan_by(*delta);
        }
        Message::ZoomAt {
            factor,
            anchor,
            viewport,
        } => {
            state.annotations.cancel();
            state.navigation.zoom_at(
                *factor,
                *anchor,
                page_size(state),
                *viewport,
            );
        }
        Message::TouchGesture { delta, viewport } => {
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
