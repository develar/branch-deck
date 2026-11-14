## ADDED Requirements

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

### Requirement: Before the first sync: The mark of a sync names its prefix

The mark of a sync in the worktree MUST name the prefix of each sync that wrote it (`branch-deck/synced`, one prefix
per line), and a session MUST be synced before by the mark only when it names the session's prefix: `brd sync --prefix
X` marks the session of `X` alone. The `lastSyncTime` of the app is of the path.

#### Scenario: A prefix changed after a sync
- **GIVEN** a repository the app synced under one prefix
- **WHEN** its prefix is another one, never synced
- **THEN** the session of the new prefix syncs as it opens, as `lastSyncTime` is of the path
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`syncs the session of a prefix set in a terminal once the focus finds it`)

#### Scenario: brd synced under another prefix
- **GIVEN** a worktree only `brd sync --prefix other` synced
- **WHEN** a window opens it under the prefix `test`
- **THEN** the session of `test` does not sync of itself: the mark names `other` alone
- **AND** a sync under `test` adds its own line, so both sessions sync of themselves
  [@test] crates/sync-core/src/synced_mark_test.rs (`the_mark_names_the_prefix_each_sync_built_under`)

## MODIFIED Requirements

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
