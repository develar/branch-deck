## ADDED Requirements

### Requirement: Hosts: The page in the IDE offers no folder dialog

In `brd mcp`, the dialog host MUST answer that it has no folder dialog (`has_folder_dialog`), which `bd_bootstrap` gives
the page as `titleBar.folderDialog: false`. The page then offers no Open Repository… beside the selector of the
repositories or in the command palette, and no Add Companion… on Settings → Landing; it only words the fact.

#### Scenario: The main view in the IDE
- **WHEN** the IDE shows the main view
- **THEN** its selector lists the roots without the folder button beside it, and the command palette offers no Open
  Repository…
  [@test] crates/mcp-host/src/hosts_test.rs (`the_hosts_without_a_place_in_the_ide_do_nothing`)
  [@test] tests/e2e/air/air-page.spec.ts (`the main view in the IDE offers no Open Repository, as the IDE has no folder dialog: the roots of the project are the repositories of its selector`)
  [@test] src/lib/command-palette.test.ts (`offers no Open Repository… where the host has no folder dialog`)

### Requirement: Windows: The notification of the page names its news

The page MUST name the news its own notification tells (`extra: {repositoryPath, news}`). brd leaves it out once it
told that news, and else gives it the kind and the action of the news, so the IDE shows one notification of a failure,
and it opens the branch.

#### Scenario: The notification of the page
- **WHEN** the page notifies the failure as its frame loses the focus
- **THEN** the IDE shows no second notification of a failure brd told, and the notification of one brd did not tell
  opens the branch too; with "Safe Push failures" off brd tells none
  [@test] crates/mcp-host/src/notices_test.rs (`brd_tells_a_failure_while_the_main_view_is_hidden_and_the_notification_of_the_page_takes_its_action_while_shown`)
  [@test] src/platform/air/shim.test.ts (`The plugins: a link, the clipboard and the log go to the host, the badge, the progress, a notification and the close of the window are bd_invoke of their plugin calls, the size does nothing`)

### Requirement: Windows: Show in Branch Deck reveals the branch in the main view

The tool `reveal_branch` MUST ask `openView` of `main` and tell the instance the IDE answers the event `reveal-branch`
with the repository and the branch, which the main view reveals in the repository it shows.

#### Scenario: Show in Branch Deck
- **WHEN** the IDE runs the action of the notification
- **THEN** brd asks `openView` of `main`, and tells the instance the IDE answered `reveal-branch`: the main view reveals
  the branch as Go to of the command palette does, in the repository it shows
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] tests/e2e/air/air-page.spec.ts (`a Safe Push that fails while the IDE shows another tab is a notification of the IDE, whose action shows the branch in the tool window`)

## MODIFIED Requirements

### Requirement: Windows: A notification of the OS is a notification of the IDE

A notification of the desktop app MUST be `x-air-app/notify`. The failure of a robot, a news of the session, brd MUST
tell itself while the main view is not shown (its last `viewHidden` after its `viewShown`) and "Safe Push failures" of
Settings is on, with the action Show in Branch Deck: `run` of the tool `reveal_branch {repositoryPath, branchName}` of
the branch the robot tested (the root of a whole stack). While the main view shows, its toast tells it.

#### Scenario: A failed Safe Push
- **WHEN** a Safe Push fails while the IDE shows another tab
- **THEN** the IDE shows a notification whose action opens the branch
  [@test] tests/e2e/air/air-page.spec.ts (`a Safe Push that fails while the IDE shows another tab is a notification of the IDE, whose action shows the branch in the tool window`)
  [@test] crates/mcp-host/src/server_test.rs (`a_failed_robot_of_a_patch_is_a_notification_of_the_ide_after_the_change_of_the_mirror`)
  [@test] crates/mcp-host/src/notices_test.rs (`a_failed_robot_is_a_notification_whose_action_reveals_its_branch`)

### Requirement: Tools: Every tool of brd mcp is only for the app

`tools/list` MUST list `bd_invoke`, `bd_bootstrap`, the tool of each action of the app file, `sync` and
`open_command_palette`, and `reveal_branch`, the action of a notification of brd. Each has
`_meta.ui.visibility: ["app"]`, so no agent sees it.

#### Scenario: The list
- **WHEN** air-ide sends `tools/list`
- **THEN** it gets the five tools, each only for the app
  [@test] crates/mcp-host/src/server_test.rs (`every_tool_is_only_for_the_app_and_each_action_of_the_app_file_has_its_tool`)
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Tools: bd_bootstrap gives the settings, the init data and a title bar of the IDE

`bd_bootstrap` MUST give the settings of the window of the instance as `{values, version}`, the warm frame and the
roots of the main view, the data of the open of any other view, and the title bar `native`, without updates and without
a folder dialog (`folderDialog: false`).

#### Scenario: The settings of the desktop app
- **WHEN** the settings window bootstraps after the desktop app saved a setting
- **THEN** `settings.values` holds the setting
  [@test] crates/mcp-host/src/process_test.rs (`brd_mcp_serves_the_settings_of_the_app_until_its_input_ends`)
