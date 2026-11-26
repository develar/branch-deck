# Proposal

## Why

The agent CLI answers as the app does (`brd push`, `brd safe-push`): a branch only rebuilt on a newer baseline since
its push (Update Project) was `up_to_date`, and `brd safe-push` started it without a push, so the builds of its pull
request kept testing the older master. The change `rebased-push` makes the app push it on request and before a start;
this one gives `brd` the same.

## What Changes

- `brd push` lists such a branch `up_to_date` as before, with `older_base: true`; `brd status` and `brd sync` say
  `older_base` in `remote`.
- `brd push --rebased` pushes it, the branches it is built on that are too first, leased, without `--force`.
- `brd safe-push` and its `--create-pull-request` push it first, as the app's start does (`push_needed`).

Capabilities: `cli/agent-cli`.
