//! Shared native camera adapter and standalone probe; no document/UI state.

use crate::image_input::{
    ImageFormatKind, MAX_IMAGE_BYTES, MAX_IMAGE_PIXELS, decode_image,
};
use atomicwrites::{AtomicFile, DisallowOverwrite};
use image::{DynamicImage, ImageFormat, RgbaImage};
use nokhwa::utils::{FrameFormat, Resolution};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[path = "backend.rs"]
mod backend;

pub mod catalog;
pub mod stream;

const USAGE: &str = "Usage: camera_probe list | formats INDEX | \
    capture INDEX OUTPUT.png [WIDTH HEIGHT FPS FOURCC]\n\
    Defaults: 640 480 30 MJPG. FOURCC: MJPG YUYV NV12 RGB3 BGR3 GREY.";
type ProbeResult<T> = Result<T, Box<dyn Error>>;
const EXPOSURE_WARMUP: Duration = Duration::from_secs(2);

#[derive(Debug)]
enum Command {
    Help,
    List,
    Formats(u32),
    Capture {
        index: u32,
        output: PathBuf,
        spec: Spec,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub fourcc: [u8; 4],
}

impl std::fmt::Display for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}×{} · {} FPS · {}",
            self.width,
            self.height,
            self.fps,
            String::from_utf8_lossy(&self.fourcc),
        )
    }
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
            fps: 30,
            fourcc: *b"MJPG",
        }
    }
}

impl Spec {
    fn parse(args: &[OsString]) -> ProbeResult<Self> {
        let [width, height, fps, fourcc] = args else {
            return Err("Expected WIDTH HEIGHT FPS FOURCC.".into());
        };
        let spec = Self {
            width: width.to_str().ok_or(USAGE)?.parse()?,
            height: height.to_str().ok_or(USAGE)?.parse()?,
            fps: fps.to_str().ok_or(USAGE)?.parse()?,
            fourcc: fourcc.to_str().ok_or(USAGE)?.as_bytes().try_into()?,
        };
        spec.validate()?;
        Ok(spec)
    }

    fn format(&self) -> ProbeResult<FrameFormat> {
        match &self.fourcc {
            b"MJPG" => Ok(FrameFormat::MJPEG),
            b"YUYV" => Ok(FrameFormat::YUYV),
            b"NV12" => Ok(FrameFormat::NV12),
            b"RGB3" => Ok(FrameFormat::RAWRGB),
            b"BGR3" => Ok(FrameFormat::RAWBGR),
            b"GREY" => Ok(FrameFormat::GRAY),
            _ => Err("Unsupported FOURCC. Use --help.".into()),
        }
    }

    fn validate(&self) -> ProbeResult<()> {
        if self.width == 0
            || self.height == 0
            || u64::from(self.width) * u64::from(self.height) > MAX_IMAGE_PIXELS
            || u64::from(self.width) * u64::from(self.height) * 4
                > MAX_IMAGE_BYTES as u64
            || !(1..=120).contains(&self.fps)
        {
            return Err(
                "Use 1–120 FPS and dimensions within 50 MP/100 MiB RGBA."
                    .into(),
            );
        }
        let format = self.format()?;
        if (matches!(format, FrameFormat::YUYV | FrameFormat::NV12)
            && !self.width.is_multiple_of(2))
            || (format == FrameFormat::NV12 && !self.height.is_multiple_of(2))
        {
            return Err(
                "YUYV needs even width; NV12 needs even dimensions.".into()
            );
        }
        Ok(())
    }
}

fn parse(args: &[OsString]) -> ProbeResult<Command> {
    if args.iter().any(|arg| arg.is_empty()) {
        return Err(USAGE.into());
    }
    let number = |i: usize| -> ProbeResult<u32> {
        Ok(args[i].to_str().ok_or(USAGE)?.parse()?)
    };
    match args.first().and_then(|arg| arg.to_str()) {
        Some("--help") if args.len() == 1 => Ok(Command::Help),
        Some("list") if args.len() == 1 => Ok(Command::List),
        Some("formats") if args.len() == 2 => Ok(Command::Formats(number(1)?)),
        Some("capture") if matches!(args.len(), 3 | 7) => {
            let output = PathBuf::from(&args[2]);
            if !output
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("png"))
            {
                return Err("Choose a new .png output filename.".into());
            }
            let spec = if args.len() == 7 {
                Spec::parse(&args[3..])?
            } else {
                Spec::default()
            };
            spec.validate()?;
            Ok(Command::Capture {
                index: number(1)?,
                output,
                spec,
            })
        }
        _ => Err(USAGE.into()),
    }
}

/// Run the opt-in command-line camera diagnostic.
///
/// # Errors
/// Returns invalid arguments, camera/worker failures or output-write errors.
pub fn run() -> ProbeResult<()> {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let worker = args.first().is_some_and(|s| s == "--worker");
    if worker {
        args.remove(0);
    }
    let command = parse(&args)?;
    if matches!(command, Command::Help) {
        println!("{USAGE}");
        return Ok(());
    }
    if let Command::Capture { output, .. } = &command
        && output.try_exists()?
    {
        return Err("Output already exists; choose a new filename.".into());
    }
    if !worker {
        // Native drivers can block: isolate the spike in a killable process.
        let mut process = std::process::Command::new(std::env::current_exe()?);
        process.arg("--worker").args(args);
        return supervise(&mut process, Duration::from_secs(20));
    }
    match command {
        Command::List => {
            for device in backend::devices()? {
                println!("{device}");
            }
            Ok(())
        }
        Command::Formats(index) => {
            for mode in catalog::normalize(backend::modes(index)?) {
                println!("{mode}");
            }
            Ok(())
        }
        Command::Capture {
            index,
            output,
            spec,
        } => {
            let start = Instant::now();
            eprintln!("Warming camera exposure for at least two seconds…");
            let pixels = backend::capture(index, &spec)?;
            write_png(&output, &pixels)?;
            println!(
                "Wrote {} ({}x{}, {:?}).",
                output.display(),
                pixels.width(),
                pixels.height(),
                start.elapsed()
            );
            Ok(())
        }
        Command::Help => unreachable!(),
    }
}

fn first_frame<T>(
    wait_for_exposure: bool,
    mut read: impl FnMut() -> ProbeResult<T>,
    mut elapsed: impl FnMut() -> Duration,
) -> ProbeResult<T> {
    // Live preview shows startup frames; one-shot capture still warms up.
    if !wait_for_exposure {
        return read();
    }
    // Time actual streaming, not a potentially slow device-open/first read.
    drop(read()?);
    let first_frame = elapsed();
    // Drain, don't sleep: release queued startup buffers as exposure adjusts.
    for _ in 0..2 {
        drop(read()?);
    }
    while elapsed().saturating_sub(first_frame) < EXPOSURE_WARMUP {
        drop(read()?);
    }
    read()
}

fn supervise(
    process: &mut std::process::Command,
    timeout: Duration,
) -> ProbeResult<()> {
    let mut child = process.spawn()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!("Worker exited: {status}").into());
            }
            Ok(None) if start.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(20));
            }
            result => {
                child.kill()?;
                child.wait()?;
                result?;
                return Err("Camera timed out; worker stopped. \
                    Check device/permissions."
                    .into());
            }
        }
    }
}

fn decode(spec: &Spec, bytes: &[u8]) -> ProbeResult<RgbaImage> {
    spec.validate()?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err("Empty frame or frame exceeds 100 MiB.".into());
    }
    let format = spec.format()?;
    if format == FrameFormat::MJPEG {
        // Reuse bounded JPEG decoding; no second native JPEG runtime.
        let (info, image) = decode_image(bytes)?;
        if info.format != ImageFormatKind::Jpeg
            || (info.width, info.height) != (spec.width, spec.height)
        {
            return Err(
                "MJPG dimensions/encoding differ from the request.".into()
            );
        }
        return Ok(image.into_rgba8());
    }
    let pixels = u64::from(spec.width) * u64::from(spec.height);
    let expected = match format {
        FrameFormat::RAWRGB | FrameFormat::RAWBGR => pixels * 3,
        FrameFormat::YUYV => pixels * 2,
        FrameFormat::NV12 => pixels * 3 / 2,
        _ => pixels,
    };
    if u64::try_from(bytes.len())? != expected {
        return Err(
            "Unexpected raw frame length/stride; refusing a distorted image."
                .into(),
        );
    }
    let frame = nokhwa::Buffer::new(
        Resolution::new(spec.width, spec.height),
        bytes,
        format,
    );
    let rgb = frame.decode_image::<nokhwa::pixel_format::RgbFormat>()?;
    Ok(DynamicImage::ImageRgb8(rgb).into_rgba8())
}

fn write_png(output: &Path, pixels: &RgbaImage) -> ProbeResult<()> {
    AtomicFile::new(output, DisallowOverwrite)
        .write(|file| pixels.write_to(file, ImageFormat::Png))?;
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
