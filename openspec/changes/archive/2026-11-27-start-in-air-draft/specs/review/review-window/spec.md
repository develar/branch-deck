## MODIFIED Requirements

### Requirement: Actions: The window runs each action itself but a form under a row

The window MUST run each action itself (an intent, a dialog of the window, a copy, the next review), but one that
opens a form under the row of its subject: those Rust names (`ActionId::opens_row_form`, `ROW_FORM_ACTIONS` of the
bindings), the fix of a conflict, the forms and the rewrites of a commit.

#### Scenario: A dialog of the window and an open of the pull request
- **WHEN** the user creates or opens the pull request from the window
- **THEN** the window runs it itself, with the context of its own session, and asks nothing of the main window
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should create the pull request in the window, with the context of its own session`, `should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)

#### Scenario: The actions Rust names as forms under a row
- **WHEN** an action opens a form under the row of its subject
- **THEN** the catalog the page reads (`ROW_FORM_ACTIONS` of the bindings) names it as Rust does
  (`ActionId::opens_row_form`): a rename, a move, a stack, an ungroup, the amend, the issue reference, the squash,
  unapply, park, a declared need, the typed delete of a kept branch, the popover of a Safe Push and of its rarer starts,
  Request Review, a push (also with its children), which asks at the row when it would drop remote
  commits, the pick of remote commits, the push into the target and the merge of the pull request of a landing, the
  close of the pull request of a stack
- **AND** the window does not run it itself
  [@test] crates/domain-policy/src/action_catalog_test.rs (`every_action_and_every_blocker_code_is_in_the_action_catalog_the_page_reads`)
