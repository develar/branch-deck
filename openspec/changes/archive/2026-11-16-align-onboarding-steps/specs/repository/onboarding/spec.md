## MODIFIED Requirements

### Requirement: Welcome card: First-time onboarding keeps both steps visible

First-time onboarding MUST show an ordered overview: active repository step 1 and a noninteractive prefix preview numbered 2. After selection it MUST retain a completed repository row numbered 1 with the wrapping path and expand step 2 only for `no_branch_prefix`; while the session opens it MUST say "Checking branch prefix…". Later prefix setup MUST remain standalone step 1.

#### Scenario: The step of the repository
- **GIVEN** first-time onboarding without a repository
- **WHEN** the welcome card shows
- **THEN** step 1 asks for a repository and quieter step 2 says "After selecting a repository, set a personal prefix if needed." without a form or action
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the welcome card for new users, which asks for the repository first`, `directs users to the repository selector when the host has no folder dialog`)

#### Scenario: The step of the prefix
- **GIVEN** a selected repository without a prefix
- **WHEN** the welcome card shows
- **THEN** first-time onboarding keeps the completed repository step and numbers the prefix step 2; later setup shows only prefix step 1
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the prefix step as step 1 when the repository has no branch prefix configured`, `keeps Step 2 visible after selecting a repository without branch prefix`)

#### Scenario: The prefix is not known yet
- **WHEN** the selected repository session has not answered
- **THEN** the completed repository stays visible while step 2 says "Checking branch prefix…"
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps the completed repository visible while checking its prefix`)

#### Scenario: Another repository is selected before prefix setup
- **WHEN** the user switches repositories during unfinished first-time onboarding
- **THEN** the completed row shows the currently selected repository and step 2 uses its session answer
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`updates the completed repository row when switching before prefix setup`)

#### Scenario: Consistent columns and guidance
- **WHEN** the welcome card shows before or after repository selection, in either theme or a narrow window
- **THEN** all step numbers and content share aligned columns, and selecting a repository does not shift step 1 horizontally
- **AND** the footer says "Once your repository and branch prefix are ready, you can sync and start organizing your commits."
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps onboarding columns aligned through selection in ${theme} theme`)

### Requirement: Welcome card: Step changes identify progress and carry keyboard focus

The welcome steps MUST form an ordered list, identify the active step with `aria-current="step"`, and retain completed step 1's number with a success check beside "Repository selected" and accessible completion text. Upcoming and completed rows MUST be informational. An in-app repository selection revealing prefix setup MUST focus its input; resuming on launch MUST NOT take focus.

#### Scenario: Repository selection reveals prefix setup
- **WHEN** the user selects a repository without a prefix by pointer or keyboard
- **THEN** step 1 is marked complete and focus moves to the input of active step 2
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps Step 2 visible after selecting a repository without branch prefix`, `focuses prefix step 2 after repository selection by ${activation}`)

#### Scenario: Focus returns after the native picker
- **WHEN** prefix setup appears before the native picker returns focus to the page
- **THEN** the input receives keyboard focus after the page returns, including the missing-prefix subscription retry
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`focuses prefix step 2 after the native picker returns focus to the page`)

#### Scenario: A resumed walkthrough
- **WHEN** the app starts with saved progress at prefix setup
- **THEN** the input is available without taking focus
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`resumes first-time progress from saved settings in a fresh window without taking focus`)

### Requirement: Welcome card: The repository step opens the folder picker

On a host with a folder dialog, the whole numbered repository step MUST be a button named "Select a Git Repository…",
described by "Browse to a local Git repository.", with an action-colored title, adjacent folder icon, no visible border,
pointer cursor, hover, pressed and keyboard-focus feedback. Click, Enter and Space MUST open the existing folder picker; canceling changes nothing. A host
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

#### Scenario: The repository action gives interaction feedback
- **WHEN** the user hovers, presses or keyboard-focuses the repository row
- **THEN** the borderless row provides visible feedback and the upcoming prefix row stays outside the keyboard sequence
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`provides borderless repository action feedback and skips the upcoming step`)

## ADDED Requirements

### Requirement: Welcome card: Step colors consistently show progress

Active step numbers and titles MUST use the primary color, upcoming steps MUST use muted numbers and titles, and completed steps MUST use neutral numbers and titles with a success check. The active-title rule MUST apply during prefix loading, standalone prefix setup and on hosts without a folder picker.

#### Scenario: Repository selection changes the active step
- **WHEN** repository selection advances onboarding in either theme
- **THEN** the prefix title and number share the active color, the completed repository title becomes neutral, and the upcoming prefix title was muted before selection
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps onboarding columns aligned through selection in ${theme} theme`)

#### Scenario: Other active welcome states
- **WHEN** the welcome card checks a prefix, shows standalone prefix setup, or directs a host without a picker to select a repository
- **THEN** the active title and number share the primary color
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps the completed repository visible while checking its prefix`, `shows the prefix step as step 1 when the repository has no branch prefix configured`, `directs users to the repository selector when the host has no folder dialog`)
