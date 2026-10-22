use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/windows.rc");
    println!("cargo:rerun-if-changed=assets/AppIcon.ico");

    // The portable Windows build uses MSVC and its installed Windows SDK.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }

    let program_files = env::var_os("ProgramFiles(x86)")
        .expect("Windows Program Files missing");
    let sdk = PathBuf::from(program_files).join("Windows Kits/10/bin");
    let compiler = fs::read_dir(sdk)
        .expect("Windows SDK missing")
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("x64/rc.exe"))
        .filter(|path| path.is_file())
        .max()
        .expect("Windows SDK resource compiler missing");
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR missing"))
            .join("app.res");
    let status = Command::new(compiler)
        .arg("/nologo")
        .arg(format!("/fo{}", output.display()))
        .arg("assets/windows.rc")
        .status()
        .expect("Could not run Windows resource compiler");
    assert!(status.success(), "Could not compile Windows app icon");
    println!(
        "cargo:rustc-link-arg-bin=fastfill-rust={}",
        output.display()
    );
}
