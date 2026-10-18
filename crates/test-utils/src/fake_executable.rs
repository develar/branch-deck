//! A fake CLI (`gh`, `claude`, the `ssh` of git) for tests: a shell script in a temp dir

use std::path::{Path, PathBuf};

/// Writes `#!/bin/sh` + `body` to `dir/name` and makes it executable. A git config value names it by `shell_path`
pub fn fake_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
  let path = dir.join(name);
  std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
  }
  path
}

/// `path` as one word that `sh` reads back as the same path: for a git config value that names a program
/// (`core.sshCommand` and a `credential.helper` by absolute path run through `sh -c`; `core.askPass` too, for one
/// rule) and for a path in the body of a fake that git runs. On Windows its separators are `/`: Git for Windows' sh
/// reads `\` as an escape, so `C:\Users\me\ssh` would run `C:Usersmessh`, while `C:/Users/me/ssh` stays an absolute
/// path for git and for sh. No quotes: a quoted `credential.helper` is no absolute path to git, so a path that sh
/// would split, unquote or expand panics instead.
pub fn shell_path(path: &Path) -> String {
  let path = path.to_str().expect("a test path is UTF-8");
  let word = if cfg!(windows) { path.replace('\\', "/") } else { path.to_string() };
  assert!(
    !word.contains(|c: char| c.is_whitespace() || "\\\"'`$|&;<>()*?[{".contains(c)),
    "sh would not read {word:?} as one path: point TMPDIR (TEMP on Windows) to a directory without such characters"
  );
  word
}
