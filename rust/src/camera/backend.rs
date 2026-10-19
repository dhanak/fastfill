use super::catalog::Device;
use super::{ProbeResult, Spec};
#[cfg(target_os = "linux")]
#[path = "backend/ranges.rs"]
mod ranges;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use super::{decode, first_frame};
use image::RgbaImage;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::time::Instant;

pub(super) fn capture(index: u32, spec: &Spec) -> ProbeResult<RgbaImage> {
    let mut saved = None;
    stream(index, spec, true, |pixels| {
        saved = Some(pixels);
        Ok(false)
    })?;
    saved.ok_or_else(|| "Camera produced no frame.".into())
}

#[cfg(target_os = "linux")]
pub(super) fn devices() -> ProbeResult<Vec<Device>> {
    let entries = match std::fs::read_dir("/sys/class/video4linux") {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error.into()),
    };
    let mut devices = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        if let Some(index) = name
            .to_str()
            .and_then(|s| s.strip_prefix("video"))
            .and_then(|s| s.parse::<u32>().ok())
        {
            let label = std::fs::read_to_string(entry.path().join("name"))?;
            devices.push(Device {
                index,
                name: label.trim().into(),
            });
        }
    }
    devices.sort_by_key(|device| device.index);
    Ok(devices)
}

#[cfg(target_os = "linux")]
pub(super) fn modes(index: u32) -> ProbeResult<Vec<Spec>> {
    let camera = rscam::new(&format!("/dev/video{index}"))?;
    let mut modes = Vec::new();
    for format in camera.formats().take(64) {
        let format = format?;
        if (Spec {
            fourcc: format.format,
            ..Spec::default()
        })
        .format()
        .is_err()
        {
            continue;
        }
        for (width, height) in
            ranges::sizes(camera.resolutions(&format.format)?)
        {
            let spec = Spec {
                width,
                height,
                fourcc: format.format,
                ..Spec::default()
            };
            if spec.validate().is_err() {
                continue;
            }
            for fps in ranges::rates(
                &camera.intervals(&format.format, (width, height))?,
            ) {
                modes.push(Spec { fps, ..spec });
                if modes.len() == 4096 {
                    return Ok(modes);
                }
            }
        }
    }
    Ok(modes)
}

#[cfg(target_os = "linux")]
pub(super) fn stream(
    index: u32,
    spec: &Spec,
    wait_for_exposure: bool,
    mut receive: impl FnMut(RgbaImage) -> ProbeResult<bool>,
) -> ProbeResult<()> {
    let mut camera = rscam::new(&format!("/dev/video{index}"))?;
    camera.start(&rscam::Config {
        interval: (1, spec.fps),
        resolution: (spec.width, spec.height),
        format: &spec.fourcc,
        ..Default::default()
    })?;
    let started = Instant::now();
    let result: ProbeResult<()> = (|| {
        let mut frame = first_frame(
            wait_for_exposure,
            || Ok(camera.capture()?),
            || started.elapsed(),
        )?;
        loop {
            if frame.resolution != (spec.width, spec.height)
                || frame.format != spec.fourcc
            {
                return Err("Driver changed the requested format.".into());
            }
            let keep_reading = receive(decode(spec, &frame)?)?;
            drop(frame);
            if !keep_reading {
                return Ok(());
            }
            frame = camera.capture()?;
        }
    })();
    let stopped = camera.stop();
    result?;
    stopped?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open(
    index: u32,
    request: nokhwa::utils::RequestedFormat<'_>,
) -> ProbeResult<nokhwa::Camera> {
    Ok(nokhwa::Camera::new(
        nokhwa::utils::CameraIndex::Index(index),
        request,
    )?)
}

#[cfg(target_os = "windows")]
pub(super) fn devices() -> ProbeResult<Vec<Device>> {
    nokhwa::query(nokhwa::utils::ApiBackend::MediaFoundation)?
        .into_iter()
        .map(|camera| {
            Ok(Device {
                index: camera.index().as_index()?,
                name: camera.human_name(),
            })
        })
        .collect()
}

#[cfg(target_os = "windows")]
pub(super) fn modes(index: u32) -> ProbeResult<Vec<Spec>> {
    use nokhwa::utils::{RequestedFormat, RequestedFormatType};
    let mut camera = open(
        index,
        RequestedFormat::with_formats(
            RequestedFormatType::None,
            nokhwa::utils::frame_formats(),
        ),
    )?;
    use nokhwa::utils::FrameFormat;
    Ok(camera
        .compatible_camera_formats()?
        .into_iter()
        .take(4096)
        .filter_map(|format| {
            let fourcc = match format.format() {
                FrameFormat::MJPEG => *b"MJPG",
                FrameFormat::YUYV => *b"YUYV",
                FrameFormat::NV12 => *b"NV12",
                FrameFormat::RAWRGB => *b"RGB3",
                FrameFormat::RAWBGR => *b"BGR3",
                FrameFormat::GRAY => *b"GREY",
                _ => return None,
            };
            Some(Spec {
                width: format.resolution().width(),
                height: format.resolution().height(),
                fps: format.frame_rate(),
                fourcc,
            })
        })
        .collect())
}

#[cfg(target_os = "windows")]
pub(super) fn stream(
    index: u32,
    spec: &Spec,
    wait_for_exposure: bool,
    mut receive: impl FnMut(RgbaImage) -> ProbeResult<bool>,
) -> ProbeResult<()> {
    use nokhwa::utils::{CameraFormat, RequestedFormat, RequestedFormatType};
    let format = CameraFormat::new_from(
        spec.width,
        spec.height,
        spec.format()?,
        spec.fps,
    );
    let accepted = [spec.format()?];
    let mut camera = open(
        index,
        RequestedFormat::with_formats(
            RequestedFormatType::Exact(format),
            &accepted,
        ),
    )?;
    camera.open_stream()?;
    let started = Instant::now();
    let result: ProbeResult<()> = (|| {
        let mut frame = first_frame(
            wait_for_exposure,
            || Ok(camera.frame()?),
            || started.elapsed(),
        )?;
        loop {
            if frame.resolution().width() != spec.width
                || frame.resolution().height() != spec.height
                || frame.source_frame_format() != spec.format()?
            {
                return Err("Driver changed the requested format.".into());
            }
            if !receive(decode(spec, frame.buffer())?)? {
                return Ok(());
            }
            frame = camera.frame()?;
        }
    })();
    let stopped = camera.stop_stream();
    result?;
    stopped?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub(super) fn devices() -> ProbeResult<Vec<Device>> {
    Err(
        "Camera adapters currently support native Linux and Windows only."
            .into(),
    )
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub(super) fn modes(_: u32) -> ProbeResult<Vec<Spec>> {
    Err("Native camera modes require Linux or Windows.".into())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub(super) fn stream(
    _: u32,
    _: &Spec,
    _: bool,
    _: impl FnMut(RgbaImage) -> ProbeResult<bool>,
) -> ProbeResult<()> {
    Err("Native camera capture requires Linux or Windows.".into())
}
