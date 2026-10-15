---
name: Outgoing Commits
description: Requirements for the header indicator of the commits waiting to be pushed and its overview popover, with a row per branch and the push button of the branch row.
style: plain-1
targets:
  - ../../src/components/repository/OutgoingIndicator.tsx
  - ../../src/components/repository/OutgoingPopover.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/components/branch/DroppedCommitList.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/hooks/usePushSummary.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/hooks/useOutgoingAttention.ts
  - ../../src/hooks/useLandedIn.ts
  - ../../src/lib/push-summary.ts
  - ../../src/lib/ui-types.ts
  - ../../src/stores/push-confirm.ts
  - ../../src/lib/remote-status.ts
  - ../../src/lib/outgoing-attention.ts
  - ../../crates/sync-core/src/remote_status.rs
  - ../../crates/sync-core/src/commit_key.rs
  - ../../crates/sync-core/src/push.rs
  - ../../crates/sync-core/src/push_lease.rs
  - ../../src/lib/git/push.ts
  - ../../src/lib/toast-message.ts
  - ../../crates/sync-core/src/transfer_progress.rs
  - ../../src/lib/transfer-progress.ts
  - ../../src/stores/commit-move.ts
---

# Outgoing Commits

Status: Implemented
Date: 2026-10-15

## Summary

The header shows how many commits wait to be pushed, over all virtual branches, next to a tiny bar with a segment per
branch. Hovering or clicking it opens an overview: a row per branch with the size of its push, what the push does and
the push button of the branch row.
The overview answers "what is my work waiting on" without scanning the branch table.

## Goals

- See at a glance that something is not pushed, and how much.
- Tell a plain push from a first push and from a force push before pushing.
- Push a branch, or go to its row, from the overview.

## Non-goals

- There is no "Push all": a push stays deliberate and per branch, or per stack from a stacked branch whose ancestors
  have work for the remote ([Stacked Branches](../branch/stacked-branches.spec.md#pull-requests)); the description of
  its button names each push.
- The overview does not fetch: it shows the remote status of the last sync or push.
- The indicator does not count the commits the remote has and the branch lacks; a force push drops or replaces them,
  and its row says how many.

## Requirements

### What is outgoing

The sync rebuilds a virtual branch on the latest baseline, so its hashes change while its work stays the same. What
is outgoing compares content, not hashes: the question is whether the remote has the user's work as it is now.

- A branch is outgoing when its row offers a push that is not disabled as up to date: the branch synced without an
  error, has commits, and its remote status is loaded and not up to date.
- The commits of a push must be the commits whose content the remote lacks. The baseline commits must not count (the
  remote has them), nor the rebased copies of pushed commits (patch-equivalent). A branch not on the remote counts all
  its commits, the baseline excluded; mine counts only the user's commits, also for a first push.
  [@test] ../../src/lib/push-summary.test.ts (`counts the commits whose content the remote lacks`)
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_new_branch_counts_commits_by_author`)
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_new_commit_after_baseline_merge`)
- A branch only rebased onto a newer baseline must be up to date: the remote has all its commits by content. Its row
  says so, and it is not outgoing.
  [@test] ../../src/lib/push-summary.test.ts (`is up to date for a branch only rebased onto a newer baseline: the remote has its work`)
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_rebased_branch_shows_zero_unpushed`)
- A branch with a conflict, a sync error or no commits must not count; a branch whose remote status is not loaded yet
  must not count either, but the overview says how many there are.
  [@test] ../../src/lib/push-summary.test.ts (`skips the branches whose row offers no push and counts the ones not loaded`)
- A branch the fetched baseline has all of ("landed", [Landed Commits](landed-commits.spec.md#before-the-update)) must
  not count: the update archives it, nothing of the user's is left to push. Its push button stays, quiet.
  [@test] ../../src/lib/push-summary.test.ts (`skips a branch the fetched baseline has all of: the update archives it`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
- The state of a branch must follow the label of its push button:
  - "create" for a branch not on the remote (`Create Remote (3)`);
  - "force" for a push that is not a fast-forward: the remote has commits the branch lacks, older versions of commits
    amended or restacked (below), or the branch was rebased onto a newer baseline (`Force Push`). It follows the
    remote branch and the local one, not the result of the last sync, so it stays the same across syncs and restarts;
  - "push" otherwise (`Push (2)`).
  [@test] ../../src/lib/push-summary.test.ts (`follows the rules of the push button label`, `force pushes a branch whose remote has older versions of commits amended or restacked`)
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_rebased_branch_with_new_commit_is_not_fast_forward`)
- A remote commit whose content the branch lacks must be told by its identity, the author email, the author time and
  the subject without the `(prefix)` (as [landed commits](landed-commits.spec.md) are): amended when an unpushed
  commit of the branch has it (an older version of a commit amended here) and makes other changes, a rebased copy
  counted nowhere when it makes the same changes on lines that moved around them, restacked when a commit of a branch
  it is built on has it (the parent was amended or rebuilt since the branch was pushed: the child's remote branch has
  the parent's commits as they were), dropped otherwise. Only the dropped ones are behind (`commitsBehind`,
  `droppedCommits`): a push replaces the older versions and drops the rest. Another author's commit is dropped, also
  with the subject and the time of a commit of the branch. The sync knows the commits of the ancestors; a push
  outside it reads them from the parent's virtual branch, only while a remote commit matches nothing of the branch.
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`an_amended_commit_is_amended_not_dropped`, `a_rebased_copy_whose_context_moved_is_not_amended`, `a_child_of_an_amended_parent_is_restacked`, `another_authors_commit_stays_dropped`, `an_amended_commit_and_a_dropped_one_are_told_apart`)
- A branch is up to date only when the remote has nothing to receive and nothing the branch lacks, older versions
  included (`isRemoteUpToDate`); the push button, the status text and the review status of a robot or a pull request
  decide by it.
  [@test] ../../src/lib/push-summary.test.ts (`force pushes a branch whose remote has older versions of commits amended or restacked`)
  [@test] ../../src/lib/review-commit.test.ts (`compares with the remote branch and the remote status by content, not with the hash of the local tip`)
- The remote status must name the remote commit it was computed against (`remoteTip`, null without a remote branch)
  and list the newest ten of the commits a push drops (`droppedCommits`: id, subject, author, time), newest first;
  `commitsBehind` counts them all. A push leases on that commit ([Push](#push)).
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_dropped_commits_are_listed_newest_first_up_to_the_limit`)
- The total and the mine counts must sum the outgoing branches; "mine" uses the remote status count of the user's
  commits.
  [@test] ../../src/lib/push-summary.test.ts (`sums the outgoing branches in order and counts them by state`)
- The count of a push must read `3↑` when its commits are all the user's, and `mine (total)↑` only when some are
  others' (`1 (3)↑`; `0 (3)↑` when none is theirs), then `· 1 amended` for the older versions of commits amended
  here, `· restacked` for those of the branches it is built on, `· 2↓` for the remote commits it drops, in that
  order; a push that sends no commit starts with the next part (`2↓`, `restacked`, `1 amended`). The status text of
  the branch row and the overview row read the same.
  [@test] ../../src/lib/push-summary.test.ts (`reads as the status text of a branch row, the total in parentheses only when some commits are others'`, `names the older versions a push replaces after the commits it sends, before the ones it drops`)
  [@test] ../../src/lib/remote-status.test.ts (`repeats the count in parentheses only when some commits to push are others'`, `says that the remote has older versions of commits amended here or of the parent's, not commits the branch lacks`)

### Indicator

- The indicator is a ghost button in the header, before the time of the last sync: an up arrow, the total commits and
  a bar.
- Its accessible name must be the title of the overview, e.g. "12 commits to push in 7 branches", with ", 1 needs
  attention" while the attention dot shows.
- The bar must have a segment per outgoing branch, in the order of the rows, its width proportional to the commits of
  the branch; a force push without commits keeps a segment of one commit.
- A segment must be stacked: the user's commits solid, the others' commits light, as `1 (3)↑` reads. The one commit of
  a force push without commits is solid.
  [@test] ../../src/lib/push-summary.test.ts (`has a segment per branch, a force push without commits still visible`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show the commits to push with a bar segment per branch`)
- With more than 8 outgoing branches, the bar must have a segment per hue instead, so the segments stay readable.
  [@test] ../../src/lib/push-summary.test.ts (`has a segment per hue for many branches`)
- The indicator must be hidden when nothing is outgoing, also right after the last push, unless a pull request is open
  ([Pull Requests](../branch/pull-requests.spec.md)).
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should hide when every branch is pushed`)
- A branch that waits for its author, an open pull request with changes requested or failing checks
  ([Pull Requests](../branch/pull-requests.spec.md)) or a robot that failed on the branch as it is, also rebuilt on a
  newer baseline since ([Patronus](../branch/patronus.spec.md)), must put a `destructive` dot on the icon of the indicator. The overview
  explains it with a line per such branch, in the table order, below its title: "ABC-123 needs attention: changes
  requested, failing checks", "ABC-123 needs attention: failed Safe Push".
  [@test] ../../src/lib/outgoing-attention.test.ts (`names what each branch waits for, in the table order, a branch once`, `adds how many branches need attention to the title of the overview`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the chips in a column and a dot for a pull request that waits for the author`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the chips in a column and a dot for a robot that failed on the branch tip`)
- It must follow every sync and push result without a reload.

### Overview

- Hover (after a short delay) or a click must open the overview; it is loaded while the hover delay runs.
- The title must say "12 commits to push in 4 branches"; a muted line below says how many of them are the user's when
  that differs, and how many branches are not checked yet.
- The overview must list a row per outgoing branch in the order of the branch table, a branch nested under its issue
  where the table shows it.
- A row must show the branch name, a bar proportional to the largest push, the count as the status text of the branch
  row reads it (`3↑`, `1 (3)↑ · 2↓`, [What is outgoing](#what-is-outgoing)), the state word ("new" for a first push,
  "force" for a force push, none for a plain push) and the push button of the branch row.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should list the outgoing branches in the overview on hover`)
- A row bar must be stacked as the segments of the indicator.
- A legend must name the hues present ("push" for a safe push, "drops remote commits"), and, when some commits are not
  the user's, the solid "yours" and the light "others'".
  [@test] ../../src/lib/push-summary.test.ts (`has a swatch per colour present`)
- Below the rows, after a rule, the overview shows the update state of the baseline with an Update button
  ([Update Project](update-project.spec.md)).
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show the update state in the footer of the outgoing overview`)
- A click on a branch name must close the overview, expand the branch row and scroll it into view.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show and expand the row of a branch clicked in the overview`)
- The push button must push as the button of the row does, with the same description and disabled states, but with
  its label (the row's compact button shows only the arrow and the count, [Branch Row](../branch/branch-row.spec.md#actions-cell))
  and in the text colour: the bar of the row carries the colour, and a column of coloured buttons would only repeat it.

### Push colours

One rule colours a push wherever it shows: the push button and the status text of a branch row, the segments of the
indicator and the row bars of the overview. The hue says how risky the push is, the lightness whose commits it sends.

- The hue must follow the risk of the push: `warning` when it drops remote commits the branch lacks (`· N↓`), `primary`
  otherwise. A force push of a rebased branch only replaces the remote commits with their rebased copies, one of an
  amended or restacked branch their older versions: it is safe, `primary`, and keeps the `Force Push` label, which
  says what git does.
  [@test] ../../src/lib/push-summary.test.ts (`takes the hue from the risk: warning only for a push that drops remote commits`)
- Up to date is neutral. A safe push of no commit of the user is neutral too (muted button, grey status dot): there is
  nothing of theirs to push. A first push and a push that drops remote commits keep their hue whoever authored the
  commits, and so does a push that replaces older versions (`restacked` with nothing of its own to push): the remote
  branch is behind the user's work.
  [@test] ../../src/lib/push-summary.test.ts (`is neutral for a safe push of no commit of the user`, `keeps the push hue for a restacked branch with no commit of the user to push`)
- The status text of a push state is muted text with a dot of the push colour, never a badge: a force push after a sync
  is routine. Only an error or a conflict is a badge.
- The overview must stay open across a push and follow its result: a pushed branch leaves the list, a branch whose
  remote diverged turns into a force push.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should push a branch from the overview and follow the push results`)

### Push

A push replaces exactly the remote state the user saw: the row's remote status, not Branch Deck's last push.

- A push label must count the commits the push sends, whoever authored them: `Push (2)`, `Create Remote (3)`,
  `Force Push (1)`; a force push that sends none is `Force Push`. The user's share is in the description: "1 commit to
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
  is a step of another action (a Safe Push start, a merge request, Push and Restart, [Patronus](../branch/patronus.spec.md))
  has none, and a push of several branches of a stack is one toast
  ([Stacked Branches](../branch/stacked-branches.spec.md#pull-requests)).
  [@test] ../../src/lib/push-summary.test.ts (`says what the push did from the status it was made against`, `says that a push of an amended or restacked branch replaced the older versions, not that it dropped them`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should push a branch from the overview and follow the push results`)
- A push must send the remote head of the row's remote status (`expectedRemote`, null when the row showed no remote
  branch) and lease on it. A Force Push drops the commits the row listed and nothing else, also when someone else's
  commit was fetched after Branch Deck's last push; a plain push of a branch that took that commit in goes through.
  [@test] ../../crates/sync-core/src/push_test.rs (`a_force_push_drops_the_remote_commit_the_user_saw`, `a_plain_push_with_the_seen_head_replaces_a_fetched_remote_commit`, `a_rebuilt_branch_replaces_the_remote_tip_it_saw`, `a_branch_not_on_the_remote_is_created_when_expected_absent`)
  [@test] ../../crates/sync-core/src/push_lease_test.rs (`an_expected_lease_is_the_remote_tip_the_caller_saw`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)
- A remote branch that moved since the row's status (a newer commit, a branch created or deleted elsewhere) must not
  be overwritten: nothing is pushed, the branch alone is fetched, and its row shows the fetched remote status. A
  warning toast stays until the user acts: "login changed on the remote", "3 commits you don't have (by Ann and Bob).
  Nothing was pushed.", with the push the row now offers ("Force Push", "Push", "Create Remote") as its action,
  leased on the fetched head; a push that drops the commits the remote has now confirms first, as below, and it asks
  before a push into a running Safe Push as any push does ([Patronus](../branch/patronus.spec.md)). A stack push stops
  at that branch.
  [@test] ../../crates/sync-core/src/push_test.rs (`a_remote_that_moved_since_the_status_is_fetched_not_overwritten`, `a_branch_created_on_the_remote_meanwhile_is_fetched_not_overwritten`, `a_branch_deleted_on_the_remote_is_shown_as_not_pushed`)
  [@test] ../../src/lib/remote-status.test.ts (`says how many commits the remote has that the branch lacks, and by whom; the action is a force push`, `offers the push the button would show when the remote lacks nothing of the branch`, `offers to create a branch deleted on the remote`, `names the distinct authors in the order of the commits`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show what the remote has when it moved since the status, and force push over it from the toast`)
- The description of a push that drops remote commits must list them under its text, newest first: subject, author,
  short id and age, then "and N more" past the ten listed.
  [@test] ../../src/hooks/usePushButton.test.ts (`lists the remote commits a force push drops under its description, with how many more there are`, `lists nothing for a push that drops no remote commit`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)
- A push that drops remote commits (the `warning` hue, `· 2↓`), someone else's work, must ask first: the click opens its
  description with the dropped commits and a `Force Push (1)` button below them, which pushes. Leaving the button keeps
  the question open; Escape, a click elsewhere or on the push button again dismisses it, and nothing is pushed. A safe
  push (Push, Create Remote, the Force Push of a rebased, amended or restacked branch) pushes on the click. The action
  of the remote-moved toast and P in the [branch preview](../branch/branch-preview.spec.md) ask the same way: the toast
  opens the question of the row's push button (scrolled into view), P that of the preview's footer.
  [@test] ../../src/hooks/usePushButton.test.ts (`asks first only for a push that drops remote commits`, `says what a force push of an amended or restacked branch replaces, and pushes it on the click in the push colour`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`, `should show what the remote has when it moved since the status, and force push over it from the toast`, `should push a rebased branch at once: its force push drops nothing`)
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should ask before P force pushes over a commit the branch lacks`)
- The description must open on hover also while the button is disabled: "Everything is pushed.", why a blocked
  stacked branch can't push, the progress of a running push.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should describe a disabled push on hover`)
- A push the remote refuses while the lease holds (a hook, a protected branch) must be an error.
  [@test] ../../crates/sync-core/src/push_test.rs (`a_push_the_remote_refuses_for_another_reason_is_an_error`)
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

## User Experience

- The indicator button is `ghost` `sm`, the count in tabular figures, the bar `w-16 h-1.5`. The attention dot is
  `size-1.5 rounded-full bg-destructive` on the top end corner of the icon (`absolute -top-0.5 -right-0.5`), so the
  count and the bar keep their place; the parts are spaced by gaps, not margins.
- The colours are the semantic tokens `bg-primary` and `bg-warning` for the user's commits, `bg-primary/40` and
  `bg-warning/40` for the others'. `info` is not a push colour. The segments of the branches are separated by a 2px gap
  of the surface, the two parts of a segment touch; both are rounded by `rounded-sm`, derived from `--radius`.
- Text never takes a state colour: the counts, the state words and the legend use `text-muted-foreground`; the colour
  is carried by the bars and the legend swatches.
- The overview is a `Popover` as wide as its content up to `max-w-2xl`, aligned to the end of the button. The rows form a
  grid, so the bars and the counts line up; the name column is `minmax(8rem,max-content)`, so a name is truncated only
  past 8rem, never to a letter; a row bar sits on a `bg-muted` track.
- The chips column is at least `min-w-16` and its chips are aligned to its end, so the rows don't move when the chips
  load; the push buttons are aligned to the end of the rows.
- The branch name is a plain button that underlines on hover; the muted "on login" of a stacked branch follows it after
  a gap and is never truncated.
- The attention lines are muted `text-xs`, each after a `destructive` dot as the indicator's.
- The footer lines wrap instead of truncating: an update state or a hint is read in full.
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

- `pnpm test:unit` covers the summary (`src/lib/push-summary.test.ts`).
- `pnpm e2e` covers the indicator, the overview and the push (`tests/e2e/outgoing`). The E2E server has no remote:
  the tests answer `push_branch` in the page (`routePush`) to turn branches into a plain push, a force push, up to
  date or a remote that moved, and read the `expectedRemote` the pushes sent. A result can send progress to the push's
  channel and hold the push until the test releases it; the mock passes the channel's key in `X-Progress-Channel`.
- `pnpm test:backend` covers the push against a bare remote (`crates/sync-core/src/push_test.rs`).

## Open Questions / Risks

- The `primary` blue has a 4.6:1 contrast on the light card, the light tints (`/40`) 1.7:1; the counts and the state
  words carry the values in text.
- Solid blue and solid amber stay far apart for deuteranopia, which keeps the blue–yellow axis (ΔE76 121 in a Machado
  simulation, the light tints 49); the 2px gaps, the state words and the button labels carry the state too.
- No test covers a branch whose remote status is not loaded yet, or the bar of more than 8 branches in the page.

## References

- [Remote Status for Virtual Branches](../../docs/design/remote-status.md): how ahead, behind and mine are computed.
- [Branch Preview](../branch/branch-preview.spec.md): the same push button and labels.
- [Pull Requests](../branch/pull-requests.spec.md): the chip column, the attention dot and the pull request footer line.
- [Patronus](../branch/patronus.spec.md): the robot chip in the same column, the attention dot of a failed robot and the
  Patronus footer line.
- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
