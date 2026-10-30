//! The durations that a process test shortens through the environment, and the hold of a follow. Each duration
//! variable holds milliseconds. Without a variable, the agent uses the value of the protocol

use host_link::proto::{HEARTBEAT_SECONDS, PACK_STALL_SECONDS, PACK_WAIT_SECONDS, UPGRADE_DRAIN_SECONDS, UPGRADE_WATCHDOG_SECONDS};
use std::path::PathBuf;
use std::time::Duration;

/// How long an interrupted command group has before SIGKILL: `BD_HOST_TEST_GRACE_MS`, else 120 s
pub(crate) fn grace() -> Duration {
  from_env("BD_HOST_TEST_GRACE_MS", Duration::from_secs(120))
}

/// The total deadline of a TLS handshake: `BD_HOST_TEST_HANDSHAKE_MS`, else 3 s
pub(crate) fn handshake() -> Duration {
  from_env("BD_HOST_TEST_HANDSHAKE_MS", Duration::from_secs(3))
}

/// How often a waiting request or a quiet follow sends a heartbeat: `BD_HOST_TEST_HEARTBEAT_MS`, else
/// [`HEARTBEAT_SECONDS`]
pub(crate) fn heartbeat() -> Duration {
  from_env("BD_HOST_TEST_HEARTBEAT_MS", Duration::from_secs(HEARTBEAT_SECONDS))
}

/// How long a run waits for the pack of its commit: `BD_HOST_TEST_PACK_WAIT_MS`, else [`PACK_WAIT_SECONDS`]
pub(crate) fn pack_wait() -> Duration {
  from_env("BD_HOST_TEST_PACK_WAIT_MS", Duration::from_secs(PACK_WAIT_SECONDS))
}

/// How long a run waits for the next chunk of its pack: `BD_HOST_TEST_PACK_STALL_MS`, else [`PACK_STALL_SECONDS`]
pub(crate) fn pack_stall() -> Duration {
  from_env("BD_HOST_TEST_PACK_STALL_MS", Duration::from_secs(PACK_STALL_SECONDS))
}

/// The shortest time between two notes of the chunks of a pack, which keep the wait of its run alive:
/// `BD_HOST_TEST_PACK_PROGRESS_MS`, else 1 s
pub(crate) fn pack_progress_every() -> Duration {
  from_env("BD_HOST_TEST_PACK_PROGRESS_MS", Duration::from_secs(1))
}

/// How long a run waits for its pack before it says so: `BD_HOST_TEST_PACK_SAY_MS`, else 1 s
pub(crate) fn pack_wait_say() -> Duration {
  from_env("BD_HOST_TEST_PACK_SAY_MS", Duration::from_secs(1))
}

/// How long the processes that a command left in its group have after SIGTERM, before SIGKILL:
/// `BD_HOST_TEST_GROUP_TERM_MS`, else 5 s
pub(crate) fn group_term_wait() -> Duration {
  from_env("BD_HOST_TEST_GROUP_TERM_MS", Duration::from_secs(5))
}

/// How long a group whose command ended can take to empty and still count as empty: `BD_HOST_TEST_GROUP_SETTLE_MS`,
/// else 500 ms
pub(crate) fn group_settle() -> Duration {
  from_env("BD_HOST_TEST_GROUP_SETTLE_MS", Duration::from_millis(500))
}

/// The pause of a wait for a change of the run files without a kqueue: `BD_HOST_TEST_WATCH_POLL_MS`, else 200 ms
pub(crate) fn watch_poll() -> Duration {
  from_env("BD_HOST_TEST_WATCH_POLL_MS", Duration::from_millis(200))
}

/// The longest wait of a follow for a change of the run files. A run that lost its supervisor changes no file, so this
/// bounds the time until the follow ends it: `BD_HOST_TEST_FOLLOW_RECHECK_MS`, else 1 s
pub(crate) fn follow_recheck() -> Duration {
  from_env("BD_HOST_TEST_FOLLOW_RECHECK_MS", Duration::from_secs(1))
}

/// How long an upgraded `serve` waits for its other requests before it exits: `BD_HOST_TEST_UPGRADE_DRAIN_MS`, else
/// [`UPGRADE_DRAIN_SECONDS`]
pub(crate) fn upgrade_drain() -> Duration {
  from_env("BD_HOST_TEST_UPGRADE_DRAIN_MS", Duration::from_secs(UPGRADE_DRAIN_SECONDS))
}

/// How long the watchdog of an upgrade waits for the new binary to listen, after the drain:
/// `BD_HOST_TEST_UPGRADE_WAIT_MS`, else [`UPGRADE_WATCHDOG_SECONDS`]
pub(crate) fn upgrade_wait() -> Duration {
  from_env("BD_HOST_TEST_UPGRADE_WAIT_MS", Duration::from_secs(UPGRADE_WATCHDOG_SECONDS))
}

/// A file that holds a follow before each read of the run files while it exists: `BD_HOST_TEST_FOLLOW_HOLD`. A test
/// holds the follow thread with it to check that the heartbeats go on
pub(crate) fn follow_hold() -> Option<PathBuf> {
  std::env::var_os("BD_HOST_TEST_FOLLOW_HOLD").map(PathBuf::from)
}

/// How often the keeper of the host slots wakes without a start: `BD_HOST_TEST_KEEPER_MS`, else 60 s
pub(crate) fn keeper_tick() -> Duration {
  from_env("BD_HOST_TEST_KEEPER_MS", Duration::from_secs(60))
}

fn from_env(name: &str, default: Duration) -> Duration {
  std::env::var(name).ok().and_then(|value| value.parse().ok()).map_or(default, Duration::from_millis)
}
