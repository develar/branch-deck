---
name: Onboarding
description: Requirements for the first run of Branch Deck, from an empty window to the first branches: the welcome card with the branch prefix step, the card before the first sync, the grouping hint, and the git, identity and repository problems that block the way.
style: plain-1
targets:
  - ../../src/components/branch/BranchCreator.tsx
  - ../../src/components/repository/WelcomeCard.tsx
  - ../../src/components/repository/ReadyToSyncCard.tsx
  - ../../src/components/repository/GitSetupAlert.tsx
  - ../../src/components/repository/BranchPrefixHelp.tsx
  - ../../src/components/settings/BranchPrefixSetting.tsx
  - ../../src/components/repository/SyncButton.tsx
  - ../../src/hooks/useSyncOnOpen.ts
  - ../../src/components/unassigned/UnassignedCommitListCard.tsx
  - ../../src/components/unassigned/UnassignedCommitsHelpPopover.tsx
  - ../../src/components/shared/CommandLine.tsx
  - ../../src/components/shared/InlineBranchTarget.tsx
  - ../../src/components/ai/BranchNameSuggestions.tsx
  - ../../src/hooks/useInstallAgentCli.ts
  - ../../src/components/settings/AgentsPage.tsx
  - ../../src/lib/branch-prefix.ts
  - ../../src/stores/repository.ts
  - ../../src-tauri/src/commands/branch_prefix.rs
  - ../../src-tauri/src/menu.rs
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useMenuSync.ts
  - ../../crates/sync-core/src/git_setup.rs
  - ../../crates/sync-core/src/branch_prefix.rs
  - ../../crates/sync-core/src/repository_validation.rs
  - ../../crates/history-rewrite/src/baseline.rs
  - ../../crates/sync-core/src/sync_inputs.rs
  - ../../crates/git-executor/src/git_info.rs
---

# Onboarding

Status: Implemented
Date: 2026-10-15

## Summary

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

### Git and the identity

- When the window opens and whenever the selected repository changes, one `check_git_setup` checks that Git is found
  and new enough, and runs one `git var GIT_COMMITTER_IDENT` (in the repository, else in the global config).
  [@test] ../../crates/sync-core/src/git_setup_test.rs (`reads_the_identity_of_the_repository`)
- Git missing or too old is a `destructive` alert above everything else: the title is the summary ("Cannot find Git:
  …", "Git 2.39 is too old: Branch Deck requires Git 2.54 or newer"), the description how to install or upgrade it on
  this platform, then "Check Again". It replaces the path and sync errors, which would say the same; the welcome card is
  hidden. The branch prefix load reports a discovery error as itself, not as "Failed to access repository".
  [@test] ../../crates/git-executor/src/git_info.rs (`a_missing_explicit_git_is_not_found_with_the_hint`, `a_failing_git_is_not_found_with_its_first_error_line`, `an_old_git_is_too_old_with_the_hint`)
- An identity git cannot form is a `warning` alert "Git does not know your name and email" with the two
  `git config --global` commands (each with a copy button) and "Check Again".
- A message of several lines in the error alert shows its first line as the title, the rest as the description.

### Welcome card

- Shown on the first run (no repository, no recent projects), and while the selected repository has no branch prefix
  (once the prefix has loaded); hidden while an error alert explains a problem.
  [@test] ../../tests/e2e/welcome-card/welcome-card.spec.ts (`shows welcome card for new users without branch prefix`, `shows the prefix step as step 1 when the repository has no branch prefix configured`, `keeps Step 2 visible after selecting a repository without branch prefix`)
  [@test] ../../tests/e2e/repository/browse.spec.ts (`should show error for non-git directory`)
- The steps shown are numbered from 1: "Select a Git repository", then "Choose your branch prefix".
- The prefix step has a field pre-filled with the suggestion of `check_git_setup`: the local part of the committer
  email, without a `+tag`, sanitized as a branch name. Save (or Enter) stores it as the app's branch prefix (used by
  `bd` too); nothing is stored before. Below the field: "Branches will be named `<prefix>/virtual/<name>`. You can
  change it later in Settings."
  [@test] ../../crates/sync-core/src/git_setup_test.rs (`suggests_the_local_part_of_the_committer_email`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`pre-fills the prefix from the git identity and saves it only on Save`)
- A prefix is trimmed and loses its trailing slashes, as the backend names the branch (`qualified_branch_name`); one
  with a space, `..`, `@{` or any of `~ ^ : ? * [ \` is refused in place, Save disabled.
  [@test] ../../src/lib/branch-prefix.test.ts (`normalizeBranchPrefix`, `isValidBranchPrefix`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`saves a typed prefix without its trailing slash and refuses one git cannot take`)
- The alternative is the command `git config --global branchdeck.branchPrefix your-name` with a copy button, and
  "View guide" opens the branch prefix guide: the field of Settings first ("takes precedence over the global Git
  setting"), with "Open Settings", which opens Settings → General with the field focused (not in the guide Settings
  itself opens), then the global command, the grouping examples, and the prefix of one repository on Settings →
  Repository, which takes precedence over the one of Branch Deck, with "Open Repository Settings" (its field focused).
  [@test] ../../tests/e2e/welcome-card/welcome-card.spec.ts (`opens branch prefix help modal from welcome card`, `opens Settings at the field of the prefix from the guide, and at the one of the repository`)
  [@test] ../../tests/e2e/settings/settings.spec.ts (`should open the guide of the branch prefix without a button that opens Settings`)
- Once set, the prefix is changed in Settings → General (⌘,), "Branches" ([Settings](settings.spec.md)): the toolbar
  has no field for it, as the prefix is set once and the welcome card asks for it while it is missing. The field saves
  on Enter and when it loses the focus, never on a key (the prefix names the branches of the sync and the robots of
  Patronus), refuses a prefix git cannot take in place, and Escape takes the typing back before it closes the window.
  Empty, the prefix of git for the selected repository applies (the window reads it itself), shown as the placeholder.
  A sync reads the prefix when it starts: a change during one names the branches of the next. A sync that starts while
  the prefix of git loads (a selected repository, a changed config) waits for that load as long as it takes, also for
  the load of a repository selected meanwhile, and never fails on a busy machine for a fixed timeout.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`, `pre-fills the prefix from the git identity and saves it only on Save`)
  [@test] ../../src/stores/branch-sync.test.ts (`syncs with the prefix of a load that takes longer than a few seconds, without an error`, `waits for the load of a repository selected while the first load ran`)
- The specific prefix wins, in the app as in `bd` ([Settings](settings.spec.md#branch-prefix-order)): a repository
  whose own git config (local or worktree, or `git -c`) sets `branchdeck.branchPrefix` names its branches by it, whatever the field
  says; the field says so ("This repository uses its own prefix `user-name`", with "Open Repository Settings") and
  names the branches the typed prefix gives elsewhere. Without one, the app's prefix applies, then the global git prefix.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`names the branches by the repository's own prefix over the app's, and says so in Settings`, `names the branches by the app's prefix where the repository has none`)

### Before the first sync

- Sync is disabled while the selected repository has no branch prefix (not while it loads), with the tooltip "Set the
  branch prefix first".
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`disables Sync with the reason while no branch prefix is set`)
- With a repository never synced and a prefix, the "Ready to Sync" card: what Sync does (groups the commits by the
  prefix of their subject, creates `<prefix>/virtual/<name>`, lists the rest as unassigned), that the working tree and
  the checked-out branch stay as they are, and when it syncs from then on (when it opens the repository, after each
  change it makes, and on focus after a commit, a fetch or a push elsewhere, Auto-sync on Focus). Its Sync button syncs (with the
  spinner while it runs, its label stays "Sync"); the card goes when the sync completes, or gives way to the error alert.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says what Sync does before the first sync, then gives way to the branches`)
- A repository synced before syncs when the window opens it, at start and on a switch, also with Auto-sync on Focus
  off: no card, the spinner of Sync in the header, then the branches and Update. A failed sync is the error alert and is not
  repeated until the window opens the repository again (⌘R retries).
- Auto-sync on Focus (Settings → General, File → Auto-sync on Focus) is on by default and syncs on a focus
  only when what a sync reads changed since the last sync (`syncIfChanged`, the fingerprint of
  [Git Operations](../../docs/design/git-operations.md#sync-inputs-check), taken when the sync started, so a commit made
  while it ran counts): a commit, an amend or a finished rebase in the IDE, a fetch (of the baseline, also an upstream
  other than `master` or `main`) or a push elsewhere, a regroup or a description by `bd`. Otherwise the focus reads the inputs once and syncs nothing, and the header says "Checked just
  now" instead of "Last synced 18 minutes ago", the age of the sync in its tooltip: the branches are as current as a
  sync would make them. A user who turned the setting off keeps it off.
  [@test] ../../crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`, `changes_with_the_upstream_of_the_checked_out_branch`)
  [@test] ../../src/stores/branch-sync.test.ts (`syncs only when what a sync reads changed since the last sync, one read otherwise`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`syncs on focus only when what a sync reads changed, and says when it checked`)
- File → Auto-sync on Focus of the native menu is checked as the setting is: at startup as `settings.json` says
  (`stored_auto_sync_on_focus`), then by the main window for every value of the setting, whichever window changed it
  (`set_auto_sync_menu_checked`); checking the item sets the setting (`auto_sync_on_focus_toggled`, sent to the main
  window). The switch of Settings → General is the same setting: its write through the main window checks the item.
  [@test] ../../src-tauri/src/menu_test.rs (`auto_sync_on_focus_is_checked_as_the_stored_setting`, `without_a_stored_setting_auto_sync_on_focus_is_checked`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)
- A sync while git is in the middle of a rebase, merge, cherry-pick or revert, or with HEAD detached, is refused before
  it changes anything (`SyncDeferred`, [Git Operations](../../docs/design/git-operations.md#virtual-branch-sync)): a
  half-done history would archive the branches it lacks. It is no failure: no error alert, no red Sync, the branches as
  the last sync left them. A sync the user asked for (Sync, ⌘R, the Ready to Sync card) says so in an `info` toast, "Not
  synced: a rebase is in progress" / "The branches sync once git finished it" ("Not synced: HEAD is detached" / "Check
  out a branch to sync its commits"); a sync the app started (on open, on focus, after a change) waits silently.
  [@test] ../../crates/sync-core/src/sync_test.rs (`sync_waits_while_a_rebase_is_in_progress_or_head_is_detached`)
  [@test] ../../crates/bd-cli/src/sync_test.rs (`waits_while_a_rebase_is_in_progress`)
  [@test] ../../src/stores/branch-sync.test.ts (`is no failure: no error, the branches as they were, an info toast only for a sync the user asked for`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says in an info toast that a sync waits for git, and leaves the branches as they were`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`syncs a repository synced before when the window opens it`)
- A switch to another repository forgets the branches of the previous one once its running sync is done: they never
  show under the name of another repository.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`forgets the branches of the previous repository on a switch`)

### Repository problems

- A folder that is not a repository: "Not a Git repository"; a bare repository: "Bare repositories are not supported:
  Branch Deck needs a working tree"; a worktree (`.git` file) is a repository.
  [@test] ../../crates/sync-core/src/repository_validation.rs (`test_validate_path_not_git_repo`, `test_validate_path_bare_repo`, `test_validate_path_linked_worktree`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`refuses a bare repository with the reason`)
- A repository without commits fails the sync with "The repository has no commits yet: commit to the main branch, then
  sync", not with "No baseline branch found"; `bd` says the same as `no_baseline` (see [Agent CLI](../cli/agent-cli.spec.md)).
  A repository without a baseline names each branch it looked for once ("Cannot find the baseline branch: no
  origin/master or origin/main, and no upstream").
  [@test] ../../crates/history-rewrite/src/baseline_test.rs (`a_repository_without_commits_says_so`, `no_local_baseline_names_each_branch_once`, `no_remote_baseline_names_each_branch_once`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says that a repository without commits has nothing to sync yet`)
- A saved repository that no longer exists is an error alert, Sync disabled, until another one is selected.
  [@test] ../../tests/e2e/repository/saved-path-validation.spec.ts (`should display error for non-existent repository path`, `should clear error when valid repository is selected`)

### Grouping

- While there is no branch, the unassigned commits card says under its title: "Start a commit subject with `(name)` or
  an issue key like `ABC-123` to group it into a branch, or select commits and move them to one."
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`explains the grouping in the unassigned commits while there is no branch`)
- One set of examples everywhere (empty state, guide, popover, README): `(auth) Add login form`, `ABC-123 Fix login
  bug`, `[threading] ABC-123 Fix login bug`.
- Move to… says that it rewrites the commits: "Creates the branch auth with 2 commits: rewrites them to start with
  (auth)".

### Agents

- Help → Install Agent CLI… ([Agent CLI](../cli/agent-cli.spec.md#bd-install)) says in its toast how coding agents work
  from now on, by the rules of every repository ([bd workspace](../cli/workspace.spec.md)): "Agents work in their own
  worktree and land each commit on your branch; commit subjects stay as written." (by default), with "Open Agent
  Settings", which opens Settings → Agents.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says how agents work once the Agent CLI is installed, and opens their settings`)
  [@test] ../../src/hooks/useInstallAgentCli.test.ts (`says how agents work by the rules of every repository`)
- The help of the unassigned commits, where the `(name)` prefix first shows, says whether agents write it here
  (`branchdeck.agentSubjectBranch` of the repository): "Coding agents write (name) when they commit here.", else "Coding
  agents write (name) when they commit once Name the branch in each commit subject is on for this repository.", with
  "Open Agent Settings".
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says in the help of the unassigned commits whether agents write the prefix here, and opens their settings`)
- Settings → Agents without the skill installed offers "Install Agent CLI" ([Settings](settings.spec.md#agents)).

### AI and help

- The first suggestions prompt ("Not now", "Enable AI") says what enabling sends and where: "Sends the commit messages
  and changed file names to Claude (Haiku) through your Claude Code" ([Branch Name Suggestions](../branch/branch-name-suggestions.spec.md)).
  [@test] ../../tests/e2e/inline-branch-creator-ai/inline-branch-creator-ai.spec.ts (`should ask before anything is sent to Claude`)
- Help → Getting Started opens the Getting Started section of the README.

## User Experience

- The welcome card and the Ready to Sync card are stock `Card` islands; the alerts are `Alert`s
  ([UI Theme](../../docs/design/ui-theme.md)). A command is a `CommandLine`: monospace on `bg-muted`, scrolled, with a
  copy button.
- Wording follows [UX Writing](../../docs/guidelines/ux-writing.md): "Cannot find Git", "Check Again", "Ready to Sync".
