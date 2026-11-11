## ADDED Requirements

### Requirement: Log: An entry the main window recorded shows once, whichever answer comes first

An entry the main window recorded MUST show once and stay, whichever answer comes first: a read that brought it before
its record answered keeps it alone, and a read that answers after the record without it (made before the write) leaves
it, until a read asked after the record answered, which is the log (Clear Log forgets it).

#### Scenario: A read made after the write answers before the record
- **WHEN** the read the write of a rewrite asked (`activity-changed`) answers before the record of the rewrite
- **THEN** the entry shows once, and gets its Undo as the record answers
- **AND** its Undo undoes the rewrite
  [@test] src/stores/activity.test.ts (`shows an entry it recorded once when a read made after the write answered before the record`)
  [@test] tests/e2e/activity/activity.spec.ts (`should show a rewrite once whose record answers after the read its write asked, and undo it there`)

#### Scenario: A read made before the write answers after the record
- **WHEN** a read asked before the record of an entry answered answers after it, without the entry
- **THEN** the entry stays, until a read asked after the record answered
  [@test] src/stores/activity.test.ts (`keeps an entry it recorded when a read made before the write answers after the record, until a read made after it`)
