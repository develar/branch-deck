## MODIFIED Requirements

### Requirement: Status cell: The details of a chip hold still under the pointer

The details of a chip MUST hold still while the pointer is in them or a press that began in them lasts, though the chip
moves (the Status column of a row moves as its Actions widen: Stop Waiting comes in the slot of a start pressed in the
details), hung by the edge they hung by, so what comes or goes in them changes their other edge, and MUST stand by the
chip again once the pointer left them for the chip. A press ends with the pointer up or with its cancel.

#### Scenario: Stop Waiting comes in the row of a Safe Push pressed in the robot card
- **GIVEN** the pointer pressed Safe Push in the details of the robot chip, and rests there
- **WHEN** the start waits for another holder of the repository lock, and Stop Waiting comes in the slot of its row, which moves the chip
- **THEN** the details stay open where they were, the press point in Safe Push, whether they lie below the chip or over it
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should hold the robot card under the pointer that pressed its Safe Push as Stop Waiting comes in the row, 700 px high`, `should hold the robot card under the pointer that pressed its Safe Push as Stop Waiting comes in the row, 420 px high`)

#### Scenario: The pointer leaves the details for the chip
- **GIVEN** the details hold still while the chip moved
- **WHEN** the pointer leaves them for the chip
- **THEN** they stand by the chip again, where an open of them puts them
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should hold the robot card under the pointer that pressed its Safe Push as Stop Waiting comes in the row, 700 px high`)

#### Scenario: A press that began in the details
- **GIVEN** the details hold still while the chip moved
- **WHEN** a press begins in them and the pointer leaves for the chip with the button down
- **THEN** they hold still while the press lasts, and open by the chip again once it ended outside them
- **AND** a press the browser cancels ends there: they stand by the chip as the pointer left them, the button still down
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should hold the robot card while a press that began in it lasts, and stand it by its chip once the press ends or is canceled`)

### Requirement: Status cell: The details of every chip have one anatomy

The details of every chip MUST have one anatomy: the state line (the icon and the state in the hue of the chip), the
title or a sentence on what it means, `dl` rows, the notices, then the actions. Why a failed robot failed, read ahead,
MUST stand before the actions. What a robot's details tell after its card opened MUST come on the side of the actions
away from the edge the card hangs by, its first row of actions next to that edge, so nothing moves an action under the
pointer.

#### Scenario: The details of a chip
- **WHEN** the details of a Landed chip or of a robot chip open
- **THEN** they show the state line, the title or a sentence on what it means, `dl` rows, the notices, then the actions
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

#### Scenario: The details of a robot come after its card opened
- **GIVEN** the card of a failed robot open, why it failed above its Safe Push, the pointer on Safe Push, its checks
  still to come
- **WHEN** the checks come
- **THEN** they stand below the actions of a card hung by its top edge (under its chip, beside it), above them in one
  hung by its bottom edge (over its chip, aligned by its end or shifted up against the bottom of the window beside it),
  and Safe Push keeps its box, also beside its chip shifted against an edge of the window, in both engines
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 700 px high`, `should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 420 px high`, `should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, beside its chip against an edge of the window`)

#### Scenario: Why a robot failed comes after the card over its chip opened
- **GIVEN** the card of a failed robot over its chip, hung by its bottom edge, the pointer on its Restart, why it failed
  still to come
- **WHEN** it failed for missing approvals
- **THEN** the problems, the notice and the row of actions that goes stand above the first row, and the pointer stays on
  Restart (of a pull request) or on Safe Push in its place (without one), in both engines
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should keep Restart of the robot card under the pointer as why the robot failed comes after the card opened, over its chip`, `should put Safe Push in the place of Restart under the pointer as why the robot without a pull request failed comes after the card opened, over its chip`)

#### Scenario: The robot of a whole stack that needs review
- **GIVEN** the card of the robot of a whole stack without a pull request, the pointer on its Restart, why it failed
  still to come
- **WHEN** it failed for missing approvals
- **THEN** its first row goes, neither Restart nor Safe Push in it (a new Safe Push of the stack starts from its root),
  and text of the card is what stays under the pointer: accepted
