## MODIFIED Requirements

### Requirement: Welcome card: The field of Settings saves on Enter and when it loses the focus

The field of Settings MUST save on Enter, when it loses the focus and when the window closes, never on a key (the
prefix names the branches of the sync and the robots of Patronus), MUST refuse a prefix git cannot take in place, and
Escape MUST take the typing back before it closes the window.

#### Scenario: Enter or the field left
- **WHEN** the user types a prefix and presses Enter, or leaves the field
- **THEN** the prefix is saved, and not on any key before
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`)

#### Scenario: A prefix git cannot take
- **WHEN** the user types `jane doe` and presses Enter
- **THEN** the field is marked invalid and nothing is saved
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`)

#### Scenario: Escape
- **WHEN** the user types and presses Escape
- **THEN** the field takes the typing back and the window stays open
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`)

#### Scenario: The window closes with the typing
- **WHEN** the user types a prefix and closes the window by ⌘W
- **THEN** the prefix is saved before the window goes, and the window opened again shows it
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`saves the prefix typed in Settings when the window closes, and the window opened again shows it`)
