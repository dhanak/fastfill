use super::text::measure_text;
use super::{FastFill, ImagePicker, Message, TEXT_EDITOR_ID, page_size};
use fastfill_rust::annotations::{DragModifiers, Pointer, Tool};
use fastfill_rust::geometry::Point as PagePoint;
use iced::widget::text_editor;
use iced::{Task, keyboard};

pub(super) fn shortcut_message(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
    status: iced::event::Status,
) -> Option<Message> {
    // Focused text fields own Delete; Escape still cancels their numeric draft.
    if status == iced::event::Status::Captured
        && key == keyboard::Key::Named(keyboard::key::Named::Delete)
    {
        return None;
    }
    match key {
        keyboard::Key::Named(keyboard::key::Named::F1) => {
            Some(Message::ShowShortcuts)
        }
        keyboard::Key::Character(value)
            if modifiers.command() && value.eq_ignore_ascii_case("n") =>
        {
            Some(Message::NewProject)
        }
        keyboard::Key::Character(value)
            if modifiers.command() && value.eq_ignore_ascii_case("o") =>
        {
            Some(Message::OpenProject)
        }
        keyboard::Key::Character(value)
            if modifiers.command() && value.eq_ignore_ascii_case("s") =>
        {
            Some(if modifiers.shift() {
                Message::SaveProjectAs
            } else {
                Message::SaveProject
            })
        }
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
        self.clear_font_size_input();
        self.annotations_mut().cancel();
        self.dragged_page = None;
        self.input_epoch = self.input_epoch.wrapping_add(1);
    }

    pub(super) fn accept_text(&mut self) {
        self.finish_font_size_input();
        self.sync_text();
        if self.annotations_mut().accept_text() {
            self.input_epoch = self.input_epoch.wrapping_add(1);
        }
    }

    fn sync_text(&mut self) {
        let Some((_, _, format, scale)) = self.annotations().text_edit() else {
            return;
        };
        let content = self.text_content.text();
        let size = measure_text(&content, format, scale);
        self.annotations_mut().update_text(content, size);
    }

    fn start_text_editor(
        &mut self,
        point: PagePoint,
        at_end: bool,
    ) -> Task<Message> {
        let Some((content, _, _, scale)) = self.annotations().text_edit()
        else {
            return Task::none();
        };
        self.text_content = text_editor::Content::with_text(content);
        if at_end {
            self.text_content.perform(text_editor::Action::Move(
                text_editor::Motion::DocumentEnd,
            ));
        } else if let Some(annotation) = self.annotations().text_preview() {
            self.text_content.move_to(super::text::cursor_at(
                &annotation,
                point,
                scale,
            ));
        }
        iced::widget::operation::focus(TEXT_EDITOR_ID)
    }

    fn edit_text(&mut self, action: text_editor::Action) {
        self.finish_font_size_input();
        self.text_content.perform(action);
        self.sync_text();
    }

    fn undo(&mut self) {
        self.change_history(false);
    }

    fn redo(&mut self) {
        self.change_history(true);
    }

    fn change_history(&mut self, redo: bool) {
        self.clear_font_size_input();
        if self.image_picker == ImagePicker::Idle {
            let old_size = self.pages.current().size;
            match self.pages.current_mut().history(redo) {
                Ok(true) => {
                    self.input_epoch = self.input_epoch.wrapping_add(1);
                    if self.pages.current().size != old_size {
                        self.navigation.fit();
                    }
                }
                Ok(false) => {}
                Err(error) => {
                    self.status = error;
                    self.status_is_error = true;
                }
            }
        }
    }

    fn reset_rotation(&mut self) {
        if self.image_picker == ImagePicker::Idle {
            self.annotations_mut().reset_rotation();
        }
    }

    fn adjust_thickness(&mut self, delta: f32) {
        if let Some(style) = self.annotations().current_style() {
            self.annotations_mut().set_stroke_width(
                (style.stroke_width + delta).clamp(1.0, 20.0),
            );
        }
    }

    fn ask_delete_all(&mut self) {
        self.accept_text();
        if self.image_picker == ImagePicker::Idle
            && self.annotations().active_pointer().is_none()
            && !self.annotations().is_empty()
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
            let size = page_size(self);
            let modifiers = self.modifiers;
            if self
                .annotations_mut()
                .begin_interaction(pointer, point, size, scale, modifiers)
            {
                super::snap::place_mark(self, pointer, point, scale);
            }
        }
    }
}

pub(super) fn update(state: &mut FastFill, message: Message) -> Task<Message> {
    if let Message::SystemThemeChanged(mode) = &message {
        // Iced 0.14 leaves Linux's previous theme when the portal resets it.
        // Override only that case; native theme following owns every other.
        state.theme_override = (cfg!(target_os = "linux")
            && *mode == iced::theme::Mode::None)
            .then(|| <iced::Theme as iced::theme::Base>::default(*mode));
        return Task::none();
    }
    if let Some(task) = super::help::update(state, &message) {
        return task;
    }
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    if let Some(task) = super::camera::update(state, &message) {
        return task;
    }
    if super::snap::update(state, &message) {
        return super::snap::synchronize(state);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(task) = super::recovery::update(state, &message) {
        return Task::batch([task, super::snap::synchronize(state)]);
    }
    let task = update_editor(state, message);
    state.pages.current_mut().prune_versions();
    #[cfg(not(target_arch = "wasm32"))]
    super::recovery::synchronize(state);
    Task::batch([task, super::snap::synchronize(state)])
}

fn update_editor(state: &mut FastFill, message: Message) -> Task<Message> {
    if let Some(task) = super::page_edit::update(state, &message) {
        return task;
    }
    if let Some(task) = super::export::update(state, &message) {
        return task;
    }
    if let Some(task) = super::pdf::update(state, &message) {
        return task;
    }
    if let Some(task) = super::project::update(state, &message) {
        return task;
    }
    if let Some(task) = super::pages::update(state, &message) {
        return task;
    }
    if handle_delete_confirmation(state, &message) {
        return Task::none();
    }
    if let Some(task) = super::text_controls::update(state, &message) {
        return task;
    }
    if handle_navigation(state, &message) || handle_text_format(state, &message)
    {
        return Task::none();
    }
    match message {
        Message::SetTool(tool) => {
            state.accept_text();
            state.cancel_interaction();
            state.annotations_mut().set_tool(tool);
        }
        Message::Undo => state.undo(),
        Message::Redo => state.redo(),
        Message::ResetRotation => state.reset_rotation(),
        Message::DeleteSelected => {
            if state.image_picker == ImagePicker::Idle {
                state.annotations_mut().delete_selected();
            }
        }
        Message::AskDeleteAll => state.ask_delete_all(),
        Message::SendToBack => {
            if state.image_picker == ImagePicker::Idle {
                state.annotations_mut().send_selected_to_back();
            }
        }
        Message::BringToFront => {
            if state.image_picker == ImagePicker::Idle {
                state.annotations_mut().bring_selected_to_front();
            }
        }
        Message::SetColor(color) => {
            state.annotations_mut().set_color(color);
        }
        Message::AdjustThickness(delta) => state.adjust_thickness(delta),
        Message::ToggleFill => {
            if let Some(filled) = state.annotations().current_fill() {
                state.annotations_mut().set_filled(!filled);
            }
        }
        Message::TextAction(action) => state.edit_text(action),
        Message::AcceptText => state.accept_text(),
        Message::DrawPressed(pointer, point, scale) => {
            return draw_pressed(state, pointer, point, scale);
        }
        Message::DrawMoved(pointer, point, square) => {
            state.annotations_mut().drag(pointer, point, square);
        }
        Message::DrawReleased(pointer, point, square) => {
            state.annotations_mut().finish(pointer, point, square);
        }
        Message::ModifiersChanged(modifiers) => {
            state.modifiers = modifiers;
            state.annotations_mut().constrain_square(modifiers.square);
        }
        Message::CancelInteraction | Message::ViewportResized => {
            let deselect = matches!(message, Message::CancelInteraction)
                && state.annotations().active_pointer().is_none()
                && !state.annotations().has_text_edit()
                && state.dragged_page.is_none();
            state.cancel_interaction();
            if deselect {
                state.annotations_mut().clear_selection();
            }
            if state.image_picker == ImagePicker::ConfirmReplace {
                state.image_picker = ImagePicker::Idle;
            }
        }
        Message::FocusLost => {
            state.accept_text();
            state.cancel_interaction();
            state.modifiers = DragModifiers::default();
        }
        // Page, project, navigation, and format handlers own these messages.
        _ => {}
    }

    Task::none()
}

pub(super) fn handle_text_format(
    state: &mut FastFill,
    message: &Message,
) -> bool {
    match message {
        Message::SetTextSize(_) => state.clear_font_size_input(),
        Message::SetTextTypeface(_)
        | Message::SetColor(_)
        | Message::ToggleTextBold
        | Message::ToggleTextItalic
        | Message::ToggleTextUnderline => state.finish_font_size_input(),
        _ => return false,
    }
    let Some(current) = state.annotations().current_text_format() else {
        return false;
    };
    // A mixed group's shared color belongs to both text and shapes.
    if matches!(message, Message::SetColor(_))
        && state.annotations().tool() == Tool::Select
        && state.annotations().selected_count() > 1
    {
        return false;
    }
    let Some(format) = changed_text_format(message, current, current) else {
        return false;
    };
    if state.annotations().has_text_edit()
        || state.annotations().selected_text_indices().is_empty()
    {
        if state.annotations_mut().set_text_format(format) {
            state.sync_text();
        }
    } else {
        state
            .annotations_mut()
            .update_selected_text_formats(|text| {
                let format = changed_text_format(message, text.format, current)
                    .expect("validated format command");
                (format, measure_text(&text.text, format, 1.0))
            });
    }
    true
}

fn changed_text_format(
    message: &Message,
    mut format: super::TextFormat,
    selected: super::TextFormat,
) -> Option<super::TextFormat> {
    match message {
        Message::SetColor(color) => format.color = *color,
        Message::SetTextTypeface(typeface) => format.typeface = *typeface,
        Message::SetTextSize(font_size) => format.font_size = *font_size,
        Message::ToggleTextBold => format.bold = !selected.bold,
        Message::ToggleTextItalic => format.italic = !selected.italic,
        Message::ToggleTextUnderline => format.underline = !selected.underline,
        _ => return None,
    }
    Some(format)
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
    let size = page_size(state);
    match state.annotations().tool() {
        Tool::Text => {
            let at_end =
                !state.annotations_mut().begin_text_edit(point, size, scale)
                    && super::snap::begin_text(state, point, scale);
            return state.start_text_editor(point, at_end);
        }
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
            state.annotations_mut().delete_all();
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
            let pixels =
                state.image().map_or(page_size(state), |image| image.size);
            state.navigation.one_to_one(pixels, *viewport);
        }
        Message::Pan(delta) => {
            state.accept_text();
            state.annotations_mut().cancel();
            state.navigation.pan_by(*delta);
        }
        Message::ZoomAt {
            factor,
            anchor,
            viewport,
        } => {
            state.accept_text();
            state.annotations_mut().cancel();
            state.navigation.zoom_at(
                *factor,
                *anchor,
                page_size(state),
                *viewport,
            );
        }
        Message::TouchGesture { delta, viewport } => {
            state.accept_text();
            state.annotations_mut().cancel();
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
