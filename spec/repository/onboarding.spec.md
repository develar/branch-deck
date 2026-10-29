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
  - ../../src/components/repository/AgentsHint.tsx
  - ../../src/components/shared/InstallAgentCliButton.tsx
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/hooks/useAgentCliStatus.ts
  - ../../src/stores/agent-cli.ts
  - ../../src/lib/agents-hint.ts
  - ../../src/lib/sync-step.ts
  - ../../src/lib/sync-deferred.ts
  - ../../crates/repository-model/src/table.rs
  - ../../crates/repository-model/src/runtime.rs
  - ../../crates/git-ops/src/rewrite_guard.rs
  - ../../src/hooks/useSyncOnOpen.ts
  - ../../src/model/store.ts
  - ../../src/router.ts
  - ../../src-tauri/src/warm_start.rs
  - ../../crates/repository-model/src/warm_start.rs
  - ../../crates/app-settings/src/projects.rs
  - ../../crates/repository-model/src/sync_scheduler.rs
  - ../../crates/sync-core/src/sync_inputs.rs
  - ../../crates/repository-model/src/session.rs
  - ../../src/stores/settings/repositoryState.ts
  - ../../src/components/unassigned/UnassignedCommitListCard.tsx
  - ../../src/components/unassigned/UnassignedCommitsHelpPopover.tsx
  - ../../src/components/shared/CommandLine.tsx
  - ../../src/components/shared/InlineBranchTarget.tsx
  - ../../src/components/ai/BranchNameSuggestions.tsx
  - ../../src/hooks/useInstallAgentCli.ts
  - ../../src/components/settings/AgentsPage.tsx
  - ../../src/lib/branch-prefix.ts
  - ../../src/stores/repository.ts
  - ../../crates/app-api/src/reads/repository.rs
  - ../../crates/app-settings/src/resolve.rs
  - ../../crates/app-settings/src/branch_prefixes.rs
  - ../../crates/app-settings/src/service.rs
  - ../../crates/app-api/src/session/view.rs
  - ../../crates/app-api/src/context.rs
  - ../../src/stores/branch-sync.ts
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/lib/time.ts
  - ../../src-tauri/src/menu.rs
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useMenuSync.ts
  - ../../crates/sync-core/src/git_setup.rs
  - ../../crates/app-api/src/api_error.rs
  - ../../crates/git-ops/src/commit_writer.rs
  - ../../src/hooks/useSyncAvailability.ts
  - ../../crates/sync-core/src/branch_prefix.rs
  - ../../crates/sync-core/src/repository_validation.rs
  - ../../crates/app-api/src/app/desktop.rs
  - ../../crates/test-server/src/host.rs
  - ../../crates/history-rewrite/src/baseline.rs
  - ../../crates/sync-core/src/sync_inputs.rs
  - ../../crates/git-executor/src/git_info.rs
---

# Onboarding

Status: Implemented
Date: 2026-10-25

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

- No check of git runs before the first sync, and the page reads nothing for it: git and the identity are answered by
  the code of what needs them. The subscription of the window is the first command that runs git (it reads the prefix
  from the git config, `session_key_now`); without a usable git (missing, failing or older than 2.54) it fails with
  `git_not_found` (`GitNotFound` of the discovery, `app_api::api_error`), whose words are the summary and how to install
  or upgrade Git.
  [@test] ../../crates/git-executor/src/git_info.rs (`a_missing_explicit_git_is_not_found_with_the_hint`, `a_failing_git_is_not_found_with_its_first_error_line`, `an_old_git_is_too_old_with_the_hint`)
  [@test] ../../crates/app-api/src/api_error_test.rs (`api_error_answers_the_setup_of_git_by_its_code`)
- Git missing or too old is a `destructive` alert above everything else: the title is the summary ("Cannot find Git:
  …", "Git 2.39 is too old: Branch Deck requires Git 2.54 or newer"), the description how to install or upgrade it on
  this platform, then "Check Again", which subscribes again. It replaces the path and sync errors, which would say the
  same; the welcome card is hidden, Sync disabled with the summary as its reason.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says above everything else that git is missing, and opens the repository once Check Again finds it`)
- An identity git cannot form fails the write that needs it with `no_identity` (`NoIdentity` of `current_committer`:
  the notes of a sync and of its detection, which `write_cache_notes` never swallows, a rewritten commit), in the app
  as in `bd`: a sync that wrote no notes would copy every commit again on each run. A sync that failed so is the
  `warning` alert "Git does not know your name and email" with the two `git config --global` commands (each with a
  copy button) and "Sync Again", which syncs again, instead of the error alert.
  [@test] ../../crates/git-ops/src/commit_writer_test.rs (`a_committer_git_cannot_form_is_no_identity`)
  [@test] ../../crates/sync-core/src/sync_test.rs (`a_sync_whose_notes_git_cannot_sign_fails_with_no_identity`)
  [@test] ../../crates/test-server/src/session_routes_test.rs (`a_sync_without_an_identity_git_can_form_fails_with_no_identity`)
  [@test] ../../crates/app-api/src/api_error_test.rs (`api_error_answers_the_setup_of_git_by_its_code`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`warns that git does not know the user's name and email when a sync fails on it, and syncs on Sync Again`)
- A message of several lines in the error alert shows its first line as the title, the rest as the description.

### Welcome card

- Shown on the first run (no repository, no recent projects), asking for the repository, and while the subscription of
  the selected repository is refused for its prefix (`no_branch_prefix`), asking for the prefix; hidden while an error
  alert explains a problem. Without a repository the page knows no prefix (the session of a repository resolves it),
  so the card asks one step at a time, each numbered 1: "Select a Git repository", then, for a repository without a
  prefix, "Choose your branch prefix".
  [@test] ../../tests/e2e/welcome-card/welcome-card.spec.ts (`shows the welcome card for new users, which asks for the repository first`, `shows the prefix step as step 1 when the repository has no branch prefix configured`, `keeps Step 2 visible after selecting a repository without branch prefix`)
  [@test] ../../tests/e2e/repository/browse.spec.ts (`should show error for non-git directory`)
- The prefix step has a field pre-filled with the prefix the refusal suggests (`ErrorDetails::PrefixSuggestion`,
  `prefix_suggestion`: one `git var GIT_COMMITTER_IDENT` in the repository, only on the refusal): the local part of
  the committer email, without a `+tag`, sanitized as a branch name. Save (or Enter) stores it as the app's branch
  prefix (used by `bd` too); nothing is stored before. Below the field: "Branches will be named
  `<prefix>/virtual/<name>`. You can change it later in Settings."
  [@test] ../../crates/sync-core/src/git_setup_test.rs (`suggests_the_local_part_of_the_committer_email`, `suggests_the_prefix_of_the_identity_of_the_repository`)
  [@test] ../../crates/test-server/src/session_routes_test.rs (`a_repository_without_a_prefix_is_refused_with_the_api_error_of_the_app`)
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
  Empty, the prefix of the global git config applies (the window asks Rust itself, `resolve_branch_prefix`), shown as
  the placeholder. The session of a repository is keyed by the prefix Rust resolves: a change of the app's prefix (once
  the writer has it) subscribes the window again, holding the view it shows (`resubscribeSession`); the same session
  answers nothing new and the rows stay, the session of another prefix answers its view, which replaces them, and
  syncs as it opens for a repository synced before. A sync never waits for a load of the page, so a busy machine fails
  none for a timeout. Without a prefix the window holds no session, and nothing is logged as an error: the welcome
  card asks for one.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`changes the prefix in Settings on Enter or when the field is left, not on every key`, `pre-fills the prefix from the git identity and saves it only on Save`)
  [@test] ../../src/model/store.test.ts (`names only the repository: the session resolves the prefix, and a sync waits for no load of the page`, `subscribes first when asked before the window subscribed, and again after a failed subscription, whose failure it names`, `subscribes again holding its view: the same session answers nothing, another one's view replaces the rows`)
- The specific prefix wins, in the app as in `bd` ([Settings](settings.spec.md#branch-prefix-order)): a repository
  whose own git config (local or worktree, or `git -c`) sets `branchdeck.branchPrefix` names its branches by it, whatever the field
  says; the field says so ("This repository uses its own prefix `user-name`", with "Open Repository Settings") and
  names the branches the typed prefix gives elsewhere. Without one, the app's prefix applies, then the global git prefix.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`names the branches by the repository's own prefix over the app's, and says so in Settings`, `names the branches by the app's prefix where the repository has none`)

### Before the first sync

- Sync is disabled while the selected repository has no branch prefix (not while its subscription has not answered),
  with the tooltip "Set the branch prefix first". The page reads nothing before the subscription of the window (no
  prefix, no check of git) and shows the prefix of the view of the session (`TableHeader.prefix`, also before any sync
  and in the frame of the warm start): the prefix Rust resolved for the subscription from the git config as it is
  then (`session_key_now`, which the commands after it resolve by). It decides nothing by it: a sync names only the
  repository and resolves the prefix in Rust from the git config it reads, waiting for no load of the page, and
  refuses a repository without one.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`disables Sync with the reason while no branch prefix is set`)
  [@test] ../../src/model/store.test.ts (`names only the repository: the session resolves the prefix, and a sync waits for no load of the page`, `names the branches by the prefix of the view, and none after a failed subscription, which forgets the rows of the seed`)
  [@test] ../../crates/test-server/src/session_routes_test.rs (`a_subscription_opens_the_session_under_the_prefix_the_config_names_now`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_session_without_subscribers_runs_nothing_and_goes_away`)
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`shows the rows of the frame before the first sync, asking only for the subscription, and the first sync's table after it`)
  [@test] ../../crates/test-server/src/branch_prefix_test.rs (`the_repository_s_own_prefix_wins_and_a_repository_without_any_is_not_synced`)
- With a repository never synced and a prefix, the "Ready to Sync" card: what Sync does (groups the commits by the
  prefix of their subject, creates `<prefix>/virtual/<name>`, lists the rest as unassigned), that the working tree and
  the checked-out branch stay as they are, and when it syncs from then on (when it opens the repository, after each
  change it makes, and on focus after a commit, a fetch or a push elsewhere, Auto-sync on Focus). Its Sync button syncs (with the
  spinner while it runs, its label stays "Sync"); the card goes when the sync completes, or gives way to the error alert.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says what Sync does before the first sync, then gives way to the branches`)
- A repository synced before syncs when its session opens, at start and on a switch, and when a window comes back to
  a session kept idle (then only when what a sync reads changed), also with Auto-sync on Focus off: no card, the
  spinner of Sync in the header, then the branches and Update. The session starts that sync itself, with or without a
  window (`SessionHost::synced_before`), so the page asks for none and a focus during it is one check after it. A
  failed sync is the error alert and is not repeated until the window opens the repository again (⌘R retries). Synced
  before is what the last sync recorded of the repository in `repositoryState` (when it ended, the "Last synced" of the
  header, and how many branches it had, the line under the path in the repository selector), which the backend keeps
  beside the settings (`repository-state.json`): a sync never rewrites `settings.json`. The main window records every
  run of its session that ended, the ones no request of it waited for too (the sync of the open, of a rewrite, of `bd`).
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`syncs a repository synced before when the window opens it`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should age the last sync next to the Update button while it is shown`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_ticket_resolves_when_its_run_ends_not_the_chain`, `a_window_that_watches_a_kept_session_anew_syncs_it_when_the_inputs_changed`)
  [@test] ../../crates/app-settings/src/projects_test.rs (`a_project_was_synced_before_when_its_state_has_the_time_of_a_sync`)
  [@test] ../../src/model/store.test.ts (`keeps Sync busy from the request to the last patch of the run that serves it, and counts the runs that ended`)
- The app starts warm: before the main window is built, Rust reads the frame the session of the selected project saved
  last (`repository_model::warm_start`: the rows, the stacks, the sections and the facts of the remote as its last
  sync and its last answers left them) from the object cache of the repository, without a git process (the common
  git directory found by the files of the worktree), beside the prefix the git config names now (the one the
  subscription of the window resolves), opens the session of that prefix seeded from it, whose first sync starts at
  once, and puts its seed into the page as `window.__BD_MODEL__`. The page shows those rows on its first frame
  (`data-cached` on the table) and subscribes holding the seed (`subscribe_view` with `have`): the answer is the patch
  from it to the view of the session, or nothing; the rows come with that one command and its channel. Until the first
  sync commits, the rows are of the frame (`RepositoryView.cached`): a push and every other action of the remote (on
  the merge route the merge of the pull request and Cancel Auto-Merge too, the close of a pull request of a stack
  that is no more) waits for it ("Waits for the first sync"), and the backend refuses such an intent there
  (`plan_changed`) whoever asks it, after a first sync that failed (or that git deferred) with the error of that sync,
  a push that waited for it too (as a rewrite does); a rewrite pressed then (Unstack) says
  it waits ("Unstacking oauth after the sync") and runs on the table the sync committed, and a focus during that sync
  is one check after it. A first sync that committed before the subscription answered ends in the window as any run
  does: the count of Sync, "Last synced just now", the fixups of its table. It ends once: a reload of the window, whose
  init script hands the page the same seed, catches up from it again, and the page of the window remembers that it saw
  a run of that seed end (`sessionStorage`, which lives as long as the window), so the reload ends no run (no count, no
  new "Last synced"); a page that reloaded as the table of that sync went out, and never saw it, ends it after the
  reload. The frame
  keeps the landing needs with the tips they are of: a first sync that leaves those tips computes them not again, and
  no step of a row waits for them anew ("Checking what … needs to land"); the next steps, the starts and the actions of
  the rows change once, when that sync commits (the view of the robots it derives goes in the same patch when it
  comes within 16 ms). A frame of
  another prefix (saved before the prefix changed) or schema, a project without one, or a model of another project (a
  reload after a switch) start as before, without rows until the first sync.
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`shows the rows of the frame before the first sync, asking only for the subscription, and the first sync's table after it`)
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`handles the end of a first sync that committed before the subscription answered as the end of any run`)
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`ends the first sync once: a reload of the window after it ends no run`)
  [@test] ../../crates/repository-model/src/state_test.rs (`a_seeded_view_is_the_frame_at_the_first_version_and_a_window_that_holds_it_catches_up`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_start_seeds_nothing_from_a_frame_of_another_prefix_than_the_one_of_now`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`on_a_frame_nobody_checked_every_remote_action_waits_for_the_first_sync_and_the_rest_stays`)
  [@test] ../../crates/operations/src/rewrite_test.rs (`a_remote_intent_on_a_seeded_table_is_refused_before_it_is_admitted`, `a_push_that_waited_for_a_first_sync_that_failed_is_refused_and_sends_nothing`)
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`says a push of the rows of the frame waits for the first sync, and offers it once the sync committed`)
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`says the close of a pull request of a stack that is no more waits for the first sync on the rows of the frame`)
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`runs an unstack pressed on the rows of the frame once the first sync committed, waiting for it until then`)
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`checks a focus during the first sync once after it, with no second run`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_session_opened_from_the_frame_of_the_start_answers_its_seed_and_syncs_with_no_window`, `a_page_that_reloads_as_the_first_sync_commits_catches_up_from_the_seed_to_the_table_of_this_run`, `a_seeded_session_shows_the_steps_and_the_actions_of_its_rows_a_push_waiting_for_the_first_sync`, `a_frame_of_another_prefix_or_schema_seeds_nothing`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_warm_first_sync_that_leaves_the_tips_of_the_frame_changes_every_subject_in_one_patch_at_most`, `the_end_of_a_sync_goes_with_the_view_of_its_robots_in_one_patch`, `the_end_of_a_sync_waits_for_the_view_of_its_robots_one_interval_at_most`)
  [@test] ../../src-tauri/src/warm_start_test.rs (`the_common_dir_is_found_by_the_files_of_the_main_and_of_a_linked_worktree`, `the_model_is_the_json_of_the_path_and_the_view_in_a_string_that_json_parse_reads`)
  [@test] ../../crates/app-settings/src/projects_test.rs (`the_selected_project_is_the_first_recent_one`)
  [@test] ../../src/model/store.test.ts (`shows the rows of the seed before anything answers, and catches up by the patch from it, the patches before the answer after it`, `keeps the seed when the session sent nothing since, and leaves a model of another project`, `ends the first sync once: a reload that installs the seed again after its end catches up from it and ends no run`, `ends the first sync in a reload of a window whose page went away as the table of that sync came`)
  [@test] ../../src/presenters/operations.test.ts (`says that an operation waits for the sync, behind another one, or what it does`)
- While a sync runs and the page has no rows to show (the first sync of a session of a repository synced before: when
  the window opens it at start, or another session of it, under a prefix changed in a terminal or in Settings; a sync
  after one that failed or waited for git), the body is the loading table where the rows will
  be, never an empty page under a spinning Sync. Its line says what the sync does, as the tooltip of Sync does:
  "Grouping the commits…", "Building the branches…", "Checking which branches landed…", and "Syncing branches…" before a
  step is known. A failed sync's alert gives way to it while the next sync runs.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`shows the loading table in the body during the first sync of another session of a repository synced before`)
  [@test] ../../src/lib/sync-step.test.ts (`names what a running sync does, and only that it runs before a step is known`)
  [@test] ../../src/model/store.test.ts (`names the step of the running sync, and none once it ended`)
- Auto-sync on Focus (Settings → General, File → Auto-sync on Focus) is on by default and syncs on a focus
  only when what a sync reads changed since the last sync of the session (the inputs of
  [Git Operations](../../docs/design/git-operations.md#sync-inputs-check), read when the sync started, so a commit made
  while it ran counts): a commit, an amend or a finished rebase in the IDE, a fetch (of the baseline, also an upstream
  other than `master` or `main`) or a push elsewhere, a regroup or a description by `bd`, a changed prefix (the
  inputs are of the prefix Rust resolves). Otherwise the focus reads the inputs once (two git processes) and syncs nothing, and the header says "Checked just
  now" instead of "Last synced 18 minutes ago", the age of the sync in its tooltip: the branches are as current as a
  sync would make them. It says so only where the two read apart: for a check that came when "Last synced" no longer
  read "just now", a minute or more after the sync (`readsLater`). A check in the minute of the sync (the one after the
  sync of the start, which a focus during it asks, 100 ms later) leaves "Last synced just now", which says as much, and
  keeps it as both age; compared as they read now they would take turns at every edge of their buckets ("1 hour ago"
  and "2 hours ago"). A focus during a sync checks once after it, however many came. A user who turned the setting
  off keeps it off: a focus then starts no git process.
  [@test] ../../src/lib/time.test.ts (`says a check in the minute of the sync nothing "just now" does not: the check of the start, 100 ms after its sync`, `keeps its answer while both age, where their texts read the same now and apart a bucket later`)
  [@test] ../../crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`, `changes_with_the_upstream_of_the_checked_out_branch`, `changes_with_the_prefix_alone`)
  [@test] ../../crates/repository-model/src/sync_scheduler_test.rs (`a_focus_runs_only_when_the_inputs_changed`, `a_focus_during_a_run_checks_the_inputs_once_after_the_chain`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_focus_with_unchanged_inputs_reads_only_the_fingerprint`, `a_focus_during_a_run_is_one_check_after_it`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`syncs on focus only when what a sync reads changed, and says when it checked`)
  [@test] ../../tests/e2e/startup/warm-start.spec.ts (`checks a focus during the first sync once after it, with no second run`)
- A sync that failed, or that git deferred, is not repeated by a focus while what a sync reads is as that sync read
  it, with which of the files git keeps during an operation exist (`rebase-merge`, `rebase-apply`, `MERGE_HEAD`,
  `CHERRY_PICK_HEAD`, `REVERT_HEAD`, `sequencer`): the focus reads the inputs once and answers the error of that sync,
  the alert stays and no run starts, so a sync of a large repository that fails does not run again on every return to
  the window. A change syncs: a commit, the end of a rebase (also an abort, which moves no ref), a branch checked out
  again after a detached HEAD. Sync (⌘R) and the window opening the repository sync anyway; ⌘R during a run asks for the
  check after it, which a run that failed with nothing changed answers with its error.
  [@test] ../../crates/repository-model/src/sync_scheduler_test.rs (`a_focus_after_a_failed_run_with_the_same_inputs_answers_its_error_without_a_run`, `a_focus_after_a_failed_run_runs_when_the_inputs_changed`, `an_open_after_a_failed_run_runs_again_also_joining_a_focus_check`, `a_prefix_change_is_not_remembered`, `a_failed_run_settles_its_requests_with_the_error`)
  [@test] ../../crates/sync-core/src/sync_inputs_test.rs (`changes_when_a_git_operation_starts_or_ends`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_focus_after_a_failed_sync_reads_only_the_fingerprint`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`does not sync again on a focus after a failed sync while nothing changed, and syncs again on Sync`)
- The app's own push is no change for the focus: it moves the remote branches it sent, which a sync reads, but the
  table has its outcome already, and the session puts those remote branches into the inputs it knows, so a focus after
  a Push syncs nothing ("Checked just now" a minute after the sync). A push that ends while a sync or the check of a focus runs counts
  too; a remote branch moved by another clone or a fetch elsewhere still syncs.
  [@test] ../../crates/repository-model/src/sync_scheduler_test.rs (`a_focus_after_the_sessions_own_push_runs_nothing`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_focus_after_the_sessions_own_push_runs_no_sync`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`syncs nothing on the focus after its own push, the remote branch it moved known to the session`)
- A prefix changed in a terminal (`git config branchdeck.branchPrefix`) shows on the next focus: its check reads the
  prefix among the inputs of a sync, and the run of the session under the old prefix builds nothing under it: it
  answers `prefix_changed`, which is no failure, and the window subscribes again (`resubscribeSession`, holding the view
  it shows until the session of the new prefix answers its own), so the table is never the empty one of a session no
  sync built; that session syncs as it opens for a repository synced before. With Auto-sync on Focus off a focus
  starts no git process, and the next sync finds the prefix the same way. A Sync the user pressed that comes back so
  for a repository never synced (the Ready to Sync card, while the window kept the focus) is asked again once the window
  holds the session of the new prefix, Sync busy throughout: that session serves it, as no sync of an opened repository
  does. A prefix written in Settings → Repository subscribes the window again the same way (`repository_config_changed`).
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`syncs the session of a prefix set in a terminal once the focus finds it`, `syncs under a prefix set in a terminal behind the window's back when Sync of the Ready to Sync card is pressed`)
  [@test] ../../tests/e2e/settings/repository-settings.spec.ts (`should open and sync the session of a prefix written here, for a repository synced before`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_run_under_a_prefix_the_config_left_builds_nothing`)
  [@test] ../../crates/test-server/src/session_routes_test.rs (`a_prefix_set_in_a_terminal_refuses_the_run_of_the_session_under_the_old_one`)
  [@test] ../../src/model/store.test.ts (`subscribes again when the git config names another prefix than the session's, quietly`, `asks the user's own sync of a repository never synced again once the window holds the session of the new prefix, Sync busy throughout`)
- The main window tells the session of its repository when it gains and loses the focus (`set_window_focus`, the last
  state of a quick succession, 100 ms); the session decides whether a gained one syncs, by the setting as Rust's
  writer has it (`SessionHost::syncs_on_focus`), and the page checks nothing. A focus of a session no window shows
  syncs nothing and starts no git process. The answer of the check is recorded as a sync's: `unchanged` says "Checked
  just now", a run it started counts as the sync of the repository. ⌘R during a sync asks the session for the same
  check once after it (`sync` for a `focus`): a commit elsewhere during a long sync is synced, not lost.
  [@test] ../../src/model/store.test.ts (`tells the session the focus of the window, and marks the check of a gained one that found nothing changed`, `records the sync a focus started, Sync busy from its answer to the run's last patch`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_session_without_subscribers_runs_nothing_and_goes_away`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`tells the session the focus, which syncs nothing once Auto-sync on Focus is off`)
  [@test] ../../tests/e2e/repository/header.spec.ts (`should not move while a sync runs`)
- File → Auto-sync on Focus of the native menu is checked as the setting is: at startup as the backend's writer loaded
  `settings.json` (`AppSettings::auto_sync_on_focus`, on by default), then by the main window for every value of the
  setting, whichever window changed it (`set_auto_sync_menu_checked`); checking the item sets the setting
  (`auto_sync_on_focus_toggled`, sent to the main window). The switch of Settings → General is the same setting: its
  change reaches the main window, which checks the item.
  [@test] ../../src-tauri/src/menu_test.rs (`auto_sync_on_focus_is_checked_as_the_stored_setting`, `without_a_stored_setting_auto_sync_on_focus_is_checked`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it`)
- A sync while git is in the middle of a rebase, merge, cherry-pick or revert, or with HEAD detached, is refused before
  it changes anything (`SyncDeferred`, [Git Operations](../../docs/design/git-operations.md#virtual-branch-sync)): a
  half-done history would archive the branches it lacks. It is no failure: no error alert, no red Sync, the branches as
  the last sync left them. A sync the user asked for (Sync, ⌘R, the Ready to Sync card) says so in an `info` toast, "Not
  synced: a rebase is in progress" / "The branches sync once git finished it" ("Not synced: HEAD is detached" / "Check
  out a branch to sync its commits"); a sync the app started (on open, on focus, after a change) waits without a toast.
  The view of the session says why it waits (`Deferred`) until the next sync starts: a page without rows (a repository
  synced before, opened during a rebase) shows it as an `info` alert in the body, in the words of the toast, and the
  focus after git finished syncs.
  [@test] ../../crates/sync-core/src/sync_test.rs (`sync_waits_while_a_rebase_is_in_progress_or_head_is_detached`)
  [@test] ../../crates/bd-cli/src/sync_test.rs (`waits_while_a_rebase_is_in_progress`)
  [@test] ../../src/model/store.test.ts (`is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says in an info toast that a sync waits for git, and leaves the branches as they were`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`syncs a repository synced before when the window opens it`)
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says in the body that a sync waits for git while there is no table, and syncs once git finished`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_failed_sync_keeps_the_table`)
- A switch to another repository forgets the branches of the previous one at once, also while its sync runs: the
  window ends its subscription to the session of that repository and subscribes to the session of the next, so a late
  patch of the old one never shows under the name of another repository, and Sync of the next is not busy for it.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`forgets the branches of the previous repository on a switch`)
  [@test] ../../src/model/store.test.ts (`forgets the table of the session it leaves: a late patch of it reaches nothing`, `leaves a request of the session the window left to it: Sync of the next one is not busy, nothing of it is recorded`)

### Repository problems

- Browse opens the folder dialog of the system (`browse_repository` of app-api, `DialogHost`): the folder picked is
  checked as a repository, and the dialog canceled changes nothing. In an E2E test the dialog of the test server picks
  the folder of its fixture (`setDialogPick`, `POST /repositories/{id}/dialog-pick`), else the test repository itself.
  [@test] ../../tests/e2e/welcome-card/welcome-card.spec.ts (`keeps Step 2 visible after selecting a repository without branch prefix`)
  [@test] ../../tests/e2e/repository/browse.spec.ts (`should successfully select valid git repository`)
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
- A saved repository that no longer exists fails the subscription of its session with the error of git ("Repository
  not accessible: …"), which the page shows as the problem of the path: the error alert, the selector marked invalid,
  Sync disabled with it as the reason, until another one is selected, or a focus subscribes again and finds it back.
  [@test] ../../crates/sync-core/src/branch_prefix_test.rs (`a_directory_that_is_gone_is_not_accessible`)
  [@test] ../../tests/e2e/repository/saved-path-validation.spec.ts (`should display error for non-existent repository path`, `should clear error when valid repository is selected`)
  [@test] ../../src/model/store.test.ts (`subscribes again on a gained focus after its subscription failed: the user may have fixed it meanwhile`)

### Grouping

- While there is no branch, the unassigned commits card says under its title: "Start a commit subject with `(name)` or
  an issue key like `ABC-123` to group it into a branch, or select commits and move them to one."
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`explains the grouping in the unassigned commits while there is no branch`)
- With no branch and no unassigned commit after a sync, "No active branches" names the baseline the sync compared with,
  "No commits ahead of `origin/master`" (a branch that tracks another remote branch is compared with that one), says to
  commit to the branch with a prefix, and where coding agents work (below).
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`names the baseline and where coding agents work in the empty state, and installs the Agent CLI there`)
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should show empty state alert when only archived branches exist`)
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
- On the way to the first branches, the Ready to Sync card and "No active branches" say where coding agents work: the
  user never picks a worktree, an agent leases one through bd ([bd lease](../cli/lease.spec.md)). By
  `branchdeck.agentLease` of the repository (on by default): "You work in this checkout. With the Agent CLI, coding
  agents such as Claude Code lease worktrees of their own and land each commit on your branch." with "Install Agent CLI"
  (the install and the toast of Help → Install Agent CLI…), and once it is installed "You work in this checkout. Coding
  agents such as Claude Code lease worktrees of their own through bd and land each commit on your branch." with "Open
  Agent Settings"; with the rule off, "With the Agent CLI, coding agents such as Claude Code work in this checkout beside
  you." (installed: "Coding agents such as Claude Code work in this checkout beside you, through bd."). The status of the
  Agent CLI is read when the line shows, on each focus and after an install.
  [@test] ../../tests/e2e/onboarding/onboarding.spec.ts (`says in Ready to Sync where coding agents work, installs the Agent CLI there and opens Agent Settings`, `names the baseline and where coding agents work in the empty state, and installs the Agent CLI there`)
  [@test] ../../src/lib/agents-hint.test.ts (`says where agents work by the rule of the repository, and that the Agent CLI lets them`)
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
