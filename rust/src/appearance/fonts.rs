//! Browser font faces. Native builds keep using installed system fonts.

use iced::advanced::graphics::text::cosmic_text::fontdb;

pub(super) fn load(database: &mut fontdb::Database) {
    macro_rules! face {
        ($name:literal) => {
            include_bytes!(concat!("../../assets/fonts/", $name)).as_slice()
        };
    }
    for bytes in [
        face!("LiberationSans-Regular.ttf"),
        face!("LiberationSans-Bold.ttf"),
        face!("LiberationSans-Italic.ttf"),
        face!("LiberationSans-BoldItalic.ttf"),
        face!("LiberationSerif-Regular.ttf"),
        face!("LiberationSerif-Bold.ttf"),
        face!("LiberationSerif-Italic.ttf"),
        face!("LiberationSerif-BoldItalic.ttf"),
        face!("LiberationMono-Regular.ttf"),
        face!("LiberationMono-Bold.ttf"),
        face!("LiberationMono-Italic.ttf"),
        face!("LiberationMono-BoldItalic.ttf"),
    ] {
        database.load_font_data(bytes.to_vec());
    }
}
