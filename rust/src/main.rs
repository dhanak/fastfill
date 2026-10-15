mod app;
mod appearance;
#[cfg(target_arch = "wasm32")]
mod browser_download;
mod page_canvas;
mod picker;

fn main() -> iced::Result {
    app::run()
}
