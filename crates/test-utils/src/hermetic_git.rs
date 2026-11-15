//! The git config of a test that no installed program reaches: no system config, an empty global one of its own, and
//! none of the variables of Git LFS (`GIT_LFS_*`). The machine's `git lfs install` (`filter.lfs.required=true` and the
//! real smudge, which a Mac with Homebrew's git-lfs, the Studio and a CI runner have) and a `GIT_LFS_SKIP_SMUDGE` of
//! the shell would otherwise decide what a test of Git LFS sees: every such test goes through `HermeticGit`

use crate::env_vars::EnvVars;
use std::path::Path;

/// `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL` of an empty file of its own and no `GIT_LFS_*` for every git of the test
/// process, the fixtures' and brd's, until it drops; started before the first repository of the test
pub struct HermeticGit {
  _vars: EnvVars,
  global: tempfile::NamedTempFile,
}

impl HermeticGit {
  /// # Panics
  /// When the global config file cannot be created
  #[must_use]
  pub fn new() -> Self {
    let global = tempfile::NamedTempFile::new().expect("a global git config of the test");
    let lfs: Vec<String> = std::env::vars_os()
      .filter_map(|(name, _)| name.into_string().ok())
      .filter(|name| name.starts_with("GIT_LFS_"))
      .collect();
    let vars = EnvVars::set_and_remove(&[("GIT_CONFIG_NOSYSTEM", "1".as_ref()), ("GIT_CONFIG_GLOBAL", global.path().as_os_str())], &lfs);
    Self { _vars: vars, global }
  }

  /// The global config file of the test, empty unless it writes one
  #[must_use]
  pub fn global_config(&self) -> &Path {
    self.global.path()
  }
}

impl Default for HermeticGit {
  fn default() -> Self {
    Self::new()
  }
}
