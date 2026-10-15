//! A fake CLI (`gh`, `claude`) for tests: a shell script in a temp dir

use std::path::{Path, PathBuf};

/// Writes `#!/bin/sh` + `body` to `dir/name` and makes it executable
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
