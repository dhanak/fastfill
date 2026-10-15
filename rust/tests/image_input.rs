use fastfill_rust::image_input::{
    ImageInputError, MAX_IMAGE_BYTES, decode_image, validate_image,
    validate_image_length,
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
