## MODIFIED Requirements

### Requirement: Log: Each window reads the log itself when it opens and when a journal changes

The main window and the Activity window MUST each read the log themselves (`useActivityReads`): when they open, and
again when a journal changes (`activity-changed`: a write of the page, or the watch of the operations registry of a
repository a window shows sees its journal change, an operation of the app or of `brd` ended, within 2 s also where the
system told no event of it, and at once when a window gains the focus); the main window also after each operation and
each write of its own.

#### Scenario: An operation of the main window ended
- **WHEN** an operation the main window ran ended
- **THEN** Activity reads the journal it ended in
  [@test] src/model/intents.test.ts (`runs the plan Rust prepared on its fingerprint, then waits for the view of what it changed; Activity reads the journal it ended in`)

#### Scenario: A journal changes
- **WHEN** a journal changes (`activity-changed`)
- **THEN** each window that is open reads the log again
  [@test] tests/e2e/activity/activity.spec.ts (`should hide the operations of agent leases behind the Agents chip`, `should mark an unavailable journal`)

#### Scenario: A change of the journal the watch did not tell
- **GIVEN** a window shows a repository, and the watch of the system tells nothing of a change of its journal (it
  drops the changes of the first moments of a stream, and runs behind under load)
- **WHEN** an operation of `brd` ends in the journal, right after the watch started too, or a window gains the focus
- **THEN** the windows hear `activity-changed` at the next look of the watch, within 2 s, at once on the focus
  [@test] crates/operations/src/registry_watch_test.rs (`a_change_right_after_the_watch_started_is_a_change_at_the_first_look`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_focus_looks_at_the_journal_and_the_stamp_too`)
