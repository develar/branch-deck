//! Ctrl+C of the person at the terminal, for a run (openspec/specs/cli/host/spec.md, "run"), and SIGTERM and SIGHUP, which
//! count as a press: a tool that stops the client after its timeout, or a closed terminal, cancels the run on the host
//! rather than leave it to wait for a slot or the heavy turn and run for nobody. SIGKILL cannot be caught: the run goes
//! on. The handler only counts the presses; the client reads the count and acts on its own threads. The first press before the start stops each local
//! step, and nothing runs on the host. The first press after the start sends `Cancel`, stops the pack, and follows the
//! output to its end. The second press stops the follow at once, and the run refuses with `interrupted`

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

static PRESSES: AtomicU32 = AtomicU32::new(0);

/// Installs the handler of SIGINT, SIGTERM and SIGHUP once per process. Answers why it cannot; a signal then ends the
/// process as without it
pub fn install() -> Result<(), String> {
  static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();
  INSTALLED
    .get_or_init(|| {
      ctrlc::set_handler(|| {
        PRESSES.fetch_add(1, Ordering::SeqCst);
      })
      .map_err(|error| format!("cannot handle Ctrl+C, SIGTERM and SIGHUP: {error}"))
    })
    .clone()
}

/// How many times the person pressed Ctrl+C
pub fn presses() -> u32 {
  PRESSES.load(Ordering::SeqCst)
}

/// How often `wait_for_press` looks at the count
const PRESS_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// Returns at the first press of `presses`: a command whose request goes on on the host without this process ends at
/// once then, as without a handler
pub fn wait_for_press(presses: &dyn Fn() -> u32) {
  while presses() == 0 {
    std::thread::sleep(PRESS_POLL);
  }
}
