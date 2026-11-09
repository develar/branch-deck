---
name: Outgoing Commits
description: Requirements for what a branch has for the remote (the commits whose content the remote lacks), the colours of a push and the push button of the branch row; the header says where the work is on its way into master instead (Landing Overview).
style: plain-1
targets:
  - src/components/branch/BranchPushButton.tsx
  - src/components/branch/DroppedCommitList.tsx
  - src/components/branch/BranchRow.tsx
  - src/components/repository/ReviewStatusLine.tsx
  - src/presenters/push-button.ts
  - src/hooks/useBranchPushes.ts
  - src/presenters/row-status.ts
  - src/hooks/useLandedIn.ts
  - src/lib/push-summary.ts
  - src/stores/stack-collapse.ts
  - crates/domain-policy/src/push.rs
  - crates/domain-types/src/remote_status.rs
  - crates/repository-model/src/push_guard.rs
  - crates/operations/src/push.rs
  - src/lib/ui-types.ts
  - src/stores/push-confirm.ts
  - src/lib/remote-status.ts
  - crates/sync-core/src/remote_status.rs
  - crates/object-cache/src/remote_status.rs
  - crates/git-ops/src/landed_pairing.rs
  - crates/sync-core/src/push.rs
  - crates/sync-core/src/push_lease.rs
  - crates/sync-core/src/push_size.rs
  - crates/git-ops/src/push_contents.rs
  - src/lib/push-run.ts
  - src/lib/toast-message.ts
  - src/lib/event-toast.ts
  - crates/sync-core/src/transfer_progress.rs
  - src/lib/transfer-progress.ts
  - src/stores/commit-move.ts
  - crates/sync-core/src/moved_remote.rs
  - crates/repository-model/src/moved_remote.rs
  - crates/domain-policy/src/refresh.rs
  - src/lib/remote-commits-news.ts
  - src/lib/git/pick-remote.ts
  - src/lib/review-commit.ts
  - src/model/intents.ts
  - crates/repository-model/src/ref_claims.rs
  - src/presenters/lock-wait.ts
  - src/presenters/reasons.ts
  - src/presenters/push.ts
---

# Outgoing Commits

Status: Implemented
Date: 2026-10-15

## Purpose

What a branch has for the remote is what its push sends: the commits whose content the remote lacks, compared by
content, not by hash, as a sync rebuilds a virtual branch on the latest baseline. The push button of the row says it
("↑ 3", "Force Push"), in a colour that says how risky it is, and the dot of the status text has that colour. A push only
serves the landing ([Landing](../../../../docs/design/landing.md)): the header says where the work is on its way into master,
not what it has to push ([Landing Overview](../landing-overview/spec.md)).

## Goals

- Tell a plain push from a first push and from a force push before pushing.
- Never count as unpushed what the remote has by content.

## Non-goals

- There is no "Push all": a push stays deliberate and per branch, or per stack from a stacked branch whose ancestors
  have work for the remote ([Stacked Branches](../../branch/stacked-branches/spec.md#pull-requests)); the description of
  its button names each push.
- The push does not fetch: its button shows the remote status of the last sync or push, or of the fetch a pull request
  that moved past its remote branch asks ([Remote commits of others](#remote-commits-of-others)).
- A remote branch with no pull request of the user (none, someone else's, the pull request of a whole stack) or of a
  user signed out of GitHub is not fetched on its own: someone's commit on it shows once a push is refused (above).
- The commits the remote has and the branch lacks are not to push; a force push drops or replaces them, and its row
  says how many.

## Concepts

### Content, not hashes

The sync rebuilds a virtual branch on the latest baseline, so its hashes change while its work stays the same. What
is outgoing compares content, not hashes: the question is whether the remote has the user's work as it is now.

### The pushes of a stack

The pushes of a stack follow in s9-13.

### Why a moved remote branch is fetched

Someone else may push to the branch of the user's pull request: a fix on top of it, or a commit of a review. The remote
status compares with the remote-tracking ref, which only a fetch moves, so the row would show the remote as this clone
last pushed it and a push would be refused ([Push](#push)). The listing of the pull requests shows the head of each
branch on GitHub, so the session fetches a branch whose open pull request shows another head.

## Requirements

### Requirement: What is outgoing: A branch is outgoing when its row offers a push

A branch MUST be outgoing when its row offers a push that is not disabled as up to date: the branch synced without an
error, has commits, and its remote status is loaded and not up to date (its action, which Rust decides of the remote
status, `RemoteStatusUpdate::push_action`).

#### Scenario: A synced branch with work for the remote
- **GIVEN** a branch that synced without an error and has commits
- **WHEN** its remote status is loaded and not up to date
- **THEN** its row offers a push that is not disabled as up to date
- **AND** the branch is outgoing

### Requirement: What is outgoing: A side stack with nothing outgoing is quiet

A side stack of branches none of which is outgoing, failed or blocked MUST be quiet: it starts collapsed
([Stacked Branches](../../branch/stacked-branches/spec.md#ui)).

#### Scenario: A quiet side stack starts collapsed
- **GIVEN** a side stack of branches none of which is outgoing, failed or blocked
- **WHEN** the branch table shows it
- **THEN** it is quiet and starts collapsed into its root, until the user expands it
  [@test] src/stores/branch-table-rows.test.ts (`collapses a side stack into its root, and keeps a quiet one collapsed until the user expands it`)

### Requirement: What is outgoing: A push counts the commits whose content the remote lacks

The commits of a push MUST be the commits whose content the remote lacks. The baseline commits MUST NOT count (the
remote has them), nor the rebased copies of pushed commits (patch-equivalent). A branch not on the remote counts all
its commits, the baseline excluded; mine counts only the user's commits, also for a first push.

#### Scenario: A branch not on the remote
- **GIVEN** a branch not on the remote, with commits of the user and of others
- **WHEN** its remote status is computed
- **THEN** it counts all its commits, the baseline excluded
- **AND** mine counts only the user's commits, also for this first push
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_new_branch_counts_commits_by_author`)

#### Scenario: Baseline commits and rebased copies do not count
- **GIVEN** a pushed branch rebuilt on a newer baseline, with a new commit
- **WHEN** its remote status is computed
- **THEN** the baseline commits do not count, the remote has them
- **AND** the rebased copies of pushed commits do not count (patch-equivalent); only the new commit does
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_new_commit_after_baseline_merge`)

### Requirement: What is outgoing: A branch only rebased onto a newer baseline is up to date

A branch only rebased onto a newer baseline MUST be up to date: the remote has all its commits by content. Its row
shows no status and no push, as for any branch up to date, its preview says "up to date", and it is not outgoing.

#### Scenario: A rebased branch has nothing to push
- **GIVEN** a pushed branch only rebased onto a newer baseline
- **WHEN** its remote status is computed
- **THEN** it is up to date, with zero unpushed commits: the remote has all its commits by content
- **AND** its row shows no status and no push, its preview says that it is up to date, and it is not outgoing
  [@test] crates/domain-types/src/remote_status_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_rebased_branch_shows_zero_unpushed`)

### Requirement: What is outgoing: A branch not built or not loaded is not outgoing

A branch with a conflict, a sync error or no commits MUST NOT be outgoing, nor one whose remote status is not loaded
yet: a side stack that holds one is not quiet.

#### Scenario: A branch with a conflict, a sync error or no commits
- **WHEN** a branch has a conflict, a sync error or no commits
- **THEN** it is not outgoing

#### Scenario: A side stack whose remote status is not loaded yet
- **GIVEN** a side stack whose branches have no remote status loaded yet
- **WHEN** the branch table shows it
- **THEN** its branches are not outgoing, yet the side stack is not quiet: it shows
- **AND** once their remote status is loaded and up to date, it collapses
  [@test] src/stores/branch-table-rows.test.ts (`collapses a side stack into its root, and keeps a quiet one collapsed until the user expands it`)

### Requirement: What is outgoing: A push of several branches leaves out the ones it cannot push

A push of several branches MUST leave out each branch the last sync did not build, one a running Safe Push holds and
one nothing of which is left to land (`domain_policy::push::push_plan`); its row says why.

#### Scenario: A stack push with a branch not built, held or landed
- **GIVEN** a stack whose branches include one the last sync did not build, one a running Safe Push holds and one nothing of which is left to land
- **WHEN** a push of several branches of it is planned
- **THEN** the plan leaves out each of those branches, and its row says why
- **AND** the push of a stack covers the ancestors up to the nearest held one
  [@test] crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`, `push_with_children_leaves_out_what_asks_first_is_held_or_landed`)

### Requirement: What is outgoing: A landed branch is not pushed with others

A branch the fetched baseline has all of ("landed", [Landed Commits](../landed-commits/spec.md#before-the-update))
MUST NOT be pushed with others: the update archives it, nothing of the user's is left to push. Its push button MUST
stay, quiet.

#### Scenario: A push of several branches leaves out the landed one
- **GIVEN** a stack with a branch the fetched baseline has all of
- **WHEN** a push of several branches of it is planned
- **THEN** the landed branch is left out
  [@test] crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)

#### Scenario: The landed branch keeps a quiet push button
- **GIVEN** a branch the fetched baseline has all of, before the update
- **WHEN** its row shows
- **THEN** its push button stays, quiet
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

### Requirement: What is outgoing: The state follows the label of the push button

The state of a branch MUST follow the label of its push button, which reads the action Rust decides of its remote
status for the app and `brd` alike (`RemoteStatusUpdate::push_action`, stored as `RemoteStatusUpdate.action`): one
of create, force and push.

#### Scenario: A branch not on the remote
- **WHEN** the branch is not on the remote
- **THEN** the action is "create" (`Push Branch (3)`, its description "The branch isn't on the remote yet: the push creates it.")
- **AND** one name stands for every push that sends the commits as they are
  [@test] crates/domain-types/src/remote_status_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)
  [@test] src/presenters/push-button.test.ts (`counts the commits the push sends in every label, the user's share in the description`)

#### Scenario: A push that is not a fast-forward
- **WHEN** the push is not a fast-forward: the remote has commits the branch lacks, older versions of commits amended or restacked (below), or the branch was rebased onto a newer baseline
- **THEN** the action is "force" (`Force Push Branch`)
- **AND** it follows the remote branch and the local one, not the result of the last sync, so it stays the same across syncs and restarts
  [@test] crates/domain-types/src/remote_status_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_rebased_branch_with_new_commit_is_not_fast_forward`, `one_walk_tells_what_is_behind_and_the_fast_forward`)

#### Scenario: Any other push
- **WHEN** the branch is on the remote and its push is a fast-forward
- **THEN** the action is "push" otherwise (`Push Branch (2)`)
  [@test] crates/domain-types/src/remote_status_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)
  [@test] src/presenters/push-button.test.ts (`counts the commits the push sends in every label, the user's share in the description`)

### Requirement: What is outgoing: A remote commit the branch lacks is told by its identity

A remote commit whose content the branch lacks MUST be told by its identity, the author email, the author time and the
subject without the `(prefix)` (as [landed commits](../landed-commits/spec.md) are): amended, a rebased copy counted
nowhere, restacked, landed, or dropped otherwise.

#### Scenario: An older version of a commit amended here
- **WHEN** an unpushed commit of the branch has the identity of the remote commit (an older version of a commit amended here) and makes other changes
- **THEN** the remote commit is amended
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`an_amended_commit_is_amended_not_dropped`, `an_amended_commit_and_a_dropped_one_are_told_apart`)

#### Scenario: A rebased copy whose lines moved
- **WHEN** a commit of the branch with its identity makes the same changes on lines that moved around them
- **THEN** the remote commit is a rebased copy, counted nowhere
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_rebased_copy_whose_context_moved_is_not_amended`)

#### Scenario: A commit of a branch it is built on has it
- **WHEN** a commit of a branch it is built on has the identity (the parent was amended or rebuilt since the branch was pushed: the child's remote branch has the parent's commits as they were)
- **THEN** the remote commit is restacked
- **AND** it is restacked also when the parent's commit was only reworded, which the remote has by content in its old version
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_child_of_an_amended_parent_is_restacked`, `a_child_of_a_reworded_parent_is_restacked`)

#### Scenario: A commit of the baseline above the remote branch has it
- **WHEN** a commit of the baseline above the remote branch has the identity (a Safe Push landed it, also rebuilt with other changes)
- **THEN** the remote commit is landed: `commitsLanded`, `landedCommits` with the baseline commit of each
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_remote_commit_that_landed_in_the_baseline_with_other_changes_is_landed_not_dropped`)

#### Scenario: Dropped otherwise
- **WHEN** no commit of the branch, of a branch it is built on or of the baseline has the identity
- **THEN** the remote commit is dropped
- **AND** another author's commit is dropped, also with the subject and the time of a commit of the branch
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`another_authors_commit_stays_dropped`, `an_amended_commit_and_a_dropped_one_are_told_apart`)

### Requirement: What is outgoing: Only the dropped remote commits are behind

Only the dropped ones MUST be behind (`commitsBehind`, `droppedCommits`): a push replaces the older versions and drops
the rest.

#### Scenario: Older versions beside a dropped commit
- **GIVEN** a remote branch with an older version of a commit amended here and a commit of another author
- **WHEN** its remote status is computed
- **THEN** only the dropped commit is behind and listed
- **AND** a push replaces the older version and drops the rest
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`an_amended_commit_and_a_dropped_one_are_told_apart`)

### Requirement: What is outgoing: A push outside the sync reads the ancestors only when needed

The sync knows the commits of the ancestors; a push outside it MUST read them from the parent's virtual branch, only
while the remote lacks a commit of the branch or of its ancestors or has one the branch lacks; the baseline MUST be
read only while commits are left dropped, one walk more.

#### Scenario: The walks a remote status takes
- **WHEN** the remote status of a branch is computed
- **THEN** a remote branch that is the branch takes no walk but the push time
- **AND** the baseline is read, one walk more, only while commits are left dropped
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`one_walk_tells_what_is_behind_and_the_fast_forward`)

#### Scenario: A push outside the sync reads the parent's virtual branch
- **GIVEN** a stacked branch whose parent was rebuilt
- **WHEN** a push outside the sync computes its status
- **THEN** it reads the commits of the ancestors from the parent's virtual branch, and tells the same as the sync
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_child_of_an_amended_parent_is_restacked`)

### Requirement: What is outgoing: A remote commit the branch has only through the baseline is landed

A remote commit the branch has by content only through a commit of the baseline, none of its own or of the branches it
is built on above the baseline, MUST be landed too (`commitsLanded`; listed in `landedCommits` when a baseline commit
has its identity): the branch is not up to date and its push replaces them without `--force`, whether the trees differ
or not.

#### Scenario: A child rebuilt on the baseline after its parent landed
- **GIVEN** the parent of a stack landed by a rebase merge, the child rebuilt on the baseline, its remote branch still built on the old one with the parent's old commits
- **WHEN** the remote status of the child is computed
- **THEN** those commits are landed: a pull request from that remote branch shows the landed changes again, so the branch is not up to date
- **AND** its push replaces them without `--force`, whether the trees differ or not
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_child_rebuilt_on_the_baseline_after_its_parent_landed_replaces_the_parents_old_copies`, `a_child_of_a_reworded_parent_is_restacked`)
  [@test] crates/bd-remote/src/push_test.rs (`a_child_rebuilt_on_the_baseline_after_its_parent_landed_is_pushed`, `a_push_replaces_the_remote_commits_the_baseline_landed_without_force`)

#### Scenario: A branch only rebuilt on a newer baseline
- **GIVEN** a branch only rebuilt on a newer baseline, its ancestors too
- **WHEN** its remote status is computed
- **THEN** it stays up to date

#### Scenario: No walk more for the landed copies
- **WHEN** the remote status tells such landed copies
- **THEN** it takes no walk more; the baseline is read for the baseline commits of those copies, one walk, only when there are some
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`one_walk_tells_what_is_behind_and_the_fast_forward`)

### Requirement: What is outgoing: New commits of a parent make the child replace its remote branch

The commits of the branches a stacked branch is built on that its remote branch lacks by content (the parent got a new
commit since the child was pushed, `ancestorCommitsAhead`) MUST make the push of the child replace its remote branch:
a Safe Push of the remote branch would land an older piece. They are no commits of the child to push (the parent's
push sends them to the parent's remote branch), and they come from the same walk of the branch as its own.

#### Scenario: The parent got a new commit since the child was pushed
- **GIVEN** a pushed stack whose parent got a new commit since the child was pushed
- **WHEN** the remote status of the child is computed, in the sync or outside it
- **THEN** it counts the commit in `ancestorCommitsAhead`, not as a commit of the child to push
- **AND** the push of the child replaces its remote branch
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_child_whose_parent_got_a_new_commit_lacks_it_on_its_remote`)
  [@test] crates/domain-types/src/remote_status_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)

#### Scenario: A newer version of a remote commit
- **GIVEN** a stacked branch whose parent's commit was amended
- **WHEN** the remote status of the child is computed
- **THEN** the newer version of the remote commit counts as restacked only, not as a commit the remote lacks
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_child_of_an_amended_parent_is_restacked`)

#### Scenario: The words of a restacked branch
- **WHEN** the remote branch of a child lacks a new commit of its parent
- **THEN** the status text says "restacked", its hover "Rebuilt on the new auth: the remote lacks 1 of its commits, which a Safe Push of the branch lands with it."
- **AND** the toast of the push says "1 new commit of the parent"
  [@test] src/lib/push-summary.test.ts (`says that a push of an amended or restacked branch replaced the older versions, not that it dropped them`)

### Requirement: What is outgoing: A commit reworded here is reworded

A commit the remote has by content in another version, its message or its author changed here (a reword), MUST be
reworded (`commitsReworded`), with no process more than the walks: not ahead, yet the remote branch is not the
branch, and a robot started or restarted from it would land the old version. A rebased copy keeps its message and
author and stays up to date.

#### Scenario: A reworded commit
- **GIVEN** a pushed commit whose message or author was changed here
- **WHEN** the remote status is computed
- **THEN** the commit is reworded, not ahead, and the branch is not up to date
- **AND** it takes no process more than the walks
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_reworded_commit_is_not_on_the_remote_as_it_is`)

#### Scenario: A rebased copy keeps its message and author
- **GIVEN** a pushed branch rebased onto a newer baseline
- **WHEN** the remote status is computed
- **THEN** its rebased copies keep their message and author and are not reworded
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_rebased_branch_with_new_commit_is_not_fast_forward`)

### Requirement: What is outgoing: A branch is up to date only when the remote has it as it is

A branch MUST be up to date only when the remote has nothing to receive and nothing the branch lacks, older versions
included, landed and reworded ones too (the action `upToDate`, `RemoteStatusUpdate::needs_push` for `brd push`,
`brd safe-push` and `brd status`); the push button, the status text, the review status of a robot or a pull request
and the restart of a robot (pushed first when the remote is not up to date) MUST decide by it.

#### Scenario: Older versions, landed or reworded commits leave work for a push
- **WHEN** the remote has older versions of commits, landed or reworded ones
- **THEN** the branch is not up to date and a push has work
  [@test] crates/domain-types/src/remote_status_test.rs (`a_push_has_work_unless_the_remote_has_the_branch_as_it_is`)

#### Scenario: The toast of such a push
- **WHEN** a push replaced older messages or commits that landed
- **THEN** its toast says so, without the warning
  [@test] src/lib/push-summary.test.ts (`says that a push replaced older messages or commits that landed, without the warning`)

#### Scenario: The review status of a robot or a pull request
- **WHEN** the review status of a robot or a pull request is worded
- **THEN** it compares with the remote branch and the remote status by content, not with the hash of the local tip
  [@test] src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is`)
  [@test] src/lib/review-commit.test.ts (`compares with the remote branch and the remote status by content, not with the hash of the local tip`)

#### Scenario: The restart of a robot
- **WHEN** a robot is restarted
- **THEN** it restarts on the remote when the remote is up to date, and pushes first when it is not
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)

### Requirement: What is outgoing: The remote status names its remote tip and the dropped commits

The remote status MUST name the remote commit it was computed against (`remoteTip`, null without a remote branch) and
list the newest ten of the commits a push drops (`droppedCommits`: id, subject, author, time), newest first;
`commitsBehind` counts them all. A push leases on that commit ([Push](#push)).

#### Scenario: More dropped commits than the limit
- **GIVEN** a remote branch with more commits the branch lacks than ten
- **WHEN** its remote status is computed
- **THEN** it lists the newest ten, newest first, and `commitsBehind` counts them all
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_dropped_commits_are_listed_newest_first_up_to_the_limit`)

#### Scenario: No remote branch
- **WHEN** the branch has no remote branch
- **THEN** `remoteTip` is null

### Requirement: What is outgoing: The dropped commits offer Review Changes before the push

Before the push, the list of the commits it drops in the description of the push button MUST offer Review Changes: the
review window reviews them as a list ("ABC-123 on the remote",
[Review Window](../../review/review-window/spec.md#one-window)), reachable by the remote-tracking branch; the list of an
Activity entry after the push MUST offer none, its commits may be gone.

#### Scenario: Review the commits a force push would drop
- **GIVEN** a push button whose description lists the remote commits a force push would drop
- **WHEN** the user chooses Review Changes there, before the push
- **THEN** the review window reviews them as a list, "ABC-123 on the remote", reachable by the remote-tracking branch
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should review the remote commits a force push would drop from the description of its button, before the push`)

#### Scenario: The list of an Activity entry after the push
- **WHEN** an Activity entry lists the commits a push dropped
- **THEN** it offers no Review Changes: its commits may be gone

### Requirement: What is outgoing: The last push time is the newest push from this clone

The last push time of a remote status (`lastPushTime`, "Pushed …" in the description of the status text and in the
branch preview) MUST be the time of the newest push of the branch from this clone, by its remote-tracking ref: a fetch
that moved the remote branch later doesn't change it, also for a branch whose name holds a `{`. Without such a push it
is 0, and nothing is shown.

#### Scenario: A fetch after the push
- **GIVEN** a branch pushed from this clone, whose remote branch a later fetch moved
- **WHEN** its remote status is computed
- **THEN** the last push time is the time of the push, not of the fetch, also for a branch whose name holds a `{`
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`the_push_time_is_the_newest_push_not_a_later_fetch`)

#### Scenario: No push from this clone
- **WHEN** the branch was never pushed from this clone
- **THEN** the last push time is 0, and nothing is shown

### Requirement: What is outgoing: The push times of a sync are one read

The push times of the branches of a sync MUST be one read, when a status needs one; a remote branch deleted meanwhile
has none, and the others keep theirs.

#### Scenario: Several branches, one of them deleted on the remote
- **GIVEN** a sync of several pushed branches, the remote branch of one deleted meanwhile
- **WHEN** their remote statuses need push times
- **THEN** the push times are one read
- **AND** the deleted one has none, and the others keep theirs
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`the_push_times_of_several_branches_are_one_read`)

### Requirement: What is outgoing: The remote status of commits that did not move takes no walk

The remote status of commits that did not move (the branch, its remote branch, the baseline, a stacked branch's
parent) MUST be the one computed before, without a walk: the app remembers it, push time included; `brd`, a new
process each time, takes it from the object cache and reads only the push times.

#### Scenario: The app syncs unchanged commits again
- **WHEN** the app syncs again and none of those commits moved
- **THEN** the remote status is the one computed before, push time included, and runs no git
  [@test] crates/sync-core/src/sync_test.rs (`the_remote_status_of_unchanged_commits_runs_no_git`)

#### Scenario: `brd` in a new process
- **WHEN** `brd` syncs pushed branches that did not move, in a new process
- **THEN** it takes their remote status from the object cache and reads only the push times
  [@test] crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_pushed_branches_in_a_new_process`)

### Requirement: What is outgoing: The count of a push names its parts in order

The count of a push MUST read `3↑` when its commits are all the user's, and `mine (total)↑` only when some are
others', then the older versions it replaces and the remote commits it drops, in that order, as the status text of the
branch row reads it.

#### Scenario: Commits of others among the ones to push
- **WHEN** the commits to push are all the user's
- **THEN** the count reads `3↑`
- **AND** when some are others' it reads `1 (3)↑`; `0 (3)↑` when none is theirs
  [@test] src/lib/remote-status.test.ts (`repeats the count in parentheses only when some commits to push are others'`)

#### Scenario: The parts after the commits it sends
- **WHEN** the push also replaces older versions or drops remote commits
- **THEN** the count goes on with `· 1 amended` for the older versions of commits amended here, `· 1 reworded` for the commits reworded here, `· 7 landed` for older versions of commits the baseline has, `· restacked` for those of the branches it is built on, `· 2↓` for the remote commits it drops, in that order
  [@test] src/lib/push-summary.test.ts (`names the older versions a push replaces after the commits it sends, before the ones it drops`)
  [@test] src/lib/remote-status.test.ts (`says that the remote has older versions of commits amended here or of the parent's, not commits the branch lacks`, `says that the remote has the old messages of commits reworded here, or older versions of commits that landed`)

#### Scenario: A push that sends no commit
- **WHEN** the push sends no commit
- **THEN** the count starts with the next part (`2↓`, `restacked`, `1 amended`, `1 reworded`)
  [@test] src/lib/push-summary.test.ts (`names the older versions a push replaces after the commits it sends, before the ones it drops`)

### Requirement: Push colours: One rule colours a push wherever it shows

One rule MUST colour a push wherever it shows: the push button and the dot of the status text of a branch row, the
count of the stack chip. The hue says how risky the push is.

#### Scenario: The surfaces of a push take the same colour
- **WHEN** a push shows on the push button of a branch row, the dot of its status text or the count of the stack chip
- **THEN** each takes the colour of the one rule, its hue saying how risky the push is

### Requirement: Push colours: The hue follows the risk of the push

The hue MUST follow the risk of the push: `warning` when it drops remote commits the branch lacks (`· N↓`), `primary`
otherwise. A force push of a rebased branch only replaces the remote commits with their rebased copies, one of an
amended, reworded, restacked or landed branch their older versions: it is safe, `primary`, and keeps the
`Force Push Branch` label, which says what git does.

#### Scenario: A push that drops remote commits
- **WHEN** the push drops remote commits the branch lacks (`· N↓`)
- **THEN** its hue is `warning`
  [@test] src/lib/push-summary.test.ts (`takes the hue from the risk: warning only for a push that drops remote commits`)

#### Scenario: A force push that only replaces
- **WHEN** a force push of a rebased, amended, reworded, restacked or landed branch only replaces the remote commits with their rebased copies or their older versions
- **THEN** it is safe, `primary`, and keeps the `Force Push Branch` label, which says what git does
  [@test] src/lib/push-summary.test.ts (`takes the hue from the risk: warning only for a push that drops remote commits`)

### Requirement: Push colours: Up to date and a safe push of no commit of the user are neutral

Up to date MUST be neutral. A safe push of no commit of the user MUST be neutral too (muted button, grey status dot):
there is nothing of theirs to push. A first push and a push that drops remote commits MUST keep their hue whoever
authored the commits, and so MUST a push that replaces older versions (`restacked` with nothing of its own to push):
the remote branch is behind the user's work.

#### Scenario: A safe push of no commit of the user
- **WHEN** a safe push sends no commit of the user
- **THEN** it is neutral: a muted button, a grey status dot
  [@test] src/lib/push-summary.test.ts (`is neutral for a safe push of no commit of the user`)

#### Scenario: A restacked branch with nothing of its own to push
- **WHEN** a push replaces older versions, `restacked` with nothing of its own to push
- **THEN** it keeps the push hue: the remote branch is behind the user's work
  [@test] src/lib/push-summary.test.ts (`keeps the push hue for a restacked branch with no commit of the user to push`)

#### Scenario: A first push or a push that drops remote commits, of others' commits
- **WHEN** a first push or a push that drops remote commits sends only commits of others
- **THEN** it keeps its hue whoever authored the commits

### Requirement: Push colours: The status text of a push state is never a badge

The status text of a push state MUST be muted text with a dot of the push colour, never a badge: a force push after a
sync is routine. Only an error or a conflict MUST be a badge.

#### Scenario: A force push after a sync
- **WHEN** the status text of a branch row shows a push state, a force push after a sync among them
- **THEN** it is muted text with a dot of the push colour, not a badge

#### Scenario: An error or a conflict
- **WHEN** the branch has an error or a conflict
- **THEN** its status is a badge

### Requirement: Push: A push replaces exactly the remote state the user saw

A push MUST replace exactly the remote state the user saw: the row's remote status, not Branch Deck's last push. A
push of a part of a branch, up to one of its commits, is Push Up to Here
([Stacked Branches](../../branch/stacked-branches/spec.md#push-up-to-here)).

#### Scenario: Someone else's commit fetched after the last push
- **GIVEN** a row whose remote status lists a remote commit, and someone else's commit fetched after Branch Deck's last push
- **WHEN** the user pushes
- **THEN** the push replaces the remote state the row showed, not the one of Branch Deck's last push
  [@test] crates/sync-core/src/push_test.rs (`a_force_push_drops_the_remote_commit_the_user_saw`)

#### Scenario: A push of a part of a branch
- **WHEN** the user pushes a branch up to one of its commits
- **THEN** it is Push Up to Here

### Requirement: Push: The push of a branch alone is the intent `Push` that Rust plans and runs

The push of a branch alone (its push button in the row, the preview and the review window, P, the menu, the action of
the remote-moved toast) and of an unapplied branch MUST be the intent `Push` that Rust plans and runs
(`operations::push`, [Rust as the Domain Authority](../../../../docs/design/rust-domain-authority.md#s9-engine-remote)):
`prepare` answers its plan, what it asks first and what refuses it; `execute` runs that plan in the session of the
repository.

#### Scenario: The plan of a push
- **WHEN** a push of a branch alone or of an unapplied branch is asked, from any of its surfaces
- **THEN** `prepare` answers its plan (the retarget of its pull request out of a virtual branch first, the cancel of the Safe Push it restarts, the push, the restart), what it asks first and what refuses it
- **AND** `execute` runs that plan in the session of the repository

### Requirement: Push: The page decides nothing of a push

The page MUST decide nothing of the push: its button reads the push action Rust offers the row (enabled or disabled
with the reason, `domain_policy::actions`), what each push sends (`RepositoryView.pushes`) and whether a push of the
branch runs (the `operations` of the view), and words the plan and the outcome (`src/presenters/push-button.ts`,
`src/lib/push-run.ts`).

#### Scenario: The push button of a row
- **WHEN** the row shows its push button
- **THEN** the button reads the push action Rust offers, what each push sends and whether a push of the branch runs
- **AND** the page only words the plan and the outcome

### Requirement: Push: The push of a branch is named for where it goes

The push of a branch MUST be named for where it goes, on every surface (the button of the row, the preview and the
review window, the menu item and its key P, the action of a toast, the busy state, the toasts, Activity): "Push
Branch", "Force Push Branch", beside "Push to master" of a repository that lands by a push of the copy into the target
([Landing Routes](../../branch/landing-routes/spec.md)).

#### Scenario: The labels of the push of a branch
- **WHEN** a surface offers the push of a branch
- **THEN** it reads "Push Branch" or "Force Push Branch", never a bare "Push": with push rights a bare "Push" read as a push into master
- **AND** "Push to master" stands beside it in a repository that lands by a push of the copy into the target
  [@test] src/presenters/push-button.test.ts (`counts the commits the push sends in every label, the user's share in the description`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)

#### Scenario: The pushes whose names say what they push
- **WHEN** a surface offers "Push Stack", "Push with Children", "Push auth Only" or "Push Up to Here"
- **THEN** they keep their names, which say what they push

### Requirement: Push: The toasts and the failures of a push name the branch as such

The toasts and the failures of the push of a branch MUST name the branch as such: "Pushed branch auth", "Pushed
branches auth and login", "Cannot push branch auth", "Pushing branch auth…" (of a whole stack its stack branch,
"Pushed the whole stack branch of air"); a push into the target MUST read "Pushed auth into master · 3 commits".

#### Scenario: A push to the remote branch beside a push into the target
- **WHEN** a push of a branch, of several branches or of a whole stack ends, or a push into the target ends
- **THEN** its toast names the remote branch it sent to, or the target for a push into it
  [@test] src/lib/push-summary.test.ts (`names the remote branch a push sends to, beside a push into the target`)

#### Scenario: The busy state of a push
- **WHEN** a push of the branch runs, also in another process
- **THEN** its row words it as "Pushing branch auth…"
  [@test] src/presenters/operations.test.ts (`words an operation of another process as its row says it and as the hover names its pid`)

### Requirement: Push: `brd push` says that it never pushes into master

`brd push` MUST keep its name; its help MUST say that it pushes each virtual branch to its own remote branch, never
into master, and point to `brd land`.

#### Scenario: The help of `brd push`
- **WHEN** an agent reads the help of `brd push`
- **THEN** it says that it pushes each virtual branch to its own remote branch, never into master
- **AND** it points to `brd land`

### Requirement: Push: A push label counts the commits the push sends

A push label MUST count the commits the push sends, whoever authored them: `Push Branch (2)`, `Force Push Branch (1)`;
a force push that sends none is `Force Push Branch`. The user's share MUST be in the description.

#### Scenario: The count in the label
- **WHEN** a push sends commits, of the user or of others
- **THEN** its label counts them all, `Push Branch (2)`, `Force Push Branch (1)`
- **AND** a force push that sends none is `Force Push Branch`
  [@test] src/presenters/push-button.test.ts (`counts the commits the push sends in every label, the user's share in the description`)

#### Scenario: The user's share in the description
- **WHEN** the description of the push button counts the commits to push
- **THEN** it reads "1 commit to push." when all are theirs, "2 of 3 commits to push are yours.", "3 commits to push, none of them yours."
  [@test] src/presenters/push-button.test.ts (`counts the commits the push sends in every label, the user's share in the description`)
  [@test] src/lib/remote-status.test.ts (`repeats the count in parentheses only when some commits to push are others'`)

### Requirement: Push: The description says what the push does

The description MUST say what the push does, one line each: a force push starts with "Force push replaces the remote
branch.", a first push ends with "The branch isn't on the remote yet: the push creates it.", the commits the remote
has and the branch lacks read "The remote has 2 commits your branch doesn't (by Ann and Bob).", and a rebased branch
says that the push replaces the remote commits with their rebased copies.

#### Scenario: Commits the remote has and the branch lacks
- **WHEN** the remote has commits the branch lacks
- **THEN** the description starts with the force push line and says how many and by whom
  [@test] src/lib/remote-status.test.ts (`says what the remote has that the branch lacks, and by whom`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)

#### Scenario: A first push
- **WHEN** the branch is not on the remote
- **THEN** the description ends with the line that the push creates it
  [@test] src/lib/remote-status.test.ts (`says that the push creates a branch not on the remote`)

#### Scenario: A rebased branch
- **WHEN** the branch was rebased onto a newer baseline
- **THEN** the description says that the push replaces the remote commits with their rebased copies

### Requirement: Push: An up to date branch says that everything is pushed

An up to date branch MUST say "Everything is pushed." (the status text too). A pushed branch MUST end with "Pushed
Today 10:00", as the [branch preview](../../branch/branch-preview/spec.md) says it.

#### Scenario: An up to date branch, pushed today
- **GIVEN** a branch pushed today at 10:00, up to date
- **WHEN** its push button or its status text describes it
- **THEN** it says that everything is pushed, in one sentence, and ends with when it was pushed
  [@test] src/lib/remote-status.test.ts (`says that everything is pushed in one sentence, and when it was pushed`)
  [@test] src/presenters/push-button.test.ts (`says that everything is pushed, and when, for an up to date branch`)

### Requirement: Push: Older versions read as such

The older versions MUST read as such, never as commits the branch lacks: "The remote has an older version of 1 commit
amended here.", "Rebuilt on the new auth: the remote has older versions of 2 of its commits." (`auth` is the branch it
is built on).

#### Scenario: An amended or restacked branch
- **WHEN** the remote has older versions of commits amended here or of the parent's
- **THEN** the description names them as older versions, not as commits the branch lacks
  [@test] src/lib/remote-status.test.ts (`says that the remote has older versions of commits amended here or of the parent's, not commits the branch lacks`)

### Requirement: Push: A force push that drops nothing starts with what it replaces

A force push that drops nothing MUST start with what it replaces, "Force push replaces 1 commit amended here.", "Force
push replaces the older versions of auth's commits." (both joined by "and"), which the lines below don't repeat, push
on the click and list no dropped commits; with dropped commits beside them it starts with "Force push replaces the
remote branch." and asks as below.

#### Scenario: A force push of an amended or restacked branch
- **WHEN** a force push replaces older versions and drops nothing
- **THEN** its description starts with what it replaces, and the lines below don't repeat it
- **AND** it pushes on the click, in the push colour, and lists no dropped commits
  [@test] src/lib/remote-status.test.ts (`names what a force push replaces when it drops nothing`, `starts the description of a force push with it, and doesn't repeat the older versions it names`)
  [@test] src/presenters/push-button.test.ts (`says what a force push of an amended or restacked branch replaces, and pushes it on the click in the push colour`)

#### Scenario: Older versions beside dropped commits
- **WHEN** the force push replaces older versions and drops remote commits too
- **THEN** its description starts with the line that it replaces the remote branch, and it asks as a push that drops remote commits does
  [@test] src/lib/remote-status.test.ts (`starts the description of a force push with it, and doesn't repeat the older versions it names`)

### Requirement: Push: A remote that moved to older versions says nothing was pushed

A remote that moved to older versions MUST say "The remote has older versions of the branch's commits. Nothing was
pushed."

#### Scenario: The remote moved to older versions of the branch's commits
- **WHEN** a push finds the remote moved to older versions of the branch's commits
- **THEN** it says that nothing was pushed, and offers the push the button would show
  [@test] src/lib/remote-status.test.ts (`offers the push the button would show when the remote lacks nothing of the branch`)

### Requirement: Push: A push that went through is a success toast

A push that went through MUST be a `success` toast in the words of its [Activity](../activity/spec.md) entry, from the
remote status the push was made against (`describePush`); with the pull request action
([Pull Requests](../../branch/pull-requests/spec.md)) it stays 10 s.

#### Scenario: The words of a push that went through
- **WHEN** a push went through
- **THEN** its toast reads "Pushed branch JIRA-456" / "3 commits · created the remote branch", "Pushed branch auth" / "1 commit", "Force pushed branch auth" / "2 commits · replaced the rebased copies", "1 commit · replaced 1 amended commit", "replaced the parent's older commits" or "1 commit · dropped 1 remote commit", from the status it was made against
  [@test] src/lib/push-summary.test.ts (`says what the push did from the status it was made against`, `says that a push of an amended or restacked branch replaced the older versions, not that it dropped them`)

#### Scenario: A toast with the pull request action
- **WHEN** the toast of the push carries the pull request action
- **THEN** it stays 10 s

### Requirement: Push: A failed push is an error toast with git's reason

A failed push MUST be the error toast "Cannot push branch JIRA-456" with git's reason.

#### Scenario: The push fails
- **WHEN** the push of a branch fails
- **THEN** the error toast "Cannot push branch JIRA-456" says git's reason

### Requirement: Push: One action is one toast

One action MUST be one toast: a push that is a step of another action (a Safe Push start, a merge request, a restart,
[Patronus](../../branch/patronus/spec.md)) has none, and a push of several branches of a stack is one toast
([Stacked Branches](../../branch/stacked-branches/spec.md#pull-requests)).

#### Scenario: A push as a step of another action
- **WHEN** a push is a step of a Safe Push start, a merge request or a restart
- **THEN** the push shows no toast of its own

#### Scenario: A push of several branches of a stack
- **WHEN** several branches of a stack are pushed together
- **THEN** they show one toast

### Requirement: Push: A later push of the branch takes the place of its toast

The toast of a push is the toast of the pushes of the branch
([One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event)): a later push of the branch, or
of a stack with it, MUST take its place, and its pull request action MUST be the one the branch has now, "Create Pull
Request…" becoming "Open Pull Request" once it has one.

#### Scenario: A later push of the branch
- **GIVEN** the toast of a push of the branch
- **WHEN** the branch, or a stack with it, is pushed again
- **THEN** the later push is told in that toast, in place: one toast per branch
- **AND** its pull request action is the one the branch has now, "Create Pull Request…" becoming "Open Pull Request" once it has one
  [@test] src/lib/push-run.test.ts (`is one toast per branch: a later push of it takes its place, its pull request action as the branch has it now`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should tell a later push of the branch in the toast of the first, in place: one toast per branch`)

### Requirement: Push: A push sends the remote head of the row and leases on it

A push MUST send the remote head of the row's remote status (`expectRemote` of the intent, null when the row showed no
remote branch) and lease on it (`PushLease::Planned`, in the fingerprint of its plan).

#### Scenario: The lease is the remote tip the row showed
- **WHEN** a push is sent
- **THEN** it leases on the remote tip the row showed: a rebuilt branch replaces the remote tip it saw
- **AND** a branch the row showed without a remote branch is created when it is expected absent
  [@test] crates/sync-core/src/push_test.rs (`a_rebuilt_branch_replaces_the_remote_tip_it_saw`, `a_branch_not_on_the_remote_is_created_when_expected_absent`)
  [@test] crates/sync-core/src/push_lease_test.rs (`an_expected_lease_is_the_remote_tip_the_caller_saw`)

#### Scenario: A remote-tracking ref that moved without the session knowing
- **GIVEN** a remote-tracking ref that moved since without the session knowing (a push from a terminal)
- **WHEN** the push is sent
- **THEN** nothing refuses it before the push, the remote refuses the lease
- **AND** the push goes on as for a remote that moved (below)
  [@test] crates/operations/src/push_test.rs (`a_push_over_a_remote_head_the_row_no_longer_shows_leases_on_it_and_goes_again_over_the_remote`)

#### Scenario: A push that waited for a sync or another push
- **GIVEN** a push that waited for a sync or another push
- **WHEN** it runs
- **THEN** it leases on what the session has after it
- **AND** it asks first again when that drops commits
  [@test] crates/operations/src/push_test.rs (`a_push_queued_behind_a_push_of_its_ref_is_decided_again_on_what_that_one_left`)

### Requirement: Push: A Force Push drops the commits the row listed and nothing else

A Force Push MUST drop the commits the row listed and nothing else, also when someone else's commit was fetched after
Branch Deck's last push; a plain push of a branch that took that commit in goes through.

#### Scenario: Someone else's commit fetched after the last push
- **GIVEN** someone else's commit fetched after Branch Deck's last push, which the row lists
- **WHEN** the user force pushes
- **THEN** the push drops the commit the user saw and nothing else
  [@test] crates/sync-core/src/push_test.rs (`a_force_push_drops_the_remote_commit_the_user_saw`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)

#### Scenario: A plain push of a branch that took that commit in
- **GIVEN** a branch that took the fetched commit in
- **WHEN** the user pushes it, with the head the row showed
- **THEN** the plain push goes through
  [@test] crates/sync-core/src/push_test.rs (`a_plain_push_with_the_seen_head_replaces_a_fetched_remote_commit`)

### Requirement: Push: A remote branch that moved since the status is not overwritten

A remote branch that moved since the row's status (a newer commit, a branch created or deleted elsewhere) MUST NOT be
overwritten: nothing is pushed, the branch alone is fetched, and its row shows the fetched remote status.

#### Scenario: A newer commit on the remote
- **GIVEN** a remote branch with a newer commit than the row's status
- **WHEN** the user pushes
- **THEN** nothing is pushed, the branch alone is fetched, and its row shows the fetched remote status
  [@test] crates/sync-core/src/push_test.rs (`a_remote_that_moved_since_the_status_is_fetched_not_overwritten`)
  [@test] crates/test-server/src/remote_git_fixtures_test.rs (`a_commit_on_the_remote_is_what_the_next_push_finds`)

#### Scenario: A branch created or deleted elsewhere
- **WHEN** the remote branch was created elsewhere meanwhile, or deleted
- **THEN** it is fetched, not overwritten, and a deleted one is shown as not pushed
  [@test] crates/sync-core/src/push_test.rs (`a_branch_created_on_the_remote_meanwhile_is_fetched_not_overwritten`, `a_branch_deleted_on_the_remote_is_shown_as_not_pushed`)

### Requirement: Push: A remote that moved with someone's commits opens a warning toast

A remote that moved with someone's commits, or one that moved again, MUST open a warning toast that stays until the
user acts: "login changed on the remote", "3 commits you don't have (by Ann and Bob). Nothing was pushed.", with the
push the row now offers ("Force Push Branch", "Push Branch") as its action, leased on the fetched head.

#### Scenario: The remote has commits the branch lacks
- **GIVEN** a push refused as the remote moved since the status, with commits the branch lacks
- **WHEN** the branch was fetched
- **THEN** the warning toast says how many commits the remote has that the branch lacks, and by whom, the distinct authors in the order of the commits
- **AND** its action is the push the row now offers, a force push, leased on the fetched head
  [@test] src/lib/push-run.test.ts (`opens the toast of the remote that moved: its push goes over what the remote has, leased on it`)
  [@test] src/lib/remote-status.test.ts (`says how many commits the remote has that the branch lacks, and by whom; the action is a force push`, `offers the push the button would show when the remote lacks nothing of the branch`, `offers to push a branch deleted on the remote again`, `names the distinct authors in the order of the commits`)

#### Scenario: The push from the toast drops the commits the remote has now
- **WHEN** the user takes the push of the toast and it drops the commits the remote has now
- **THEN** it confirms first, as below, and force pushes over it only then
- **AND** it asks before a push into a running Safe Push as any push does ([Patronus](../../branch/patronus/spec.md))
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should show what the remote has when it moved since the status, and force push over it from the toast`)

### Requirement: Push: A remote that moved and lost nothing is pushed over at once

A remote that moved without commits the branch lacks (deleted, rebuilt older versions, a branch created there with the
same commits) loses nothing: Rust MUST push the same frozen commits over the fetched head at once, once
(`RemoteStatusUpdate::pushes_again`, `operations::push::send`), and the page says only that it pushed, what it
replaced from the status it went over (`pushedOver`).

#### Scenario: The remote lacks nothing of the branch
- **GIVEN** a push refused as the remote moved, the fetched remote with no commits the branch lacks
- **WHEN** the push is decided again
- **THEN** it goes again over the fetched head at once, once, with the same frozen commits
- **AND** the page says only that it pushed and what it replaced
  [@test] crates/domain-types/src/remote_status_test.rs (`pushes_again_over_a_moved_remote_only_when_nothing_of_it_is_lost`)
  [@test] crates/operations/src/push_test.rs (`pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks`)

#### Scenario: `brd push` over a moved remote
- **WHEN** `brd push` meets a remote that moved and lacks nothing of the branch
- **THEN** it does the same and says so (`pushed_over` of the branch)
  [@test] crates/bd-remote/src/push_test.rs (`pushes_again_over_a_moved_remote_that_lacks_nothing_of_the_branch`)

### Requirement: Push: A stack push is one atomic push

A stack push MUST be one atomic push: a branch that moved refuses all of it, nothing of the stack is pushed and every
branch of it is fetched; it goes again over the fetched heads at once, once, when each remote that moved lost nothing,
as the push of one branch does, else the toast is of the branch that moved
([Stacked Branches](../../branch/stacked-branches/spec.md#pull-requests)).

#### Scenario: A branch of the stack moved on the remote
- **WHEN** a stack push meets a branch whose remote moved
- **THEN** the lease refuses every branch and names the one that moved: nothing of the stack is pushed, every branch of it is fetched
  [@test] crates/sync-core/src/push_test.rs (`a_moved_lease_refuses_every_branch_and_names_it`)

#### Scenario: Each remote that moved lost nothing
- **WHEN** each remote that moved lost nothing
- **THEN** the stack push goes again over the fetched heads at once, once; else the toast is of the branch that moved

### Requirement: Push: The description lists the remote commits a push drops

The description of a push that drops remote commits MUST list them under its text, newest first: subject, author,
short id and age, then "and N more" past the ten listed, all eleven of eleven (`DROPPED_COMMITS_LIMIT`,
`list_cut::kept`: never "and 1 more").

#### Scenario: More dropped commits than ten
- **WHEN** the push drops more remote commits than ten
- **THEN** its description lists the newest ten under its text, then how many more there are
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_dropped_commits_are_listed_newest_first_up_to_the_limit`)
  [@test] src/presenters/push-button.test.ts (`lists the remote commits a force push drops under its description, with how many more there are`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)

#### Scenario: Eleven dropped commits
- **WHEN** the push drops eleven remote commits
- **THEN** all eleven are listed, never one more past the ten
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_dropped_commits_one_more_than_the_limit_are_listed_whole`)

#### Scenario: A push that drops nothing
- **WHEN** the push drops no remote commit
- **THEN** its description lists nothing
  [@test] src/presenters/push-button.test.ts (`lists nothing for a push that drops no remote commit`)

### Requirement: Push: A push that drops remote commits asks first

A push that drops remote commits (the `warning` hue, `· 2↓`), someone else's work, MUST ask first: its plan asks it
(`Confirmation::ForcePushDrops` of `prepare`), so the click opens its description with the dropped commits and a
`Force Push Branch (1)` button below them, which pushes with the drop accepted (`force`).

#### Scenario: The click on a push that drops remote commits
- **WHEN** the user clicks a push that drops remote commits
- **THEN** its description opens with the dropped commits and the `Force Push Branch (1)` button below them
- **AND** that button pushes with the drop accepted, leased on the remote head the row shows
  [@test] src/presenters/push-button.test.ts (`asks first only for a push that drops remote commits`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)

#### Scenario: The pointer leaves the button
- **GIVEN** the open question of a push that drops remote commits
- **WHEN** the pointer leaves the button
- **THEN** the question stays open

#### Scenario: The question is dismissed
- **GIVEN** the open question of a push that drops remote commits
- **WHEN** the user presses Escape, clicks elsewhere or clicks the push button again
- **THEN** the question is dismissed, and nothing is pushed

### Requirement: Push: A safe push pushes on the click

A safe push (Push, the first one too, the Force Push of a rebased, amended or restacked branch) MUST push on the
click.

#### Scenario: A force push of a rebased branch
- **WHEN** the user clicks the force push of a rebased branch, which drops nothing
- **THEN** it pushes at once
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should push a rebased branch at once: its force push drops nothing`)

#### Scenario: A force push of an amended or restacked branch
- **WHEN** the user clicks the force push of an amended or restacked branch
- **THEN** it pushes on the click, in the push colour
  [@test] src/presenters/push-button.test.ts (`says what a force push of an amended or restacked branch replaces, and pushes it on the click in the push colour`)

### Requirement: Push: The remote-moved toast and P ask the same way

The action of the remote-moved toast and P in the [branch preview](../../branch/branch-preview/spec.md) MUST ask the
same way: the toast opens the question of the row's push button (scrolled into view), P that of the preview's footer.

#### Scenario: The action of the remote-moved toast
- **WHEN** the user takes the force push of the remote-moved toast
- **THEN** the question of the row's push button opens, scrolled into view
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should show what the remote has when it moved since the status, and force push over it from the toast`)

#### Scenario: P in the branch preview
- **WHEN** the user presses P in the branch preview over a commit the branch lacks
- **THEN** the question of the preview's footer opens before the force push
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should ask before P force pushes over a commit the branch lacks`)

### Requirement: Push: The description opens on hover also while the button is disabled

The description MUST open on hover also while the button is disabled: "Everything is pushed.", why a blocked stacked
branch can't push, the progress of a running push.

#### Scenario: A disabled push button
- **WHEN** the pointer hovers a disabled push button
- **THEN** its description opens: that everything is pushed, why a blocked stacked branch can't push, or the progress of a running push
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should describe a disabled push on hover`)

### Requirement: Push: A push the remote refuses says why as the remote said it

A push the remote refuses while the lease holds (a hook, a protected branch, a pack too large) MUST be an error that
says why as the remote said it: the report of a ref names only the kind (`pre-receive hook declined`,
`unpacker error`), the remote's `remote:` lines and git's `error:` lines name the reason, without the progress lines.

#### Scenario: A hook or a protected branch refuses the push
- **GIVEN** a lease that holds
- **WHEN** the remote refuses the push for another reason than the lease, as a hook does
- **THEN** the push is an error that names the reason by the remote's `remote:` lines, without the progress lines
  [@test] crates/sync-core/src/push_test.rs (`a_push_the_remote_refuses_for_another_reason_is_an_error`)

#### Scenario: A pack too large
- **WHEN** the remote refuses the push for its size, its ref reported as an `unpacker error`
- **THEN** the error says why, by the remote's `remote:` lines and git's `error:` lines
  [@test] crates/sync-core/src/push_test.rs (`a_push_the_remote_refuses_for_its_size_says_why`)

### Requirement: Push: A push over the push size limit is refused before the remote

A push whose commits add more files than the push size limit (`branchdeck.pushSizeLimit`, 50 MiB uncompressed unless
set, `0` or `off` for none; [Settings → Landing](../settings/spec.md#landing)) MUST be refused before it talks to the
remote: a server refuses such a pack only after the upload, as `unpacker error`, naming no file, and what it carries is
mostly build output committed by mistake.

#### Scenario: A push over the limit
- **GIVEN** a push whose commits add more files than the push size limit
- **WHEN** the push is asked
- **THEN** it is refused before it talks to the remote, and nothing is pushed
- **AND** once the limit is off, it is pushed
  [@test] crates/sync-core/src/push_test.rs (`a_push_over_the_size_limit_is_refused_before_the_remote`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should refuse a push over the push size limit before it is sent, saying what it adds, and push once the limit is off`)

#### Scenario: The limit of an operation
- **WHEN** a push reads the push size limit
- **THEN** it is `branchdeck.pushSizeLimit` when set, else the default of 50 MiB
  [@test] crates/sync-core/src/push_size_test.rs (`the_limit_of_an_operation_is_the_config_or_the_default`)

### Requirement: Push: The push size error says what the push adds and what to do

The error (`push_too_large`) MUST say how much the push adds, the deepest directory that holds four fifths of it and
the three largest files, then what to do.

#### Scenario: A push of build output
- **WHEN** a push of 2236 files is refused for its size
- **THEN** the error reads "The push would add 229.3 MiB in 2236 files, over the push size limit of 50.0 MiB: 228.9 MiB of it in tool/target/debug/ (2235 files); the largest: … Nothing was pushed. Remove the files from the commits (build output?), or raise the push size limit (branchdeck.pushSizeLimit, Settings → Landing)"
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should refuse a push over the push size limit before it is sent, saying what it adds, and push once the limit is off`)

#### Scenario: Files spread over the tree
- **WHEN** no directory holds four fifths of what the push adds
- **THEN** the error names no directory
  [@test] crates/sync-core/src/push_size_test.rs (`files_spread_over_the_tree_name_no_directory`)

### Requirement: Push: The files of a push are the blobs the remote lacks

The files MUST be the blobs the commits of the push add or change that no remote-tracking branch of the remote reaches,
each once (one `log --raw` and one `cat-file --batch-check`,
[Git Operations](../../../../docs/design/git-operations.md#push-size)).

#### Scenario: The files a push sends
- **WHEN** the size of a push is measured
- **THEN** it counts the blobs its commits add or change that no remote-tracking branch of the remote reaches, each once
  [@test] crates/git-ops/src/push_contents_test.rs (`a_push_sends_the_files_its_commits_add_that_the_remote_lacks`)

#### Scenario: The sizes in one batch
- **WHEN** the sizes of those blobs are read
- **THEN** it is one `cat-file --batch-check`
  [@test] crates/git-ops/src/push_contents_test.rs (`the_sizes_of_objects_are_one_batch_check`)

### Requirement: Push: Every push goes through the push size limit

Every push MUST go through the push size limit, the app's and `brd`'s, a stack, a whole stack and an unapplied branch,
a dry run too.

#### Scenario: A push of any kind
- **WHEN** the app or `brd` pushes a branch, a stack, a whole stack or an unapplied branch, or runs a dry run
- **THEN** the push size limit applies to it

### Requirement: Push: A push size limit that is neither a size nor off refuses the push

A limit that is neither a size nor off MUST refuse the push.

#### Scenario: A limit that is not a size
- **WHEN** the push size limit is set to a value that is neither a size nor off
- **THEN** the push is refused
  [@test] crates/sync-core/src/push_size_test.rs (`the_limit_is_a_size_or_off`)

### Requirement: Push: A switch of the repository before the push is asked stops it

A push MUST belong to the repository it started in. A switch of the repository before the push of a branch alone is
asked (while it waits for a rewrite of the page) MUST stop it: the branch of the same name there is another branch, so
nothing of it is pushed, and "Cannot push branch auth" says "idea-1 is no longer the open repository" (a push of an
action returns that reason to its action).

#### Scenario: The main window shows another repository now
- **GIVEN** a push of a branch alone waiting for a rewrite of the page
- **WHEN** the user switches to another repository before the push is asked
- **THEN** nothing of the branch of the same name there is pushed, and the error says why
  [@test] src/lib/push-run.test.ts (`pushes nothing of the branch of the same name in another repository the main window shows now, and says why`)

### Requirement: Push: Once asked, a push runs to its end in the session of its repository

Once asked, a push MUST be an operation of the session of its repository, which runs it to its end there, its sync,
its holds and its retarget too; a stack push of the page MUST stop at a switch while it waits.

#### Scenario: The operation of the session
- **WHEN** a push of a branch or a stack push of a row is asked
- **THEN** it is an operation of the session of its repository, which runs it to its end there
- **AND** a stack push of the page that waits sends none of its branches after a switch of the repository
  [@test] src/stores/branch-sync.test.ts (`is an operation of the session: the stack push of a row, the branches it sends, none after a switch of the repository`)

### Requirement: Push: A push already sent ends in its repository

A push already sent MUST end in its repository: its spinner and its progress stay off the rows of the other
repository, whose branch of the same name pushes on its own, and the remote status it brings goes to the session of
the repository it resolved; the progress it leaves is gone, so the next push of the branch, in either repository,
starts at "Pushing…".

#### Scenario: The spinner stays off the other repository
- **WHEN** a push runs and the user switches to another repository with a branch of the same name
- **THEN** its spinner and its progress stay off the rows of the other repository, whose branch pushes on its own
  [@test] src/stores/branch-sync.test.ts (`is an operation of the session: the stack push of a row, the branches it sends, none after a switch of the repository`)

#### Scenario: The remote status goes to the session of its repository
- **WHEN** the push ends
- **THEN** the remote status it brings goes to the session of the repository it resolved, and leaves the branch of the same name in another
  [@test] crates/repository-model/src/session_test.rs (`the_outcome_of_a_push_in_one_session_leaves_the_branch_of_the_same_name_in_another`)

#### Scenario: The next push starts afresh
- **WHEN** the next push of the branch starts, in either repository
- **THEN** the progress the push left is gone, and it starts at "Pushing…"

### Requirement: Push: A stack push retargets its pull requests only while its repository is shown

A stack push MUST retarget its pull requests one after another, each only while its repository is shown: a switch
during the retarget of login retargets no pull request of the other repository's ui.

#### Scenario: A switch during the retarget
- **WHEN** the user switches the repository during the retarget of login
- **THEN** no pull request of the other repository's ui is retargeted

### Requirement: Push: A push that ends after a switch offers no pull request

A push that ends after a switch MUST say "Pushed branch auth" with what it pushed and the robot it restarted, but offer
no pull request (no Open Pull Request, no Create Pull Request, no retarget clause), since the branch of the same name
there has its own; the pull requests it lists again MUST be of its repository: the session that ran the push lists
them (its `PushEnded`), never the selected one's.

#### Scenario: The toast after a switch
- **WHEN** a push ends after a switch of the repository
- **THEN** its toast says what it pushed and the robot it restarted, with no pull request action and no retarget clause

#### Scenario: The pull requests listed again
- **WHEN** a push ends in a session
- **THEN** that session lists its pull requests again, never the selected one's
  [@test] crates/repository-model/src/session_test.rs (`the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them`)

### Requirement: Push: The robot a push cancels is canceled right before the push is sent

The robot a push cancels, to restart it (Restart Safe Push) or because a start replaces it, MUST be canceled after the
last of those checks, right before the push is sent: a switch before it cancels nothing, and from the cancel on the
push goes on in its repository with the branches as they were, so no cancel is followed by "Cannot push branch auth".

#### Scenario: A switch before the cancel
- **WHEN** the user switches the repository before the push is sent
- **THEN** the robot is not canceled

#### Scenario: A switch after the cancel
- **WHEN** the user switches the repository after the cancel
- **THEN** the push goes on in its repository with the branches as they were, and no error follows the cancel

### Requirement: Push: The restart of a push goes to its repository

The restart MUST go to the push's repository, its robot and its Activity entry there, also after a switch.

#### Scenario: A restart after a switch
- **WHEN** a push that restarts a robot ends after a switch of the repository
- **THEN** the restart goes to the push's repository, its robot and its Activity entry there

### Requirement: Push: The remote-moved toast offers its push only while its repository is shown

The "auth changed on the remote" toast of a push MUST offer its push only while its repository is shown: after a switch
it says what the remote has without the push (it offers it again on the way back), and it closes once the branch is
gone; a push of the branch takes its place
([One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event)).

#### Scenario: A switch away and back
- **GIVEN** the toast of a remote that moved
- **WHEN** the user switches to another repository, then back
- **THEN** away it says what the remote has without the push, and back it offers the push again

#### Scenario: The branch is gone or pushed
- **WHEN** the branch is gone, or a push of the branch starts
- **THEN** the toast closes, or the toast of that push takes its place

### Requirement: Push: A running push shows the percent of the whole push

While a push runs, its button MUST show the percent of the whole push, "Pushing 45%" ("Pushing…" until git reports a
stage; the compact button of the row "45%" after the spinner, the label its accessible name).

#### Scenario: Git reports a stage
- **WHEN** a push runs and git reports a stage
- **THEN** the button shows the percent of the whole push, the compact button of the row after its spinner
- **AND** before the first stage it shows that it is pushing, without a percent
  [@test] src/lib/transfer-progress.test.ts (`shows the percent once git reports a stage`)
  [@test] crates/sync-core/src/push_test.rs (`a_push_reports_its_progress`)
  [@test] src/stores/branch-sync.test.ts (`is an operation of the session: the stack push of a row, the branches it sends, none after a switch of the repository`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should show the percent on the push button and git's last line on hover while the push runs`)

### Requirement: Push: The description of a running push shows git's last line

The description of a running push on hover MUST show the last progress line of git, then what was sent so far and how
fast, "1.23 MiB · 2.40 MiB/s", as the Update button shows a fetch ([Update Project](../update-project/spec.md)),
unless git's line says the amount already (`Writing objects:  50% (2/4), 1.23 MiB | 2.40 MiB/s`).

#### Scenario: The amount below git's line
- **WHEN** the user hovers the button of a running push
- **THEN** its description shows git's last progress line, and below it the amount and the rate only when the line doesn't say them
  [@test] src/lib/transfer-progress.test.ts (`shows the amount below git's line only when the line doesn't say it`)
  [@test] crates/sync-core/src/transfer_progress_test.rs (`writing_reports_the_amount_and_the_rate`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should show the percent on the push button and git's last line on hover while the push runs`)

### Requirement: Push: The stages of a push weigh as for a fetch

The stages MUST weigh as for a fetch: counting 5 %, compressing 10 %, writing 80 %, the server resolving deltas 5 %;
the percent MUST never decrease, reaches the window at most every 100 ms, and the end of a stage always does, also the
server's `Resolving deltas: 100% (2/2), completed with 2 local objects.`, which has no `, done.`.

#### Scenario: The shares of the stages
- **WHEN** git reports the stages of a push
- **THEN** they weigh as the stages of a fetch
  [@test] crates/sync-core/src/transfer_progress_test.rs (`push_stages_have_the_same_shares`)

#### Scenario: What reaches the window
- **WHEN** git reports progress faster than every 100 ms
- **THEN** the first progress and the end of every stage reach the window, also the server's line without `, done.`
  [@test] crates/sync-core/src/transfer_progress_test.rs (`reporter_sends_the_first_progress_and_every_stage_end`, `reporter_skips_a_stage_end_that_changes_nothing`)

### Requirement: Remote commits of others: A listing that shows another head fetches that branch

A listing of the pull requests that shows the open pull request of a branch at a head that is not the tip of its
remote-tracking ref MUST fetch that branch (`fetch_remote_branches`, the branch alone) and put the status against what
it fetched into the row, with no push and no sync: the row shows `· 1↓` in `warning` and the push asks first, as for
any remote commit a push drops.

#### Scenario: Someone pushed to the branch of a pull request
- **GIVEN** an open pull request of a branch, at a head that is not the tip of its remote-tracking ref
- **WHEN** a listing of the pull requests shows it
- **THEN** the session asks the fetch of that branch alone, and fetches it with the commits of others
- **AND** the row shows the status against what it fetched, `· 1↓` in `warning`, with no push and no sync, and the push asks first
  [@test] crates/domain-policy/src/refresh_test.rs (`a_remote_branch_whose_pull_request_moved_is_fetched_once_per_head`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_listing_that_shows_a_moved_remote_branch_asks_its_fetch`)
  [@test] crates/repository-model/src/moved_remote_test.rs (`a_moved_head_is_asked_once_and_a_new_head_again`)
  [@test] crates/sync-core/src/push_test.rs (`the_head_a_pull_request_shows_is_fetched_with_the_commits_of_others`)
  [@test] tests/e2e/outgoing/remote-commits.spec.ts (`should fetch the branch of a pull request someone pushed to and name their commit before any push`)

### Requirement: Remote commits of others: A branch the app pushes is not fetched

A branch the app pushes MUST NOT be fetched (the push leaves its own status).

#### Scenario: A pushed branch and a closed pull request
- **WHEN** the app pushed the branch, or its pull request is closed
- **THEN** the listing asks no fetch of it
  [@test] crates/repository-model/src/moved_remote_test.rs (`a_pushed_branch_and_a_closed_pull_request_are_not_fetched`)

### Requirement: Remote commits of others: The fetch moves the tracking ref only from the tip it read

The fetch MUST move the remote-tracking ref only from the tip it read before it, so a push of the same remote ref never
waits for it: a push that ends during the fetch keeps the head it wrote.

#### Scenario: A push that ends during the fetch
- **GIVEN** a fetch of a remote branch running
- **WHEN** a push of the same remote ref starts and ends during it
- **THEN** the push does not wait for the fetch: it takes the free refs at once
- **AND** the remote-tracking ref keeps the head of the push after the fetch ends
  [@test] crates/sync-core/src/push_lease_test.rs (`a_fetch_that_ends_after_a_push_of_its_ref_leaves_the_head_of_the_push`)
  [@test] crates/operations/src/push_test.rs (`a_push_does_not_wait_for_the_fetch_of_its_ref`)
  [@test] crates/repository-model/src/ref_claims_test.rs (`a_claim_takes_the_free_refs_at_once_and_leaves_the_held_ones`)

### Requirement: Remote commits of others: A push leaves the tracking refs at the pushed heads

A push that went MUST leave the remote-tracking ref of each ref it pushed at the pushed head also when git's push left
it behind (another git held its lock as the push ended), by compare-and-swap on the head it replaced, so a fetch that
moved it meanwhile keeps its own.

#### Scenario: Git's push left a tracking ref behind
- **GIVEN** an atomic push whose tracking refs git left at the replaced heads, another git holding their lock
- **WHEN** the push went
- **THEN** each tracking ref left at the replaced head moves to the pushed one, by compare-and-swap
  [@test] crates/sync-core/src/process_budget_test.rs (`an_atomic_push_whose_tracking_refs_git_left_behind`)
  [@test] crates/git-ops/src/pushed_tracking_test.rs (`a_tracking_ref_left_at_the_replaced_head_moves_to_the_pushed_one_and_a_fetched_one_stays`)

#### Scenario: A fetch moved the tracking ref meanwhile
- **WHEN** a fetch moved the tracking ref meanwhile
- **THEN** the fetched one stays
  [@test] crates/git-ops/src/pushed_tracking_test.rs (`a_tracking_ref_left_at_the_replaced_head_moves_to_the_pushed_one_and_a_fetched_one_stays`)

### Requirement: Remote commits of others: A head is fetched once

A head MUST be fetched once. A head the remote-tracking ref has already (GitHub shows an older head for a moment after
a push) fetches nothing; a head the fetch didn't bring (the mirror GitHub shows lags behind the remote) is fetched
again after a minute, five and fifteen, then not until the pull request shows another head.

#### Scenario: The same head again
- **WHEN** a listing shows the head of a pull request it showed before
- **THEN** it is not fetched again; a new head is
  [@test] crates/domain-policy/src/refresh_test.rs (`a_remote_branch_whose_pull_request_moved_is_fetched_once_per_head`)
  [@test] crates/repository-model/src/moved_remote_test.rs (`a_moved_head_is_asked_once_and_a_new_head_again`)

#### Scenario: A head the remote-tracking ref has already
- **WHEN** GitHub shows an older head for a moment after a push, one the remote-tracking ref has already
- **THEN** it fetches nothing
  [@test] crates/sync-core/src/push_test.rs (`a_head_the_remote_tracking_ref_has_fetches_nothing`)

#### Scenario: A head the fetch didn't bring
- **WHEN** the fetch didn't bring the head, as the mirror GitHub shows lags behind the remote
- **THEN** the head stays unsettled and is fetched again after a minute, five and fifteen
- **AND** then not until the pull request shows another head
  [@test] crates/sync-core/src/push_test.rs (`a_head_the_remote_lacks_stays_unsettled`)
  [@test] crates/repository-model/src/moved_remote_test.rs (`a_head_the_fetch_did_not_bring_is_asked_again_three_times`, `an_ask_the_host_did_not_fetch_is_asked_again`)

### Requirement: Remote commits of others: The remote status tells the commits of others from the user's

The remote status MUST tell the commits of others from the user's own among the ones a push drops: `othersBehind`
counts the dropped commits whose author is not the branch's author (`myEmail`), and each dropped commit names its
author's email. A commit of the user pushed from another clone is dropped, not someone else's; a commit of the user
under another email counts as someone else's.

#### Scenario: Commits of others and of the user behind
- **GIVEN** a remote branch with commits of others and a commit of the user pushed from another clone
- **WHEN** its remote status is computed
- **THEN** `othersBehind` counts only the commits of others, and each dropped commit names its author's email
- **AND** a commit of the user under another email counts as someone else's
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`the_commits_of_others_behind_leave_out_the_users_own`, `test_dropped_commits_are_listed_newest_first_up_to_the_limit`)

### Requirement: Remote commits of others: The commits of others are told once per remote tip

The commits of others a fetch brought MUST be told once per remote tip, in a `warning` toast that stays until the user
acts: "Lev Leontev pushed 1 commit to auth" / "Your branch doesn't have it: a push drops it.", with Pick Remote
Commits… and Open Pull Request. A fetch of the same tip again MUST tell nothing.

#### Scenario: A fetch brought someone's commit
- **WHEN** the session fetched a commit of someone else on the branch
- **THEN** a `warning` toast names who pushed how many commits to which branch, and that a push drops them
- **AND** it offers Pick Remote Commits… and Open Pull Request
  [@test] src/lib/remote-commits-news.test.ts (`names who pushed how many commits to which branch, and that a push drops them`, `takes the commits of others the session fetched, and no other news`)
  [@test] tests/e2e/outgoing/remote-commits.spec.ts (`should fetch the branch of a pull request someone pushed to and name their commit before any push`)

#### Scenario: A fetch of the same tip again
- **WHEN** a fetch brings the same remote tip again
- **THEN** it tells nothing

### Requirement: Remote commits of others: A push or a pick of the branch takes the place of that toast

The toast of the commits of others is the toast of the pushes of the branch: a push or a pick of it MUST take its
place.

#### Scenario: The branch is pushed or picked
- **GIVEN** the toast of the commits of others on a branch
- **WHEN** the branch is pushed, or its remote commits picked
- **THEN** the toast of that push or pick takes its place

### Requirement: Remote commits of others: Pick Remote Commits takes the commits a push would drop

Pick Remote Commits MUST take the commits a push would drop into the branch (`brd pick --remote`, [brd
pick](../../cli/pick/spec.md#remote-commits)), the rewrite `pickRemote` of the engine, which the session runs after the
rewrites before it: fetched again and leased on the remote tip the row shows, picked onto HEAD with the `(branch)`
prefix, then a sync; the row's push then drops nothing and asks nothing.

#### Scenario: Pick from the toast
- **GIVEN** the toast of a commit someone pushed to the branch
- **WHEN** the user takes Pick Remote Commits…
- **THEN** the commit is picked onto HEAD with the `(branch)` prefix, then a sync runs
- **AND** the row's push then drops nothing and asks nothing
  [@test] tests/e2e/outgoing/remote-commits.spec.ts (`should pick the commit someone pushed from the toast, after which the push drops nothing, and undo it`)

#### Scenario: A remote that moved again
- **WHEN** the remote moved again since the tip the row shows
- **THEN** the pick picks nothing, `lease_rejected`

### Requirement: Remote commits of others: A pick says what it took and offers Undo

A `success` toast MUST say "Picked the remote commits of auth" / "1 commit into auth · its push drops nothing now", with
Undo (the rewrite `undo` of its `pick:` token, in the repository of the pick), which takes HEAD and the files back.

#### Scenario: Undo of the pick
- **GIVEN** the toast of a pick of the remote commits
- **WHEN** the user takes Undo
- **THEN** HEAD and the files are back as before the pick
  [@test] tests/e2e/outgoing/remote-commits.spec.ts (`should pick the commit someone pushed from the toast, after which the push drops nothing, and undo it`)

### Requirement: Remote commits of others: A conflict or local changes in the way change nothing

A conflict or local changes in the way MUST change nothing ("Cannot pick the remote commits of auth", `pick_conflict`
with the commit and its files, or `uncommitted_overlap`).

#### Scenario: A conflict
- **WHEN** a remote commit conflicts with the branch
- **THEN** nothing changes, and the error is `pick_conflict` with the commit and its files

#### Scenario: Local changes in the way
- **WHEN** local changes are in the way of the pick
- **THEN** nothing changes, and the error is `uncommitted_overlap`

### Requirement: Remote commits of others: Pick Remote Commits is offered only while the remote has more

Rust MUST offer it in the branch menu ("Pick Remote Commits…") only while the remote has commits the branch lacks,
disabled while the branch is parked, blocked, held by a Safe Push or rebuilt by a sync.

#### Scenario: The remote has commits the branch lacks
- **WHEN** the remote has commits the branch lacks
- **THEN** the branch menu offers the item, disabled while the branch is parked, blocked, held by a Safe Push or rebuilt by a sync
- **AND** without such commits it offers none
  [@test] crates/domain-policy/src/actions_test.rs (`pick_remote_commits_is_offered_while_the_remote_has_commits_the_branch_lacks`)

### Requirement: Remote commits of others: Every surface of the pick opens the question of the push button

The item, the action of the toast and the next step MUST open the question of the push button, which lists the commits
and offers Pick Remote Commits beside the Force Push; the description of the push button MUST offer it too wherever it
lists dropped commits.

#### Scenario: From the branch menu
- **WHEN** the user chooses Pick Remote Commits… in the branch menu
- **THEN** the question of the push button opens, which lists the commits and offers the pick beside the Force Push
  [@test] tests/e2e/outgoing/remote-commits.spec.ts (`should pick the commit someone pushed from the branch menu, through the question of the push button`)

#### Scenario: As the next step
- **GIVEN** a branch on Space whose remote has commits of others
- **WHEN** its next step is shown, before its Safe Push
- **THEN** the pick is the next step and runs from the slot
  [@test] tests/e2e/outgoing/remote-commits.spec.ts (`should make the pick the next step of a branch on Space, before its Safe Push, and run it from the slot`)

### Requirement: Remote commits of others: The pull request of such a branch does not ask for a push

The pull request of such a branch MUST NOT ask for a push: its details say "Shows d708d5b8; someone else pushed commits
your branch doesn't have: a push drops them." ([Pull Requests](../../branch/pull-requests/spec.md)).

#### Scenario: The details of the pull request
- **WHEN** someone else pushed commits to the remote branch of a pull request
- **THEN** its details name them as commits a push would drop, never that a push is due
  [@test] src/lib/review-commit.test.ts (`names the commits someone else pushed to the remote branch, which a push would drop, before any push due`)
  [@test] src/lib/pull-request.test.ts (`says that a push drops the commits someone else pushed to the remote branch, never that a push is due`)

## User Experience

- The push colours are the semantic tokens `bg-primary` and `bg-warning`; `info` is not a push colour. Text never takes
  a state colour: the counts and the state words use `text-muted-foreground`, the colour is carried by the icon of the
  button and the dot of the status text.
- The push button opens its description on hover, as in the branch row. The dropped commits sit below a
  `border-border` rule under the caption "Dropped from the remote" (`text-xs text-muted-foreground`); a subject is
  one truncated line with its issue links and its dimmed conventional-commit prefix, as in the commit rows, the
  author, the `font-mono` short id and the age a muted line below it.
- A routine push (`primary`) colours only its icon, the label stays `text-foreground`: the button repeats on every
  row, and a fully coloured label (a force push in `warning`) means "look here".
- A coloured push button keeps its colour and its tint while its description is open
  (`aria-expanded:bg-primary/10` with the icon in `text-primary`, `aria-expanded:text-warning aria-expanded:bg-warning/10`
  for a force push, in `ghostButtonColorClass`): the ghost variant would turn an open trigger to the foreground.
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should keep the colour of the push while its description is open`)
- A disabled push button is `focusableWhenDisabled` (`aria-disabled`, no `disabled` attribute, which would stop the
  pointer events of the hover) and dimmed by `aria-disabled:opacity-50`; a running push keeps its colour beside the
  spinner, as Update and Sync do.
- The question of a push that drops remote commits is a `destructive` `xs` button with the upload icon and the label
  of the push (`Force Push Branch (1)`), at the start of the line below the dropped commits.

## Testing / Local Run

- `pnpm test:unit` covers the words and the colours of a push (`src/lib/push-summary.test.ts`).
- `pnpm e2e` covers the commits of others on a remote branch (`tests/e2e/outgoing/remote-commits.spec.ts`) against
  the remote of the template, where the test writes their commit (`commitOnRemote`): the only kind of repository whose
  moved remotes the server fetches.
- `pnpm e2e` covers the push (`tests/e2e/outgoing`). A push goes to the remote of the test repository, a bare
  `.git/upstream.git` ([The remote of a repository](../../../../docs/design/e2e-testing.md#the-remote-of-a-repository)): a
  branch pushed before, rebuilt since or moved by someone else is a push or a commit there behind the app's back
  (`pushedBefore`, `pushInTerminal`, `commitOnRemote`, a fetch with `fetchInTerminal`), and a test asserts what the
  remote has and the pushes it took (`expectPushedToRemote`, `remotePushes`); a push held at the remote
  (`holdRemotePushes`) shows the progress git reported.
- `pnpm test:backend` covers the push against a bare remote (`crates/sync-core/src/push_test.rs`).

## Open Questions / Risks

- The `primary` blue has a 4.6:1 contrast on the light card, the light tints (`/40`) 1.7:1; the counts and the state
  words carry the values in text.
- Solid blue and solid amber stay far apart for deuteranopia, which keeps the blue–yellow axis (ΔE76 121 in a Machado
  simulation, the light tints 49); the 2px gaps, the state words and the button labels carry the state too.
- No test covers a branch whose remote status is not loaded yet.

## References

- [Remote Status for Virtual Branches](../../../../docs/design/remote-status.md): how ahead, behind and mine are computed.
- [Branch Preview](../../branch/branch-preview/spec.md): the same push button and labels.
- [Landing Overview](../landing-overview/spec.md): the header, where the work is on its way into master.
- [UI Theme: Islands Layering and the Radius Scale](../../../../docs/design/ui-theme.md)
