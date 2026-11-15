# A pick that lands nothing follows without a green check

## Why

`brd workspace` in a lease names `home.build_config_changed`, and the skill tells the holder to run
`brd -C <path> lease pick` at once when nothing is unlanded. In a repository with `pick_requires_check = true`, the
gate of the pick refused that pick before its check, as `unchecked_tree` (exit 4). The refusal told the holder to build
first, on the old build config, which is the result the hint exists to stop. Only `--allow-unchecked` let the lease
follow, and no text named it for this case.

## What Changes

- With `pick_requires_check`, a pick from a worktree whose tree has no green record no longer stops at the gate. The
  gate sets `PickParams::land_nothing`.
- Each check of the pick, before the lock and again under it, refuses a pick that would land a commit or that
  conflicts (`UncheckedTreeRefused`). The command words it as `unchecked_tree`, as before.
- A pick that lands nothing moves the worktree onto the tip with `--follow`, and `check` names the missing record.
- The skill, `brd help worktrees` and the description of `home.build_config_changed` say that a pick that lands
  nothing needs no green check.
- Capabilities `cli/pick`, `cli/lease` and `cli/workspace`.
