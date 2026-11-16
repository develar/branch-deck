## ADDED Requirements

### Requirement: Restart and cancel: A restart runs on what its admission holds

A restart MUST be admitted with what its plan holds, its robot, what it lands and the ref of its push, also where the
session refuses it as it is asked, and MUST run only on that: a plan that holds more once admitted is refused
`plan_changed` before any push or restart. A Safe Push of its branch waits for it: Patronus never runs two robots of
the branch.

#### Scenario: A refusal lifted while the restart waits
- **WHEN** a restart is asked while Patronus has not told why the Safe Push failed, and Patronus tells it while the restart waits for a sync
- **THEN** the restart runs once the sync ends, and a Safe Push of its branch asked while it pushes and restarts waits for it
  [@test] crates/operations/src/robot_test.rs (`a_restart_admitted_while_the_session_refuses_it_holds_what_it_lands_so_a_safe_push_of_its_branch_waits_for_it`)

#### Scenario: A plan that holds more once admitted
- **WHEN** the plan of a restart decided once admitted lands or pushes what its admission holds not
- **THEN** the restart is refused `plan_changed` "What the restart of robot … pushes or lands changed while it waited: nothing was pushed or restarted"
  [@test] crates/operations/src/robot_test.rs (`a_restart_whose_plan_holds_more_than_its_admission_is_refused_before_its_first_effect`)
