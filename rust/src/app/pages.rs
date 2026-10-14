use super::{FastFill, ImagePicker, Message, PickPurpose};
use crate::picker::ImportedImage;
use crate::picker::pick_image;
use fastfill_rust::annotations::Annotations;
use fastfill_rust::geometry::Size;
use fastfill_rust::image_input::ImageFormatKind;
use fastfill_rust::project::{
    CropQuad, PageFilterSettings, ProjectPage, decode_editor_page,
    editor_page_size,
};
use iced::Task;
use uuid::Uuid;

pub(super) const MAXIMUM_PAGES: usize = 20;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(super) struct PageDrag {
    pub(super) source: usize,
    pub(super) target: usize,
}

#[derive(Debug)]
pub(super) struct EditorPage {
    pub(super) image: Option<ImportedImage>,
    pub(super) annotations: Annotations,
    pub(super) metadata: ProjectPage,
    pub(super) size: Size,
}

impl Default for EditorPage {
    fn default() -> Self {
        Self {
            image: None,
            annotations: Annotations::default(),
            metadata: page_metadata(794, 1_123, "png"),
            size: super::DEFAULT_PAGE,
        }
    }
}

impl EditorPage {
    fn from_image(image: ImportedImage) -> Self {
        let extension = match image.info.format {
            ImageFormatKind::Png => "png",
            ImageFormatKind::Jpeg => "jpg",
        };
        let metadata =
            page_metadata(image.info.width, image.info.height, extension);
        let size = editor_page_size(&metadata).expect("validated image size");
        Self {
            image: Some(image),
            annotations: Annotations::default(),
            metadata,
            size,
        }
    }
}

fn page_metadata(width: u32, height: u32, extension: &str) -> ProjectPage {
    let id = Uuid::new_v4();
    ProjectPage {
        id,
        source_asset: format!("{id}.{extension}"),
        source_pixel_width: i32::try_from(width).expect("bounded image width"),
        source_pixel_height: i32::try_from(height)
            .expect("bounded image height"),
        crop: CropQuad::default(),
        rotation_quarter_turns: 0,
        filter: PageFilterSettings::default(),
        annotations: Vec::new(),
    }
}

#[derive(Debug)]
pub(super) struct Pages {
    items: Vec<EditorPage>,
    active: usize,
}

impl Default for Pages {
    fn default() -> Self {
        Self {
            items: vec![EditorPage::default()],
            active: 0,
        }
    }
}

impl Pages {
    pub(super) fn identity(&self) -> Vec<(Uuid, u64)> {
        self.items
            .iter()
            .map(|page| (page.metadata.id, page.annotations.revision()))
            .collect()
    }

    pub(super) fn from_opened(
        opened: &super::project_io::OpenedProject,
    ) -> Result<Self, String> {
        if opened.project.pages.len() != opened.images.len() {
            return Err("Page image count does not match the document.".into());
        }
        let items = opened
            .project
            .pages
            .iter()
            .zip(&opened.images)
            .map(|(metadata, image)| {
                Ok(EditorPage {
                    image: Some(image.clone()),
                    annotations: decode_editor_page(metadata)
                        .map_err(|error| error.to_string())?,
                    size: editor_page_size(metadata)
                        .map_err(|error| error.to_string())?,
                    metadata: metadata.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if items.is_empty() {
            return Ok(Self::default());
        }
        Ok(Self { items, active: 0 })
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = &EditorPage> {
        self.items.iter()
    }
    pub(super) fn current(&self) -> &EditorPage {
        &self.items[self.active]
    }

    pub(super) fn current_mut(&mut self) -> &mut EditorPage {
        &mut self.items[self.active]
    }

    pub(super) fn len(&self) -> usize {
        self.items.len()
    }

    pub(super) const fn active_index(&self) -> usize {
        self.active
    }

    pub(super) fn can_add(&self) -> bool {
        self.items.len() < MAXIMUM_PAGES
    }

    pub(super) fn add_image(&mut self, image: ImportedImage) -> bool {
        if !self.can_add() {
            return false;
        }
        if self.items.len() == 1
            && self.current().image.is_none()
            && self.current().annotations.is_empty()
        {
            self.replace_image(image);
            return true;
        }

        let settings = self.current().annotations.settings();
        let mut page = EditorPage::from_image(image);
        page.annotations.apply_settings(&settings);
        self.active += 1;
        self.items.insert(self.active, page);
        true
    }

    pub(super) fn replace_image(&mut self, image: ImportedImage) {
        let settings = self.current().annotations.settings();
        let mut page = EditorPage::from_image(image);
        page.annotations.apply_settings(&settings);
        self.items[self.active] = page;
    }

    pub(super) fn select(&mut self, index: usize) -> bool {
        if index >= self.items.len() || index == self.active {
            return false;
        }
        let settings = self.current().annotations.settings();
        self.active = index;
        self.current_mut().annotations.apply_settings(&settings);
        true
    }

    pub(super) fn move_active_to(&mut self, index: usize) -> bool {
        if index >= self.items.len() || index == self.active {
            return false;
        }
        let page = self.items.remove(self.active);
        self.items.insert(index, page);
        self.active = index;
        true
    }

    pub(super) fn delete_active(&mut self) {
        let settings = self.current().annotations.settings();
        if self.items.len() == 1 {
            self.items[0] = EditorPage::default();
        } else {
            self.items.remove(self.active);
            self.active = self.active.min(self.items.len() - 1);
        }
        self.current_mut().annotations.apply_settings(&settings);
    }

    #[cfg(test)]
    pub(super) fn names(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter_map(|page| {
                page.image.as_ref().map(|image| image.name.as_str())
            })
            .collect()
    }
}

impl FastFill {
    fn add_page(&mut self) -> Task<Message> {
        self.accept_text();
        self.cancel_interaction();
        if self.pages.len() == 1
            && self.image().is_none()
            && !self.annotations().is_empty()
        {
            self.image_picker = ImagePicker::ConfirmReplace;
        } else if self.pages.can_add() {
            return self.pick_image(PickPurpose::Add);
        } else {
            self.page_limit_error();
        }
        Task::none()
    }

    fn pick_image(&mut self, purpose: PickPurpose) -> Task<Message> {
        self.accept_text();
        self.cancel_interaction();
        self.image_picker = ImagePicker::Picking(purpose);
        self.status = "Selecting an image…".into();
        self.status_is_error = false;
        Task::perform(pick_image(self.image_directory.clone()), move |result| {
            Message::ImagePicked(purpose, result)
        })
    }

    fn image_picked(
        &mut self,
        purpose: PickPurpose,
        result: Result<Option<ImportedImage>, String>,
    ) {
        if self.image_picker != ImagePicker::Picking(purpose) {
            return;
        }
        self.image_picker = ImagePicker::Idle;
        match result {
            Ok(Some(image)) => {
                if let Some(directory) = &image.directory {
                    self.image_directory = Some(directory.clone());
                }
                self.status = format!(
                    "Opened {} ({} × {} px)",
                    image.name, image.size.width, image.size.height,
                );
                let changed = match purpose {
                    PickPurpose::Add => self.pages.add_image(image),
                    PickPurpose::Replace => {
                        self.pages.replace_image(image);
                        true
                    }
                };
                if changed {
                    self.navigation.fit();
                    self.page_scroll =
                        super::rail::slot_top(self.pages.active_index());
                    self.input_epoch = self.input_epoch.wrapping_add(1);
                    self.status_is_error = false;
                } else {
                    self.page_limit_error();
                }
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

    fn page_limit_error(&mut self) {
        self.status = "A project may contain at most 20 pages.".into();
        self.status_is_error = true;
    }

    fn select_page(&mut self, index: usize) {
        if index < self.pages.len() && index != self.pages.active_index() {
            self.accept_text();
            self.cancel_interaction();
            self.pages.select(index);
            self.navigation.fit();
            self.status = format!("Selected page {}.", index + 1);
            self.status_is_error = false;
        }
    }

    fn move_page(&mut self, target: usize) {
        if target < self.pages.len() && target != self.pages.active_index() {
            self.accept_text();
            self.cancel_interaction();
            self.pages.move_active_to(target);
            self.page_scroll = super::rail::slot_top(target);
            self.status = format!("Page moved to position {}.", target + 1);
            self.status_is_error = false;
        }
    }
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if let Message::ImagePicked(purpose, result) = message {
        state.image_picked(*purpose, result.clone());
        return Some(Task::none());
    }
    if state.image_picker != ImagePicker::Idle || state.confirm_delete_page {
        match message {
            Message::ConfirmReplacePage
                if state.image_picker == ImagePicker::ConfirmReplace =>
            {
                return Some(state.pick_image(PickPurpose::Replace));
            }
            Message::ConfirmDeletePage if state.confirm_delete_page => {
                state.pages.delete_active();
                state.confirm_delete_page = false;
                state.navigation.fit();
                state.page_scroll =
                    super::rail::slot_top(state.pages.active_index());
                state.cancel_interaction();
                state.status = "Page deleted.".into();
                state.status_is_error = false;
            }
            Message::KeepPage | Message::CancelInteraction
                if !matches!(state.image_picker, ImagePicker::Picking(_)) =>
            {
                state.image_picker = ImagePicker::Idle;
                state.confirm_delete_page = false;
            }
            Message::FocusLost => {
                state.cancel_interaction();
                state.modifiers = super::DragModifiers::default();
            }
            _ => {}
        }
        return Some(Task::none());
    }
    // Annotation confirmation owns input until dismissed.
    if state.confirm_delete_all {
        return None;
    }
    match message {
        Message::AddPage => return Some(state.add_page()),
        Message::AskReplacePage => {
            state.accept_text();
            state.cancel_interaction();
            if state.annotations().is_empty() {
                return Some(state.pick_image(PickPurpose::Replace));
            }
            state.image_picker = ImagePicker::ConfirmReplace;
        }
        Message::SelectPage(index) => state.select_page(*index),
        Message::MovePageLeft => {
            if let Some(index) = state.pages.active_index().checked_sub(1) {
                state.move_page(index);
            }
        }
        Message::MovePageRight => {
            state.move_page(state.pages.active_index() + 1);
        }
        Message::PageDragStarted(index) if *index < state.pages.len() => {
            state.select_page(*index);
            state.accept_text();
            state.cancel_interaction();
            state.dragged_page = Some(PageDrag {
                source: *index,
                target: *index,
            });
        }
        Message::PageDragEntered(index) if *index < state.pages.len() => {
            if let Some(drag) = &mut state.dragged_page {
                drag.target = *index;
            }
        }
        Message::PageDragFinished(target) => {
            if state.dragged_page.take().is_some() {
                state.move_page(*target);
            }
        }
        Message::PageDragCanceled => state.dragged_page = None,
        Message::ScrollPages(offset) => {
            if offset.is_finite() {
                state.page_scroll = offset.max(0.0);
            }
        }
        Message::AskDeletePage => {
            state.accept_text();
            state.cancel_interaction();
            state.confirm_delete_page = true;
        }
        Message::KeepPage
        | Message::ConfirmDeletePage
        | Message::ConfirmReplacePage => {}
        _ => return None,
    }
    Some(Task::none())
}
