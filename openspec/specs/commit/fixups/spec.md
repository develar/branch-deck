---
name: Squash Fixups
description: Requirements for squashing fixup!, squash! and amend! commits into their targets with git's autosquash rules, per branch, per unassigned list or one fixup at a time, without touching the working tree.
style: plain-1
targets:
  - crates/history-rewrite/src/autosquash.rs
  - crates/history-rewrite/src/fixup_targets.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/history-rewrite/src/commit_grouper.rs
  - crates/sync-utils/src/commit_subject.rs
  - crates/git-ops/src/autosquash_targets.rs
  - crates/bd-rewrite/src/commits.rs
  - crates/git-ops/src/rewrite_plan.rs
  - crates/git-ops/src/squash_commits.rs
  - crates/bd-rewrite/src/fold_command.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/app-api/src/reads/commits.rs
  - crates/operations/src/rewrite.rs
  - src/lib/autosquash.ts
  - src/lib/commit.ts
  - src/hooks/useSquashFixups.ts
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useCommitMoveList.ts
  - src/components/commit/SquashFixupsButton.tsx
  - src/components/commit/SquashFixupsPopover.tsx
  - src/components/commit/CommitList.tsx
  - src/components/branch/BranchNameCell.tsx
  - src/presenters/rewrite.ts
---

# Squash Fixups

Status: Implemented
Date: 2026-10-15

## Purpose

`git commit --fixup` and `--squash` leave commits named `fixup! X`, `squash! X` and `amend! X`, which
`git rebase --autosquash` folds into X. Branch Deck finds the target of each such commit with git's rules and squashes
the fixups of a branch, of the unassigned commits, or a single fixup, in one history rewrite that keeps HEAD's tree. The
squash engine and the undo are those of [Squash Commits](../squash/spec.md).

## Goals

- See which commits are fixups and where each one goes.
- Squash the fixups of one branch before pushing it, without a terminal rebase.

## Non-goals

- Reordering commits, editing the combined message (squash the commits by selection for that), or resolving conflicts.
- Fixups whose target is outside the commits ahead of the baseline.

## Requirements

### Requirement: Target resolution: A subject with a marker is a fixup

A subject MUST be a fixup if it starts with `fixup! `, `squash! ` or `amend! `, or if its `(prefix)` is followed by one
or more of those markers; the space after each marker MUST be present, as in git (`fixup!X` and `(b) fixup!X` are
ordinary subjects).

#### Scenario: A marker before or after the prefix
- **WHEN** a subject starts with `fixup! `, `squash! ` or `amend! `, or its `(prefix)` is followed by one or more of
  those markers
- **THEN** it is a fixup
  [@test] crates/sync-utils/src/commit_subject.rs (`autosquash_subject_reads_markers_before_or_after_a_prefix`)

#### Scenario: No space after the marker
- **WHEN** a marker is not followed by a space, as in `fixup!X` and `(b) fixup!X`
- **THEN** the subject is an ordinary subject, as in git
  [@test] crates/history-rewrite/src/autosquash_test.rs (`needs_the_space_after_the_marker`)
  [@test] crates/sync-utils/src/commit_subject.rs (`autosquash_subject_requires_the_space_after_the_marker_and_prefix`)
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`test_autosquash_commits_edge_cases`)
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_of_a_subject_is_its_prefix_else_the_issue_group_of_its_key_else_the_key`)

### Requirement: Target resolution: All markers are stripped and the first gives the kind

All markers of a fixup MUST be stripped, and the first one MUST give its kind. `(b) fixup! X` names `(b) X`.

#### Scenario: Several markers
- **WHEN** a subject carries several markers, before or after its `(prefix)`
- **THEN** all of them are stripped and the first one gives the kind
  [@test] crates/history-rewrite/src/autosquash_test.rs (`strips_every_marker_and_takes_the_kind_from_the_first`)
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_prefixed_fixup_strips_every_marker_and_takes_the_first_kind`)

#### Scenario: A marker after the prefix
- **WHEN** the subject is `(b) fixup! X`
- **THEN** it names `(b) X`
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_fixup_marker_after_the_prefix_names_the_prefixed_subject`)

### Requirement: Target resolution: A subject without a marker is never a fixup

A subject without a marker MUST NOT be a fixup, also when it starts with an issue key (`ABC-1 Fix x`) or a prefix and
an older commit has the same subject.

#### Scenario: An issue key or a prefix with an older commit of the same subject
- **GIVEN** an older commit with the same subject
- **WHEN** a subject without a marker starts with an issue key (`ABC-1 Fix x`) or a prefix
- **THEN** it is not a fixup
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_subject_without_a_marker_is_never_a_fixup`)

### Requirement: Target resolution: A prefixed fixup is never a subject-match target

A prefixed fixup MUST NOT itself be a subject-match target.

#### Scenario: A fixup names the subject of a prefixed fixup
- **GIVEN** a prefixed fixup
- **WHEN** a later fixup names its subject
- **THEN** the prefixed fixup is not its target
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_prefixed_fixup_is_not_a_subject_match_target`)

### Requirement: Target resolution: The target is the oldest older commit that matches

The target MUST be the oldest older commit of the range that matches, in this order:
1. the exact subject;
2. a unique hash prefix of at least 4 hex digits after all markers, without a `(prefix)`;
3. the subject without the `(prefix)`, the prefixes agreeing if the fixup names one;
4. a subject (or, prefixes agreeing, a subject without the `(prefix)`) starting with the text.

#### Scenario: The exact subject
- **WHEN** an older commit has the exact subject the fixup names
- **THEN** it is the target
  [@test] crates/history-rewrite/src/autosquash_test.rs (`matches_the_exact_subject_and_keeps_the_details`)
  [@test] crates/bd-rewrite/src/commits_test.rs (`entries_name_a_prefixed_fixup_target_with_its_prefix`)

#### Scenario: A unique hash prefix
- **WHEN** the text after all markers is a unique hash prefix of at least 4 hex digits, without a `(prefix)`
- **THEN** the commit it names is the target
  [@test] crates/history-rewrite/src/autosquash_test.rs (`matches_a_unique_hash_prefix`)
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_prefixed_fixup_names_a_hash_prefix_by_id`)

#### Scenario: The subject without the prefix
- **WHEN** no subject matches exactly and an older commit has the subject without the `(prefix)`, the prefixes
  agreeing if the fixup names one
- **THEN** it is the target
  [@test] crates/history-rewrite/src/autosquash_test.rs (`matches_the_subject_without_the_prefix`)

#### Scenario: A subject starting with the text
- **WHEN** nothing else matches and a subject (or, prefixes agreeing, a subject without the `(prefix)`) starts with the
  text
- **THEN** that commit is the target
  [@test] crates/history-rewrite/src/autosquash_test.rs (`falls_back_to_a_subject_starting_with_the_text`)

#### Scenario: Duplicate subjects
- **WHEN** several older commits match
- **THEN** the oldest of them is the target
  [@test] crates/history-rewrite/src/autosquash_test.rs (`picks_the_oldest_of_duplicate_subjects`)

### Requirement: Target resolution: Prefixes agree when they name the same leaf

Prefixes MUST agree when they name the same leaf (`(login)` and `(auth/login)`,
[Stacked Branches](../../branch/stacked-branches/spec.md)), and the branch of a group MUST be the leaf.

#### Scenario: The same leaf
- **WHEN** a fixup names `(login)` and its target `(auth/login)`
- **THEN** the prefixes agree and the branch of the group is the leaf
  [@test] crates/history-rewrite/src/autosquash_test.rs (`with_stacks_a_fixup_names_the_leaf_of_its_target`)

#### Scenario: Another leaf
- **WHEN** a fixup names a prefix of another leaf than its target's
- **THEN** the prefixes do not agree
  [@test] crates/history-rewrite/src/autosquash_test.rs (`with_stacks_another_leaf_does_not_agree`)

### Requirement: Target resolution: A fixup of a fixup goes to the final target

A fixup of a fixup MUST go to the final target.

#### Scenario: A fixup names a fixup
- **WHEN** a fixup names another fixup
- **THEN** it goes to the final target
  [@test] crates/history-rewrite/src/autosquash_test.rs (`resolves_a_fixup_of_a_fixup_to_the_final_target`)

### Requirement: Target resolution: A fixup without a matching older commit is an orphan

A fixup without a matching older commit (the target is newer, or outside the range) MUST be an orphan: it is reported
and MUST NOT be squashed.

#### Scenario: The target is newer or outside the range
- **WHEN** no older commit of the range matches the fixup, as the target is newer or outside the range
- **THEN** it is an orphan: reported and not squashed
  [@test] crates/history-rewrite/src/autosquash_test.rs (`orphans_a_fixup_without_an_older_target`)

### Requirement: Message: The message is built like git

With only fixups, the target MUST keep its message and its encoding byte for byte. A `squash!` MUST append its body
(the text after its subject line); an `amend!` MUST replace the message with its body; later ones apply to the result.

#### Scenario: Only fixups
- **WHEN** only fixups fold into a target
- **THEN** the target keeps its message and its encoding byte for byte
  [@test] crates/history-rewrite/src/autosquash_test.rs (`builds_the_message_like_git`)

#### Scenario: A squash and an amend
- **WHEN** a `squash!` folds into a target, or an `amend!`, and later ones after it
- **THEN** a `squash!` appends its body (the text after its subject line), an `amend!` replaces the message with its
  body
- **AND** later ones apply to the result
  [@test] crates/history-rewrite/src/autosquash_test.rs (`builds_the_message_like_git`)

### Requirement: Message: A combined message is UTF-8

A combined message MUST be written as UTF-8; if an involved commit has another encoding, the squash MUST be refused.

#### Scenario: A commit in another encoding
- **GIVEN** an involved commit with another encoding than UTF-8
- **WHEN** the squash combines messages
- **THEN** it is refused
  [@test] crates/history-rewrite/src/autosquash_test.rs (`refuses_to_combine_messages_in_another_encoding`)

### Requirement: Rewrite: The groups of the scope are squashed in one rewrite

The groups of the scope MUST be squashed in one rewrite (`branch-deck: squash fixups`): the fixups leave the history,
the targets keep their place and author.

#### Scenario: Interleaved groups
- **WHEN** the fixups of several targets are interleaved in the scope
- **THEN** they are squashed in one rewrite, `branch-deck: squash fixups`
- **AND** the fixups leave the history and the targets keep their place and author
  [@test] crates/history-rewrite/src/autosquash_test.rs (`squashes_interleaved_groups_in_one_rewrite`)

### Requirement: Rewrite: The working tree, the index and the untracked files stay

The working tree, the index and the untracked files MUST stay as they are.

#### Scenario: A dirty tree
- **GIVEN** changed, staged and untracked files
- **WHEN** the fixups are squashed
- **THEN** the working tree, the index and the untracked files stay as they are
  [@test] crates/history-rewrite/src/autosquash_test.rs (`keeps_a_dirty_tree_untouched`)

### Requirement: Rewrite: The scope is a branch, the unassigned commits or one fixup

The scope MUST be a branch (the groups whose target is grouped into it), the unassigned commits (targets without a
branch), or one fixup.

#### Scenario: A branch, then one fixup
- **WHEN** the scope is a branch, the unassigned commits or one fixup
- **THEN** only the groups whose target is grouped into that branch, the targets without a branch, or that fixup are
  squashed
  [@test] crates/history-rewrite/src/autosquash_test.rs (`squashes_only_the_fixups_of_one_branch_or_one_fixup`)

### Requirement: Rewrite: A scope with nothing to squash is refused

A scope with nothing to squash MUST be refused.

#### Scenario: Nothing to squash
- **WHEN** the scope holds no fixup to squash, only an orphan or none at all
- **THEN** the squash is refused
  [@test] crates/history-rewrite/src/autosquash_test.rs (`squashes_only_the_fixups_of_one_branch_or_one_fixup`)

### Requirement: Rewrite: The app runs the squash on HEAD as its session reads it

The squash of the app MUST be the rewrite `squashFixups` of the engine, named on the table the window shows. Once its
turn in the queue comes, the session MUST run it on HEAD as it reads it then: the HEAD of that table, the one the
rewrites before it in its queue left, or one another process moved it to.

#### Scenario: A squash of the app
- **WHEN** the app squashes fixups
- **THEN** it sends the rewrite `squashFixups` of the engine, named on the table the window shows
- **AND** once its turn comes the session runs it on HEAD as it reads it then: the HEAD of that table, the one the
  rewrites before it in its queue left, or one another process moved it to

### Requirement: Rewrite: The app runs the squash once more on HEAD another process moved during its wait

When another process moves HEAD while the squash of the app waits for the lock, so that the squash refuses that HEAD
with nothing written, the session MUST run it once more on HEAD as it is then, at most three runs in all and none after
a Stop Waiting.

#### Scenario: Another process moves HEAD during the wait for the lock
- **GIVEN** the squash waits for the lock another process holds
- **WHEN** that process moves HEAD
- **THEN** the squash refuses that HEAD with nothing written
- **AND** the session runs it once more on HEAD as it is then, at most three runs in all

### Requirement: Rewrite: The plan is computed again and another HEAD refuses the squash

The plan MUST be computed again on HEAD as the squash reads it, and the squash MUST be refused (`HeadMoved`, nothing
written) if HEAD is not the one its request names (the one the session of the app runs it on, `brd fold --expect-head`).

#### Scenario: The branch moved since the plan
- **WHEN** HEAD is not the head the squash names
- **THEN** the squash is refused with nothing written
  [@test] crates/history-rewrite/src/autosquash_test.rs (`refuses_when_the_branch_moved_since_the_plan`)

### Requirement: Rewrite: HEAD, the planned range and the rewritten history are one head

HEAD, the newest commit of the planned range and the history the squash rewrites MUST all be that head, so a commit made
between these reads never gives a plan for one head and a rewrite of another.

#### Scenario: A commit made between the reads
- **WHEN** a commit is made between the reads of HEAD, the planned range and the history
- **THEN** the squash never plans for one head and rewrites another

### Requirement: Rewrite: The squash holds the repository lock only for the witness and the move

The squash MUST run under the repository lock, taken after the plan is read again and the squashed commits are written
on temporary refs, for the witness of HEAD and the compare-and-swap move only (a HEAD that moved meanwhile plans once
more under the lock).

#### Scenario: What runs under the lock
- **WHEN** the squash runs
- **THEN** the plan is read again and the squashed commits are written on temporary refs before the lock is taken
- **AND** under the lock it only witnesses HEAD and moves it with a compare-and-swap
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_fold_locks_only_the_witness_and_the_write`)

#### Scenario: HEAD moved before the lock
- **WHEN** HEAD moved meanwhile
- **THEN** the squash plans once more under the lock
- **AND** one whose request names its HEAD is refused there if HEAD is another one

### Requirement: Rewrite: The squash waits while a named holder lives

While another process holds the lock (an agent's `brd pick --follow`, `brd`) or the app's own push, the squash MUST
wait as long as the holder lives, its surface saying whom for, the step of its operation ("Waiting for `brd pick` (pid
4242, for 12 s)"), with Stop Waiting (`cancel_operation`).

#### Scenario: A live holder
- **GIVEN** another process holds the lock (an agent's `brd pick --follow`, `brd`) or the app's own push
- **WHEN** the squash starts
- **THEN** it waits as long as the holder lives, its surface saying whom for, the step of its operation ("Waiting for
  `brd pick` (pid 4242, for 12 s)"), with Stop Waiting (`cancel_operation`)
- **AND** it runs once the holder lets go
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_waits_for_the_holder`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`)

### Requirement: Rewrite: Stop Waiting ends the wait with nothing changed

Stop Waiting MUST end the wait with nothing changed ("Canceled squashing the fixups before it started").

#### Scenario: The user stops the wait
- **WHEN** the user presses Stop Waiting while the squash waits for the lock
- **THEN** the wait ends with nothing changed, "Canceled squashing the fixups before it started"
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

### Requirement: Rewrite: Only a holder no record names refuses the squash after 10 s

Only a holder no record names MUST refuse the squash after 10 s, with nothing changed and the toast "Cannot squash the
fixups" saying that another operation holds the lock.

#### Scenario: A holder without a record
- **GIVEN** a holder of the lock that no record names
- **WHEN** the squash has waited 10 s
- **THEN** it is refused with nothing changed and the toast "Cannot squash the fixups" says that another operation
  holds the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

### Requirement: Rewrite: `brd fold` passes its lock timeout and `--expect-head`

`brd fold` MUST run the same squash ([Agent CLI](../../cli/agent-cli/spec.md)): it passes its lock timeout and
`--expect-head` to the squash, which takes the lock and refuses another HEAD, so `brd fold` fails with `locked` or
`head_moved` and changes nothing.

#### Scenario: The same squash
- **WHEN** `brd fold` runs
- **THEN** it runs the same squash, which keeps the tree, and its undo restores the head
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_keeps_the_tree_and_the_undo_restores_the_head`)

#### Scenario: Another HEAD or a held lock
- **WHEN** HEAD is not the one `--expect-head` names, or another holds the lock past its lock timeout
- **THEN** `brd fold` fails with `head_moved` or `locked` and changes nothing
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_is_refused_on_another_head_or_while_the_lock_is_held`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`)

### Requirement: Rewrite: Without `--expect-head` the fold folds HEAD as it reads it

Without `--expect-head` the squash MUST name no HEAD: it folds the fixups of HEAD as it reads it, once more under the
lock when HEAD moved meanwhile, and answers what it folded (each target with its subject, its new id and its fixups,
and the orphans), which `brd fold` prints.

#### Scenario: HEAD moved after the plan
- **WHEN** `brd fold` runs without `--expect-head` and HEAD moved after its plan
- **THEN** the squash folds the fixups of HEAD as it reads it once more under the lock
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_head_moved_after_the_plan_of_the_fold_is_folded_as_its_core_finds_it`)

#### Scenario: What it folded
- **WHEN** the fold succeeds
- **THEN** `brd fold` prints each target with its subject, its new id and its fixups, and the orphans
  [@test] crates/bd-rewrite/src/fold_test.rs (`fold_folds_a_fixup_with_its_marker_after_the_prefix`)

### Requirement: Rewrite: A conflicting fixup refuses the squash as a squash of a selection

A fixup that conflicts with the commits between it and its target or after it, or that leaves a later commit empty,
MUST refuse the squash as the squash of a selection does ([Squash Commits](../squash/spec.md)): `squash_conflict` with
the commits and files it names, `squash_empty`.

#### Scenario: A fixup that conflicts
- **WHEN** a fixup, with its marker before or after the prefix, conflicts with a commit between it and its target
- **THEN** the squash is refused with `squash_conflict`, the commits and files it names
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_fold_whose_fixup_conflicts_is_refused_as_a_squash_conflict`)
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_prefixed_fixup_that_conflicts_is_refused_as_a_squash_conflict`)

#### Scenario: A later commit left empty
- **WHEN** a fixup leaves a later commit empty
- **THEN** the squash is refused with `squash_empty`

### Requirement: Rewrite: The commits after the folded ones keep their mapping notes

The commits after the folded ones MUST keep their mapping notes.

#### Scenario: A replayed commit
- **GIVEN** the commits carry mapping notes of a synced branch
- **WHEN** the fixups fold and the commits after them are replayed
- **THEN** each replayed commit carries its mapping note
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_fold_locks_only_the_witness_and_the_write`)

### Requirement: Rewrite: Fixups that cancel their target take it out of the history

Fixups that cancel their target (the target as they leave it has no changes, as an agent's fixup that undoes its own
commit) MUST take it out of the history with them, the commits in between replayed onto its parent and HEAD's tree
kept; the other groups of the scope fold as always. `git rebase --autosquash` would leave an empty commit or stop
there; the target as the fixups leave it is nothing, so nothing stays.

#### Scenario: A fixup undoes its target
- **WHEN** the fixups of a target leave it without changes
- **THEN** the target leaves the history with them, the commits in between replayed onto its parent and HEAD's tree
  kept
- **AND** the other groups of the scope fold as always
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_target_its_fixups_cancel_leaves_the_history_with_them`)
  [@test] crates/git-ops/src/squash_commits_test.rs (`a_group_its_folded_commits_cancel_leaves_the_history_only_when_it_says_so`)
  [@test] crates/bd-rewrite/src/fold_test.rs (`a_fixup_that_cancels_its_target_drops_it_and_the_dry_run_says_so`)

### Requirement: Rewrite: The result names a cancelled target without a new id

The result MUST name a target its fixups cancel without a new id, so a queued rewrite that names it finds it gone.

#### Scenario: A cancelled target in the result
- **WHEN** a target leaves the history with its fixups
- **THEN** the result names it without a new id, and a queued rewrite that names it finds it gone
  [@test] crates/history-rewrite/src/autosquash_test.rs (`a_target_its_fixups_cancel_leaves_the_history_with_them`)

### Requirement: Rewrite: A root target its fixups cancel is refused

A root target its fixups cancel MUST be refused (`squash_empty`): no history would be left below the later commits.

#### Scenario: The root commit cancelled
- **WHEN** the fixups of the root commit cancel it
- **THEN** the squash is refused, `squash_empty`
  [@test] crates/git-ops/src/squash_commits_test.rs (`a_group_its_folded_commits_cancel_leaves_the_history_only_when_it_says_so`)

### Requirement: UI: A branch with fixups shows the "N fixups" badge

After a sync that found fixup commits, the plan MUST be loaded once; a branch with fixups MUST show a muted "N fixups"
badge next to its name. Orphans MUST NOT be counted.

#### Scenario: The badge of a branch
- **WHEN** a sync found fixup commits
- **THEN** the plan is loaded once and a branch with fixups shows a muted "N fixups" badge next to its name
- **AND** orphans are not counted
  [@test] src/lib/autosquash.test.ts (`takes the groups by the branch of their target and the orphans by their own branch`)
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from the fixup badge of its row and undo it`)

### Requirement: UI: The badge is a button that opens the confirmation

The badge MUST be a button whose tooltip says "Squash N Fixups…": its press opens the confirmation, the commits
collapsed or not, and MUST neither select nor expand the row.

#### Scenario: The press of the badge
- **WHEN** the user hovers the badge, then presses it
- **THEN** its tooltip says "Squash N Fixups…" and its press opens the confirmation
- **AND** the row is neither selected nor expanded by it
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from the fixup badge of its row and undo it`)

#### Scenario: The badge of a collapsed row
- **GIVEN** the commits of the branch are collapsed
- **WHEN** the user presses the badge
- **THEN** it opens the confirmation as well
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should keep every row in its place when the fixup of the branch that began first is squashed`)

### Requirement: UI: A fixup commit row shows its marker

A fixup commit row MUST show its marker, muted (warning colour for an orphan), with a tooltip "Squashes into “X”" or
"No target in this range".

#### Scenario: A fixup with a target
- **WHEN** the user hovers the marker of a fixup with a target
- **THEN** the marker is muted and its tooltip says "Squashes into “X”"
  [@test] src/lib/autosquash.test.ts (`names the target of a fixup, or none for an orphan`)
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from the fixup badge of its row and undo it`)

#### Scenario: An orphan
- **WHEN** the user hovers the marker of an orphan
- **THEN** the marker has the warning colour and its tooltip says "No target in this range"
  [@test] src/lib/autosquash.test.ts (`names the target of a fixup, or none for an orphan`)
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from the fixup badge of its row and undo it`)

### Requirement: UI: "Squash N Fixups…" in the unassigned card header and the badge

"Squash N Fixups…" ("Squash 1 Fixup…", ghost `xs`, `formatSquashFixupsLabel`) MUST appear in the unassigned card
header, and as the tooltip of the badge of a branch; it opens the confirmation, hence the ellipsis.

#### Scenario: The unassigned card header and the badge
- **WHEN** fixups are to be squashed among the unassigned commits or in a branch
- **THEN** "Squash N Fixups…" ("Squash 1 Fixup…") appears in the unassigned card header, and as the tooltip of the
  badge of the branch
- **AND** it opens the confirmation, hence the ellipsis
  [@test] src/lib/autosquash.test.ts (`names the squash of the fixups alike on the button and in the branch menu, which open the confirmation`)

### Requirement: UI: No line above the commits of a branch holds the squash

A line above the commits of a branch MUST NOT hold "Squash N Fixups…".

#### Scenario: An expanded branch with fixups
- **WHEN** a branch with fixups is expanded
- **THEN** no line above its commits holds "Squash N Fixups…"

### Requirement: UI: The branch menu offers the squash in a "History" group

The branch menu MUST have the same item, named alike, in a "History" group, which opens the confirmation at the badge of
the row and leaves its commits as they are.

#### Scenario: The squash from the branch menu
- **WHEN** the user chooses "Squash N Fixups…" in the "History" group of the branch menu
- **THEN** the confirmation opens at the badge of the row and its commits are left as they are
  [@test] src/lib/autosquash.test.ts (`names the squash of the fixups alike on the button and in the branch menu, which open the confirmation`)
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should squash from the branch menu and one fixup from the commit menu`)

### Requirement: UI: The confirmation lists each target with its fixups, then the orphans

The confirmation popover MUST read the plan again and list each target with its fixups, then the orphans under "Won't
be squashed: no target in this range".

#### Scenario: The confirmation opens
- **WHEN** the confirmation opens
- **THEN** it reads the plan again and lists each target with its fixups
- **AND** the orphans follow under "Won't be squashed: no target in this range"
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from the fixup badge of its row and undo it`)

### Requirement: UI: Squash runs it, Cancel, Escape or a click outside closes it

Squash MUST run the squash; Cancel, Escape or a click outside MUST close the confirmation.

#### Scenario: Squash
- **WHEN** the user presses Squash
- **THEN** the squash runs

#### Scenario: Cancel, Escape or a click outside
- **WHEN** the user presses Cancel or Escape, or clicks outside
- **THEN** the confirmation closes

### Requirement: UI: Nothing closes the confirmation while the squash runs

Nothing MUST close the confirmation while the squash runs (as the squash dialog): it closes when the squash is done or
failed.

#### Scenario: The squash runs
- **WHEN** the squash runs
- **THEN** nothing closes the confirmation
- **AND** it closes when the squash is done or failed

### Requirement: UI: The confirmation says what the squash does in the number of the fixups

The title of the confirmation MUST be "Squash N fixups" ("Squash 1 fixup"), and the sentence under it MUST say in the
number of the fixups what the squash does: "It is folded into its target, as with rebase --autosquash. A target it
cancels leaves the history; your files are not touched." for one, "Each one is folded" and "A target they cancel"
in it for more; with nothing to squash the title MUST be "No fixups to squash", without such a sentence.

#### Scenario: One fixup
- **WHEN** the confirmation of one fixup opens
- **THEN** its title is "Squash 1 fixup" and it says "It is folded into its target, as with rebase --autosquash. A
  target it cancels leaves the history; your files are not touched."
  [@test] src/lib/autosquash.test.ts (`says what the squash does in the number of its fixups, one as one`)
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should keep every row in its place when the fixup of the branch that began first is squashed`)

#### Scenario: More fixups
- **WHEN** the confirmation of more fixups opens
- **THEN** its title is "Squash N fixups" and it says "Each one is folded into its target, as with rebase
  --autosquash. A target they cancel leaves the history; your files are not touched."
  [@test] src/lib/autosquash.test.ts (`says what the squash does in the number of its fixups, one as one`)

#### Scenario: No fixups
- **WHEN** the confirmation opens with nothing to squash
- **THEN** its title is "No fixups to squash" and no such sentence follows it
  [@test] src/lib/autosquash.test.ts (`says what the squash does in the number of its fixups, one as one`)

### Requirement: UI: The menu of a fixup offers "Squash into “X”"

The menu of a fixup commit with a target MUST offer "Squash into “X”", which squashes that fixup only.

#### Scenario: Squash one fixup
- **WHEN** the user chooses "Squash into “X”" in the menu of a fixup commit with a target
- **THEN** that fixup only is squashed
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should squash from the branch menu and one fixup from the commit menu`)

### Requirement: UI: Success shows "Squashed N fixups" with Undo

Success MUST show "Squashed N fixups" with Undo, as for a squash of selected commits (the rewrite `undo` of its
`squash` token).

#### Scenario: Squashed, then undone
- **WHEN** the squash succeeds
- **THEN** the toast says "Squashed N fixups" with Undo
- **AND** Undo runs the rewrite `undo` of its `squash` token, as for a squash of selected commits
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from the fixup badge of its row and undo it`)

### Requirement: UI: The toast names a target that left the history

A target that left the history MUST be named in the description of the toast: "“X” left the history: its fixups
cancel it" ("“X” and “Y” left the history: their fixups cancel them").

#### Scenario: A target its fixups cancel
- **WHEN** the squash takes a target out of the history
- **THEN** the description says "“X” left the history: its fixups cancel it" ("“X” and “Y” left the history: their
  fixups cancel them")
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should drop a target its fixup cancels, say so in the toast and bring it back with Undo`)
  [@test] src/lib/autosquash.test.ts (`names the targets their fixups cancel, none when every target stays`)

## Testing / Local Run

- `pnpm test:backend`: `autosquash_test.rs`, `commit_grouper_test.rs`.
- `pnpm test:unit`: `src/lib/autosquash.test.ts`.
- `pnpm e2e`: `tests/e2e/squash-fixups` uses the `fixups` template, and `fixup_order` for the order of the rows.

## Open Questions / Risks

- git resolves a hash prefix against the whole repository; Branch Deck only against the commits of the range, so a
  prefix that git finds ambiguous may still match here.
- The badges use the plan of the last sync; the confirmation and the squash always read it again.

## References

- [Squash Commits](../squash/spec.md)
- [Git Operations](../../../../docs/design/git-operations.md)
- `git help rebase` (`--autosquash`), `git help commit` (`--fixup`)
