# Proposal

## Why

An agent pushes and lands through `brd`, not the page: the question the app asks before the push or the start of a
copy that holds a generated file the words merged (`232-regenerate-asks-first`,
[develar/branch-deck#232](https://github.com/develar/branch-deck/issues/232)) must be a refusal there, which only the
user lifts.

## What Changes

- `ErrorCode::GeneratedMergedByWords` (`generated_merged_by_words`, exit 4) of `brd push` (before anything is pushed,
  `--force` too; `--dry-run` prints the push), `brd safe-push` (also in `refusals` of `--dry-run`) and `brd land`.
- One message for the three (`crates/bd-remote/src/regenerate_refusal.rs`): the file, why it is generated
  (`GeneratedBy::describe`), the copy that merged it, the commit it lacks and the fix; its next step the
  `brd restack <root> <onto>` of the fix, which needs approval. The flag is named only in the message.
- `--allow-generated-merged-by-words` of the three lifts it, once the user agreed.

Capabilities: `cli/agent-cli`.
