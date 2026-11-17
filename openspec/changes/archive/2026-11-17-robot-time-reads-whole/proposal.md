# Proposal

## Why

A robot past its usual time ("1h 57m · usually ~1h 39m", about 185 px) painted over the pull request chip after it. The
`state` track is capped at `max-w-40` (160 px) and the robot's chip did not shrink: its box was 160 px, its text ran
past it. Both "Live values hold their own width" of the UI Theme and the branch row ("the Status column widens when a
robot is wider than every status text") promised the column would widen; the cap held it.

## What Changes

- A running robot of a row is a live value (`data-live-width`): the cap of the `state` track leaves it, its track widens.
- The robot chip of a row shrinks to its track (`min-w-0`): a finished robot's label truncates under the cap.
- What an item of a track paints lies inside its track: the E2E check `trackOverlaps` of every row, in the layout specs.
