## MODIFIED Requirements

### Requirement: UI: The branch menu offers the squash in a "History" group

The branch menu MUST have the same item, named alike, in a "History" group, which opens the confirmation at the badge of
the row and leaves its commits as they are. While the plan of the fixups has not read a fixup commit of the branch, the
item MUST hold its place, disabled, so a menu opened right after the sync shows it there once the plan answers.

#### Scenario: The squash from the branch menu
- **WHEN** the user chooses "Squash N Fixups…" in the "History" group of the branch menu
- **THEN** the confirmation opens at the badge of the row and its commits are left as they are
  [@test] src/lib/autosquash.test.ts (`names the squash of the fixups alike on the button and in the branch menu, which open the confirmation`)
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should squash from the branch menu and one fixup from the commit menu`)

#### Scenario: A menu opened before the plan of the fixups answered
- **WHEN** the branch menu opens after the sync that found a fixup commit of the branch, before the plan of the fixups answered
- **THEN** "Squash Fixups…" holds the place of the squash, disabled, "Reading the fixups of the branch…"
- **AND** once the plan answers, the open menu shows "Squash 1 Fixup…" in that place, and it opens the confirmation
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should show Squash 1 Fixup… in a branch menu opened before the plan of the fixups answered, in the place it held`)
