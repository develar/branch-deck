## ADDED Requirements

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

## MODIFIED Requirements

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
