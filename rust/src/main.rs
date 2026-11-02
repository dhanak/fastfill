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
    #[cfg(not(target_arch = "wasm32"))]
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    {
        use fastfill_rust::camera::{catalog, stream};
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
    #[cfg(not(target_arch = "wasm32"))]
    return app::run(project_argument(&args));
    #[cfg(target_arch = "wasm32")]
    app::run(None)
}

#[cfg(not(target_arch = "wasm32"))]
fn project_argument(args: &[std::ffi::OsString]) -> Option<std::path::PathBuf> {
    args.iter().find_map(|arg| {
        let path = std::path::PathBuf::from(arg);
        path.extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("docscan"))
            .then_some(path)
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod cli_tests {
    use super::project_argument;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn first_docscan_argument_matches_csharp_launch() {
        assert_eq!(project_argument(&[]), None);
        let args = [
            OsString::from("--ignored"),
            OsString::from("picture.png"),
            OsString::from("Műszer/first.DOCSCAN"),
            OsString::from("second.docscan"),
        ];
        assert_eq!(
            project_argument(&args),
            Some(PathBuf::from("Műszer/first.DOCSCAN"))
        );
        assert_eq!(project_argument(&args[..2]), None);
    }
}
