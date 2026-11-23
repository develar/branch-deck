# Proposal

## Why

`brd -C <lease> update --continue` in a lease answered `next: ["brd -C <lease> -C <lease> lease pick"]`: the brd of the
command (`RepoContext::bd`) names the repository with `-C` already, and the next command added it again. Without a
known binary that brd was a bare `brd`, so the tests saw one `-C` and the doubling only showed in the app's brd.

## What Changes

- A command that brd names for a worktree gets its `-C` in one place, `bd_core::bd_command::in_worktree`;
  `RepoContext::bd` is that for the context's repository, also when the binary is unknown (`brd -C <repo>`), and a
  `next` of the repository appends only its arguments to it: `lease pick` and `fold` after `update --continue`.

Capabilities: `cli/update`.
