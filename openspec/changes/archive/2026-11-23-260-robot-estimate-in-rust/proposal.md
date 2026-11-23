# Proposal

## Why

The estimate of a robot's duration was a rule of the page (`estimateRobotDuration` of `src/lib/robot-eta.ts`), fed by
the robots the window listed and the history the main window wrote into `settings.json` (`robotHistory`). Each process
and window had other samples, so the IDE said "~44m" where the app said "~40m" (#260, "the robot estimate").

## What Changes

- The rule is `domain_policy::robot_estimate`: the median of the successful robots of the merge profile and the push
  mode that started in the 30 days before the robot (of its own start, not of a clock), the same target when three or
  more, else roughly of the push mode.
- The history of the finished robots is the session's, in the object cache of the repository (`robot_history`), which
  every process of the user writes, merged by robot id; the first session moves `robotHistory` of the settings there
  and removes the key.
- The view says the estimate of each running robot (`RepositoryView.robotEstimates`); the page keeps the clock and the
  words (`getRobotTiming`, `withCheckProgress`), and `src/stores/robot-history.ts` goes.

Capabilities: `branch/patronus`.
