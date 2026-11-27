# Proposal

## Why

A live check on GitHub found two answers of `brd` that hide what was done to a review after landing: the close of an
open one deletes both of its branches but names none (only the close of one closed on GitHub had `deleted`), and a
`--reviewer` recorded in a draft that asks nobody yet answered `"reviewers":[]`, as though nobody were recorded.

## What Changes

- `brd close-review` of an open shown review names the branches it deleted in `deleted`, those the remote had
  (`operations::shown::ShownRun::deleted`, what `sync_core::review_head::delete_review` returns); `--dry-run` names its
  head and the base the remote has. One closed on GitHub names them from the same run.
- `brd request-review <branch> --reviewer <login>` of a draft whose branch nothing landed yet names in `reviewers` whom
  its body records, whom its open asks, as the mark already does; `requested` stays empty.

Capabilities: `cli/agent-cli`.
