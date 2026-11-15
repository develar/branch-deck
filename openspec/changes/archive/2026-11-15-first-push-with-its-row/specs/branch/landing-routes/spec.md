## MODIFIED Requirements

### Requirement: What a sync reads for it: The sync sends the facts of the route in `BranchesGrouped`

A sync MUST send the route's facts in `BranchesGrouped` (`landing`): the setting, whether `origin` pushes to Space, the
github.com repository the remote of the baseline pushes to, the branch of the baseline there (`master` of
`origin/master`) and its remote (`origin`; none of both for a local baseline). The session MUST keep them in the header
of its view.

#### Scenario: The facts a sync sends
- **WHEN** a sync groups the branches
- **THEN** `BranchesGrouped` carries in `landing` the setting, whether `origin` pushes to Space, the github.com repository the remote of the baseline pushes to, the branch of the baseline there and its remote
- **AND** a local baseline has neither the branch nor the remote
  [@test] crates/sync-core/src/repo_facts_test.rs (`the_landing_remote_is_read_with_the_facts_of_a_sync`)

#### Scenario: The session keeps them in the header of its view
- **WHEN** the session takes `BranchesGrouped`
- **THEN** the header of its view holds the facts of the route
  [@test] crates/repository-model/src/session_test.rs (`a_push_route_reads_the_rules_of_the_target`)

#### Scenario: The session answers what git tells of the services
- **WHEN** the session takes `BranchesGrouped` of a repository Patronus doesn't know, or of branches on no repository of GitHub, and no answer of the robots, or of the pull requests, stands
- **THEN** it answers that Patronus has no robots of it, that the pull requests are not on GitHub, with no request, in the patch with the rows the grouping places
- **AND** a repository Patronus knows, on GitHub, waits for their answers
  [@test] crates/repository-model/src/session_test.rs (`the_facts_git_tells_of_the_remote_come_with_the_rows_of_a_sync`)
