//! Run with a trusted .docscan path to measure the archive pipeline.
use fastfill_rust::project::{load_archive, load_archive_file, save_archive};
use std::io::Cursor;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let [path] = args.as_slice() else {
        return Err("Usage: project_timing <trusted-project.docscan>".into());
    };
    let start = Instant::now();
    let loaded = load_archive_file(path)?;
    println!("Open and validate: {:?}", start.elapsed());
    let pixels = loaded
        .assets
        .values()
        .map(|asset| u64::from(asset.info.width) * u64::from(asset.info.height))
        .sum::<u64>();
    println!(
        "{} pages, {pixels} decoded pixels",
        loaded.project.pages.len()
    );
    let start = Instant::now();
    let saved = save_archive(&loaded, Cursor::new(Vec::new()))?.into_inner();
    println!("Validate and write: {:?}", start.elapsed());
    let start = Instant::now();
    let reloaded = load_archive(Cursor::new(saved))?;
    println!("Read-back validation: {:?}", start.elapsed());
    assert_eq!(loaded.project, reloaded.project);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let directory = std::env::temp_dir()
            .join(format!("fastfill-timing-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory)?;
        let path = directory.join("timing.docscan");
        let start = Instant::now();
        let result = fastfill_rust::project::save_archive_file(&loaded, &path);
        println!("Atomic save with verification: {:?}", start.elapsed());
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        std::fs::remove_dir(directory)?;
        result?;
    }
    Ok(())
}
