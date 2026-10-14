---
name: Squash Fixups
description: Requirements for squashing fixup!, squash! and amend! commits into their targets with git's autosquash rules, per branch, per unassigned list or one fixup at a time, without touching the working tree.
style: plain-1
targets:
  - ../../crates/sync-core/src/autosquash.rs
  - ../../crates/sync-core/src/commit_grouper.rs
  - ../../src-tauri/src/commands/squash_fixups.rs
  - ../../src/lib/autosquash.ts
  - ../../src/lib/commit.ts
  - ../../src/hooks/useSquashFixups.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/components/commit/SquashFixupsButton.tsx
  - ../../src/components/commit/SquashFixupsPopover.tsx
  - ../../src/components/commit/CommitList.tsx
  - ../../src/components/branch/BranchNameCell.tsx
---

# Squash Fixups

Status: Implemented
Date: 2026-10-14

## Summary

`git commit --fixup` and `--squash` leave commits named `fixup! X`, `squash! X` and `amend! X`, which
`git rebase --autosquash` folds into X. Branch Deck finds the target of each such commit with git's rules and squashes
the fixups of a branch, of the unassigned commits, or a single fixup, in one history rewrite that keeps HEAD's tree. The
squash engine and the undo are those of [Squash Commits](squash.spec.md).

## Goals

- See which commits are fixups and where each one goes.
- Squash the fixups of one branch before pushing it, without a terminal rebase.

## Non-goals

- Reordering commits, editing the combined message (squash the commits by selection for that), or resolving conflicts.
- Fixups whose target is outside the commits ahead of the baseline.

## Requirements

### Target resolution

- A subject is a fixup if it starts with `fixup! `, `squash! ` or `amend! `; the space is required, as in git
  (`fixup!X` is an ordinary subject). All leading markers are stripped; the first one gives the kind.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`strips_every_marker_and_takes_the_kind_from_the_first`, `needs_the_space_after_the_marker`)
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`test_autosquash_commits_edge_cases`)
  [@test] ../../src/lib/commit.test.ts (`groupFor`)
- The target is the oldest older commit of the range that matches, in this order:
  1. the exact subject;
  2. a unique hash prefix of at least 4 hex digits;
  3. the subject without the `(prefix)`, the prefixes agreeing if the fixup names one;
  4. a subject (or, prefixes agreeing, a subject without the `(prefix)`) starting with the text.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`matches_the_exact_subject_and_keeps_the_details`, `matches_the_subject_without_the_prefix`, `matches_a_unique_hash_prefix`, `falls_back_to_a_subject_starting_with_the_text`, `picks_the_oldest_of_duplicate_subjects`)
- Prefixes agree when their text is the same. With [stacked branches](../branch/stacked-branches.spec.md) on, they agree
  when they name the same leaf (`(login)` and `(auth/login)`), and the branch of a group is the leaf.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`without_stacks_prefixes_are_compared_as_text`, `with_stacks_a_fixup_names_the_leaf_of_its_target`)
- A fixup of a fixup goes to the final target. A fixup without a matching older commit (the target is newer, or outside
  the range) is an orphan: it is reported and not squashed.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`resolves_a_fixup_of_a_fixup_to_the_final_target`, `orphans_a_fixup_without_an_older_target`)

### Message

- Only fixups: the target keeps its message and its encoding byte for byte. A `squash!` appends its body (the text
  after its subject line); an `amend!` replaces the message with its body; later ones apply to the result.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`builds_the_message_like_git`)
- A combined message is written as UTF-8; if an involved commit has another encoding, the squash is refused.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`refuses_to_combine_messages_in_another_encoding`)

### Rewrite

- The groups of the scope are squashed in one rewrite (`branch-deck: squash fixups`): the fixups leave the history, the
  targets keep their place and author. The working tree, the index and the untracked files stay as they are.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`squashes_interleaved_groups_in_one_rewrite`, `keeps_a_dirty_tree_untouched`)
- The scope is a branch (the groups whose target is grouped into it), the unassigned commits (targets without a
  branch), or one fixup. A scope with nothing to squash is refused.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`squashes_only_the_fixups_of_one_branch_or_one_fixup`)
- The plan is computed again and the squash is refused if HEAD is not the one the user confirmed.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`refuses_when_the_branch_moved_since_the_plan`)

### UI

- After a sync that found fixup commits, the plan is loaded once; a branch with fixups shows a muted "N fixups" badge
  next to its name. Orphans are not counted.
  [@test] ../../src/lib/autosquash.test.ts (`takes the groups by the branch of their target and the orphans by their own branch`)
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from its commits and undo it`)
- A fixup commit row shows its marker, muted (warning colour for an orphan), with a tooltip "Squashes into “X”" or
  "No target in this range".
  [@test] ../../src/lib/autosquash.test.ts (`names the target of a fixup, or none for an orphan`)
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from its commits and undo it`)
- "Squash N Fixups" (ghost `xs`) appears above the commits of an expanded branch and in the unassigned card header. The
  branch menu has "Squash Fixups (N)" in a "History" group, which expands the branch and opens the confirmation.
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should squash from the branch menu and one fixup from the commit menu`)
- The confirmation popover reads the plan again and lists each target with its fixups, then the orphans under
  "Won't be squashed: no target in this range"; Squash runs it, Cancel or Escape closes it.
- The menu of a fixup commit with a target offers "Squash into “X”", which squashes that fixup only.
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should squash from the branch menu and one fixup from the commit menu`)
- Success shows "Squashed N fixups" with Undo, as for a squash of selected commits.
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from its commits and undo it`)

## Testing / Local Run

- `pnpm test:backend`: `autosquash_test.rs`, `commit_grouper_test.rs`.
- `pnpm test:unit`: `src/lib/autosquash.test.ts`, `src/lib/commit.test.ts`.
- `pnpm e2e`: `tests/e2e/squash-fixups` uses the `fixups` template.

## Open Questions / Risks

- git resolves a hash prefix against the whole repository; Branch Deck only against the commits of the range, so a
  prefix that git finds ambiguous may still match here.
- The badges use the plan of the last sync; the confirmation and the squash always read it again.

## References

- [Squash Commits](squash.spec.md)
- [Git Operations](../../docs/design/git-operations.md)
- `git help rebase` (`--autosquash`), `git help commit` (`--fixup`)
