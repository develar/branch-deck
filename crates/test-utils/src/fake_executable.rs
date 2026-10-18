//! A fake CLI (`gh`, `claude`, `teamcity`, the `ssh` of git) for tests: a shell script in a temp dir. Windows runs no
//! `#!` script, so there the script is `<name>.sh` beside `<name>.exe`, a copy of the shim (`fake_cli_shim.rs`) that
//! runs it with the sh of Git for Windows. Either way a test runs the fake by the path it gets, and a lookup of
//! `<name>` on `PATH` (`<name>.exe` on Windows) finds it.

use crate::git_test_utils::git_command;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The second line of a Windows script, after the sh of its `#!` line: the arguments as `"$@"`, from the environment
/// that the shim passes them in (`FAKE_CLI_ARGC`, `FAKE_CLI_ARG_<n>`; see `fake_cli_shim.rs` for why)
const WINDOWS_ARGUMENTS: &str =
  r#"i=0; set --; while [ "$i" -lt "${FAKE_CLI_ARGC:-0}" ]; do i=$((i + 1)); eval "set -- \"\$@\" \"\$FAKE_CLI_ARG_$i\""; unset "FAKE_CLI_ARG_$i"; done; unset i FAKE_CLI_ARGC"#;

/// Writes `body` as the sh script of the fake `name` in `dir` and returns the path to run: on Unix `dir/name`, the
/// script itself (`#!/bin/sh`, executable); on Windows `dir/name.exe`, which runs `dir/name.sh` with its arguments,
/// stdin, stdout, stderr and environment, exits with its code, and ends with it the processes it started. A second
/// fake of the same name replaces the first. A git config value names it by `shell_path`.
pub fn fake_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
  if cfg!(windows) {
    return windows_fake_executable(dir, name, body);
  }
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

fn windows_fake_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
  let script = dir.join(format!("{name}.sh"));
  std::fs::write(&script, format!("#!{}\n{WINDOWS_ARGUMENTS}\n{body}\n", git_sh().display())).unwrap();
  let exe = dir.join(format!("{name}.exe"));
  // every fake has the same shim, and Windows may still hold the image of a run of it that just ended: a fake written
  // again changes only its script
  if !exe.is_file() {
    std::fs::copy(fake_cli_shim(), &exe).unwrap_or_else(|e| panic!("cannot copy the fake CLI shim to {}: {e}", exe.display()));
  }
  exe
}

/// The shim that `build.rs` builds for a Windows target
fn fake_cli_shim() -> &'static str {
  option_env!("FAKE_CLI_SHIM").unwrap_or_else(|| panic!("the fake CLI shim was not built for this target: see the warning of the build script of test-utils"))
}

/// The sh of Git for Windows: `usr\bin\sh.exe` of the installation, three levels above `git --exec-path`
/// (`C:/Program Files/Git/mingw64/libexec/git-core`)
fn git_sh() -> &'static Path {
  static SH: OnceLock<PathBuf> = OnceLock::new();
  SH.get_or_init(|| {
    let output = git_command().arg("--exec-path").output().expect("git --exec-path");
    assert!(output.status.success(), "git --exec-path: {}", String::from_utf8_lossy(&output.stderr));
    let exec_path = String::from_utf8_lossy(&output.stdout);
    let exec_path = exec_path.trim();
    let root: PathBuf = Path::new(exec_path)
      .ancestors()
      .nth(3)
      .unwrap_or_else(|| panic!("git's exec path {exec_path} is not in a Git for Windows installation"))
      .components()
      .collect();
    let sh = root.join("usr").join("bin").join("sh.exe");
    assert!(sh.is_file(), "no sh of Git for Windows at {}, three levels above git's exec path {exec_path}", sh.display());
    sh
  })
}
