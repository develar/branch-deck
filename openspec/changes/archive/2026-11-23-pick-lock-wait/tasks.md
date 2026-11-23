# Tasks

- [x] `brd pick` and `brd lease pick` wait with `LockWait::WhileHeld` and a waiter of `brd` (`CommandSteps`) that names
      the holder at once and every 10 s and ends the wait past `--lock-wait`
- [x] The update of a lease after its conflict waits the same way, within the same bound
- [x] A wait past the bound is `locked`, retryable, naming the bound
- [x] A witness that missed after a wait for another holder checks under that lock at once (`land`)
- [x] Tests: a wait for a live holder that lands after it, the bound, a contended wait that checks under the lock
- [x] The doc of `lock_repository_while_held`, "Pick" and "Lock spans" of docs/design/git-operations.md,
      docs/contributing/agent-cli.md and `skill_text()`
