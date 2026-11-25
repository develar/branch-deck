## MODIFIED Requirements

### Requirement: `brd safe-push`: A rewrite while the push sends skips the start as `plan_changed`

A rewrite of another session while the push sends, after the lock, MUST skip the start as `plan_changed`, naming what
was pushed before it, with `brd safe-push <branch>` as the next step; the push sends the commits it froze. A rebuild of
the branch onto another base with the same commits (a sync after `brd update` or Update Project) MUST NOT skip it: the
start lands the pushed commits.

#### Scenario: Another session rewrites while the push sends
- **GIVEN** another session rewrites a ref the plan moves to other commits while the push sends, after the lock
- **WHEN** the push ends
- **THEN** the push has sent the commits it froze
- **AND** the start is skipped as `plan_changed`, naming what was pushed before it, with `brd safe-push <branch>` as
  the next step
  [@test] crates/operations/src/land_execute_test.rs (`a_rewrite_while_the_push_sends_skips_the_start_and_the_outcome_tells_what_was_done`)

#### Scenario: Another session rebuilds onto a new baseline while the push sends
- **GIVEN** another session rebuilds the branch onto the new baseline with the same commits while the push sends
- **WHEN** the push ends
- **THEN** the start goes on, its robot on the pushed commits
  [@test] crates/operations/src/land_execute_test.rs (`a_rebuild_onto_a_new_baseline_while_the_push_sends_starts_on_the_pushed_commits`)
