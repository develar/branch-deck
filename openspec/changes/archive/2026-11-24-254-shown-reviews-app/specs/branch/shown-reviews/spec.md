## ADDED Requirements

### Requirement: App: Safe Push and Request Review… picks the reviewers beside the start

The app MUST offer Safe Push and Request Review… of a branch where a Safe Push of Patronus lands into a GitHub
repository ("Patronus ›" of its menu, the command palette, and the popover of its Safe Push beside the start), disabled
as its start would refuse it (its review after landing open already, an open pull request, more than the branch,
approvals before landing, then the reasons of the Safe Push), never for nobody to ask. Its popover MUST be the Safe Push
popover with a picker of the reviewers that starts with whom `brd` would ask (`get_show_reviewers`: one available code
owner of each owner group of the changes), in which the user removes one or adds any GitHub login of the repository by
its search, and whose start MUST plan the Safe Push with them (`Land.requestReview`); none refuses it (`noReviewer`).

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

## MODIFIED Requirements

### Requirement: Start: A review after landing is refused where it can't review what lands

A Safe Push with a review after landing MUST be refused when the branch has an open pull request (`pull_request_open`:
its review is there), when its review after landing is open already (`show_review_open`: a fix lands by its own Safe
Push and goes into it; a draft, whose robot failed, is started again), when it lands more than the branch or a whole
stack (`show_lands_stack`), when the code owners must approve the change first (`approvals_required`), when the
repository lands without Patronus (`show_needs_patronus`), and when nobody is to be asked (`no_reviewer`).

#### Scenario: What refuses it
- **WHEN** the branch has an open pull request or an open review after landing, is stacked on a branch not in master,
  or needs the approvals first
- **THEN** the start is refused before anything is pushed
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_with_a_review_after_landing_opens_its_draft_before_the_start_and_lands_only_the_branch`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_check_names_whom_the_review_after_landing_asks_of_the_owner_groups_and_refuses_a_branch_with_a_pull_request`)

### Requirement: Reviewers: One available member of each owner group, never the author

Without `--reviewer`, a review after landing MUST ask one reviewer of each owner group of the paths the branch changes
(the `OWNERSHIP` files of the target, the members from the CodeOwners service): from the reviewers of the group when
it names any, else from its members, available, not passed by the rotation, with a GitHub login, never the author; one
chosen for a group covers every other group that has them. The picker of the app MUST start with the same reviewers,
read the same way (`remote_facts::show_reviewers`, the Space token of Patronus).

#### Scenario: The reviewers of a group before its members
- **WHEN** a group names reviewers and members, the author among them
- **THEN** the first available reviewer but the author is asked, and a group of which nobody can be asked is named
  [@test] crates/domain-policy/src/shown_test.rs (`one_available_reviewer_of_each_owner_group_never_the_author`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_check_names_whom_the_review_after_landing_asks_of_the_owner_groups_and_refuses_a_branch_with_a_pull_request`)
  [@test] crates/codeowners/src/client_test.rs (`a_lookup_names_the_reviewers_and_the_members_with_their_logins_and_availability`, `a_group_the_service_lacks_has_no_members_and_a_rejected_token_is_an_error`)

#### Scenario: The lookup as the real service answers it
- **WHEN** the service answers members only, `isReviewer` null, one of them away on vacation, with fields Branch Deck
  doesn't read
- **THEN** the lookup reads, and the member away is not asked
  [@test] crates/codeowners/src/client_test.rs (`a_lookup_of_the_real_shape_has_members_without_a_reviewers_list_and_tells_who_is_away`)
  [@test] crates/bd-remote/src/shown_review_test.rs (`a_member_away_is_not_asked_of_a_group_as_the_real_service_answers_it`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should start a Safe Push and Request Review… from the popover of S with the code owners of the changes, the draft opened into the fork point before the robot`)

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
for), beside the facts of its landing, never in their place; its words MUST say whose move it is in each state: the
Safe Push that opens it, whom it waits for, who requested changes and how a fix goes into it (a commit `(<branch>) …`,
its own Safe Push), the fix that lands, who approved it.

#### Scenario: A blocked branch with a review after landing
- **WHEN** a blocked branch has a shown review
- **THEN** the block stays the main fact and the review is the last mark
  [@test] crates/domain-policy/src/row_status_test.rs (`a_shown_review_is_marked_beside_the_landing_never_in_its_place`)

#### Scenario: The words of each state
- **WHEN** the review is a draft, waits for alice and bob, has changes requested by alice, waits for a fix, or alice
  approved it
- **THEN** the mark says "#12 opens for review once the Safe Push of auth merges", "In review after landing: waits for
  alice and bob in #12", "alice requested changes in #12: commit a fix as (auth) …, and its Safe Push adds it to the
  review", "A fix of auth lands: #12 takes it once its Safe Push merged and asks its reviewers again", "Approved by
  alice in #12: it closes with its branches"
  [@test] src/presenters/row-status.test.ts (`marks the review after landing beside what landed the branch, its hover whose move it is in each state`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should return the review to the author on changes requested, wait while a fix lands, then put the fix onto its head and ask again`)
