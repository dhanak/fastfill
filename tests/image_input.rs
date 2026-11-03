use fastfill::image_input::{
    ImageFormatKind, ImageInputError, MAX_IMAGE_BYTES, decode_image,
    decode_import_image, validate_image, validate_image_length,
};

const PNG_1X1: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1,
    0, 0, 0, 1, 8, 4, 0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84,
    120, 218, 99, 100, 248, 15, 0, 1, 5, 1, 1, 39, 24, 227, 102, 0, 0, 0, 0,
    73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn accepts_a_complete_png_and_reports_dimensions() {
    let image = validate_image(PNG_1X1).expect("valid PNG");

    assert_eq!(image.width, 1);
    assert_eq!(image.height, 1);
    let (info, pixels) = decode_image(PNG_1X1).unwrap();
    assert_eq!(info, image);
    assert_eq!(pixels.width(), 1);
    assert_eq!(pixels.height(), 1);
}

#[test]
fn rejects_empty_oversized_unsupported_and_corrupt_images() {
    assert_eq!(validate_image(&[]), Err(ImageInputError::Empty));
    assert_eq!(
        validate_image_length(MAX_IMAGE_BYTES + 1),
        Err(ImageInputError::TooLarge),
    );
    assert_eq!(
        validate_image(b"GIF89a"),
        Err(ImageInputError::UnsupportedFormat),
    );
    assert_eq!(
        validate_image(&PNG_1X1[..40]),
        Err(ImageInputError::InvalidData),
    );
}

#[test]
fn standalone_imports_normalize_all_exif_orientations_and_saved_pixels() {
    let positions = [
        [0, 1, 2, 3, 4, 5],
        [2, 1, 0, 5, 4, 3],
        [5, 4, 3, 2, 1, 0],
        [3, 4, 5, 0, 1, 2],
        [0, 3, 1, 4, 2, 5],
        [3, 0, 4, 1, 5, 2],
        [5, 2, 4, 1, 3, 0],
        [2, 5, 1, 4, 0, 3],
    ];
    for orientation in 1..=8 {
        let bytes = oriented_jpeg(orientation);
        let (raw_info, raw_pixels) = decode_image(&bytes).unwrap();
        assert_eq!((raw_info.width, raw_info.height), (3, 2));
        let raw: Vec<_> = raw_pixels.to_rgb8().pixels().copied().collect();
        let (info, pixels, saved) = decode_import_image(bytes.clone()).unwrap();
        let dimensions = if orientation >= 5 { (2, 3) } else { (3, 2) };
        assert_eq!((info.width, info.height), dimensions);
        let expected: Vec<_> = positions[usize::from(orientation - 1)]
            .iter()
            .map(|&index| raw[index])
            .collect();
        let actual: Vec<_> = pixels.to_rgb8().pixels().copied().collect();
        assert_eq!(actual, expected, "orientation {orientation}");

        if orientation == 1 {
            assert_eq!(saved, bytes, "untransformed JPEG stays unchanged");
            assert_eq!(info.format, ImageFormatKind::Jpeg);
        } else {
            assert_eq!(info.format, ImageFormatKind::Png);
        }
        let (saved_info, saved_pixels) = decode_image(&saved).unwrap();
        assert_eq!(saved_info, info);
        assert_eq!(saved_pixels.to_rgb8(), pixels.to_rgb8());
        let (again_info, again_pixels, again_bytes) =
            decode_import_image(saved.clone()).unwrap();
        assert_eq!(again_info, info);
        assert_eq!(again_pixels.to_rgb8(), pixels.to_rgb8());
        assert_eq!(again_bytes, saved, "orientation must apply only once");
    }
}

#[test]
fn untransformed_png_import_keeps_its_encoded_asset() {
    let (info, pixels, bytes) = decode_import_image(PNG_1X1.to_vec()).unwrap();
    assert_eq!(bytes, PNG_1X1);
    assert_eq!(info, validate_image(PNG_1X1).unwrap());
    assert_eq!((pixels.width(), pixels.height()), (1, 1));
    assert_eq!(
        decode_import_image(Vec::new()).unwrap_err(),
        ImageInputError::Empty,
    );
}

fn oriented_jpeg(orientation: u8) -> Vec<u8> {
    let pixels = image::RgbImage::from_fn(3, 2, |x, y| {
        image::Rgb([u8::try_from(30 + x * 60 + y * 20).unwrap(); 3])
    });
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 100)
        .encode_image(&pixels)
        .unwrap();
    let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0".to_vec();
    exif.extend_from_slice(b"\x12\x01\x03\0\x01\0\0\0");
    exif.extend_from_slice(&[orientation, 0, 0, 0, 0, 0, 0, 0]);
    let length = u16::try_from(exif.len() + 2).unwrap().to_be_bytes();
    let mut segment = vec![0xff, 0xe1, length[0], length[1]];
    segment.extend_from_slice(&exif);
    bytes.splice(2..2, segment);
    bytes
}
