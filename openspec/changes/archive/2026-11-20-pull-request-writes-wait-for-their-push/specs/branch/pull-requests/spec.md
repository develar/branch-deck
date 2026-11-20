## ADDED Requirements

### Requirement: Creating a pull request: A write whose push can't be made is disabled with why

"Create Pull Request…" and "Request Review…" MUST be disabled with the reason while the push they make first can't be
made, by the blocker the engine refuses them by (`BranchPush::action_blocker`): the sync did not build the branch or a
branch of its chain, a running Safe Push holds it. A push that drops remote commits is the user's decision: the write
stays offered and its dialog says it at the submit.

#### Scenario: The branch is not built
- **WHEN** the sync did not build a branch the remote lacks, or a running Safe Push holds its push
- **THEN** "Create Pull Request…" and "Request Review…" are disabled with that reason
  [@test] crates/domain-policy/src/actions_test.rs (`a_pull_request_write_waits_for_the_push_it_makes_first_as_the_engine_refuses_it`)

#### Scenario: The push drops remote commits
- **WHEN** the push of the creation would drop commits of the remote branch
- **THEN** "Create Pull Request…" stays enabled, and its dialog says it at Create
  [@test] crates/domain-policy/src/actions_test.rs (`a_pull_request_write_waits_for_the_push_it_makes_first_as_the_engine_refuses_it`)
