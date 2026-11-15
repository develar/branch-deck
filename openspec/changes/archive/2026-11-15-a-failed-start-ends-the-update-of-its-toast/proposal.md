# A start that fails after its push or merge ends the update of its toast

## Why

The toast of a push or a merge into the target stays while the update after it runs (6ccae40f), "Rebuilding on the
new master…", until the update tells its end on the channel or `execute` answers. An error `execute` answered after the
end of the push (`tellStartRefusal`) and a call that failed (an IPC failure, which rejected the start) never ended that
update: the toast stayed until the user closed it, and an error answered was told as a second toast of a refusal.

## What Changes

- `followPush` has `failed`: after the end of the push an error of `execute`, or the `Error` of a failed call, ends a
  running update in the toast of the push ("Cannot update the project: …", Update Project), which goes in its time; a
  failed call is logged. Before the end of the push nothing changes: a refusal of the start, or the rejection.
- `routeAnswerAfter` of the E2E helpers runs a command on the server and replaces its answer after the channel
  messages a test keeps.
- Capability `branch/landing-routes`.
