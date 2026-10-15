//! Whole-page output; share drawing code, never screenshot the live UI.

use super::render::draw_annotation;
use fastfill_rust::annotations::{AnnotationRef, Annotations, OwnedAnnotation};
use fastfill_rust::geometry::{Rect, Size as PageSize, fit_contain};
use fastfill_rust::pdf_import::raster_size;
use iced::advanced::Renderer as _;
use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::renderer::Headless;
use iced::widget::{canvas, image::Handle};
use iced::{Color, Point, Rectangle, Renderer};
use krilla::{Document, geom::Size, image::Image, page::PageSettings};

pub(crate) struct ExportPage {
    size: PageSize,
    image: Option<Handle>,
    annotations: Vec<OwnedAnnotation>,
}

impl ExportPage {
    pub(crate) fn new(
        size: PageSize,
        image: Option<Handle>,
        annotations: &Annotations,
    ) -> Self {
        Self {
            size,
            image,
            annotations: annotations
                .ordered_annotations()
                .map(|annotation| match annotation {
                    AnnotationRef::Shape(_, shape) => {
                        OwnedAnnotation::Shape(shape.clone())
                    }
                    AnnotationRef::Text(_, text) => {
                        OwnedAnnotation::Text(text.clone())
                    }
                })
                .collect(),
        }
    }
}

pub(crate) async fn export_pdf(
    pages: Vec<ExportPage>,
    mut progress: impl FnMut(usize, usize) -> bool,
) -> Result<Vec<u8>, String> {
    if pages.is_empty() || pages.len() > 20 {
        return Err("PDF export requires between 1 and 20 pages.".into());
    }
    let mut total_pixels = 0_u64;
    let sizes = pages
        .iter()
        .map(|page| {
            let (width, height) =
                raster_size(page.size.width, page.size.height)
                    .map_err(|error| error.to_string())?;
            total_pixels += u64::from(width) * u64::from(height);
            // Work is page-at-a-time, but cap total CPU/output work as well.
            if total_pixels > 200_000_000 {
                return Err("PDF export exceeds 200 megapixels.".into());
            }
            Ok((width, height))
        })
        .collect::<Result<Vec<_>, String>>()?;
    report(&mut progress, 0, pages.len())?;
    let mut renderer = new_renderer().await?;
    let mut document = Document::new();
    for (index, (page, dimensions)) in pages.iter().zip(sizes).enumerate() {
        let pixels = rasterize(&mut renderer, page, dimensions)?;
        let settings = PageSettings::from_wh(page.size.width, page.size.height)
            .ok_or("PDF page dimensions are invalid.")?;
        let mut output = document.start_page_with(settings);
        let mut surface = output.surface();
        // Lossless raw pixels avoid a PNG encode/decode or JPEG artifacts.
        surface.draw_image(
            Image::from_rgba8(pixels, dimensions.0, dimensions.1),
            Size::from_wh(page.size.width, page.size.height)
                .ok_or("PDF page dimensions are invalid.")?,
        );
        surface.finish();
        output.finish();
        report(&mut progress, index + 1, pages.len())?;
    }
    document
        .finish()
        .map_err(|error| format!("PDF export failed: {error}"))
}

fn report(
    progress: &mut impl FnMut(usize, usize) -> bool,
    done: usize,
    total: usize,
) -> Result<(), String> {
    if progress(done, total) {
        Ok(())
    } else {
        Err("PDF export canceled.".into())
    }
}

async fn new_renderer() -> Result<Renderer, String> {
    <Renderer as Headless>::new(
        iced::Font::DEFAULT,
        iced::Pixels(16.0),
        Some("tiny-skia"),
    )
    .await
    .ok_or_else(|| "PDF software renderer is unavailable.".into())
}

fn rasterize(
    renderer: &mut Renderer,
    page: &ExportPage,
    dimensions: (u32, u32),
) -> Result<Vec<u8>, String> {
    // Draw in page points, just like the editor. DPI belongs to the output
    // transform, not font layout or underline thickness.
    let logical = iced::Size::new(page.size.width, page.size.height);
    let bounds = Rectangle::new(Point::ORIGIN, logical);
    renderer.reset(bounds);
    let mut frame = canvas::Frame::new(renderer, logical);
    frame.fill_rectangle(Point::ORIGIN, logical, Color::WHITE);
    if let Some(image) = &page.image {
        frame.draw_image(bounds, image);
    }
    renderer.draw_geometry(frame.into_geometry());
    let view = fit_contain(
        page.size,
        Rect::new(0.0, 0.0, page.size.width, page.size.height),
    )
    .ok_or("PDF page dimensions are invalid.")?;
    for annotation in &page.annotations {
        // Match the UI stack's explicit renderer layers, not only geometry:
        // the software backend otherwise batches all text above all paths.
        let mut frame = canvas::Frame::new(renderer, logical);
        draw_annotation(&mut frame, view, annotation.as_ref(), 1.0);
        let geometry = frame.into_geometry();
        renderer.with_layer(bounds, |renderer| {
            renderer.draw_geometry(geometry);
        });
    }
    let pixels = renderer.screenshot(
        iced::Size::new(dimensions.0, dimensions.1),
        300.0 / 72.0,
        Color::WHITE,
    );
    if pixels.len()
        != usize::try_from(
            u64::from(dimensions.0) * u64::from(dimensions.1) * 4,
        )
        .map_err(|_| "PDF page buffer is too large.")?
    {
        return Err("PDF renderer returned an invalid page buffer.".into());
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests;
