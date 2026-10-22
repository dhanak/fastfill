//! Accepted page versions share annotation history through opaque context keys.

use super::EditorPage;
use fastfill_rust::correction::{
    PageCorrection, PageMapping, corrected_dimensions,
};
use fastfill_rust::geometry::Size;
use fastfill_rust::project::{
    ProjectAnnotation, ProjectPage, encode_corrected_editor_page,
};
use iced::widget::image::Handle;

#[derive(Debug)]
pub(super) struct PageVersion {
    metadata: ProjectPage,
    size: Size,
    display: Option<Handle>,
    pixels: Size,
    retained_bytes: usize,
}

impl PageVersion {
    fn new(
        metadata: ProjectPage,
        size: Size,
        display: Option<Handle>,
        pixels: Size,
    ) -> Self {
        let annotation_bytes = metadata.annotations.iter().map(|item| {
            std::mem::size_of::<ProjectAnnotation>()
                + match item {
                    ProjectAnnotation::Freehand { points, .. } => {
                        points.len()
                            * std::mem::size_of::<
                                fastfill_rust::project::NormalizedPoint,
                            >()
                    }
                    ProjectAnnotation::Text {
                        text, font_family, ..
                    } => text.len() + font_family.len(),
                    ProjectAnnotation::Shape { .. } => 0,
                }
        });
        let retained_bytes = std::mem::size_of::<Self>()
            + metadata.source_asset.len()
            + annotation_bytes.sum::<usize>()
            + display.as_ref().map_or(0, |handle| match handle {
                Handle::Rgba { pixels, .. } => pixels.len(),
                _ => 0,
            });
        Self {
            metadata,
            size,
            display,
            pixels,
            retained_bytes,
        }
    }
}

impl EditorPage {
    fn version(&self) -> PageVersion {
        PageVersion::new(
            self.metadata.clone(),
            self.size,
            self.image.as_ref().and_then(|image| image.display.clone()),
            self.image.as_ref().map_or(self.size, |image| image.size),
        )
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
            PageVersion::new(metadata, size, Some(display), pixels),
        );
        self.restore_version()?;
        self.prune_versions();
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
        self.prune_versions();
        Ok(changed)
    }

    pub(in crate::app) fn prune_versions(&mut self) {
        loop {
            self.versions.retain(|context, _| {
                self.annotations.uses_page_context(*context)
            });
            let retained = self
                .versions
                .iter()
                .filter(|(context, _)| {
                    **context != self.annotations.page_context()
                })
                .map(|(_, version)| version.retained_bytes)
                .sum();
            if !self.annotations.trim_history(retained) {
                break;
            }
        }
        self.versions
            .retain(|context, _| self.annotations.uses_page_context(*context));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_old_page_version_prunes_undo_and_its_pixels() {
        let mut page = EditorPage::default();
        let staged = page.annotations.prepare_page_change(page.size).unwrap();
        page.annotations.commit_page_change(staged, 1).unwrap();
        let mut old = page.version();
        old.retained_bytes = 65 * 1024 * 1024;
        page.versions.insert(0, old);
        page.versions.insert(1, page.version());

        page.prune_versions();

        assert!(!page.annotations.can_undo());
        assert_eq!(page.versions.len(), 1);
        assert!(page.versions.contains_key(&1));
    }
}
