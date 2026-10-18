//! Builds `src/fake_cli_shim.rs`, the program of a fake CLI on Windows (`fake_executable`), for a Windows target, with
//! the rustc, target, linker and flags of this build, and names it in `FAKE_CLI_SHIM`. A build that can't link it (a
//! check for Windows on another OS) warns and leaves `FAKE_CLI_SHIM` unset: a fake then panics with that reason.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

fn main() {
  println!("cargo:rerun-if-changed=src/fake_cli_shim.rs");
  if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
    return;
  }
  let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("src/fake_cli_shim.rs");
  let shim = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("fake-cli-shim.exe");
  let mut rustc = Command::new(env::var_os("RUSTC").expect("RUSTC"));
  rustc
    .args(["--edition", "2024", "--crate-type", "bin", "--crate-name", "fake_cli_shim", "--target"])
    .arg(env::var_os("TARGET").expect("TARGET"));
  if let Some(linker) = env::var_os("RUSTC_LINKER") {
    let mut option = OsString::from("linker=");
    option.push(linker);
    rustc.arg("-C").arg(option);
  }
  let flags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
  rustc.args(flags.split('\x1f').filter(|flag| !flag.is_empty()));
  rustc.arg("-o").arg(&shim).arg(&source);
  let failure = match rustc.output() {
    Ok(output) if output.status.success() => {
      println!("cargo:rustc-env=FAKE_CLI_SHIM={}", shim.display());
      return;
    }
    Ok(output) => String::from_utf8_lossy(&output.stderr).into_owned(),
    Err(e) => e.to_string(),
  };
  println!("cargo:warning=cannot build the fake CLI shim, so a fake CLI of a test can't run:");
  for line in failure.lines().filter(|line| !line.trim().is_empty()).take(20) {
    println!("cargo:warning={line}");
  }
}
