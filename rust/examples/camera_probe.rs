//! Standalone diagnostic using the same native adapter as the GUI.

#[cfg(not(target_arch = "wasm32"))]
use fastfill_rust::camera as native;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(error) = native::run() {
        eprintln!("Camera probe failed: {error}");
        std::process::exit(1);
    }
    #[cfg(target_arch = "wasm32")]
    eprintln!("The camera probe requires native Linux or Windows.");
}
