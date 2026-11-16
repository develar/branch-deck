# A restart runs on what its admission holds

## Why

The review of 46c8ecc2 (bd-flakes-review): `execute_restart` admitted a restart with the plan of `prepare_restart`,
which is an error where the session refuses the restart (`.ok().flatten()`), so a restart asked while Patronus had not
told why the Safe Push failed held only its robot. The refusal can lift while it waits for a sync, and then it pushed and
restarted without holding what it lands or the ref of its push: a Safe Push of its branch ran beside it, two robots of
the branch.

## What Changes

- The admission reads the plan apart from the refusal (`read_restart`), so it holds the robot, what it lands and the
  ref of its push also while the restart is refused.
- The plan decided once admitted runs only on what the admission holds (`refuse_unheld`): one that holds more (no plan
  as it was asked, a branch its stack took, a push it needs now) is refused `plan_changed` before any push or restart.
- Capability `branch/patronus`: a requirement that a restart runs on what its admission holds.
