//! Ctrl+C of the person at the terminal, for a run (spec/cli/host.spec.md, "run"). The handler only counts the
//! presses; the client reads the count and acts on its own threads. The first press before the start stops each local
//! step, and nothing runs on the host. The first press after the start sends `Cancel`, stops the pack, and follows the
//! output to its end. The second press stops the follow at once, and the run refuses with `interrupted`

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

static PRESSES: AtomicU32 = AtomicU32::new(0);

/// Installs the handler once per process. Answers why it cannot; Ctrl+C then ends the process as without it
pub fn install() -> Result<(), String> {
  static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();
  INSTALLED
    .get_or_init(|| {
      ctrlc::set_handler(|| {
        PRESSES.fetch_add(1, Ordering::SeqCst);
      })
      .map_err(|error| format!("cannot handle Ctrl+C: {error}"))
    })
    .clone()
}

/// How many times the person pressed Ctrl+C
pub fn presses() -> u32 {
  PRESSES.load(Ordering::SeqCst)
}
