//! Killable native driver process; bounded raw-frame protocol, no UI types.

use super::{Spec, backend};
use crate::image_input::{MAX_IMAGE_BYTES, MAX_IMAGE_PIXELS};
use image::RgbaImage;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

pub const WORKER_FLAG: &str = "--fastfill-camera-stream";
pub const QUERY_FLAG: &str = "--fastfill-camera-query";
pub const INTERRUPTED: &str = "Camera interrupted after sleep or a long pause.";
const ERROR_LIMIT: u32 = 4096;

/// A frame is width/height in little endian followed by tightly packed RGBA.
/// Width zero reserves height bytes for a bounded UTF-8 driver error.
///
/// # Errors
/// Rejects invalid dimensions or a failed pipe write.
pub fn write_frame(
    output: &mut impl Write,
    pixels: &RgbaImage,
) -> Result<(), String> {
    frame_length(pixels.width(), pixels.height())?;
    output
        .write_all(&pixels.width().to_le_bytes())
        .and_then(|()| output.write_all(&pixels.height().to_le_bytes()))
        .and_then(|()| output.write_all(pixels.as_raw()))
        .and_then(|()| output.flush())
        .map_err(|error| error.to_string())
}

/// Read one validated frame without allocating from unchecked dimensions.
///
/// # Errors
/// Returns driver errors, malformed/truncated packets or failed pipe reads.
pub fn read_frame(input: &mut impl Read) -> Result<RgbaImage, String> {
    let mut header = [0; 8];
    input.read_exact(&mut header).map_err(|error| {
        format!("Camera stream stopped ({error}). Check camera permissions.")
    })?;
    let width =
        u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let height =
        u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if width == 0 && (1..=ERROR_LIMIT).contains(&height) {
        let mut message = vec![0; height as usize];
        input.read_exact(&mut message).map_err(|e| e.to_string())?;
        return Err(String::from_utf8_lossy(&message).into_owned());
    }
    let mut bytes = vec![0; frame_length(width, height)?];
    input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| "Invalid camera pixels.".into())
}

fn frame_length(width: u32, height: u32) -> Result<usize, String> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || pixels > MAX_IMAGE_PIXELS
        || pixels * 4 > MAX_IMAGE_BYTES as u64
    {
        return Err("Invalid or oversized camera frame.".into());
    }
    usize::try_from(pixels * 4).map_err(|error| error.to_string())
}

/// Called before starting the GUI in the helper process only.
///
/// # Errors
/// Rejects invalid arguments or a failed write to the parent process.
pub fn worker(args: &[OsString]) -> Result<(), String> {
    let Some((index, settings)) = args.split_first() else {
        return Err("Camera worker needs INDEX WIDTH HEIGHT FPS FOURCC.".into());
    };
    let index = index
        .to_str()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or("Expected a nonnegative camera index.")?;
    let spec = Spec::parse(settings).map_err(|e| e.to_string())?;
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    let result = backend::stream(index, &spec, false, |pixels| {
        write_frame(&mut output, &pixels)?;
        Ok(true)
    });
    if let Err(error) = result {
        let message = error.to_string();
        let bytes = message.as_bytes();
        let length = bytes.len().min(ERROR_LIMIT as usize);
        let header = u32::try_from(length)
            .map_err(|error| error.to_string())?
            .to_le_bytes();
        output
            .write_all(&0_u32.to_le_bytes())
            .and_then(|()| output.write_all(&header))
            .and_then(|()| output.write_all(&bytes[..length]))
            .and_then(|()| output.flush())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub struct Stream {
    canceled: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Relaxed);
        // Send termination before app exit; only the watchdog waits/reaps.
        if let Some(child) = self.child.lock().unwrap().as_mut() {
            let _ = child.kill();
        }
    }
}

// Always kill/reap, including thread-spawn failures and unwinding.
struct Process(Arc<Mutex<Option<Child>>>);

impl Drop for Process {
    fn drop(&mut self) {
        // Never hold the shared lock while waiting on a stuck driver.
        let child = self.0.lock().unwrap().take();
        if let Some(mut child) = child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Stream {
    /// Start a preview; dropping the handle stops the driver process.
    ///
    /// # Errors
    /// Returns executable, process-spawn or worker-thread creation failures.
    pub fn start(
        index: u32,
        spec: Spec,
        receive: impl FnMut(Result<RgbaImage, String>) -> bool + Send + 'static,
    ) -> Result<Self, String> {
        spec.validate().map_err(|e| e.to_string())?;
        let command = command(
            WORKER_FLAG,
            &[
                index.to_string().into(),
                spec.width.to_string().into(),
                spec.height.to_string().into(),
                spec.fps.to_string().into(),
                String::from_utf8_lossy(&spec.fourcc).into_owned().into(),
            ],
        )?;
        Self::spawn(command, receive)
    }

    fn spawn(
        mut command: Command,
        receive: impl FnMut(Result<RgbaImage, String>) -> bool + Send + 'static,
    ) -> Result<Self, String> {
        let mut receive = receive;
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("Could not start camera: {error}"))?;
        let mut input = child.stdout.take().expect("piped stdout");
        let child = Arc::new(Mutex::new(Some(child)));
        let process = Process(child.clone());
        let canceled = Arc::new(AtomicBool::new(false));
        let last_frame = Arc::new(Mutex::new(Instant::now()));
        let failure = Arc::new(Mutex::new(None::<String>));
        let (stop, last, error) =
            (canceled.clone(), last_frame.clone(), failure.clone());
        std::thread::Builder::new()
            .name("fastfill-camera-reader".into())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let result = read_frame(&mut input);
                    *last.lock().unwrap() = Instant::now();
                    let result = match error.lock().unwrap().take() {
                        Some(error) => Err(error),
                        None => result,
                    };
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let failed = result.is_err();
                    if !receive(result) || failed {
                        break;
                    }
                }
                stop.store(true, Ordering::Relaxed);
            })
            .map_err(|error| error.to_string())?;
        let stop = canceled.clone();
        std::thread::Builder::new()
            .name("fastfill-camera-watchdog".into())
            .spawn(move || {
                supervise(
                    process,
                    &stop,
                    &last_frame,
                    &failure,
                    Duration::from_secs(20),
                    SystemTime::now,
                );
            })
            .map_err(|error| {
                canceled.store(true, Ordering::Relaxed);
                error.to_string()
            })?;
        Ok(Self { canceled, child })
    }
}

fn command(flag: &str, args: &[OsString]) -> Result<Command, String> {
    let mut command = Command::new(
        std::env::current_exe().map_err(|error| error.to_string())?,
    );
    command.arg(flag).args(args);
    // Avoid extra console windows when launched from Explorer.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW.
    }
    Ok(command)
}

pub(super) fn query(
    args: &[OsString],
    receive: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) -> Result<Stream, String> {
    start_query(command(QUERY_FLAG, args)?, Duration::from_secs(20), receive)
}

fn start_query(
    mut command: Command,
    timeout: Duration,
    receive: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) -> Result<Stream, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let input = child.stdout.take().expect("piped stdout");
    let child = Arc::new(Mutex::new(Some(child)));
    let process = Process(child.clone());
    let canceled = Arc::new(AtomicBool::new(false));
    let failure = Arc::new(Mutex::new(None));
    let (stop, error) = (canceled.clone(), failure.clone());
    let watchdog = std::thread::Builder::new()
        .name("fastfill-camera-discovery".into())
        .spawn(move || {
            supervise(
                process,
                &stop,
                &Mutex::new(Instant::now()),
                &error,
                timeout,
                SystemTime::now,
            );
        })
        .map_err(|error| error.to_string())?;
    let stream = Stream { canceled, child };
    let stop = stream.canceled.clone();
    std::thread::Builder::new()
        .name("fastfill-camera-catalog-reader".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = input
                .take(super::catalog::LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string());
            let canceled = stop.swap(true, Ordering::Relaxed);
            let result = result.and(
                watchdog
                    .join()
                    .map_err(|_| "Camera query worker failed.".to_string()),
            );
            if !canceled {
                receive(result.and_then(|()| {
                    if let Some(error) = failure.lock().unwrap().take() {
                        Err(error)
                    } else if bytes.is_empty()
                        || bytes.len() as u64 > super::catalog::LIMIT
                    {
                        Err("Empty or oversized camera catalog.".into())
                    } else {
                        Ok(bytes)
                    }
                }));
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(stream)
}

fn supervise(
    child: Process,
    canceled: &AtomicBool,
    last: &Mutex<Instant>,
    failure: &Mutex<Option<String>>,
    timeout: Duration,
    mut clock: impl FnMut() -> SystemTime,
) {
    let mut previous = clock();
    while !canceled.load(Ordering::Relaxed) {
        let now = clock();
        // A dedicated thread keeps ticking during window moves/focus loss.
        // Wall time includes suspend on Linux, unlike Instant. No OS hook.
        if !now
            .duration_since(previous)
            .is_ok_and(|gap| gap < Duration::from_secs(3))
        {
            *failure.lock().unwrap() = Some(INTERRUPTED.into());
            break;
        }
        previous = now;
        if last.lock().unwrap().elapsed() >= timeout {
            *failure.lock().unwrap() =
                Some("Camera timed out. Retry or check device access.".into());
            break;
        }
        let exited = child
            .0
            .lock()
            .unwrap()
            .as_mut()
            .is_none_or(|child| !matches!(child.try_wait(), Ok(None)));
        if exited {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(child);
}

#[cfg(test)]
mod tests;
