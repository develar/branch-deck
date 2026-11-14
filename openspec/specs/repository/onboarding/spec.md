---
name: Onboarding
description: Requirements for the first run of Branch Deck, from an empty window to the first branches: the welcome card with the branch prefix step, the card before the first sync, the grouping hint, and the git, identity and repository problems that block the way.
style: plain-1
targets:
  - src/components/branch/BranchCreator.tsx
  - src/components/repository/WelcomeCard.tsx
  - src/components/repository/ReadyToSyncCard.tsx
  - src/components/repository/GitSetupAlert.tsx
  - src/components/repository/BranchPrefixHelp.tsx
  - src/components/settings/BranchPrefixSetting.tsx
  - src/components/repository/SyncButton.tsx
  - src/components/repository/AgentsHint.tsx
  - src/components/shared/InstallAgentCliButton.tsx
  - src/components/branch/BranchTableCard.tsx
  - src/hooks/useAgentCliStatus.ts
  - src/stores/agent-cli.ts
  - src/lib/agents-hint.ts
  - src/lib/sync-step.ts
  - src/lib/sync-deferred.ts
  - crates/repository-model/src/table.rs
  - crates/repository-model/src/runtime.rs
  - crates/git-ops/src/rewrite_guard.rs
  - src/hooks/useSyncOnOpen.ts
  - src/model/store.ts
  - src/router.ts
  - src-tauri/src/warm_start.rs
  - crates/app-api/src/app/warm_start.rs
  - crates/repository-model/src/warm_start.rs
  - crates/app-settings/src/projects.rs
  - crates/repository-model/src/sync_scheduler.rs
  - crates/sync-core/src/synced_mark.rs
  - crates/sync-core/src/sync_inputs.rs
  - crates/repository-model/src/session.rs
  - src/stores/settings/repositoryState.ts
  - src/components/unassigned/UnassignedCommitListCard.tsx
  - src/components/unassigned/UnassignedCommitsHelpPopover.tsx
  - src/components/shared/CommandLine.tsx
  - src/components/shared/InlineBranchTarget.tsx
  - src/components/ai/BranchNameSuggestions.tsx
  - src/hooks/useInstallAgentCli.ts
  - src/components/settings/AgentsPage.tsx
  - src/lib/branch-prefix.ts
  - src/stores/repository.ts
  - crates/app-api/src/reads/repository.rs
  - crates/app-settings/src/resolve.rs
  - crates/app-settings/src/branch_prefixes.rs
  - crates/app-settings/src/service.rs
  - crates/app-api/src/session/view.rs
  - crates/app-api/src/context.rs
  - src/stores/branch-sync.ts
  - src/components/repository/ConfigurationHeader.tsx
  - src/lib/time.ts
  - src-tauri/src/menu.rs
  - src/pages/IndexPage.tsx
  - src/hooks/useMenuSync.ts
  - crates/sync-core/src/git_setup.rs
  - crates/app-api/src/api_error.rs
  - crates/git-ops/src/commit_writer.rs
  - src/hooks/useSyncAvailability.ts
  - crates/sync-core/src/branch_prefix.rs
  - crates/sync-core/src/repository_validation.rs
  - crates/app-api/src/app/desktop.rs
  - crates/test-server/src/host.rs
  - crates/history-rewrite/src/baseline.rs
  - crates/git-executor/src/git_info.rs
  - crates/git-ops/src/commit_chain.rs
  - crates/git-ops/src/commit_list.rs
  - crates/repository-model/src/branch_table.rs
  - crates/domain-types/src/repository_view.rs
---

# Onboarding

Status: Implemented
Date: 2026-10-25

## Purpose

A new user opens an empty window. Branch Deck needs four things before it shows a branch: Git 2.54 or newer, a
repository, a branch prefix, and a sync. The welcome card walks through the repository and the prefix, the prefix
pre-filled from the Git identity; a card before the first sync says what Sync does, and the unassigned commits say how
commits become branches. What blocks the way (Git missing, a folder that is not a repository, no commits yet) is an
alert with the reason and the next step, never a raw Git error.

## Goals

- From the first window to the first branch without reading documentation or opening a terminal.
- Every blocking problem names its fix: the command to install Git, the commands to set the identity.
- Nothing is written to the repository or to the settings without an explicit action of the user.

## Non-goals

- No guided tour, no shortcut overlay: the help sits where it is used (popovers, tooltips, the prefix guide).
- No automatic first sync: Sync creates branches and notes in the repository, so the user starts it.
- Optional integrations explain themselves where they apply, and their hint opens the setting.

## Requirements

### Requirement: Git and the identity: No check of git runs before the first sync

No check of git MUST run before the first sync, and the page MUST read nothing for it: git and the identity are
answered by the code of what needs them.

#### Scenario: The window opens a repository
- **WHEN** the window opens a repository, before its first sync
- **THEN** no check of git runs and the page reads nothing for it
- **AND** git and the identity are answered by the code of what needs them

### Requirement: Git and the identity: Without a usable git the subscription fails with `git_not_found`

The subscription of the window is the first command that runs git (it reads the prefix from the git config,
`session_key_now`); without a usable git (missing, failing or older than 2.54) it MUST fail with `git_not_found`
(`GitNotFound` of the discovery, `app_api::api_error`), whose words are the summary and how to install or upgrade Git.

#### Scenario: Git is missing
- **GIVEN** the git Branch Deck is told to run is missing
- **WHEN** the subscription of the window runs git
- **THEN** it fails with `git_not_found`, whose words are the summary and how to install Git
  [@test] crates/git-executor/src/git_info.rs (`a_missing_explicit_git_is_not_found_with_the_hint`)

#### Scenario: Git fails
- **GIVEN** git fails
- **WHEN** the subscription of the window runs git
- **THEN** it fails with `git_not_found`, its first error line in the summary
  [@test] crates/git-executor/src/git_info.rs (`a_failing_git_is_not_found_with_its_first_error_line`)

#### Scenario: Git is older than 2.54
- **GIVEN** git is older than 2.54
- **WHEN** the subscription of the window runs git
- **THEN** it fails with `git_not_found`, whose words are the summary and how to upgrade Git
  [@test] crates/git-executor/src/git_info.rs (`an_old_git_is_too_old_with_the_hint`)

#### Scenario: The error of the app
- **WHEN** the discovery fails with `GitNotFound`
- **THEN** `app_api::api_error` answers it with the code `git_not_found`, the words of the discovery its message
  [@test] crates/app-api/src/api_error_test.rs (`api_error_answers_the_setup_of_git_by_its_code`)

### Requirement: Git and the identity: Git missing or too old is an alert above everything else

Git missing or too old MUST be a `destructive` alert above everything else: the title is the summary ("Cannot find
Git: …", "Git 2.39 is too old: Branch Deck requires Git 2.54 or newer"), the description how to install or upgrade it
on this platform, then "Check Again", which subscribes again.

#### Scenario: Git is missing, then found
- **GIVEN** git is missing
- **WHEN** the window opens the repository
- **THEN** a `destructive` alert above everything else says so, with how to install it on this platform
- **AND** "Check Again" subscribes again and opens the repository once it finds git
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says above everything else that git is missing, and opens the repository once Check Again finds it`)

### Requirement: Git and the identity: The alert of git replaces the path and sync errors

The alert of git missing or too old MUST replace the path and sync errors, which would say the same; the welcome card
MUST be hidden, and Sync disabled with the summary as its reason.

#### Scenario: The alert of git is shown
- **GIVEN** git is missing
- **WHEN** the alert of git shows
- **THEN** no path or sync error shows beside it
- **AND** the welcome card is hidden, Sync disabled with the summary as its reason
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says above everything else that git is missing, and opens the repository once Check Again finds it`)

### Requirement: Git and the identity: An identity git cannot form fails the write with `no_identity`

An identity git cannot form MUST fail the write that needs it with `no_identity` (`NoIdentity` of `current_committer`:
the notes of a sync and of its detection, which `write_cache_notes` never swallows, a rewritten commit), in the app as
in `brd`: a sync that wrote no notes would copy every commit again on each run.

#### Scenario: The committer of a write
- **GIVEN** git cannot form the committer's identity
- **WHEN** a write needs the committer
- **THEN** it fails with `NoIdentity` of `current_committer`
  [@test] crates/git-ops/src/commit_writer_test.rs (`a_committer_git_cannot_form_is_no_identity`)

#### Scenario: The notes of a sync
- **GIVEN** git cannot form the committer's identity
- **WHEN** a sync writes its notes
- **THEN** it fails with `no_identity`, as `write_cache_notes` never swallows the error
  [@test] crates/sync-core/src/sync_test.rs (`a_sync_whose_notes_git_cannot_sign_fails_with_no_identity`)
  [@test] crates/test-server/src/session_routes_test.rs (`a_sync_without_an_identity_git_can_form_fails_with_no_identity`)

#### Scenario: The code of the error
- **WHEN** a command fails with `NoIdentity`
- **THEN** its error answers the code `no_identity`
  [@test] crates/app-api/src/api_error_test.rs (`api_error_answers_the_setup_of_git_by_its_code`)

### Requirement: Git and the identity: A sync without an identity is the warning of the name and email

A sync that failed with `no_identity` MUST be the `warning` alert "Git does not know your name and email" with the two
`git config --global` commands (each with a copy button) and "Sync Again", which syncs again, instead of the error
alert.

#### Scenario: The warning and Sync Again
- **GIVEN** a sync failed with `no_identity`
- **WHEN** the window shows the failure
- **THEN** the `warning` alert "Git does not know your name and email" shows the two `git config --global` commands,
  each with a copy button, instead of the error alert
- **AND** "Sync Again" syncs again
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`warns that git does not know the user's name and email when a sync fails on it, and syncs on Sync Again`)

### Requirement: Git and the identity: The error alert splits a message of several lines

A message of several lines in the error alert MUST show its first line as the title, the rest as the description.

#### Scenario: A message of several lines
- **WHEN** the error alert shows a message of several lines
- **THEN** its first line is the title and the rest the description

### Requirement: Welcome card: The card asks for the repository on the first run

The welcome card MUST show on the first run (no repository, no recent projects), asking for the repository.

#### Scenario: A new user
- **GIVEN** no repository and no recent projects
- **WHEN** the window opens
- **THEN** the welcome card asks for the repository
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the welcome card for new users, which asks for the repository first`)

### Requirement: Welcome card: The card asks for the prefix while the subscription is refused for it

The welcome card MUST show while the subscription of the selected repository is refused for its prefix
(`no_branch_prefix`), asking for the prefix.

#### Scenario: A repository without a branch prefix
- **GIVEN** the selected repository has no branch prefix
- **WHEN** its subscription is refused with `no_branch_prefix`
- **THEN** the welcome card asks for the prefix
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the prefix step as step 1 when the repository has no branch prefix configured`)

#### Scenario: A repository without a prefix is selected
- **GIVEN** the welcome card asks for the repository
- **WHEN** the user selects a repository without a branch prefix
- **THEN** the welcome card stays, asking for the prefix
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps Step 2 visible after selecting a repository without branch prefix`)

### Requirement: Welcome card: The card is hidden while an error alert explains a problem

The welcome card MUST be hidden while an error alert explains a problem.

#### Scenario: A folder that is not a repository
- **WHEN** the user selects a folder that is not a repository and the error alert explains it
- **THEN** the welcome card is hidden
  [@test] tests/e2e/repository/browse.spec.ts (`should show error for non-git directory`)

### Requirement: Welcome card: The card asks one step at a time, each numbered 1

Without a repository the page knows no prefix (the session of a repository resolves it), so the card MUST ask one step
at a time, each numbered 1: "Select a Git repository", then, for a repository without a prefix, "Choose your branch
prefix".

#### Scenario: The step of the repository
- **GIVEN** no repository
- **WHEN** the welcome card shows
- **THEN** its one step is "Select a Git repository", numbered 1, without the field of the prefix
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the welcome card for new users, which asks for the repository first`)

#### Scenario: The step of the prefix
- **GIVEN** a repository without a prefix
- **WHEN** the welcome card shows
- **THEN** its one step is "Choose your branch prefix", numbered 1, and "Select a Git repository" is gone
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`shows the prefix step as step 1 when the repository has no branch prefix configured`, `keeps Step 2 visible after selecting a repository without branch prefix`)

### Requirement: Welcome card: The field of the prefix step is pre-filled with the suggested prefix

The prefix step MUST have a field pre-filled with the prefix the refusal suggests (`ErrorDetails::PrefixSuggestion`,
`prefix_suggestion`: one `git var GIT_COMMITTER_IDENT` in the repository, only on the refusal): the local part of the
committer email, without a `+tag`, sanitized as a branch name.

#### Scenario: The local part of the committer email
- **WHEN** the committer email has a local part, maybe with a `+tag`
- **THEN** the suggestion is that local part without the `+tag`, sanitized as a branch name
  [@test] crates/sync-core/src/git_setup_test.rs (`suggests_the_local_part_of_the_committer_email`)

#### Scenario: The identity of the repository
- **WHEN** the repository's own git config sets the committer's identity
- **THEN** the suggestion is of that identity, read by one `git var GIT_COMMITTER_IDENT` in the repository
  [@test] crates/sync-core/src/git_setup_test.rs (`suggests_the_prefix_of_the_identity_of_the_repository`)

#### Scenario: The refusal carries the suggestion
- **WHEN** the subscription of a repository without a prefix is refused
- **THEN** its error is `no_branch_prefix` with `ErrorDetails::PrefixSuggestion`
  [@test] crates/test-server/src/session_routes_test.rs (`a_repository_without_a_prefix_is_refused_with_the_api_error_of_the_app`)

#### Scenario: The field shows the suggestion
- **GIVEN** a repository whose committer is test@example.com, without a prefix
- **WHEN** the prefix step shows
- **THEN** its field holds `test`
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`pre-fills the prefix from the git identity and saves it only on Save`)

### Requirement: Welcome card: Save stores the prefix as the app's branch prefix

Save (or Enter) MUST store the prefix of the field as the app's branch prefix (used by `brd` too); nothing MUST be
stored before.

#### Scenario: Nothing before Save
- **GIVEN** the field is pre-filled
- **WHEN** the user has not pressed Save
- **THEN** nothing is stored: Sync still waits for a prefix
- **AND** Save stores it as the app's branch prefix, and the welcome card goes
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`pre-fills the prefix from the git identity and saves it only on Save`)

#### Scenario: Enter saves
- **WHEN** the user presses Enter in the field
- **THEN** the prefix is stored as the app's branch prefix
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`saves a typed prefix without its trailing slash and refuses one git cannot take`)

### Requirement: Welcome card: The prefix step says how the branches will be named

Below the field the prefix step MUST say: "Branches will be named `<prefix>/virtual/<name>`. You can change it later in
Settings."

#### Scenario: The line below the field
- **WHEN** the field holds `test`
- **THEN** the line below it says the branches will be named `test/virtual/<name>`
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`pre-fills the prefix from the git identity and saves it only on Save`)

### Requirement: Welcome card: A prefix is trimmed and loses its trailing slashes

A prefix MUST be trimmed and lose its trailing slashes, as the backend names the branch (`qualified_branch_name`).

#### Scenario: A prefix with a trailing slash
- **WHEN** the user types `jane/` and saves it
- **THEN** the prefix stored is `jane`
  [@test] src/lib/branch-prefix.test.ts (`normalizeBranchPrefix`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`saves a typed prefix without its trailing slash and refuses one git cannot take`)

### Requirement: Welcome card: A prefix git cannot take is refused in place

A prefix with a space, `..`, `@{` or any of `~ ^ : ? * [ \` MUST be refused in place, Save disabled.

#### Scenario: A prefix with a space
- **WHEN** the user types `jane doe`
- **THEN** the field is marked invalid and Save is disabled
  [@test] src/lib/branch-prefix.test.ts (`isValidBranchPrefix`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`saves a typed prefix without its trailing slash and refuses one git cannot take`)

### Requirement: Welcome card: The alternative is the global git command

The prefix step MUST offer the alternative, the command `git config --global branchdeck.branchPrefix your-name` with a
copy button.

#### Scenario: The command of the alternative
- **WHEN** the prefix step shows
- **THEN** it shows `git config --global branchdeck.branchPrefix your-name` with a copy button

### Requirement: Welcome card: View guide opens the branch prefix guide

"View guide" MUST open the branch prefix guide: the field of Settings first ("takes precedence over the global Git
setting"), with "Open Settings", then the global command, the grouping examples, and the prefix of one repository on
Settings → Branches, the tab of the repository, which takes precedence over the one of Branch Deck, with "Open
Repository Settings".

#### Scenario: The guide from the welcome card
- **WHEN** the user presses "View guide" on the welcome card
- **THEN** the guide opens: the field of Settings, the global command, the grouping examples and the prefix of one
  repository
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`opens branch prefix help modal from welcome card`)

### Requirement: Welcome card: Open Settings of the guide opens the field of the prefix

"Open Settings" of the guide MUST open Settings → Branches with the field focused; the guide Settings itself opens
MUST NOT have it.

#### Scenario: Open Settings
- **WHEN** the user presses "Open Settings" in the guide
- **THEN** the guide closes and Settings → Branches opens with the field of the prefix focused
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`opens Settings at the field of the prefix from the guide, and at the one of the repository`)

#### Scenario: The guide in Settings
- **WHEN** Settings opens the guide of the branch prefix
- **THEN** the guide has no "Open Settings"
  [@test] tests/e2e/settings/settings.spec.ts (`should open the guide of the branch prefix without a button that opens Settings`)

### Requirement: Welcome card: Open Repository Settings of the guide opens the field of the repository

"Open Repository Settings" of the guide MUST open Branches on the tab of the repository, its field focused.

#### Scenario: Open Repository Settings
- **WHEN** the user presses "Open Repository Settings" in the guide
- **THEN** the guide closes and Settings → Branches opens on the tab of the repository, its field of the prefix focused
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`opens Settings at the field of the prefix from the guide, and at the one of the repository`)

### Requirement: Welcome card: Once set, the prefix is changed in Settings only

Once set, the prefix MUST be changed in Settings → Branches (⌘,), "All repositories" ([Settings](../settings/spec.md)):
the toolbar MUST have no field for it, as the prefix is set once and the welcome card asks for it while it is missing.

#### Scenario: The prefix after Save
- **GIVEN** the prefix was saved from the welcome card
- **WHEN** the user looks for it
- **THEN** the header has no field of the prefix, and Settings → Branches shows it
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`pre-fills the prefix from the git identity and saves it only on Save`)

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

### Requirement: Welcome card: An empty field applies the prefix of the global git config

Empty, the field MUST apply the prefix of the global git config (the window asks Rust itself, `resolve_branch_prefix`),
shown as the placeholder.

#### Scenario: The field is empty
- **WHEN** the field of the app's prefix is empty
- **THEN** the prefix of the global git config applies, and the field shows it as the placeholder

### Requirement: Welcome card: A change of the app's prefix subscribes the window again

The session of a repository is keyed by the prefix Rust resolves: a change of the app's prefix (once the writer has
it) MUST subscribe the window again, holding the view it shows (`resubscribeSession`); the same session answers nothing
new and the rows stay, the session of another prefix answers its view, which replaces them, and syncs as it opens for
a repository synced before.

#### Scenario: The same session or another one
- **WHEN** the app's prefix changes and the window subscribes again holding its view
- **THEN** a prefix that names the same session answers nothing new and the rows stay
- **AND** the session of another prefix answers its view, which replaces the rows
  [@test] src/model/store.test.ts (`subscribes again holding its view: the same session answers nothing, another one's view replaces the rows`)

### Requirement: Welcome card: A sync never waits for a load of the page

A sync MUST NOT wait for a load of the page, so a busy machine fails none for a timeout.

#### Scenario: A sync asked early
- **WHEN** a sync is asked for
- **THEN** it names only the repository, the session resolves the prefix, and it waits for no load of the page
  [@test] src/model/store.test.ts (`names only the repository: the session resolves the prefix, and a sync waits for no load of the page`)

### Requirement: Welcome card: Without a prefix the window holds no session

Without a prefix the window MUST hold no session, and nothing MUST be logged as an error: the welcome card asks for
one.

#### Scenario: A repository without a prefix
- **WHEN** the subscription of a repository without a prefix fails
- **THEN** the window holds no session and nothing is logged as an error
- **AND** a sync asked then subscribes again, names the failure and syncs nothing
  [@test] src/model/store.test.ts (`subscribes first when asked before the window subscribed, and again after a failed subscription, whose failure it names`)

### Requirement: Welcome card: The repository's own prefix wins over the app's

The specific prefix MUST win, in the app as in `brd` ([Settings](../settings/spec.md#branch-prefix-order)): a
repository whose own git config (local or worktree, or `git -c`) sets `branchdeck.branchPrefix` names its branches by
it, whatever the field says; the field says so ("This repository uses its own prefix `user-name`", with "Open
Repository Settings") and names the branches the typed prefix gives elsewhere.

#### Scenario: A repository with its own prefix
- **GIVEN** the repository's own git config sets `branchdeck.branchPrefix` to `user-name`
- **WHEN** the user saves the app's prefix `jane` in Settings
- **THEN** the branches of the repository are named by `user-name`
- **AND** the field says "This repository uses its own prefix `user-name`" and names the branches `jane` gives elsewhere
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`names the branches by the repository's own prefix over the app's, and says so in Settings`)

### Requirement: Welcome card: Without the repository's own prefix the app's prefix applies

Without a prefix of the repository, the app's prefix MUST apply, then the global git prefix.

#### Scenario: A repository without its own prefix
- **GIVEN** the repository's own git config sets no prefix
- **WHEN** the user saves the app's prefix `jane`
- **THEN** the branches of the repository are named by `jane`
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`names the branches by the app's prefix where the repository has none`)

### Requirement: Before the first sync: Sync is disabled while the repository has no branch prefix

Sync MUST be disabled while the selected repository has no branch prefix (not while its subscription has not
answered), with the tooltip "Set the branch prefix first".

#### Scenario: A repository without a prefix
- **GIVEN** the selected repository has no branch prefix
- **WHEN** the user hovers Sync, or presses ⌘R
- **THEN** Sync is disabled with the tooltip "Set the branch prefix first", and no sync starts
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`disables Sync with the reason while no branch prefix is set`)

### Requirement: Before the first sync: The page shows the prefix of the view of the session

The page MUST read nothing before the subscription of the window (no prefix, no check of git) and MUST show the prefix
of the view of the session (`TableHeader.prefix`, also before any sync and in the frame of the warm start): the prefix
Rust resolved for the subscription from the git config as it is then (`session_key_now`, which the commands after it
resolve by).

#### Scenario: The prefix of the view
- **WHEN** the subscription of the window answers
- **THEN** the page names the branches by the prefix of the view, and by none after a failed subscription
  [@test] src/model/store.test.ts (`names the branches by the prefix of the view, and none after a failed subscription, which forgets the rows of the seed`)

#### Scenario: Before any sync
- **GIVEN** a session no sync ran in yet
- **WHEN** a window subscribes to it
- **THEN** its view names the prefix of its key
  [@test] crates/repository-model/src/session_test.rs (`a_session_without_subscribers_runs_nothing_and_goes_away`)

#### Scenario: The prefix the git config names now
- **WHEN** the git config names another prefix and the window subscribes again
- **THEN** the subscription opens the session under the prefix the config names now, and a sync after it runs in that
  session
  [@test] crates/test-server/src/session_routes_test.rs (`a_subscription_opens_the_session_under_the_prefix_the_config_names_now`)

#### Scenario: The frame of the warm start
- **WHEN** the window starts warm
- **THEN** it asks only for the subscription, and shows the rows of the frame under the prefix of its view
  [@test] tests/e2e/startup/warm-start.spec.ts (`shows the rows of the frame before the first sync, asking only for the subscription, and the first sync's table after it`)

### Requirement: Before the first sync: A sync names only the repository and Rust resolves the prefix

The page MUST decide nothing by the prefix: a sync names only the repository and resolves the prefix in Rust from the
git config it reads, waiting for no load of the page, and refuses a repository without one.

#### Scenario: The request of a sync
- **WHEN** the page asks for a sync
- **THEN** the request names only the repository and waits for no load of the page
  [@test] src/model/store.test.ts (`names only the repository: the session resolves the prefix, and a sync waits for no load of the page`)

#### Scenario: The prefix Rust resolves
- **WHEN** a sync runs
- **THEN** the repository's own prefix wins, and a repository without any prefix is not synced
  [@test] crates/test-server/src/branch_prefix_test.rs (`the_repository_s_own_prefix_wins_and_a_repository_without_any_is_not_synced`)

### Requirement: Before the first sync: The Ready to Sync card says what Sync does

With a session that does not sync of itself and a prefix, the window MUST show the "Ready to Sync" card: what Sync does
(groups the commits by the prefix of their subject, creates `<prefix>/virtual/<name>`, lists the rest as unassigned),
that the working tree and the checked-out branch stay as they are, and when it syncs from then on (when it opens the
repository, after each change it makes, and on focus after a commit, a fetch or a push elsewhere, Auto-sync on Focus).

#### Scenario: A repository never synced
- **GIVEN** a repository never synced, with the prefix `user-name`
- **WHEN** the window opens it
- **THEN** the "Ready to Sync" card names `user-name/virtual/<name>` and says that the working tree and the
  checked-out branch stay as they are
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says what Sync does before the first sync, then gives way to the branches`)

### Requirement: Before the first sync: The page reads whether the session syncs of itself from its view

Whether the session syncs of itself MUST be what its view says (`RepositoryView.synced`: synced before, by the app or by
`brd`, or a sync of the session committed its table): the Ready to Sync card, the sync after a write of Settings and the
Sync asked again read it, and only what shows "Last synced" reads `lastSyncTime` (`last-sync-read` of `pnpm lint:arch`).

#### Scenario: The view says whether the session syncs of itself
- **WHEN** the session of a worktree never synced opens, `brd sync` marks the worktree, or a sync of the session
  commits its first table
- **THEN** its view says so (`synced`), in its first view or by a patch, and the page takes it from there
  [@test] crates/repository-model/src/session_test.rs (`the_view_says_the_session_syncs_of_itself_once_the_worktree_was_synced`)
  [@test] src/model/store.test.ts (`says the repository was synced before as its session does: by the view of one synced by the app or by brd, by the patch of the first table`)
  [@test] scripts/lint-arch.test.mjs (`a read of the time of the last sync is refused outside what shows Last synced: the session says it synced before`)

### Requirement: Before the first sync: The Sync button of the card syncs, and the card goes

The Sync button of the Ready to Sync card MUST sync (with the spinner while it runs, its label stays "Sync"); the card
MUST go when the sync completes, or give way to the error alert.

#### Scenario: Sync of the card
- **WHEN** the user presses Sync of the card
- **THEN** it syncs, its label staying "Sync" under the spinner
- **AND** the card goes when the sync completes, the branches in its place
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says what Sync does before the first sync, then gives way to the branches`)

#### Scenario: The sync fails
- **WHEN** the sync of the card fails
- **THEN** the card gives way to the error alert

### Requirement: Before the first sync: A repository synced before syncs when its session opens

A repository synced before MUST sync when its session opens, at start and on a switch, and when a window comes back to
a session kept idle (then only when what a sync reads changed), also with Auto-sync on Focus off: no card, the spinner
of Sync in the header, then the branches and Update.

#### Scenario: The window opens a repository synced before
- **GIVEN** a repository synced an hour ago, Auto-sync on Focus off
- **WHEN** the window opens it
- **THEN** it syncs with no click and no card, and the branches and Update show
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs a repository synced before when the window opens it`)

#### Scenario: The window opens a worktree brd synced
- **GIVEN** a worktree an agent's `brd sync` synced, which the app never synced
- **WHEN** the window opens it
- **THEN** it syncs with no click: Sync spins in the header and the table loads, no Ready to Sync card, then the
  branches show
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs a worktree that brd synced when the window opens it, though the app never synced it: no Ready to Sync card`)

#### Scenario: A window comes back to a kept session
- **GIVEN** a session kept idle after its window left
- **WHEN** a window watches it anew
- **THEN** it syncs only when what a sync reads changed, else the inputs are read once and nothing runs
  [@test] crates/repository-model/src/session_test.rs (`a_window_that_watches_a_kept_session_anew_syncs_it_when_the_inputs_changed`)

### Requirement: Before the first sync: The session starts the sync of the open itself

The session MUST start the sync of the open itself, with or without a window (`SessionHost::synced_before`), so the
page asks for none and a focus during it is one check after it.

#### Scenario: The open of a session
- **WHEN** the session of a repository synced before opens
- **THEN** it starts the sync itself, with or without a window, and the page asks for none
- **AND** a focus during it is one check after it

### Requirement: Before the first sync: A failed sync of the open is not repeated

A failed sync MUST be the error alert and MUST NOT be repeated until the window opens the repository again (⌘R
retries).

#### Scenario: The sync of the open fails
- **WHEN** the sync of the open fails
- **THEN** the error alert shows, and no sync repeats it until the window opens the repository again or the user
  presses ⌘R

### Requirement: Before the first sync: Synced before is what the last sync recorded

Synced before MUST be what the last sync recorded of the repository in `repositoryState` (when it ended, the "Last
synced" of the header, and how many branches it had, the line under the path in the repository selector), which the
backend keeps beside the settings (`repository-state.json`): a sync MUST NOT rewrite `settings.json`.

#### Scenario: The state of a project
- **WHEN** the state of a project has the time of a sync
- **THEN** the project was synced before
  [@test] crates/app-settings/src/projects_test.rs (`a_project_was_synced_before_when_its_state_has_the_time_of_a_sync`)

#### Scenario: Last synced in the header
- **WHEN** a sync ended and the clock moves on
- **THEN** the header says "Last synced just now", then ages it without a sync
  [@test] tests/e2e/update-project/update-project.spec.ts (`should age the last sync next to the Update button while it is shown`)

### Requirement: Before the first sync: The main window records every run of its session that ended

The main window MUST record every run of its session that ended, the ones no request of it waited for too (the sync of
the open, of a rewrite, of `brd`).

#### Scenario: Runs that ended
- **WHEN** a run of the session ends
- **THEN** the window counts it and records it as the sync of the repository: when it ended and how many branches it
  had
  [@test] src/model/store.test.ts (`keeps Sync busy from the request to the last patch of the run that serves it, and counts the runs that ended`)

### Requirement: Before the first sync: The start reads the frame the session saved last

The app MUST start warm: before the main window is built, Rust reads the frame the session of the selected project
saved last (`repository_model::warm_start`: the rows, the stacks, the sections and the facts of the remote as its last
sync and its last answers left them) from the object cache of the repository, without a git process (the common git
directory found by the files of the worktree), beside the prefix the git config names now (the one the subscription of
the window resolves).

#### Scenario: The selected project
- **WHEN** the app starts
- **THEN** the selected project is the first recent one
  [@test] crates/app-settings/src/projects_test.rs (`the_selected_project_is_the_first_recent_one`)

#### Scenario: The common git directory without git
- **WHEN** the start reads the frame of a main or a linked worktree
- **THEN** it finds the common git directory by the files of the worktree, without a git process
  [@test] crates/app-api/src/app/warm_start_test.rs (`the_common_dir_is_found_by_the_files_of_the_main_and_of_a_linked_worktree`)

### Requirement: Before the first sync: The start opens the session seeded from the frame

The start MUST open the session of that prefix seeded from the frame, whose first sync starts at once, and put its seed
into the page as `window.__BD_MODEL__`.

#### Scenario: The seeded session
- **WHEN** the start opens the session from the frame
- **THEN** the session answers its seed and syncs, with no window
  [@test] crates/repository-model/src/session_test.rs (`a_session_opened_from_the_frame_of_the_start_answers_its_seed_and_syncs_with_no_window`)

#### Scenario: The model of the page
- **WHEN** the start puts the seed into the page
- **THEN** `window.__BD_MODEL__` is the JSON of the path and the view, in a string that `JSON.parse` reads
  [@test] src-tauri/src/warm_start_test.rs (`the_model_is_the_json_of_the_path_and_the_view_in_a_string_that_json_parse_reads`)

### Requirement: Before the first sync: The page shows the rows of the seed on its first frame

The page MUST show the rows of the seed on its first frame (`data-cached` on the table) and subscribe holding the seed
(`subscribe_view` with `have`): the answer is the patch from it to the view of the session, or nothing; the rows come
with that one command and its channel.

#### Scenario: The rows of the frame before the first sync
- **WHEN** the window starts warm
- **THEN** it shows the rows of the frame, `data-cached` on the table, asking only for the subscription
- **AND** the first sync's table replaces them after it
  [@test] tests/e2e/startup/warm-start.spec.ts (`shows the rows of the frame before the first sync, asking only for the subscription, and the first sync's table after it`)

#### Scenario: The seed catches up
- **WHEN** the window subscribes holding the seed
- **THEN** the view of the seed is the frame at the first version, and the answer is the patch from it to the view of
  the session, the patches before the answer applied after it
  [@test] crates/repository-model/src/state_test.rs (`a_seeded_view_is_the_frame_at_the_first_version_and_a_window_that_holds_it_catches_up`)
  [@test] src/model/store.test.ts (`shows the rows of the seed before anything answers, and catches up by the patch from it, the patches before the answer after it`)

#### Scenario: The session sent nothing since
- **WHEN** the session sent nothing since the seed
- **THEN** the answer is nothing, and the page keeps the seed
  [@test] src/model/store.test.ts (`keeps the seed when the session sent nothing since, and leaves a model of another project`)

### Requirement: Before the first sync: Every action of the remote on the rows of the frame waits

Until the first sync commits, the rows are of the frame (`RepositoryView.cached`): a push and every other action of the
remote (on the merge route the merge of the pull request and Cancel Auto-Merge too, the close of a pull request of a
stack that is no more) MUST wait for it ("Waits for the first sync").

#### Scenario: The actions of a frame nobody checked
- **GIVEN** the rows of the frame
- **WHEN** the actions of a row are decided
- **THEN** every action of the remote waits for the first sync and the rest stays
  [@test] crates/domain-policy/src/actions_test.rs (`on_a_frame_nobody_checked_every_remote_action_waits_for_the_first_sync_and_the_rest_stays`)
  [@test] crates/repository-model/src/session_test.rs (`a_seeded_session_shows_the_steps_and_the_actions_of_its_rows_a_push_waiting_for_the_first_sync`)

#### Scenario: A push of the rows of the frame
- **GIVEN** the rows of the frame
- **WHEN** the user looks at the push of a row
- **THEN** it says "Waits for the first sync", and is offered once the sync committed
  [@test] tests/e2e/startup/warm-start.spec.ts (`says a push of the rows of the frame waits for the first sync, and offers it once the sync committed`)

#### Scenario: The close of a pull request of a stack that is no more
- **GIVEN** the rows of the frame
- **WHEN** the user looks at the close of a pull request of a stack that is no more
- **THEN** it says it waits for the first sync
  [@test] tests/e2e/startup/warm-start.spec.ts (`says the close of a pull request of a stack that is no more waits for the first sync on the rows of the frame`)

### Requirement: Before the first sync: The backend refuses an intent of the remote on the frame

The backend MUST refuse an intent of the remote on the rows of the frame whoever asks it: with `plan_changed`, and
after a first sync that failed (or that git deferred) with the error of that sync, a push that waited for it too (as a
rewrite does).

#### Scenario: An intent of the remote on a seeded table
- **GIVEN** the table of the session is the seed
- **WHEN** an intent of the remote comes, whoever asks it
- **THEN** it is refused with `plan_changed` before it is admitted
  [@test] crates/operations/src/rewrite_test.rs (`a_remote_intent_on_a_seeded_table_is_refused_before_it_is_admitted`)

#### Scenario: A push that waited for a first sync that failed
- **GIVEN** a push waits for the first sync
- **WHEN** that sync fails, or git defers it
- **THEN** the push is refused with the error of that sync and sends nothing
- **AND** an intent of the remote asked after it on the rows of the frame is refused with that error too
  [@test] crates/operations/src/rewrite_test.rs (`a_push_that_waited_for_a_first_sync_that_failed_is_refused_and_sends_nothing`)

### Requirement: Before the first sync: A rewrite pressed on the frame waits for the first sync

A rewrite pressed on the rows of the frame (Unstack) MUST say it waits ("Unstacking oauth after the sync" on the "⋯"
of its row and on its item wherever it shows, the command palette too) and run on the table the sync committed.

#### Scenario: An Unstack pressed on the rows of the frame
- **GIVEN** the rows of the frame
- **WHEN** the user presses Unstack
- **THEN** its row and its item say "Unstacking oauth after the sync"
- **AND** it runs once the first sync committed, on its table
  [@test] tests/e2e/startup/warm-start.spec.ts (`runs an unstack pressed on the rows of the frame once the first sync committed, waiting for it until then`)

#### Scenario: The words of an operation that waits
- **WHEN** an operation waits for the sync
- **THEN** its row and its item wherever it shows name what it waits for
  [@test] src/presenters/operations.test.ts (`says that an operation waits for the sync, behind another one, or what it does`)
  [@test] src/presenters/actions.test.ts (`says on an item that runs as an operation its busy state as its row does, and names the one an item waits for or is busy behind`)

### Requirement: Before the first sync: A focus during the first sync is one check after it

A focus during the first sync MUST be one check after it.

#### Scenario: A focus during the first sync
- **WHEN** the window gains the focus while the first sync runs
- **THEN** the session checks once after it, with no second run
  [@test] tests/e2e/startup/warm-start.spec.ts (`checks a focus during the first sync once after it, with no second run`)

### Requirement: Before the first sync: A first sync committed before the subscription ends as any run

A first sync that committed before the subscription answered MUST end in the window as any run does: the count of
Sync, "Last synced just now", the fixups of its table.

#### Scenario: The first sync commits before the answer
- **WHEN** the first sync committed before the subscription of the window answered
- **THEN** the window ends it as any run: the count of Sync, "Last synced just now", the fixups of its table
  [@test] tests/e2e/startup/warm-start.spec.ts (`handles the end of a first sync that committed before the subscription answered as the end of any run`)

### Requirement: Before the first sync: The first sync ends once, a reload ends no run

The first sync MUST end once: a reload of the window, whose init script hands the page the same seed, catches up from
it again, and the page of the window remembers that it saw a run of that seed end (`sessionStorage`, which lives as
long as the window), so the reload ends no run (no count, no new "Last synced"); a page that reloaded as the table of
that sync went out, and never saw it, ends it after the reload.

#### Scenario: A reload after the end of the first sync
- **GIVEN** the window saw the first sync end
- **WHEN** the window reloads and its init script installs the same seed
- **THEN** the page catches up from it and ends no run: no count, no new "Last synced"
  [@test] tests/e2e/startup/warm-start.spec.ts (`ends the first sync once: a reload of the window after it ends no run`)
  [@test] src/model/store.test.ts (`ends the first sync once: a reload that installs the seed again after its end catches up from it and ends no run`)

#### Scenario: A reload as the table of the first sync went out
- **WHEN** the page reloads as the table of the first sync went out, and never saw it
- **THEN** it catches up from the seed to the table of that run and ends it after the reload
  [@test] src/model/store.test.ts (`ends the first sync in a reload of a window whose page went away as the table of that sync came`)
  [@test] crates/repository-model/src/session_test.rs (`a_page_that_reloads_as_the_first_sync_commits_catches_up_from_the_seed_to_the_table_of_this_run`)

### Requirement: Before the first sync: The frame keeps the landing needs with their tips

The frame MUST keep the landing needs with the tips they are of: a first sync that leaves those tips computes them not
again, and no step of a row waits for them anew ("Checking what … needs to land"); the next steps, the starts and the
actions of the rows change once, when that sync commits.

#### Scenario: A warm first sync that leaves the tips
- **GIVEN** the frame keeps the landing needs of its tips
- **WHEN** the first sync leaves those tips
- **THEN** no step of a row waits for them anew, and every subject changes in one patch at most, when the sync commits
  [@test] crates/repository-model/src/session_test.rs (`a_warm_first_sync_that_leaves_the_tips_of_the_frame_changes_every_subject_in_one_patch_at_most`)

### Requirement: Before the first sync: The end of a sync goes with the view of its robots

The view of the robots a sync derives MUST go in the same patch as the end of that sync when it comes within 16 ms.

#### Scenario: The view of the robots comes in time
- **WHEN** the view of the robots of a sync comes within 16 ms of its end
- **THEN** the end of the sync and that view go in one patch
  [@test] crates/repository-model/src/session_test.rs (`the_end_of_a_sync_goes_with_the_view_of_its_robots_in_one_patch`)

#### Scenario: The view of the robots is late
- **WHEN** the view of the robots does not come
- **THEN** the end of the sync waits for it one interval at most
  [@test] crates/repository-model/src/session_test.rs (`the_end_of_a_sync_waits_for_the_view_of_its_robots_one_interval_at_most`)

### Requirement: Before the first sync: A frame of another prefix or schema seeds nothing

A frame of another prefix (saved before the prefix changed) or schema, a project without one, or a model of another
project (a reload after a switch) MUST start as before, without rows until the first sync.

#### Scenario: A frame of another prefix or schema
- **WHEN** the frame was saved under another prefix than the one of now, or of another schema
- **THEN** the start seeds nothing from it
  [@test] crates/repository-model/src/session_test.rs (`the_start_seeds_nothing_from_a_frame_of_another_prefix_than_the_one_of_now`, `a_frame_of_another_prefix_or_schema_seeds_nothing`)

#### Scenario: A model of another project
- **WHEN** the page reloads after a switch, its model of another project
- **THEN** the page leaves that model and shows no rows until the first sync
  [@test] src/model/store.test.ts (`keeps the seed when the session sent nothing since, and leaves a model of another project`)

#### Scenario: A project without a frame
- **WHEN** the selected project has no frame
- **THEN** the window starts without rows until the first sync

### Requirement: Before the first sync: A sync without rows to show is the loading table

While a sync runs and the page has no rows to show (the first sync of a session of a repository synced before: when
the window opens it at start, or another session of it, under a prefix changed in a terminal or in Settings; a sync
after one that failed or waited for git), and until the session of the repository answers the window, the body MUST be
the loading table where the rows will be, never an empty page under a spinning Sync.

#### Scenario: Another session of a repository synced before
- **GIVEN** a repository synced before, whose prefix was set in a terminal
- **WHEN** the session of the new prefix runs its first sync
- **THEN** the body is the loading table, no row and no error alert
- **AND** the rows replace it once the sync ends
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the loading table in the body during the first sync of another session of a repository synced before`)

#### Scenario: The window before the session answers
- **GIVEN** a repository synced before, the window without the frame of its session
- **WHEN** the page draws its shell before the subscription answered
- **THEN** the shell is the title row and the loading table, which the rows replace
  [@test] tests/e2e/repository/first-frame.spec.ts (`should show the main window on the first rows of its sync, the table loading where they come until then`)

### Requirement: Before the first sync: The loading table says what the sync does

The line of the loading table MUST say what the sync does, as the tooltip of Sync does: "Grouping the commits…",
"Building the branches…", "Checking which branches landed…", and "Syncing branches…" before a step is known.

#### Scenario: The step of the running sync
- **WHEN** a sync runs
- **THEN** the line names its step, "Syncing branches…" before a step is known, and none once it ended
  [@test] src/lib/sync-step.test.ts (`names what a running sync does, and only that it runs before a step is known`)
  [@test] src/model/store.test.ts (`names the step of the running sync, and none once it ended`)

### Requirement: Before the first sync: A failed sync's alert gives way to the loading table

A failed sync's alert MUST give way to the loading table while the next sync runs.

#### Scenario: The sync after a failed one
- **GIVEN** a sync failed and its alert shows, the page has no rows
- **WHEN** the next sync runs
- **THEN** the loading table replaces the alert

### Requirement: Before the first sync: Auto-sync on Focus syncs only when the inputs changed

Auto-sync on Focus (Settings → General, File → Auto-sync on Focus) MUST be on by default and sync on a focus only when
what a sync reads changed since the last sync of the session (the inputs of
[Git Operations](../../../../docs/design/git-operations.md#sync-inputs-check), read when the sync started, so a commit
made while it ran counts).

#### Scenario: What changes the inputs
- **WHEN** a commit, an amend or a finished rebase in the IDE, a fetch (of the baseline, also an upstream other than
  `master` or `main`) or a push elsewhere, a regroup or a description by `brd`, or a changed prefix (the inputs are of
  the prefix Rust resolves) came since the last sync
- **THEN** the inputs changed, and a focus syncs
- **AND** what a sync writes changes none of them
  [@test] crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`, `changes_with_the_upstream_of_the_checked_out_branch`, `changes_with_the_prefix_alone`)

#### Scenario: A focus runs only when the inputs changed
- **WHEN** the window gains the focus
- **THEN** the session runs a sync only when the inputs changed
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_runs_only_when_the_inputs_changed`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs on focus only when what a sync reads changed, and says when it checked`)

### Requirement: Before the first sync: A focus syncs nothing before the first sync

A focus of the window and a stamp of `brd` MUST run nothing for a session that does not sync of itself: its repository
was never synced, by the app or by `brd` (the mark of a sync in the worktree), and the user asked no Sync and no
rewrite of it in the session. The first sync of a repository never synced is the user's, Auto-sync on Focus on or off.

#### Scenario: A focus before the first sync
- **GIVEN** a repository never synced, the Ready to Sync card shown, Auto-sync on Focus on
- **WHEN** the window gains the focus
- **THEN** the session runs nothing and answers that it skipped the focus, the card stays and no sync ran
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_or_a_stamp_of_a_session_that_does_not_sync_of_itself_runs_nothing`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_and_a_stamp_sync_a_worktree_never_synced_only_once_brd_synced_it`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs nothing on a focus before the first sync, which is the user's: the Ready to Sync card stays`)

#### Scenario: A stamp of brd before the first sync
- **GIVEN** a repository never synced, the Ready to Sync card shown
- **WHEN** a mutation of `brd` that is no sync of the worktree (an agent's `brd reword`, a `brd lease get` beside it)
  touched the stamp of the repository
- **THEN** the session runs nothing and answers that it skipped the stamp, the card stays and no sync ran
- **AND** after the user's Sync a stamp syncs what a mutation of `brd` changed
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_or_a_stamp_of_a_session_that_does_not_sync_of_itself_runs_nothing`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs nothing on a stamp of brd before the first sync, as on a focus, and what a mutation of brd changed once the user synced`)

#### Scenario: A focus after the user's first Sync
- **GIVEN** the user synced the repository with the Sync of the Ready to Sync card
- **WHEN** a commit came in a terminal, and the window gains the focus
- **THEN** the session syncs it
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_or_a_stamp_of_a_session_that_does_not_sync_of_itself_runs_nothing`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_after_a_failed_sync_reads_only_the_fingerprint`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs on a focus after the user's first Sync once what a sync reads changed`)

#### Scenario: A focus after the user's rewrite
- **GIVEN** the user rewrote branches of a repository never synced, and the sync after the rewrite failed
- **WHEN** the window gains the focus with nothing changed
- **THEN** the session checks what a sync reads, as after the user's Sync, and answers the error of that sync
  [@test] crates/repository-model/src/session_test.rs (`a_focus_after_a_rewrite_whose_sync_failed_checks_the_inputs`)

#### Scenario: brd synced the repository
- **GIVEN** a window shows a repository the app never synced
- **WHEN** `brd sync` in a terminal built its branches and touched the stamp of the repository
- **THEN** the session syncs, as the sync of `brd` marked the worktree synced under its prefix
  [@test] crates/sync-core/src/synced_mark_test.rs (`a_sync_marks_its_worktree_synced_and_one_git_deferred_does_not`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_stamp_after_brd_synced_a_worktree_the_app_never_synced_runs`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_and_a_stamp_sync_a_worktree_never_synced_only_once_brd_synced_it`)
  [@test] crates/operations/src/registry_watch_test.rs (`a_stamp_of_bd_syncs_the_session_a_window_shows`)

### Requirement: Before the first sync: The mark of a sync names its prefix

The mark of a sync in the worktree MUST name the prefix of each sync that wrote it (`branch-deck/synced.d/`, a file per
prefix, so two syncs at once both land), and a session MUST be synced before by the mark only when it names the
session's prefix: `brd sync --prefix X` marks the session of `X` alone. The `lastSyncTime` of the app is of the path.

#### Scenario: A prefix changed after a sync
- **GIVEN** a repository the app synced under one prefix
- **WHEN** its prefix is another one, never synced
- **THEN** the session of the new prefix syncs as it opens, as `lastSyncTime` is of the path
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs the session of a prefix set in a terminal once the focus finds it`)

#### Scenario: brd synced under another prefix
- **GIVEN** a worktree only `brd sync --prefix other` synced
- **WHEN** a window opens it under the prefix `test`
- **THEN** the session of `test` does not sync of itself: the mark names `other` alone
- **AND** a sync under `test` adds its own, so both sessions sync of themselves
  [@test] crates/sync-core/src/synced_mark_test.rs (`the_mark_names_the_prefix_each_sync_built_under`)

#### Scenario: Two syncs under two prefixes at once
- **WHEN** syncs under several prefixes mark the worktree at once
- **THEN** every prefix is in the mark
  [@test] crates/sync-core/src/synced_mark_test.rs (`two_marks_under_two_prefixes_at_once_both_land`)

#### Scenario: The mark of a build before
- **GIVEN** a build before left one file of lines, a time among them
- **THEN** its prefixes count and the time names none
- **AND** the next mark moves its prefixes into files of their own and removes it
  [@test] crates/sync-core/src/synced_mark_test.rs (`the_mark_of_a_build_before_is_read_and_moved_by_the_next_mark`)

### Requirement: Before the first sync: A focus with unchanged inputs reads them once and syncs nothing

Otherwise the focus MUST read the inputs once (two git processes) and sync nothing, and the header says "Checked just
now" instead of "Last synced 18 minutes ago", the age of the sync in its tooltip: the branches are as current as a sync
would make them.

#### Scenario: A focus with nothing changed
- **WHEN** the window gains the focus and the inputs are as the last sync read them
- **THEN** the session reads only the fingerprint of the inputs and runs no sync
- **AND** the header says "Checked just now" instead of "Last synced 18 minutes ago", the age of the sync in its tooltip
  [@test] crates/repository-model/src/session_test.rs (`a_focus_with_unchanged_inputs_reads_only_the_fingerprint`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs on focus only when what a sync reads changed, and says when it checked`)

### Requirement: Before the first sync: The header says Checked only where the check and the sync read apart

The header MUST say "Checked just now" only where the two read apart: for a check that came when "Last synced" no
longer read "just now", a minute or more after the sync (`readsLater`). A check in the minute of the sync MUST leave
"Last synced just now", which says as much.

#### Scenario: The check of the start
- **WHEN** a check comes in the minute of the sync (the one after the sync of the start, which a focus during it asks,
  100 ms later)
- **THEN** the header leaves "Last synced just now"
  [@test] src/lib/time.test.ts (`says a check in the minute of the sync nothing "just now" does not: the check of the start, 100 ms after its sync`)

### Requirement: Before the first sync: The header keeps its answer while the check and the sync age

The header MUST keep its answer of a check while both the check and the sync age; compared as they read now they would
take turns at every edge of their buckets ("1 hour ago" and "2 hours ago").

#### Scenario: Both age
- **GIVEN** the header said "Last synced just now" for a check in the minute of the sync
- **WHEN** both age, where their texts read the same now and apart a bucket later
- **THEN** the header keeps that answer
  [@test] src/lib/time.test.ts (`keeps its answer while both age, where their texts read the same now and apart a bucket later`)

### Requirement: Before the first sync: A focus during a sync checks once after it

A focus during a sync MUST check once after it, however many came.

#### Scenario: Several focuses during a run
- **WHEN** the window gains the focus, maybe several times, while a sync runs
- **THEN** the session checks the inputs once after the chain
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_during_a_run_checks_the_inputs_once_after_the_chain`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_during_a_run_is_one_check_after_it`)
  [@test] tests/e2e/startup/warm-start.spec.ts (`checks a focus during the first sync once after it, with no second run`)

### Requirement: Before the first sync: Auto-sync on Focus turned off stays off

A user who turned Auto-sync on Focus off MUST keep it off: a focus then starts no git process.

#### Scenario: The setting is off
- **GIVEN** the user turned Auto-sync on Focus off
- **WHEN** the window gains the focus after a change elsewhere
- **THEN** the focus starts no git process and syncs nothing
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`tells the session the focus, which syncs nothing once Auto-sync on Focus is off`)

### Requirement: Before the first sync: A focus does not repeat a failed sync while nothing changed

A sync that failed, or that git deferred, MUST NOT be repeated by a focus while what a sync reads is as that sync read
it, with which of the files git keeps during an operation exist (`rebase-merge`, `rebase-apply`, `MERGE_HEAD`,
`CHERRY_PICK_HEAD`, `REVERT_HEAD`, `sequencer`): the focus reads the inputs once and answers the error of that sync, the
alert stays and no run starts, so a sync of a large repository that fails does not run again on every return to the
window.

#### Scenario: A focus after a failed sync with the same inputs
- **GIVEN** a sync failed
- **WHEN** a focus finds the inputs as that sync read them
- **THEN** it reads only the fingerprint, answers the error of that sync, and no run starts
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_after_a_failed_run_with_the_same_inputs_answers_its_error_without_a_run`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_after_a_failed_sync_reads_only_the_fingerprint`)

#### Scenario: A failed sync whose inputs git could not read
- **GIVEN** a sync failed after git could not read what a sync reads
- **WHEN** a focus cannot read them either
- **THEN** it answers the error of that sync and no run starts; a focus that reads them runs
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_after_a_failed_run_without_inputs_answers_its_error_while_git_cannot_read_them`)

#### Scenario: The alert stays until Sync
- **GIVEN** a sync failed and its alert shows
- **WHEN** the window gains the focus with nothing changed
- **THEN** the alert stays and no sync runs, until the user presses Sync
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`does not sync again on a focus after a failed sync while nothing changed, and syncs again on Sync`)

#### Scenario: A run the change of the prefix ended
- **GIVEN** a run ended with `prefix_changed`, which is no failure
- **WHEN** a focus finds the inputs as they were
- **THEN** it answers nothing changed, not that error
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_prefix_change_is_not_remembered`)

### Requirement: Before the first sync: A change after a failed sync syncs on the focus

After a failed or deferred sync a change MUST sync on the focus: a commit, the end of a rebase (also an abort, which
moves no ref), a branch checked out again after a detached HEAD.

#### Scenario: The inputs changed after a failed run
- **GIVEN** a sync failed
- **WHEN** a focus finds the inputs changed
- **THEN** it runs a sync
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_after_a_failed_run_runs_when_the_inputs_changed`)

#### Scenario: A git operation starts or ends
- **WHEN** a git operation starts or ends, also an abort of a rebase that moves no ref
- **THEN** the inputs change
  [@test] crates/sync-core/src/sync_inputs_test.rs (`changes_when_a_git_operation_starts_or_ends`)

### Requirement: Before the first sync: Sync and the open sync anyway after a failed sync

Sync (⌘R) and the window opening the repository MUST sync anyway after a failed sync; ⌘R during a run asks for the
check after it, which a run that failed with nothing changed answers with its error.

#### Scenario: The open after a failed run
- **GIVEN** a sync failed
- **WHEN** the window opens the repository, also while a check of a focus runs
- **THEN** it syncs again, joining that check
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`an_open_after_a_failed_run_runs_again_also_joining_a_focus_check`)

#### Scenario: A request during a run that fails
- **WHEN** a run fails while requests wait for it
- **THEN** they are answered with its error
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_failed_run_settles_its_requests_with_the_error`)

#### Scenario: Sync after a failed sync
- **GIVEN** a sync failed and nothing changed
- **WHEN** the user presses Sync
- **THEN** it syncs again
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`does not sync again on a focus after a failed sync while nothing changed, and syncs again on Sync`)

### Requirement: Before the first sync: The app's own push is no change for the focus

The app's own push MUST be no change for the focus: it moves the remote branches it sent, which a sync reads, but the
table has its outcome already, and the session puts those remote branches into the inputs it knows, so a focus after a
Push syncs nothing ("Checked just now" a minute after the sync).

#### Scenario: A focus after the session's own push
- **GIVEN** the app pushed a branch
- **WHEN** the window gains the focus
- **THEN** the session syncs nothing, the remote branch it moved known to it
- **AND** the header says "Checked just now" a minute after the sync
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_focus_after_the_sessions_own_push_runs_nothing`)
  [@test] crates/repository-model/src/session_test.rs (`a_focus_after_the_sessions_own_push_runs_no_sync`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs nothing on the focus after its own push, the remote branch it moved known to the session`)

### Requirement: Before the first sync: A push during a run counts, a move elsewhere syncs

A push that ends while a sync or the check of a focus runs MUST count too; a remote branch moved by another clone or a
fetch elsewhere MUST still sync.

#### Scenario: A push that ends during a run or a check
- **WHEN** the app's push ends while a sync or the check of a focus runs
- **THEN** the remote branch it moved is no change for the next focus

#### Scenario: A remote branch moved elsewhere
- **WHEN** another clone or a fetch elsewhere moved a remote branch
- **THEN** the next focus syncs

### Requirement: Before the first sync: A run under a prefix the config left builds nothing

A prefix changed in a terminal (`git config branchdeck.branchPrefix`) MUST show on the next focus of a session that
syncs of itself, and of a repository never synced on its first Sync: the check reads the prefix among the inputs of a
sync, and the run of the session under the old prefix builds nothing under it: it answers `prefix_changed`, which is
no failure.

#### Scenario: The run of the session under the old prefix
- **GIVEN** the prefix was changed in a terminal
- **WHEN** the session of the old prefix runs
- **THEN** it builds nothing under the old prefix and answers `prefix_changed`
  [@test] crates/repository-model/src/session_test.rs (`a_run_under_a_prefix_the_config_left_builds_nothing`)
  [@test] crates/test-server/src/session_routes_test.rs (`a_prefix_set_in_a_terminal_refuses_the_run_of_the_session_under_the_old_one`)

### Requirement: Before the first sync: A changed prefix subscribes the window again

On `prefix_changed` the window MUST subscribe again (`resubscribeSession`, holding the view it shows until the session
of the new prefix answers its own), so the table is never the empty one of a session no sync built; that session syncs
as it opens for a repository synced before.

#### Scenario: The focus finds a prefix set in a terminal
- **GIVEN** a repository synced before, whose prefix was set in a terminal
- **WHEN** the window gains the focus
- **THEN** the window subscribes again, quietly, holding the view it shows
- **AND** the session of the new prefix syncs as it opens, and its rows replace the old ones
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs the session of a prefix set in a terminal once the focus finds it`)
  [@test] src/model/store.test.ts (`subscribes again when the git config names another prefix than the session's, quietly`)

### Requirement: Before the first sync: With Auto-sync on Focus off the next sync finds the prefix

With Auto-sync on Focus off a focus MUST start no git process, and the next sync finds the changed prefix the same way.

#### Scenario: The setting is off and the prefix changed
- **GIVEN** Auto-sync on Focus is off and the prefix was changed in a terminal
- **WHEN** the window gains the focus, then the next sync runs
- **THEN** the focus starts no git process
- **AND** the next sync finds the changed prefix the same way as the check of a focus does

### Requirement: Before the first sync: The user's Sync under a changed prefix is asked again

A Sync the user pressed that comes back so (`prefix_changed`) MUST be asked again once the window holds the session of
the new prefix, Sync busy throughout, when that session does not sync of itself (its view's `synced`: the Ready to
Sync card, while the window kept the focus): that session serves it, as no sync of its open does.

#### Scenario: Sync of the Ready to Sync card under a prefix set in a terminal
- **GIVEN** a repository never synced, its prefix set in a terminal while the window kept the focus
- **WHEN** the user presses Sync of the Ready to Sync card
- **THEN** the session of the old prefix refuses the run, and the sync is asked again of the session of the new one
- **AND** Sync is busy throughout, and the branches are built under the new prefix only
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs under a prefix set in a terminal behind the window's back when Sync of the Ready to Sync card is pressed`)
  [@test] src/model/store.test.ts (`asks the user's own sync of a repository never synced again once the window holds the session of the new prefix, Sync busy throughout`)

### Requirement: Before the first sync: A prefix written in Settings subscribes the window again

A prefix written in Settings → Branches MUST subscribe the window again the same way (`repository_config_changed`).

#### Scenario: The prefix of the repository written in Settings
- **GIVEN** a repository synced before
- **WHEN** the user writes the prefix of the repository in Settings → Branches
- **THEN** the window subscribes again, and the session of that prefix opens and syncs
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should open and sync the session of a prefix written here, for a repository synced before`)

### Requirement: Before the first sync: The main window tells its session the focus

The main window MUST tell the session of its repository when it gains and loses the focus (`set_window_focus`, the
last state of a quick succession, 100 ms); the session decides whether a gained one syncs, by the setting as Rust's
writer has it (`SessionHost::syncs_on_focus`), and the page checks nothing.

#### Scenario: The window gains and loses the focus
- **WHEN** the window gains or loses the focus, maybe several times in a quick succession
- **THEN** it tells the session the last state with `set_window_focus`, and checks nothing itself
  [@test] src/model/store.test.ts (`tells the session the focus of the window, and marks the check of a gained one that found nothing changed`)

#### Scenario: Auto-sync on Focus is off
- **GIVEN** the user turned Auto-sync on Focus off, and Rust's writer has it
- **WHEN** the window gains the focus after a change elsewhere
- **THEN** the window still tells its session, which syncs nothing; Sync syncs the change
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`tells the session the focus, which syncs nothing once Auto-sync on Focus is off`)

### Requirement: Before the first sync: A focus of a session no window shows syncs nothing

A focus of a session no window shows MUST sync nothing and start no git process.

#### Scenario: A session without subscribers
- **GIVEN** a session whose window unsubscribed
- **WHEN** a focus comes for it after a commit
- **THEN** it answers skipped and starts no git process
  [@test] crates/repository-model/src/session_test.rs (`a_session_without_subscribers_runs_nothing_and_goes_away`)

### Requirement: Before the first sync: The answer of the check of a focus is recorded as a sync's

The answer of the check of a focus MUST be recorded as a sync's: `unchanged` says "Checked just now", a run it started
counts as the sync of the repository.

#### Scenario: The check found nothing changed
- **WHEN** the check of a gained focus answers `unchanged`
- **THEN** the window marks it, and the header says "Checked just now"
  [@test] src/model/store.test.ts (`tells the session the focus of the window, and marks the check of a gained one that found nothing changed`)

#### Scenario: The check started a run
- **WHEN** the check of a focus starts a run
- **THEN** the window records it as the sync of the repository, Sync busy from its answer to the run's last patch
  [@test] src/model/store.test.ts (`records the sync a focus started, Sync busy from its answer to the run's last patch`)

### Requirement: Before the first sync: ⌘R during a sync asks for the check of a focus after it

⌘R during a sync MUST ask the session for the same check once after it (`sync` for a `focus`): a commit elsewhere
during a long sync is synced, not lost.

#### Scenario: ⌘R while a sync runs
- **GIVEN** a sync runs
- **WHEN** the user presses ⌘R
- **THEN** no second sync starts, only a check of the session after it, which syncs nothing when nothing changed
  [@test] tests/e2e/repository/header.spec.ts (`should not move while a sync runs`)

### Requirement: Before the first sync: The native menu item is checked as the setting is

File → Auto-sync on Focus of the native menu MUST be checked as the setting is: at startup as the backend's writer
loaded `settings.json` (`AppSettings::auto_sync_on_focus`, on by default), then by the main window for every value of
the setting, whichever window changed it (`set_auto_sync_menu_checked`).

#### Scenario: At startup
- **WHEN** the app starts with a stored setting, or without one
- **THEN** the item is checked as the stored setting is, and checked without one
  [@test] src-tauri/src/menu_test.rs (`auto_sync_on_focus_is_checked_as_the_stored_setting`, `without_a_stored_setting_auto_sync_on_focus_is_checked`)

#### Scenario: A change of the setting in another window
- **WHEN** the setting changes, in whichever window
- **THEN** the main window checks the item as the setting is now
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)

### Requirement: Before the first sync: The menu item and the switch of Settings are one setting

Checking the item MUST set the setting (`auto_sync_on_focus_toggled`, sent to the main window). The switch of Settings
→ General is the same setting: its change MUST reach the main window, which checks the item.

#### Scenario: The item and the switch
- **WHEN** the user unchecks the item
- **THEN** the setting follows, and the switch of Settings → General is off
- **AND** the switch turned on and off again checks and unchecks the item
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)

### Requirement: Before the first sync: A sync during a git operation or with a detached HEAD is deferred

A sync while git is in the middle of a rebase, merge, cherry-pick or revert, or with HEAD detached, MUST be refused
before it changes anything (`SyncDeferred`,
[Git Operations](../../../../docs/design/git-operations.md#virtual-branch-sync)): a half-done history would archive
the branches it lacks.

#### Scenario: A rebase in progress or a detached HEAD
- **WHEN** a sync runs while a rebase is in progress or HEAD is detached
- **THEN** it waits, refused before it changes anything, in the app as in `brd`
  [@test] crates/sync-core/src/sync_test.rs (`sync_waits_while_a_rebase_is_in_progress_or_head_is_detached`)
  [@test] crates/bd-cli/src/sync_test.rs (`waits_while_a_rebase_is_in_progress`)

### Requirement: Before the first sync: A deferred sync is no failure

A deferred sync MUST be no failure: no error alert, no red Sync, the branches as the last sync left them.

#### Scenario: The window of a deferred sync
- **WHEN** a sync is deferred
- **THEN** no error alert shows, Sync is not red, and the branches stay as the last sync left them
  [@test] src/model/store.test.ts (`is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says in an info toast that a sync waits for git, and leaves the branches as they were`)
  [@test] crates/repository-model/src/session_test.rs (`a_failed_sync_keeps_the_table`)

### Requirement: Before the first sync: A deferred sync the user asked for says so in a toast

A sync the user asked for (Sync, ⌘R, the Ready to Sync card) MUST say so in an `info` toast, "Not synced: a rebase is
in progress" / "The branches sync once git finished it" ("Not synced: HEAD is detached" / "Check out a branch to sync
its commits"); a sync the app started (on open, on focus, after a change) MUST wait without a toast.

#### Scenario: A sync the user asked for
- **WHEN** the user's sync is deferred by a rebase in progress
- **THEN** an `info` toast says "Not synced: a rebase is in progress" / "The branches sync once git finished it"
  [@test] src/model/store.test.ts (`is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says in an info toast that a sync waits for git, and leaves the branches as they were`)

#### Scenario: A detached HEAD
- **WHEN** the user's sync is deferred by a detached HEAD
- **THEN** the toast says "Not synced: HEAD is detached" / "Check out a branch to sync its commits", and the view of the
  session the same
  [@test] src/model/store.test.ts (`is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for`)

#### Scenario: A sync the app started
- **WHEN** the sync of the open, of a focus or after a change is deferred
- **THEN** it waits without a toast
  [@test] src/model/store.test.ts (`is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for`)

### Requirement: Before the first sync: The view says why the sync waits

The view of the session MUST say why it waits (`Deferred`) until the next sync starts: a page without rows (a
repository synced before, opened during a rebase) shows it as an `info` alert in the body, in the words of the toast,
and the focus after git finished syncs.

#### Scenario: The state of the view
- **WHEN** a sync is deferred
- **THEN** the view of the session is `Deferred`, the table kept, until the next sync
  [@test] crates/repository-model/src/session_test.rs (`a_failed_sync_keeps_the_table`)

#### Scenario: A page without rows
- **GIVEN** a repository synced before, opened during a rebase
- **WHEN** its sync is deferred
- **THEN** the body shows an `info` alert in the words of the toast
- **AND** the focus after git finished syncs
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says in the body that a sync waits for git while there is no table, and syncs once git finished`)

### Requirement: Before the first sync: A sync on a checked-out branch under the prefix is deferred

A sync while a branch under the prefix is checked out (`git switch jane/virtual/auth` by hand: a virtual, unapplied,
archived or stack branch) MUST be refused the same way, before any event, and move no ref under the prefix
(`managed_branch`): its history holds the commits of one group, and the sync would archive every other branch.

#### Scenario: A virtual branch checked out
- **WHEN** a sync runs while a branch under the prefix is checked out
- **THEN** it is refused before any event and moves no ref under the prefix
  [@test] crates/sync-core/src/sync_test.rs (`sync_waits_while_a_branch_under_the_prefix_is_checked_out`)

#### Scenario: The branches under the prefix
- **WHEN** the checked-out branch is under a namespace of the prefix, or not
- **THEN** it is managed, or it is not
  [@test] crates/domain-types/src/branch_names_test.rs (`a_branch_under_a_namespace_of_the_prefix_is_managed_and_any_other_is_not`)

### Requirement: Before the first sync: A managed branch checked out is named in the toast and the alert

The toast of a sync the user asked for and the alert MUST say "Not synced: jane/virtual/auth is a branch Branch Deck
manages" / "Check out your main branch to sync its commits"; checking out the baseline's branch syncs again as before.

#### Scenario: The words of a managed branch
- **GIVEN** `jane/virtual/auth` is checked out
- **WHEN** the user's sync is refused
- **THEN** an info toast says "Not synced: jane/virtual/auth is a branch Branch Deck manages" / "Check out your main
  branch to sync its commits"
  [@test] src/model/store.test.ts (`is no failure on a checked out branch under the prefix: an info toast that names the branch and says to check out the main branch`)

#### Scenario: The baseline's branch checked out again
- **WHEN** the user checks out the baseline's branch
- **THEN** the sync runs again as before

### Requirement: Before the first sync: A switch forgets the branches of the previous repository

A switch to another repository MUST forget the branches of the previous one at once, also while its sync runs: the
window ends its subscription to the session of that repository and subscribes to the session of the next, so a late
patch of the old one never shows under the name of another repository, and Sync of the next is not busy for it.

#### Scenario: A switch to another repository
- **WHEN** the user switches to another repository
- **THEN** the branches of the previous one are gone at once
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`forgets the branches of the previous repository on a switch`)

#### Scenario: A late patch of the session left
- **WHEN** a patch of the session the window left comes after the switch
- **THEN** it reaches nothing
  [@test] src/model/store.test.ts (`forgets the table of the session it leaves: a late patch of it reaches nothing`)

#### Scenario: A sync of the previous repository still runs
- **GIVEN** a sync of the previous repository runs
- **WHEN** the user switches
- **THEN** Sync of the next one is not busy for it, and nothing of it is recorded
  [@test] src/model/store.test.ts (`leaves a request of the session the window left to it: Sync of the next one is not busy, nothing of it is recorded`)

### Requirement: Before the first sync: What the app keeps of a slot of a lease pool is of its lease

What the app keeps of a worktree by its path MUST be of the epoch it was recorded under (`worktree_epoch`: the lease a
slot of a lease pool holds now), and a record of another known epoch MUST count as absent: the entry of
`repositoryState`, which the writer forgets, the frame of the session, which seeds nothing, and a session kept idle,
which a window that comes back does not join. An epoch the files cannot tell MUST decide nothing.

#### Scenario: The epoch of a slot
- **WHEN** a slot is leased, returned and leased again
- **THEN** its epoch is the lease it holds, another after each return and get; the main checkout has none
  [@test] crates/bd-lease/src/lease_test.rs (`the_epoch_of_a_slot_is_its_lease_and_changes_with_each_return_and_get`)

#### Scenario: The state of a slot of an earlier lease
- **GIVEN** the app synced a slot under one lease
- **WHEN** the slot holds another lease
- **THEN** the writer forgets the entry of the slot before the key leaves it, and the slot was not synced before
- **AND** an entry of a path outside a pool and one of the lease now stay
  [@test] crates/app-settings/src/projects_test.rs (`an_entry_of_another_epoch_of_its_worktree_is_forgotten_before_the_key_leaves_the_writer`)

#### Scenario: The frame and the kept session of an earlier lease
- **GIVEN** a frame or a session kept idle of a slot under one lease
- **WHEN** a window opens the slot under another lease
- **THEN** the frame seeds nothing and the window gets a new session, without the rows of the lease before
  [@test] crates/repository-model/src/session_test.rs (`a_frame_of_an_earlier_lease_of_the_slot_seeds_nothing`, `a_window_that_comes_back_to_a_session_kept_under_an_earlier_lease_opens_a_new_one`)

#### Scenario: The window opens a slot of a new lease
- **GIVEN** the app synced a slot an hour ago, and the slot went back and an agent leased it again
- **WHEN** the window opens it
- **THEN** the Ready to Sync card shows, no sync runs, and the header says no "Last synced"
- **AND** a slot the app synced under the lease it holds syncs as the window opens it, "Last synced" in the header
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the Ready to Sync card for a slot of the lease pool the app synced under an earlier lease, and no Last synced`, `syncs a slot of the lease pool the app synced under its lease now when the window opens it, and says when`)

#### Scenario: An epoch the files cannot tell
- **GIVEN** the app recorded a sync of a slot under its lease
- **WHEN** the state of the pool cannot be read or parsed
- **THEN** the entry stays, in memory and on disk, and a write meanwhile keeps the epoch it names
- **AND** the frame seeds nothing then and is saved under the epoch it names, and a session kept idle stays
  [@test] crates/app-settings/src/projects_test.rs (`an_entry_whose_epoch_cannot_be_read_stays_as_it_is_and_on_disk`)
  [@test] crates/repository-model/src/session_test.rs (`a_frame_keeps_its_epoch_while_the_epoch_of_the_worktree_cannot_be_read`, `a_kept_session_stays_while_the_epoch_of_the_worktree_cannot_be_read`)

#### Scenario: A write while the epoch is read
- **GIVEN** the writer reads the epoch of an entry outside its mutex
- **WHEN** a write of the lease now changes the entry before the writer holds the mutex again
- **THEN** that entry stays: only an entry that still names the epoch it named when the read began is forgotten
  [@test] crates/app-settings/src/projects_test.rs (`an_entry_written_while_its_epoch_was_read_stays`)

#### Scenario: The collapsed stacks of a slot leased again
- **GIVEN** stacks were collapsed in the table of a slot under its lease
- **WHEN** the slot holds another lease
- **THEN** the writer forgets those roots, nothing else of the project, and keeps them under an epoch the files could
  not tell
  [@test] crates/app-settings/src/projects_test.rs (`the_collapsed_stacks_of_a_slot_are_forgotten_under_another_lease_and_kept_under_an_unknown_one`)

### Requirement: Before the first sync: A window on a slot leased again shows the new lease

A session a window shows while its slot of a lease pool is leased again MUST open anew for the new lease, in a new
epoch of its view, and the window MUST show nothing of the rows of the lease before, also of a branch of the same name.

#### Scenario: A window on a slot leased again
- **GIVEN** a window shows a slot of a lease pool the app synced under its lease
- **WHEN** the slot goes back and an agent leases it again, and `brd` touches the stamp
- **THEN** the session opens anew for the new lease by the code of an open, in a new epoch of the view: its table, its
  facts, its sync scheduler and the state of its frames are what an open makes, and the answer of a read of the remote
  services of the lease before applies to it no more
- **AND** each window gets one patch into the new epoch, which applies to no view, and takes the snapshot of the
  session as the view of another session, with no subscription again: the rows and "Last synced" of the lease before
  go, and the Ready to Sync card shows
- **AND** an epoch the files could not tell changes nothing
  [@test] crates/repository-model/src/session_test.rs (`a_stamp_under_another_lease_opens_the_session_anew_for_that_lease`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the Ready to Sync card in the window of a slot once an agent leased the slot again, without the rows and Last synced of the lease before`)

#### Scenario: The rows of a slot leased again under the window
- **GIVEN** the user expanded, selected and focused a row of a slot, or opened its preview
- **WHEN** the window takes the view of the session of a new lease of the slot, whose rows have a branch of the same name
- **THEN** no row is expanded, selected or focused, and the preview, the inline form, the card of a commit and the path
  of a stack of the rows before are gone, as on a switch: a branch of the same name of another lease is another branch
  [@test] src/model/store.test.ts (`takes the view of a session opened anew by its snapshot, from the patch into its epoch, and forgets the rows it held: a branch of the same name is not selected`)
  [@test] src/stores/branch-preview.test.ts (`closes when the window takes the view of another session of the repository: a branch of the same name there is another branch`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`shows the rows of the new lease of a slot an agent leased again under the window, none of them selected, expanded or focused as the row of the same name of the lease before was`)

### Requirement: Repository problems: Browse opens the folder dialog of the system

Browse MUST open the folder dialog of the system (`browse_repository` of app-api, `DialogHost`): the folder picked is
checked as a repository, and the dialog canceled changes nothing.

#### Scenario: A valid repository picked
- **WHEN** the user presses Browse and picks the folder of a repository
- **THEN** the folder is checked as a repository and selected
  [@test] tests/e2e/repository/browse.spec.ts (`should successfully select valid git repository`)

#### Scenario: The dialog canceled
- **WHEN** the user cancels the dialog
- **THEN** nothing changes

### Requirement: Repository problems: In an E2E test the dialog picks the folder of the fixture

In an E2E test the dialog of the test server MUST pick the folder of its fixture (`setDialogPick`,
`POST /repositories/{id}/dialog-pick`), else the test repository itself.

#### Scenario: A test sets the folder to pick
- **GIVEN** a test set the folder of its fixture with `setDialogPick`
- **WHEN** the user presses Browse
- **THEN** the dialog picks that folder
  [@test] tests/e2e/welcome-card/welcome-card.spec.ts (`keeps Step 2 visible after selecting a repository without branch prefix`)

#### Scenario: No folder set
- **WHEN** no test set a folder and the user presses Browse
- **THEN** the dialog picks the test repository itself
  [@test] tests/e2e/repository/browse.spec.ts (`should successfully select valid git repository`)

### Requirement: Repository problems: A folder that is no repository or a bare one is refused

A folder that is not a repository MUST be refused with "Not a Git repository"; a bare repository with "Bare
repositories are not supported: Branch Deck needs a working tree"; a worktree (`.git` file) MUST be accepted as a
repository.

#### Scenario: Not a repository
- **WHEN** the folder is not a repository
- **THEN** it is refused with "Not a Git repository"
  [@test] crates/sync-core/src/repository_validation.rs (`test_validate_path_not_git_repo`)

#### Scenario: A bare repository
- **WHEN** the folder is a bare repository
- **THEN** it is refused with "Bare repositories are not supported: Branch Deck needs a working tree"
  [@test] crates/sync-core/src/repository_validation.rs (`test_validate_path_bare_repo`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`refuses a bare repository with the reason`)

#### Scenario: A linked worktree
- **WHEN** the folder is a worktree, its `.git` a file
- **THEN** it is a repository
  [@test] crates/sync-core/src/repository_validation.rs (`test_validate_path_linked_worktree`)

### Requirement: Repository problems: A repository without commits says so

A repository without commits MUST fail the sync with "The repository has no commits yet: commit to the main branch,
then sync", not with "No baseline branch found"; `brd` says the same as `no_baseline` (see
[Agent CLI](../../cli/agent-cli/spec.md)).

#### Scenario: No commits yet
- **WHEN** a sync runs in a repository without commits
- **THEN** it fails with "The repository has no commits yet: commit to the main branch, then sync"
  [@test] crates/history-rewrite/src/baseline_test.rs (`a_repository_without_commits_says_so`)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says that a repository without commits has nothing to sync yet`)

### Requirement: Repository problems: A repository without a baseline names each branch once

A repository without a baseline MUST name each branch it looked for once ("Cannot find the baseline branch: no
origin/master or origin/main, and no upstream").

#### Scenario: No local or remote baseline
- **WHEN** the repository has no local, or no remote, baseline
- **THEN** the error names each branch it looked for once
  [@test] crates/history-rewrite/src/baseline_test.rs (`no_local_baseline_names_each_branch_once`, `no_remote_baseline_names_each_branch_once`)

### Requirement: Repository problems: A saved repository that is gone is the problem of the path

A saved repository that no longer exists MUST fail the subscription of its session with the error of git ("Repository
not accessible: …"), which the page shows as the problem of the path: the error alert, the selector marked invalid,
Sync disabled with it as the reason, until another one is selected, or a focus subscribes again and finds it back.

#### Scenario: The directory is gone
- **WHEN** the directory of a saved repository is gone
- **THEN** it is not accessible
  [@test] crates/sync-core/src/branch_prefix_test.rs (`a_directory_that_is_gone_is_not_accessible`)

#### Scenario: The page shows the problem of the path
- **GIVEN** the saved repository path does not exist
- **WHEN** the window opens it
- **THEN** the error alert shows, the selector marked invalid, Sync disabled with it as the reason
- **AND** the error clears once a valid repository is selected
  [@test] tests/e2e/repository/saved-path-validation.spec.ts (`should display error for non-existent repository path`, `should clear error when valid repository is selected`)

#### Scenario: A focus after the failed subscription
- **GIVEN** the subscription failed
- **WHEN** the window gains the focus
- **THEN** it subscribes again, and finds the repository back once the user fixed it
  [@test] src/model/store.test.ts (`subscribes again on a gained focus after its subscription failed: the user may have fixed it meanwhile`)

### Requirement: Grouping: The unassigned commits card explains the grouping while there is no branch

While there is no branch, the unassigned commits card MUST say under its title: "Start a commit subject with `(name)`
or an issue key like `ABC-123` to group it into a branch, or select commits and move them to one."

#### Scenario: Unassigned commits and no branch
- **GIVEN** a sync left only unassigned commits, no branch
- **WHEN** the unassigned commits card shows
- **THEN** it says under its title how to group a commit into a branch
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`explains the grouping in the unassigned commits while there is no branch`)

### Requirement: Grouping: No active branches names the baseline the sync compared with

With no branch and no unassigned commit after a sync, "No active branches" MUST name the baseline the sync compared
with, "No commits ahead of `origin/master`" (a branch that tracks another remote branch is compared with that one), say
to commit to the branch with a prefix, and where coding agents work (below).

#### Scenario: Nothing ahead of the baseline
- **GIVEN** no branch and no unassigned commit after a sync
- **WHEN** the empty state shows
- **THEN** "No active branches" says "No commits ahead of `origin/master`", to commit to the branch with a prefix, and
  where coding agents work
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`names the baseline and where coding agents work in the empty state, and installs the Agent CLI there`)

#### Scenario: Only archived branches
- **GIVEN** only archived branches exist
- **WHEN** the window shows the branches
- **THEN** the empty state alert shows
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should show empty state alert when only archived branches exist`)

### Requirement: Grouping: One set of examples everywhere

One set of examples MUST be used everywhere (empty state, guide, popover, README): `(auth) Add login form`, `ABC-123
Fix login bug`, `[threading] ABC-123 Fix login bug`.

#### Scenario: The examples of the grouping
- **WHEN** the empty state, the guide, the popover or the README shows examples of the grouping
- **THEN** they are `(auth) Add login form`, `ABC-123 Fix login bug`, `[threading] ABC-123 Fix login bug`

### Requirement: Grouping: Move to says that it rewrites the commits

Move to… MUST say that it rewrites the commits: "Creates auth with 2 commits: rewrites them to start with (auth)".

#### Scenario: Move two commits to a new branch
- **WHEN** the user moves 2 commits to a new branch `auth`
- **THEN** Move to… says "Creates auth with 2 commits: rewrites them to start with (auth)"

### Requirement: Agents: Install Agent CLI says how coding agents work from now on

Help → Install Agent CLI… ([Agent CLI](../../cli/agent-cli/spec.md#brd-install)) MUST say in its toast how coding
agents work from now on, by the rules of every repository ([brd workspace](../../cli/workspace/spec.md)): "Agents work
in their own worktree and land each commit on your branch, and name the branch in each commit subject." (by default),
with "Open Agent Settings", which opens Settings → Agents.

#### Scenario: The toast of the install
- **WHEN** the user installs the Agent CLI from Help → Install Agent CLI…
- **THEN** its toast says "Agents work in their own worktree and land each commit on your branch, and name the branch in
  each commit subject." by default
- **AND** "Open Agent Settings" opens Settings → Agents
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says how agents work once the Agent CLI is installed, and opens their settings`)

#### Scenario: The rules of every repository
- **WHEN** the rules of every repository differ from the defaults
- **THEN** the toast says how agents work by them
  [@test] src/hooks/useInstallAgentCli.test.ts (`says how agents work by the rules of every repository`)

### Requirement: Agents: The help of the unassigned commits says whether agents write the prefix

The help of the unassigned commits, where the `(name)` prefix first shows, MUST say whether agents write it here
(`branchdeck.agentSubjectBranch` of the repository): "Coding agents write (name) when they commit here.", else "Coding
agents write (name) when they commit once Name the branch in each commit subject is on for this repository.", with
"Open Agent Settings", which opens Settings → Agents on the tab of the repository, the rule focused.

#### Scenario: The help of the unassigned commits
- **WHEN** the user opens the help of the unassigned commits
- **THEN** it says whether coding agents write (name) here, by `branchdeck.agentSubjectBranch` of the repository
- **AND** "Open Agent Settings" opens Settings → Agents on the tab of the repository, the rule focused
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says in the help of the unassigned commits whether agents write the prefix here, and opens their settings`)

### Requirement: Agents: The way to the first branches says where coding agents work

On the way to the first branches, the Ready to Sync card and "No active branches" MUST say where coding agents work:
the user never picks a worktree, an agent leases one through brd ([brd lease](../../cli/lease/spec.md)), by
`branchdeck.agentLease` of the repository (on by default). The status of the Agent CLI MUST be read when the line
shows, on each focus and after an install.

#### Scenario: The lease rule on, the Agent CLI not installed
- **GIVEN** `branchdeck.agentLease` is on and the Agent CLI is not installed
- **WHEN** the Ready to Sync card or "No active branches" shows
- **THEN** the line says "You work in this checkout. With the Agent CLI, coding agents such as Claude Code lease
  worktrees of their own and land each commit on your branch." with "Install Agent CLI" (the install and the toast of
  Help → Install Agent CLI…)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says in Ready to Sync where coding agents work, installs the Agent CLI there and opens Agent Settings`, `names the baseline and where coding agents work in the empty state, and installs the Agent CLI there`)
  [@test] src/lib/agents-hint.test.ts (`says where agents work by the rule of the repository, and that the Agent CLI lets them`)

#### Scenario: The lease rule on, the Agent CLI installed
- **GIVEN** `branchdeck.agentLease` is on and the Agent CLI is installed
- **WHEN** the line shows
- **THEN** it says "You work in this checkout. Coding agents such as Claude Code lease worktrees of their own through
  brd and land each commit on your branch." with "Open Agent Settings" (Settings → Agents on the tab of the
  repository, the rule focused)
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`says in Ready to Sync where coding agents work, installs the Agent CLI there and opens Agent Settings`)
  [@test] src/lib/agents-hint.test.ts (`says where agents work by the rule of the repository, and that the Agent CLI lets them`)

#### Scenario: The lease rule off
- **GIVEN** `branchdeck.agentLease` is off
- **WHEN** the line shows
- **THEN** it says "With the Agent CLI, coding agents such as Claude Code work in this checkout beside you."
  (installed: "Coding agents such as Claude Code work in this checkout beside you, through brd.")
  [@test] src/lib/agents-hint.test.ts (`says where agents work by the rule of the repository, and that the Agent CLI lets them`)

#### Scenario: The status of the Agent CLI
- **WHEN** the line shows, the window gains the focus, or an install ends
- **THEN** the status of the Agent CLI is read again

### Requirement: Agents: Settings Agents asks to install the skill

Settings → Agents without the skill installed MUST say "Agents read these rules once the skill is installed" with
"Install Agent CLI" ([Settings](../settings/spec.md#agents)); Settings → Tools MUST name the skill once it is
installed.

#### Scenario: The skill is not installed
- **GIVEN** the skill is not installed
- **WHEN** the user opens Settings → Agents
- **THEN** it says "Agents read these rules once the skill is installed" with "Install Agent CLI"

#### Scenario: The skill is installed
- **GIVEN** the skill is installed
- **WHEN** the user opens Settings → Tools
- **THEN** it names the skill

### Requirement: AI and help: The first suggestions prompt says what enabling sends

The first suggestions prompt ("Not now", "Enable AI") MUST say what enabling sends and where: "Sends the commit messages
and changed file names to Claude (Haiku) through your Claude Code"
([Branch Name Suggestions](../../branch/branch-name-suggestions/spec.md)).

#### Scenario: The first suggestions prompt
- **WHEN** the first suggestions prompt shows
- **THEN** it says "Sends the commit messages and changed file names to Claude (Haiku) through your Claude Code", with
  "Not now" and "Enable AI"
- **AND** nothing is sent to Claude before the user enables it
  [@test] tests/e2e/inline-branch-creator-ai/inline-branch-creator-ai.spec.ts (`should ask before anything is sent to Claude`)

### Requirement: AI and help: Help Getting Started opens the README

Help → Getting Started MUST open the Getting Started section of the README.

#### Scenario: Getting Started
- **WHEN** the user chooses Help → Getting Started
- **THEN** the Getting Started section of the README opens

## User Experience

- The welcome card and the Ready to Sync card are stock `Card` islands; the alerts are `Alert`s
  ([UI Theme](../../../../docs/design/ui-theme.md)). A command is a `CommandLine`: monospace on `bg-muted`, scrolled, with a
  copy button.
- Wording follows [UX Writing](../../../../docs/contributing/ux-writing.md): "Cannot find Git", "Check Again", "Ready to Sync".
