use fastfill_rust::geometry::{Navigation, Point, Rect, Size, fit_contain};

const EPSILON: f32 = 0.001;

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < EPSILON);
}

#[test]
fn portrait_page_fits_and_centers_in_a_wide_viewport() {
    let page = Size::new(210.0, 297.0);
    let viewport = Rect::new(20.0, 30.0, 1160.0, 700.0);

    let view = fit_contain(page, viewport).expect("valid page and viewport");

    close(view.page_rect().height, 700.0);
    close(view.page_rect().width, 700.0 * 210.0 / 297.0);
    close(view.page_rect().center().x, viewport.center().x);
    close(view.page_rect().center().y, viewport.center().y);
}

#[test]
fn page_and_view_coordinates_round_trip() {
    let page = Size::new(210.0, 297.0);
    let viewport = Rect::new(12.0, 34.0, 900.0, 500.0);
    let view = fit_contain(page, viewport).expect("valid page and viewport");
    let original = Point::new(37.25, 248.75);

    let restored = view.view_to_page(view.page_to_view(original));

    close(restored.x, original.x);
    close(restored.y, original.y);
}

#[test]
fn invalid_dimensions_are_rejected() {
    let viewport = Rect::new(0.0, 0.0, 100.0, 100.0);

    assert!(fit_contain(Size::new(0.0, 20.0), viewport).is_none());
    assert!(fit_contain(Size::new(f32::NAN, 20.0), viewport).is_none());
    assert!(
        fit_contain(
            Size::new(20.0, 20.0),
            Rect::new(0.0, 0.0, f32::INFINITY, 100.0,)
        )
        .is_none()
    );
}

#[test]
fn zoom_keeps_the_page_point_under_the_pointer() {
    let page = Size::new(1_000.0, 2_000.0);
    let viewport = Rect::new(10.0, 20.0, 800.0, 600.0);
    let mut navigation = Navigation::default();
    let anchor = Point::new(275.0, 190.0);
    let before = navigation
        .view(page, viewport)
        .expect("valid view")
        .view_to_page(anchor);

    navigation.zoom_at(2.0, anchor, page, viewport);

    let after = navigation
        .view(page, viewport)
        .expect("valid view")
        .view_to_page(anchor);
    close(after.x, before.x);
    close(after.y, before.y);
}

#[test]
fn pan_and_fit_change_then_restore_the_page_origin() {
    let page = Size::new(100.0, 200.0);
    let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
    let mut navigation = Navigation::default();
    let original = navigation.view(page, viewport).expect("valid view");

    navigation.pan_by(Point::new(25.0, -40.0));
    let moved = navigation.view(page, viewport).expect("valid view");
    close(moved.page_rect().x, original.page_rect().x + 25.0);
    close(moved.page_rect().y, original.page_rect().y - 40.0);

    navigation.fit();
    assert_eq!(navigation.view(page, viewport), Some(original));
}

#[test]
fn one_to_one_maps_one_image_pixel_to_one_view_unit() {
    let page = Size::new(2_000.0, 1_000.0);
    let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
    let mut navigation = Navigation::default();

    navigation.one_to_one(page, viewport);
    let view = navigation.view(page, viewport).expect("valid view");
    let origin = view.page_to_view(Point::new(0.0, 0.0));
    let adjacent = view.page_to_view(Point::new(1.0, 1.0));

    close(adjacent.x - origin.x, 1.0);
    close(adjacent.y - origin.y, 1.0);
}

#[test]
fn zoom_is_clamped_and_invalid_changes_are_ignored() {
    let page = Size::new(100.0, 100.0);
    let viewport = Rect::new(0.0, 0.0, 500.0, 500.0);
    let anchor = viewport.center();
    let mut navigation = Navigation::default();

    navigation.zoom_at(f32::INFINITY, anchor, page, viewport);
    close(navigation.zoom(), 1.0);
    navigation.zoom_at(1_000_000.0, anchor, page, viewport);
    close(navigation.zoom(), Navigation::MAX_ZOOM);
    navigation.zoom_at(0.000_001, anchor, page, viewport);
    close(navigation.zoom(), Navigation::MIN_ZOOM);
}
