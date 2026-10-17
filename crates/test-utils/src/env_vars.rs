//! Environment variables a test sets for itself, restored when the guard drops, also when the test panics

use std::ffi::{OsStr, OsString};

/// Sets `vars` until the guard drops, then restores the values they had
#[must_use = "the variables are restored when the guard drops"]
pub struct EnvVars {
  previous: Vec<(String, Option<OsString>)>,
}

impl EnvVars {
  pub fn set<V: AsRef<OsStr>>(vars: &[(&str, V)]) -> Self {
    let previous = vars.iter().map(|(name, _)| ((*name).to_string(), std::env::var_os(name))).collect();
    for (name, value) in vars {
      write(name, Some(value.as_ref()));
    }
    Self { previous }
  }
}

impl Drop for EnvVars {
  fn drop(&mut self) {
    for (name, value) in self.previous.drain(..).rev() {
      write(&name, value.as_deref());
    }
  }
}

#[expect(unsafe_code, reason = "`set_var` and `remove_var` are unsafe in edition 2024")]
fn write(name: &str, value: Option<&OsStr>) {
  // SAFETY: nextest runs every test in its own process, and a test sets the variables before it starts a thread that
  // reads the environment
  unsafe {
    match value {
      Some(value) => std::env::set_var(name, value),
      None => std::env::remove_var(name),
    }
  }
}
