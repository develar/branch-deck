## ADDED Requirements

### Requirement: Every platform: An empty repository selector explains how to open a repository

The repository selector MUST be named "Repository". With no options, its popup MUST show "No recent repositories" and
"Use the folder button to open a Git repository. Repositories you open appear here.", with padded, wrapping,
nonselectable text that describes the open selector. Without a folder dialog it MUST instead say "No repositories
available" and "Open a Git repository in your IDE project." The popup MUST support keyboard opening and Escape with
focus returned to the selector.

#### Scenario: An empty selector opened
- **GIVEN** the welcome card is visible and there are no repository choices
- **WHEN** the user opens the repository selector by click or Arrow Down
- **THEN** its explanation is readable over the card, is not a repository option, and Escape closes it and returns focus
  [@test] tests/e2e/repository/header.spec.ts (`should explain the empty repository selector opened by ${activation} and return focus on Escape`)

#### Scenario: A repository opened from the folder button
- **GIVEN** the selector showed its empty explanation
- **WHEN** the user opens a repository through the folder button and opens the selector again
- **THEN** the repository is listed and the empty explanation is gone
  [@test] tests/e2e/repository/header.spec.ts (`should replace the empty selector with the repository opened through the folder button`)

#### Scenario: The host has no folder dialog
- **WHEN** the user opens an empty selector on an IDE host without a folder dialog
- **THEN** it explains how to open a Git repository in the IDE project and offers no folder button
  [@test] tests/e2e/repository/header.spec.ts (`should explain an empty repository selector without offering a folder dialog on an IDE host`)
