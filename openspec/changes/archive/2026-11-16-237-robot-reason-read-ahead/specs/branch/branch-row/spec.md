## MODIFIED Requirements

### Requirement: Status cell: The details of every chip have one anatomy

The details of every chip MUST have one anatomy: the state line (the icon and the state in the hue of the chip), the
title or a sentence on what it means, `dl` rows, the notices, then the actions. Why a failed robot failed, read ahead,
MUST stand before the actions. What a robot's details tell after its card opened MUST come on the side of the actions
away from the edge the card hangs by, so it moves no action under the pointer.

#### Scenario: The details of a chip
- **WHEN** the details of a Landed chip or of a robot chip open
- **THEN** they show the state line, the title or a sentence on what it means, `dl` rows, the notices, then the actions
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

#### Scenario: The details of a robot come after its card opened
- **GIVEN** the card of a failed robot open, why it failed above its Safe Push, the pointer on Safe Push, its checks
  still to come
- **WHEN** the checks come
- **THEN** they stand below the actions of a card under its chip (above them in one over its chip), and Safe Push keeps its box, in both engines
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 700 px high`, `should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 420 px high`)
