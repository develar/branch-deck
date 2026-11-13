# Proposal

## Why

`brd status` of the monorepo said of `air-ide-ahp` "landed: nothing of it is left to land", row 1, `robot_finished`,
although the branch had two new commits the remote and master lack: a Safe Push of the same name had merged earlier
commits that day. The view knew better (no `landed_by`: by content the robot took none of the commits the branch has),
but the slot of the robot (`domain_policy::next_step::slot`) took any finished robot for one of the branch as it is
while no remote status said what the remote has: `is_outdated` answered "not outdated" without one. A merged robot in
that slot is "landed whole" (`landed_whole`), so the next step, the Status cell, the node and the actions said landed,
and the Safe Push of the new commits was disabled. `brd status` reads no remote status at all; the app has none before
the first one of a session, and the slot also skipped the check while Patronus was pending or the sync had failed
(`offered`), so a merged robot of an earlier state counted as landing the branch there too.

## What Changes

- A merged robot of the subject is of the subject as it is only by the hashes (the subject at its commit, or its remote
  branch there with what the subject has) or by what the view says it landed by content (`own_robot_landed`); a subject
  at another tip without a remote status is new work under the same name (`is_outdated`).
- A robot of an earlier state gives way whether or not a start is offered now: no slot (`Slot::None`) where nothing is
  offered (Patronus pending, a failed sync), so nothing of the subject counts as landed by it there either.
- A failed or canceled robot keeps its slot until a remote status tells that the branch changed since, as before.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/next-step`: a merged robot lands only the commits it took, also before a remote status, while Patronus is
  pending and while the sync failed; the step says what landed a subject before the view only of a robot that merged it
  as it is.
- `branch/patronus`: a merged robot is of an earlier state whenever nothing proves it took the branch as it is, a
  remote status not read yet included.

## Impact

- `crates/domain-policy/src/next_step.rs` (`slot`, `is_outdated`, `landed_whole`); every consumer of the slot follows:
  the step, the Status cell (`row_status`), the node (`landing_node`), the actions (`actions::landed_whole`), the app
  and `brd status` alike.
- A case of the scenario table (`row-11-merged-robot-of-earlier-commits`), tests of the rule, of the Status cell and of
  the derivation of the session.
