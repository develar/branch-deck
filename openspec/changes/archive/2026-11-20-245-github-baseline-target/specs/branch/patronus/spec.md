## RENAMED Requirements

- FROM: `### Requirement: Starting a Safe Push or a Dry Run: The target is the branch of origin the baseline tracks`
- TO: `### Requirement: Starting a Safe Push or a Dry Run: The target is the branch the baseline tracks on origin or on the GitHub repository`

## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: The target is the branch the baseline tracks on origin or on the GitHub repository

The target branch MUST be the branch the baseline tracks on `origin` (`origin/master` → `master`) or on the remote that
fetches from the GitHub repository of the branches, where their pull requests go and which holds the branches of origin
under their names (`github/master` → `master`); a local baseline or one on another remote (a fork, whose branch origin
may lack) MUST NOT be started. The sync finds it (`SyncReport::target_branch`), a start takes it.

#### Scenario: The target of a baseline
- **WHEN** the baseline is `origin/master`, a local branch, or a branch of another remote (a fork)
- **THEN** the target is `master`, and none for the local baseline and the other remote, which can't be started
  [@test] crates/sync-core/src/patronus_test.rs (`the_target_is_the_branch_of_origin_or_of_the_github_repository_of_the_branches`)
  [@test] crates/sync-core/src/patronus_test.rs (`an_action_reads_the_target_with_the_urls_of_the_remotes`)

#### Scenario: A baseline on the remote of the GitHub repository of the branches
- **GIVEN** origin on Space, whose committed settings name the GitHub repository of the branches, and a remote `github`
  that fetches from it
- **WHEN** the baseline is `github/master`
- **THEN** the target is `master`: a Safe Push lands into it, a pull request goes into it
  [@test] crates/sync-core/src/sync_test.rs (`a_baseline_on_the_remote_of_the_github_repository_of_the_branches_has_its_target`)
  [@test] crates/operations/src/land_test.rs (`a_start_takes_the_target_the_sync_found_on_the_remote_of_the_github_repository_of_the_branches_too`)

#### Scenario: A start takes the target of the sync
- **WHEN** a start, a pull request or a review request of the app begins
- **THEN** it takes the target of the table of its session (`BranchTable::target_branch`), no git of its own; a baseline
  without one refuses it, naming the baseline
  [@test] crates/operations/src/land_test.rs (`a_start_takes_the_target_the_sync_found_on_the_remote_of_the_github_repository_of_the_branches_too`)
