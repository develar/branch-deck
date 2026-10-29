---
name: Squash Fixups
description: Requirements for squashing fixup!, squash! and amend! commits into their targets with git's autosquash rules, per branch, per unassigned list or one fixup at a time, without touching the working tree.
style: plain-1
targets:
  - ../../crates/history-rewrite/src/autosquash.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/history-rewrite/src/commit_grouper.rs
  - ../../crates/sync-utils/src/commit_subject.rs
  - ../../crates/git-ops/src/autosquash_targets.rs
  - ../../crates/app-api/src/reads/commits.rs
  - ../../crates/operations/src/rewrite.rs
  - ../../src/lib/autosquash.ts
  - ../../src/lib/commit.ts
  - ../../src/hooks/useSquashFixups.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/components/commit/SquashFixupsButton.tsx
  - ../../src/components/commit/SquashFixupsPopover.tsx
  - ../../src/components/commit/CommitList.tsx
  - ../../src/components/branch/BranchNameCell.tsx
  - ../../src/presenters/rewrite.ts
---

# Squash Fixups

Status: Implemented
Date: 2026-10-15

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
  (`fixup!X` is an ordinary subject). All leading markers are stripped; the first one gives the kind. A subject without
  a marker is never a fixup, also when it starts with an issue key (`ABC-1 Fix x`) or a prefix and an older commit has
  the same subject; a prefixed commit stays the target of a fixup that names it.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`strips_every_marker_and_takes_the_kind_from_the_first`, `needs_the_space_after_the_marker`, `a_subject_without_a_marker_is_never_a_fixup`)
  [@test] ../../crates/history-rewrite/src/commit_grouper_test.rs (`test_autosquash_commits_edge_cases`)
  [@test] ../../crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_of_a_subject_is_its_prefix_else_the_issue_group_of_its_key_else_the_key`)
- The target is the oldest older commit of the range that matches, in this order:
  1. the exact subject;
  2. a unique hash prefix of at least 4 hex digits;
  3. the subject without the `(prefix)`, the prefixes agreeing if the fixup names one;
  4. a subject (or, prefixes agreeing, a subject without the `(prefix)`) starting with the text.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`matches_the_exact_subject_and_keeps_the_details`, `matches_the_subject_without_the_prefix`, `matches_a_unique_hash_prefix`, `falls_back_to_a_subject_starting_with_the_text`, `picks_the_oldest_of_duplicate_subjects`)
- Prefixes agree when their text is the same. With [stacked branches](../branch/stacked-branches.spec.md) on, they agree
  when they name the same leaf (`(login)` and `(auth/login)`), and the branch of a group is the leaf.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`without_stacks_prefixes_are_compared_as_text`, `with_stacks_a_fixup_names_the_leaf_of_its_target`)
- A fixup of a fixup goes to the final target. A fixup without a matching older commit (the target is newer, or outside
  the range) is an orphan: it is reported and not squashed.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`resolves_a_fixup_of_a_fixup_to_the_final_target`, `orphans_a_fixup_without_an_older_target`)

### Message

- Only fixups: the target keeps its message and its encoding byte for byte. A `squash!` appends its body (the text
  after its subject line); an `amend!` replaces the message with its body; later ones apply to the result.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`builds_the_message_like_git`)
- A combined message is written as UTF-8; if an involved commit has another encoding, the squash is refused.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`refuses_to_combine_messages_in_another_encoding`)

### Rewrite

- The groups of the scope are squashed in one rewrite (`branch-deck: squash fixups`): the fixups leave the history, the
  targets keep their place and author. The working tree, the index and the untracked files stay as they are.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`squashes_interleaved_groups_in_one_rewrite`, `keeps_a_dirty_tree_untouched`)
- The scope is a branch (the groups whose target is grouped into it), the unassigned commits (targets without a
  branch), or one fixup. A scope with nothing to squash is refused.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`squashes_only_the_fixups_of_one_branch_or_one_fixup`)
- The squash of the app is the rewrite `squashFixups` of the engine, named on the table the window shows; the session
  runs it on the HEAD of that table, or on the one the rewrites before it in its queue left. The plan is computed again
  and the squash is refused if HEAD is not that one. HEAD, the newest commit of the planned range and the history the
  squash rewrites must all be that head, so a commit made between these reads never gives a plan for one head and a
  rewrite of another.
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`refuses_when_the_branch_moved_since_the_plan`)
- The squash must run under the repository lock, taken after the plan is read again and the squashed commits are
  written on temporary refs, for the witness of HEAD and the compare-and-swap move only (a HEAD that moved meanwhile
  plans once more under the lock): while another process holds it
  (an agent's `bd pick --follow`, `bd`) or the app's own push, the squash waits as long as the holder lives, its
  surface saying whom for, the step of its operation ("Waiting for `bd pick` (pid 4242, for 12 s)"), with Stop Waiting
  (`cancel_operation`), which ends the wait with nothing changed ("Canceled squashing the fixups before it started");
  only a holder no record names refuses it after 10 s, with nothing changed and the toast "Cannot squash the fixups"
  saying that another operation holds the lock.
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`, `a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)
  [@test] ../../crates/history-rewrite/src/autosquash_test.rs (`a_fold_locks_only_the_witness_and_the_write`)
- `bd fold` runs the same squash ([Agent CLI](../cli/agent-cli.spec.md)): it passes its lock timeout and
  `--expect-head` to the squash, which takes the lock and refuses another HEAD, so `bd fold` fails with `locked` or
  `head_moved` and changes nothing. Without `--expect-head` the squash names no HEAD: it folds the fixups of HEAD as it
  reads it, once more under the lock when HEAD moved meanwhile, and answers what it folded (each target with its
  subject, its new id and its fixups, and the orphans), which `bd fold` prints.
  [@test] ../../crates/bd-rewrite/src/fold_test.rs (`fold_keeps_the_tree_and_the_undo_restores_the_head`, `fold_is_refused_on_another_head_or_while_the_lock_is_held`, `a_head_moved_after_the_plan_of_the_fold_is_folded_as_its_core_finds_it`)
- A fixup that conflicts with the commits between it and its target or after it, or that leaves its target or a later
  commit empty, refuses the squash as the squash of a selection does ([Squash Commits](squash.spec.md)):
  `squash_conflict` with the commits and files it names, `squash_empty`; the commits after the folded ones keep their
  mapping notes.
  [@test] ../../crates/bd-rewrite/src/fold_test.rs (`a_fold_whose_fixup_conflicts_is_refused_as_a_squash_conflict`)

### UI

- After a sync that found fixup commits, the plan is loaded once; a branch with fixups shows a muted "N fixups" badge
  next to its name. Orphans are not counted.
  [@test] ../../src/lib/autosquash.test.ts (`takes the groups by the branch of their target and the orphans by their own branch`)
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from its commits and undo it`)
- A fixup commit row shows its marker, muted (warning colour for an orphan), with a tooltip "Squashes into “X”" or
  "No target in this range".
  [@test] ../../src/lib/autosquash.test.ts (`names the target of a fixup, or none for an orphan`)
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from its commits and undo it`)
- "Squash N Fixups…" ("Squash 1 Fixup…", ghost `xs`, `formatSquashFixupsLabel`) appears above the commits of an
  expanded branch and in the unassigned card header; it opens the confirmation, hence the ellipsis. The branch menu has
  the same item, named alike, in a "History" group, which expands the branch and opens the confirmation.
  [@test] ../../src/lib/autosquash.test.ts (`names the squash of the fixups alike on the button and in the branch menu, which open the confirmation`)
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should squash from the branch menu and one fixup from the commit menu`)
- The confirmation popover reads the plan again and lists each target with its fixups, then the orphans under
  "Won't be squashed: no target in this range"; Squash runs it, Cancel, Escape or a click outside closes it. Nothing
  closes it while the squash runs (as the squash dialog): it closes when the squash is done or failed.
- Its title "Squash N fixups" ("Squash 1 fixup") must say in the number of the fixups what the squash does: "It is
  folded into its target, as with rebase --autosquash. Your files are not touched." for one, "Each one is folded…"
  for more; "No fixups to squash" has no such sentence.
  [@test] ../../src/lib/autosquash.test.ts (`says what the squash does in the number of its fixups, one as one`)
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should keep every row in its place when the fixup of the branch that began first is squashed`)
- The menu of a fixup commit with a target offers "Squash into “X”", which squashes that fixup only.
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should squash from the branch menu and one fixup from the commit menu`)
- Success shows "Squashed N fixups" with Undo, as for a squash of selected commits (the rewrite `undo` of its
  `squash` token).
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from its commits and undo it`)

## Testing / Local Run

- `pnpm test:backend`: `autosquash_test.rs`, `commit_grouper_test.rs`.
- `pnpm test:unit`: `src/lib/autosquash.test.ts`.
- `pnpm e2e`: `tests/e2e/squash-fixups` uses the `fixups` template, and `fixup_order` for the order of the rows.

## Open Questions / Risks

- git resolves a hash prefix against the whole repository; Branch Deck only against the commits of the range, so a
  prefix that git finds ambiguous may still match here.
- The badges use the plan of the last sync; the confirmation and the squash always read it again.

## References

- [Squash Commits](squash.spec.md)
- [Git Operations](../../docs/design/git-operations.md)
- `git help rebase` (`--autosquash`), `git help commit` (`--fixup`)
