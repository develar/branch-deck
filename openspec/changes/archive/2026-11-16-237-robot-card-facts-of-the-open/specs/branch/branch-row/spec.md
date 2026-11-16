## MODIFIED Requirements

### Requirement: Status cell: The details of a chip hold still under the pointer

The details of a chip MUST hold still while the pointer is in them or a press that began in them lasts, though the chip
moves (the Status column of a row moves as its Actions widen: Stop Waiting comes in the slot of a start pressed in the
details), hung by the edge they hung by, without collision avoidance (they may pass the window, accepted), and MUST stand
by the chip again once the pointer left them. A press ends with the pointer up, its cancel or the window's blur.

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
