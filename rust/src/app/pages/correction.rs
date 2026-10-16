//! Accepted page versions share annotation history through opaque context keys.

use super::EditorPage;
use fastfill_rust::correction::{
    PageCorrection, PageMapping, corrected_dimensions,
};
use fastfill_rust::geometry::Size;
use fastfill_rust::project::{ProjectPage, encode_corrected_editor_page};
use iced::widget::image::Handle;

#[derive(Debug)]
pub(super) struct PageVersion {
    metadata: ProjectPage,
    size: Size,
    display: Option<Handle>,
    pixels: Size,
}

impl EditorPage {
    fn version(&self) -> PageVersion {
        PageVersion {
            metadata: self.metadata.clone(),
            size: self.size,
            display: self
                .image
                .as_ref()
                .and_then(|image| image.display.clone()),
            pixels: self.image.as_ref().map_or(self.size, |image| image.size),
        }
    }

    pub(in crate::app) fn accept_correction(
        &mut self,
        correction: PageCorrection,
        display: Handle,
    ) -> Result<usize, String> {
        if PageCorrection::from(&self.metadata) == correction {
            return Ok(0);
        }
        let (width, height) = corrected_dimensions(
            u32::try_from(self.metadata.source_pixel_width)
                .map_err(|error| error.to_string())?,
            u32::try_from(self.metadata.source_pixel_height)
                .map_err(|error| error.to_string())?,
            correction,
        )
        .map_err(|error| error.to_string())?;
        if !matches!(&display, Handle::Rgba { width: w, height: h, .. }
            if (*w, *h) == (width, height))
        {
            return Err("Corrected preview dimensions do not match.".into());
        }
        #[allow(clippy::cast_precision_loss)]
        let pixels = Size::new(width as f32, height as f32);
        let size = Size::new(612.0, 612.0 * pixels.height / pixels.width);
        let mapping = PageMapping::new(
            u32::try_from(self.metadata.source_pixel_width)
                .map_err(|error| error.to_string())?,
            u32::try_from(self.metadata.source_pixel_height)
                .map_err(|error| error.to_string())?,
            PageCorrection::from(&self.metadata),
            correction,
        )
        .map_err(|error| error.to_string())?;
        let staged = self
            .annotations
            .prepare_mapped_page_change(size, |center| {
                mapping.placement(center)
            })
            .ok_or(concat!(
                "Correction exceeds supported annotation sizes ",
                "(text: 6–144 pt)."
            ))?;
        let metadata = encode_corrected_editor_page(
            &self.metadata,
            staged.annotations(),
            correction,
        )
        .map_err(|error| error.to_string())?;
        let old = self.version();
        let old_context = self.annotations.page_context();
        let context =
            self.versions.last_key_value().map_or(1, |(key, _)| key + 1);
        let moved = self
            .annotations
            .commit_page_change(staged, context)
            .ok_or("Page edit is stale; current page kept.")?;
        self.versions.entry(old_context).or_insert(old);
        self.versions.insert(
            context,
            PageVersion {
                metadata,
                size,
                display: Some(display),
                pixels,
            },
        );
        self.restore_version()?;
        Ok(moved)
    }

    fn restore_version(&mut self) -> Result<(), String> {
        let version = self
            .versions
            .get(&self.annotations.page_context())
            .ok_or("Page history is unavailable; current page kept.")?;
        self.metadata.clone_from(&version.metadata);
        self.size = version.size;
        if let Some(image) = &mut self.image {
            image.display.clone_from(&version.display);
            image.size = version.pixels;
        }
        Ok(())
    }

    pub(in crate::app) fn history(
        &mut self,
        redo: bool,
    ) -> Result<bool, String> {
        let context = self.annotations.page_context();
        let changed = if redo {
            self.annotations.redo()
        } else {
            self.annotations.undo()
        };
        if changed
            && context != self.annotations.page_context()
            && let Err(error) = self.restore_version()
        {
            if redo {
                self.annotations.undo();
            } else {
                self.annotations.redo();
            }
            return Err(error);
        }
        Ok(changed)
    }

    pub(in crate::app) fn prune_versions(&mut self) {
        self.versions
            .retain(|context, _| self.annotations.uses_page_context(*context));
    }
}
