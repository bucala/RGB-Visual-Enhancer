use std::env;

fn main() {
    let mut attrs = tauri_build::Attributes::new();

    // Tauri needs Common Controls v6, which requires an application manifest.
    // tauri-build embeds its manifest only into the app binary, so test
    // binaries crash on Windows with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139).
    // With the MSVC linker, embed the manifest via link args instead: those
    // apply to every binary of this crate, test harnesses included.
    let is_msvc = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if is_msvc {
        attrs =
            attrs.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest = env::current_dir()
            .expect("build script cwd")
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
