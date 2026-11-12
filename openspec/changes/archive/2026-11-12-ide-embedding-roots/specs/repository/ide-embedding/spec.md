## ADDED Requirements

### Requirement: Repositories: brd remembers the root picked last for the roots of a project

A pick of the page (its write of `recentProjects` with another root first) MUST be remembered by the set of the roots,
in `ideRootPicks` of `repository-state.json`; the next start of a project with the same roots, in any order, shows it.
Rust alone reads and writes it, never the page.

#### Scenario: The next start of the project
- **GIVEN** the user picked the second root of a project
- **WHEN** the project opens again
- **THEN** the main view shows the second root
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_main_view_shows_the_root_picked_last_for_the_roots_of_its_project`)
  [@test] tests/e2e/air/air-page.spec.ts (`the selector of the main view lists the Git roots of the project, and the root picked shows again at the next start of the project`)
  [@test] crates/app-settings/src/ide_roots_test.rs (`the_root_picked_last_is_kept_by_the_set_of_the_roots_in_the_repository_state`)

### Requirement: Repositories: A pick in the IDE keeps the recent projects of the desktop app

What the page writes of a root (a collapsed card, its issue links) MUST go into the recent projects of the settings in
the place of that root, a root they lack at their end; their order, which selects the repository of the desktop app,
stays.

#### Scenario: A pick in the IDE
- **WHEN** the user picks another root in the IDE while the desktop app shows a repository
- **THEN** the desktop app keeps the repository it shows
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_main_view_shows_the_root_picked_last_for_the_roots_of_its_project`)
  [@test] crates/mcp-host/src/projects_test.rs (`the_views_see_the_roots_as_the_recent_projects_and_a_write_keeps_the_order_of_the_desktop_app`)

## MODIFIED Requirements

### Requirement: Repositories: The roots of the project are the repositories of brd mcp

`brd mcp` MUST read the repositories from `roots/list`, at its start and on `notifications/roots/list_changed`. The
views see the Git roots as their recent projects (`recentProjects`), the root the main view shows first, and
`bd_bootstrap` names that root as `initData.repositoryPath`: the page selects it and its selector lists the roots.

#### Scenario: A project with two roots
- **WHEN** the project has two Git roots
- **THEN** the main view shows the first one until the user picks the other, and its selector lists both
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_main_view_shows_the_root_picked_last_for_the_roots_of_its_project`)
  [@test] tests/e2e/air/air-page.spec.ts (`the selector of the main view lists the Git roots of the project, and the root picked shows again at the next start of the project`)
  [@test] crates/mcp-host/src/projects_test.rs (`the_main_view_shows_the_root_picked_last_else_the_one_it_shows_else_the_first`)

### Requirement: Repositories: A change of the roots tells the main view

When `roots/list` answers other Git roots than before, brd MUST send the main view the event `roots-changed` with them,
and the views that took the settings the recent projects of the roots, as a change of `recentProjects`.

#### Scenario: A root added to the project
- **WHEN** the project gets a second Git root
- **THEN** the main view hears `roots-changed` with both roots, and its selector lists both
  [@test] crates/test-server/src/mcp_corpus_test.rs (`a_root_added_to_the_project_tells_the_main_view_and_the_recent_projects_of_the_views`)
