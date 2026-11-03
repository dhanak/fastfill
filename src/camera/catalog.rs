//! Bounded discovery in the same killable driver helper as streaming.

use super::{ProbeResult, Spec, backend, stream};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::cmp::Reverse;
use std::ffi::OsString;
use std::io::Write;

pub(super) const LIMIT: u64 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub index: u32,
    pub name: String,
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.index)
    }
}

/// Discover native devices without opening a stream.
///
/// # Errors
/// Returns worker, permission, timeout, or invalid catalog errors.
pub fn devices(
    receive: impl FnOnce(Result<Vec<Device>, String>) + Send + 'static,
) -> Result<stream::Stream, String> {
    stream::query(&["list".into()], move |bytes| {
        receive(decode(bytes).and_then(|mut devices: Vec<Device>| {
            if devices.len() > 128
                || devices.iter().any(|d| d.name.len() > 1024)
            {
                return Err("Invalid or oversized camera device list.".into());
            }
            devices.sort_by_key(|d| d.index);
            devices.dedup_by_key(|d| d.index);
            Ok(devices)
        }));
    })
}

/// List only bounded modes supported by `FastFill`'s existing decoders.
///
/// # Errors
/// Returns worker, permission, timeout, or invalid catalog errors.
pub fn modes(
    index: u32,
    receive: impl FnOnce(Result<Vec<Spec>, String>) + Send + 'static,
) -> Result<stream::Stream, String> {
    stream::query(&["modes".into(), index.to_string().into()], move |bytes| {
        receive(decode(bytes).map(normalize));
    })
}

fn decode<T: DeserializeOwned>(
    bytes: Result<Vec<u8>, String>,
) -> Result<Vec<T>, String> {
    let bytes = bytes?;
    let values: Result<Vec<T>, String> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Invalid camera catalog: {error}"))?;
    let values = values?;
    if values.len() > 4096 {
        return Err("Camera catalog exceeds 4096 entries.".into());
    }
    Ok(values)
}

pub(super) fn normalize(mut modes: Vec<Spec>) -> Vec<Spec> {
    modes.retain(|mode| mode.validate().is_ok());
    modes.sort_by_key(|m| {
        (
            Reverse(u64::from(m.width) * u64::from(m.height)),
            m.width,
            Reverse(m.fps),
            m.fourcc != *b"MJPG",
            m.fourcc,
        )
    });
    modes.dedup();
    modes
}

/// Offer each resolution once, at its highest FPS; prefer MJPEG on ties.
#[must_use]
pub fn resolutions(modes: Vec<Spec>) -> Vec<Spec> {
    let mut modes = normalize(modes);
    modes.dedup_by_key(|m| (m.width, m.height));
    modes
}

/// Prefer the largest resolution up to full HD, or the largest available.
#[must_use]
pub fn preferred(modes: &[Spec]) -> Option<Spec> {
    modes
        .iter()
        .find(|m| u64::from(m.width) * u64::from(m.height) <= 1920 * 1080)
        .or_else(|| modes.first())
        .copied()
}

/// Called before GUI startup, only in the discovery helper process.
///
/// # Errors
/// Rejects malformed arguments before accessing drivers or failed pipe writes.
pub fn worker(args: &[OsString]) -> Result<(), String> {
    match args {
        [command] if command == "list" => reply(backend::devices()),
        [command, index] if command == "modes" => {
            let index = index
                .to_str()
                .and_then(|value| value.parse::<u32>().ok())
                .ok_or("Expected a nonnegative camera index.")?;
            reply(backend::modes(index))
        }
        _ => Err("Expected list or modes INDEX.".into()),
    }
}

fn reply<T: Serialize>(result: ProbeResult<Vec<T>>) -> Result<(), String> {
    let bytes = serde_json::to_vec(&result.map_err(|e| e.to_string()))
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Camera catalog exceeds 1 MiB.".into());
    }
    let mut output = std::io::stdout().lock();
    output.write_all(&bytes).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
