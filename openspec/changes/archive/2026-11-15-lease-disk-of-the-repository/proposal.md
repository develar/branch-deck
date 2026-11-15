# brd lease disk reports the pool of its repository, and --all every pool of the machine

## Why

`brd lease disk` walked every slot of every pool of the machine. On 2026-11-15 a run in branch-deck walked the idea-1
pool too, 11 slots with 1.4M files each, for more than 12 minutes, and printed nothing. A session that asks about its
own repository needs only the pool of that repository.

## What Changes

- `brd lease disk` reports the pool of the repository it runs in (`-C`, else the current directory) and its orphans.
- `brd lease disk --all` reports every pool and every orphan of the machine, as before.
- The Disk window of the app and the agent of `brd-host` keep every pool (`pool_disk::DiskScope::Machine`).
- `--all --cached` takes the latest saved measure of an orphan from the files of all pools.
- Capabilities `cli/lease` ("Disk") and `cli/host` ("disk").
