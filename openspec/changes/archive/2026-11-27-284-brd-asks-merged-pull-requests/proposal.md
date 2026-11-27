# Proposal

## Why

The app pairs the squash of a pull request by its merge commit from the listing of its session (branch-deck#284), but
`brd update` has no session: without the merged pull requests it still stops on the round of a squash with an edit of
the review or under a noreply address.

## What Changes

- `brd update` and `brd update --dry-run` ask GitHub once, only when the check conflicts with local commits that nothing
  else paired, before the repository lock: one search of the user's pull requests merged since a day before the oldest
  author time of those commits (`remote_facts::merged_pull_requests`), through the token chain of `brd`. A clean update
  makes no request; signed out, offline or a failure pair as before.
- The app reads the merged pull requests with its listing (the 50 most recently updated), `brd` with its search since the
  commits: the two may find other pull requests.
- An ask from inside a runtime answers none (`Handle::try_current`): the block on the runtime of the command would panic.

Capabilities: `cli/update`.
