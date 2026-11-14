## MODIFIED Requirements

### Requirement: Review window: The decisions of a stack are those of the layer shown

Approve, Request Changes… and Safe Push… of the title bar MUST act on the layer of the item shown, with that layer's
actions as Rust decides them (`pull:<number>`: a Safe Push into another branch than the target is refused there, and
Approve says when Patronus counts it), on the head of that layer as the fetch answered it. All Changes MUST offer the
actions of the stack itself (`review-stack:<top>`, `stack_review_actions`): Approve N Pull Requests alone, the primary.

#### Scenario: Approve a layer
- **WHEN** the user approves while a layer is shown
- **THEN** GitHub gets the review of that pull request with the head of the layer as its commit, and no other pull request of the stack gets one
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review all changes of a stack and each layer, and approve the layer shown on its head`)

#### Scenario: All Changes of a stack
- **WHEN** All Changes of a stack is shown
- **THEN** the title bar offers Approve N Pull Requests, the primary, and no Approve or Request Changes…
- **AND** on a layer Approve is the primary and Approve N Pull Requests an outline button beside it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review all changes of a stack and each layer, and approve the layer shown on its head`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should approve every layer of a stack on the head it shows by its button, skipping a layer approved on it`)
  [@test] crates/domain-policy/src/stack_approval_test.rs (`the_actions_of_a_stack_are_approve_n_pull_requests_alone_disabled_once_nothing_is_left_to_approve`)
  [@test] src/lib/review-request-actions.test.ts (`offers no decision of one pull request where Rust offers the actions of the stack alone`)

#### Scenario: The files of a layer while they are read
- **WHEN** the user chooses a layer and its files are still read
- **THEN** Files and Changes say no count until they are read: GitHub's count of the pull request is of another range when the layer is not rebased
  [@test] src/lib/range-review.test.ts (`counts the files once read, GitHub's count of a pull request alone before, and none of a layer of a stack while they are read`)

### Requirement: Review window: Approve N Pull Requests approves every layer the window shows

Review Stack MUST offer Approve N Pull Requests: one write (`ApproveStack`, holding the review of every layer) that
approves each layer on the head the fetch of the window brought, the bottom first, one `POST pulls/<n>/reviews` with
that head as `commit_id` each. A layer the user approved on that head, or an own one, MUST be skipped; N counts the
layers it approves. Rust decides it (`domain_policy::stack_approval`, `RepositoryView.reviewStacks`); the page words it.

#### Scenario: A layer approved on its head before
- **GIVEN** a stack of three, the user approved #302 on the head the window shows
- **WHEN** the user presses Approve 2 Pull Requests
- **THEN** GitHub gets the reviews of #301 and #303, each with its head as the commit, and no second review of #302
- **AND** the toast says "Approved #301 and #303", and the hover of the button "Skips #302: you already approved its commit"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should approve every layer of a stack on the head it shows by its button, skipping a layer approved on it`)
  [@test] crates/domain-policy/src/stack_approval_test.rs (`a_stack_approval_approves_each_layer_on_its_shown_head_and_skips_those_approved_on_it`)
  [@test] crates/operations/src/review_test.rs (`a_stack_approval_approves_each_layer_on_its_shown_head_but_those_approved_on_it`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_decides_approve_n_pull_requests_of_a_stack_on_the_heads_its_fetch_brought`)

#### Scenario: Where Patronus merges
- **WHEN** Patronus merges the repository and the bottom of the stack goes into master
- **THEN** the hover of Approve N Pull Requests says "Patronus counts #301 for its own Safe Push, #302 and #303 once each goes into master", as Rust says of each layer ([Approve of a layer](#requirement-review-window-approve-of-a-layer-says-when-patronus-counts-it))
  [@test] src/presenters/reviews.test.ts (`says Patronus counts the bottom for its own Safe Push and the layers above once each goes into the target, nothing without Patronus`)
  [@test] src/lib/review-request-actions.test.ts (`counts the layers Rust approves on the heads the window shows, names the ones it skips, and waits for the heads and a write`)

### Requirement: Review window: Approve N Pull Requests is a button, an item of "⋯", ⇧A and an item of the stack

Approve N Pull Requests MUST be a button before Approve in the title bar (the primary on All Changes, which offers it
alone; outline on a layer), an item of "⋯" of the title bar and ⇧A; the menu of the header of the stack in Your Turn
MUST offer Approve N Pull Requests…, which opens Review Stack with the focus on the button once the stack is fetched,
since an approval is on the heads the user saw.

#### Scenario: The menu of the header of the stack, "⋯" and ⇧A
- **WHEN** the user chooses Approve 3 Pull Requests… in the menu of the header of the stack in Your Turn
- **THEN** Review Stack opens with the focus on Approve 3 Pull Requests once the stack is fetched
- **AND** its item in "⋯" and ⇧A approve as the button does
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should refuse a layer whose head moved and every layer above it by number, from the menus and by ⇧A`)

### Requirement: Review window: A layer whose head moved is refused, and the layers above it

Before the first write Approve N Pull Requests MUST read GitHub's head of every layer again (one `GET pulls/<n>` each;
a failed read approves nothing). The layers below the lowest one whose head moved MUST be approved; that layer and
every layer above it MUST be refused by number in one error toast, "Cannot approve #302 and #303", with Fetch Again,
which lists the requests again. A stack lands bottom up, each layer with what it is built on: a layer above changes
from a head nobody read.

#### Scenario: Someone pushed to a middle layer
- **GIVEN** a stack of three, and GitHub shows another head of #302 than the window
- **WHEN** the user approves the stack
- **THEN** #301 is approved, and #302 and #303 are refused: "The head of #302 moved from 1a2b3c4d to ffffffff since the window fetched it: fetch the stack again to read the new commits"
- **AND** Fetch Again of the toast lists the requests again
- **AND** once GitHub shows the head the window shows again, the approval of the stack approves #302 and #303, #301 skipped
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should refuse a layer whose head moved and every layer above it by number, from the menus and by ⇧A`)
  [@test] crates/domain-policy/src/stack_approval_test.rs (`the_layers_below_the_lowest_moved_head_are_approved_and_it_and_those_above_are_refused`)
