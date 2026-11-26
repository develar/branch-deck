---
name: Shown Reviews
description: Requirements for a review after landing (show of ship/show/ask), a GitHub pull request from the commit a Safe Push lands into its fork point that can never land, opened as a draft before the start or marked ahead of the landing, opened on whatever landed the branch since, ready with its reviewers asked once the robot merged, the author's move when they request changes, extended by each fix the branch lands after it, closed with its branches once every reviewer it asked approved, and its branches deleted an hour after a close on GitHub.
style: plain-1
targets:
  - crates/domain-policy/src/shown.rs
  - crates/domain-policy/src/land.rs
  - crates/domain-policy/src/actions.rs
  - crates/domain-policy/src/approval_landing.rs
  - crates/domain-policy/src/row_status.rs
  - crates/domain-types/src/code_owners.rs
  - crates/domain-types/src/branch_names.rs
  - crates/codeowners/src/client.rs
  - crates/sync-core/src/review_head.rs
  - crates/domain-policy/src/refresh.rs
  - crates/remote-facts/src/scheduler_github.rs
  - crates/git-ops/src/fix_replay.rs
  - crates/sync-core/src/sync_inputs.rs
  - crates/remote-facts/src/pull_requests.rs
  - crates/remote-facts/src/pull_request_writes.rs
  - crates/repository-model/src/facts.rs
  - crates/repository-model/src/pull_request_facts.rs
  - crates/operations/src/shown.rs
  - crates/operations/src/land_execute.rs
  - crates/bd-remote/src/shown_command.rs
  - crates/bd-remote/src/show_reviewers.rs
  - crates/bd-remote/src/request_review_command.rs
  - crates/bd-remote/src/robot_command.rs
  - src/lib/shown-news.ts
  - src/lib/outgoing-attention.ts
  - crates/remote-facts/src/show_reviewers.rs
  - crates/app-api/src/remote/pull_requests.rs
  - crates/operations/src/land_session.rs
  - src/lib/shown-review.ts
  - src/hooks/useShowReviewers.ts
  - src/presenters/shown.ts
  - src/presenters/row-status.ts
  - src/components/branch/SafePushPopover.tsx
  - src/components/branch/InlineShownReviewers.tsx
  - src/components/branch/InlineCloseShownReview.tsx
  - src/components/shared/ShownBadge.tsx
  - src/components/branch/InlineMarkShownReview.tsx
  - src/hooks/useBranchContextActions.ts
  - crates/repository-model/src/branch_table.rs
  - crates/repository-model/src/action_view.rs
  - src/components/repository/LandedCard.tsx
---

# Shown Reviews

Status: Implemented
Date: 2026-11-24

## Purpose

The monorepo works by ship/show/ask: ship lands without a review, ask is the review before landing in the pull request
of the branch, and show lands first and is reviewed after. A shown review is the show: a GitHub pull request of the
author from `{prefix}/review/<branch>/head`, the commit a Safe Push lands, into `{prefix}/review/<branch>/base`, its
fork point, never master, so it can never land. It opens as a draft before the robot starts, asks its reviewers once
the robot merged, returns to the author when they request changes, takes each fix the branch lands after it on its
head, and closes once every reviewer it asked approved.

## Goals

- A branch lands now and is reviewed after, in the same review window and Your Turn as any pull request.
- Its reviewers are the owner groups of what it changes unless the user names them.
- Nothing of it can land again: its pull request goes into the fork point, and a reviewer is offered no Safe Push of it.

## Non-goals

- A commit-set review of Space: the reviews of the monorepo move to GitHub.
- A follow-up pull request of a fix: a fix lands by its own Safe Push and goes onto the head of the same review.
- A fix reviewed before it lands: it lands first as the branch did, and the review takes it after.
- A Close Review… of a branch that left the table: its "Returned to you" line opens its pull request, where it closes.

## Requirements

### Requirement: Start: A Safe Push with a review after landing opens its draft first

A Safe Push of a branch with a review after landing (`brd safe-push --request-review`) MUST push the branch, then push
the head and the base of its shown review, then open the draft pull request between them (or take the open one of its
head), then start the robot through the API; the draft MUST ask nobody and record in its body the reviewers its open
asks. Only a draft names `draft` in the body of its `POST pulls`; an ordinary pull request sends none.

#### Scenario: The draft before the start
- **WHEN** `brd safe-push auth --request-review` starts
- **THEN** the remote has `{prefix}/review/auth/head` at the commit the robot takes and `/base` at its fork point
- **AND** a draft pull request goes from the head into the base, asks nobody, and its body records the reviewers
- **AND** the robot starts through the API
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_with_a_review_after_landing_opens_its_draft_before_the_start_and_lands_only_the_branch`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved`)
  [@test] crates/remote-facts/src/pull_request_create_test.rs (`only_a_draft_names_draft_in_the_body_of_its_creation`)

### Requirement: Start: A review after landing is refused where it can't review what lands

A Safe Push with a review after landing MUST be refused for its review after landing open already
(`show_review_open`; a draft is started again), more than the branch or a whole stack (`show_lands_stack`), approvals
the code owners give first (`approvals_required`), a repository without Patronus (`show_needs_patronus`), and nobody
to ask (`no_reviewer`). A branch with an open pull request MUST start from its comment, refused as any start from it.

#### Scenario: What refuses it
- **WHEN** the branch has an open review after landing, is stacked on a branch not in master, or needs the approvals
  first
- **THEN** the start is refused before anything is pushed
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_with_a_review_after_landing_opens_its_draft_before_the_start_and_lands_only_the_branch`)

#### Scenario: A branch with an open pull request
- **WHEN** auth has the open pull request #7 and `brd safe-push auth --request-review` starts
- **THEN** the draft opens before the start, and the robot starts from the comment on #7; a draft #7 refuses it
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_with_a_review_after_landing_opens_its_draft_before_the_start_and_lands_only_the_branch`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_check_names_whom_the_review_after_landing_asks_of_the_owner_groups_and_a_branch_with_a_pull_request_starts_from_its_comment`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_review_after_landing_is_offered_as_its_start_refuses_it_and_its_open_review_is_asked_and_closed_from_the_row`)

### Requirement: Reviewers: One available member of each owner group, never the author

Without `--reviewer`, a review after landing MUST ask one reviewer of each owner group of the paths the branch changes
(the `OWNERSHIP` files of the target, the members from the CodeOwners service): from the reviewers of the group when
it names any, else from its members, available, not passed by the rotation, with a GitHub login, never the author; one
chosen for a group covers every other group. The picker of the app MUST start with them (`remote_facts::show_reviewers`).

#### Scenario: The reviewers of a group before its members
- **WHEN** a group names reviewers and members, the author among them
- **THEN** the first available reviewer but the author is asked, and a group of which nobody can be asked is named
  [@test] crates/domain-policy/src/shown_test.rs (`one_available_reviewer_of_each_owner_group_never_the_author`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_check_names_whom_the_review_after_landing_asks_of_the_owner_groups_and_a_branch_with_a_pull_request_starts_from_its_comment`)
  [@test] crates/codeowners/src/client_test.rs (`a_lookup_names_the_reviewers_and_the_members_with_their_logins_and_availability`, `a_group_the_service_lacks_has_no_members_and_a_rejected_token_is_an_error`)

#### Scenario: The lookup as the real service answers it
- **WHEN** the service answers members only, `isReviewer` null, one of them away on vacation, with fields Branch Deck
  doesn't read
- **THEN** the lookup reads, and the member away is not asked
  [@test] crates/codeowners/src/client_test.rs (`a_lookup_of_the_real_shape_has_members_without_a_reviewers_list_and_tells_who_is_away`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_member_away_is_not_asked_of_a_group_as_the_real_service_answers_it`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should start a Safe Push and Request Review… from the popover of S with the code owners of the changes, the draft opened into the fork point before the robot`)

### Requirement: Branches: The head and the base move in one leased atomic push

The branches of a shown review MUST be pushed in one atomic push, each leased on what the remote has (`ls-remote`), so
both are created or moved all or nothing, and nothing is pushed when both are where they go; their delete MUST take
only the ones the remote has, each leased on its tip; they MUST never be an input of a sync.

#### Scenario: A push, a repeat and a delete
- **WHEN** the branches of a review are pushed, pushed again, moved and deleted after one was deleted elsewhere
- **THEN** the first push creates both, the repeat pushes nothing, the move takes the head on, the delete takes the one left
  [@test] crates/sync-core/src/review_head_test.rs (`the_head_and_its_fork_point_go_in_one_push_and_a_repeat_pushes_nothing`, `a_delete_takes_only_the_branches_the_remote_has`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_push_and_a_delete_of_a_shown_review`)
  [@test] crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`)

### Requirement: Listing: The open shown reviews are a search of the listing of their own

The listing of the pull requests MUST list the open shown reviews of the user by a search of their own in the same
query, by the branch each reviews, a head of exactly `{prefix}/review/<branch>/head` of the repository only, each with
the reviewers its body records and what it waits for.

#### Scenario: Only the heads of the namespace
- **WHEN** the search answers a head of the namespace, one that only starts so, another owner's and a closed one
- **THEN** only the first is a shown review, with its recorded reviewers
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_shown_reviews_are_the_heads_of_the_review_namespace_with_the_reviewers_their_body_records`)

### Requirement: Open: Once something landed the branch since the draft opened, the review opens and asks its reviewers

Once something landed the branch since its draft opened (`Open on what landed`), the draft MUST open: its branches go
to what landed and its fork point, it is marked ready, the reviewers its body records are asked, and its body names the
merged pull request where the branch was reviewed before. A review ready already MUST only ask again those who neither
approved nor are asked now; before a landing it MUST be refused (`no_shown_review`). A reviewer the user adds MUST be
asked and recorded.

#### Scenario: The open after the merge
- **WHEN** the robot of the branch merged and the review opens
- **THEN** its head is the commit the robot took, it is ready for review, and its reviewers are asked
  [@test] crates/operations/src/shown_test.rs (`a_draft_opens_on_what_the_robot_landed_and_asks_the_reviewers_its_body_records`, `the_open_and_the_close_of_a_shown_review_against_origin_and_github`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved`)

#### Scenario: Landed with no robot listed, a reviewer added
- **WHEN** the fetched baseline has the branch whole and no robot of it is listed (its robot is older than the
  listing), or the user adds a reviewer (`--reviewer`) to a review open already
- **THEN** the draft opens on the head it has without a push, and the added reviewer is asked and recorded in its
  body, the plan built with them
  [@test] crates/operations/src/shown_test.rs (`a_draft_the_baseline_landed_opens_on_its_head_and_a_reviewer_the_user_adds_is_recorded`)
  [@test] crates/domain-policy/src/shown_test.rs (`a_draft_whose_branch_the_baseline_landed_opens_while_no_robot_of_it_is_listed`)
  [@test] crates/repository-model/src/facts_test.rs (`a_draft_whose_branch_landed_by_content_opens_once_the_robots_are_known_and_none_of_it_is_listed`)

#### Scenario: Reviewed before landing
- **WHEN** a safe-merge from the pull request #7 of auth landed it
- **THEN** the body of the review says "Reviewed before landing in #7.", once
  [@test] crates/operations/src/shown_test.rs (`a_mark_opens_its_draft_and_its_open_says_where_the_branch_was_reviewed_before`)
  [@test] crates/domain-policy/src/shown_test.rs (`the_body_says_once_where_the_branch_was_reviewed_before_it_landed`)

### Requirement: Mark: A review after landing is marked ahead of time and opens once the branch lands

A review after landing MUST be markable ahead of the landing (`Intent::MarkShownReview`): its branches at the tip of
the branch and its fork point, its draft asking nobody, its body recording the reviewers. It MUST be refused as the
Safe Push with the review is, but by nothing of its robot, and by the approvals first only without an open pull
request. Every Safe Push that lands a marked branch MUST move its head to what it lands; a draft MUST be canceled with
its branches.

#### Scenario: Marked, then its plain Safe Push merged
- **WHEN** the user marks auth from its menu with the code owner the picker starts with, then starts its plain Safe
  Push, which merges
- **THEN** the draft opens at the tip of auth into its fork point asking nobody, its row says "Review after landing
  marked: once auth lands, #1 opens and asks AlexPl292", the popover of the Safe Push "#1 opens once it merges, asking
  AlexPl292.", and once it merged the review opens and asks AlexPl292
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should mark auth for a review after landing from its menu, say it in its row and its Safe Push, and open the review with its reviewers once that Safe Push merged`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_mark_before_landing_opens_by_what_lands_the_branch_and_never_by_an_older_robot_of_the_name`)
  [@test] crates/operations/src/shown_test.rs (`a_mark_opens_the_draft_at_the_tip_refused_as_the_safe_push_with_its_review_and_a_landed_branch_opens_it_ready`)
  [@test] crates/domain-policy/src/shown_test.rs (`a_mark_is_refused_as_the_safe_push_with_its_review_without_what_refuses_the_robot`)

#### Scenario: Beside its own pull request, picked again, canceled
- **WHEN** auth has the open pull request #7, the user marks it from "Patronus ›" with alice, picks alice and bob
  again, then cancels the mark
- **THEN** Request Review… stays the one of #7, the record names alice, then alice and bob, nobody is asked, and the
  cancel closes the draft with its comment and deletes its branches
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should mark auth beside the review of its own pull request from the Patronus menu, pick its reviewers again, and cancel the mark with Cancel Review After Landing…`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_review_after_landing_is_offered_as_its_start_refuses_it_and_its_open_review_is_asked_and_closed_from_the_row`)

#### Scenario: Of a draft, of a branch that landed, refused
- **WHEN** auth is marked again with bob, or it landed without a draft, or it lands with login, or it needs the
  approvals of Auth Team without a pull request
- **THEN** the record of its draft names bob, nobody asked; the review opens ready on what landed; the mark is refused
  (`show_lands_stack`, `approvals_required`)
  [@test] crates/operations/src/shown_test.rs (`a_mark_opens_the_draft_at_the_tip_refused_as_the_safe_push_with_its_review_and_a_landed_branch_opens_it_ready`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_review_after_landing_is_offered_as_its_start_refuses_it_and_its_open_review_is_asked_and_closed_from_the_row`)

#### Scenario: A branch that left the table, from the Landed island
- **WHEN** a robot of the user landed feature-auth, the update took it out of the table, and the user presses Request
  Review After Landing… of its row of the Landed island and picks alice
- **THEN** the review opens ready on the commit the robot landed, asks alice, and the row offers it no more
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should open the review after landing of a branch that left the table from its row of the Landed island, asking the reviewers picked there`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_branch_that_landed_and_left_the_table_offers_its_review_after_landing_refused_without_what_it_opens_on`)

#### Scenario: A Safe Push that lands a marked branch
- **WHEN** a Safe Push lands auth, marked: its own, or one of login built on it
- **THEN** it pushes the head of the review of auth to the commit it lands before the start; a Dry Run pushes none, and
  its Safe Push and Request Review… with bob records bob
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_that_lands_a_marked_branch_moves_the_head_of_its_review_to_what_it_lands`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Open on what landed: Only what landed the branch since its review opened opens it

What landed the branch of a draft MUST be, in this order, each only if not older than the draft (`createdAt`): a merged
Safe Push of the branch, on the commit it took, whoever started it; its pull request merged into the target, once the
baseline has the branch whole, on its head; the baseline by content, no robot of it running, on its tip. A robot or a
pull request of an earlier branch of the name MUST open nothing, nor be a fix of the review.

#### Scenario: An older robot, then a safe-merge someone else started
- **WHEN** auth is marked after a robot of an earlier auth merged, then a safe-merge someone else started from its pull
  request #7 merged it
- **THEN** the older robot opens nothing, and once the baseline has auth the review opens on the head of #7 and says
  "Reviewed before landing in #7."
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should open a mark by its pull request merged by a safe-merge someone else started, never by an older robot of auth, and say where auth was reviewed before`)
  [@test] crates/domain-policy/src/shown_test.rs (`a_mark_opens_by_what_landed_its_branch_since_never_by_an_older_robot_or_pull_request_of_the_name`)
  [@test] crates/repository-model/src/facts_test.rs (`a_mark_opens_by_what_landed_its_branch_since_never_by_an_older_robot_of_the_name`)
  [@test] crates/repository-model/src/branch_table_test.rs (`the_robot_sources_are_the_pushed_branches_and_stack_branches_not_in_the_baseline`)
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_shown_reviews_are_the_heads_of_the_review_namespace_with_the_reviewers_their_body_records`)

### Requirement: Extend: A fix that landed goes onto the same review

Once the Safe Push of a fix merged, the open shown review MUST take it: its commits past the target replayed onto the
head the remote has (signed per `commit.gpgsign`), pushed leased on it, the reviewers asked again once GitHub shows it,
then the body records it. A commit that conflicts MUST take its conflicting files as master has them after the fix; a
fix master has under no identity of its commit MUST leave the head, named in a comment once. A repeat MUST add nothing.

#### Scenario: The fix on the head, its reviewers asked again
- **WHEN** a fix of auth merged after alice requested changes
- **THEN** the head of the review has the fix on its commits, alice is asked again, and its body records the fix
  [@test] crates/operations/src/shown_test.rs (`an_extend_adds_the_fixes_asks_the_reviewers_again_and_records_them_last`, `the_extend_of_a_shown_review_against_origin_and_github`)
  [@test] crates/sync-core/src/review_head_test.rs (`a_fix_that_landed_goes_onto_the_head_and_a_repeat_adds_nothing`, `a_fix_on_the_head_is_signed_as_commit_gpgsign_says`, `a_head_the_repository_lacks_is_fetched_first`)
  [@test] crates/sync-core/src/process_budget_test.rs (`an_extend_of_a_shown_review_by_a_fix_that_landed`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should return the review to the author on changes requested, wait while a fix lands, then put the fix onto its head and ask again`)

#### Scenario: A fix that conflicts takes the files as master has them
- **WHEN** master changed the lines of the review after it landed, and a fix of that change merged
- **THEN** the commit that conflicts takes the files git cannot merge as the commit that landed it on master has them
  (paired by the author, the author time and the subject; a rename and a delete too, the `<path>~<side>` copies of a
  file/directory conflict deleted), fetching the target first when it lacks that commit, and the next fix applies on it
- **AND** a comment names the fix and the files once ("Landed a follow-up as <sha>; `auth.rs` also shows what master
  changed in it after this review: <url>"), a repeat adds no commit and no comment
  [@test] crates/sync-core/src/review_head_test.rs (`a_fix_that_conflicts_takes_the_files_of_what_landed_it_the_next_fix_applies_and_a_repeat_adds_nothing`, `a_conflicting_commit_takes_a_rename_and_a_delete_as_master_landed_them`, `a_fix_that_conflicts_finds_what_landed_it_after_the_last_fetch_by_a_fetch_of_the_target`)
  [@test] crates/sync-core/src/process_budget_test.rs (`an_extend_of_a_shown_review_by_a_fix_that_conflicts`)
  [@test] crates/operations/src/shown_test.rs (`the_extend_of_a_shown_review_against_origin_and_github`)
  [@test] crates/domain-policy/src/shown_test.rs (`the_comment_of_a_fix_that_took_files_names_them_as_a_list_of_names_does`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should take the file of a fix that conflicts on the commits of the review as master has it after the fix, and name it in a comment once`)

#### Scenario: A fix that doesn't apply
- **WHEN** the commit of a fix that conflicts has no commit of its identity on master, fetched (an autosquash folded
  it, a reword renamed it)
- **THEN** the head stays where the fix found it, the fixes after it apply all the same, and a comment names the fix
  once ("Landed a follow-up as <sha>; it does not apply on this review's commits: <url>"), a repeat comments nothing
  again
  [@test] crates/sync-core/src/review_head_test.rs (`a_fix_that_master_has_under_no_identity_of_its_commit_does_not_apply_and_the_next_fix_still_does`)
  [@test] crates/operations/src/shown_test.rs (`the_extend_of_a_shown_review_against_origin_and_github`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should comment once on a fix that master has under no identity of its commit, the head as it was`)

### Requirement: State: Approved once every reviewer it asked approved

A shown review MUST be approved once the latest review of every reviewer its body records is an approval and none of
them is asked again; GitHub's `reviewDecision`, a bot and someone not asked MUST decide nothing, and a comment is no
approval.

#### Scenario: The decision of GitHub, a bot and someone not asked
- **WHEN** GitHub says approved after the first of two approvals, and a bot or someone not asked requests changes
- **THEN** the review waits for the second, and neither the bot nor the other holds it back
  [@test] crates/domain-policy/src/shown_test.rs (`approved_once_every_reviewer_it_asked_approved_never_by_github_decision_or_others`, `the_record_of_the_body_names_the_reviewers_it_asks`, `a_draft_waits_for_the_landing_and_one_without_a_record_never_closes`)

### Requirement: State: Changes requested return it to the author

Changes requested by a reviewer a shown review asked MUST make it the author's move: a "Returned to you" line of Your
Turn after the branches of the table, also of a branch that left the table.

#### Scenario: Changes requested
- **WHEN** a reviewer it asked requests changes
- **THEN** the review is returned to the author
  [@test] crates/domain-policy/src/shown_test.rs (`changes_requested_by_a_reviewer_it_asked_return_it_to_the_author`)
  [@test] src/lib/outgoing-attention.test.ts (`names after the rows the reviews after landing a reviewer handed back, also of a branch that left the table`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should return the review to the author on changes requested, wait while a fix lands, then put the fix onto its head and ask again`)

### Requirement: State: A fix that lands is waited for

While a Safe Push of the branch of an open shown review runs, and once it merged until the review took it, the review
MUST wait for the fix (`extending`): it is not the author's move, and it MUST NOT close, whatever its reviews say.

#### Scenario: A fix runs, then merged
- **WHEN** alice requested changes and a fix of the branch runs, then merged
- **THEN** the review is `extending`, no "Returned to you", no close, until its record names the fix
  [@test] crates/domain-policy/src/shown_test.rs (`a_fix_that_merged_after_the_review_opened_waits_until_the_review_took_it`)
  [@test] src/lib/outgoing-attention.test.ts (`names after the rows the reviews after landing a reviewer handed back, also of a branch that left the table`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should return the review to the author on changes requested, wait while a fix lands, then put the fix onto its head and ask again`)

### Requirement: Close: It closes once approved and deletes its branches

A shown review every reviewer approved MUST close with a comment that names who approved it, and its branches MUST be
deleted; a close of one not approved MUST be refused (`review_not_approved`) unless the user says so
(`--allow-unapproved`), and then its comment says whose approval it closed without.

#### Scenario: A close before and after the approvals
- **WHEN** the review is closed while a reviewer waits, then after every reviewer approved
- **THEN** the first close is refused, the second closes it with the comment and deletes both branches
  [@test] crates/domain-policy/src/shown_test.rs (`a_close_waits_for_every_approval_unless_the_user_says_so`)
  [@test] crates/operations/src/shown_test.rs (`a_close_waits_for_every_approval_and_deletes_the_branches_of_the_review`, `the_open_and_the_close_of_a_shown_review_against_origin_and_github`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should close a review after landing every reviewer it asked approved, with its comment, and delete its branches`)

### Requirement: Close: A review closed elsewhere loses its branches

The listing MUST ask the user's shown reviews closed in the last 7 days, at the open of the session and then once an
hour, with the tips each ended at. An hour after its close, while no open
shown review of the branch is listed, the branches the remote still has at those tips MUST be deleted in one atomic
push leased on them (`Intent::DeleteClosedShownReview`); a branch gone or moved since stays. An open of a review whose
base the remote lacks MUST push its branches again.

#### Scenario: Closed on GitHub, a newer review of the branch open
- **WHEN** the review #12 of auth is closed on GitHub while #13 of auth is open on the same branches, then #13 is closed
  too
- **THEN** the branches stay while #13 is open, and once both are closed, an hour after the search of the closed ones
  deletes them, writing nothing to the pull requests
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should delete the branches of a review after landing closed on GitHub an hour after, and keep them while a newer review of the branch is open`)
  [@test] crates/domain-policy/src/shown_test.rs (`a_review_closed_elsewhere_an_hour_ago_loses_the_branches_still_at_its_tips_unless_the_branch_has_an_open_one`)
  [@test] crates/repository-model/src/facts_test.rs (`a_shown_review_closed_elsewhere_is_told_an_hour_after_its_close_once_while_no_open_review_of_its_branch_is_listed`)
  [@test] src/lib/shown-news.test.ts (`deletes the branches of a review after landing closed on GitHub and toasts a failure of it`)

#### Scenario: The search of the closed ones
- **WHEN** the listing asks the closed shown reviews, at the open of the session and an hour after
- **THEN** a head of exactly `{prefix}/review/<branch>/head` of the repository is one, with the tips it ended at and the
  tips of its branches the remote has; a list within the hour asks none
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_closed_shown_reviews_are_a_search_of_their_own_with_what_the_remote_has_of_their_branches`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_closed_shown_reviews_are_asked_by_the_first_list_then_once_an_hour`)
  [@test] crates/domain-policy/src/refresh_test.rs (`the_closed_shown_reviews_are_asked_by_the_first_list_of_the_session_then_once_an_hour`)

#### Scenario: Only the branches at its tips
- **WHEN** a newer review of auth took the head since #12 closed, and later both branches are at the tips of #12
- **THEN** the first delete takes only the base, the second both in one push, a repeat nothing; a review open again
  without its base pushes its branches first
  [@test] crates/sync-core/src/review_head_test.rs (`a_delete_of_a_review_closed_elsewhere_takes_only_the_branches_still_at_its_tips`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_delete_of_a_shown_review_closed_elsewhere`)
  [@test] crates/operations/src/shown_test.rs (`a_review_closed_elsewhere_deletes_the_branches_still_at_its_tips_and_a_reopened_one_without_its_base_pushes_it_again`, `the_branches_of_a_review_closed_on_github_are_deleted_where_they_are_at_its_tips`)

### Requirement: News: The app opens and closes them by the news of the session

The session of a repository MUST tell once a draft whose branch landed (`ShownReviewLanded`), once each fix that landed
after a review opened (`ShownReviewFixLanded`, by its robot) and once a review every reviewer approved
(`ShownReviewApproved`); the page MUST ask the open, the extend and the close of each, and toast what failed. A run of
one that failed MUST be told again at the next answer of the listing or the robots, never at once, three times per
session at most.

#### Scenario: The news of the listing and the robots
- **WHEN** a merged Safe Push of the branch of a draft and an approved review are known
- **THEN** the first is told to open, the second to close, and the page asks each
  [@test] crates/domain-policy/src/shown_test.rs (`a_draft_opens_once_a_safe_push_of_its_branch_merged_and_an_approved_one_closes`)
  [@test] src/lib/shown-news.test.ts (`opens the review after landing whose Safe Push merged, closes the approved one, and toasts what failed`)

#### Scenario: A fix, and a run that failed
- **WHEN** the Safe Push of a fix of an open review merged, and the run of its extend failed
- **THEN** the extend is told once, the page asks it and toasts the failure, and it is told again at the next answer
  [@test] crates/repository-model/src/facts_test.rs (`a_fix_of_a_shown_review_is_waited_for_and_its_merge_is_told_once_and_again_after_a_failed_run`)
  [@test] src/lib/shown-news.test.ts (`adds the fix that landed to its review after landing and toasts a failure of it`)

### Requirement: Reviewer: A shown review gates no Safe Push

A pull request into the base of a shown review (`<prefix>/review/<branch>/base`, any prefix) MUST be told a shown
review: no Safe Push of it is offered (`ActionBlocker::Shown`), its Approve and Request Changes stay, what an approval
of it counts for says that its change is in master already, and its row in Your Turn and Reviews and the review window
carry the badge "Shown" (`ReviewRequest.shown`), its hover that the review gates no Safe Push.

#### Scenario: A shown review to review
- **WHEN** the user reviews a shown review of someone else
- **THEN** no Safe Push is offered, Approve says it gates no Safe Push
  [@test] crates/domain-policy/src/shown_test.rs (`a_shown_review_is_told_by_its_base_of_any_prefix`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_shown_review_is_offered_no_safe_push_and_its_review_stays`)
  [@test] crates/domain-policy/src/approval_landing_test.rs (`an_approval_counts_for_the_safe_push_of_its_own_pull_request_into_the_target_and_a_layer_above_once_it_goes_there`)

#### Scenario: The badge, the Safe Push, the approval
- **WHEN** a review after landing of carol waits for the user's review
- **THEN** its row and its review window carry "Shown", the Safe Push of its menu and its window is disabled with why
- **AND** Approve approves it
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should tell a reviewer a review after landing by its badge, offer no Safe Push of it, and take the approval`)

### Requirement: Row: The review is marked beside the landing of the branch

The Status cell of a branch with an open shown review MUST mark the review last (`RowFact::Shown` with what it waits
for and whom it asks), beside the facts of its landing, never in their place; its words MUST say whose move it is in
each state: the Safe Push that opens it, the mark ahead of the landing and whom it asks, whom it waits for, who
requested changes and how a fix goes into it (a commit `(<branch>) …`, its own Safe Push), the fix that lands, who
approved it.

#### Scenario: A blocked branch with a review after landing
- **WHEN** a blocked branch has a shown review
- **THEN** the block stays the main fact and the review is the last mark
  [@test] crates/domain-policy/src/row_status_test.rs (`a_shown_review_is_marked_beside_the_landing_never_in_its_place`)

#### Scenario: The words of each state
- **WHEN** the review is a draft whose Safe Push runs, a draft marked ahead of time asking alice, waits for alice and
  bob, has changes requested by alice, waits for a fix, or alice approved it
- **THEN** the mark says "auth is landing: once its Safe Push merges, #12 opens and its reviewers are asked", "Review
  after landing marked: once auth lands, #12 opens and asks alice", "In review after landing: waits for alice and bob in
  #12", "alice requested changes in #12: commit a fix as (auth) …, and its Safe Push adds it to the review", "A fix of
  auth is landing: once its Safe Push merges, it goes into #12 and the reviewers are asked again", "Approved by alice:
  #12 closes and its branches are deleted; the change stays in master"
  [@test] src/presenters/row-status.test.ts (`marks the review after landing beside what landed the branch, its hover whose move it is in each state`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should return the review to the author on changes requested, wait while a fix lands, then put the fix onto its head and ask again`)
  [@test] src/presenters/shown.test.ts (`say what a mark ahead of the landing opens and whom, its draft picked again, its cancel and the Safe Push of it`)

### Requirement: App: Safe Push and Request Review… picks the reviewers beside the start

The app MUST offer Safe Push and Request Review… where a Safe Push of Patronus lands into a GitHub repository
("Patronus ›", the palette, the Safe Push popover), disabled as its start would refuse it. Its popover MUST hold a
picker that starts with whom `brd` would ask (`get_show_reviewers`), where the user removes one or adds a GitHub login;
its start MUST plan the Safe Push with them (`Land.requestReview`), none refusing it (`noReviewer`).

#### Scenario: From the popover of S
- **WHEN** S opens the popover of auth and the user presses Safe Push and Request Review… in it
- **THEN** the picker holds the one available member of Auth Team, the one away on vacation skipped
- **AND** its start pushes the branches of the review, opens the draft from the head into the base, and starts the
  robot through the API
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should start a Safe Push and Request Review… from the popover of S with the code owners of the changes, the draft opened into the fork point before the robot`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_review_after_landing_is_offered_as_its_start_refuses_it_and_its_open_review_is_asked_and_closed_from_the_row`)

#### Scenario: From the menu, the reviewers picked
- **WHEN** the user starts it from "Patronus ›", removes the code owner and adds alice by the search
- **THEN** without a reviewer the start is disabled ("Add a reviewer: a review after landing asks at least one")
- **AND** the draft records alice, and once the robot merged the review opens with alice asked
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should start it from the Patronus menu with the reviewers the user picks, and open the review with them once the robot merged`)
  [@test] src/presenters/shown.test.ts (`say whom the picker starts with, of which group nobody can be asked, and why it starts empty`)

### Requirement: App: Request Review… asks more reviewers of an open review after landing

A branch whose review is the one after landing MUST offer Request Review… of it in place of the request of a pull
request (a form under its row with the search of the users of the repository): the reviewers the user adds MUST be
asked and recorded in its body (`Intent::OpenShownReview { reviewers }`); of a draft they MUST only be recorded, its open
asks them with the others.

#### Scenario: Bob added
- **WHEN** the review asks alice and the user adds bob in Request Review… of the row
- **THEN** bob is asked, and the record of its body names alice and bob
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should ask more reviewers with Request Review… of an open review after landing, and close it unapproved with Close Review… after its confirmation`)
  [@test] crates/operations/src/shown_test.rs (`a_draft_the_baseline_landed_opens_on_its_head_and_a_reviewer_the_user_adds_is_recorded`)

### Requirement: App: Close Review… closes it after a confirmation, approved or not

A branch whose review is the one after landing MUST offer Close Review… of it, a confirmation under its row that says
what it closes without (the approvals it waits for, the changes requested, a fix that lands, a draft that asked nobody)
or who approved it; its Close Review MUST close it with its comment and delete its branches, approved or not
(`allow_unapproved`).

#### Scenario: Closed while a reviewer waits
- **WHEN** the review waits for alice and bob, and the user confirms Close Review…
- **THEN** it is closed with "Closed without the approval of …. The change is in master." and its branches are deleted
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should ask more reviewers with Request Review… of an open review after landing, and close it unapproved with Close Review… after its confirmation`)
  [@test] src/presenters/shown.test.ts (`confirm a close by what the review waits for, closed anyway without the approvals or the fix`)

## User Experience

- `brd safe-push <branch> --request-review [--reviewer <login>]`: `shown` names `reviewers`, `groups`, `head`, `base`,
  `number`; `brd request-review <branch> [--after-landing]`, before the landing `shown.marked`, whose `shown.extended`
  names the fixes it added (`fixes`, `head`, `not_applied`, `took_files`); `brd robot <branch>`: `extends`; `brd close-review <branch> [--allow-unapproved]`, of a review closed on GitHub
  `state: closed` and `deleted`.
- The Status cell: "review after landing", "review after landing marked", "in review after landing", "changes requested
  after landing", "fix goes into review after landing", "approved after landing"; the hover what happens next: "auth is
  landing: once its Safe Push merges, #12 opens and its reviewers are asked", "Review after landing marked: once auth
  lands, #12 opens and asks alice", "In review after landing: waits for alice in #12", "alice requested
  changes in #12: commit a fix as (auth) …, and its Safe Push adds it to the review", "A fix of auth is landing: once its
  Safe Push merges, it goes into #12 and the reviewers are asked again", "Approved by alice: #12 closes and its branches
  are deleted; the change stays in master".
- The menu of the branch: "Safe Push and Request Review…" and "Request Review After Landing…" in "Patronus ›" (also
  "Review after landing: Safe Push and Request Review…" in the popover of its Safe Push); "Request Review After
  Landing…" beside "Request Review…" (of its own pull request, when it has one); of a branch with an open one, "Open Pull
  Request #12", "Request Review…", "Close Review…"; of a marked one, "Request Review After Landing…" (its reviewers
  picked again) and "Cancel Review After Landing…"; a row of the Landed island of a branch that left the table, the
  button "Request Review After Landing…", disabled while nothing tells the commit that landed it ("Nothing tells the
  commit that landed feature-auth: its robot and its pull request are not listed"). The form of the mark: "Request review after landing of auth", "A
  draft pull request opens now and asks nobody. Once auth lands, by its Safe Push, a Safe Push that lands it with more,
  or a safe-merge from its pull request, it opens on what landed and asks these reviewers.", of a draft "Request review
  after landing on #12", "Once auth lands, #12 opens and asks the reviewers you pick here; nobody is asked before.", the
  picker with the code owners of the changes first, its submit "Request Review After Landing", the toast "Review after
  landing of auth requested" ("It opens once auth lands and asks alice."). The confirmation of the cancel "Cancel Review
  After Landing #12?", "#12 closes on GitHub with a comment and its branches are deleted; once auth lands, nothing opens
  and nobody is asked.", its submit "Close Draft #12", the toast "Canceled review after landing #12 of auth". The
  popover of the Safe Push of a marked branch: "#12 opens once it merges, asking alice." The popover: "auth lands now; the reviewers are asked once it is in master. The review closes when they
  all approve; it can never land itself.", the picker, "One available code owner of Auth Team.", the readiness "No
  code-owner approval needed before landing.", no whole stack beside it, the start "Safe Push and Request Review";
  without a reviewer "Add a reviewer: a review after landing asks at least one". The confirmation of Close Review…
  "Close Review #12?", what closing now leaves without a review ("alice has not approved #12 yet; closing now ends the
  review without that approval.", "A fix of auth is still landing and is not in #12 yet; closing now leaves it
  unreviewed."), then "#12 closes on GitHub with a comment and its branches are deleted; the change stays in master.";
  the toasts "Asked bob to review #12 after landing", "Closed review after landing #12 of auth".
- A reviewer's row in Your Turn and Reviews and the header of the review window: the badge "Shown", its hover "Already
  in master: your review does not block landing. The review closes when every requested reviewer approves."; the
  author's "Returned to you" line of it the same badge, its hover "Already in master: reviewed after landing. A fix of
  auth goes into #12 by its own Safe Push."
- The comment of a fix that doesn't apply: "Landed a follow-up as <sha>; it does not apply on this review's commits:
  <the robot page>"; of one whose commits took files as master has them: "Landed a follow-up as <sha>; `auth.rs` also
  shows what master changed in it after this review: <the robot page>" (up to four files, from five on the first three
  and the count); of a canceled mark: "Canceled: this review after landing won't open."; the body of a review whose
  branch was reviewed in its pull request before it landed: "Reviewed before landing in #7."
- Your Turn: a review after landing a reviewer handed back is a "Returned to you" line.
- The review window of a reviewer: "Already in master: your review does not block landing; the review closes when every
  requested reviewer approves"; the Safe Push of it disabled: "#12 is a review after landing: its change is in master
  already, so there is nothing to Safe Push".
- The toasts of a failure: "Cannot open the review after landing of auth", "Cannot add the fix to the review after
  landing of auth", "Cannot close the review after landing of auth", "Cannot request the review after landing of auth",
  "Cannot delete the branches of the closed review after landing of auth".

## References

- [Landing](../../../../docs/design/landing.md) ("Show: a review after landing")
- [Git Operations](../../../../docs/design/git-operations.md) ("Review head")
