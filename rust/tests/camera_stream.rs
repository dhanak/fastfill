#![cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]

use fastfill_rust::camera::stream::{read_frame, write_frame};
use image::{Rgba, RgbaImage};
use std::io::Cursor;

#[test]
fn preview_protocol_preserves_pixels_and_rejects_bad_frames() {
    let pixels = RgbaImage::from_pixel(2, 3, Rgba([40, 100, 200, 255]));
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &pixels).unwrap();
    assert_eq!(read_frame(&mut Cursor::new(&bytes)).unwrap(), pixels);
    for bytes in [
        vec![],
        vec![0; 8],
        [u32::MAX.to_le_bytes(), u32::MAX.to_le_bytes()].concat(),
        [2_u32.to_le_bytes(), 3_u32.to_le_bytes()].concat(),
    ] {
        assert!(read_frame(&mut Cursor::new(bytes)).is_err());
    }
}

#[test]
fn preview_protocol_keeps_frame_boundaries_and_bounded_driver_errors() {
    let first = RgbaImage::from_pixel(1, 2, Rgba([255, 0, 0, 255]));
    let second = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 255, 255]));
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &first).unwrap();
    write_frame(&mut bytes, &second).unwrap();
    let mut input = Cursor::new(bytes);
    assert_eq!(read_frame(&mut input).unwrap(), first);
    assert_eq!(read_frame(&mut input).unwrap(), second);
    assert!(read_frame(&mut input).is_err());

    let message = "Camera permission denied — try privacy settings.";
    let mut bytes = Vec::from(0_u32.to_le_bytes());
    bytes.extend(u32::try_from(message.len()).unwrap().to_le_bytes());
    bytes.extend(message.as_bytes());
    assert_eq!(read_frame(&mut Cursor::new(bytes)).unwrap_err(), message);
    let oversized = [0_u32.to_le_bytes(), 4097_u32.to_le_bytes()].concat();
    assert!(read_frame(&mut Cursor::new(oversized)).is_err());
    assert!(write_frame(&mut Vec::new(), &RgbaImage::new(0, 1)).is_err());
}

#[test]
fn full_hd_capture_preserves_resolution_and_edge_pixels() {
    let mut pixels =
        RgbaImage::from_pixel(1920, 1080, Rgba([40, 80, 120, 255]));
    pixels[(0, 0)] = Rgba([255, 0, 0, 255]);
    pixels[(1919, 1079)] = Rgba([0, 255, 0, 255]);
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &pixels).unwrap();
    assert_eq!(read_frame(&mut Cursor::new(bytes)).unwrap(), pixels);
}
