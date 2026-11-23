## MODIFIED Requirements

### Requirement: Tools: bd_bootstrap gives what the page needs before its main

`bd_bootstrap {view}` MUST answer the settings, the warm frame, the init data of the view, the view of the title bar
and the home directory of `brd mcp` (`homeDir`). The page takes them where the desktop app takes its init script.

#### Scenario: The main view
- **WHEN** the page of `main` calls `bd_bootstrap`
- **THEN** the result has `settings`, `warmFrame`, `initData`, `titleBar` and `homeDir`
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_app_link_corpus_runs`)

### Requirement: Repositories: The roots of the project are the repositories of brd mcp

`brd mcp` MUST read the repositories from `roots/list`, at its start and on `notifications/roots/list_changed`. The
views see the Git roots as their recent projects (`recentProjects`), the root the main view shows first, and
`bd_bootstrap` names that root as `initData.repositoryPath`: the page selects it and its selector lists the roots. The
path of a root MUST have no trailing separator but the root of a file system, as the desktop app spells it.

#### Scenario: A project with two roots
- **WHEN** the project has two Git roots
- **THEN** the main view shows the first one until the user picks the other, and its selector lists both
  [@test] crates/test-server/src/mcp_corpus_test.rs (`the_main_view_shows_the_root_picked_last_for_the_roots_of_its_project`)
  [@test] tests/e2e/air/air-page.spec.ts (`the selector of the main view lists the Git roots of the project, and the root picked shows again at the next start of the project`)
  [@test] crates/mcp-host/src/projects_test.rs (`the_main_view_shows_the_root_picked_last_else_the_one_it_shows_else_the_first`)

#### Scenario: The URI of a directory
- **WHEN** `roots/list` names `file:///x/repo/`, as the IDE writes the URI of a directory
- **THEN** the repository is `/x/repo`, the path whose session the desktop app opens; `file:///` stays `/`
  [@test] crates/mcp-host/src/server_test.rs (`the_path_of_a_root_has_no_trailing_separator`)

#### Scenario: A project with one root
- **WHEN** the project has one Git root
- **THEN** the main view shows its path as text, without the menu of a selector and without Open Repository
  [@test] tests/e2e/air/air-page.spec.ts (`the main view in the IDE offers no Open Repository, as the IDE has no folder dialog: the one root of the project is the text of the repository, without the menu of a selector`)

## ADDED Requirements

### Requirement: Page: The home directory of brd mcp is the home directory of the page

The shim MUST answer `homeDir` of Tauri (`plugin:path|resolve_directory` of the home) with `homeDir` of `bd_bootstrap`,
so the page shows a path under it with `~`, as the desktop app does. Another directory, or a process without a home,
is an error, and the page shows the whole path.

#### Scenario: The repository under the home directory
- **WHEN** `bd_bootstrap` names the home `/Users/ann`, and the main view shows `/Users/ann/projects/idea-1`
- **THEN** the page shows `~/projects/idea-1`
  [@test] src/platform/air/shim.test.ts (`homeDir of Tauri is the home directory of brd mcp that bd_bootstrap gave, so the page shows a path under it with ~; the IDE resolves no other directory`)
