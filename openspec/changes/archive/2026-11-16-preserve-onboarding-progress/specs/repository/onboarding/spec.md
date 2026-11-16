## RENAMED Requirements

- FROM: `### Requirement: Welcome card: The card asks one step at a time, each numbered 1`
- TO: `### Requirement: Welcome card: First-time onboarding keeps both steps visible`

## MODIFIED Requirements

### Requirement: Welcome card: First-time onboarding keeps both steps visible

First-time onboarding MUST show an ordered overview: active repository step 1 and a noninteractive prefix preview numbered 2. After selection it MUST retain a completed repository row with the wrapping path and expand step 2 only for `no_branch_prefix`; while the session opens it MUST say "Checking branch prefix…". Later prefix setup MUST remain standalone step 1.

#### Scenario: The step of the repository
- **GIVEN** first-time onboarding without a repository
- **WHEN** the welcome card shows
- **THEN** step 1 asks for a repository and step 2 previews the prefix task without a form or action
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

## ADDED Requirements

### Requirement: Welcome card: First-time progress survives restarting

The page MUST persist `onboardingStatus` as `inProgress` or `complete` through the settings writer. After settings catch-up, an absent status MUST become `complete` with recent repositories, otherwise `inProgress`. Unfinished onboarding MUST resume across reloads and restarts; cancellation and errors MUST preserve progress. Completion MUST not replay for later repositories.

#### Scenario: Initializing from current settings
- **WHEN** a window starts without saved onboarding status
- **THEN** it waits for settings catch-up before distinguishing a new user from an existing user
  [@test] src/stores/onboarding.test.ts (`initializes after settings catch-up and treats restored recent projects as existing users`, `starts fresh onboarding and waits for the current subscription before completing`)

#### Scenario: Resuming prefix setup
- **WHEN** the app reopens with unfinished onboarding and a selected repository without a prefix
- **THEN** it shows the completed repository and active prefix step 2
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`resumes first-time progress from saved settings in a fresh window without taking focus`, `keeps Step 2 visible after selecting a repository without branch prefix`)
  [@test] src/stores/onboarding.test.ts (`resumes saved onboarding even when recent projects already exist`)

#### Scenario: Cancellation and errors
- **WHEN** the folder picker is canceled, a folder is invalid, or the subscription is refused
- **THEN** progress remains unfinished and a valid retry can continue
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps the welcome step and its focus when the folder dialog is canceled`, `retains first-time progress after an invalid folder and a successful retry`)
  [@test] src/stores/onboarding.test.ts (`keeps progress through a refused subscription and completes after retry`)

#### Scenario: A completion write fails
- **WHEN** the settings writer rolls a completion write back
- **THEN** the same subscription does not repeatedly send completion writes
  [@test] src/stores/onboarding.test.ts (`does not loop completion writes when the settings writer rolls one back`)

#### Scenario: Later prefix setup
- **WHEN** a completed user selects a repository without a prefix
- **THEN** onboarding stays complete and only the standalone prefix task is shown
  [@test] src/stores/onboarding.test.ts (`does not replay completed onboarding for a later repository without a prefix`)
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the prefix step as step 1 when the repository has no branch prefix configured`)

### Requirement: Welcome card: A usable current subscription completes onboarding

First-time onboarding MUST finish automatically when the accepted current repository subscription confirms a usable prefix, whether configured already or saved in onboarding. A draft, suggestion, pending subscription, cached frame or late answer from another repository MUST NOT complete it. Ready to Sync and the repository view MUST retain their existing behavior.

#### Scenario: An existing prefix or a saved prefix
- **WHEN** the current subscription confirms a usable prefix
- **THEN** onboarding finishes and gives way to Ready to Sync or the existing repository view
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`selects a repository from the welcome step by ${activation}`, `keeps Step 2 visible after selecting a repository without branch prefix`)
  [@test] src/stores/onboarding.test.ts (`starts fresh onboarding and waits for the current subscription before completing`)

#### Scenario: A late answer or an unmounted tracker
- **WHEN** an earlier repository answers after a switch, or tracking has stopped
- **THEN** that answer does not initialize or complete the walkthrough
  [@test] src/stores/onboarding.test.ts (`ignores a late subscription from a repository left behind`, `does not initialize or complete after tracking stops`)

### Requirement: Welcome card: Step changes identify progress and carry keyboard focus

The welcome steps MUST form an ordered list, identify the active step with `aria-current="step"`, and expose completed step 1 with a check mark and accessible completion text. Upcoming and completed rows MUST be informational. An in-app repository selection revealing prefix setup MUST focus its input; resuming on launch MUST NOT take focus.

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
