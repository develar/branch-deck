# The backend refuses a park of a branch a robot holds

## Why

The audit after the review of 595c16e9: the menus disable Park Branch… while a robot lands the branch
(`ActionBlocker::ParkHeld`), but the plan of a rewrite has no blockers and the run parked it: a window of a state
before or another client parked a branch the robot lands anyway, and the table said parked while it landed.

## What Changes

- `repository_model::derive::park_held`: the hold of a robot on the branch now (`live_hold`), but a robot that landed
  it, as the menus decide it.
- `prepare` and the run of a park refuse it by it (`operations::rewrite::park_refusal`, `robot_running`): "Nothing was
  parked: A Safe Push of JIRA-456 is running (…): it lands JIRA-456 whatever a park says".
- Capability `branch/parked-branches`.
