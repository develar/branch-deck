//! When the Landed island hides what the user saw (openspec/specs/repository/landed-commits/spec.md, "After the update:
//! The island hides after the user saw it"): a window with the focus that shows the island up to a `landedAt` is a
//! sighting of it from then (`seen`: one of a newer `landedAt` starts again), and the first successful sync of the
//! session at least `MIN_SEEN` after a sighting by a window that has the focus as it ends hides the branches that
//! landed up to the newest `landedAt` so seen (`hidden_by_sync`), so the sync that follows an update keeps the island.
//! The session keeps the sightings of its windows and writes the hide (`repository_model::runtime`, `landedHiddenUntil`
//! of the lease it opened under); a sighting of no more than the hide is spent by it (`stays_seen`).

use std::time::Duration;

/// How long the island stays at least after the user saw it: the sync that follows an update keeps it
pub const MIN_SEEN: Duration = Duration::from_secs(10);

/// A window showed the island up to `landed_at` (the newest `landedAt` of its branches, Unix seconds) while it had the
/// focus, from `at` on (the session clock)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seen {
  pub at: Duration,
  pub landed_at: u64,
}

/// The sighting of a window that shows the island up to `landed_at` at `now` with the focus, after its sighting
/// `before`: the same `landedAt` is seen since it was first, a newer one (more landed) is seen from now
#[must_use]
pub const fn seen(before: Option<Seen>, landed_at: u64, now: Duration) -> Seen {
  match before {
    Some(before) if before.landed_at == landed_at => before,
    _ => Seen { at: now, landed_at },
  }
}

/// What a successful sync of the session that ends at `now` hides: up to the newest `landedAt` of the sightings
/// `seen`, each with whether its window has the focus now, that is at least `MIN_SEEN` old; nothing when there is none
#[must_use]
pub fn hidden_by_sync(seen: impl IntoIterator<Item = (Seen, bool)>, now: Duration) -> Option<u64> {
  seen
    .into_iter()
    .filter(|(seen, focused)| *focused && now.saturating_sub(seen.at) >= MIN_SEEN)
    .map(|(seen, _)| seen.landed_at)
    .max()
}

/// Whether a sighting stays after a hide up to `hidden`: one of what landed later does, the rest the hide spent
#[must_use]
pub const fn stays_seen(seen: Seen, hidden: u64) -> bool {
  seen.landed_at > hidden
}
