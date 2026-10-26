---
name: Outgoing Commits
description: Requirements for what a branch has for the remote (the commits whose content the remote lacks), the colours of a push and the push button of the branch row; the header says where the work is on its way into master instead (Landing Overview).
style: plain-1
targets:
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/components/branch/DroppedCommitList.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/hooks/usePushButton.ts
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/hooks/useLandedIn.ts
  - ../../src/lib/push-summary.ts
  - ../../src/stores/stack-collapse.ts
  - ../../crates/domain-policy/src/push.rs
  - ../../crates/repository-model/src/push_guard.rs
  - ../../src/lib/ui-types.ts
  - ../../src/stores/push-confirm.ts
  - ../../src/lib/remote-status.ts
  - ../../crates/sync-core/src/remote_status.rs
  - ../../crates/object-cache/src/remote_status.rs
  - ../../crates/git-ops/src/landed_pairing.rs
  - ../../crates/sync-core/src/push.rs
  - ../../crates/sync-core/src/push_lease.rs
  - ../../crates/sync-core/src/push_size.rs
  - ../../crates/git-ops/src/push_contents.rs
  - ../../src/lib/git/push.ts
  - ../../src/lib/toast-message.ts
  - ../../src/lib/event-toast.ts
  - ../../crates/sync-core/src/transfer_progress.rs
  - ../../src/lib/transfer-progress.ts
  - ../../src/stores/commit-move.ts
  - ../../crates/sync-core/src/moved_remote.rs
  - ../../crates/repository-model/src/moved_remote.rs
  - ../../crates/domain-policy/src/refresh.rs
  - ../../src/lib/remote-commits-news.ts
  - ../../src/lib/review-commit.ts
---

# Outgoing Commits

Status: Implemented
Date: 2026-10-15

## Summary

What a branch has for the remote is what its push sends: the commits whose content the remote lacks, compared by
content, not by hash, as a sync rebuilds a virtual branch on the latest baseline. The push button of the row says it
("↑ 3", "Force Push"), in a colour that says how risky it is, and the dot of the status text has that colour. A push only
serves the landing ([Landing](../../docs/design/landing.md)): the header says where the work is on its way into master,
not what it has to push ([Landing Overview](landing-overview.spec.md)).

## Goals

- Tell a plain push from a first push and from a force push before pushing.
- Never count as unpushed what the remote has by content.

## Non-goals

- There is no "Push all": a push stays deliberate and per branch, or per stack from a stacked branch whose ancestors
  have work for the remote ([Stacked Branches](../branch/stacked-branches.spec.md#pull-requests)); the description of
  its button names each push.
- The push does not fetch: its button shows the remote status of the last sync or push, or of the fetch a pull request
  that moved past its remote branch asks ([Remote commits of others](#remote-commits-of-others)).
- A remote branch with no pull request of the user (none, someone else's, the pull request of a whole stack) or of a
  user signed out of GitHub is not fetched on its own: someone's commit on it shows once a push is refused (above).
- The commits the remote has and the branch lacks are not to push; a force push drops or replaces them, and its row
  says how many.

## Requirements

### What is outgoing

The sync rebuilds a virtual branch on the latest baseline, so its hashes change while its work stays the same. What
is outgoing compares content, not hashes: the question is whether the remote has the user's work as it is now.

- A branch is outgoing when its row offers a push that is not disabled as up to date: the branch synced without an
  error, has commits, and its remote status is loaded and not up to date (its action, which Rust decides of the remote
  status, `domain_policy::push::action`). A side stack of branches none of which is outgoing, failed or blocked is
  quiet: it starts collapsed ([Stacked Branches](../branch/stacked-branches.spec.md#ui)).
- The commits of a push must be the commits whose content the remote lacks. The baseline commits must not count (the
  remote has them), nor the rebased copies of pushed commits (patch-equivalent). A branch not on the remote counts all
  its commits, the baseline excluded; mine counts only the user's commits, also for a first push.
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`test_new_branch_counts_commits_by_author`)
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`test_new_commit_after_baseline_merge`)
- A branch only rebased onto a newer baseline must be up to date: the remote has all its commits by content. Its row
  shows no status and no push, as for any branch up to date, its preview says "up to date", and it is not outgoing.
  [@test] ../../crates/domain-policy/src/push_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`test_rebased_branch_shows_zero_unpushed`)
- A branch with a conflict, a sync error or no commits must not be outgoing, nor one whose remote status is not loaded
  yet: a side stack that holds one is not quiet. A push of several branches leaves out each branch the last sync did not
  build, one a running Safe Push holds and one nothing of which is left to land (`domain_policy::push::push_plan`); its
  row says why.
  [@test] ../../src/stores/branch-table-rows.test.ts (`collapses a side stack into its root, and keeps a quiet one collapsed until the user expands it`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`, `push_with_children_leaves_out_what_asks_first_is_held_or_landed`)
- A branch the fetched baseline has all of ("landed", [Landed Commits](landed-commits.spec.md#before-the-update)) must
  not be pushed with others: the update archives it, nothing of the user's is left to push. Its push button stays,
  quiet.
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
- The state of a branch must follow the label of its push button, which reads the action Rust decides of its remote
  status for the app and `bd` alike (`domain_policy::push::action`, `RemoteStatusUpdate.action`):
  - "create" for a branch not on the remote (`Push (3)`, its description "The branch isn't on the remote yet: the push
    creates it."); one name for every push that sends the commits as they are;
  - "force" for a push that is not a fast-forward: the remote has commits the branch lacks, older versions of commits
    amended or restacked (below), or the branch was rebased onto a newer baseline (`Force Push`). It follows the
    remote branch and the local one, not the result of the last sync, so it stays the same across syncs and restarts;
  - "push" otherwise (`Push (2)`).
  [@test] ../../crates/domain-policy/src/push_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)
  [@test] ../../src/hooks/usePushButton.test.ts (`counts the commits the push sends in every label, the user's share in the description`)
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`test_rebased_branch_with_new_commit_is_not_fast_forward`, `one_walk_tells_what_is_behind_and_the_fast_forward`)
- A remote commit whose content the branch lacks must be told by its identity, the author email, the author time and
  the subject without the `(prefix)` (as [landed commits](landed-commits.spec.md) are): amended when an unpushed
  commit of the branch has it (an older version of a commit amended here) and makes other changes, a rebased copy
  counted nowhere when it makes the same changes on lines that moved around them, restacked when a commit of a branch
  it is built on has it (the parent was amended or rebuilt since the branch was pushed: the child's remote branch has
  the parent's commits as they were; also when the parent's commit was only reworded, which the remote has by content
  in its old version), landed when a commit of the baseline above the remote branch has it (a Safe
  Push landed it, also rebuilt with other changes: `commitsLanded`, `landedCommits` with the baseline commit of each),
  dropped otherwise. Only the dropped ones are behind (`commitsBehind`, `droppedCommits`): a push replaces the older
  versions and drops the rest. Another author's commit is dropped, also with the subject and the time of a commit of
  the branch. The sync knows the commits of the ancestors; a push outside it reads them from the parent's virtual
  branch, only while the remote lacks a commit of the branch or of its ancestors or has one the branch lacks; the
  baseline is read only while commits are left dropped, one walk more.
- The commits of the branches a stacked branch is built on that its remote branch lacks by content (the parent got a
  new commit since the child was pushed, `ancestorCommitsAhead`) must make the push of the child replace its remote
  branch: a Safe Push of the remote branch would land an older piece. They are no commits of the child to push (the
  parent's push sends them to the parent's remote branch), they come from the same walk of the branch as its own, and a
  newer version of a remote commit counts as restacked only. The status text says "restacked", its hover "Rebuilt on
  the new auth: the remote lacks 1 of its commits, which a Safe Push of the branch lands with it.", the toast of the
  push "1 new commit of the parent".
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`a_child_whose_parent_got_a_new_commit_lacks_it_on_its_remote`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)
  [@test] ../../src/lib/push-summary.test.ts (`says that a push of an amended or restacked branch replaced the older versions, not that it dropped them`)
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`an_amended_commit_is_amended_not_dropped`, `a_rebased_copy_whose_context_moved_is_not_amended`, `a_child_of_an_amended_parent_is_restacked`, `a_child_of_a_reworded_parent_is_restacked`, `another_authors_commit_stays_dropped`, `an_amended_commit_and_a_dropped_one_are_told_apart`, `a_remote_commit_that_landed_in_the_baseline_with_other_changes_is_landed_not_dropped`, `one_walk_tells_what_is_behind_and_the_fast_forward`)
- A commit the remote has by content in another version, its message or its author changed here (a reword), must be
  reworded (`commitsReworded`), with no process more than the walks: not ahead, yet the remote branch is not the
  branch, and a robot started or restarted from it would land the old version. A rebased copy keeps its message and
  author and stays up to date.
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`a_reworded_commit_is_not_on_the_remote_as_it_is`, `test_rebased_branch_with_new_commit_is_not_fast_forward`)
- A branch is up to date only when the remote has nothing to receive and nothing the branch lacks, older versions
  included, landed and reworded ones too (the action `upToDate`, `domain_policy::push::needs_push` for `bd push`,
  `bd safe-push` and `bd status`); the push button, the status text, the review status of a robot or a pull request
  and the restart of a robot (pushed first when the remote is not up to date) decide by it.
  [@test] ../../crates/domain-policy/src/push_test.rs (`a_push_has_work_unless_the_remote_has_the_branch_as_it_is`)
  [@test] ../../src/lib/push-summary.test.ts (`says that a push replaced older messages or commits that landed, without the warning`)
  [@test] ../../src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is`)
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] ../../src/lib/review-commit.test.ts (`compares with the remote branch and the remote status by content, not with the hash of the local tip`)
- The remote status must name the remote commit it was computed against (`remoteTip`, null without a remote branch)
  and list the newest ten of the commits a push drops (`droppedCommits`: id, subject, author, time), newest first;
  `commitsBehind` counts them all. A push leases on that commit ([Push](#push)).
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`test_dropped_commits_are_listed_newest_first_up_to_the_limit`)
- The last push time of a remote status (`lastPushTime`, "Pushed …" in the description of the status text and in the
  branch preview) must be the time of the newest push of the branch from this clone, by its remote-tracking ref: a
  fetch that moved the remote branch later doesn't change it, also for a branch whose name holds a `{`. Without such
  a push it is 0, and nothing is shown. The push times of the branches of a sync are one read, when a status needs
  one; a remote branch deleted meanwhile has none, and the others keep theirs.
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`the_push_time_is_the_newest_push_not_a_later_fetch`, `the_push_times_of_several_branches_are_one_read`)
- The remote status of commits that did not move (the branch, its remote branch, the baseline, a stacked branch's
  parent) must be the one computed before, without a walk: the app remembers it, push time included; `bd`, a new
  process each time, takes it from the object cache and reads only the push times.
  [@test] ../../crates/sync-core/src/sync_test.rs (`the_remote_status_of_unchanged_commits_runs_no_git`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`an_unchanged_sync_of_pushed_branches_in_a_new_process`)
- The count of a push must read `3↑` when its commits are all the user's, and `mine (total)↑` only when some are
  others' (`1 (3)↑`; `0 (3)↑` when none is theirs), then `· 1 amended` for the older versions of commits amended
  here, `· 1 reworded` for the commits reworded here, `· 7 landed` for older versions of commits the baseline has,
  `· restacked` for those of the branches it is built on, `· 2↓` for the remote commits it drops, in that order; a
  push that sends no commit starts with the next part (`2↓`, `restacked`, `1 amended`, `1 reworded`), as the status
  text of the branch row reads it.
  [@test] ../../src/lib/push-summary.test.ts (`names the older versions a push replaces after the commits it sends, before the ones it drops`)
  [@test] ../../src/lib/remote-status.test.ts (`repeats the count in parentheses only when some commits to push are others'`, `says that the remote has older versions of commits amended here or of the parent's, not commits the branch lacks`, `says that the remote has the old messages of commits reworded here, or older versions of commits that landed`)

### Push colours

One rule colours a push wherever it shows: the push button and the dot of the status text of a branch row, the count of
the stack chip. The hue says how risky the push is.

- The hue must follow the risk of the push: `warning` when it drops remote commits the branch lacks (`· N↓`), `primary`
  otherwise. A force push of a rebased branch only replaces the remote commits with their rebased copies, one of an
  amended, reworded, restacked or landed branch their older versions: it is safe, `primary`, and keeps the
  `Force Push` label, which says what git does.
  [@test] ../../src/lib/push-summary.test.ts (`takes the hue from the risk: warning only for a push that drops remote commits`)
- Up to date is neutral. A safe push of no commit of the user is neutral too (muted button, grey status dot): there is
  nothing of theirs to push. A first push and a push that drops remote commits keep their hue whoever authored the
  commits, and so does a push that replaces older versions (`restacked` with nothing of its own to push): the remote
  branch is behind the user's work.
  [@test] ../../src/lib/push-summary.test.ts (`is neutral for a safe push of no commit of the user`, `keeps the push hue for a restacked branch with no commit of the user to push`)
- The status text of a push state is muted text with a dot of the push colour, never a badge: a force push after a sync
  is routine. Only an error or a conflict is a badge.

### Push

A push replaces exactly the remote state the user saw: the row's remote status, not Branch Deck's last push. A push
of a part of a branch, up to one of its commits, is Push Up to Here
([Stacked Branches](../branch/stacked-branches.spec.md#push-up-to-here)).

- A push label must count the commits the push sends, whoever authored them: `Push (2)`, `Force Push (1)`; a force push that sends none is `Force Push`. The user's share is in the description: "1 commit to
  push." when all are theirs, "2 of 3 commits to push are yours.", "3 commits to push, none of them yours.".
  [@test] ../../src/hooks/usePushButton.test.ts (`counts the commits the push sends in every label, the user's share in the description`)
  [@test] ../../src/lib/remote-status.test.ts (`repeats the count in parentheses only when some commits to push are others'`)
- The description must say what the push does, one line each: a force push starts with "Force push replaces the
  remote branch.", a first push ends with "The branch isn't on the remote yet: the push creates it.", the commits the
  remote has and the branch lacks read "The remote has 2 commits your branch doesn't (by Ann and Bob).", and a rebased
  branch says that the push replaces the remote commits with their rebased copies. An up to date branch says
  "Everything is pushed." (the status text too). A pushed branch ends with "Pushed Today 10:00", as the
  [branch preview](../branch/branch-preview.spec.md) says it.
  [@test] ../../src/lib/remote-status.test.ts (`says what the remote has that the branch lacks, and by whom`, `says that everything is pushed in one sentence, and when it was pushed`, `says that the push creates a branch not on the remote`)
  [@test] ../../src/hooks/usePushButton.test.ts (`says that everything is pushed, and when, for an up to date branch`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)
- The older versions must read as such, never as commits the branch lacks: "The remote has an older version of 1 commit
  amended here.", "Rebuilt on the new auth: the remote has older versions of 2 of its commits." (`auth` is the branch it
  is built on). A force push that drops nothing starts with what it replaces, "Force push replaces 1 commit amended
  here.", "Force push replaces the older versions of auth's commits." (both joined by "and"), which the lines below
  don't repeat, pushes on the click and lists no dropped commits; with dropped commits beside them it starts with "Force
  push replaces the remote branch." and asks as below. A remote that moved to older versions says "The remote has older
  versions of the branch's commits. Nothing was pushed."
  [@test] ../../src/lib/remote-status.test.ts (`says that the remote has older versions of commits amended here or of the parent's, not commits the branch lacks`, `names what a force push replaces when it drops nothing`, `starts the description of a force push with it, and doesn't repeat the older versions it names`, `offers the push the button would show when the remote lacks nothing of the branch`)
  [@test] ../../src/hooks/usePushButton.test.ts (`says what a force push of an amended or restacked branch replaces, and pushes it on the click in the push colour`)
- A push that went through is a `success` toast in the words of its [Activity](activity.spec.md) entry, from the
  remote status the push was made against (`describePush`): "Pushed JIRA-456" / "3 commits · created the remote
  branch", "Pushed auth" / "1 commit", "Force pushed auth" / "2 commits · replaced the rebased copies", "1 commit ·
  replaced 1 amended commit", "replaced the parent's older commits" or "1 commit · dropped 1 remote commit"; with the
  pull request action ([Pull Requests](../branch/pull-requests.spec.md)) it stays
  10 s. A failed push is the error toast "Cannot push JIRA-456" with git's reason. One action is one toast: a push that
  is a step of another action (a Safe Push start, a merge request, a restart, [Patronus](../branch/patronus.spec.md))
  has none, and a push of several branches of a stack is one toast
  ([Stacked Branches](../branch/stacked-branches.spec.md#pull-requests)). It is the toast of the pushes of the branch
  ([One toast per event](../../docs/guidelines/ux-writing.md#one-toast-per-event)): a later push of the branch, or of a
  stack with it, takes its place, and its pull request action is the one the branch has now, "Create Pull Request…"
  becoming "Open Pull Request" once it has one.
  [@test] ../../src/lib/push-summary.test.ts (`says what the push did from the status it was made against`, `says that a push of an amended or restacked branch replaced the older versions, not that it dropped them`)
  [@test] ../../src/lib/git/push.test.ts (`is one toast per branch: a later push of it takes its place, its pull request action as the branch has it now`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should tell a later push of the branch in the toast of the first, in place: one toast per branch`)
- A push must send the remote head of the row's remote status (`expectedRemote`, null when the row showed no remote
  branch) and lease on it. A Force Push drops the commits the row listed and nothing else, also when someone else's
  commit was fetched after Branch Deck's last push; a plain push of a branch that took that commit in goes through.
  [@test] ../../crates/sync-core/src/push_test.rs (`a_force_push_drops_the_remote_commit_the_user_saw`, `a_plain_push_with_the_seen_head_replaces_a_fetched_remote_commit`, `a_rebuilt_branch_replaces_the_remote_tip_it_saw`, `a_branch_not_on_the_remote_is_created_when_expected_absent`)
  [@test] ../../crates/sync-core/src/push_lease_test.rs (`an_expected_lease_is_the_remote_tip_the_caller_saw`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)
- A remote branch that moved since the row's status (a newer commit, a branch created or deleted elsewhere) must not
  be overwritten: nothing is pushed, the branch alone is fetched, and its row shows the fetched remote status. A
  warning toast stays until the user acts: "login changed on the remote", "3 commits you don't have (by Ann and Bob).
  Nothing was pushed.", with the push the row now offers ("Force Push", "Push") as its action,
  leased on the fetched head; a push that drops the commits the remote has now confirms first, as below, and it asks
  before a push into a running Safe Push as any push does ([Patronus](../branch/patronus.spec.md)). A remote that
  moved without commits the branch lacks (deleted, rebuilt older versions, a branch created there with the same
  commits) loses nothing: Rust pushes over the fetched head at once, once (`domain_policy::push::over_moved_remote`,
  the lock and the checks again), and the page says only that it pushed, what it replaced from the status it went over
  (`pushedOver`); `bd push` does the same and says so (`pushed_over` of the branch). The toast is for a remote with
  someone's commits, or one that moved again. A stack push is one atomic push: a branch that moved refuses all of it,
  nothing of the stack is pushed and every branch of it is fetched; it goes again over the fetched heads at once, once,
  when each remote that moved lost nothing, as the push of one branch does, else the toast is of the branch that moved
  ([Stacked Branches](../branch/stacked-branches.spec.md#pull-requests)).
  [@test] ../../crates/domain-policy/src/push_test.rs (`pushes_again_over_a_moved_remote_only_when_nothing_of_it_is_lost`)
  [@test] ../../crates/repository-model/src/push_guard_test.rs (`pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks`)
  [@test] ../../crates/bd-remote/src/push_test.rs (`pushes_again_over_a_moved_remote_that_lacks_nothing_of_the_branch`)
  [@test] ../../src/lib/git/push.test.ts (`says a push Rust made again over a remote that moved from what it went over, in one push of the page`, `tells a remote with commits the branch lacks, and pushes nothing over it`, `pushes all or none: a remote branch that moved refuses the push, and its toast says what that remote has`, `says a stack push Rust made again over a remote that moved from what it went over, in one push of the page`)
  [@test] ../../crates/sync-core/src/push_test.rs (`a_remote_that_moved_since_the_status_is_fetched_not_overwritten`, `a_branch_created_on_the_remote_meanwhile_is_fetched_not_overwritten`, `a_branch_deleted_on_the_remote_is_shown_as_not_pushed`, `a_moved_lease_refuses_every_branch_and_names_it`)
  [@test] ../../src/lib/remote-status.test.ts (`says how many commits the remote has that the branch lacks, and by whom; the action is a force push`, `offers the push the button would show when the remote lacks nothing of the branch`, `offers to push a branch deleted on the remote again`, `names the distinct authors in the order of the commits`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show what the remote has when it moved since the status, and force push over it from the toast`)
- The description of a push that drops remote commits must list them under its text, newest first: subject, author,
  short id and age, then "and N more" past the ten listed, all eleven of eleven (`DROPPED_COMMITS_LIMIT`,
  `list_cut::kept`: never "and 1 more").
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`test_dropped_commits_are_listed_newest_first_up_to_the_limit`, `test_dropped_commits_one_more_than_the_limit_are_listed_whole`)
  [@test] ../../src/hooks/usePushButton.test.ts (`lists the remote commits a force push drops under its description, with how many more there are`, `lists nothing for a push that drops no remote commit`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)
- A push that drops remote commits (the `warning` hue, `· 2↓`), someone else's work, must ask first: the click opens its
  description with the dropped commits and a `Force Push (1)` button below them, which pushes. Leaving the button keeps
  the question open; Escape, a click elsewhere or on the push button again dismisses it, and nothing is pushed. A safe
  push (Push, the first one too, the Force Push of a rebased, amended or restacked branch) pushes on the click. The action
  of the remote-moved toast and P in the [branch preview](../branch/branch-preview.spec.md) ask the same way: the toast
  opens the question of the row's push button (scrolled into view), P that of the preview's footer.
  [@test] ../../src/hooks/usePushButton.test.ts (`asks first only for a push that drops remote commits`, `says what a force push of an amended or restacked branch replaces, and pushes it on the click in the push colour`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`, `should show what the remote has when it moved since the status, and force push over it from the toast`, `should push a rebased branch at once: its force push drops nothing`)
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should ask before P force pushes over a commit the branch lacks`)
- The description must open on hover also while the button is disabled: "Everything is pushed.", why a blocked
  stacked branch can't push, the progress of a running push.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should describe a disabled push on hover`)
- A push the remote refuses while the lease holds (a hook, a protected branch, a pack too large) must be an error
  that says why as the remote said it: the report of a ref names only the kind (`pre-receive hook declined`,
  `unpacker error`), the remote's `remote:` lines and git's `error:` lines name the reason, without the progress lines.
  [@test] ../../crates/sync-core/src/push_test.rs (`a_push_the_remote_refuses_for_another_reason_is_an_error`, `a_push_the_remote_refuses_for_its_size_says_why`)
- A push whose commits add more files than the push size limit (`branchdeck.pushSizeLimit`, 50 MiB uncompressed
  unless set, `0` or `off` for none; [Settings → Patronus](settings.spec.md#patronus)) must be refused before it
  talks to the remote: a server refuses such a pack only after the upload, as `unpacker error`, naming no file, and
  what it carries is mostly build output committed by mistake. The error (`push_too_large`) says how much it adds, the
  deepest directory that holds four fifths of it and the three largest files, then what to do: "The push would add
  229.3 MiB in 2236 files, over the push size limit of 50.0 MiB: 228.9 MiB of it in tool/target/debug/ (2235 files);
  the largest: … Nothing was pushed. Remove the files from the commits (build output?), or raise the push size limit
  (branchdeck.pushSizeLimit, Settings → Patronus)". The files are the blobs the commits of the push add or change
  that no remote-tracking branch of the remote reaches, each once (one `log --raw` and one `cat-file --batch-check`,
  [Git Operations](../../docs/design/git-operations.md#push-size)); every push goes through it, the app's and `bd`'s,
  a stack, a whole stack and an unapplied branch, a dry run too. A limit that is neither a size nor off refuses the
  push.
  [@test] ../../crates/sync-core/src/push_test.rs (`a_push_over_the_size_limit_is_refused_before_the_remote`)
  [@test] ../../crates/sync-core/src/push_size_test.rs (`the_limit_is_a_size_or_off`, `the_limit_of_an_operation_is_the_config_or_the_default`, `files_spread_over_the_tree_name_no_directory`)
  [@test] ../../crates/git-ops/src/push_contents_test.rs (`a_push_sends_the_files_its_commits_add_that_the_remote_lacks`, `the_sizes_of_objects_are_one_batch_check`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should refuse a push over the push size limit before it is sent, saying what it adds, and push once the limit is off`)
- A push belongs to the repository it started in. A switch of the repository while it waits (for the sync, the holds,
  a retarget) stops it before it reads the branches again: the branch of the same name there is another branch, so
  nothing of it is pushed, and "Cannot push auth" says "idea-1 is no longer the open repository" (a push of an action
  returns that reason to its action). A push already sent ends in its repository: its spinner and its progress stay
  off the rows of the other repository, whose branch of the same name pushes on its own, and the remote status it
  brings goes to the session of the repository it resolved; the progress it leaves is gone, so the next push of the
  branch, in either repository, starts at "Pushing…".
  A stack push retargets its pull requests one after another, each only while its repository is shown: a switch during
  the retarget of login retargets no pull request of the other repository's ui. A push that ends after a switch says
  "Pushed auth" with what it pushed and the robot it restarted, but offers no pull request (no Open Pull Request, no
  Create Pull Request, no retarget clause), since the branch of the same name there has its own; the pull requests it
  lists again are of its repository: the session that ran the push lists them (its `PushEnded`), never the selected
  one's.
  [@test] ../../src/lib/git/push.test.ts (`pushes nothing of the branch of the same name there, and says why`, `pushes no stack of the same names there`, `refuses the push of an action in place`)
  [@test] ../../src/lib/git/push.test.ts (`retargets no pull request of the branch of the same name there after a switch during a retarget of the stack`, `offers no pull request of the branch of the same name there in the toast of a push that ends after a switch`, `offers no pull request of the branches of the same names there in the toast of a stack push that ends after a switch`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them`)
  [@test] ../../src/stores/branch-sync.test.ts (`keeps the spinner of a push that ends after a switch of the repository off the branch of the same name there`, `drops the progress of a push that ends after a switch: the next push of the branch starts without its percent`, `keeps a stack push that runs on after a switch off the row of the same name there`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_outcome_of_a_push_in_one_session_leaves_the_branch_of_the_same_name_in_another`)
- The robot a push cancels, to restart it (Restart Safe Push) or because a start replaces it, must be canceled after
  the last of those checks, right before the push is sent: a switch before it cancels nothing, and from the cancel on
  the push goes on in its repository with the branches as they were, so no cancel is followed by "Cannot push auth".
  The restart goes to the push's repository, its robot and its Activity entry there, also after a switch. The "auth
  changed on the remote" toast of a push offers its push only while its repository is shown: after a switch it says
  what the remote has without the push (it offers it again on the way back), and it closes once the branch is gone;
  a push of the branch takes its place ([One toast per event](../../docs/guidelines/ux-writing.md#one-toast-per-event)).
  [@test] ../../src/lib/git/push.test.ts (`restarts the robot of Restart Safe Push where the push went, after a switch during the push`, `pushes and restarts the robot it canceled after a switch during the cancel: no cancel is followed by a refusal`, `pushes the branches over the robot a start replaces after a switch during the cancel`, `cancels nothing when the switch comes before the push's last check`, `pushes and restarts the whole stack's robot in the repository of the push after a switch during the cancel`, `offers no push of the branch of the same name in the toast of a remote that moved during the push`)
- While a push runs, its button must show the percent of the whole push, "Pushing 45%" ("Pushing…" until git reports
  a stage; the compact button of the row "45%" after the spinner, the label its accessible name), and its description on hover the last progress line of git, then what was sent so far and how fast,
  "1.23 MiB · 2.40 MiB/s", as the Update button shows a fetch ([Update Project](update-project.spec.md)), unless git's
  line says the amount already (`Writing objects:  50% (2/4), 1.23 MiB | 2.40 MiB/s`). The stages
  weigh as for a fetch: counting 5 %, compressing 10 %, writing 80 %, the server resolving deltas 5 %; the percent
  never decreases, reaches the window at most every 100 ms, and the end of a stage always does, also the server's
  `Resolving deltas: 100% (2/2), completed with 2 local objects.`, which has no `, done.`.
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`push_stages_have_the_same_shares`, `writing_reports_the_amount_and_the_rate`, `reporter_sends_the_first_progress_and_every_stage_end`, `reporter_skips_a_stage_end_that_changes_nothing`)
  [@test] ../../crates/sync-core/src/push_test.rs (`a_push_reports_its_progress`)
  [@test] ../../src/lib/transfer-progress.test.ts (`shows the percent once git reports a stage`, `shows the amount below git's line only when the line doesn't say it`)
  [@test] ../../src/stores/branch-sync.test.ts (`keeps the push progress only while the push runs`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show the percent on the push button and git's last line on hover while the push runs`)

### Remote commits of others

Someone else may push to the branch of the user's pull request: a fix on top of it, or a commit of a review. The remote
status compares with the remote-tracking ref, which only a fetch moves, so the row would show the remote as this clone
last pushed it and a push would be refused ([Push](#push)). The listing of the pull requests shows the head of each
branch on GitHub, so the session fetches a branch whose open pull request shows another head.

- A listing of the pull requests that shows the open pull request of a branch at a head that is not the tip of its
  remote-tracking ref must fetch that branch (`fetch_remote_branches`, the branch alone) and put the status against
  what it fetched into the row, with no push and no sync: the row shows `· 1↓` in `warning` and the push asks first,
  as for any remote commit a push drops. A branch the app pushes is not fetched (the push leaves its own status), and
  a push of the same remote ref waits for a fetch that writes it.
  [@test] ../../crates/domain-policy/src/refresh_test.rs (`a_remote_branch_whose_pull_request_moved_is_fetched_once_per_head`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`a_listing_that_shows_a_moved_remote_branch_asks_its_fetch`)
  [@test] ../../crates/repository-model/src/moved_remote_test.rs (`a_moved_head_is_asked_once_and_a_new_head_again`, `a_pushed_branch_and_a_closed_pull_request_are_not_fetched`)
  [@test] ../../crates/sync-core/src/push_test.rs (`the_head_a_pull_request_shows_is_fetched_with_the_commits_of_others`)
  [@test] ../../tests/e2e/outgoing/remote-commits.spec.ts (`should fetch the branch of a pull request someone pushed to and name their commit before any push`)
- A head is fetched once. A head the remote-tracking ref has already (GitHub shows an older head for a moment after a
  push) fetches nothing; a head the fetch didn't bring (the mirror GitHub shows lags behind the remote) is fetched
  again after a minute, five and fifteen, then not until the pull request shows another head.
  [@test] ../../crates/sync-core/src/push_test.rs (`a_head_the_remote_tracking_ref_has_fetches_nothing`, `a_head_the_remote_lacks_stays_unsettled`)
  [@test] ../../crates/repository-model/src/moved_remote_test.rs (`a_head_the_fetch_did_not_bring_is_asked_again_three_times`, `an_ask_the_host_did_not_fetch_is_asked_again`)
- The remote status must tell the commits of others from the user's own among the ones a push drops: `othersBehind`
  counts the dropped commits whose author is not the branch's author (`myEmail`), and each dropped commit names its
  author's email. A commit of the user pushed from another clone is dropped, not someone else's; a commit of the user
  under another email counts as someone else's.
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`the_commits_of_others_behind_leave_out_the_users_own`, `test_dropped_commits_are_listed_newest_first_up_to_the_limit`)
- The commits of others a fetch brought must be told once per remote tip, in a `warning` toast that stays until the
  user acts: "Lev Leontev pushed 1 commit to auth" / "Your branch doesn't have it: a push drops it.", with Pick Remote
  Commits… and Open Pull Request. It is the toast of the pushes of the branch: a push or a pick of it takes its place.
  A fetch of the same tip again tells nothing.
  [@test] ../../src/lib/remote-commits-news.test.ts (`names who pushed how many commits to which branch, and that a push drops them`, `takes the commits of others the session fetched, and no other news`)
  [@test] ../../tests/e2e/outgoing/remote-commits.spec.ts (`should fetch the branch of a pull request someone pushed to and name their commit before any push`)
- Pick Remote Commits must take the commits a push would drop into the branch (`bd pick --remote`, [bd
  pick](../cli/pick.spec.md#remote-commits)): fetched again and leased on the remote tip the row shows (a remote that
  moved again picks nothing, `lease_rejected`), picked onto HEAD with the `(branch)` prefix, then a sync; the row's push
  then drops nothing and asks nothing. A `success` toast says "Picked the remote commits of auth" / "1 commit into auth
  · its push drops nothing now", with Undo, which takes HEAD and the files back; a conflict or local changes in the way
  change nothing ("Cannot pick the remote commits of auth", `pick_conflict` with the commit and its files, or
  `uncommitted_overlap`).
  [@test] ../../tests/e2e/outgoing/remote-commits.spec.ts (`should pick the commit someone pushed from the toast, after which the push drops nothing, and undo it`)
- Rust offers it in the branch menu ("Pick Remote Commits…") only while the remote has commits the branch lacks,
  disabled while the branch is parked, blocked, held by a Safe Push or rebuilt by a sync. The item, the action of the
  toast and the next step open the question of the push button, which lists the commits and offers Pick Remote Commits
  beside the Force Push; the description of the push button offers it too wherever it lists dropped commits.
  [@test] ../../crates/domain-policy/src/actions_test.rs (`pick_remote_commits_is_offered_while_the_remote_has_commits_the_branch_lacks`)
  [@test] ../../tests/e2e/outgoing/remote-commits.spec.ts (`should pick the commit someone pushed from the branch menu, through the question of the push button`, `should make the pick the next step of a branch on Space, before its Safe Push, and run it from the slot`)
- The pull request of such a branch must not ask for a push: its details say "Shows d708d5b8; someone else pushed
  commits your branch doesn't have: a push drops them." ([Pull Requests](../branch/pull-requests.spec.md)).
  [@test] ../../src/lib/review-commit.test.ts (`names the commits someone else pushed to the remote branch, which a push would drop, before any push due`)
  [@test] ../../src/lib/pull-request.test.ts (`says that a push drops the commits someone else pushed to the remote branch, never that a push is due`)

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
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should keep the colour of the push while its description is open`)
- A disabled push button is `focusableWhenDisabled` (`aria-disabled`, no `disabled` attribute, which would stop the
  pointer events of the hover) and dimmed by `aria-disabled:opacity-50`; a running push keeps its colour beside the
  spinner, as Update and Sync do.
- The question of a push that drops remote commits is a `destructive` `xs` button with the upload icon and the label
  of the push (`Force Push (1)`), at the start of the line below the dropped commits.

## Testing / Local Run

- `pnpm test:unit` covers the words and the colours of a push (`src/lib/push-summary.test.ts`).
- `pnpm e2e` covers the commits of others on a remote branch (`tests/e2e/outgoing/remote-commits.spec.ts`) against a
  the remote of the template, where the test writes their commit (`commitOnRemote`): the only kind of repository whose
  moved remotes the server fetches.
- `pnpm e2e` covers the push (`tests/e2e/outgoing`). The E2E server has no remote:
  the tests answer `push_branch` in the page (`routePush`) to turn branches into a plain push, a force push, up to
  date or a remote that moved, and read the `expectedRemote` the pushes sent. A result can send progress to the push's
  channel and hold the push until the test releases it; the mock passes the channel's key in `X-Progress-Channel`.
- `pnpm test:backend` covers the push against a bare remote (`crates/sync-core/src/push_test.rs`).

## Open Questions / Risks

- The `primary` blue has a 4.6:1 contrast on the light card, the light tints (`/40`) 1.7:1; the counts and the state
  words carry the values in text.
- Solid blue and solid amber stay far apart for deuteranopia, which keeps the blue–yellow axis (ΔE76 121 in a Machado
  simulation, the light tints 49); the 2px gaps, the state words and the button labels carry the state too.
- No test covers a branch whose remote status is not loaded yet.

## References

- [Remote Status for Virtual Branches](../../docs/design/remote-status.md): how ahead, behind and mine are computed.
- [Branch Preview](../branch/branch-preview.spec.md): the same push button and labels.
- [Landing Overview](landing-overview.spec.md): the header, where the work is on its way into master.
- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
