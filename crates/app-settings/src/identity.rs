//! The identity of the app on this machine: the bundle identifier its binary was built with and the data directory it
//! names (openspec/specs/repository/ide-embedding/spec.md, "Bridge: The data of the bridge is the data of the desktop
//! app"). The one source of the data directory of the app's backend, of the bridge `branch-deck ide` and of `brd`, so
//! the socket key of the backend (a hash of the data directory and the executable) is the same on both sides: the
//! identifier of `src-tauri/tauri.conf.json` under the overlay of `tauri build --config` (`identity_source`), compiled in
//! by the build script of this crate. Tauri's `app_data_dir` is the same directory (`dirs::data_dir()` and the identifier
//! of the same configuration).

use std::path::PathBuf;

/// The bundle identifier the binary was built with (`org.develar.branch-deck`, a check copy's own)
pub const APP_IDENTIFIER: &str = env!("BRANCH_DECK_APP_IDENTIFIER");

/// The data directory of the app (`~/Library/Application Support/<identifier>` on macOS): its settings, its caches, its
/// journals and the key of its backend
pub fn app_data_dir() -> Option<PathBuf> {
  dirs::data_dir().map(|dir| dir.join(APP_IDENTIFIER))
}
