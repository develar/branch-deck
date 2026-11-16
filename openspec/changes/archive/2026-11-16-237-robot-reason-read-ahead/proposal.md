# Proposal

## Why

The card of a failed robot showed why the robot failed a frame or more after it opened: its problems came with the
details the card asks as it opens, so 46ece10d6 put them past the actions to keep Safe Push under the pointer, and the
card read "action, then reason". The session reads the problems of each failed robot of the table ahead anyway (to name
"Needs review"), and threw the list away, keeping only the flag; the details then asked them again. Counted on the fake
Patronus of the E2E server, after the list read the problems ahead (2 requests: the failed robot's problems, the running
robot's failing check), the first open of the card of the failed robot sent 3 requests (`/robots/{id}`,
`/teamcity-checks`, `/problems` again), a reopen none; the card of a running robot 4 (the same three and
`robots/progress`), again every minute while open.

## What Changes

- The robots fact keeps the problems of severity error of each failed robot of the table it read (`RobotsView.problems`,
  beside `needsReview`), so the card shows why the robot failed in its first frame, above its actions: the reason, then
  what to do about it. `FRAME_SCHEMA` goes to 18.
- The details of a finished robot take its problems from the cache the read ahead fills (`caches.problems`, now the
  problems of every severity, so its failure and its prompt read them too): the first open of the card of a failed robot
  sends 2 requests, its robot and its checks.
- What still comes late stays past the actions, away from the edge the card hangs by: the profile, the progress, the
  checks, the problems of a running robot or of one whose problems were not read when its card opened (the reason stands
  where it stood as the card opened, so its coming moves nothing either). The checks are not read ahead: they are a
  count, the reason is in the problems, and a read ahead would ask one more request per failed robot of the table that
  no card may open.
- The E2E test holds the checks of the robot (`robotChecks`, a new held answer of the test server): the problems are in
  the card as it opens, above Safe Push, no problems are asked again, and Safe Push keeps its box as the checks come.

## Capabilities

### Modified Capabilities

- `branch/patronus`: the details of a finished robot whose problems were read ahead; why a failed robot failed shows
  as its card opens.
- `branch/branch-row`: the anatomy of the details of a robot, its reason before its actions.

## Impact

- `crates/patronus/src/{actions.rs,service.rs}`, `crates/domain-types/src/session.rs`,
  `crates/repository-model/src/{facts.rs,patch.rs,warm_start.rs}`, `crates/test-server/src/remote_host.rs`,
  `src/stores/patronus.ts`, `src/model/patch.ts`, `src/components/branch/PatronusChip.tsx`,
  `tests/e2e/patronus/patronus-actions.spec.ts`, `docs/design/git-operations.md`, `docs/contributing/frontend.md`.
