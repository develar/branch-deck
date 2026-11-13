## MODIFIED Requirements

### Requirement: Reading the requests: One GraphQL search of the direct requests per refresh

Each refresh MUST be one GraphQL request (3 points of the rate limit), with the token of [Pull
Requests](../../branch/pull-requests/spec.md#the-token): the search `repo:<owner>/<name> is:pr is:open archived:false
draft:false user-review-requested:@me`, `first: 50` with the `issueCount`, and `repository(owner, name)`, whose null
is "no access" as for the pull requests. Each pull request MUST come with the names of its labels
(`labels(first: 20) { nodes { name } }`, `ReviewRequest.labels`): the words of its author, which nothing decides by.

#### Scenario: The query of a refresh
- **WHEN** the session refreshes the review requests of a GitHub repository
- **THEN** it sends one GraphQL request: the search of the direct requests, `first: 50` with the `issueCount`, and `repository(owner, name)`
- **AND** a null `repository(owner, name)` is "no access" as for the pull requests
  [@test] crates/remote-facts/src/review_requests_test.rs (`asks_for_the_direct_requests_of_the_repository`)

#### Scenario: The labels of a request
- **WHEN** GitHub lists a pull request with labels
- **THEN** its request carries their names, in GitHub's order, a null label skipped
  [@test] crates/remote-facts/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`)

### Requirement: Card: A row shows the unread dot, the title and the meta

A row, 36 px, MUST show the unread dot, the title (`font-medium`, the issue key it starts with muted and regular, where
Rust split the title, `titleLead`), the labels of its pull request as muted chips (`Badge variant="secondary"`, the
first two, then "+2"), and the meta "#101 · carol · 2 files · +12 −3", which give way before the title: the title is
cut only once the labels and the meta have no room left (muted line counts, as in every row); the labels and the meta
are hidden below `sm`.

#### Scenario: Someone else than the author asked
- **WHEN** someone else than the author asked for the review, as for a pull request a bot opened
- **THEN** the meta says "#830 · agent-bot · requested by dave · …": that one waits for the user
  [@test] src/lib/review-requests.test.ts (`names who asked for the review when it is not the author`)

#### Scenario: Another base than the default branch
- **WHEN** the base of the pull request is not the default branch
- **THEN** the meta ends with "· → release/253"
- **AND** a layer of a stack leaves it out: its base is the layer below it, and the header of the stack names the base of its bottom
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`, `should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)

#### Scenario: The labels of a pull request
- **WHEN** the pull request has labels on GitHub
- **THEN** its row shows the first two as muted chips after the title, then "+2" for the rest
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)

### Requirement: Card: Three rows, then Show N More

The card MUST show three rows, then "Show N More" ("Show Less" back), "Show 1 More" too: the named exception to the cut
rule of [UX Writing](../../../../docs/contributing/ux-writing.md), a fourth row would come and go with the polls. A
stack is one row, its layers not counted.

#### Scenario: More than three requests
- **WHEN** more than three requests wait
- **THEN** the oldest three show, the rest behind "Show N More" ("Show 1 More" for one), and "Show Less" goes back
  [@test] src/lib/review-requests.test.ts (`shows the oldest three and every unread request, an unread one in the place of a read one`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the oldest three first, the rest behind Show More, and say how long each waits`)

#### Scenario: A stack among the rows
- **WHEN** a stack of four requests and three requests alone wait
- **THEN** the stack and the two oldest of the others show, the third behind "Show 1 More"
- **AND** a stack with an unread layer is shown as an unread row is
  [@test] src/lib/review-requests.test.ts (`shows a stack as one row of the three, unread with an unread layer`)

### Requirement: Card: The keys of the rows

↑/↓ MUST move the focus between the rows (one tab stop), the header of a stack and its shown layers among them, Enter
reviews, `D` reviews its changes (the window opens in Changes once the pull request is fetched, as D in the review shows
them); the main window keeps the focus on the row. On the header of a stack, Enter MUST show or hide its layers, →
show them and ← hide them. Space MUST do nothing: it is Quick Look elsewhere, and a pull request has none.

#### Scenario: The arrows and Enter
- **WHEN** the user presses ↑/↓ and Enter on the rows
- **THEN** the focus moves between the rows, one tab stop, and Enter reviews the focused one
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should move with the arrows, review with Enter and open the menu from the keyboard`)

#### Scenario: The keys of the header of a stack
- **WHEN** the user presses ←, → and Enter on the header of a stack, then ↓
- **THEN** ← hides its layers, → shows them, Enter shows or hides them
- **AND** ↓ moves to its first layer, where Enter reviews it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)

#### Scenario: D on a row
- **WHEN** the user presses `D` on a row
- **THEN** the window opens in Changes once the pull request is fetched, and the main window keeps the focus on the row
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should review the changes of a row with D`)

#### Scenario: Space on a row
- **WHEN** the user presses Space on a row
- **THEN** nothing happens

## ADDED Requirements

### Requirement: Reading the requests: A chain of requests is one row of the view

The session MUST give the rows of the requests in their order (`ReviewRequestsView.rows`,
`domain_policy::review_request_chains::request_rows`): a request whose base is the head branch of another request of
the repository is the layer above it, and a chain that does not fork is one row, its bottom first, at the place of its
layer that waits the longest. A branch two requests go into MUST link neither of them; a request from a fork MUST never
be a layer below another, its head branch not being one of the repository; a request into the default branch MUST be the
bottom of its chain. The names alone decide: GitHub's own stacks are such chains too. A stack whose bottom is not
requested from the user starts at its lowest requested layer.

#### Scenario: A chain into the head branches of each other
- **WHEN** three requests go each into the head branch of the one below, the bottom into master
- **THEN** they are one row, the bottom first, at the place of the layer that waits the longest
  [@test] crates/domain-policy/src/review_request_chains_test.rs (`a_chain_of_requests_is_one_stack_bottom_first_at_the_place_of_its_longest_waiting_layer`)
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_chain_of_requests_is_one_row_of_the_view`)

#### Scenario: A fork, a pull request from a fork, the default branch
- **WHEN** two requests go into one branch, a request comes from a fork, or a request goes into the default branch
- **THEN** the two link neither of them, the one from a fork is no layer below another, and the one into the default branch is a bottom
  [@test] crates/domain-policy/src/review_request_chains_test.rs (`a_chain_of_requests_is_one_stack_bottom_first_at_the_place_of_its_longest_waiting_layer`)

### Requirement: Card: A chain of requests is one stack

A stack MUST be one row of the card, collapsed: the unread dot of its layers, a chevron, "Stack of 12 pull requests",
the muted meta "#979–#991 · niyaz · +120 −30" ("3 authors" when several opened its layers; the base of its bottom
"· → release/253" when it is not the default branch) and how long its layer that waits the longest waits. A click shows
its layers under it, the bottom first, indented, each a row of a request; a click again hides them. Filtered, a stack
MUST show the layers that match, open, its title counting all of them.

#### Scenario: A stack of three requests
- **WHEN** three requests of one author go each into the branch of the one below
- **THEN** the card shows one row "Stack of 3 pull requests" with "#201–#203 · niyaz · +36 −9" and the age of its oldest layer
- **AND** a click shows the three layers under it, the bottom first
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels`)
  [@test] src/lib/review-requests.test.ts (`names a stack by its size, its bottom, its top and its author`)

#### Scenario: A filter that matches one layer
- **WHEN** the filter of Reviews matches one layer of a stack
- **THEN** the stack shows open with that layer, still "Stack of 3 pull requests"
  [@test] src/lib/review-requests.test.ts (`groups the requests by the rows of the session, a stack with the layers a filter left`)
