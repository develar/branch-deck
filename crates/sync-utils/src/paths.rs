//! Paths as the user's shell takes them: `bd` prints the copies of a resolve session, the slots of the lease pool and
//! the files of a pick, and Windows wants backslashes and, for a long path, the verbatim form.

use std::path::Path;

/// `path` as a shell and its tools take it: as it is on macOS and Linux; on Windows with backslashes, and in the `\\?\`
/// form from 260 characters on, which Windows PowerShell 5.1 opens without long paths enabled
pub fn display_path(path: &Path) -> String {
  let text = path.to_string_lossy();
  if cfg!(windows) { windows_display(&text) } else { text.into_owned() }
}

/// The Windows form of a path that git may write with `/`: backslashes, and `\\?\` (`\\?\UNC\` for a share) from 260
/// characters on. A shorter path loses the `\\?\` prefix that `canonicalize` gives.
pub fn windows_display(text: &str) -> String {
  let path = text.replace('/', "\\");
  let plain = match path.strip_prefix(r"\\?\UNC\") {
    Some(share) => format!(r"\\{share}"),
    None => path.strip_prefix(r"\\?\").filter(|rest| rest.as_bytes().get(1) == Some(&b':')).unwrap_or(&path).to_string(),
  };
  if plain.chars().count() < 260 {
    return plain;
  }
  match plain.strip_prefix(r"\\") {
    Some(share) if !share.starts_with(r"?\") => format!(r"\\?\UNC\{share}"),
    Some(_) => plain,
    None => format!(r"\\?\{plain}"),
  }
}
