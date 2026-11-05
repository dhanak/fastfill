//! Whole-page output; share drawing code, never screenshot the live UI.

use super::render::draw_annotation;
use fastfill::annotations::{Annotations, OwnedAnnotation};
use fastfill::geometry::{Rect, Size as PageSize, fit_contain};
use fastfill::pdf_import::{PdfStage, raster_size};
use iced::advanced::Renderer as _;
use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::renderer::Headless;
use iced::widget::{canvas, image::Handle};
use iced::{Color, Point, Rectangle, Renderer};
use image::ImageEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use pdf_writer::types::Predictor;
use pdf_writer::{Content, Filter, Name, Pdf, Ref};

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
                .map(OwnedAnnotation::from)
                .collect(),
        }
    }
}

pub(crate) async fn export_pdf(
    pages: Vec<ExportPage>,
    lossless: bool,
    mut progress: impl AsyncFnMut(usize, usize, PdfStage) -> bool,
) -> Result<Vec<u8>, String> {
    use pdf_writer::Finish as _;

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
    report(&mut progress, 0, pages.len(), PdfStage::Rasterizing).await?;
    let mut renderer = new_renderer().await?;
    let mut document = Pdf::new();
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let page_ids: Vec<_> = (0..pages.len())
        .map(|i| Ref::new(3 + i32::try_from(i).expect("at most 20 pages") * 3))
        .collect();
    document.catalog(catalog).pages(tree);
    document
        .pages(tree)
        .kids(page_ids.iter().copied())
        .count(i32::try_from(pages.len()).expect("at most 20 pages"));
    for (index, (page, dimensions)) in pages.iter().zip(sizes).enumerate() {
        if index > 0 {
            report(&mut progress, index, pages.len(), PdfStage::Rasterizing)
                .await?;
        }
        let pixels = rasterize(&mut renderer, page, dimensions)?;
        report(&mut progress, index, pages.len(), PdfStage::Encoding).await?;
        let (compressed, gray, filter) =
            image_stream(pixels, dimensions, lossless)?;
        let page_id = page_ids[index];
        let image_id = page_id.next();
        let content_id = image_id.next();
        let name = Name(b"Page");
        let mut output = document.page(page_id);
        output.parent(tree).media_box(pdf_writer::Rect::new(
            0.0,
            0.0,
            page.size.width,
            page.size.height,
        ));
        output.contents(content_id);
        output.resources().x_objects().pair(name, image_id);
        output.finish();
        let width = i32::try_from(dimensions.0).expect("bounded raster width");
        let mut image = document.image_xobject(image_id, &compressed);
        image
            .width(width)
            .height(i32::try_from(dimensions.1).expect("bounded raster height"))
            .bits_per_component(8)
            .filter(filter);
        if gray {
            image.color_space().device_gray();
        } else {
            image.color_space().device_rgb();
        }
        if matches!(filter, Filter::FlateDecode) {
            image
                .decode_parms()
                .predictor(Predictor::PngOptimum)
                .colors(if gray { 1 } else { 3 })
                .bits_per_component(8)
                .columns(width);
        }
        image.finish();
        let mut content = Content::new();
        content.transform([
            page.size.width,
            0.0,
            0.0,
            page.size.height,
            0.0,
            0.0,
        ]);
        content.x_object(name);
        document.stream(content_id, &content.finish());
        report(&mut progress, index + 1, pages.len(), PdfStage::Ready).await?;
    }
    Ok(document.finish())
}

fn image_stream(
    pixels: Vec<u8>,
    (width, height): (u32, u32),
    lossless: bool,
) -> Result<(Vec<u8>, bool, Filter), String> {
    // The software canvas has already composited onto opaque white.
    let gray = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2]);
    let channels = if gray { 1 } else { 3 };
    let mut samples = Vec::with_capacity(pixels.len() / 4 * channels);
    for pixel in pixels.as_chunks::<4>().0 {
        samples.extend_from_slice(&pixel[..channels]);
    }
    drop(pixels);
    let mut png = Vec::new();
    let color = if gray {
        image::ExtendedColorType::L8
    } else {
        image::ExtendedColorType::Rgb8
    };
    PngEncoder::new_with_quality(
        &mut png,
        CompressionType::Default,
        FilterType::Adaptive,
    )
    .write_image(&samples, width, height, color)
    .map_err(|error| format!("PDF image compression failed: {error}"))?;
    // Only parse our own PNG encoder output. Reuse its filtered image stream
    // directly: PDF supports PNG predictors, without decoding/recompressing.
    let mut chunks = &png[8..];
    let mut stream = Vec::new();
    while chunks.len() >= 12 {
        let length = usize::try_from(u32::from_be_bytes(
            chunks[..4].try_into().expect("four-byte chunk length"),
        ))
        .map_err(|_| "PDF image chunk is too large.")?;
        let (chunk, rest) = chunks
            .split_at_checked(length + 12)
            .ok_or("PDF image compression returned invalid data.")?;
        if &chunk[4..8] == b"IDAT" {
            stream.extend_from_slice(&chunk[8..length + 8]);
        }
        chunks = rest;
    }
    drop(png);
    if !lossless {
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 95)
            .encode(&samples, width, height, color)
            .map_err(|error| {
                format!("PDF image compression failed: {error}")
            })?;
        // Keep crisp line art lossless when it is smaller than JPEG.
        if jpeg.len() < stream.len() {
            return Ok((jpeg, gray, Filter::DctDecode));
        }
    }
    Ok((stream, gray, Filter::FlateDecode))
}

async fn report(
    progress: &mut impl AsyncFnMut(usize, usize, PdfStage) -> bool,
    done: usize,
    total: usize,
    stage: PdfStage,
) -> Result<(), String> {
    if progress(done, total, stage).await {
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
