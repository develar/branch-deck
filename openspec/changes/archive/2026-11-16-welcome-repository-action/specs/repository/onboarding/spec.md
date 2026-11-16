## MODIFIED Requirements

### Requirement: Welcome card: The card asks one step at a time, each numbered 1

Without a repository the page knows no prefix (the session of a repository resolves it), so the card MUST ask one step
at a time, each numbered 1: "Select a Git Repository…" on a host with a folder dialog ("Select a Git repository"
otherwise), then, for a repository without a prefix, "Choose your branch prefix".

#### Scenario: The step of the repository
- **GIVEN** no repository
- **WHEN** the welcome card shows
- **THEN** its one step asks for a repository, numbered 1, without the field of the prefix
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the welcome card for new users, which asks for the repository first`)

#### Scenario: The step of the prefix
- **GIVEN** a repository without a prefix
- **WHEN** the welcome card shows
- **THEN** its one step is "Choose your branch prefix", numbered 1, and the repository step is gone
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the prefix step as step 1 when the repository has no branch prefix configured`, `keeps Step 2 visible after selecting a repository without branch prefix`)

## ADDED Requirements

### Requirement: Welcome card: The repository step opens the folder picker

On a host with a folder dialog, the whole numbered repository step MUST be a button named "Select a Git Repository…",
described by "Browse to a local Git repository.", with a folder icon, border, pointer cursor, hover, pressed and
keyboard-focus feedback. Click, Enter and Space MUST open the existing folder picker; canceling changes nothing. A host
without a folder dialog MUST instead direct the user to the repository selector.

#### Scenario: The repository step activated
- **WHEN** the user clicks the description of the repository step or activates it with Enter or Space and picks a valid repository
- **THEN** the repository is selected and the welcome card gives way to Ready to Sync
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`selects a repository from the welcome step by ${activation}`)

#### Scenario: The folder dialog canceled
- **WHEN** the user activates the repository step and cancels the folder dialog
- **THEN** the same step remains focused, with no repository selected and no error
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps the welcome step and its focus when the folder dialog is canceled`)

#### Scenario: An invalid folder or a failed dialog
- **WHEN** the user picks a non-git folder through the welcome step, or its folder dialog fails
- **THEN** the existing repository error explains the problem and replaces the welcome card
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`reports a non-git folder selected from the welcome step`, `reports a folder dialog failure from the welcome step`)

#### Scenario: The host has no folder dialog
- **WHEN** the welcome card shows on a host without a folder dialog
- **THEN** the repository step is informational and directs the user to the repository selector, without offering a folder action
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`directs users to the repository selector when the host has no folder dialog`)
