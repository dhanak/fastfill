use fastfill_rust::annotations::{AnnotationColor, TextFormat, TextTypeface};
use iced::font::{Family, Style, Weight};
use iced::{Color, Font};
use std::sync::{Once, OnceLock};

pub(crate) const TOOLBAR_HEIGHT: f32 = 168.0;
pub(crate) const TOOLBAR_ROW_HEIGHT: f32 = TOOLBAR_HEIGHT / 3.0;
pub(crate) const CONTROL_HEIGHT: f32 = 40.0;
pub(crate) const CONTROL_GAP: f32 = 8.0;
pub(crate) const TOOLBAR_PADDING: [u16; 2] = [8, 12];
pub(crate) const PAGE_MARGIN: f32 = 36.0;
pub(crate) const PAGE_SHADOW_X: f32 = 5.0;
pub(crate) const PAGE_SHADOW_Y: f32 = 7.0;
pub(crate) const SELECTION_HANDLE_SIZE: f32 = 14.0;
pub(crate) const SELECTION_STROKE: f32 = 2.0;

pub(crate) fn accent() -> Color {
    Color::from_rgb8(0, 102, 204)
}

pub(crate) fn canvas_background() -> Color {
    Color::from_rgb8(67, 72, 78)
}

pub(crate) fn page_shadow() -> Color {
    Color::from_rgba8(0, 0, 0, 0.22)
}

pub(crate) fn annotation_color(color: AnnotationColor) -> Color {
    let [red, green, blue] = color.rgb();
    Color::from_rgb8(red, green, blue)
}

pub(crate) fn text_font(format: TextFormat) -> Font {
    ensure_generic_font_fallbacks();
    Font {
        family: match format.typeface {
            TextTypeface::Sans => Family::SansSerif,
            TextTypeface::Serif => Family::Serif,
            TextTypeface::Monospace => Family::Monospace,
        },
        weight: if format.bold {
            Weight::Bold
        } else {
            Weight::Normal
        },
        style: if format.italic && italic_face_available(format.typeface) {
            Style::Italic
        } else {
            Style::Normal
        },
        ..Font::DEFAULT
    }
}

fn italic_face_available(typeface: TextTypeface) -> bool {
    use iced::advanced::graphics::text::cosmic_text::fontdb;

    static AVAILABLE: OnceLock<[bool; 3]> = OnceLock::new();
    let available = AVAILABLE.get_or_init(|| {
        let mut system = iced::advanced::graphics::text::font_system()
            .write()
            .expect("font system lock");
        let database = system.raw().db_mut();
        [
            fontdb::Family::SansSerif,
            fontdb::Family::Serif,
            fontdb::Family::Monospace,
        ]
        .map(|family| {
            let name = database.family_name(&family);
            database.faces().any(|face| {
                face.style == fontdb::Style::Italic
                    && face.families.iter().any(|family| family.0 == name)
            })
        })
    });
    available[match typeface {
        TextTypeface::Sans => 0,
        TextTypeface::Serif => 1,
        TextTypeface::Monospace => 2,
    }]
}

fn ensure_generic_font_fallbacks() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        use iced::advanced::graphics::text::cosmic_text::fontdb;

        let mut system = iced::advanced::graphics::text::font_system()
            .write()
            .expect("font system lock");
        let database = system.raw().db_mut();
        let sans = database.family_name(&fontdb::Family::SansSerif).to_owned();
        if !font_family_exists(database, &sans) {
            let family = first_font_family(
                database,
                &[
                    "Segoe UI",
                    "Arial",
                    "Helvetica",
                    "DejaVu Sans",
                    "Liberation Sans",
                    "Noto Sans",
                    "Fira Sans",
                ],
            );
            database.set_sans_serif_family(family);
        }
        let serif = database.family_name(&fontdb::Family::Serif).to_owned();
        if !font_family_exists(database, &serif) {
            let family = first_font_family(
                database,
                &[
                    "Times New Roman",
                    "DejaVu Serif",
                    "Liberation Serif",
                    "Noto Serif",
                    "Georgia",
                    "Fira Sans",
                ],
            );
            database.set_serif_family(family);
        }
        let monospace =
            database.family_name(&fontdb::Family::Monospace).to_owned();
        if !font_family_exists(database, &monospace) {
            let family = first_font_family(
                database,
                &[
                    "Consolas",
                    "Menlo",
                    "DejaVu Sans Mono",
                    "Liberation Mono",
                    "Noto Sans Mono",
                    "Courier New",
                    "Fira Sans",
                ],
            );
            database.set_monospace_family(family);
        }
    });
}

fn first_font_family(
    database: &iced::advanced::graphics::text::cosmic_text::fontdb::Database,
    candidates: &'static [&'static str],
) -> &'static str {
    candidates
        .iter()
        .find(|candidate| font_family_exists(database, candidate))
        .copied()
        .unwrap_or("Fira Sans")
}

fn font_family_exists(
    database: &iced::advanced::graphics::text::cosmic_text::fontdb::Database,
    name: &str,
) -> bool {
    database.faces().any(|face| {
        face.families
            .iter()
            .any(|family| family.0.eq_ignore_ascii_case(name))
    })
}
