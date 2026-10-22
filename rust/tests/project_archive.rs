#[cfg(not(target_arch = "wasm32"))]
use fastfill_rust::project::save_archive_file;
use fastfill_rust::project::{
    ProjectAnnotation, ProjectLoadError, load_archive, load_archive_file,
    load_archive_with_images, save_archive,
};
use serde_json::{Value, json};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::io::{Cursor, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const PNG_1X1: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1,
    0, 0, 0, 1, 8, 4, 0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84,
    120, 218, 99, 100, 248, 15, 0, 1, 5, 1, 1, 39, 24, 227, 102, 0, 0, 0, 0,
    73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn staged_display_reuses_the_validated_pixels_and_preserves_source_bytes() {
    let bytes = archive_with_manifest(
        &valid_manifest(),
        &[("assets/page.png", PNG_1X1)],
    );
    let mut decoded = Vec::new();
    let loaded =
        load_archive_with_images(Cursor::new(bytes), |name, pixels| {
            decoded.push((name.to_owned(), pixels.width(), pixels.height()));
        })
        .unwrap();
    assert_eq!(decoded, [("page.png".into(), 1, 1)]);
    assert_eq!(loaded.assets["page.png"].bytes, PNG_1X1);
}

#[test]
fn rejects_shared_assets_and_enforces_aggregate_pixels_before_decode() {
    let mut manifest = valid_manifest();
    let mut second = manifest["pages"][0].clone();
    second["id"] = json!("39cf856d-dc68-48bf-b98a-f4999aa7796b");
    second["annotations"] = json!([]);
    manifest["pages"].as_array_mut().unwrap().push(second);
    assert_invalid(
        archive_with_manifest(&manifest, &[]),
        "image assets must be unique",
    );

    let pages = manifest["pages"].as_array_mut().unwrap();
    for (index, page) in pages.iter_mut().enumerate() {
        page["sourceAsset"] = json!(format!("{index}.png"));
        page["sourcePixelWidth"] = json!(10_000);
        page["sourcePixelHeight"] = json!(5_000);
    }
    for index in 2..5 {
        let mut page = pages[0].clone();
        page["id"] = json!(uuid::Uuid::new_v4());
        page["sourceAsset"] = json!(format!("{index}.png"));
        pages.push(page);
    }
    assert_invalid(
        archive_with_manifest(&manifest, &[]),
        "200-megapixel limit",
    );
}

#[test]
fn non_ascii_four_byte_asset_stem_does_not_panic() {
    let mut manifest = valid_manifest();
    manifest["pages"][0]["sourceAsset"] = json!("aaé.png");
    let bytes =
        archive_with_manifest(&manifest, &[("assets/aaé.png", PNG_1X1)]);
    assert!(load_archive(Cursor::new(bytes)).is_ok());
}

#[test]
fn loads_csharp_schema_without_discarding_edit_data() {
    let archive = archive_with_manifest(
        &valid_manifest(),
        &[("assets/page.png", PNG_1X1)],
    );

    let loaded = load_archive(Cursor::new(archive)).expect("valid project");

    assert_eq!(loaded.project.title, "Compatibility fixture");
    assert_eq!(loaded.project.pages.len(), 1);
    assert_eq!(loaded.project.pages[0].annotations.len(), 3);
    assert!(matches!(
        &loaded.project.pages[0].annotations[0],
        ProjectAnnotation::Text {
            text,
            is_bold: true,
            is_italic: true,
            is_underlined: true,
            ..
        } if text == "Hello\nworld"
    ));
    assert!(matches!(
        &loaded.project.pages[0].annotations[1],
        ProjectAnnotation::Freehand {
            is_highlighter: true,
            filled: false,
            points,
            ..
        } if points.len() == 2
    ));
    assert!(matches!(
        &loaded.project.pages[0].annotations[2],
        ProjectAnnotation::Shape { filled: true, .. }
    ));
    let asset = loaded.asset("page.png").expect("loaded page asset");
    assert_eq!(asset.bytes, PNG_1X1);
    assert_eq!((asset.info.width, asset.info.height), (1, 1));
}

#[test]
fn schema_v2_round_trips_brightness_without_changing_v1_output() {
    let mut legacy = valid_manifest();
    legacy.as_object_mut().unwrap().remove("schemaVersion");
    let v1 = load_archive(Cursor::new(archive_with_manifest(
        &legacy,
        &[("assets/page.png", PNG_1X1)],
    )))
    .unwrap();
    assert_eq!(v1.project.schema_version, 1);
    let manifest = serde_json::to_value(&v1.project).unwrap();
    assert!(manifest["pages"][0].get("brightness").is_none());
    for brightness in [-100, -20, 0, 20, 100] {
        let mut manifest = valid_manifest();
        manifest["schemaVersion"] = json!(2);
        manifest["pages"][0]["brightness"] = json!(brightness);
        let loaded = load_archive(Cursor::new(archive_with_manifest(
            &manifest,
            &[("assets/page.png", PNG_1X1)],
        )))
        .unwrap();
        let output = save_archive(&loaded, Cursor::new(Vec::new()))
            .unwrap()
            .into_inner();
        let saved = load_archive(Cursor::new(output)).unwrap();
        assert_eq!(saved.project, loaded.project);
        assert_eq!(saved.assets["page.png"].bytes, PNG_1X1);
    }
}

#[test]
fn schemas_and_brightness_are_validated_before_decoding_assets() {
    for (version, brightness, message) in [
        (0, 0, "schema is unsupported"),
        (3, 0, "newer FastFill"),
        (1, 10, "Brightness requires schema-v2"),
        (2, -101, "between -100 and 100"),
        (2, 101, "between -100 and 100"),
    ] {
        let mut manifest = valid_manifest();
        manifest["schemaVersion"] = json!(version);
        manifest["pages"][0]["brightness"] = json!(brightness);
        // Missing assets prove metadata validation happens first.
        assert_invalid(archive_with_manifest(&manifest, &[]), message);
    }
    let mut oversized = valid_manifest();
    let page = &mut oversized["pages"][0];
    page["sourcePixelWidth"] = json!(50_000_000);
    page["sourcePixelHeight"] = json!(1);
    let point = |x, y| json!({"x": x, "y": y});
    page["crop"] = json!({
        "topLeft": point(0.0, 0.0),
        "topRight": point(1.0, 0.2),
        "bottomRight": point(0.9, 1.0),
        "bottomLeft": point(0.1, 0.8),
    });
    assert_invalid(
        archive_with_manifest(&oversized, &[]),
        "Corrected image exceeds",
    );
}

#[test]
fn rejects_unsafe_duplicate_and_unknown_archive_data() {
    let unsafe_archive = archive_with_manifest(
        &valid_manifest(),
        &[("../escape.png", PNG_1X1), ("assets/page.png", PNG_1X1)],
    );
    assert_invalid(unsafe_archive, "Unsafe project entry");

    let duplicate = archive_with_entries(&[
        ("project.json", manifest_bytes(&valid_manifest())),
        ("assets/page.png", PNG_1X1.to_vec()),
        ("assets\\page.png", PNG_1X1.to_vec()),
    ]);
    assert_invalid(duplicate, "Duplicate project entry");

    let mut unknown = valid_manifest();
    unknown["surprise"] = json!(true);
    assert_invalid(
        archive_with_manifest(&unknown, &[("assets/page.png", PNG_1X1)]),
        "Project manifest is invalid",
    );
}

#[test]
fn rejects_invalid_geometry_and_asset_mismatches() {
    let mut bad_crop = valid_manifest();
    bad_crop["pages"][0]["crop"]["topLeft"]["x"] = json!(-0.1);
    assert_invalid(
        archive_with_manifest(&bad_crop, &[("assets/page.png", PNG_1X1)]),
        "Crop corners",
    );

    assert_invalid(
        archive_with_manifest(&valid_manifest(), &[]),
        "Page asset is missing",
    );

    let mut wrong_dimensions = valid_manifest();
    wrong_dimensions["pages"][0]["sourcePixelWidth"] = json!(2);
    assert_invalid(
        archive_with_manifest(
            &wrong_dimensions,
            &[("assets/page.png", PNG_1X1)],
        ),
        "dimensions do not match",
    );

    let mut wrong_format = valid_manifest();
    wrong_format["pages"][0]["sourceAsset"] = json!("page.jpg");
    assert_invalid(
        archive_with_manifest(&wrong_format, &[("assets/page.jpg", PNG_1X1)]),
        "format does not match",
    );
}

#[test]
fn rejects_duplicate_ids_model_limits_and_unusable_transforms() {
    let mut duplicate = valid_manifest();
    duplicate["pages"][0]["annotations"][1]["id"] =
        duplicate["pages"][0]["annotations"][0]["id"].clone();
    assert_invalid(
        archive_with_manifest(&duplicate, &[("assets/page.png", PNG_1X1)]),
        "Annotation IDs must be unique",
    );

    let mut too_many_pages = valid_manifest();
    let page = too_many_pages["pages"][0].clone();
    too_many_pages["pages"] = Value::Array(vec![page; 21]);
    assert_invalid(
        archive_with_manifest(&too_many_pages, &[("assets/page.png", PNG_1X1)]),
        "at most 20 pages",
    );

    let mut long_text = valid_manifest();
    long_text["pages"][0]["annotations"][0]["text"] = json!("x".repeat(10_001));
    assert_invalid(
        archive_with_manifest(&long_text, &[("assets/page.png", PNG_1X1)]),
        "Text annotation is too long",
    );

    let mut singular = valid_manifest();
    singular["pages"][0]["annotations"][0]["transform"]["m11"] =
        json!(1e-40_f32);
    assert_invalid(
        archive_with_manifest(&singular, &[("assets/page.png", PNG_1X1)]),
        "Annotation transform is invalid",
    );
}

#[test]
fn rejects_manifest_before_allocating_beyond_its_limit() {
    let oversized = vec![b' '; 5 * 1024 * 1024 + 1];
    let archive = archive_with_entries(&[("project.json", oversized)]);

    assert_invalid(archive, "Project manifest is too large");
}

#[test]
fn round_trips_external_csharp_fixture_when_requested() {
    let Some(path) = std::env::var_os("FASTFILL_DOCSCAN_FIXTURE") else {
        return;
    };

    let loaded = load_archive_file(path).expect("external C# project");

    assert_ne!(loaded.project.pages.len(), 0);
    let output = save_archive(&loaded, Cursor::new(Vec::new()))
        .expect("saved external project")
        .into_inner();
    let reloaded =
        load_archive(Cursor::new(output)).expect("reloaded external project");
    assert_eq!(reloaded.project, loaded.project);
    assert_eq!(reloaded.assets.len(), loaded.assets.len());
}

#[test]
fn saves_a_lossless_archive_that_the_loader_accepts() {
    let source = archive_with_manifest(
        &valid_manifest(),
        &[("assets/page.png", PNG_1X1)],
    );
    let loaded = load_archive(Cursor::new(source)).expect("source project");

    let output = save_archive(&loaded, Cursor::new(Vec::new()))
        .expect("saved project")
        .into_inner();
    let reloaded =
        load_archive(Cursor::new(output)).expect("saved project reloads");

    assert_eq!(reloaded.project, loaded.project);
    assert_eq!(
        reloaded.asset("page.png").expect("saved asset").bytes,
        PNG_1X1
    );
}

#[test]
fn refuses_to_save_when_a_referenced_asset_is_missing() {
    let source = archive_with_manifest(
        &valid_manifest(),
        &[("assets/page.png", PNG_1X1)],
    );
    let mut loaded = load_archive(Cursor::new(source)).expect("source project");
    loaded.assets.clear();

    let error = save_archive(&loaded, Cursor::new(Vec::new()))
        .expect_err("missing asset must fail");

    assert!(error.to_string().contains("Page asset is missing"));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn atomic_save_replaces_valid_files_and_preserves_old_files_on_error() {
    let directory = temporary_test_directory();
    let path = directory.join("project.docscan");
    fs::write(&path, b"old project").unwrap();
    let source = archive_with_manifest(
        &valid_manifest(),
        &[("assets/page.png", PNG_1X1)],
    );
    let mut loaded = load_archive(Cursor::new(source)).expect("source project");

    loaded.project.pages[0].source_pixel_width = 2;
    save_archive_file(&loaded, &path).expect_err("invalid save must fail");
    assert_eq!(fs::read(&path).unwrap(), b"old project");
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);

    loaded.project.pages[0].source_pixel_width = 1;
    save_archive_file(&loaded, &path).expect("atomic save");
    let reloaded = load_archive_file(&path).expect("committed project");
    assert_eq!(reloaded.project, loaded.project);
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);

    fs::remove_dir_all(directory).unwrap();
}

fn assert_invalid(bytes: Vec<u8>, expected: &str) {
    let error = load_archive(Cursor::new(bytes)).expect_err("invalid archive");
    assert!(
        matches!(error, ProjectLoadError::Invalid(_)),
        "unexpected error kind: {error}"
    );
    assert!(
        error.to_string().contains(expected),
        "expected {expected:?}, got {error}"
    );
}

fn archive_with_manifest(
    manifest: &Value,
    entries: &[(&str, &[u8])],
) -> Vec<u8> {
    let mut files = vec![("project.json", manifest_bytes(manifest))];
    files.extend(entries.iter().map(|(name, bytes)| (*name, bytes.to_vec())));
    archive_with_entries(&files)
}

fn archive_with_entries(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        let method = if *name == "project.json" {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        };
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn manifest_bytes(manifest: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(&manifest).unwrap()
}

fn valid_manifest() -> Value {
    let point = |x: f32, y: f32| json!({"x": x, "y": y, "vector": {}});
    let transform = || {
        json!({
            "m11": 1.0,
            "m12": 0.0,
            "m21": 0.0,
            "m22": 1.0,
            "m31": 0.0,
            "m32": 0.0
        })
    };
    json!({
        "schemaVersion": 1,
        "id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "title": "Compatibility fixture",
        "createdUtc": "2026-09-15T09:19:06.8877742+00:00",
        "updatedUtc": "2026-09-15T09:20:31.9428559+00:00",
        "pages": [{
            "id": "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "sourceAsset": "page.png",
            "sourcePixelWidth": 1,
            "sourcePixelHeight": 1,
            "crop": {
                "topLeft": point(0.0, 0.0),
                "topRight": point(1.0, 0.0),
                "bottomRight": point(1.0, 1.0),
                "bottomLeft": point(0.0, 1.0)
            },
            "rotationQuarterTurns": 0,
            "filter": {"mode": 0, "contrast": 0},
            "annotations": [
                {
                    "kind": "text",
                    "text": "Hello\nworld",
                    "bounds": {
                        "x": 0.1,
                        "y": 0.1,
                        "width": 0.4,
                        "height": 0.2,
                        "right": 0.5,
                        "bottom": 0.3
                    },
                    "fontSize": 24.0,
                    "fontFamily": "Segoe UI",
                    "maximumWidth": 0.4,
                    "isBold": true,
                    "isItalic": true,
                    "isUnderlined": true,
                    "id": "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
                    "colorArgb": 4_294_901_760_u32,
                    "strokeWidth": 3.0,
                    "transform": transform()
                },
                {
                    "kind": "freehand",
                    "points": [point(0.2, 0.2), point(0.4, 0.4)],
                    "isHighlighter": true,
                    "filled": false,
                    "id": "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
                    "colorArgb": 4_294_967_040_u32,
                    "strokeWidth": 12.0,
                    "transform": transform()
                },
                {
                    "kind": "shape",
                    "shape": 2,
                    "start": point(0.2, 0.5),
                    "end": point(0.8, 0.8),
                    "filled": true,
                    "id": "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
                    "colorArgb": 4_278_190_335_u32,
                    "strokeWidth": 4.0,
                    "transform": transform()
                }
            ]
        }]
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn temporary_test_directory() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "fastfill-project-save-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    path
}
