//! How to run the bd of this binary: the command the outputs, the help texts and the skills name (`BdCommand`), and a
//! word quoted for the user's shell. `bd install` writes the symlink these read

use crate::context::Environment;
use crate::error::CliError;
use domain_types::error::ErrorCode;
use std::path::{Path, PathBuf};
use sync_core::resolve_copies::display_path;

/// Where the `bd` symlink goes, under the home directory
#[cfg(unix)]
pub const LINK_DIR: &str = ".local/bin";

/// How to run the bd of this binary, without writing anything: the `bd` symlink in `~/.local/bin` when it points at
/// this binary, else the binary itself, with the `cli` subcommand on macOS and Linux, where the bare app binary starts
/// the app (on Windows `bd.exe` next to the app, the console binary)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BdCommand {
  pub program: PathBuf,
  /// `cli` when `program` is the app binary
  pub subcommand: Option<&'static str>,
}

impl BdCommand {
  /// `program [cli]` with the path as it is
  pub fn display(&self) -> String {
    let program = display(&self.program);
    match self.subcommand {
      Some(subcommand) => format!("{program} {subcommand}"),
      None => program,
    }
  }

  /// The command for the user's shell, run as written: on macOS and Linux a POSIX shell, the path single-quoted when it
  /// needs it; on Windows PowerShell, `& '<path>'` (a quoted path needs the call operator there)
  pub fn for_shell(&self) -> String {
    let program = shell_word(&display(&self.program));
    let program = if cfg!(windows) { format!("& {program}") } else { program };
    match self.subcommand {
      Some(subcommand) => format!("{program} {subcommand}"),
      None => program,
    }
  }

  /// `for_shell` with `-C <repository>`, the repository quoted the same way
  pub fn for_shell_in(&self, repository: &str) -> String {
    format!("{} -C {}", self.for_shell(), shell_word(repository))
  }
}

/// A word for the user's shell: as it is when plain, else single-quoted, where nothing expands: for a POSIX shell a
/// quote is `'\''`, for PowerShell `''`
pub fn shell_word(word: &str) -> String {
  let plain = !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '@' | '+'));
  if plain {
    word.to_string()
  } else if cfg!(windows) {
    format!("'{}'", word.replace('\'', "''"))
  } else {
    format!("'{}'", word.replace('\'', r"'\''"))
  }
}

/// The bd of this binary, see `BdCommand`
pub fn bd_command(environment: &Environment) -> Result<BdCommand, CliError> {
  let target = app_binary(environment)?;
  #[cfg(unix)]
  if let Some(home) = &environment.home {
    let link = home.join(LINK_DIR).join("bd");
    if links_to(&link, &target) {
      return Ok(BdCommand { program: link, subcommand: None });
    }
  }
  // the app binary needs `cli`; a binary named bd (a cargo build, `bd.exe` on Windows) is the CLI itself
  let is_bd = target.file_stem().is_some_and(|stem| stem == "bd");
  let subcommand = (!cfg!(windows) && !is_bd).then_some("cli");
  Ok(BdCommand { program: target, subcommand })
}

/// The command `bd help skill` names, quoted for a shell: `bd_command`, or `bd` when the binary is unknown
pub fn skill_command(environment: &Environment) -> String {
  bd_command(environment).map_or_else(|_| "bd".to_string(), |command| command.for_shell())
}

/// `path` without the `\\?\` prefix that `canonicalize` gives on Windows, when it is shorter than 260 characters: every
/// command bd prints names the program, and many tools refuse the verbatim form
pub fn without_verbatim(path: &Path) -> PathBuf {
  PathBuf::from(display_path(path))
}

/// The binary the command runs: on macOS and Linux the app binary (a `bd` symlink resolves to it), on Windows `bd.exe`
/// next to the app, the console binary
pub fn app_binary(environment: &Environment) -> Result<PathBuf, CliError> {
  let executable = environment
    .executable
    .as_deref()
    .ok_or_else(|| CliError::new(ErrorCode::InstallRefused, "Can't tell where the bd binary is"))?;
  let executable = without_verbatim(&std::fs::canonicalize(executable).map_err(|e| io_error(executable, &e))?);
  // an `APPIMAGE` that is gone is not the app's
  let appimage = environment
    .appimage
    .as_deref()
    .and_then(|path| std::fs::canonicalize(path).ok())
    .map(|path| without_verbatim(&path));
  let executable = app_binary_path(&executable, appimage.as_deref());
  if cfg!(windows) {
    let bd = executable.with_file_name("bd.exe");
    if !bd.is_file() {
      return Err(CliError::new(ErrorCode::InstallRefused, format!("No bd.exe next to {}", executable.display())));
    }
    return Ok(bd);
  }
  Ok(executable)
}

/// The app binary of the running `executable`: the `.AppImage` file when the app runs from one (`appimage`, the `APPIMAGE`
/// variable), not the binary in its mount under `/tmp/.mount_*`, which goes when the app quits
pub fn app_binary_path(executable: &Path, appimage: Option<&Path>) -> PathBuf {
  appimage.unwrap_or(executable).to_path_buf()
}

/// Whether `link` is a symlink that resolves to `target`
#[cfg(unix)]
pub fn links_to(link: &Path, target: &Path) -> bool {
  std::fs::symlink_metadata(link).is_ok_and(|metadata| metadata.file_type().is_symlink()) && std::fs::canonicalize(link).is_ok_and(|resolved| resolved == target)
}

pub fn display(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

pub fn io_error(path: &Path, error: &std::io::Error) -> CliError {
  CliError::new(ErrorCode::InstallRefused, format!("{}: {error}", path.display()))
}
