# Proposal

## Why

On 2026-11-23 `brd lease pick` in idea-1 answered `locked` after about 180 s of work: it waited at most 10 s for
`branch-deck.lock` (`DEFAULT_LOCK_TIMEOUT`), while the holder, another pick, held it for 1 to 3.5 minutes. Agents wrote
22 retry loops by hand (about 2900 s) and wrapped `brd` in `timeout N` 178 times. The app already waits while the holder
lives (`LockWait::WhileHeld`); `brd` did not. And a pick whose witness missed after such a wait went round again
without the lock, where the next pick most likely moved the target again.

## What Changes

- `brd pick`, `brd lease pick` and the update of a lease after its conflict wait for the repository lock while its
  holder lives, naming the holder on stderr at once and every 10 s, up to `--lock-wait <secs>` in all (540 by default,
  below the 600 s of a Bash call of an agent). Past it the answer is `locked` (retryable, exit 3), naming the bound.
- A pick whose witness missed after a wait for another holder checks again under that lock at once.
- The agent skill says: no retry loop, no `timeout` wrapper; the pick waits and prints its steps.

Capabilities: `cli/pick`.
