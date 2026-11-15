# The backend refuses a restart the session offers not

## Why

The audit after the review of 595c16e9: the plan of a restart (`operations::robot::restart_planned`) took only the
refusal of the rules (`restart.refusal`) and a push that drops commits; the menus also hold a restart back while
Patronus has not told why the Safe Push failed, once it failed to tell it, while the Safe Push waits for approvals, and
offer no Push and Restart of a robot of no row (`domain_policy::actions::robot_actions`). A window of a state before or
another client could restart a robot, and push first, that would fail again on the same approvals.

## What Changes

- `repository_model::state::robot_actions_now` derives the actions of one robot as the view does (over the next steps
  of the rows as they are), without the operations of the session, which the engine admits by itself.
- `prepare` and the run of `RestartRobot` refuse a restart by those actions before any push or restart
  (`operations::robot::restart_refusal_of`): `plan_changed`, `patronus_error`, `approvals_missing`, `invalid_arguments`
  for no restart offered; a reason of the rules stays a blocker of the plan.
- Capability `branch/patronus`.
