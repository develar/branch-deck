use tauri_build::{Attributes, WindowsAttributes};

fn main() {
  // tauri-build embeds its manifest (Common-Controls 6.0) through a resource linked into the binaries only
  // (`rustc-link-arg-bins`), so the unit-test exe of `branch_deck_lib` loaded comctl32 v5 and aborted on Windows with
  // 0xc0000139 (an entry point v6 has). The linker embeds the same manifest into every exe of the package instead.
  let windows = if is_windows_msvc() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    WindowsAttributes::new_without_app_manifest()
  } else {
    WindowsAttributes::new()
  };
  tauri_build::try_build(Attributes::new().windows_attributes(windows)).expect("tauri-build failed");
}

fn is_windows_msvc() -> bool {
  std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") && std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc")
}
