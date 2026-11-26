//! Embed the program icon (and version details) in the Windows .exe; on
//! macOS, link clang's runtime library.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/atari_editor.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        link_clang_runtime();
    }
    #[cfg(windows)]
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icons/atari_editor.ico");
        res.set("ProductName", "Edit Atari");
        res.set("FileDescription", "Edit Atari");
        if let Err(e) = res.compile() {
            panic!("embedding the Windows icon failed: {e}");
        }
    }
}

/// SDL's Objective-C code (built in by the sdl2 crate) uses `@available`,
/// which newer Xcode versions compile into calls to
/// `__isPlatformVersionAtLeast`, in clang's runtime library. Rust links
/// with `-nodefaultlibs`, which leaves that library out, so name it here.
/// Without the library nothing is added (older Xcode doesn't need it).
fn link_clang_runtime() {
    let Ok(out) = std::process::Command::new("clang")
        .arg("--print-resource-dir")
        .output()
    else {
        return;
    };
    let dir = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
        .join("lib")
        .join("darwin");
    if dir.join("libclang_rt.osx.a").exists() {
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rustc-link-lib=static=clang_rt.osx");
    }
}
