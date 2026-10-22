//! Bounded PDF page rasterization, independent of the GUI.

use crate::image_input::{ImageFormatKind, ImageInfo, MAX_IMAGE_PIXELS};
use hayro::hayro_interpret::{InterpreterSettings, util::TransformExt};
use hayro::hayro_syntax::{LoadPdfError, Pdf, object::Object};
use hayro::kurbo::Affine;
use hayro::vello_cpu::{
    Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit,
    color::palette::css::WHITE, peniko::ImageAlphaType,
};
use hayro::{RenderCache, RenderSettings, render_into};
use image::ImageEncoder;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_PDF_BYTES: usize = 100 * 1024 * 1024;
const MAX_PAGES: usize = 20;
const MAX_BATCH_PIXELS: u64 = 200_000_000;
const MAX_BATCH_BYTES: usize = 600 * 1024 * 1024;
const SCALE: f64 = 300.0 / 72.0;

#[derive(Debug)]
pub struct PdfPageImage {
    pub bytes: Vec<u8>,
    pub info: ImageInfo,
    pub pixels: image::RgbaImage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdfStage {
    Parsing,
    Interpreting,
    Rasterizing,
    Encoding,
    Ready,
}

#[derive(Debug)]
pub struct PdfImportError(String);

impl fmt::Display for PdfImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PdfImportError {}

/// Validate encoded length before reading/allocating a PDF file.
///
/// # Errors
/// Rejects empty files and files exceeding 100 MiB.
pub fn validate_pdf_length(length: usize) -> Result<(), PdfImportError> {
    if length == 0 || length > MAX_PDF_BYTES {
        return Err(error("PDF must be between 1 byte and 100 MiB."));
    }
    Ok(())
}

/// Convert physical page points to bounded, rounded-up 300-DPI pixels.
///
/// # Errors
/// Rejects nonpositive/nonfinite sizes, dimensions beyond the renderer's
/// 16-bit range, and pages exceeding 50 megapixels.
pub fn raster_size(
    width: f32,
    height: f32,
) -> Result<(u32, u32), PdfImportError> {
    // Multiply before dividing: 792 * (300 / 72) rounds just above 3300.
    let dimensions = [
        f64::from(width) * 300.0 / 72.0,
        f64::from(height) * 300.0 / 72.0,
    ];
    if dimensions.iter().any(|value| {
        !value.is_finite()
            || *value <= 0.0
            || value.ceil() > f64::from(u16::MAX)
    }) {
        return Err(error("PDF page dimensions are invalid or too large."));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let [width, height] = dimensions.map(|value| value.ceil() as u32);
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(error("PDF page exceeds the 50-megapixel limit."));
    }
    Ok((width, height))
}

/// Rasterize a complete PDF batch. No partial result is returned.
///
/// `progress` receives `(completed, total)`, initially `(0, total)`. Return
/// false to cancel before rendering or between pages, including the last.
///
/// # Errors
/// Rejects invalid/unsupported PDFs, excessive pages or raster budgets,
/// renderer warnings/failures, and canceled imports. Input/output limits do
/// not sandbox the renderer's internal decompression or CPU consumption.
pub fn import_pdf(
    bytes: Vec<u8>,
    available: usize,
    mut progress: impl FnMut(usize, usize) -> bool,
) -> Result<Vec<PdfPageImage>, PdfImportError> {
    import_pdf_with_progress(bytes, available, |done, total, stage| {
        stage != PdfStage::Ready || progress(done, total)
    })
}

/// Rasterize with stage progress in addition to completed-page counts.
/// `Parsing` has an unknown total (zero); other stages refer to the next
/// page after `completed`. `Ready` reports initial/final page boundaries.
/// Return false to cancel at any stage boundary, never during a stage.
///
/// # Errors
/// Returns the same validation, rendering, and cancellation errors as
/// `import_pdf`.
pub fn import_pdf_with_progress(
    bytes: Vec<u8>,
    available: usize,
    progress: impl FnMut(usize, usize, PdfStage) -> bool,
) -> Result<Vec<PdfPageImage>, PdfImportError> {
    import_pdf_with_progress_limited(
        bytes,
        available,
        MAX_BATCH_PIXELS,
        progress,
    )
}

/// Rasterize within a caller's remaining project pixel budget.
///
/// # Errors
/// Returns the same errors as `import_pdf_with_progress`.
pub fn import_pdf_with_progress_limited(
    bytes: Vec<u8>,
    available: usize,
    pixel_budget: u64,
    progress: impl FnMut(usize, usize, PdfStage) -> bool,
) -> Result<Vec<PdfPageImage>, PdfImportError> {
    validate_pdf_length(bytes.len())?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Renderer state is local to this transaction; discard it on panic.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            render_pdf(bytes, available, pixel_budget, progress)
        }))
        .unwrap_or_else(|_| Err(error("PDF renderer failed; import canceled.")))
    }
    #[cfg(target_arch = "wasm32")]
    render_pdf(bytes, available, pixel_budget, progress)
}

fn render_pdf(
    bytes: Vec<u8>,
    available: usize,
    pixel_budget: u64,
    mut progress: impl FnMut(usize, usize, PdfStage) -> bool,
) -> Result<Vec<PdfPageImage>, PdfImportError> {
    report(&mut progress, 0, 0, PdfStage::Parsing)?;
    let pdf = Pdf::new(bytes).map_err(|reason| match reason {
        LoadPdfError::Decryption(_) => {
            error("PDF needs a password or uses unsupported encryption.")
        }
        LoadPdfError::Invalid => error("PDF is damaged or invalid."),
    })?;
    let count = pdf.pages().len();
    if count == 0 {
        return Err(error("PDF contains no pages."));
    }
    if count > available.min(MAX_PAGES) {
        return Err(PdfImportError(format!(
            "PDF has {count} pages; this project has room for {}.",
            available.min(MAX_PAGES)
        )));
    }
    let sizes = preflight(&pdf, pixel_budget.min(MAX_BATCH_PIXELS))?;
    report(&mut progress, 0, count, PdfStage::Ready)?;
    let warning = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&warning);
    let settings = InterpreterSettings {
        warning_sink: Arc::new(move |_| flag.store(true, Ordering::Relaxed)),
        ..InterpreterSettings::default()
    };
    let cache = RenderCache::new();
    let mut images = Vec::with_capacity(count);
    let mut total_bytes = 0;
    for (index, (page, (width, height, unit))) in
        pdf.pages().iter().zip(sizes).enumerate()
    {
        report(&mut progress, index, count, PdfStage::Interpreting)?;
        // The simple render API truncates pixel dimensions. Use an explicit
        // context so fractional page sizes keep the last row/column.
        let mut context = RenderContext::new(
            u16::try_from(width).expect("preflight bounded width"),
            u16::try_from(height).expect("preflight bounded height"),
        );
        render_into(
            page,
            &cache,
            &settings,
            &RenderSettings::default(),
            &mut context,
            Affine::scale(SCALE * f64::from(unit))
                * page.initial_transform(true).to_kurbo(),
        );
        if warning.load(Ordering::Relaxed) {
            return Err(PdfImportError(format!(
                "PDF page {} has unsupported or damaged content.",
                index + 1
            )));
        }
        context.flush();
        report(&mut progress, index, count, PdfStage::Rasterizing)?;
        let mut pixmap = Pixmap::new(context.width(), context.height());
        context.render_with(
            &mut pixmap,
            &mut Resources::default(),
            RasterizerSettings {
                target_init: TargetInit::Clear(WHITE),
                ..RasterizerSettings::default()
            },
        );
        report(&mut progress, index, count, PdfStage::Encoding)?;
        let image = encode_page(pixmap, width, height)?;
        total_bytes += image.bytes.len();
        if image.bytes.len() > MAX_PDF_BYTES || total_bytes > MAX_BATCH_BYTES {
            return Err(error(
                "Rendered PDF exceeds the image/archive byte limits.",
            ));
        }
        images.push(image);
        report(&mut progress, index + 1, count, PdfStage::Ready)?;
    }
    Ok(images)
}

fn encode_page(
    pixmap: Pixmap,
    width: u32,
    height: u32,
) -> Result<PdfPageImage, PdfImportError> {
    let pixels = image::RgbaImage::from_raw(
        width,
        height,
        pixmap.take_rgba8(ImageAlphaType::Alpha),
    )
    .expect("renderer dimensions match pixel storage");
    let mut bytes = Vec::new();
    // Use the existing codec's fast default, not the renderer's balanced
    // compression. Keep pixels for immediate display; do not decode PNG.
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|reason| {
            PdfImportError(format!("Could not encode PDF page: {reason}"))
        })?;
    Ok(PdfPageImage {
        bytes,
        pixels,
        info: ImageInfo {
            width,
            height,
            format: ImageFormatKind::Png,
        },
    })
}

fn report(
    progress: &mut impl FnMut(usize, usize, PdfStage) -> bool,
    completed: usize,
    total: usize,
    stage: PdfStage,
) -> Result<(), PdfImportError> {
    if progress(completed, total, stage) {
        Ok(())
    } else {
        Err(error("PDF import canceled."))
    }
}

fn preflight(
    pdf: &Pdf,
    pixel_budget: u64,
) -> Result<Vec<(u32, u32, f32)>, PdfImportError> {
    let mut total_pixels = 0_u64;
    pdf.pages()
        .iter()
        .map(|page| {
            let crop = page.intersected_crop_box();
            if !crop.width().is_finite()
                || !crop.height().is_finite()
                || crop.width() <= 0.0
                || crop.height() <= 0.0
            {
                return Err(error("PDF page crop bounds are invalid."));
            }
            let unit = page.raw().get::<f32>(b"UserUnit").unwrap_or(1.0);
            let (width, height) = page.render_dimensions();
            if !unit.is_finite() || unit <= 0.0 {
                return Err(error("PDF page unit is invalid."));
            }
            let (width, height) = raster_size(width * unit, height * unit)?;
            total_pixels += u64::from(width) * u64::from(height);
            if total_pixels > pixel_budget {
                return Err(error(if pixel_budget < MAX_BATCH_PIXELS {
                    "PDF exceeds the remaining project image budget."
                } else {
                    "PDF batch exceeds the 200-megapixel limit."
                }));
            }
            // Missing Contents means blank paper; failed decoding does not.
            if page.raw().get::<Object<'_>>(b"Contents").is_some()
                && page.page_stream().is_none()
            {
                return Err(error("PDF page content could not be decoded."));
            }
            Ok((width, height, unit))
        })
        .collect()
}

fn error(message: &str) -> PdfImportError {
    PdfImportError(message.into())
}
