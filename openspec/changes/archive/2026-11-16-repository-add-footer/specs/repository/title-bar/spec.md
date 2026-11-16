## MODIFIED Requirements

### Requirement: Every platform: An empty repository selector explains how to open a repository

The repository selector MUST be named "Repository". With no options, its popup MUST show "No recent repositories" and
"Use Add Repository… to open a Git repository. Repositories you open appear here.", with padded, wrapping,
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


## ADDED Requirements

### Requirement: Every platform: The repository selector offers a fixed Add Repository footer

On hosts with a folder dialog, the popup MUST show a compact "Add Repository…" button with a plus icon, outside its repository options and visible while they scroll, including with no repositories. Pointer and keyboard activation MUST close the popup and use the existing folder dialog and validation flow. The button MUST share the selector's disabled state and be absent on hosts without a folder dialog.

#### Scenario: The first repository is opened
- **GIVEN** the selector shows its empty explanation and the Add Repository… footer
- **WHEN** the user activates the footer by click, Enter or Space and picks a valid repository
- **THEN** the popup closes, the repository is selected and listed, and the empty explanation is gone
  [@test] tests/e2e/repository/header.spec.ts (`should add the first repository from the selector footer by ${activation}`)

#### Scenario: Another repository is opened
- **WHEN** the user activates the footer of a populated selector and picks another repository
- **THEN** that repository becomes selected and both repositories appear as options
  [@test] tests/e2e/repository/header.spec.ts (`should add another repository from the selector footer`)

#### Scenario: The dialog is cancelled
- **WHEN** the user reaches the footer with Tab, activates it with Enter or Space and cancels the folder dialog
- **THEN** the popup closes, focus returns to the selector, and its selected repository and options stay the same
  [@test] tests/e2e/repository/header.spec.ts (`should keep the selected repository when adding from the footer by ${activation} is cancelled`)

#### Scenario: An invalid folder is picked
- **WHEN** the user picks a non-Git folder through the footer
- **THEN** the popup closes and the existing invalid-path feedback appears
  [@test] tests/e2e/repository/header.spec.ts (`should report an invalid folder chosen from the selector footer`)

#### Scenario: A long list scrolls
- **WHEN** the user scrolls through more repository options than fit in the popup
- **THEN** the compact 24px footer stays visible and usable in the same position, outside the listbox
  [@test] tests/e2e/repository/header.spec.ts (`should keep the add footer visible while repository options scroll`)

#### Scenario: The host has no folder dialog
- **WHEN** the selector is opened on an IDE host without a folder dialog
- **THEN** it offers neither the footer nor the adjacent folder button
  [@test] tests/e2e/repository/header.spec.ts (`should explain an empty repository selector without offering a folder dialog on an IDE host`)
