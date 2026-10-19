//! Camera I/O and photo encoding; no document mutations.

use super::{Event, ImportedImage, Message, orientation::Orientation};
use fastfill_rust::camera::stream::Stream;
use fastfill_rust::camera::{Spec, catalog};
use fastfill_rust::image_input::{ImageFormatKind, ImageInfo};
use iced::Task;
use iced::futures::{SinkExt, channel::mpsc};
use image::RgbaImage;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct Running {
    _stream: Stream,
    _task: iced::task::Handle,
}

pub(super) fn start(
    index: u32,
    spec: Spec,
    orientation: Orientation,
    id: u64,
    pending: Arc<AtomicBool>,
) -> Result<(Running, Task<Message>), String> {
    let (mut sender, receiver) = mpsc::channel(1);
    let stream = Stream::start(index, spec, move |result| {
        let failed = result.is_err();
        if !failed && pending.swap(true, Ordering::Relaxed) {
            return true;
        }
        let event = Event::Frame(
            id,
            result.map(|pixels| Arc::new(orientation.apply(pixels))),
        );
        if failed {
            // Deliver terminal errors even when a preview slot is occupied.
            iced::futures::executor::block_on(sender.send(event)).is_ok()
        } else {
            // One in-flight frame, including iced's internal event queue.
            match sender.try_send(event) {
                Ok(()) => true,
                Err(error) => {
                    pending.store(false, Ordering::Relaxed);
                    error.is_full()
                }
            }
        }
    })?;
    let (task, abort) = Task::run(receiver, Message::Camera).abortable();
    Ok((
        Running {
            _stream: stream,
            _task: abort.abort_on_drop(),
        },
        task,
    ))
}

pub(super) fn discover(
    id: u64,
    index: Option<u32>,
) -> Result<(Stream, Task<Message>), String> {
    let (sender, receiver) = iced::futures::channel::oneshot::channel();
    let stream = match index {
        Some(index) => catalog::modes(index, move |result| {
            let _ = sender.send(Event::Modes(id, result));
        }),
        None => catalog::devices(move |result| {
            let _ = sender.send(Event::Devices(id, result));
        }),
    }?;
    let task = Task::perform(
        async move {
            let failed = |error| match index {
                Some(_) => Event::Modes(id, Err(error)),
                None => Event::Devices(id, Err(error)),
            };
            receiver
                .await
                .unwrap_or_else(|error| failed(error.to_string()))
        },
        Message::Camera,
    );
    Ok((stream, task))
}

pub(super) fn prepare(pixels: &RgbaImage) -> Result<ImportedImage, String> {
    let mut encoded = std::io::Cursor::new(Vec::new());
    pixels
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|error| format!("Could not prepare photo: {error}"))?;
    let mut image = ImportedImage::from_validated(
        "Camera photo.png".into(),
        encoded.into_inner(),
        ImageInfo {
            width: pixels.width(),
            height: pixels.height(),
            format: ImageFormatKind::Png,
        },
        None,
    );
    image.set_pixels(pixels.clone());
    Ok(image)
}

pub(super) fn prepare_task(id: u64, pixels: Arc<RgbaImage>) -> Task<Message> {
    Task::perform(
        async move {
            let (sender, receiver) = iced::futures::channel::oneshot::channel();
            std::thread::Builder::new()
                .name("fastfill-camera-photo".into())
                .spawn(move || {
                    let _ = sender.send(prepare(&pixels));
                })
                .map_err(|error| error.to_string())?;
            receiver.await.map_err(|error| error.to_string())?
        },
        move |result| Message::Camera(Event::Prepared(id, result)),
    )
}
