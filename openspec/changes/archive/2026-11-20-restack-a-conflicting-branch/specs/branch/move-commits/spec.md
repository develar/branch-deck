## MODIFIED Requirements

### Requirement: Drag and drop: A target refuses as the menu of the same action does

A target MUST refuse as the menu of the same action does, and say why while the pointer is over it.

#### Scenario: Whole branches one of which failed to sync
- **WHEN** a drag of whole branches, one of which has a sync error, is over a target
- **THEN** the target refuses it as Move to… of a branch does, and says why

#### Scenario: A branch whose sync conflicts
- **WHEN** a drag of a branch with a merge conflict is over the "Stack on ‹name›" chip of a row
- **THEN** the chip takes it, as Stack On… of the branch does
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a branch whose sync conflicts on the branch it needs from the menu and by a drop on its Stack on chip, the conflict gone`)
