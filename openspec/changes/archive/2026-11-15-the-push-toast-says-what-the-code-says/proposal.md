# The toast of a push says what the code says, and its stay has tests

## Why

The review of 6ccae40f: the requirement of the toast of a push said "Updating the project…", while the page says
"Rebuilding on the new master…" (`describeFollowingUpdate`); the notice of a target that moved stays while the update
runs too, which its requirement did not say; and neither stay had an E2E test of its own, only the merge's.

## What Changes

- The push requirement words the running update as the page does, and says that a start that fails after the push ends
  the update in its toast; the moved-target requirement says the notice stays while the update runs.
- An E2E test of each stay: the push and the moved target with the sync of the update held past the time of a toast.
- Capability `branch/landing-routes`.
