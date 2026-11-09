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

## Purpose

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

### Requirement: Signing: `commit.gpgsign` of the repository decides

Branch Deck MUST read `commit.gpgsign` from the repository. When it is false or unset, existing commit ids and behavior
MUST stay unchanged.

#### Scenario: Signing is off
- **WHEN** `commit.gpgsign` is false or unset
- **THEN** a commit Branch Deck writes has the id it had before, unsigned
  [@test] crates/git-ops/src/commit_writer_test.rs (`signing_configuration_and_unsigned_writer_ids_are_unchanged`)

### Requirement: Signing: Every commit written onto a branch is signed

When `commit.gpgsign` is true, every commit written onto a branch MUST have a signature that `git verify-commit`
accepts with the configured `gpg.format`, `user.signingkey` and signing program. The tree, parents, author, committer,
dates, encoding and message MUST remain as before the rewrite.

#### Scenario: A commit written with signing on
- **GIVEN** `commit.gpgsign` is true
- **WHEN** Branch Deck writes a commit onto a branch
- **THEN** `git verify-commit` accepts its signature
- **AND** its tree, parents, author, committer, dates, encoding and message are as before the rewrite
  [@test] crates/git-ops/src/commit_writer_test.rs (`signing_writes_verifiable_user_commits_but_leaves_store_and_notes_commits_unsigned`)

#### Scenario: A chain of commits
- **GIVEN** `commit.gpgsign` is true
- **WHEN** Branch Deck writes a chain of commits
- **THEN** `git verify-commit` accepts every one of them
  [@test] crates/git-ops/src/commit_chain_test.rs (`a_signing_chain_writes_verifiable_signatures_for_every_commit`)

### Requirement: Signing: Store and notes commits stay unsigned

Store commits and notes commits written in the same `fast-import` stream MUST stay unsigned.

#### Scenario: One stream of user, store and notes commits
- **GIVEN** `commit.gpgsign` is true
- **WHEN** one `fast-import` stream writes user commits, store commits and notes commits
- **THEN** the store and notes commits are unsigned
  [@test] crates/git-ops/src/commit_writer_test.rs (`signing_writes_verifiable_user_commits_but_leaves_store_and_notes_commits_unsigned`)

### Requirement: Signing: A failure to sign moves no ref

A missing key, locked signer or missing signing program MUST fail with `signing_failed` and Git's words. No branch or
store ref MUST move, and temporary refs MUST be cleaned up.

#### Scenario: The key is missing
- **GIVEN** `commit.gpgsign` is true
- **WHEN** the key is missing, the signer locked or the signing program missing
- **THEN** the operation fails with `signing_failed` and Git's words
- **AND** no branch or store ref moves, and temporary refs are cleaned up
  [@test] crates/git-ops/src/commit_writer_test.rs (`a_fast_import_signing_failure_is_typed_and_does_not_move_the_branch`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_with_a_missing_key_moves_no_virtual_branch_ref`)

### Requirement: Branch operations: New commits are signed before their ref transaction

History rewrites, amend, move-hunk, unapply/reapply, pick, Update Project and its resolve sessions MUST sign new
commits before their ref transaction.

#### Scenario: A history rewrite
- **GIVEN** `commit.gpgsign` is true
- **WHEN** a fold, a drop, a reword or a squash rewrites the history
- **THEN** every new commit is signed before the ref transaction
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_fold_signs_replacements_and_replayed_commits`)
  [@test] crates/history-rewrite/src/drop_commits_test.rs (`dropping_a_commit_signs_the_replayed_survivors`)
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_reword_signs_every_rewritten_commit`)
  [@test] crates/history-rewrite/src/squash_test.rs (`a_squash_and_its_replayed_descendants_are_signed`)

#### Scenario: An amend
- **GIVEN** `commit.gpgsign` is true
- **WHEN** an amend runs
- **THEN** the replacement and its replayed descendants are signed
  [@test] crates/git-ops/src/amend_operations_test.rs (`amend_signs_the_replacement_and_replayed_descendants`)

#### Scenario: A move of a hunk
- **GIVEN** `commit.gpgsign` is true
- **WHEN** a hunk moves into another commit
- **THEN** every rewritten commit is signed
  [@test] crates/git-ops/src/move_hunk_test.rs (`moving_a_hunk_signs_every_rewritten_commit`)

#### Scenario: Unapply and reapply
- **GIVEN** `commit.gpgsign` is true
- **WHEN** a branch is unapplied or reapplied
- **THEN** its new commits are signed
  [@test] crates/sync-core/src/reapply_branch_test.rs (`reapply_signs_the_restored_messages_and_target_copies`)

#### Scenario: A pick that writes new commits
- **GIVEN** `commit.gpgsign` is true
- **WHEN** a pick writes new commits
- **THEN** they are signed before its ref transaction

#### Scenario: Update Project and its resolve sessions
- **GIVEN** `commit.gpgsign` is true
- **WHEN** Update Project runs, or a resolve session finishes
- **THEN** its new commits are signed before its ref moves
  [@test] crates/update-core/src/process_budget_test.rs (`signing_an_update_adds_one_batch_read_and_write`)
  [@test] crates/git-ops/src/update_resolve_test.rs (`a_words_resolved_finish_is_signed_before_its_ref_moves`)

### Requirement: Branch operations: A fast-forward pick keeps the original commits

A pick that is a fast-forward MUST keep the original commits.

#### Scenario: A fast-forward pick with signing on
- **GIVEN** `commit.gpgsign` is true
- **WHEN** a pick is a fast-forward
- **THEN** the branch gets the original commits, not signed copies
  [@test] crates/git-ops/src/pick_test.rs (`a_fast_forward_pick_keeps_the_original_commits_with_signing_enabled`)

### Requirement: Branch operations: Sync signs new virtual and stack branch copies

Sync MUST sign new virtual and stack branch copies.

#### Scenario: Virtual branch copies
- **GIVEN** `commit.gpgsign` is true
- **WHEN** a sync writes new copies of virtual branches
- **THEN** they are signed
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_marks_and_reuses_its_virtual_copies`)

#### Scenario: Stack branch copies
- **GIVEN** `commit.gpgsign` is true
- **WHEN** a sync builds a stack branch
- **THEN** every copy of it is signed
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_builds_a_signed_stack_branch`)
  [@test] crates/sync-core/src/stack_branch_test.rs (`a_stack_build_signs_every_copy`)

#### Scenario: The key is missing
- **GIVEN** `commit.gpgsign` is true and the key is missing
- **WHEN** a sync runs
- **THEN** no virtual branch ref moves
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_with_a_missing_key_moves_no_virtual_branch_ref`)

### Requirement: Branch operations: A copy is reused by the signed marker of its mapping note

With signing on, a mapping note without the signed marker MUST NOT be reused; once a signed copy is remembered, later
syncs MUST reuse it. With signing off, signed copies MUST remain reusable.

#### Scenario: Signing turned on over unsigned copies
- **GIVEN** a sync left unsigned copies
- **WHEN** signing is on and a sync runs
- **THEN** a mapping note without the signed marker is not reused, and the copies are written again, signed
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_marks_and_reuses_its_virtual_copies`)
  [@test] crates/sync-core/src/stack_branch_test.rs (`an_unsigned_stack_build_is_not_reused_when_signing_is_enabled`)

#### Scenario: A signed copy is remembered
- **GIVEN** a signed copy is remembered
- **WHEN** a later sync runs with signing on
- **THEN** it reuses the signed copy
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_marks_and_reuses_its_virtual_copies`)

#### Scenario: Signing turned off over signed copies
- **GIVEN** signed copies are remembered
- **WHEN** signing is off and a sync runs
- **THEN** the signed copies are reused
  [@test] crates/sync-core/src/process_budget_test.rs (`a_signed_sync_marks_and_reuses_its_virtual_copies`)

### Requirement: Git processes: `fast-import` signs user commits in its existing run

`fast-import` MUST sign user commits in its existing run.

#### Scenario: Commits written by `fast-import`
- **GIVEN** `commit.gpgsign` is true
- **WHEN** `fast-import` writes user commits
- **THEN** it signs them in that same run

### Requirement: Git processes: A `git replay` result is re-signed in one batch

A `git replay` result MUST be re-signed in one batch before the ref transaction, with at most one `cat-file --batch`
and one `fast-import` added per operation.

#### Scenario: An operation that replays commits
- **GIVEN** `commit.gpgsign` is true
- **WHEN** an operation replays commits with `git replay`
- **THEN** the result is re-signed in one batch before the ref transaction
- **AND** the operation adds at most one `cat-file --batch` and one `fast-import`
  [@test] crates/update-core/src/process_budget_test.rs (`signing_an_update_adds_one_batch_read_and_write`)

### Requirement: Git processes: Signing is prepared before the repository lock

Signing MUST be prepared before the repository lock is taken.

#### Scenario: An operation under the repository lock
- **GIVEN** `commit.gpgsign` is true
- **WHEN** an operation takes the repository lock
- **THEN** its signing was prepared before it

## Testing / Local Run

- `cargo nextest run -p git-ops`: writer and signature tests.
- `pnpm lint:specs`: specification targets and links.

## References

- [Git Operations](../../../../docs/design/git-operations.md)
- [Git Rules](../../../../docs/contributing/git-rules.md)