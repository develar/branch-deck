# Proposal

## Why

The IDE writes its instance file in `<PathManager.getCommonDataPath()>/air/ahp`. On Windows that path falls back to
`~\AppData\Roaming` when `APPDATA` is unset or empty, and an empty variable counts as unset on each platform. Branch
Deck found no directory there, so it found no IDE.

## What Changes

- The directory of the instance files follows the rule of the IDE on each platform: an empty variable is unset, and
  Windows falls back to `~\AppData\Roaming`.

Capabilities: `branch/start-in-air`.
