# Proposal

## Why

The message of `approvals_missing` told an agent to run a bare `brd safe-push auth --create-pull-request`, while its
`next_steps` said `brd -C <repo> safe-push auth --create-pull-request`: the text and the step disagreed, and the bare
command copied from the text runs in whatever directory the agent is in. Other answers named bare commands the same
way: the target conflict of a Safe Push, the push that dismisses the approvals, a remote stack branch with commits the
stack lacks, the trial build of a branch alone, a robot of another branch, `no_pull_request` of `brd request-review`,
an unsupported conflict of `brd update`, and a lease cut from a detached HEAD.

## What Changes

- A command that a message names for the repository is built as its step is: `RepoContext::bd` and the arguments, a
  command of another worktree `in_worktree`; where the refusal has a step, the message names that very command.
- Messages that name a command only as a pointer (`brd sync` says what holds it), the help of clap and the generic
  examples of `skill_text()` stay as they are.

Capabilities: `cli/agent-cli`.
