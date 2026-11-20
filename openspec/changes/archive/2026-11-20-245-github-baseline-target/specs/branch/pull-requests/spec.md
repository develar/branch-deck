## MODIFIED Requirements

### Requirement: Creating a pull request: The base is the baseline for every branch

The base MUST be the baseline as the remote names it (`master`) for every branch, stacked or not
([Stacked Branches](../stacked-branches/spec.md#pull-requests)): a pull request holds what the Safe Push of its branch
lands.

#### Scenario: The pull requests of a stack
- **WHEN** the pull requests of a stack are created
- **THEN** each goes into `master`
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)
  [@test] src/lib/pull-request-dialog.test.ts (`creates the pull request of a stacked branch into the baseline, with the commits of the branches it is built on`)

#### Scenario: A baseline on the remote of the GitHub repository of the branches
- **WHEN** the baseline is `github/master`, of the remote that fetches from the GitHub repository of the branches
- **THEN** the pull request goes into `master`, as for `origin/master`
  [@test] crates/operations/src/land_test.rs (`a_start_takes_the_target_the_sync_found_on_the_remote_of_the_github_repository_of_the_branches_too`)
