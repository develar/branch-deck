## Why

Past its usual time a running robot showed only the hours from an hour on: "1h · usually ~1h 14m" for a robot that
ran 1 h 36 min, which reads as if it were still within its time. Within its usual time the same slot says
"1h 36m · ~23m".

## What Changes

- Past its usual time the row says the time a robot runs as within it, with the minutes after the hour:
  "1h 36m · usually ~1h 14m"; from 10 hours the hours only, "14h · usually ~1h", as `formatShortDuration` does
  everywhere. The separator stays `·`.

## Capabilities

### Modified Capabilities

- `branch/patronus`: Timing: The row says how long the robot runs and how long it has left.
