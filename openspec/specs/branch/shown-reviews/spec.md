---
name: Shown Reviews
description: Requirements for a review after landing (show of ship/show/ask), a GitHub pull request from the commit a Safe Push lands into its fork point that can never land, opened as a draft before the start, ready with its reviewers asked once the robot merged, the author's move when they request changes, and closed with its branches once every reviewer it asked approved.
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
  - crates/sync-core/src/sync_inputs.rs
  - crates/remote-facts/src/pull_requests.rs
  - crates/remote-facts/src/pull_request_writes.rs
  - crates/repository-model/src/facts.rs
  - crates/operations/src/shown.rs
  - crates/operations/src/land_execute.rs
  - crates/bd-remote/src/shown_command.rs
  - crates/bd-remote/src/show_reviewers.rs
  - crates/bd-remote/src/request_review_command.rs
  - src/lib/shown-news.ts
  - src/lib/outgoing-attention.ts
---

# Shown Reviews

Status: Implemented
Date: 2026-11-24

## Purpose

The monorepo works by ship/show/ask: ship lands without a review, ask is the review before landing in the pull request
of the branch, and show lands first and is reviewed after. A shown review is the show: a GitHub pull request of the
author from `{prefix}/review/<branch>/head`, the commit a Safe Push lands, into `{prefix}/review/<branch>/base`, its
fork point, never master, so it can never land. It opens as a draft before the robot starts, asks its reviewers once
the robot merged, returns to the author when they request changes, and closes once every reviewer it asked approved.

## Goals

- A branch lands now and is reviewed after, in the same review window and Your Turn as any pull request.
- Its reviewers are the owner groups of what it changes unless the user names them.
- Nothing of it can land again: its pull request goes into the fork point, and a reviewer is offered no Safe Push of it.

## Non-goals

- A commit-set review of Space: the reviews of the monorepo move to GitHub.
- A fix of a review added to the same pull request: the next step (a fix lands as a new Safe Push of the branch).
- The app's start of a review after landing (its popover and picker of the reviewers): the next step.

## Requirements

### Requirement: Start: A Safe Push with a review after landing opens its draft first

A Safe Push of a branch with a review after landing (`brd safe-push --request-review`) MUST push the branch, then push
the head and the base of its shown review, then open the draft pull request between them (or take the open one of its
head), then start the robot through the API; the draft MUST ask nobody and record in its body the reviewers its open
asks.

#### Scenario: The draft before the start
- **WHEN** `brd safe-push auth --request-review` starts
- **THEN** the remote has `{prefix}/review/auth/head` at the commit the robot takes and `/base` at its fork point
- **AND** a draft pull request goes from the head into the base, asks nobody, and its body records the reviewers
- **AND** the robot starts through the API
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_with_a_review_after_landing_opens_its_draft_before_the_start_and_lands_only_the_branch`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved`)

### Requirement: Start: A review after landing is refused where it can't review what lands

A Safe Push with a review after landing MUST be refused when the branch has an open pull request (`pull_request_open`:
its review is there), when it lands more than the branch or a whole stack (`show_lands_stack`), when the code owners
must approve the change first (`approvals_required`), when the repository lands without Patronus
(`show_needs_patronus`), and when nobody is to be asked (`no_reviewer`).

#### Scenario: What refuses it
- **WHEN** the branch has an open pull request, is stacked on a branch not in master, or needs the approvals first
- **THEN** the start is refused before anything is pushed
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_with_a_review_after_landing_opens_its_draft_before_the_start_and_lands_only_the_branch`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_check_names_whom_the_review_after_landing_asks_of_the_owner_groups_and_refuses_a_branch_with_a_pull_request`)

### Requirement: Reviewers: One available member of each owner group, never the author

Without `--reviewer`, a review after landing MUST ask one reviewer of each owner group of the paths the branch changes
(the `OWNERSHIP` files of the target, the members from the CodeOwners service): from the reviewers of the group when
it names any, else from its members, available, not passed by the rotation, with a GitHub login, never the author; one
chosen for a group covers every other group that has them.

#### Scenario: The reviewers of a group before its members
- **WHEN** a group names reviewers and members, the author among them
- **THEN** the first available reviewer but the author is asked, and a group of which nobody can be asked is named
  [@test] crates/domain-policy/src/shown_test.rs (`one_available_reviewer_of_each_owner_group_never_the_author`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_check_names_whom_the_review_after_landing_asks_of_the_owner_groups_and_refuses_a_branch_with_a_pull_request`)
  [@test] crates/codeowners/src/client_test.rs (`a_lookup_names_the_reviewers_and_the_members_with_their_logins_and_availability`, `a_group_the_service_lacks_has_no_members_and_a_rejected_token_is_an_error`)

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

### Requirement: Open: Once the robot merged, the review opens and asks its reviewers

Once a merged Safe Push landed the branch, its draft shown review MUST open: its branches go to the commit the robot
landed and its fork point, it is marked ready for review, and the reviewers its body records are asked; a review ready
already MUST only ask again those it asked who neither approved nor are asked now; before such a merge it MUST be
refused (`no_shown_review`).

#### Scenario: The open after the merge
- **WHEN** the robot of the branch merged and the review opens
- **THEN** its head is the commit the robot took, it is ready for review, and its reviewers are asked
  [@test] crates/operations/src/shown_test.rs (`a_draft_opens_on_what_the_robot_landed_and_asks_the_reviewers_its_body_records`, `the_open_and_the_close_of_a_shown_review_against_origin_and_github`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`the_draft_opens_before_the_start_its_reviewers_are_asked_once_it_merged_and_it_closes_once_they_approved`)

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

### Requirement: News: The app opens and closes them by the news of the session

The session of a repository MUST tell once a draft whose branch a merged Safe Push landed (`ShownReviewLanded`) and
once a review every reviewer approved (`ShownReviewApproved`); the page MUST ask the open and the close of each, and
toast what failed.

#### Scenario: The news of the listing and the robots
- **WHEN** a merged Safe Push of the branch of a draft and an approved review are known
- **THEN** the first is told to open, the second to close, and the page asks each
  [@test] crates/domain-policy/src/shown_test.rs (`a_draft_opens_once_a_safe_push_of_its_branch_merged_and_an_approved_one_closes`)
  [@test] src/lib/shown-news.test.ts (`opens the review after landing whose Safe Push merged, closes the approved one, and toasts what failed`)

### Requirement: Reviewer: A shown review gates no Safe Push

A pull request into the base of a shown review (`<prefix>/review/<branch>/base`, any prefix) MUST be told a shown
review: no Safe Push of it is offered (`ActionBlocker::Shown`), its Approve and Request Changes stay, and what an
approval of it counts for says that its change is in master already.

#### Scenario: A shown review to review
- **WHEN** the user reviews a shown review of someone else
- **THEN** no Safe Push is offered, Approve says it gates no Safe Push
  [@test] crates/domain-policy/src/shown_test.rs (`a_shown_review_is_told_by_its_base_of_any_prefix`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_shown_review_is_offered_no_safe_push_and_its_review_stays`)
  [@test] crates/domain-policy/src/approval_landing_test.rs (`an_approval_counts_for_the_safe_push_of_its_own_pull_request_into_the_target_and_a_layer_above_once_it_goes_there`)

### Requirement: Row: The review is marked beside the landing of the branch

The Status cell of a branch with an open shown review MUST mark the review last (`RowFact::Shown` with what it waits
for), beside the facts of its landing, never in their place.

#### Scenario: A blocked branch with a review after landing
- **WHEN** a blocked branch has a shown review
- **THEN** the block stays the main fact and the review is the last mark
  [@test] crates/domain-policy/src/row_status_test.rs (`a_shown_review_is_marked_beside_the_landing_never_in_its_place`)

## User Experience

- `brd safe-push <branch> --request-review [--reviewer <login>]`: `shown` names `reviewers`, `groups`, `head`, `base`,
  `number`; `brd request-review <branch> --after-landing`; `brd close-review <branch> [--allow-unapproved]`.
- The Status cell: "review after landing", "in review after landing", "changes requested after landing", "approved
  after landing"; the hover names the reviewers and the number.
- Your Turn: a review after landing a reviewer handed back is a "Returned to you" line.
- The review window of a reviewer: "Shown: in master already; your review gates no Safe Push, its approval closes the
  review"; the Safe Push of it disabled: "#12 is a review after landing: its change is in master already, so there is
  nothing to Safe Push".
- The toasts of a failure: "Cannot open the review after landing of auth", "Cannot close the review after landing of
  auth".

## References

- [Landing](../../../../docs/design/landing.md) ("Show: a review after landing")
- [Git Operations](../../../../docs/design/git-operations.md) ("Review head")
