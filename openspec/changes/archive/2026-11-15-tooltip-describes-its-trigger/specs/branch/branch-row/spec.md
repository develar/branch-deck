## ADDED Requirements

### Requirement: Hover and click: The words of a tooltip are the description of its trigger

The words of a tooltip, a string, MUST be the accessible description of its trigger, the closed tooltip's too, so a
screen reader hears why a control is disabled ("Waits for the answer of Patronus"); where the trigger wraps the control
the focus reaches, that control MUST have them too. A tooltip that only says the name of its trigger MUST add no
description.

#### Scenario: A disabled item and its reason
- **WHEN** the Safe Push of the row menu is disabled while Patronus has not answered
- **THEN** its description is "Waits for the answer of Patronus" with its tooltip closed, and the same once its tooltip opened: Base UI adds no second one
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should keep Safe Push in its place while Patronus answers, disabled until it does, and the item under the pointer where it was @webkit`)

#### Scenario: A button the trigger of its tooltip wraps
- **WHEN** the description of Approve came in a review window
- **THEN** the Approve button has it as its description, where the focus is
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open the description of Approve that came while the pointer rested on it, after the delay of its hover`)

#### Scenario: A tooltip that only says the name of its trigger
- **WHEN** a tooltip says the name of its trigger or the start of it ("Expand All" of "Expand all unchanged lines of
  RowMapper.kt", "Dismiss", "More actions")
- **THEN** its trigger has no description, open or closed: a screen reader hears the name once
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`)

#### Scenario: A tooltip whose content is not a string
- **WHEN** a tooltip shows keys beside its words, a relative time or the steps of an update
- **THEN** its trigger is described by a short phrase of what it says, never by a copy of its content
  [@test] tests/e2e/next-step/next-step.spec.ts (`should open a pull request into another base that may be meant, and keep its retarget in the menu`)

#### Scenario: A key as the platform writes it
- **WHEN** a description or a menu item says a key
- **THEN** on a Mac it is its symbols side by side ("Shortcut: ⌘R", ⌫ for Delete), elsewhere their words joined by
  "+" ("Shortcut: Ctrl+R", Del, Alt+←, Ctrl+Enter)
  [@test] tests/e2e/branch-row-layout/mac-keys.spec.ts (`should name the keys as a Mac does: ⌘R and ⌘1 in the descriptions, ⌫ for the drop of a commit in its menu`)

#### Scenario: A lazy tooltip of a table row
- **WHEN** the trigger of a lazy tooltip of a row was neither hovered nor focused yet
- **THEN** it has no description and its tooltip adds no node; once its root mounted (the first hover or focus), the
  words of the tooltip describe it
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should check what a branch needs to land before it offers its Safe Push, S and the popover waiting for the answer`)

#### Scenario: The status of a lost start
- **WHEN** the slot of a row says "Start Unknown"
- **THEN** its reason, which its tooltip says, is its description at once: its tooltip mounts with it
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should show a start whose answer was lost on its row, refuse a start of it, and start again once it is forgotten`)
