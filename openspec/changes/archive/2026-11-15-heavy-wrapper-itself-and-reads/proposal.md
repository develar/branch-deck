# The wrapper itself counts, each script is read once, and a script past the depth is heavy

## Why

The review of 74b9f253: a shell or a script run by its path stood only for what it ran, so a pattern that names the
wrapper (`./probe/run.sh*`, `zsh -c *`, `./node_modules/.bin/playwright test*`, whose shim is `#!/bin/sh`) no longer
matched it; a script that sources or calls itself was read again at every mention, 40⁴ times for 40 lines; and a
script run by its path on the limit of the depth went light where a shell went heavy.

## What Changes

- The command itself comes before what it runs, so a pattern of the wrapper matches it, in the run and in a script.
- Each path is read once a classification, 64 at most; past them a script counts as heavy.
- A script that the depth leaves unread, run by its path or sourced, counts as heavy, as a shell does.
- `env -u NAME` keeps the environment and is no word of the command; `env -i` stays one.
- Capability `cli/host`.
