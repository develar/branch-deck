## MODIFIED Requirements

### Requirement: View: subscribe_view is the mirror of the repository

`x-air-app/subscribe {instance, params}` MUST answer the mirror `repositoryPath` and the view of the session with its
`version`, which the page compares with the versions the commands answer. The views of one repository share that
mirror.

#### Scenario: The main view of a repository
- **WHEN** air-ide subscribes `main:1` to a repository
- **THEN** the answer names the path of the repository and holds the view
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)
  [@test] crates/mcp-host/src/mirror_test.rs (`the_state_is_the_view_with_its_version`)

### Requirement: View: Each patch of the session is one change of the mirror

Each `ViewPatch` MUST become one `x-air-app/viewChanged`. A keyed part sets each changed key and removes each removed
key, every other part is replaced whole, and the part `version` is set last to the version after the patch.

#### Scenario: A renamed branch
- **WHEN** a sync renames the branch `a` to `b`
- **THEN** one change sets `branches/b`, removes `branches/a`, replaces `order` and sets `version`
  [@test] crates/mcp-host/src/mirror_test.rs (`a_renamed_branch_sets_the_new_key_removes_the_old_and_replaces_the_order`)

#### Scenario: The news
- **WHEN** a patch carries news
- **THEN** the change replaces the part `news` with the last news of the session, so a resync tells none twice

## ADDED Requirements

### Requirement: Events: An emit of the page is an event of the views

`bd_invoke` of `plugin:event|emit {event, payload, target}` MUST send `x-air-app/event` of the event to every view, or
with the instance of the window `target` names.

#### Scenario: An event for the main window
- **WHEN** a review tab emits an event to the main window
- **THEN** brd sends `x-air-app/event` with the instance of the main view
  [@test] crates/test-server/src/mcp_corpus_test.rs (`an_emit_of_the_page_is_an_event_of_the_views`)
