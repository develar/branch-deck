# Proposal

## Why

bt of the IntelliJ monorepo no longer passes `--head` to `brd-host run` (idea-1 e1c0946d). `bt --host` ships the
working tree, and its check record keeps the tree that ran. The spec still names `--head` in the command line of bt.

## What Changes

- The command line of bt in "A machine caller names its run options in full" has no `--head`: bt runs the working
  tree, HEAD plus the local changes as one commit.
- The skill of brd-host no longer says that `bt --host` refuses a dirty tree.

Capabilities: `cli/host`.
