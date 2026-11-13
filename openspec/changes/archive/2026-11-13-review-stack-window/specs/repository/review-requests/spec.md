## MODIFIED Requirements

### Requirement: Card: The keys of the rows

↑/↓ MUST move the focus between the rows (one tab stop), the header of a stack and its shown layers among them, Enter
reviews, `D` reviews its changes (the window opens in Changes once the pull request is fetched, as D in the review shows
them); the main window keeps the focus on the row. On the header of a stack, Enter MUST show or hide its layers, →
show them and ← hide them, `D` MUST review the stack ([Review Stack](#requirement-card-review-stack-opens-a-stack-in-one-window))
in Changes, and the context menu key and Shift+F10 open its menu. Space MUST do nothing: it is Quick Look elsewhere,
and a pull request has none.

#### Scenario: The arrows and Enter
- **WHEN** the user presses ↑/↓ and Enter on the rows
- **THEN** the focus moves between the rows, one tab stop, and Enter reviews the focused one
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should move with the arrows, review with Enter and open the menu from the keyboard`)

#### Scenario: The keys of the header of a stack
- **WHEN** the user presses ←, → and Enter on the header of a stack, then ↓
- **THEN** ← hides its layers, → shows them, Enter shows or hides them
- **AND** ↓ moves to its first layer, where Enter reviews it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)

#### Scenario: D and the menu key on the header of a stack
- **WHEN** the user presses `D` on the header of a stack, or Shift+F10
- **THEN** `D` opens Review Stack in Changes and the main window keeps the focus on the header; Shift+F10 opens its menu
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open one window of a stack by the button, the menu and D of its header`)

#### Scenario: D on a row
- **WHEN** the user presses `D` on a row
- **THEN** the window opens in Changes once the pull request is fetched, and the main window keeps the focus on the row
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review the changes of a row with D`)

#### Scenario: Space on a row
- **WHEN** the user presses Space on a row
- **THEN** nothing happens

## ADDED Requirements

### Requirement: Card: Review Stack opens a stack in one window

The header of a stack MUST open the whole stack in one review window, Review Stack: its button on hover
(`FileDiffIcon`, "Review Stack", as the button of a row opens its pull request), the item "Review Stack" (`D`) of its
menu (the right click, the context menu key, Shift+F10), beside "Show Pull Requests" / "Hide Pull Requests" (Enter),
and `D` on it. It opens every layer of the stack, also the ones a filter hides, and marks them read.

#### Scenario: The button, the menu and D of the header
- **WHEN** the user clicks the button of the header of a stack, chooses Review Stack in its menu, or presses `D` on it
- **THEN** the review window opens on the stack, "Review Stack #301–#303", its layers the bottom first; `D` opens it in Changes
- **AND** the header keeps its layers shown or hidden: Enter and Show Pull Requests still do that
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open one window of a stack by the button, the menu and D of its header`)
  [@test] src/stores/review-requests.test.ts (`opens the window on every layer of the stack Your Turn lists, the bottom first, with the intent of D`)

### Requirement: Review window: Review Stack reviews a stack in one window

The review of a stack MUST be one review of the window (`kind: "pullRequestStack"`, named by its top and its bottom),
its native title "Review Stack #979–#991", its window title "Stack", "Stack › 2 of 3" on a layer. Its commit column
MUST hold All Changes (the whole stack, from the base of its bottom to the head of its top), then a scope per layer in
the order of the card, the bottom first ("#202 Drop workTreeRoot", its commits; the range of the layer from the head of
the layer below to its own head, as GitHub's "Files changed" of it), then the commits newest first under the heading of
their layer, each commit once. A scope or a commit of a layer MUST show that layer: its pull request in the island,
its description and its files and commits in the overview; ↑/↓ in the overview MUST walk the layers. The overview of
All Changes MUST say "Stack of 3 pull requests", "#301–#303 · niyaz · into master", and list the layers, each a link
to its scope.

#### Scenario: All Changes and the layers of a stack
- **WHEN** the window reviews a stack of three layers of a commit each
- **THEN** the overview says "Stack of 3 pull requests" and lists "#301 Add the auth module · 1 commit" and the two others, the rail the three commits
- **AND** Changes lists All Changes, a scope per layer and the commits under their layers, All Changes chosen
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review all changes of a stack and each layer, and approve the layer shown on its head`)
  [@test] src/lib/range-review.test.ts (`lists All Changes of the stack, a scope per layer the bottom first, then the commits newest first under their layers, each once`, `has a scope per layer, without counts, while the stack is fetched`)

#### Scenario: A layer of the stack
- **WHEN** the user chooses the scope of a layer, or a commit of it, or walks to it with ↓
- **THEN** the island shows its pull request, "Stack › 2 of 3", and the overview its description, files and commits
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review all changes of a stack and each layer, and approve the layer shown on its head`)
  [@test] src/lib/range-review.test.ts (`names the layer of the item shown: its scope, a commit of it, none for All Changes`)

### Requirement: Review window: The decisions of a stack are those of the layer shown

Approve, Request Changes… and Safe Push… of the title bar MUST act on the layer of the item shown, with that layer's
actions as Rust decides them (`pull:<number>`: an approval Patronus would not count where the layer goes into another
branch than its target is refused there, never in the page), on the head of that layer as the fetch of the stack
answered it. On All Changes they MUST be disabled: "Choose a pull request of the stack".

#### Scenario: Approve a layer
- **WHEN** the user approves while a layer is shown
- **THEN** GitHub gets the review of that pull request with the head of the layer as its commit, and no other pull request of the stack gets one
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review all changes of a stack and each layer, and approve the layer shown on its head`)

#### Scenario: All Changes of a stack
- **WHEN** All Changes of a stack is shown
- **THEN** Approve and Request Changes… are disabled with "Choose a pull request of the stack"
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review all changes of a stack and each layer, and approve the layer shown on its head`)

### Requirement: Review window: A stack is one fetch of its top

The fetch of a stack MUST be one operation of the session (the intent `fetchReviewStack { top, bottom }`,
`operations::review_fetch`, holding the review refs of its top: a fetch of the top alone waits for it), whose core
(`sync_core::review_fetch::fetch_review_stack`) is one review fetch of the head of the top with the base of the bottom
(nothing fetched when both are local) into `refs/branch-deck/review/<top>/`: its range holds every layer built on the
one below, cut at their heads, with no process per layer. A layer not built on the head of the layer below as it is
now (its author rewrote a lower layer and did not rebase it: restacked) MUST be a fetch of its own, from its own base,
into `refs/branch-deck/review/<top>/layers/<n>/{head,base}`, which the session keeps and prunes with the refs of the
top; its scope says "not rebased" and its overview why. The head each layer shows goes to the session, which asks the
readiness of each pull request at it.

#### Scenario: A stack of three on GitHub
- **WHEN** the window reviews a stack of three pull requests none of whose commits are local
- **THEN** one fetch of the head of the top brings them, only `refs/branch-deck/review/<top>/head` is written, and each layer is its range from the head below
- **AND** the processes are those of the review fetch of one pull request; with the commits local, no fetch
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_stack_is_one_fetch_of_its_top_cut_into_its_layers`, `a_stack_without_layers_is_an_error`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_review_fetch_of_a_stack_is_one_fetch`)
  [@test] crates/operations/src/review_fetch_test.rs (`a_review_fetch_of_a_stack_answers_the_range_of_each_layer`)

#### Scenario: A layer not rebased onto the one below
- **WHEN** the author amended a middle layer and did not rebase the one above it
- **THEN** the middle layer and the one above are fetches of their own, the upper one from where it was cut, the older commits of the middle one among its own
- **AND** their refs stand under the refs of the top, and a prune that keeps the top keeps them
  [@test] crates/sync-core/src/review_fetch_test.rs (`a_restacked_layer_is_a_fetch_of_its_own`)

### Requirement: Review window: The layers of a stack are those Rust chains now

The layers of a stack MUST be a rule of Rust (`domain_policy::review_request_chains::stack_layers`, through
`ReviewRequestsFact::stack`): from the top down, the pull request whose head branch is the base of the layer above,
of every pull request a listing named, until the bottom the window opened with: a layer the user reviewed (gone from
Your Turn) stays, a merged one leaves (GitHub moved the layer above onto the default branch), one put in between
joins. The window MUST fetch again when a layer it knows or a layer of the row of its top in Your Turn moves (a push, a
rebase) or comes or goes, and show the layers the fetch answered.

#### Scenario: A reviewed, a merged and an inserted layer
- **WHEN** the user approved a middle layer, the bottom merged, or a pull request was put between two layers
- **THEN** the approved layer stays a layer, the merged bottom is no layer any more, the inserted one is one
  [@test] crates/domain-policy/src/review_request_chains_test.rs (`the_stack_of_a_review_window_is_the_chain_under_its_top_as_the_known_pull_requests_link_it`)
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_stack_of_a_review_window_keeps_a_reviewed_layer_and_drops_a_merged_one`)
