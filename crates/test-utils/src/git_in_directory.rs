//! A process that works in a directory, as a shell or a build of an agent does: a git whose current directory it is,
//! alive until the test lets it go, in a process group of its own (of another session than the test's, which a scan of
//! the processes leaves out). On Windows it keeps the directory from a removal and a rename (a sharing error), as a
//! real holder would; the scan of macOS and Linux finds it there by its working directory.

use crate::git_test_utils::git_command;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};

/// A git with its current directory in a directory, until dropped: `hash-object --stdin-paths`, which hashes each path
/// it reads until its stdin closes
pub struct GitInDirectory {
  child: Child,
  stdin: Option<ChildStdin>,
}

impl GitInDirectory {
  /// Starts the git in `dir`, and returns once it runs there: it answered the hash of a file. A process that Windows
  /// has only created opens its current directory as it starts, and a removal before that does not see it
  pub fn start(dir: &Path) -> Self {
    let probe = tempfile::NamedTempFile::new().expect("a file for the git to hash");
    let mut command = git_command();
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
      .args(["hash-object", "--stdin-paths"])
      .env("GIT_FLUSH", "1")
      .current_dir(dir)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .expect("a git in the directory");
    let mut stdin = child.stdin.take().expect("the stdin of the git");
    writeln!(stdin, "{}", probe.path().display()).expect("a path for the git");
    let mut hash = String::new();
    std::io::BufReader::new(child.stdout.take().expect("the stdout of the git"))
      .read_line(&mut hash)
      .expect("the hash of the git");
    assert!(!hash.trim().is_empty(), "the git in {} answered no hash", dir.display());
    Self { child, stdin: Some(stdin) }
  }

  pub fn pid(&self) -> u32 {
    self.child.id()
  }
}

impl Drop for GitInDirectory {
  /// Its stdin closes, and the git exits: waited for, so no test leaves it running
  fn drop(&mut self) {
    drop(self.stdin.take());
    let _ = self.child.wait();
  }
}
