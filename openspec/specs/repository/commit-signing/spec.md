---
name: Signed Commits
description: Requirements for signing commits Branch Deck writes to branches when the repository has commit.gpgsign enabled.
style: plain-1
targets:
  - crates/git-ops/src/commit_signing.rs
  - crates/git-ops/src/commit_writer.rs
  - crates/git-ops/src/commit_chain.rs
  - crates/git-ops/src/replay.rs
  - crates/git-ops/src/notes.rs
  - crates/git-ops/src/copy_commit.rs
  - crates/git-ops/src/rewrite_plan.rs
  - crates/git-ops/src/amend_operations.rs
  - crates/git-ops/src/move_hunk.rs
  - crates/git-ops/src/reword_commits.rs
  - crates/git-ops/src/update_branch.rs
  - crates/git-ops/src/update_resolve.rs
  - crates/git-ops/src/word_replay.rs
  - crates/update-core/src/pick.rs
  - crates/sync-core/src/config_snapshot.rs
  - crates/sync-core/src/branch_processor.rs
  - crates/sync-core/src/stack_branch.rs
  - crates/domain-types/src/error.rs
  - crates/app-api/src/api_error.rs
  - crates/bd-core/src/error.rs
---

# Signed Commits

Status: Draft
Date: 2026-11-06

## Summary

When `commit.gpgsign` is true, Branch Deck signs every new commit it writes onto a branch with Git's configured key.
Internal store and notes commits remain unsigned. A failure to sign stops the operation before branch refs move and
returns `signing_failed` with Git's error.

## Goals

- Leave checked-out branches, picks, updates and synced virtual branches with commits Git can verify.
- Keep commit metadata unchanged apart from the signature header.
- Never move a branch ref to an unsigned rewritten commit when signing is required.

## Non-goals

- A Branch Deck-specific signing setting or preservation of the old commit signature.
- Signing store commits, notes commits, worktree snapshots, carried reviews or trial replays.
- Rewriting a fast-forward pick that keeps its original commits.

## Requirements

### Signing

- Read `commit.gpgsign` from the repository. When it is false or unset, existing commit ids and behavior stay unchanged.
- When it is true, every commit written onto a branch has a signature that `git verify-commit` accepts with the configured
  `gpg.format`, `user.signingkey` and signing program. The tree, parents, author, committer, dates, encoding and message
  remain as before the rewrite.
  [@test] crates/git-ops/src/commit_writer_test.rs (`signing_configuration_and_unsigned_writer_ids_are_unchanged`, `signing_writes_verifiable_user_commits_but_leaves_store_and_notes_commits_unsigned`)
  [@test] crates/git-ops/src/commit_chain_test.rs (`a_signing_chain_writes_verifiable_signatures_for_every_commit`)
- Store commits and notes commits written in the same `fast-import` stream stay unsigned.
  [@test] crates/git-ops/src/commit_writer_test.rs (`signing_writes_verifiable_user_commits_but_leaves_store_and_notes_commits_unsigned`)
- A missing key, locked signer or missing signing program fails with `signing_failed` and Git's words. No branch or store
  ref moves, and temporary refs are cleaned up.
  [@test] crates/git-ops/src/commit_writer_test.rs (`a_fast_import_signing_failure_is_typed_and_does_not_move_the_branch`)

### Branch operations

- History rewrites, amend, move-hunk, unapply/reapply, pick, Update Project and its resolve sessions sign new commits
  before their ref transaction. A pick that is a fast-forward keeps the original commits.
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_signs_the_replacement_and_replayed_descendants`)
  [@test] crates/git-ops/src/pick_test.rs (`a_fast_forward_pick_keeps_the_original_commits_with_signing_enabled`)
  [@test] crates/git-ops/src/move_hunk_test.rs (`moving_a_hunk_signs_every_rewritten_commit`)
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_words_resolved_finish_is_signed_before_its_ref_moves`)
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_fold_signs_replacements_and_replayed_commits`)
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`dropping_a_commit_signs_the_replayed_survivors`)
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_reword_signs_every_rewritten_commit`)
  [@test] crates/history-rewrite/src/squash_test.rs (`a_squash_and_its_replayed_descendants_are_signed`)
  [@test] crates/sync-core/src/reapply_branch_test.rs (`reapply_signs_the_restored_messages_and_target_copies`)
  [@test] crates/update-core/src/process_budget_test.rs (`signing_an_update_adds_one_batch_read_and_write`)
- Sync signs new virtual and stack branch copies. With signing on, a mapping note without the signed marker is not reused;
  once a signed copy is remembered, later syncs reuse it. With signing off, signed copies remain reusable.
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_marks_and_reuses_its_virtual_copies`, `a_signed_sync_with_a_missing_key_moves_no_virtual_branch_ref`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_builds_a_signed_stack_branch`)
  [@test] crates/sync-core/src/stack_branch_test.rs (`a_stack_build_signs_every_copy`, `an_unsigned_stack_build_is_not_reused_when_signing_is_enabled`)

### Git processes

- `fast-import` signs user commits in its existing run. A `git replay` result is re-signed in one batch before the ref
  transaction, with at most one `cat-file --batch` and one `fast-import` added per operation.
- Signing is prepared before the repository lock is taken.

## Testing / Local Run

- `cargo nextest run -p git-ops`: writer and signature tests.
- `pnpm lint:specs`: specification targets and links.

## References

- [Git Operations](../../../../docs/design/git-operations.md)
- [Git Rules](../../../../docs/contributing/git-rules.md)