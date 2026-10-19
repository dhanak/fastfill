use fastfill_rust::annotations::{
    AnnotationColor, AnnotationRef, Annotations, DragModifiers, Pointer,
    TextFormat, TextTypeface, Tool,
};
use fastfill_rust::geometry::{Point, Size};
use fastfill_rust::image_input::validate_image;
use fastfill_rust::project::{
    AffineTransform, CropQuad, LoadedAsset, LoadedProject, NormalizedPoint,
    PageFilterSettings, Project, ProjectAnnotation, ProjectPage,
    ProjectShapeKind, decode_editor_page, editor_page_size, encode_editor_page,
    load_archive, save_archive,
};
use std::collections::BTreeMap;
use std::io::Cursor;
use uuid::Uuid;

const MOUSE: Pointer = Pointer::Mouse;

#[test]
fn every_tool_survives_schema_conversion_with_layer_order_and_identity() {
    let source = blank_page();
    let size = editor_page_size(&source).unwrap();
    assert_eq!(size, Size::new(612.0, 918.0));
    let mut editor = Annotations::default();
    for tool in Tool::ALL {
        if !tool.creates_annotations() || tool == Tool::Text {
            continue;
        }
        editor.set_tool(tool);
        editor.begin(MOUSE, Point::new(60.0, 70.0), size);
        editor.drag(MOUSE, Point::new(120.0, 80.0), false);
        editor.finish(MOUSE, Point::new(180.0, 150.0), false);
    }
    editor.set_tool(Tool::Text);
    editor.set_text_format(TextFormat {
        typeface: TextTypeface::Monospace,
        bold: true,
        italic: true,
        underline: true,
        ..TextFormat::default()
    });
    editor.begin_text(Point::new(80.0, 250.0), size, 1.0);
    editor.update_text("Form\nentry", Size::new(120.0, 62.4));
    editor.accept_text();
    assert!(editor.send_selected_to_back());

    let manifest = encode_editor_page(&source, &editor).unwrap();
    let decoded = decode_editor_page(&manifest).unwrap();
    assert_eq!(manifest.annotations.len(), 9);
    assert_eq!(decoded.len(), editor.len());
    assert!(!decoded.can_undo());
    assert_eq!(decoded.revision(), 0);
    assert_eq!(encode_editor_page(&manifest, &decoded).unwrap(), manifest);
    let ids = |annotations: &Annotations| {
        annotations
            .ordered_annotations()
            .map(|annotation| match annotation {
                AnnotationRef::Shape(_, shape) => shape.id,
                AnnotationRef::Text(_, text) => text.id,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&editor), ids(&decoded));
    for (first, second) in editor
        .shapes()
        .zip(decoded.shapes().cloned().collect::<Vec<_>>())
    {
        assert_eq!(first.kind(), second.kind());
        assert_eq!(first.color, second.color);
        assert_eq!(first.filled, second.filled);
        assert!((first.bounds().x - second.bounds().x).abs() < 0.001);
        assert!((first.bounds().width - second.bounds().width).abs() < 0.001);
    }

    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        600,
        900,
        image::Rgb([255, 255, 255]),
    ))
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    let bytes = png.into_inner();
    let info = validate_image(&bytes).unwrap();
    let archive = LoadedProject {
        project: Project {
            schema_version: 1,
            id: Uuid::from_u128(10),
            title: "Editor fixture".into(),
            created_utc: time::OffsetDateTime::UNIX_EPOCH,
            updated_utc: time::OffsetDateTime::UNIX_EPOCH,
            pages: vec![manifest],
        },
        assets: BTreeMap::from([(
            "page.png".into(),
            LoadedAsset { bytes, info },
        )]),
    };
    let mut zip = Cursor::new(Vec::new());
    save_archive(&archive, &mut zip).unwrap();
    zip.set_position(0);
    let loaded = load_archive(zip).unwrap();
    assert_eq!(loaded.project, archive.project);
    let page = &loaded.project.pages[0];
    let editor = decode_editor_page(page).unwrap();
    assert_eq!(encode_editor_page(page, &editor).unwrap(), *page);
}

#[test]
fn rigid_transform_on_a_portrait_page_does_not_flatten_rotation() {
    let mut source = blank_page();
    source.source_pixel_height = 1_200;
    let angle = 0.5_f32;
    let (sin, cos) = angle.sin_cos();
    source.annotations.push(ProjectAnnotation::Shape {
        id: Uuid::from_u128(2),
        shape: ProjectShapeKind::Rectangle,
        start: NormalizedPoint { x: 0.1, y: 0.2 },
        end: NormalizedPoint { x: 0.3, y: 0.4 },
        filled: true,
        color_argb: 0x8033_ffaa,
        stroke_width: 6.0,
        transform: AffineTransform {
            m11: cos,
            m12: sin / 2.0,
            m21: -sin * 2.0,
            m22: cos,
            m31: 0.2 - cos * 0.2 + sin * 2.0 * 0.3 + 0.05,
            m32: 0.3 - sin / 2.0 * 0.2 - cos * 0.3,
        },
    });

    let editor = decode_editor_page(&source).unwrap();
    let shape = &editor.shapes().next().unwrap().clone();
    assert!((shape.rotation - angle).abs() < 0.0001);
    assert_eq!(shape.id, Uuid::from_u128(2));
    assert_eq!(shape.color.to_argb(), 0x8033_ffaa);
    assert!((shape.bounds().width - 122.4).abs() < 0.001);
    assert!((shape.bounds().height - 244.8).abs() < 0.001);
    assert_eq!(encode_editor_page(&source, &editor).unwrap(), source);
}

#[test]
fn editing_a_loaded_object_preserves_identity_and_supports_undo() {
    let source = blank_page();
    let size = editor_page_size(&source).unwrap();
    let mut first = Annotations::default();
    first.set_tool(Tool::Rectangle);
    first.begin(MOUSE, Point::new(60.0, 70.0), size);
    first.finish(MOUSE, Point::new(180.0, 150.0), false);
    let original_id = first.shapes().next().unwrap().id;
    let manifest = encode_editor_page(&source, &first).unwrap();
    let mut editor = decode_editor_page(&manifest).unwrap();
    editor.set_tool(Tool::Select);
    editor.begin_interaction(
        MOUSE,
        Point::new(120.0, 110.0),
        size,
        1.0,
        DragModifiers::default(),
    );
    assert!(editor.finish(MOUSE, Point::new(120.0, 110.0), false));
    editor.begin_interaction(
        MOUSE,
        Point::new(120.0, 110.0),
        size,
        1.0,
        DragModifiers::default(),
    );
    editor.finish(MOUSE, Point::new(150.0, 130.0), false);

    assert_eq!(editor.shapes().next().unwrap().id, original_id);
    assert!(editor.revision() > 0);
    let edited = encode_editor_page(&manifest, &editor).unwrap();
    assert_ne!(edited, manifest);
    editor.undo();
    assert_eq!(editor.revision(), 0);
    assert_eq!(encode_editor_page(&manifest, &editor).unwrap(), manifest);
}

#[test]
fn corrected_pages_use_output_geometry_and_keep_annotation_metadata() {
    let mut source = blank_page();
    source.crop.top_right.x = 0.5;
    source.crop.bottom_right.x = 0.5;
    source.rotation_quarter_turns = 1;
    source.filter.contrast = 10;
    let size = editor_page_size(&source).unwrap();
    assert_eq!(size, Size::new(612.0, 612.0 * 301.0 / 900.0));
    let mut editor = Annotations::default();
    editor.set_tool(Tool::Rectangle);
    editor.begin(MOUSE, Point::new(100.0, 60.0), size);
    editor.finish(MOUSE, Point::new(200.0, 100.0), false);
    let page = encode_editor_page(&source, &editor).unwrap();
    assert_eq!(page.crop, source.crop);
    assert_eq!(page.rotation_quarter_turns, 1);
    assert_eq!(page.filter, source.filter);
    let decoded = decode_editor_page(&page).unwrap();
    assert_eq!(encode_editor_page(&page, &decoded).unwrap(), page);
    let bounds = decoded.shapes().next().unwrap().bounds();
    assert!((bounds.x - 100.0).abs() < 0.001);
    assert!((bounds.y - 60.0).abs() < 0.001);
    assert!((bounds.width - 100.0).abs() < 0.001);
    assert!((bounds.height - 40.0).abs() < 0.001);
}

#[test]
fn unsupported_annotation_distortion_still_fails_explicitly() {
    let mut source = blank_page();
    source.annotations.push(ProjectAnnotation::Shape {
        id: Uuid::from_u128(2),
        shape: ProjectShapeKind::Rectangle,
        start: NormalizedPoint { x: 0.1, y: 0.2 },
        end: NormalizedPoint { x: 0.3, y: 0.4 },
        filled: false,
        color_argb: 0xff11_1827,
        stroke_width: 3.0,
        transform: AffineTransform {
            m21: 0.2,
            ..Default::default()
        },
    });
    assert!(
        decode_editor_page(&source)
            .unwrap_err()
            .to_string()
            .contains("transform")
    );
}

#[test]
fn pending_edits_cannot_be_serialized_as_committed_content() {
    let source = blank_page();
    let mut editor = Annotations::default();
    editor.set_tool(Tool::Rectangle);
    editor.begin(
        MOUSE,
        Point::new(60.0, 70.0),
        editor_page_size(&source).unwrap(),
    );
    assert!(
        encode_editor_page(&source, &editor)
            .unwrap_err()
            .to_string()
            .contains("pending")
    );
}

#[test]
fn content_revisions_track_commits_not_selection_defaults_or_cancel() {
    let page = Size::new(612.0, 918.0);
    let mut editor = Annotations::default();
    editor.set_tool(Tool::Rectangle);
    editor.set_color(AnnotationColor::Blue);
    assert_eq!(editor.revision(), 0);
    editor.begin(MOUSE, Point::new(60.0, 70.0), page);
    let id = editor.preview().unwrap().id;
    editor.drag(MOUSE, Point::new(180.0, 150.0), false);
    assert_eq!(editor.preview().unwrap().id, id);
    editor.finish(MOUSE, Point::new(180.0, 150.0), false);
    assert_eq!(editor.shapes().next().unwrap().id, id);
    let saved = editor.revision();
    assert!(saved > 0);
    editor.set_tool(Tool::Select);
    editor.begin_interaction(
        MOUSE,
        Point::new(120.0, 110.0),
        page,
        1.0,
        DragModifiers::default(),
    );
    editor.finish(MOUSE, Point::new(120.0, 110.0), false);
    assert_eq!(editor.revision(), saved);
    editor.set_tool(Tool::Oval);
    editor.begin(MOUSE, Point::new(250.0, 250.0), page);
    editor.cancel();
    assert_eq!(editor.revision(), saved);
    editor.delete_all();
    let deleted = editor.revision();
    assert_ne!(deleted, saved);
    editor.undo();
    assert_eq!(editor.revision(), saved);
    editor.redo();
    assert_eq!(editor.revision(), deleted);
    editor.undo();
    editor.delete_selected();
    assert!(editor.revision() > deleted);
    assert!(!editor.can_redo());
}

#[test]
fn palette_and_custom_argb_preserve_exact_alpha() {
    for value in [
        0,
        0xff00_0000,
        0xffff_ffff,
        0xffdc_2020,
        0xff00_66cc,
        0xff00_a048,
        0xfff5_c400,
        0x8033_ffaa,
        0xff11_1827,
    ] {
        assert_eq!(AnnotationColor::from_argb(value).to_argb(), value);
    }
    assert_eq!(
        AnnotationColor::from_argb(0xff00_66cc),
        AnnotationColor::Blue
    );
    assert!(
        (AnnotationColor::from_argb(0x8033_ffaa).opacity() - 128.0 / 255.0)
            .abs()
            < f32::EPSILON
    );
}

#[test]
fn unsupported_fonts_fail_but_legacy_width_hints_load_without_data_loss() {
    let source = blank_page();
    let page = editor_page_size(&source).unwrap();
    let mut editor = Annotations::default();
    editor.set_tool(Tool::Text);
    editor.begin_text(Point::new(80.0, 250.0), page, 1.0);
    editor.update_text("Sample", Size::new(120.0, 31.2));
    editor.accept_text();
    let mut manifest = encode_editor_page(&source, &editor).unwrap();
    if let ProjectAnnotation::Text { font_family, .. } =
        &mut manifest.annotations[0]
    {
        *font_family = "Unknown family".into();
    }
    assert!(
        decode_editor_page(&manifest)
            .unwrap_err()
            .to_string()
            .contains("font family")
    );
    if let ProjectAnnotation::Text {
        font_family,
        maximum_width,
        ..
    } = &mut manifest.annotations[0]
    {
        *font_family = "Segoe UI".into();
        *maximum_width = 0.4;
    }
    let editor = decode_editor_page(&manifest).unwrap();
    assert_eq!(editor.texts().next().unwrap().text, "Sample");
    assert_eq!(encode_editor_page(&manifest, &editor).unwrap(), manifest);
}

#[test]
fn early_csharp_text_defaults_preserve_content_bounds_and_save_data() {
    let mut source = blank_page();
    let legacy = serde_json::json!({
        "kind": "text",
        "id": "eaebc00a-591e-411d-9c24-9a204cc341c4",
        "text": "hahó\nbubu",
        "fontSize": 15,
        "bounds": { "x": 0.2, "y": 0.3, "width": 0.4, "height": 0.12 }
    });
    source
        .annotations
        .push(serde_json::from_value(legacy).unwrap());
    let editor = decode_editor_page(&source).unwrap();
    assert_eq!(editor.texts().next().unwrap().text, "hahó\nbubu");
    assert_eq!(editor.texts().next().unwrap().format.font_size, 15.0);
    assert_eq!(
        editor.texts().next().unwrap().format.typeface,
        TextTypeface::Sans
    );
    assert!((editor.texts().next().unwrap().bounds.x - 122.4).abs() < 0.001);
    assert_eq!(encode_editor_page(&source, &editor).unwrap(), source);
}

#[test]
fn legacy_csharp_font_choices_load_and_survive_non_font_edits() {
    for (family, typeface) in [
        ("Segoe UI", TextTypeface::Sans),
        ("Arial", TextTypeface::Sans),
        ("Calibri", TextTypeface::Sans),
        ("Times New Roman", TextTypeface::Serif),
        ("Consolas", TextTypeface::Monospace),
    ] {
        let mut source = blank_page();
        source.annotations.push(
            serde_json::from_value(serde_json::json!({
                "kind": "text", "id": Uuid::new_v4(),
                "text": "Legacy text", "fontSize": 15,
                "fontFamily": family,
                "bounds": { "x": 0.2, "y": 0.3, "width": 0.4, "height": 0.12 }
            }))
            .unwrap(),
        );
        let mut editor = decode_editor_page(&source).unwrap();
        assert_eq!(editor.texts().next().unwrap().format.typeface, typeface);
        assert_eq!(encode_editor_page(&source, &editor).unwrap(), source);
        editor.set_tool(Tool::Select);
        let center = editor.texts().next().unwrap().bounds.center();
        editor.begin_interaction(
            MOUSE,
            center,
            editor_page_size(&source).unwrap(),
            1.0,
            DragModifiers::default(),
        );
        editor.finish(MOUSE, center, false);
        editor.set_color(AnnotationColor::Red);
        let edited = encode_editor_page(&source, &editor).unwrap();
        let ProjectAnnotation::Text {
            font_family,
            color_argb,
            ..
        } = &edited.annotations[0]
        else {
            panic!("text required")
        };
        assert_eq!(font_family, family);
        assert_eq!(*color_argb, AnnotationColor::Red.to_argb());
    }
}

#[test]
fn point_conversion_rejects_finite_schema_geometry_that_overflows() {
    let mut source = blank_page();
    source.source_pixel_width = 1;
    source.source_pixel_height = 1;
    source.annotations.push(ProjectAnnotation::Shape {
        id: Uuid::from_u128(2),
        shape: ProjectShapeKind::Rectangle,
        start: NormalizedPoint { x: 0.0, y: 0.0 },
        end: NormalizedPoint { x: 1e37, y: 0.2 },
        filled: false,
        color_argb: 0xff11_1827,
        stroke_width: 3.0,
        transform: AffineTransform::default(),
    });
    assert!(
        decode_editor_page(&source)
            .unwrap_err()
            .to_string()
            .contains("numeric range")
    );
}

#[test]
fn newly_rotated_shapes_and_ink_keep_frames_on_a_corrected_page() {
    let mut source = blank_page();
    source.crop = CropQuad {
        top_left: NormalizedPoint { x: 0.1, y: 0.1 },
        top_right: NormalizedPoint { x: 0.9, y: 0.1 },
        bottom_right: NormalizedPoint { x: 0.9, y: 0.9 },
        bottom_left: NormalizedPoint { x: 0.1, y: 0.9 },
    };
    source.rotation_quarter_turns = 1;
    let page = editor_page_size(&source).unwrap();
    for tool in [
        Tool::Rectangle,
        Tool::Oval,
        Tool::Checkmark,
        Tool::Cross,
        Tool::Pen,
        Tool::Highlighter,
    ] {
        let mut editor = Annotations::default();
        editor.set_tool(tool);
        editor.begin(MOUSE, Point::new(150.0, 150.0), page);
        editor.drag(MOUSE, Point::new(190.0, 220.0), false);
        editor.finish(MOUSE, Point::new(250.0, 180.0), false);
        editor.set_tool(Tool::Select);
        let shape = &editor.shapes().next().unwrap().clone();
        let id = shape.id;
        // Zoom in so narrow shapes expose their editing controls.
        let handle = shape.rotation_handle(4.0).unwrap();
        let center = shape.bounds().center();
        editor.begin_interaction(
            MOUSE,
            handle,
            page,
            4.0,
            DragModifiers::default(),
        );
        editor.finish(
            MOUSE,
            Point::new(center.x + 100.0, center.y + 50.0),
            false,
        );
        assert_eq!(editor.shapes().next().unwrap().id, id);
        assert!(editor.shapes().next().unwrap().rotation.abs() > 0.01);
        let expected = editor.shapes().next().unwrap().frame_corners().unwrap();
        let manifest = encode_editor_page(&source, &editor).unwrap();
        let decoded = decode_editor_page(&manifest).unwrap();
        let actual = decoded.shapes().next().unwrap().frame_corners().unwrap();
        for (expected, actual) in expected.into_iter().zip(actual) {
            assert!((expected.x - actual.x).abs() < 0.001, "{tool:?}");
            assert!((expected.y - actual.y).abs() < 0.001, "{tool:?}");
        }
        assert_eq!(decoded.shapes().next().unwrap().id, id);
    }
}

fn blank_page() -> ProjectPage {
    ProjectPage {
        id: Uuid::from_u128(1),
        source_asset: "page.png".into(),
        source_pixel_width: 600,
        source_pixel_height: 900,
        crop: CropQuad::default(),
        rotation_quarter_turns: 0,
        filter: PageFilterSettings::default(),
        brightness: 0,
        annotations: Vec::new(),
    }
}

fn mixed_contract_page() -> ProjectPage {
    let page = blank_page();
    let size = editor_page_size(&page).unwrap();
    let mut editor = Annotations::default();
    editor.set_tool(Tool::Rectangle);
    editor.set_color(AnnotationColor::Argb(0x8055_aa77));
    assert!(editor.begin(MOUSE, Point::new(76.5, 114.75), size));
    assert!(editor.finish(MOUSE, Point::new(-76.5, 229.5), false));
    editor.set_tool(Tool::Text);
    assert!(editor.begin_text(Point::new(153.0, 344.25), size, 1.0));
    editor.update_text("Contract\ntext", Size::new(76.5, 57.375));
    assert!(editor.accept_text());
    editor.set_tool(Tool::Line);
    assert!(editor.begin(MOUSE, Point::new(306.0, 573.75), size));
    assert!(editor.finish(MOUSE, Point::new(459.0, 688.5), false));
    encode_editor_page(&page, &editor).unwrap()
}

fn annotation_id(annotation: &ProjectAnnotation) -> Uuid {
    match annotation {
        ProjectAnnotation::Shape { id, .. }
        | ProjectAnnotation::Text { id, .. }
        | ProjectAnnotation::Freehand { id, .. } => *id,
    }
}

#[test]
fn mixed_history_matches_schema_oracle_without_storage_indices() {
    let source = mixed_contract_page();
    let size = editor_page_size(&source).unwrap();
    let mut editor = decode_editor_page(&source).unwrap();
    editor.set_tool(Tool::Select);
    assert!(editor.begin_interaction(
        MOUSE,
        Point::new(250.0, 500.0),
        size,
        1.0,
        DragModifiers::default(),
    ));
    assert!(editor.finish(MOUSE, Point::new(-100.0, 0.0), false));
    assert_eq!(editor.selected_count(), 2);
    let line_id = annotation_id(&source.annotations[2]);
    let mut expected = source.clone();
    let mut states = vec![(source.clone(), editor.revision())];
    for step in 0..6 {
        match step {
            0 => {
                assert!(editor.set_color(AnnotationColor::Argb(0x6066_7788)));
                for annotation in &mut expected.annotations[..2] {
                    match annotation {
                        ProjectAnnotation::Shape { color_argb, .. }
                        | ProjectAnnotation::Text { color_argb, .. } => {
                            *color_argb = 0x6066_7788;
                        }
                        ProjectAnnotation::Freehand { .. } => unreachable!(),
                    }
                }
            }
            1 => {
                assert!(editor.set_stroke_width(7.0));
                let ProjectAnnotation::Shape { stroke_width, .. } =
                    &mut expected.annotations[0]
                else {
                    unreachable!()
                };
                *stroke_width = 7.0;
            }
            2 => {
                assert!(editor.set_filled(true));
                let ProjectAnnotation::Shape { filled, .. } =
                    &mut expected.annotations[0]
                else {
                    unreachable!()
                };
                *filled = true;
            }
            3 => {
                assert!(editor.bring_selected_to_front());
                expected.annotations.rotate_right(1);
            }
            4 => {
                assert!(editor.send_selected_to_back());
                expected.annotations.rotate_left(1);
            }
            _ => {
                assert!(editor.delete_selected());
                expected.annotations.retain(|a| annotation_id(a) == line_id);
            }
        }
        let encoded = encode_editor_page(&source, &editor).unwrap();
        assert_eq!(encoded, expected, "operation {step}");
        let reopened = decode_editor_page(&encoded).unwrap();
        assert_eq!(encode_editor_page(&encoded, &reopened).unwrap(), encoded);
        states.push((expected.clone(), editor.revision()));
    }
    for (page, revision) in states[..states.len() - 1].iter().rev() {
        assert!(editor.undo());
        assert_eq!(encode_editor_page(&source, &editor).unwrap(), *page);
        assert_eq!(editor.revision(), *revision);
        assert_eq!(editor.selected_count(), 2);
    }
    assert!(!editor.can_undo());
    for (page, revision) in &states[1..] {
        assert!(editor.redo());
        assert_eq!(encode_editor_page(&source, &editor).unwrap(), *page);
        assert_eq!(editor.revision(), *revision);
    }
    assert!(!editor.has_selection());
    assert!(!editor.can_redo());
}

#[test]
fn reordered_mixed_group_keeps_geometry_and_text_edit_targets_by_id() {
    let source = mixed_contract_page();
    let size = editor_page_size(&source).unwrap();
    let mut editor = decode_editor_page(&source).unwrap();
    let shape = editor.shapes().next().unwrap().clone();
    let text = editor.texts().next().unwrap().clone();
    editor.set_tool(Tool::Select);
    assert!(editor.begin_interaction(
        MOUSE,
        Point::new(250.0, 500.0),
        size,
        1.0,
        DragModifiers::default(),
    ));
    assert!(editor.finish(MOUSE, Point::new(-100.0, 0.0), false));
    let selected = editor.selected_ids().to_vec();
    assert!(editor.bring_selected_to_front());
    let before_move = encode_editor_page(&source, &editor).unwrap();
    assert!(editor.begin_interaction(
        MOUSE,
        Point::new(160.0, 350.0),
        size,
        1.0,
        DragModifiers::default(),
    ));
    assert_eq!(
        editor
            .geometric_previews()
            .filter(|a| a.text().is_some())
            .count(),
        1
    );
    assert_eq!(
        editor
            .geometric_previews()
            .filter(|a| a.shape().is_some())
            .count(),
        1
    );
    assert!(editor.finish(MOUSE, Point::new(180.0, 380.0), false));
    assert_eq!(editor.selected_ids(), selected);
    let moved = editor.shapes().find(|a| a.id == shape.id).unwrap();
    assert_eq!(moved.bounds().x, shape.bounds().x + 20.0);
    assert_eq!(moved.bounds().y, shape.bounds().y + 30.0);
    let moved = editor.texts().find(|a| a.id == text.id).unwrap();
    assert_eq!(moved.bounds.x, text.bounds.x + 20.0);
    assert_eq!(moved.bounds.y, text.bounds.y + 30.0);
    assert!(editor.undo());
    assert_eq!(encode_editor_page(&source, &editor).unwrap(), before_move);
    assert!(editor.redo());
    editor.set_tool(Tool::Text);
    assert!(editor.begin_text_edit(Point::new(180.0, 380.0), size, 1.0));
    editor.update_text("Edited after reorder", Size::new(150.0, 40.0));
    assert!(editor.accept_text());
    let saved = encode_editor_page(&source, &editor).unwrap();
    assert_eq!(saved.annotations[0], source.annotations[2]);
    assert_eq!(annotation_id(&saved.annotations[1]), shape.id);
    assert_eq!(annotation_id(&saved.annotations[2]), text.id);
    let reopened = decode_editor_page(&saved).unwrap();
    assert_eq!(
        reopened.texts().next().unwrap().text,
        "Edited after reorder"
    );
    assert_eq!(encode_editor_page(&saved, &reopened).unwrap(), saved);
}

#[test]
fn pending_text_formats_cancel_or_accept_without_changing_other_layers() {
    let source = mixed_contract_page();
    let size = editor_page_size(&source).unwrap();
    let mut editor = decode_editor_page(&source).unwrap();
    editor.set_tool(Tool::Text);
    let point = Point::new(160.0, 350.0);
    let format = TextFormat {
        typeface: TextTypeface::Monospace,
        font_size: 32.0,
        bold: true,
        italic: true,
        underline: true,
        color: AnnotationColor::Argb(0x8033_66ff),
    };
    for accept in [false, true] {
        assert!(editor.begin_text_edit(point, size, 1.0));
        assert!(editor.set_text_format(format));
        editor.update_text("Changed", Size::new(153.0, 57.375));
        assert!(encode_editor_page(&source, &editor).is_err());
        assert_eq!(editor.revision(), 0);
        if accept {
            assert!(editor.accept_text());
        } else {
            editor.cancel();
            assert_eq!(encode_editor_page(&source, &editor).unwrap(), source);
        }
    }
    let changed = encode_editor_page(&source, &editor).unwrap();
    assert_eq!(changed.annotations[0], source.annotations[0]);
    assert_eq!(changed.annotations[2], source.annotations[2]);
    assert_eq!(
        annotation_id(&changed.annotations[1]),
        annotation_id(&source.annotations[1])
    );
    assert_ne!(changed.annotations[1], source.annotations[1]);
    assert!(editor.undo());
    assert_eq!(encode_editor_page(&source, &editor).unwrap(), source);
    assert!(!editor.can_undo());
    assert!(editor.redo());
    assert_eq!(encode_editor_page(&source, &editor).unwrap(), changed);
}
