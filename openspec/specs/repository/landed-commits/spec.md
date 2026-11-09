---
name: Landed Commits
description: Requirements for telling the user that their commits reached the baseline (a Safe Push robot, a merged pull request, a cherry-pick): before the update the header and the rows name what the fetched baseline already has, the update toast leads with what landed, and an island keeps the report until the user has seen it or dismissed it, also for an update made outside Branch Deck; Activity keeps it after.
style: plain-1
targets:
  - crates/domain-types/src/landed.rs
  - crates/domain-types/src/upstream_commit.rs
  - crates/sync-core/src/landed.rs
  - crates/domain-policy/src/landed.rs
  - crates/git-ops/src/landed_pairing.rs
  - crates/update-core/src/update_project.rs
  - crates/sync-core/src/sync.rs
  - crates/git-ops/src/update_branch.rs
  - crates/bd-core/src/sync_command.rs
  - crates/operations/src/update.rs
  - src/lib/landed.ts
  - src/lib/landing-toast.ts
  - src/lib/event-toast.ts
  - src/stores/landed.ts
  - src/lib/landed-run.ts
  - src/stores/update-project.ts
  - src/stores/branch-sync.ts
  - src/components/repository/LandedCard.tsx
  - src/lib/review-window.ts
  - src/components/repository/ActivityLog.tsx
  - src/stores/settings/repositorySettings.ts
  - src/components/repository/UpdateProjectButton.tsx
  - src/components/branch/LandedChip.tsx
  - src/hooks/useLandedIn.ts
  - src/presenters/row-status.ts
  - src/presenters/push-button.ts
  - src/hooks/useBranchPushes.ts
  - crates/domain-policy/src/push.rs
  - src/components/shared/CopyButton.tsx
  - src/components/branch/BranchRow.tsx
  - src/components/branch/BranchCreator.tsx
  - src/components/commit/CommitList.tsx
  - src/model/intents.ts
---

# Landed Commits

Status: Implemented
Date: 2026-10-14

## Purpose

A Safe Push robot rebases the commits of a virtual branch onto `master`; the next Update Project drops them locally.
Branch Deck says so at three moments: before the update, the header and the branch rows name what the fetched baseline
already has; the update toast leads with what landed; afterwards an island lists it, with the baseline commits and the
robot or pull request, until the user has seen it or dismisses it, and Activity keeps the report. An update made elsewhere (the IDE, `git pull --rebase`) is reported
by the next sync the same way.

## Goals

- Landing is the user's news: never hide it behind "rebased 116 commits" or a silent move into the archive.
- Name the user's own branches and commits, with the baseline commit that carries each, and part of a branch too.
- Know it whoever updated, and keep it until the user has seen it.

## Non-goals

- No count of the upstream's commits; only the user's own landed commits are named.
- No notification of its own: the moment a robot merges is the Patronus toast (`openspec/specs/branch/patronus/spec.md`), and the
  landing is the next state of that toast, never a second toast of the same landing.

## Requirements

### Requirement: Detection: A commit landed when the baseline has a commit of its key

A local commit MUST count as landed when the baseline since the merge base has a commit with the same author email,
author time and subject without the `(prefix)`.

#### Scenario: The baseline has the commits under the same author, time and subject
- **GIVEN** a robot rebased the commits of a branch onto the baseline
- **WHEN** the baseline since the merge base has a commit with the same author email, author time and subject without
  the `(prefix)` for each of them
- **THEN** each of those local commits landed
  [@test] crates/update-core/src/landed_test.rs (`whole_branch_landed_by_a_rebase_upstream`)

#### Scenario: The same author and time with another subject
- **WHEN** the baseline has a commit with the same author email and author time but another subject
- **THEN** the local commit has not landed
  [@test] crates/update-core/src/landed_test.rs (`same_author_and_time_with_another_subject_is_not_landed`)

### Requirement: Detection: The landed commit names its baseline commit

The landed commit MUST name the baseline commit that matched it, with its author, committer and time.

#### Scenario: A landed commit names the robot's commit
- **WHEN** a local commit landed
- **THEN** it names its baseline commit with that commit's author, committer and time
  [@test] crates/update-core/src/landed_test.rs (`whole_branch_landed_by_a_rebase_upstream`)

### Requirement: Detection: Landed commits are grouped by branch

The landed commits MUST be grouped by branch, unassigned ones apart, each branch with the commits that stay local.

#### Scenario: Part of a branch landed
- **WHEN** some commits of a branch landed and the others did not
- **THEN** the branch lists the landed commits with the commits that stay local
  [@test] crates/update-core/src/landed_test.rs (`part_of_a_branch_landed`)

#### Scenario: Commits without a branch
- **WHEN** unassigned commits landed
- **THEN** they are grouped apart from the branches

### Requirement: Detection: A fixup landed with its target once the replay drops it

A fixup whose target landed MUST count as landed with it once the replay drops it.

#### Scenario: The robot folded the fixup into its target
- **GIVEN** the target of a fixup landed and the robot folded the fixup into it
- **WHEN** the replay drops the fixup
- **THEN** the fixup landed with its target
  [@test] crates/update-core/src/landed_test.rs (`fixups_folded_upstream_landed_with_their_target`)

#### Scenario: A fixup made after the push
- **GIVEN** the target of a fixup landed and the robot folded the fixup pushed with it
- **WHEN** another fixup of it was made after the push
- **THEN** the replay drops the folded fixup, which landed with its target
- **AND** the replay keeps the fixup made after the push: it is not reported as landed and stays local
  [@test] crates/update-core/src/landed_test.rs (`fixup_made_after_the_push_stays_local`)

### Requirement: Detection: Without a replay a fixup counts only by its own key

Without a replay (a conflict) a fixup MUST count as landed only by a key match of its own (or its proof, "The proof"),
never with its target, so a fixup made after the push MUST NOT be reported as landed.

#### Scenario: The replay conflicts
- **GIVEN** the target of a fixup landed by its key, and the replay conflicts
- **WHEN** the baseline has no commit of the fixup's own key
- **THEN** the fixup is not reported as landed: it counts among the commits that stay local

### Requirement: Detection: A commit the replay drops landed without a counterpart

Where the replay onto the upstream is known, a commit it drops MUST count as landed even without a counterpart
(squashed upstream, no baseline commit).

#### Scenario: A branch squashed upstream
- **GIVEN** the upstream has the commits of a branch squashed into one commit of another key
- **WHEN** the replay onto the upstream drops them
- **THEN** they landed, squashed, without a baseline commit
  [@test] crates/update-core/src/landed_test.rs (`branch_squashed_upstream_landed_without_an_upstream_commit`)

### Requirement: Detection: A conflicting update still reports the key matches

A conflicting update MUST still report the key matches.

#### Scenario: The update conflicts
- **GIVEN** a commit landed by key
- **WHEN** the update of another commit conflicts
- **THEN** the landed commit is still reported
  [@test] crates/update-core/src/landed_test.rs (`landed_commits_are_reported_while_the_update_conflicts`)

### Requirement: The proof: A key match is a candidate, never a proof

A key match MUST be a candidate, never a proof. What no rung of the proof proves MUST stay a candidate.

#### Scenario: A commit amended after the push
- **GIVEN** a local commit was amended after the push, and a fixup of it made
- **WHEN** the baseline has a commit of its key
- **THEN** no rung proves the pair, and the commit stays a candidate, with its fixups
  [@test] crates/git-ops/src/landed_pairing_test.rs (`an_amend_after_the_push_is_a_candidate_with_its_fixups`)

### Requirement: The proof: A twin is proven by a ladder before the replay

Before the replay, the check MUST pair each local commit with the baseline commit of its key (a twin) and prove the
pair by a ladder: the same patch id with context (`patch_id`, as `git rebase` compares); else a cherry-pick of the
local commit onto the baseline commit's own parent that writes that commit's tree (`replayed`); else the same changed
lines without context, paths, statuses and modes, with the same new object of each binary file and gitlink
(`changed_lines`).

#### Scenario: A twin of the same patch
- **WHEN** a twin has the same patch id with context
- **THEN** the pair is proven before the replay, `patch_id`
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_twin_of_the_same_patch_is_proven_by_its_patch_id_before_the_replay`)

#### Scenario: A commit without a twin
- **WHEN** no baseline commit has the key of a local commit
- **THEN** the commit stays unpaired, and the pairing of the twins runs no process
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_pairing_of_the_twins_runs_no_process_without_a_twin`)

#### Scenario: A twin on other context
- **WHEN** the context lines of a twin differ, as twins that landed in another order
- **THEN** the cherry-pick of the local commit onto the baseline commit's own parent writes that commit's tree, and the
  pair is proven, `replayed`
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_twin_with_other_context_is_proven_by_its_replay`)
  [@test] crates/git-ops/src/landed_pairing_test.rs (`twins_landed_in_the_other_order_are_both_proven_by_their_replay`)

#### Scenario: A twin whose cherry-pick conflicts
- **WHEN** the cherry-pick conflicts on the line next to the change
- **THEN** the same changed lines without context, paths, statuses and modes prove the pair, `changed_lines`
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_twin_whose_cherry_pick_conflicts_is_proven_by_its_changed_lines`)

#### Scenario: Equal changed lines beside a chmod or another binary edit
- **WHEN** the changed lines are equal but the mode of a file differs, or a binary file gets another new object
- **THEN** the changed lines prove nothing and the twin stays a candidate
  [@test] crates/git-ops/src/landed_pairing_test.rs (`equal_changed_lines_beside_a_chmod_or_a_binary_edit_prove_nothing`)

### Requirement: The proof: The same patch of the author under another key is paired

Before the replay too, the check MUST pair the local commits of the same patch id under another key by the same author
(`patch_id`: a reworded one, one landed at another author time, the squash of a pull request of one commit), among the
baseline commits of that author with the same set of changed paths, as git compares.

#### Scenario: The same patch under another key of its author
- **WHEN** a baseline commit of the same author has the same patch id under another key
- **THEN** the pair is proven before the replay, `patch_id`
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_patch_under_another_identity_of_its_author_is_proven_before_the_replay`)

#### Scenario: Only among the author's sets of paths
- **WHEN** no baseline commit of the author has the same set of changed paths
- **THEN** no patch id is compared and the commit stays unpaired
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_patch_of_the_author_is_looked_for_only_among_its_sets_of_paths`)

#### Scenario: The same lines under another key
- **WHEN** a baseline commit under another key has the same changed lines but another patch id
- **THEN** the local commit pairs with nothing
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_lines_under_another_identity_pair_with_nothing`)

### Requirement: The proof: The same patch by another author is proven only after a conflict

When the replay conflicts, the check MUST also pair the local commits of the same patch id by another author, once:
that walk of the whole baseline costs seconds in a monorepo.

#### Scenario: The same patch by another author
- **WHEN** a baseline commit of another author has the same patch id as a local commit
- **THEN** the pair is proven only after the replay conflicts, and not before it
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_patch_by_another_author_is_proven_only_after_a_conflict`)

### Requirement: The proof: The pairing is one to one

The pairing MUST be one to one: each baseline commit proves at most one local commit, the oldest first. Merge commits
MUST NOT pair.

#### Scenario: A change made twice locally with one copy upstream
- **GIVEN** a change made twice locally (A, a revert of A, A again) with one copy upstream
- **WHEN** the check pairs them
- **THEN** only the first A is left out
  [@test] crates/git-ops/src/landed_pairing_test.rs (`one_upstream_commit_proves_one_local_commit`)

#### Scenario: A merge commit
- **WHEN** a merge commit is on either side
- **THEN** it never pairs

### Requirement: The proof: A proven commit is left out of the replay and landed

A proven commit MUST be left out of the replay and landed as its baseline commit, also when the baseline reworded it.

#### Scenario: Twins in another order
- **WHEN** the update proves twins that landed in another order
- **THEN** they are left out of the replay by their proof and land as their baseline commits
  [@test] crates/bd-update/src/resolve_test.rs (`twins_in_another_order_are_left_out_by_their_proof_and_land`)

### Requirement: The proof: A local fixup of a proven commit is no candidate

A local fixup of a proven commit MUST NOT be a candidate: it replays on its own.

#### Scenario: A fixup of a proven twin
- **GIVEN** a local fixup of a twin that the replay proves
- **WHEN** the check pairs the commits
- **THEN** the fixup is no candidate and stays out of the pairing
  [@test] crates/git-ops/src/landed_pairing_test.rs (`a_fixup_of_a_proven_twin_stays_out_of_the_pairing`)

### Requirement: The proof: A candidate is landed only by a decision

A candidate MUST be landed only by a decision: `brd update --continue --landed` in its round drops it as its baseline
commit, at once when the baseline commit has every line it changes, else after the user's yes (`--user`).

#### Scenario: A twin that differs
- **WHEN** the baseline commit lacks a line the candidate changes
- **THEN** `--landed` asks the user and drops it as its baseline commit only after the yes (`--user`)
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_that_differs_asks_the_user_and_drops_after_the_yes`)

#### Scenario: A twin with every line of the commit
- **WHEN** the baseline commit has every line the candidate changes
- **THEN** `--landed` drops it as its baseline commit at once, without a question
  [@test] crates/bd-update/src/resolve_test.rs (`a_twin_with_every_line_of_the_commit_drops_without_a_question`)

### Requirement: The proof: A drop by --drop-empty lands only a candidate

A drop by `--drop-empty` MUST land only a candidate; any other is the user's drop and MUST NOT count as landed.

#### Scenario: An empty resolution of a commit without a counterpart
- **WHEN** the agent drops an empty resolution of a commit that is no candidate with `--drop-empty`
- **THEN** it is the user's drop, and the commit did not land
  [@test] crates/bd-update/src/resolve_test.rs (`a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it`)

### Requirement: The proof: A conflicting update pairs once

The check of a conflicting update MUST pair once: the session replays the plan of the check and pairs nothing again.

#### Scenario: The update conflicts
- **WHEN** an update conflicts and its session starts
- **THEN** the session replays the plan of the check, and the commits are paired once
  [@test] crates/update-core/src/process_budget_test.rs (`a_conflicting_update_pairs_its_commits_once`)

### Requirement: The proof: Detection runs a fixed set of read-only processes

Detection MUST cost three read-only `log` processes, one more for a proven commit of another author, and one
`diff-tree --name-only` of the local commits and the baseline commits of their authors. The check inside a sync MUST
reuse them for an unchanged state, and a failure MUST NOT fail the check or the update.

#### Scenario: The processes of a detection
- **WHEN** the check detects what landed
- **THEN** it runs three read-only `log` processes, one more for a proven commit of another author, and one
  `diff-tree --name-only`

#### Scenario: One diff-tree for the paths of the commits
- **WHEN** the check looks for the same patch of an author under another key
- **THEN** it reads the changed paths of the local commits and the baseline commits of their authors in one
  `diff-tree` process
  [@test] crates/git-ops/src/landed_pairing_test.rs (`the_same_patch_of_the_author_is_looked_for_only_among_its_sets_of_paths`)

#### Scenario: A sync of an unchanged state
- **WHEN** the check inside a sync runs again for an unchanged state
- **THEN** it reuses what it read

#### Scenario: Detection fails
- **WHEN** the detection fails
- **THEN** the check and the update do not fail

### Requirement: Update: The replay leaves out the commits proven before it

The check MUST replay without the commits proven before the replay ("The proof": the twins and the same patch of their
author under another key), as `git rebase` leaves out a commit of the same patch id and as a drop does (a run of kept
commits per replay): a clean merge of such a commit onto a baseline that changed the lines around it would add its
lines a second time.

#### Scenario: A twin of the same patch
- **GIVEN** the baseline changed the lines around a twin of the same patch id
- **WHEN** the check replays
- **THEN** the twin is left out of the clean replay, which would add its lines again
  [@test] crates/update-core/src/landed_test.rs (`a_twin_of_the_same_patch_is_left_out_of_a_clean_replay_that_would_add_its_lines_again`)

#### Scenario: A twin on other context
- **WHEN** a twin on other context is proven by its replay
- **THEN** it is left out of the clean replay
  [@test] crates/update-core/src/landed_test.rs (`a_twin_on_other_context_is_left_out_of_a_clean_replay_by_its_replay`)

#### Scenario: The same patch under another key
- **WHEN** a local commit has the same patch as a baseline commit of its author under another key
- **THEN** it is left out of the clean replay, which would add its lines again
  [@test] crates/update-core/src/landed_test.rs (`the_same_patch_under_another_key_is_left_out_of_a_clean_replay_that_would_add_its_lines_again`)

### Requirement: Update: A conflicting replay leaves out the same patch by another author

When the replay conflicts, the check MUST replay again without the commits of the same patch id by another author too.

#### Scenario: The replay conflicts
- **WHEN** the replay without the proven commits conflicts
- **THEN** the check replays again without the commits of the same patch id by another author too

### Requirement: Update: A replay that conflicts again leaves out the candidates

If the replay without the commits of the same patch id by another author conflicts too, the check MUST leave out the
candidates together with their fixups, then the candidates alone.

#### Scenario: The robot folded the fixups into their target
- **GIVEN** a robot folded the fixups of a candidate into their target
- **WHEN** the replay conflicts
- **THEN** the check leaves out the candidate together with its fixups, and the update no longer conflicts
  [@test] crates/update-core/src/landed_test.rs (`fixups_folded_upstream_landed_with_their_target`)

### Requirement: Update: A result without the candidates counts only if it keeps their change

A result that leaves out candidates MUST count only if it keeps the change of each candidate: the merge of the change
from the candidate's parent to its last local fixup into the result gives the result's tree. So a commit amended after
the push, or a fixup made after it, MUST NOT be left out; the update then conflicts as before.

#### Scenario: A fixup made after the push
- **WHEN** a fixup of a candidate was made after the push
- **THEN** it is never left out
  [@test] crates/update-core/src/landed_test.rs (`fixup_made_after_the_push_stays_local`)

#### Scenario: A commit amended after the push
- **WHEN** a candidate was amended after the push
- **THEN** it is never left out, and the update conflicts as before
  [@test] crates/update-core/src/landed_test.rs (`commit_amended_after_the_push_is_never_left_out`)

### Requirement: Update: The preview of a sync decides as the update does

The preview of every sync and the update MUST make the same decision, so the header shows such an update as clean.

#### Scenario: An update clean by leaving out landed commits
- **WHEN** an update is clean only by leaving out the proven commits or the candidates
- **THEN** the preview of the sync makes the same decision and the header shows the update as clean

### Requirement: Report: An applied update adds what it landed to the report

An applied update MUST add what it landed to the report in `branch-deck/landed.json` of the worktree's git directory
and return it; the next sync MUST NOT add it again.

#### Scenario: The update applies
- **WHEN** an update that landed commits applies
- **THEN** it returns what landed and adds it to the report in `branch-deck/landed.json` of the worktree's git directory
- **AND** the sync after it does not add it again
  [@test] crates/update-core/src/landed_test.rs (`whole_branch_landed_by_a_rebase_upstream`)

### Requirement: Report: An update onto a local branch adds nothing

An update onto a local branch (the update of a lease onto its home after a conflict of `brd lease pick`) MUST add
nothing, at its apply or at the finish of its session: what the local branch has did not reach the baseline by it.

#### Scenario: The update of a lease onto its home
- **WHEN** a lease is updated onto its home branch after a conflict of `brd lease pick`
- **THEN** nothing is added to the report, at its apply or at the finish of its session
  [@test] crates/update-core/src/update_project_test.rs (`an_update_onto_a_local_branch_moves_a_detached_head_and_its_undo_moves_it_back`)

### Requirement: Report: A sync reports an update made elsewhere once

Each sync MUST remember where the branch is; when the merge base moved since the previous sync, the commits that were
local then and match the upstream now MUST be added, once.

#### Scenario: An update made elsewhere
- **GIVEN** the merge base moved since the previous sync, by an update made outside Branch Deck
- **WHEN** the next sync runs
- **THEN** the commits that were local then and match the upstream now are added to the report, once
  [@test] crates/update-core/src/landed_test.rs (`update_made_elsewhere_is_reported_by_the_next_sync_once`)

### Requirement: Report: A rewrite between two syncs reports nothing

Amend, reword, squash and drop between two syncs MUST report nothing.

#### Scenario: Amend, reword, squash or drop between syncs
- **WHEN** a commit is amended, reworded, squashed or dropped between two syncs
- **THEN** nothing is reported as landed
  [@test] crates/update-core/src/landed_test.rs (`reword_squash_or_amend_between_syncs_is_not_landed`)

### Requirement: Report: The sync sends the landing and the report

The sync MUST send the landing (`BaselineStatus.landing`) and the undismissed report (`BaselineStatus.landed`).

#### Scenario: A sync with a landing and a report
- **WHEN** the fetched baseline has local commits and an undismissed report exists
- **THEN** the sync sends the landing (`BaselineStatus.landing`) and the report (`BaselineStatus.landed`)
  [@test] crates/update-core/src/landed_test.rs (`sync_sends_the_landing_and_the_report`)

### Requirement: Report: Dismiss clears the report everywhere

Dismiss MUST clear the report for every window and `brd`, as an operation of the session (`Intent::DismissLanded`,
under the lock of the report).

#### Scenario: Dismiss of the report
- **WHEN** the user dismisses the report
- **THEN** the report of what an update landed is cleared for every window and `brd`
  [@test] crates/operations/src/update_test.rs (`dismiss_landed_clears_the_report_of_what_an_update_landed`)

### Requirement: Report: A report is kept at most 7 days

A report MUST be kept at most 7 days.

#### Scenario: An old report
- **WHEN** a report is older than 7 days
- **THEN** it is no longer kept

### Requirement: Report: A new report for a known branch merges into it

A new report for a known branch MUST add its new commits and take the new count of the ones that stay.

#### Scenario: More of a known branch landed
- **GIVEN** the report has a branch
- **WHEN** a new report for that branch arrives
- **THEN** its new commits are added and the count of the commits that stay local is the new one
  [@test] src/lib/landed.test.ts (`adds new commits to a known branch and takes its new count of local commits`)

### Requirement: Report: brd sync prints the landing and the report

`brd sync` MUST print the landing as `update.landing` and the report as `landed`, with short ids.

#### Scenario: brd sync with landed commits
- **WHEN** `brd sync` runs while the baseline already has local commits
- **THEN** it prints the landing as `update.landing` and the report as `landed`, with short ids
  [@test] crates/bd-cli/src/sync_test.rs (`reports_the_commits_the_baseline_already_has`)

### Requirement: Wording: Each number has its verb on every surface

Every surface (the Update tooltip, the footer of the landing overview, the toast, the island, the chip) MUST say it in
one grammar: each number has its verb, never a ratio like "3 of 12", which hides the number that landed.

#### Scenario: Part of a branch landed
- **WHEN** a surface names a part of a branch that landed
- **THEN** it reads "3 commits of IJPL-257337 landed · 9 stay local", never "3 of 12"
- **AND** what stays is "1 stays local" / "9 stay local" ("9 local" on the chip)
  [@test] src/lib/landed.test.ts (`gives each number its verb`)

### Requirement: Wording: A commit subject shows as in the commit rows

A commit subject MUST show its issue links and its dimmed conventional-commit prefix, as in the commit rows: in the
island, in the popover of the chip.

#### Scenario: The popover of the chip
- **WHEN** the popover of the chip of a branch lists its landed commits before the update
- **THEN** each subject shows its issue links and its dimmed conventional-commit prefix
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

#### Scenario: The island
- **WHEN** the island lists the landed commits after the update
- **THEN** each subject shows its issue links and its dimmed conventional-commit prefix
  [@test] tests/e2e/update-project/landed.spec.ts (`should lead the toast with what landed and keep the island until it is dismissed`)

### Requirement: Before the update: The Update tooltip adds what the baseline brings in

The tooltip of the Update button MUST add what the baseline brings in.

#### Scenario: The baseline brings in a branch and a part of another
- **WHEN** the fetched baseline has all of `feature-auth` and one commit of `search`
- **THEN** the tooltip reads "Rebases cleanly onto origin/master · brings in your feature-auth and 1 commit of search"
  [@test] src/lib/landed.test.ts (`names the user's branches, a part as a count of the branch`)
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

### Requirement: Before the update: A clean update shows the info package

A clean update that brings in the user's commits MUST show the `info` package instead of the check, on the button and
in the footer line of the landing overview.

#### Scenario: A clean update brings in landed commits
- **WHEN** the update is clean and the fetched baseline has some of the user's commits
- **THEN** the Update button and the footer line of the landing overview show the `info` package instead of the check
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

### Requirement: Before the update: The footer line says it in a few words

The footer line of the landing overview MUST say what landed in a few words.

#### Scenario: The footer line
- **WHEN** the fetched baseline has some of the user's commits
- **THEN** the footer line says "your wsm landed", "40 of your commits landed" (a part of a branch, or commits without
  one), "your 2 branches landed"
  [@test] src/lib/landed.test.ts (`says it in a few words for the footer`)

### Requirement: Before the update: The branch row shows a Landed chip

The branch row MUST show an `info` chip "Landed" (all its commits) or "1 landed · 1 local" in the third column of its
Status cell ([Branch Row](../../branch/branch-row/spec.md)).

#### Scenario: All or part of a branch landed
- **WHEN** the fetched baseline has all of a branch
- **THEN** its row shows the chip "Landed"
- **AND** a branch the baseline has a part of shows "1 landed · 1 local"
  [@test] src/lib/landed.test.ts (`says it in fewer words on the chip, "Landed" when the baseline has all of the branch`)
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

### Requirement: Before the update: The popover of the chip has the anatomy of the details

The popover of the chip MUST have the anatomy of the pull request and Patronus details: the state line (the chip
text), "Already in origin/master · the update removes the local copies", then the rows "Commits" (each subject with
its baseline commit) and "Stays local" ("1 commit").

#### Scenario: The popover of the chip
- **WHEN** the user opens the popover of the chip
- **THEN** it shows the state line, "Already in origin/master · the update removes the local copies"
- **AND** the rows "Commits", each subject with its baseline commit, and "Stays local" ("1 commit")
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

### Requirement: Before the update: A landed commit says it is in the baseline

A landed commit MUST show "in origin/master" after its subject; its popover says "Already in origin/master · the
update removes the local copy" and has the row "Landed as".

#### Scenario: A landed commit row
- **WHEN** the fetched baseline has a commit of the branch
- **THEN** its row shows "in origin/master" after its subject
- **AND** its popover says "Already in origin/master · the update removes the local copy" and has the row "Landed as"
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

### Requirement: Before the update: A fully landed branch has nothing left to push

A branch the fetched baseline has all of MUST have nothing left to push: the update archives it. A part that landed
MUST change nothing of this: the branch still has commits to push.

#### Scenario: The baseline has all of the branch
- **WHEN** the fetched baseline has all of a branch
- **THEN** the branch has nothing left to push
  [@test] src/lib/landed.test.ts (`has nothing to push for a branch the fetched baseline has all of, before the update`)

#### Scenario: A part of the branch landed
- **WHEN** the fetched baseline has a part of a branch
- **THEN** the branch still has commits to push
  [@test] src/lib/landed.test.ts (`has nothing to push for a branch the fetched baseline has all of, before the update`)

### Requirement: Before the update: The status of a fully landed branch reads landed

The status of a branch the fetched baseline has all of MUST read "landed" after an `info` dot, its push button MUST
keep its label (`Push Branch (1)`) but be neutral, and both descriptions say "Already in origin/master. Update to
archive the branch.".

#### Scenario: The status cell
- **WHEN** the fetched baseline has all of a branch
- **THEN** its status reads "landed" after an `info` dot, whatever its push state
- **AND** its description says "Already in origin/master. Update to archive the branch."
  [@test] src/presenters/row-status.test.ts (`says landed, in the merged hue, for a branch the baseline has all of: its push state no longer matters`)
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

#### Scenario: The push button
- **WHEN** the fetched baseline has all of a branch
- **THEN** its push button keeps its label (`Push Branch (1)`) but is neutral
- **AND** its description says "Already in origin/master. Update to archive the branch."
  [@test] src/presenters/push-button.test.ts (`is quiet for a branch the baseline has all of: the update archives it`)

### Requirement: Before the update: A fully landed branch is not outgoing

A branch the fetched baseline has all of MUST NOT be outgoing, so a push of the stack leaves it out
([Outgoing Commits](../outgoing/spec.md#what-is-outgoing), `domain_policy::push::push_plan`).

#### Scenario: A push of the stack with a landed branch
- **WHEN** the user pushes a stack one of whose branches landed
- **THEN** the push leaves that branch out
  [@test] crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)

### Requirement: Before the update: Landed is the first answer of what landed a branch

A branch the fetched baseline has all of MUST be landed, the first answer of what landed it
(`domain_policy::landed::landed_state`, the view's `landedStates`, [Next Step](../../branch/next-step/spec.md#the-table),
row 1).

#### Scenario: The baseline before merged Safe Pushes and the pull request
- **WHEN** the fetched baseline has all of a branch, whatever its Safe Pushes and its pull request say
- **THEN** the branch is landed by the baseline
  [@test] crates/domain-policy/src/landed_test.rs (`what_landed_a_branch_the_baseline_first_then_merged_safe_pushes_then_its_pull_request`)

### Requirement: Before the update: A landed branch has no Safe Push

The Safe Push and Dry Run of a landed branch MUST be disabled with "Landed in origin/master", and `brd safe-push` MUST
refuse it as `landed` (a whole stack: every branch of it, a whole stack being landed in the baseline only when the
fetched baseline has all of every branch of it).

#### Scenario: Safe Push of a landed branch
- **WHEN** the fetched baseline has all of the branch
- **THEN** its Safe Push and Dry Run are disabled with "Landed in origin/master"
- **AND** `brd safe-push` refuses it as `landed`
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A whole stack
- **WHEN** the fetched baseline has all of every branch of a whole stack
- **THEN** the whole stack is landed in origin/master, and `brd safe-push` refuses it as `landed`
  [@test] crates/domain-policy/src/landed_test.rs (`a_whole_stack_landed_when_every_branch_of_it_did`)

### Requirement: After the update: The toast leads with what landed

The toast MUST lead with what landed, then say how the branch was updated; ", squashed" when no commit of the branch
has a counterpart. Without anything landed it MUST stay as the update alone words it.

#### Scenario: Branches and commits landed
- **WHEN** an update landed commits
- **THEN** the toast reads "wsm landed in origin/master" / "Updated master: rebased 116 commits"
- **AND** "3 branches landed in origin/master" / "wsm, IJPL-257337 and IJAI-3828 · rebased 98 commits"
- **AND** "40 commits of IJPL-257337 landed · 12 stay local" / "Updated master: rebased 113 commits"
- **AND** "2 of your commits landed in origin/master" for commits without a branch
- **AND** ", squashed" when no commit of the branch has a counterpart
  [@test] src/lib/landed.test.ts (`leads with what landed, then how the branch was updated`)

#### Scenario: Nothing landed
- **WHEN** an update landed nothing
- **THEN** the toast stays "Updated master: rebased 2 commits onto origin/master", and a fast-forward keeps its wording
  [@test] src/lib/landed.test.ts (`keeps the fast-forward wording`)

### Requirement: After the update: The toast of the update takes the place of the merge toasts

The toast of the update, and the "already up to date" toast, MUST take the place of the open toasts of the merges of
its repository, whose Update Project it did ([One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event)):
in place of the newest, the others closed, one toast of the update whatever started it (the toast's Update Project,
the header, a key).

#### Scenario: What landed, or up to date
- **GIVEN** toasts of merges of the repository are open
- **WHEN** the update they brought in is done, or is "already up to date"
- **THEN** its toast takes the place of the newest of them and the others close
  [@test] src/stores/update-project.test.ts (`takes the place of the toasts of the merges of its repository it brought in: what landed, or up to date`)

#### Scenario: The toast's Update Project is pressed
- **GIVEN** the toast "auth merged into master" with Update Project is open
- **WHEN** the user presses its Update Project
- **THEN** it becomes "auth landed in origin/master, squashed" / "Updated master: rebased 104 commits" in place
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should turn the toast of a merge into what the update landed, in place, when its Update Project is pressed`)

#### Scenario: The update starts from the header
- **GIVEN** the toast of a merge is open
- **WHEN** the update starts from the header
- **THEN** the toast of the merge turns into what the update landed, in place
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should turn the toast of a merge into what the update landed, in place, when the update starts from the header`)

### Requirement: After the update: The toast of what landed has no action

The toast that took the place of a merge toast MUST have no action: the update is done, the island has the rest.

#### Scenario: The merge toast after the update
- **WHEN** the toast of a merge turned into what the update landed
- **THEN** it has no action
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should turn the toast of a merge into what the update landed, in place, when its Update Project is pressed`, `should turn the toast of a merge into what the update landed, in place, when the update starts from the header`)

### Requirement: After the update: A report found by a sync says it in the merge toast

A report a sync found after an update elsewhere (`brd update`, the IDE, `git pull`) MUST have no toast of its own:
the open toast of a merge of its branches says it in place, in the words of its Activity entry (`foundLandedNews`).

#### Scenario: The toast of a merge is open
- **GIVEN** the toast of a merge of the branch is open
- **WHEN** a sync finds what landed after an update elsewhere (`brd update`, the IDE)
- **THEN** the toast says in place "auth landed in origin/master" / "Found by a sync"
  [@test] src/lib/landing-toast.test.ts (`says in place what a sync found landed after an update elsewhere (brd update, the IDE)`)
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should say in the toast of a merge what a sync found landed after an update outside the app`)

#### Scenario: No toast of a merge is open
- **WHEN** a sync finds what landed after an update elsewhere and no toast of a merge is open
- **THEN** nothing shows but the island and Activity

### Requirement: After the update: The Landed island lists what landed

The Landed island MUST sit above the branches: "Landed in origin/master" and when, a row per branch with its count, the
pull request and Patronus chips and the committer; a row expands to the commits, each with the short baseline commit
(copy), its committer and time, or "squashed upstream".

#### Scenario: The island after an update that landed commits
- **WHEN** an update landed commits
- **THEN** the island above the branches says "Landed in origin/master" and when
- **AND** a row per branch says "1 landed", "40 landed · 12 stay local" or "3 landed · squashed", with the pull
  request and Patronus chips and the committer
- **AND** a row expands to the commits, each with the short baseline commit (copy), its committer and time, or
  "squashed upstream"
  [@test] tests/e2e/update-project/landed.spec.ts (`should lead the toast with what landed and keep the island until it is dismissed`)

### Requirement: After the update: The island names one committer once

When every row has the same committer, the header of the island MUST name it once instead.

#### Scenario: Every branch has the same committer
- **WHEN** every row of the island has the same committer
- **THEN** the header names it once instead: "Landed in origin/master · by Space Team · just now"
  [@test] src/lib/landed.test.ts (`names the committer once when every branch has the same one`)

### Requirement: After the update: Dismiss closes the island and clears the report

Dismiss (`XIcon`) MUST close the island and clear the report, the chips and the "New" marks of the archive.

#### Scenario: Dismiss of the island
- **WHEN** the user presses Dismiss on the island
- **THEN** the island closes, and the report, the chips and the "New" marks of the archive are cleared
  [@test] tests/e2e/update-project/landed.spec.ts (`should lead the toast with what landed and keep the island until it is dismissed`)

### Requirement: After the update: D reviews what landed of a branch

D on a branch row of the island and its Review Changes button MUST review what landed of it in the review window as a
list of commits named by the branch ([Review Window](../../review/review-window/spec.md#one-window),
`kind: "commits"`), each commit as the baseline has it, else as it was here (squashed upstream).

#### Scenario: D or Review Changes on a branch row
- **WHEN** the user presses D on a branch row of the island, or its Review Changes button
- **THEN** the review window shows what landed of the branch as a list of commits named by the branch
- **AND** each commit is as the baseline has it, else as it was here (squashed upstream)
  [@test] tests/e2e/update-project/landed.spec.ts (`should review what landed of a branch and of a commit of it from the island, with D and with their buttons`)

### Requirement: After the update: D reviews the list at a landed commit

D on a commit of an expanded row and its button MUST review the list at that commit (`openCommitIdsReview`: the
commits read by `get_review` of the target `commits`).

#### Scenario: D or the button on a commit
- **WHEN** the user presses D on a commit of an expanded row of the island, or its button
- **THEN** the review window shows the list at that commit
  [@test] tests/e2e/update-project/landed.spec.ts (`should review what landed of a branch and of a commit of it from the island, with D and with their buttons`)

### Requirement: After the update: Activity lists the same rows

Activity MUST list the same rows as the island, without that review: no D and no Review Changes button.

#### Scenario: The Activity entry of the report
- **WHEN** the user opens the Activity entry of an update that landed commits
- **THEN** it lists the same rows as the island, without D and the Review Changes buttons
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`)

### Requirement: After the update: The island hides after the user saw it

The island MUST show from the report until the first successful sync that completes while the window has focus, at
least 10 s after the user saw the island (it showed while the window had focus): the sync that follows the update
keeps it, a later one hides what was seen.

#### Scenario: A sync after the island was seen
- **GIVEN** the island showed while the window had focus
- **WHEN** a successful sync completes while the window has focus, less than 10 s after the user saw it
- **THEN** the island stays
- **AND** the first such sync at least 10 s after the user saw it hides what was seen
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)

### Requirement: After the update: Hidden is remembered for the repository

Hidden MUST be remembered for the repository (`landedHiddenUntil` of its `repositoryState`, the newest `landedAt`
seen), so a restart doesn't show it again; what lands later MUST show again.

#### Scenario: The island was hidden
- **WHEN** a sync hid the island
- **THEN** `landedHiddenUntil` of the `repositoryState` of the repository is the newest `landedAt` seen, and a
  restart doesn't show the island again
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)

#### Scenario: More lands later
- **WHEN** something lands after the hidden report
- **THEN** the island shows again

### Requirement: After the update: The chips and New marks stay until Dismiss

The "N landed" chips and the "New" marks MUST stay until Dismiss, which the Activity entry of the report offers once
the island is hidden, and the entry MUST keep the report ([Activity](../activity/spec.md)).

#### Scenario: The island is hidden
- **GIVEN** a sync hid the island
- **WHEN** the user opens the Activity entry of the report, in the window or in the panel
- **THEN** the "N landed" chips still show, and the entry keeps the report and offers Dismiss
- **AND** its Dismiss clears the chips and the entry stays
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)

### Requirement: After the update: The copy button of a baseline commit shows on hover

The copy button of a baseline commit (the full hash) MUST show while its commit row is hovered or the button has
keyboard focus, as the copy button of the commit that merged an archived branch.

#### Scenario: A baseline commit of the island
- **WHEN** the commit row of a baseline commit is hovered, or its copy button has keyboard focus
- **THEN** the copy button of the full hash shows
  [@test] tests/e2e/update-project/landed.spec.ts (`should show the copy button of a baseline commit on hover`)

#### Scenario: The commit that merged an archived branch
- **WHEN** the row of the commit that merged an archived branch is hovered
- **THEN** its copy button shows the same way
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should show the copy button of the merged-by commit on hover`)

### Requirement: After the update: A branch that stays active keeps its chip until Dismiss

A branch that stays active MUST show "1 landed · 1 local" until Dismiss; a fully landed branch MUST move to the
archive, marked "New".

#### Scenario: Part of a branch landed
- **WHEN** an update landed part of a branch
- **THEN** the branch stays active and shows "1 landed · 1 local" until Dismiss
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`)

#### Scenario: All of a branch landed
- **WHEN** an update landed all of a branch
- **THEN** the branch moves to the archive, marked "New"

## User Experience

- `info` (the hue of merged) everywhere, `PackageCheckIcon`; the island is a stock `Card` with a one-time
  `animate-in fade-in-0 slide-in-from-top-2`, no celebration beyond it.
- The chip and the marker are `StatusChip`s without a click action: hover or a click opens the details.

## Testing / Local Run

- `cargo nextest run --workspace`: `crates/update-core/src/landed_test.rs`, the `landed_upstream` template
  in `update_project_test.rs`, `crates/bd-cli/src/sync_test.rs`.
- `pnpm test:unit`: `src/lib/landed.test.ts`.
- `pnpm e2e`: `tests/e2e/update-project/landed.spec.ts` with the template `landed_upstream`: a robot pushed `feature-auth`
  and the first `search` commit, committer `Patronus`.

## Open Questions / Risks

- A fixup of a prefix branch is pushed without its marker (the virtual commit takes the subject without the `(prefix)`
  and the marker before it), so a robot can't fold it: it lands as a commit of its own and matches by key. Only the
  fixups of an issue or unassigned branch keep the marker and are folded.
- Leaving the candidates out is tried only when the replay without the proven commits conflicts, and the no-loss
  proof is strict: an upstream commit that changed the landed lines again after the robot makes it fail, and the
  update conflicts.
- A copy of the same patch by another author (a cherry-pick under another identity, a squash under another email) is
  found only after a conflict, and a change the baseline has with no proof at all (a squash of several commits)
  replays: a clean merge of either can still add its lines a second time. A walk of the patch ids of the whole
  baseline before every replay, as `git rebase` does, costs 1.5–5 s over a day of the IntelliJ monorepo (1500
  commits) and 10–17 s over three weeks (20,000), on every check of a sync; the paths of the local commits narrow it
  to 0.6 s for one or two paths but leave 7.5 s for 50 (2026-10-29, load 30–50).
- Two commits of one author in the same second with the same subject are one key.
- The shared-element move of a row into the island (`ViewTransition`) is not done: the rows change on a later sync
  event than the report, and zustand updates are not transitions.

## References

- [Integration Detection](../../../../docs/design/integration-detection.md): decision 11, "Landed commits".
- [Update Project](../update-project/spec.md), [Patronus](../../branch/patronus/spec.md).
- [Branch Row](../../branch/branch-row/spec.md): the Status cell and the anatomy of the chip details.
