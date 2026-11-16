## MODIFIED Requirements

### Requirement: Restart and cancel: A restart runs on what its admission holds

A restart MUST be admitted with what its plan holds (its robot, what it lands, the ref of its push, the base of the
pull request it retargets first), also where the session refuses it as it is asked, so a refused restart may wait
behind an operation of its branch before its refusal; once admitted it runs on what it holds as every plan does
(Operations: A plan that needs more once admitted holds it once). A Safe Push of its branch waits for it: Patronus never
runs two robots of the branch.

#### Scenario: A refusal lifted while the restart waits
- **WHEN** a restart is asked while Patronus has not told why the Safe Push failed, and Patronus tells it while the restart waits for a sync
- **THEN** the restart runs once the sync ends, and a Safe Push of its branch asked while it pushes and restarts waits for it
  [@test] crates/operations/src/robot_test.rs (`a_restart_admitted_while_the_session_refuses_it_holds_what_it_lands_so_a_safe_push_of_its_branch_waits_for_it`)

#### Scenario: What a restart holds
- **WHEN** a restart pushes its branch first, its pull request into a virtual branch
- **THEN** it holds its robot, what it lands, the ref of its push and the base of the pull request it retargets
  [@test] crates/operations/src/robot_test.rs (`a_restart_holds_its_robot_what_it_lands_its_push_and_the_base_it_retargets`)

## ADDED Requirements

### Requirement: Operations: An admission holds what the plan as the session has it needs

The admission of a start, a push, a pull request intent, a restart and a close MUST hold what their plan as the
session has it needs, by one builder of each family; without a plan, what the intent names (the branch a start lands,
the head a pull request intent pushes, the pull request a close closes). A push MUST hold besides the base of the open
pull request of each branch of its stack and, restarting the Safe Push of its branch, its listed robots and the branch.

#### Scenario: The remote effects held alike
- **WHEN** a plan of a start, a push, a restart or a link retargets a pull request, links a stack, cancels a robot or restarts one
- **THEN** its admission holds the base of the pull request, the bases of the open pull requests of the chain, the robot it cancels, the robot it restarts and what that lands
  [@test] crates/operations/src/cover_test.rs (`the_admission_of_every_family_holds_what_its_effects_on_the_remote_hold`)

#### Scenario: What a push holds besides
- **WHEN** a push of a branch of a stack is admitted, restarting the Safe Push of its branch or not
- **THEN** it holds the bases of the open pull requests of the stack, below and above the branch, and where it restarts, the robots listed of the branch and the branch, nothing of a branch outside the stack, nothing more of an unapplied branch
  [@test] crates/operations/src/push_test.rs (`a_push_is_admitted_with_the_bases_of_its_stack_and_the_robots_of_the_branch_it_restarts`)

### Requirement: Operations: A plan that needs more once admitted holds it once

A plan decided once admitted (each plan of a start) MUST run only on what its operation holds, HEAD aside. Where it
needs more, the operation MUST hold it first, after the operations that hold it, and be decided again on what they
left (a push on the robots asked again); a second time it MUST be refused `plan_changed` before its first effect. A
start runs over the host of the route its plan decided.

#### Scenario: A push that sends a branch more
- **WHEN** a push of a branch with its ancestors, admitted with the ref of the branch, waits behind a rewrite while the remote branch of its parent goes, and a push of the parent runs
- **THEN** it holds the ref of the parent after that push, its step `queued` behind it, pushes nothing meanwhile, and sends both once it ended
  [@test] crates/operations/src/cover_test.rs (`a_push_whose_plan_once_admitted_sends_a_branch_more_holds_it_after_the_operation_that_holds_it`)

#### Scenario: A push decided again on the robots as they are then
- **WHEN** a push waited for a start of a branch it sends once its plan needed that branch, and Patronus lists the Safe Push that start started
- **THEN** it is decided on that robot, and the branch its Safe Push holds is not pushed
  [@test] crates/operations/src/cover_test.rs (`a_push_that_waited_for_what_it_needs_more_is_decided_on_the_robots_as_patronus_has_them_then`)

#### Scenario: A start over the host of its route
- **WHEN** a start of a branch the last sync had not lands by a robot of Patronus once admitted, off Space
- **THEN** it is refused `not_on_space` before anything is pushed
  [@test] crates/operations/src/cover_test.rs (`a_start_whose_plan_once_admitted_lands_by_a_robot_runs_over_the_host_of_patronus`)

#### Scenario: A plan that needs more a second time
- **WHEN** a push that held the ref of the parent once its plan sent it waits behind the push of the parent while the remote branch of the grandparent goes
- **THEN** it is refused `plan_changed`, and nothing is pushed
  [@test] crates/operations/src/cover_test.rs (`a_push_whose_plan_needs_more_a_second_time_is_refused_before_its_first_effect`)
  [@test] crates/operations/src/cover_test.rs (`a_plan_that_needs_more_holds_it_once_and_is_refused_the_second_time`)

#### Scenario: A start, a pull request and a close that need more
- **WHEN** a start of a branch the last sync had not, the creation of a pull request whose push sends the parent once its remote branch went, or the closes of a landing the session told only while they were admitted are admitted, and an operation holding what their plan needs once admitted runs
- **THEN** each holds it after that operation, its step `queued` behind it, and runs its effects once it ended
  [@test] crates/operations/src/cover_test.rs (`a_start_whose_plan_once_admitted_reserves_more_holds_it_after_the_operation_that_holds_it`)
  [@test] crates/operations/src/cover_test.rs (`a_pull_request_whose_plan_once_admitted_pushes_more_holds_it_after_the_operation_that_holds_it`)
  [@test] crates/operations/src/cover_test.rs (`a_close_whose_plan_once_admitted_closes_more_holds_it_after_the_operation_that_holds_it`)
