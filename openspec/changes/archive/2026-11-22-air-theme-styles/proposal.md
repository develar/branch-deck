# Proposal

## Why

Branch Deck in the IDE did not look like the desktop app: merged was blue, not violet. The page mapped `--info` of
Branch Deck, the violet of merged, landed and integrated, onto the info of the IDE, which is the color of a link. Its
`--border`, the separator of the rows, took the border of a control of the IDE, so the lines of the rows were darker.
The page also read the accent of `com.jetbrains.air/variables`, which Air never sends an app view. And `apply` only
added variables: one that a later context lacked stayed on the root.

## What Changes

- `theme.ts`: no `--info`, `--primary` or `--primary-foreground` from the host, no read of
  `com.jetbrains.air/variables`. `--border` takes `--color-border-tertiary`, `--input` `--color-border-primary`,
  `--radius` `--border-radius-lg`. The `styles.variables` of a context is the whole set: a variable that it lacks
  leaves the root.
- The app file gets the field `styles` of Air: `all`, `neutral` (no status tones) or `none`. An install writes
  `neutral`, keeps the value of an existing file, and the status compares the file with the text of that value.
- The fake IDE of the E2E sends the exact output of `ideStyleVariables` of Air, and filters it by `styles` as Air does.

Capabilities: `repository/ide-embedding`.
