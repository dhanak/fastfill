#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod appearance;
#[cfg(target_arch = "wasm32")]
mod browser_download;
mod icons;
mod page_canvas;
mod picker;

fn main() -> iced::Result {
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    {
        use fastfill_rust::camera::{catalog, stream};
        let args = std::env::args_os().skip(1).collect::<Vec<_>>();
        let worker = match args.first().and_then(|arg| arg.to_str()) {
            Some(stream::WORKER_FLAG) => Some(stream::worker(&args[1..])),
            Some(stream::QUERY_FLAG) => Some(catalog::worker(&args[1..])),
            _ => None,
        };
        if let Some(result) = worker {
            if let Err(error) = result {
                eprintln!("Camera worker: {error}");
                std::process::exit(1);
            }
            return Ok(());
        }
    }
    app::run()
}
