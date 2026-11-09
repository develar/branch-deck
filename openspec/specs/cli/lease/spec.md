---
name: brd lease
description: Requirements for brd lease, the pool of worktrees that agent sessions lease, one session per slot: get moves a free slot to the HEAD of the source, else takes over an idle lease whose work landed, else adds one and names where its commits land, pick lands them on the branch of the source, return frees it and applies the clean rules of .branchdeck.toml, status reads the state, adopt adds existing worktrees, clean applies the rules on demand, trim deletes the free slots past branchdeck.leaseMaxSlots of every pool of the machine with their Bazel output bases, which a get does itself below the minimum free space before it deletes the build output of the free slots of every pool by their clean rules, the ones returned longest ago first, and then free slots below the limit, disk reports what the pools hold on disk; the host slots are leases of the pool that a lend table lends to one run at a time, which the agent of brd-host drives in process.
style: plain-1
targets:
  - crates/bd-lease/src/lease_command.rs
  - crates/bd-cli/src/lib.rs
  - crates/bd-update/src/pick_command.rs
  - crates/update-core/src/pick.rs
  - crates/git-ops/src/pick.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/domain-types/src/error.rs
  - crates/bd-cli/src/schema.rs
  - crates/lease-pool/src/lease.rs
  - crates/lease-pool/src/landed_work.rs
  - crates/lease-pool/src/agent_workspace.rs
  - crates/bd-lease/src/workspace_command.rs
  - crates/git-ops/src/landed_pairing.rs
  - crates/lease-pool/src/host_slots.rs
  - crates/lease-pool/src/host_refs.rs
  - crates/lease-pool/src/host_git.rs
  - crates/git-ops/src/host_objects.rs
  - crates/git-ops/src/worktree_snapshot.rs
  - crates/git-ops/src/host_client.rs
  - crates/lease-pool/src/state_file.rs
  - crates/lease-pool/src/lease_clean.rs
  - crates/lease-pool/src/check_record.rs
  - crates/lease-pool/src/clone_dir.rs
  - crates/lease-pool/src/free_space.rs
  - crates/lease-pool/src/bazel_servers.rs
  - crates/lease-pool/src/output_bases.rs
  - crates/lease-pool/src/trim.rs
  - crates/lease-pool/src/shrink.rs
  - crates/bd-lease/src/trim_command.rs
  - crates/bd-lease/src/disk_command.rs
  - crates/pool-disk/src/lib.rs
  - crates/pool-disk/src/report.rs
  - crates/pool-disk/src/report_test.rs
  - crates/fs-walk/src/lib.rs
  - crates/fs-walk/src/changes.rs
  - crates/bd-core/src/error.rs
  - .branchdeck.toml
  - crates/sync-utils/src/process_scan.rs
  - crates/sync-utils/src/size.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/git-ops/src/lease_slot.rs
  - crates/git-executor/src/worktree_safety_test.rs
---

# brd lease

Status: Implemented
Date: 2026-10-19

## Purpose

Agent sessions that share a checkout each work in a worktree of their own and land their commits with `brd lease pick`
([brd pick](../pick/spec.md)). `brd lease` keeps the pool of those worktrees. `brd lease get` leases a slot at the HEAD of
the source checkout: a free slot is moved there, so only the diff between the two commits is written, and a new slot
is added only when no slot is free and no idle lease, one whose holder process is gone, holds only landed work. Its
answer names where the commits of the lease land (`home`) and what the session runs (`next`), so an agent never works
out the landing itself. `brd lease pick` in the lease lands each commit on that
branch, onto its tip as it is then, and the lease follows. `brd lease return` frees the slot once its commits landed. The state in
`$GIT_COMMON_DIR/branch-deck/leases.json` is the authority. A free slot is pool property, and the next get replaces
its content, so no ancestry check decides whether a slot is free. How it works: "Lease" in
[Git Operations](../../../../docs/design/git-operations.md). Working beside other sessions: `brd help worktrees`
([Agent CLI](../agent-cli/spec.md)). Whether an agent leases at all, `branchdeck.agentLease`, says
[brd workspace](../workspace/spec.md), which it runs before its first edit.

## Goals

- A get in a repository of 1.4M files takes seconds when a slot is free: one `status` and a move of the slot by the
  diff, never a full checkout and never a scan of the other slots.
- A slot whose last commit a rewrite of master made unreachable is reused as any other free slot.
- `brd lease status` answers from the state in milliseconds: no `git status` of any slot.
- Concurrent gets never get the same slot and never wait for a `brd update`; a get that reuses a slot never waits for
  another's checkout, and a pool adds one slot at a time, so a burst of gets never clones a large checkout many times
  at once and a slot freed meanwhile wins over a new one.
- A return never loses work silently: unlanded commits, local changes and a process of another session in the slot
  refuse it. Nothing that cannot hurt the slot refuses it: the session that returns it, git's background maintenance,
  the Bazel servers of the slot, which the return stops itself.
- A get never starts on a volume that the builds of the slot would fill (`branchdeck.leaseMinFree`), and a new slot
  never carries the build output of the checkout it was cloned from.
- A pool gives its space back: it grows while sessions need slots, and below the minimum free space it shrinks to
  `branchdeck.leaseMaxSlots` by itself, with the Bazel output bases outside its slots, then deletes the build output
  that its free slots keep for leases that rarely come, the slot returned longest ago first, and only then free slots
  below the limit. On 2026-11-02 the pools of this
  machine held 12 slots of idea-1, 11 of branch-deck and 6 of accio, all free; the output bases of the idea-1 slots
  took 380 GB and the `target/` dirs of the branch-deck slots 410 GB, and nothing ever deleted a slot.
- A session that died costs the pool no clone: its lease goes to the next get once every commit of it is on its home
  branch, and never while it holds work that branch lacks. On 2026-10-29 a get cloned a 16 GB slot of idea-1 (its
  checkout alone took more than 15 minutes) while six slots had a lease of a dead holder, four of them with nothing
  unlanded.

## Non-goals

- A fetch: the slot moves to the HEAD of the source as it is.
- A reset of a returned slot to master: the next get moves it.
- A walk of the tree for `.worktreeinclude`: only literal paths and globs in one directory are seeded.
- The process scan on Windows: there it is reported as not checked, and a lease never waits for it; only the Bazel
  servers of a slot are found there, by their output base.

## Concepts

### Host slots

The slots that the agent of `brd-host` keeps for the runs of other machines (openspec/specs/cli/host/spec.md): it calls the
functions of `lease_pool::host_slots` in process on the host. A remote slot holds no state of its own, since the commit
comes from the client and the outputs go back to it, so a slot is lent to one run for the duration of that run only.

### Host objects

The git of the runs on the host goes through these functions too (`lease_pool::host_refs`, `git_ops::host_objects`),
so the agent starts no git of its own: which commits the host has, the packs of the clients, the refs that keep their
commits, and what a commit holds.

### Host runs

The files of a run: on the host the scans of the slot before and after its command, on the client (`brd-host run`,
`git_ops::host_client`) the commit the run ships, its pack and the apply of its changes. A commit of the files of a
worktree (`worktree_commit`) is what `git add --all` stages, on HEAD, as the requirements of Host runs below say.

## Requirements

### Requirement: Get: The answer names the home of the lease and the next commands

The answer of a get MUST name where the commits of the lease land, `home`: `checkout` (the source checkout, its top
level) and `branch` (the branch it had checked out, `null` for a detached HEAD), and `next`, what the session runs in
order.

#### Scenario: A lease with a home branch
- **WHEN** a get leases a slot from a checkout with a branch checked out
- **THEN** `next` is `brd -C <path> lease pick` after each commit that builds and passes its tests, then
  `brd lease return --lease-id <id>` once the task is done
  [@test] crates/bd-lease/src/lease_pick_test.rs (`the_get_names_the_home_of_the_lease_and_the_commands_of_the_session`)

#### Scenario: No branch in home
- **WHEN** `home` has no branch
- **THEN** there is nothing to pick onto, and `next` is only the return

### Requirement: Get: A get leases a free slot at the HEAD of the source

`brd lease get` MUST lease a free slot of the pool of this repository, moved to the exact HEAD commit of `-C` (or the
current directory), detached, and print `path`, `lease_id` (a UUID), `holder` (`--holder`, default `agent-<uuid>`),
`slot` (the worktree id), `source_head`, `leased_at`, `reused`, `seeded` and `processes_checked`.

#### Scenario: A get of a free slot
- **GIVEN** a free slot in the pool of this repository
- **WHEN** `brd lease get` runs
- **THEN** the slot is leased, detached at the exact HEAD commit of `-C` (or the current directory)
- **AND** the answer prints the fields of the lease

### Requirement: Get: A slot whose commit master no longer holds is reused

A slot whose commit master no longer holds, after a rewrite of master, MUST be reused like any free slot.

#### Scenario: Master was rewritten past the commit of a free slot
- **GIVEN** a free slot whose commit master no longer holds, after a rewrite of master
- **WHEN** a get runs
- **THEN** it reuses that slot like any free slot
  [@test] crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)

### Requirement: Get: A get below the minimum free space refuses before it reserves

With a minimum free space, `--min-free <size>` or else `branchdeck.leaseMinFree` of the git config, a get MUST refuse
before it reserves anything when the volume of the pool has less free space (`f_bavail` of `statfs`):
`low_disk_space` (exit 4), the message naming the free space and the minimum in GiB, the free slots and the leases
whose holder process is gone.

#### Scenario: The volume has less free space than the minimum
- **GIVEN** a minimum free space of `--min-free <size>` or of `branchdeck.leaseMinFree`
- **WHEN** a get finds less free space on the volume of the pool (`f_bavail` of `statfs`)
- **THEN** it refuses with `low_disk_space` (exit 4) before it reserves anything
- **AND** the message names the free space and the minimum in GiB, the free slots and the leases whose holder process
  is gone
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_free_space_refuses_and_names_it`)

### Requirement: Get: The minimum free space is a size of the git config of the machine

The minimum MUST be a number and a binary unit, `200G` or `200GiB`, K to P, as `df -h` counts; `0` and no setting
check nothing. A size that does not parse MUST be `invalid_arguments`. The minimum is the machine's, so it MUST be git
config, never `.branchdeck.toml`.

#### Scenario: A size is a number and a binary unit
- **WHEN** the minimum is `200G` or `200GiB`, K to P
- **THEN** it counts as `df -h` counts
  [@test] crates/sync-utils/src/size_test.rs (`a_size_is_a_number_and_a_binary_unit`)

#### Scenario: No minimum
- **WHEN** the minimum is `0` or there is no setting
- **THEN** the get checks nothing

#### Scenario: A size that does not parse
- **WHEN** `--min-free` or `branchdeck.leaseMinFree` does not parse
- **THEN** the get fails with `invalid_arguments`

### Requirement: Get: A successful get prints the free space

A successful get MUST print `free_bytes`; where the system does not tell it (Windows) nothing is checked.

#### Scenario: The free space of a successful get
- **WHEN** a get succeeds
- **THEN** its answer prints `free_bytes`

#### Scenario: A system that does not tell the free space
- **WHEN** the system does not tell the free space (Windows)
- **THEN** nothing is checked

### Requirement: Get: A get that adds a slot checks the minimum again under the clones lock

A get that adds a slot MUST check again under the clones lock (below), before its reserve there, so a queue of gets
never adds slots past the minimum.

#### Scenario: A get that waited for the clones lock
- **WHEN** a get waited for the clones lock to add a slot
- **THEN** it reads `branchdeck.leaseMinFree` again
- **AND** below the minimum it refuses with nothing reserved
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_waited_for_the_clones_lock_checks_the_minimum_free_space_under_it`)

### Requirement: Get: A free slot is a linked worktree no other session works in

A free slot MUST be a linked worktree of the same git common dir whose directory exists, and no process of another
session may work in it; a busy free slot MUST be skipped (`skipped`, `reason: "busy"`, `processes` [{`pid`, `command`,
`argv`, `kind`, `workspace`?}]). A get SHALL allow the source to be itself a leased slot.

#### Scenario: A process of another session works in a free slot
- **GIVEN** a process of another session works in a free slot
- **WHEN** a get runs
- **THEN** it skips the slot as busy and names the processes
  [@test] crates/bd-lease/src/lease_test.rs (`a_busy_free_slot_is_skipped`)

#### Scenario: Only a linked worktree of this repository is a slot
- **WHEN** a worktree is not a linked worktree of the same git common dir, or its directory does not exist
- **THEN** it is no free slot
  [@test] crates/bd-lease/src/lease_test.rs (`adopt_adds_clean_linked_worktrees_and_refuses_the_rest`)

### Requirement: Get: A get stops the Bazel servers of a free slot it reserved

A get MUST stop the Bazel servers of a free slot it reserved (`--workspace_directory` in the slot, wherever their
working directory is) with SIGTERM, wait up to 30 s, name them in `stopped_servers` and lease the slot: the slot is
the pool's, a server serves only it, and a `--watchfs` server could miss the events of the checkout and build stale.

#### Scenario: A Bazel server of the slot exits
- **GIVEN** a Bazel server whose `--workspace_directory` is in a free slot the get reserved
- **WHEN** the get runs
- **THEN** it stops the server with SIGTERM, names it in `stopped_servers` and leases the slot
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_stops_the_bazel_server_of_a_free_slot_and_leases_it`)

#### Scenario: A server that does not exit
- **WHEN** a server does not exit within 30 s
- **THEN** the get gives the slot back free and skips it as busy

#### Scenario: Windows
- **WHEN** the get runs on Windows
- **THEN** it finds and stops the servers by their output base, as the return does ([Return](#return))

### Requirement: Get: A get records the process it runs for

A get MUST record the process it runs for, the nearest ancestor that is no shell or wrapper (the coding agent, the
terminal), with its start time (`holder_pid`, `holder_started`): the `pid` of a lease is of the get, which exits at
once.

#### Scenario: The holder of a lease
- **WHEN** a get records its lease
- **THEN** the lease names the nearest ancestor that is no shell or wrapper with its start time
- **AND** the status tells whether that holder is alive
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`)

### Requirement: Get: A gone holder alone frees no lease

A gone holder alone MUST free no lease: its slot may hold unlanded work, and a resumed session runs as another
process. A get that adds a new slot MUST name the `ready` leases whose holder process is gone (`idle_leases`, each
with its `lease_id` and `idle`), and so MUST `low_disk_space`.

#### Scenario: A get adds a slot beside a lease whose holder is gone
- **GIVEN** a `ready` lease whose holder process is gone
- **WHEN** a get adds a new slot, or refuses with `low_disk_space`
- **THEN** the lease stays and the answer names it in `idle_leases`

### Requirement: Get: A get takes over an idle lease before it adds a slot

After the free slots, before it adds a slot, a get MUST take over an idle lease, the oldest first: `ready`, its holder
process gone, no host slot, a linked worktree of this repository, no process in it but its Bazel servers, and quiet
for an hour, its get and the last move of its HEAD (the mtime of the slot's `logs/HEAD`).

#### Scenario: An idle lease whose work landed
- **GIVEN** no free slot and an idle lease, quiet for an hour, whose commits landed
- **WHEN** a get runs
- **THEN** it takes the idle lease over instead of adding a slot
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_takes_over_an_idle_lease_whose_commits_landed`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_takes_over_an_idle_lease`)

#### Scenario: A holder recorded as a wrapper that exits at once
- **GIVEN** a holder recorded as a wrapper that exits at once (`cargo run`)
- **WHEN** a get runs within the hour of the get of that lease or of the last move of its HEAD
- **THEN** it does not take it over, since the agent of the lease just began it
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`)

### Requirement: Get: A get takes over only a lease whose work is on its home branch

A get MUST take the lease over only when every commit of the lease that its home branch lacks (`HEAD
^refs/heads/<home> ^<leased commit>`, whatever other branch, its own `-b` one too, holds it) is proven on that branch
by the landed rule of the return ([Return](#return)), the slot has no local changes, no git operation in progress and
no open resolve session. A lease without a home branch MUST be taken over only without commits of its own.

#### Scenario: Commits landed, one with a conflict resolution
- **WHEN** every commit the home branch lacks is proven on it by the landed rule of the return
- **THEN** the get takes the lease over
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_takes_over_an_idle_lease_whose_commits_landed`, `a_get_takes_over_an_idle_lease_whose_commit_landed_with_a_conflict_resolution`)

#### Scenario: An idle lease on its own branch
- **GIVEN** the commits of the lease are held by its own `-b` branch
- **WHEN** a get checks it
- **THEN** it takes it over only when home has its commits
  [@test] crates/bd-lease/src/takeover_test.rs (`an_idle_lease_on_its_own_branch_is_taken_over_only_when_home_has_its_commits`)

#### Scenario: A git operation in progress
- **WHEN** the git dir of the slot holds `rebase-merge`, `rebase-apply`, `MERGE_HEAD`, `CHERRY_PICK_HEAD`,
  `REVERT_HEAD`, `sequencer` or `BISECT_LOG`, or the slot has local changes or an open resolve session
- **THEN** the get does not take the lease over

#### Scenario: A lease without a home branch
- **GIVEN** a lease without a home branch
- **WHEN** a get checks it
- **THEN** it takes it over only without commits of its own

### Requirement: Get: The answer of a takeover names the lease it took over

The answer of a takeover MUST name the lease it took over (`took_over`: `lease_id`, `holder`, `leased_at`, its
`branch`, which stays, and `landed`, each commit with its proof) and the slot MUST keep its record (`reclaimed` of
`brd lease status`) until the next get of the free slot.

#### Scenario: The answer and the record of a takeover
- **WHEN** a get takes over an idle lease
- **THEN** its answer names it in `took_over` with the proof of each commit in `landed`, and its branch stays
- **AND** `brd lease status` shows the record as `reclaimed` until the next get of the free slot
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_takes_over_an_idle_lease_whose_commits_landed`)

### Requirement: Get: A takeover is a return of the old lease by the pool

The takeover MUST be a return of the old lease by the pool: its seeded paths removed, HEAD detached, the clean rules of
the source applied into the trash, which the get leaves for the next return or clean to delete (the trash of a run that
is gone), then the move of a reused slot.

#### Scenario: The slot of a takeover
- **WHEN** a get takes over an idle lease
- **THEN** the seeded paths are removed, HEAD is detached and the clean rules of the source go into the trash
- **AND** the get leaves the trash for the next return or clean and moves the slot as a reused one

### Requirement: Get: An idle lease with work its home branch lacks is never taken over

An idle lease with work its home branch lacks MUST NOT be taken over: the get skips it (`skipped`, `reason:
"idle_with_work"`, `idle` with its `commits`, `paths` or `operation`), records the check in the lease (`idle_check`)
and adds a slot, and `brd lease status` shows it as `idle_with_work`.

#### Scenario: An idle lease with unlanded work
- **GIVEN** an idle lease with commits its home branch lacks, local changes or a git operation in progress
- **WHEN** a get runs
- **THEN** it skips the lease, records `idle_check` and adds a slot
- **AND** `brd lease status` shows it as `idle_with_work`
  [@test] crates/bd-lease/src/takeover_test.rs (`an_idle_lease_with_unlanded_work_is_kept_and_named`)

### Requirement: Get: A kept idle lease is checked again only when its verdict moved

A later get MUST check a kept idle lease again only when what the verdict rests on moved: the tree work while the slot
(its `HEAD`, the length and the mtime of its reflog) is unchanged, the commits also while the home tip is; else it
costs one `rev-parse`. One get MUST check at most two idle leases in full.

#### Scenario: Neither the slot nor home moved
- **WHEN** a later get finds the slot and the home tip as the check found them
- **THEN** it keeps the verdict for one `rev-parse`
  [@test] crates/bd-lease/src/takeover_test.rs (`a_kept_idle_lease_is_checked_again_only_when_its_slot_or_home_moved`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_passes_over_an_idle_lease_it_checked`)

#### Scenario: Many idle leases
- **WHEN** a get finds more than two idle leases to check
- **THEN** it checks at most two of them in full

### Requirement: Get: A get holds the slot it checks by a preparing lease

While it checks, the get MUST hold the slot by a `preparing` lease that keeps the idle one (`taking_over`); a check
that finds work, an error and a get whose process is gone MUST give it back (from the copy of the get, so a brd that
wrote the state without the field loses nothing), in the reserve of the next get, the mark of `brd lease clean`, a
return and the status alike.

#### Scenario: A get killed while it checks
- **WHEN** the get that checks an idle lease is killed
- **THEN** the next reader of the state gives the idle lease back
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_killed_while_it_checks_an_idle_lease_gives_the_lease_back`)

### Requirement: Get: A takeover commits under the pool lock

The commit point of a takeover MUST be under the pool lock: the lease still the get's and the slot as the check found
it. From there a get that dies leaves the slot free, as its work landed.

#### Scenario: A commit during the check
- **WHEN** a resumed session commits in the slot during the check
- **THEN** the takeover stops
  [@test] crates/lease-pool/src/lease_test.rs (`a_commit_during_the_check_stops_the_takeover`)

### Requirement: Get: A get below the minimum deletes the trash of gone runs first

A get whose volume is below the minimum free space with trash in the pool MUST delete the trash of runs that are gone
first (what a takeover left there) and measure again; `low_disk_space` MUST name what the trash of a running process
keeps.

#### Scenario: Trash of gone runs below the minimum
- **GIVEN** trash in the pool that a takeover left there
- **WHEN** a get finds the volume below the minimum free space
- **THEN** it deletes the trash of runs that are gone first and measures again
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_below_the_minimum_free_space_deletes_the_trash_of_gone_runs_first`)

### Requirement: Get: The trash is never deleted under the clones lock

The delete of the trash MUST NOT run under the clones lock: a get whose turn at the lock finds the volume below the
minimum and such a trash releases the lock, deletes the trash (`deleting the trash of runs that are gone`) and takes
its turn again; it deletes the trash once per wait for the lock.

#### Scenario: A turn at the clones lock below the minimum
- **WHEN** a get's turn at the clones lock finds the volume below the minimum and trash of gone runs
- **THEN** it releases the lock, deletes the trash and takes its turn again
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_deletes_the_trash_without_the_clones_lock`)

### Requirement: Get: A get still below the minimum after the trash trims the pools

A get whose volume is still below the minimum free space after the trash MUST trim the pools of the machine once
(`deleting the free slots past branchdeck.leaseMaxSlots …`, "Trim" below), never under the clones lock, and measure
again; its answer MUST name what went (`trimmed`, the output of `brd lease trim`), and a `low_disk_space` after it how
many slots it deleted and the bytes it freed.

#### Scenario: The trim of a get
- **WHEN** a get is still below the minimum free space after the trash
- **THEN** it deletes the free slots past `branchdeck.leaseMaxSlots` of every pool and measures again
- **AND** a `low_disk_space` after it names how many slots it deleted and the bytes it freed
  [@test] crates/bd-lease/src/trim_test.rs (`a_get_below_the_minimum_free_space_trims_the_pools_first_and_names_what_it_deleted`)

### Requirement: Get: A get still below the minimum after the trim shrinks the free slots

A get whose volume is still below the minimum free space after the trim MUST shrink the pools of the machine once
(`deleting the build output of the free slots …`), never under the clones lock, and measure again: each free slot by
the clean rules of its own `.branchdeck.toml` with `keep` 0 and `age` 0 under the `cleaning` marker, as
`brd lease clean` cleans a free slot: the worktree stays, what the rules name goes through the trash of its pool,
deleted once the marker is gone.

#### Scenario: The shrink of a get
- **WHEN** a get is still below the minimum free space after the trim
- **THEN** it deletes the build output of the free slots by their clean rules with `keep` 0 and `age` 0, the
  worktrees staying
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_free_space_deletes_the_build_output_of_the_free_slots_returned_longest_ago_first`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_shrinks_and_evicts`)

#### Scenario: Never under the clones lock
- **WHEN** a get that waited for the clones lock is below the minimum
- **THEN** it checks the minimum free space under the lock, and shrinks only after it released it
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_waited_for_the_clones_lock_checks_the_minimum_free_space_under_it`)

#### Scenario: The free slots of 2026-11-03
- **GIVEN** on 2026-11-03 each free slot of branch-deck held 50 to 59 GiB, mostly `target/`, and the volume had
  227 GiB against a minimum of 200 GiB
- **WHEN** the rules of a return keep the 8 newest units of every group, warm output that the next lease of a slot
  rarely reuses, as a get takes the slot returned last, and no trim deleted a slot of pools within their limit
- **THEN** `brd lease clean --free --keep 0 --age 1m` freed about 600 GiB

### Requirement: Get: The shrink takes the free slots returned longest ago first

The shrink MUST take the free slots of every pool, the ones returned longest ago first (`returned_at`; a slot never
returned, then one returned before that field, first of all), and the slot each pool's next get would take (clean
before dirty, returned last first) only after every other.

#### Scenario: Free slots of several pools
- **GIVEN** free slots of several pools, returned at different times
- **WHEN** a get shrinks them
- **THEN** it takes them in the order of `returned_at` across pools, and each pool's next slot last
  [@test] crates/lease-pool/src/shrink_test.rs (`the_shrink_takes_the_free_slots_returned_longest_ago_across_pools_and_each_pools_next_one_last`, `the_next_slot_of_a_pool_is_the_one_its_get_would_take`)

### Requirement: Get: The shrink cleans only the free slots of the volume the get measures

Only the free slots on the volume the get measures (the device of its pool directory) MUST be candidates of the
shrink: a pool of `branchdeck.leasePool` or a worktree adopted where it was can live on another disk, whose deletes
would not raise that free space.

#### Scenario: A slot on another device
- **WHEN** a free slot lives on another disk, or on another APFS volume of the same container, which shares its free
  space but not its device
- **THEN** the shrink passes over its slots
  [@test] crates/lease-pool/src/free_space_test.rs (`a_volume_holds_the_paths_of_its_own_device_only`)

### Requirement: Get: The shrink passes over busy slots and cleans dirty ones

A free slot with a process in it MUST stay (`busy`), as MUST one without rules. A free slot marked dirty and one never
returned MUST be cleaned as well: their rules name build output, never the changes in their worktree.

#### Scenario: A process in a slot and a slot without rules
- **WHEN** the shrink comes to a free slot with a process in it, or one without rules
- **THEN** the slot stays as it is
  [@test] crates/bd-lease/src/lease_test.rs (`a_shrink_passes_over_a_free_slot_with_a_process_in_it_and_one_without_rules`)

#### Scenario: A dirty slot and one never returned
- **WHEN** the shrink comes to a free slot marked dirty, or one never returned
- **THEN** it cleans the build output its rules name and leaves the changes in its worktree

### Requirement: Get: The shrinks and evictions of the machine take turns under one lock

The shrinks and the evictions of the gets of the machine MUST take turns under one lock
(`~/.branch-deck/pressure.lock`; without a home, beside the state of the pool): a get that waits tells it
(`waiting for the shrink or the eviction of another get …`) and measures again after its turn, so gets side by side
clean no slot that the volume did not need.

#### Scenario: A get that waited for the shrink of another
- **WHEN** a get waited for the shrink of another get and the volume has the target after it
- **THEN** it cleans nothing
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_that_waited_for_the_shrink_of_another_cleans_nothing_once_the_volume_has_the_target`)

### Requirement: Get: The shrink stops at the minimum and a tenth of it

Before each slot the get MUST measure (one `statfs`) and stop once the volume has the minimum and a tenth of it:
without the margin every later get would find the volume just below the minimum again and clean one more slot.

#### Scenario: One slot lifts the volume past the minimum
- **WHEN** the shrink of one slot lifts the volume past the minimum and a tenth of it
- **THEN** the shrink cleans only that slot
  [@test] crates/bd-lease/src/lease_test.rs (`a_shrink_that_one_slot_lifts_past_the_minimum_cleans_only_that_slot`)

### Requirement: Get: The answer names the slots the shrink cleaned

The answer of a get that shrank MUST name each slot it cleaned with what its rules freed (`shrunk`), and a
`low_disk_space` after it MUST name how many slots it cleaned and the bytes they freed.

#### Scenario: Still below the minimum after the shrink and the eviction
- **WHEN** a get is still below the minimum after the shrink and the eviction
- **THEN** its `low_disk_space` names how many slots it cleaned and the bytes they freed
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_still_below_the_minimum_after_the_shrink_and_the_eviction_refuses_and_names_what_it_freed`)

### Requirement: Get: A get still below the minimum after the shrink evicts free slots

A get whose volume is still below the minimum free space after the shrink MUST evict free slots below
`branchdeck.leaseMaxSlots` once (`deleting free slots below …`), never under the clones lock, in the order of the
shrink across every pool of the machine, each as the trim deletes a slot, deleted before the next is chosen.

#### Scenario: The eviction of a get
- **WHEN** a get is still below the minimum free space after the shrink
- **THEN** it deletes free slots below `branchdeck.leaseMaxSlots`, returned longest ago first
- **AND** each goes as the trim deletes a slot: the `cleaning` marker, its Bazel servers stopped, its directory and
  its output bases through the trash, its registration and its entry dropped
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_still_below_the_minimum_after_the_shrink_deletes_free_slots_below_the_limit_returned_longest_ago_first`)

#### Scenario: The trim, the shrink and the eviction run without the clones lock
- **WHEN** a get trims, shrinks or evicts
- **THEN** it holds no clones lock
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_deletes_the_trash_without_the_clones_lock`)
  [@test] crates/bd-lease/src/trim_test.rs (`a_get_below_the_minimum_free_space_trims_the_pools_first_and_names_what_it_deleted`)

### Requirement: Get: The eviction stops at the minimum without the margin of the shrink

The eviction MUST measure (one `statfs`) before each slot and stop once the volume has the minimum, without the margin
of the shrink: a slot is a whole worktree, and with slots of 15 GiB a margin of 20 GiB would delete one more after the
volume is already above the minimum.

#### Scenario: The volume reaches the minimum
- **WHEN** an evicted slot brings the volume to the minimum
- **THEN** the eviction stops
  [@test] crates/bd-lease/src/lease_test.rs (`the_eviction_stops_at_the_minimum_without_the_margin_of_the_shrink`)

### Requirement: Get: The eviction keeps leased, busy and three kinds of free slots

Only the slots on the volume the get measures MUST be candidates, as for the shrink. A slot with a lease of any state
(a host slot too) and one with a process in it but its Bazel servers MUST stay, and so MUST three free slots: the one
the get takes next in its own pool, one marked dirty, and one never returned.

#### Scenario: The next slot of the pool
- **WHEN** the eviction comes to the free slot the get takes next in its own pool
- **THEN** it stays, as the get reuses it warm where an eviction would make it clone the same worktree again

#### Scenario: A dirty slot and one never returned
- **WHEN** the eviction comes to a free slot marked dirty, whose changes made without a lease may be their only copy,
  or one never returned, such as a worktree that `brd lease adopt` kept where it was
- **THEN** it stays
  [@test] crates/bd-lease/src/lease_test.rs (`the_eviction_keeps_dirty_and_never_returned_free_slots`)

#### Scenario: A leased or busy slot
- **WHEN** a slot has a lease of any state (a host slot too), or a process in it but its Bazel servers
- **THEN** it stays

### Requirement: Get: The answer names the slots the eviction deleted

The answer of a get that evicted MUST name the slots it deleted (`evicted`, as `brd lease trim` prints them), and a
`low_disk_space` after it MUST name how many and the bytes they freed. A free slot is pool property: after its build
output, what is left of it is its worktree, which a later get clones again, and the user, told only `low_disk_space`,
would delete the slots by hand.

#### Scenario: Still below the minimum after the eviction
- **WHEN** a get is still below the minimum after the shrink and the eviction
- **THEN** its `low_disk_space` names how many slots it deleted and the bytes they freed
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_still_below_the_minimum_after_the_shrink_and_the_eviction_refuses_and_names_what_it_freed`, `a_get_below_the_minimum_free_space_refuses_and_names_it`)

### Requirement: Get: A minimum above the size of the volume neither shrinks nor evicts

A get whose minimum free space is more than the volume holds in all (`f_blocks` of the same `statfs`) MUST neither
shrink nor evict: nothing they delete could reach it, and one mistaken `branchdeck.leaseMinFree` or `--min-free` would
delete every free slot of every pool. Its `low_disk_space` MUST say that the volume holds less than the minimum.

#### Scenario: A minimum of more than the volume
- **WHEN** the minimum free space is more than the volume holds in all
- **THEN** the get deletes no build output and no free slot below `branchdeck.leaseMaxSlots`
- **AND** its `low_disk_space` says that the volume holds less than the minimum
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_free_space_refuses_and_names_it`)
  [@test] crates/bd-lease/src/trim_test.rs (`a_get_below_the_minimum_free_space_trims_the_pools_first_and_names_what_it_deleted`)

### Requirement: Get: A free slot with local changes is skipped and marked dirty

A free slot with local changes (one `status --porcelain=v1`, the repository's own `core.fsmonitor` applies) MUST be
skipped (`skipped`, `reason: "dirty"`, `paths`), stay free, and be marked dirty; later gets MUST try a dirty slot after
the clean ones. With no free slot left, the get MUST add a new slot (`reused: false`).

#### Scenario: A dirty free slot
- **GIVEN** a free slot with local changes
- **WHEN** a get runs
- **THEN** it skips the slot as dirty with its `paths`, the slot stays free and is marked dirty
- **AND** with no free slot left the get adds a new slot (`reused: false`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_dirty_free_slot_is_skipped_and_a_new_slot_is_added`)

### Requirement: Get: The dirty verdict is recorded with its proof

The verdict MUST be recorded with the proof of it, no git: the slot's mark (its HEAD and reflog), the stat (exists,
length, modification time, inode) of up to 8 of the dirty paths, and the stat of the slot's top-level `.gitignore` and
of `info/exclude`.

#### Scenario: The record of a dirty slot
- **WHEN** a get finds a free slot dirty
- **THEN** it records the mark of the slot, the stat of up to 8 of the dirty paths and of the two ignore files

### Requirement: Get: A later get skips a slot its files prove dirty without a status

A later get MUST skip the slot as dirty without a `status`, naming the paths found then, only while the mark and the two
ignore files are unchanged and at least one recorded path has exactly its stat of then: a file whose stat did not move
kept its content, so the slot is dirty still. Otherwise it MUST run one `status` again.

#### Scenario: A recorded path keeps its stat
- **GIVEN** the mark and the two ignore files are unchanged and a recorded path has exactly its stat of then
- **WHEN** a later get comes to the slot
- **THEN** it skips the slot as dirty without a `status`, naming the paths found then
  [@test] crates/bd-lease/src/lease_test.rs (`a_dirty_slot_that_its_files_prove_dirty_is_skipped_without_a_status`)
  [@test] crates/lease-pool/src/lease_test.rs (`a_dirty_verdict_holds_while_one_dirty_path_has_its_stat`)

#### Scenario: The proof moved
- **WHEN** a path was deleted, reverted or edited, as a clean by hand does, or the slot moved
- **THEN** the get runs one `status` again

### Requirement: Get: Every status of the pool lists untracked files in the configured mode

Every `status` of the pool (a get, the check of a clone, a return, an adopt) MUST list untracked files with
`--untracked-files=all` when the repository's `status.showUntrackedFiles` is `all`, else with `normal`, also for `no`:
git's untracked cache answers only in the mode it was built for, and git builds it only in the configured mode.

#### Scenario: The mode of the repository
- **WHEN** a `status` of the pool runs
- **THEN** it lists untracked files with `--untracked-files=all` for `all`, else with `normal`, also for `no`
  [@test] crates/bd-lease/src/lease_test.rs (`every_status_of_the_pool_lists_untracked_files_in_the_mode_the_repository_configures`)

### Requirement: Get: Every status of the pool may write the index of the slot

Every `status` of the pool MUST be allowed to write the slot's index (`GIT_OPTIONAL_LOCKS=1`), so its refreshed stat
data, fsmonitor token and untracked cache stay.

#### Scenario: The untracked cache stays warm
- **WHEN** a get and its return ran in a slot
- **THEN** a `status` that may not write finds the untracked cache filled
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_of_a_slot_leaves_its_untracked_cache_warm`)

### Requirement: Get: The clean free slot returned last is leased first

Among the clean free slots a get MUST take the one returned last first, then the ones never returned in the order they
joined the pool: the slot returned last has the warmest index and caches and the commit closest to the source.

#### Scenario: Several clean free slots
- **WHEN** a get chooses among clean free slots
- **THEN** it takes the one returned last first
  [@test] crates/bd-lease/src/lease_test.rs (`the_free_slot_returned_last_is_leased_first`)

### Requirement: Get: Each return records its number and its time

Each return (and a reclaim) MUST record the number of the return in the pool (`returned`) and its time
(`returned_at`, Unix seconds), which orders the free slots of different pools; a state without the time reads as
returned before every slot that has one.

#### Scenario: A state without the time of a return
- **WHEN** a free slot has `returned` but no `returned_at`
- **THEN** it reads as returned before every slot that has one

### Requirement: Get: Every checkout of the pool runs with a worker per core

Every checkout and move of the pool (a reused slot, a clone, a new slot after its registration) MUST run with
`-c checkout.workers=0` (one worker per core), unless the repository config sets `checkout.workers`.

#### Scenario: The workers of a checkout
- **WHEN** the pool checks out or moves a slot
- **THEN** it runs with `-c checkout.workers=0`, unless the repository sets `checkout.workers`
  [@test] crates/bd-lease/src/lease_test.rs (`every_checkout_of_the_pool_runs_with_a_worker_per_core_unless_the_repository_sets_it`)

### Requirement: Get: A reused slot moves to the source HEAD by the diff

A get of a reused slot, after its clean `status`, MUST move the slot to the source HEAD by the diff of the two commits:
`reset --merge` of its detached HEAD (a HEAD on a branch is detached first, the branch stays), which reads and writes
only the paths of the diff; a slot at the source HEAD writes nothing. The move MUST keep the repository's
`core.fsmonitor`.

#### Scenario: A clean slot at another commit and one at the commit
- **WHEN** a get moves a clean slot
- **THEN** a slot at another commit moves by the diff, and one at the source HEAD writes nothing
  [@test] crates/git-ops/src/lease_slot_test.rs (`a_clean_slot_moves_by_the_diff_and_one_at_the_commit_stays`)

#### Scenario: A slot on a branch
- **WHEN** the HEAD of the slot is on a branch
- **THEN** it is detached before the move, and the branch stays
  [@test] crates/git-ops/src/lease_slot_test.rs (`a_slot_on_a_branch_is_detached_before_the_move_and_its_branch_stays`)

#### Scenario: The fsmonitor token
- **WHEN** the move writes the index
- **THEN** it keeps `core.fsmonitor`, as a git that writes the index without it drops the token, and the next `status`
  walks every file

### Requirement: Get: The new branch is attached in one transaction

The new branch of `-b` MUST be created and HEAD attached to it in one ref transaction, which reads no file.

#### Scenario: A new branch and an existing one
- **WHEN** the get creates the branch of `-b`
- **THEN** it creates the branch and attaches HEAD in one transaction, and an existing branch fails it
  [@test] crates/git-ops/src/lease_slot_test.rs (`a_new_branch_is_attached_in_one_transaction_and_an_existing_one_fails_it`)

### Requirement: Get: Every file is checked out only when git refuses the diff

Every file MUST be checked out (`checkout --force`) only when git refuses the diff (an untracked file or a local change
in a path of it), with git's reason among the steps on stderr.

#### Scenario: An untracked file in a path of the diff
- **WHEN** an untracked file is in a path of the diff
- **THEN** git refuses the move, and the get checks out every file, telling git's reason on stderr
  [@test] crates/git-ops/src/lease_slot_test.rs (`an_untracked_file_in_a_path_of_the_diff_refuses_the_move`)

### Requirement: Get: An index lock no process owns is removed

An `index.lock` that no process owns (a git killed in the slot) MUST be removed, and one of a live process MUST be
waited for.

#### Scenario: A killed git left the index lock
- **WHEN** a git killed in the slot left its `index.lock`
- **THEN** the get removes it and moves the slot
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_removes_the_index_lock_a_killed_git_left_in_the_slot`)

#### Scenario: The index lock of a live process
- **WHEN** a live process holds the `index.lock` of the slot
- **THEN** the get waits for it

### Requirement: Get: On macOS a new slot is an APFS clone of a template

On macOS a new slot MUST be an APFS clone (`created_by: "clone"`, `cloned_from`) of a template: a free slot of this
repository that the get passed over (one without a process first, among those a clean one first), marked `templating`
while it is cloned, else the source checkout. The template MUST be free again afterwards, and the source unchanged.

#### Scenario: A clone of the source
- **WHEN** no free slot was passed over
- **THEN** the new slot is a clone of the source checkout, and the source is unchanged
  [@test] crates/bd-lease/src/lease_test.rs (`a_new_slot_is_a_clone_of_the_source_without_its_local_work`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_of_a_new_slot_by_a_clone`)

#### Scenario: A clone of a free slot
- **WHEN** the get passed over a free slot
- **THEN** that slot is the template, marked `templating` while it is cloned, and free again afterwards
  [@test] crates/bd-lease/src/lease_test.rs (`a_new_slot_is_a_clone_of_a_free_slot`)

### Requirement: Get: A clone keeps only the ignored files of its template

The clone MUST get the `.git` file of a `worktree add --no-checkout --lock`, the template's index refreshed with
`core.checkStat=minimal`, then `checkout --force` to the source HEAD, the removal of the untracked files (the discard
below) and a clean `status`: the template's local changes, staged files and untracked files never reach the lease, its
ignored files do.

#### Scenario: The local work of the template
- **GIVEN** a template with local changes, staged files, untracked files and ignored files
- **WHEN** the get clones it
- **THEN** only its ignored files reach the lease
  [@test] crates/bd-lease/src/lease_test.rs (`a_new_slot_is_a_clone_of_the_source_without_its_local_work`)

#### Scenario: A prune while the clone is away
- **WHEN** a `worktree prune` runs beside the clone, the user's or a gc's
- **THEN** it never deletes the registration, which is unlocked once the clone is at its path
  [@test] crates/git-ops/src/lease_slot_test.rs (`a_prune_while_a_cloned_slot_is_away_keeps_its_registration_until_the_unlock`)

### Requirement: Get: A clone leaves out what the delete rules of the source delete

A clone MUST leave out the paths that the `delete` clean rules of the source delete, but for their `except` entries (a
rule below a kept entry applies too), a link among them as the link, and the `.git` of the template with them, so a new
slot starts as a returned one and a clone of a main checkout never copies its build output.

#### Scenario: The build output of a main checkout
- **WHEN** the get clones a main checkout whose `delete` rules name its build output (in idea-1 188 GB and 760,000
  files of `out/`, its `bazel-*` links into the output base of the main checkout among them)
- **THEN** the clone leaves those paths out
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_cloned_slot_leaves_out_what_the_delete_rules_delete`)

#### Scenario: The kept entries and the rules below them
- **WHEN** a left-out path has `except` entries, or a rule lies below a kept entry
- **THEN** the kept entries are cloned, and the rule below applies too
- **AND** the directories on the way to a left-out path are made anew and the rest of each is cloned entry by entry
  (`clonefile` with `CLONE_NOFOLLOW`); without such rules the whole tree is one `clonefile`
  [@test] crates/lease-pool/src/clone_dir_test.rs (`a_clone_leaves_out_the_skipped_paths_but_for_their_kept_entries`, `a_rule_below_a_kept_entry_applies_too`)

#### Scenario: A tracked file under a left-out path, and rules that can't be read
- **WHEN** a tracked file lies under a left-out path, or the rules file can't be read
- **THEN** the tracked file comes back with the checkout, and a rules file that can't be read clones everything

### Requirement: Get: A clone that fails falls back to a checkout

A clone that fails (another volume, no APFS, a file it can't read, not macOS) MUST leave nothing behind and fall back
to `git worktree add --no-checkout` and a full checkout (`created_by: "checkout"`).

#### Scenario: A failed clone
- **WHEN** the clone of a new slot fails
- **THEN** it leaves nothing behind and the slot is a full checkout
  [@test] crates/bd-lease/src/lease_test.rs (`a_clone_that_fails_falls_back_to_a_checkout`)

### Requirement: Get: A new slot keeps the name of the repository

A new slot MUST be `<pool>/<n>/<repository name>`, so the leaf directory keeps the name of the repository. The pool
directory MUST be `~/.branch-deck/pool/<repository name>-<8 hex digits of the SHA-256 of the canonical common dir>`,
or `branchdeck.leasePool` of the git config.

#### Scenario: The path of a new slot
- **WHEN** a get adds a slot
- **THEN** its path is `<pool>/<n>/<repository name>`
  [@test] crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)

#### Scenario: The name of the repository
- **WHEN** the pool names the repository
- **THEN** it takes the directory of its main worktree
  [@test] crates/lease-pool/src/lease_test.rs (`the_repository_name_is_the_directory_of_its_main_worktree`)

### Requirement: Get: Two gets never lease the same slot

Two gets at the same time MUST never lease the same slot: each records its lease as `preparing` under the pool lock
(`leases.lock`, not the repository lock), then moves its slot without the lock.

#### Scenario: Two gets at once
- **WHEN** two gets run at the same time
- **THEN** they lease different slots
  [@test] crates/bd-lease/src/lease_test.rs (`two_leases_never_get_the_same_slot`)

### Requirement: Get: A pool adds one slot at a time under the clones lock

A pool MUST add one slot at a time: a get that finds no free slot takes the clones lock (`clones.lock` beside the
state, an exclusive file lock), reserves again under it, and holds it until its new slot is checked out or given back:
the clone of a template or of the source, or the registration and the full checkout; never across the seed, a copy
that the lock does not guard. A get that reuses a slot MUST never take the lock.

#### Scenario: Two gets that must add a slot
- **WHEN** a get must add a slot while another adds one
- **THEN** it waits for the clones lock and reserves again under it, so a slot a return freed meanwhile wins over a
  new one (`reused: true`)
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_must_add_a_slot_waits_for_the_one_that_adds_a_slot`)
  [@test] crates/bd-lease/src/lease_test.rs (`two_leases_never_get_the_same_slot`)

#### Scenario: The seed runs without the lock
- **WHEN** the new slot is checked out
- **THEN** the get releases the clones lock before it seeds the slot
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_seeds_its_new_slot_without_the_clones_lock`)

#### Scenario: Fourteen gets at once
- **GIVEN** on 2026-10-24 fourteen gets cloned the idea-1 checkout at once
- **WHEN** they ran side by side
- **THEN** several ran past the deadline of their caller, and the disk stalled for 1.5 h

### Requirement: Get: A slot freed while a get waits for the clones lock ends the wait

A slot freed while a get waits for the lock (by a return, or a get or a clean whose process is gone) MUST end the wait
within about a second, while the holder still clones: the get reserves that slot without the lock. A slot the get
already tried (dirty, busy) MUST end no wait again: a dirty one freed meanwhile ends it once, and the get marks it dirty
and waits again.

#### Scenario: A slot freed during the wait
- **WHEN** a return frees a slot while a get waits for the clones lock
- **THEN** the get reserves that slot without the lock
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_slot_freed_while_a_get_waits_for_the_clones_lock_ends_the_wait`)

#### Scenario: A dirty slot freed during the wait
- **WHEN** a dirty slot is freed while a get waits
- **THEN** it ends the wait once, and the get marks it dirty and waits again
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_dirty_slot_freed_while_a_get_waits_ends_the_wait_once`)

### Requirement: Get: A running return or clean keeps a get from adding a slot

Under the clones lock, with no free slot, a slot whose return or clean runs (a live process, `returning` or `cleaning`)
MUST NOT make the get add a slot: the get says so (`waiting for the return of <slot> (pid <pid>)`) and waits until that
return or clean ends, then reserves again: the slot it freed, else (the return failed, its process died) a new one. It
waits for each return or clean once.

#### Scenario: A running return
- **WHEN** a get finds no free slot and a return runs on a slot
- **THEN** it waits for the return and reserves the slot it freed
  [@test] crates/bd-lease/src/lease_test.rs (`a_running_return_reserves_the_slot_against_a_get`)

#### Scenario: A running clean
- **WHEN** `brd lease clean` cleans a slot
- **THEN** the slot is reserved against a get
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_slot_that_clean_cleans_is_reserved_against_a_get`)

#### Scenario: A failed return
- **WHEN** the return the get waited for failed, or its process died
- **THEN** the get adds a new slot
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_waited_for_a_failed_return_adds_a_slot`)

### Requirement: Get: A get that waits for the clones lock tells it

A get that waits MUST say so on stderr when the wait starts, when the holder or the slot it adds changes and once a
minute, naming the holder's command, pid and how long it holds the lock, and the new slot and the holder of its lease
once it reserved one, and MUST print how long it waited (`waited_for_clone_ms`, omitted when it waited for none).

#### Scenario: The messages of a wait
- **WHEN** a get waits for the clones lock
- **THEN** stderr names the holder's command, pid and how long it holds the lock, at the start, on a change and once a
  minute
- **AND** the answer prints `waited_for_clone_ms`
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_must_add_a_slot_waits_for_the_one_that_adds_a_slot`)

### Requirement: Get: The wait for the clones lock lasts while its holder lives

The wait MUST last while the record of the holder (`clones.lock.holder`, as the repository lock writes it) names a live
process: a holder that is killed frees the lock with its process, on every platform; a lock no record names a live
holder of MUST be `locked` after the lock timeout.

#### Scenario: A killed holder
- **WHEN** the holder of the clones lock is killed
- **THEN** the lock is free with its process
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_killed_holder_frees_the_clones_lock_with_its_process`)

#### Scenario: A lock without a live holder
- **WHEN** no record names a live holder of the lock
- **THEN** the get is `locked` after the lock timeout

### Requirement: Get: The locks of the pool are taken in one order

The order of the locks MUST be `clones.lock`, `leases.lock`, `worktrees.lock`; a return and a clean MUST never take
the clones lock.

#### Scenario: A return beside a get that adds a slot
- **WHEN** a return or a clean runs while a get holds the clones lock
- **THEN** it does not wait for the clones lock

### Requirement: Get: A get prints each step on stderr as it starts

A get MUST print each step on stderr as it starts (`brd lease get: <step>`): the reserve of a free slot, under the
clones lock the reserve of a slot freed meanwhile or a new one, the wait for the clones lock, the stop of the Bazel
servers, the `status` of a slot, the wait for a slot whose return or clean runs, the delete of the trash, the clone of
a template, the refresh of its index, the checkout and the seed; the JSON goes to stdout at the end.

#### Scenario: The steps of a get
- **WHEN** a get runs
- **THEN** its steps arrive on stderr in order and the JSON on stdout at the end
- **AND** the core tells the same steps to its caller (`LeaseGetParams::progress`)
  [@test] crates/bd-lease/src/clones_lock_test.rs (`the_steps_of_a_get_arrive_in_order`)

### Requirement: Get: The worktree commands of the pool run one at a time

The `worktree` commands of the pool (the registration of a new slot, its unlock, the move of an adopt) MUST run one at
a time, under `worktrees.lock` beside the state, and without the pool lock, which no git holds for a get: git writes
the admin directory of a new worktree in several files, and a `worktree` command beside it reads them half written (an
empty `commondir`) and fails.

#### Scenario: A worktree command beside the registration of a new slot
- **WHEN** a get registers a new slot with `worktree add --no-checkout` and another `worktree` command of the pool runs
- **THEN** that command waits for `worktrees.lock`, and the slot gets its files after the lock, by a checkout or a
  clone
- **AND** two gets never register beside each other: the clones lock comes first
  [@test] crates/bd-lease/src/lease_test.rs (`two_leases_never_get_the_same_slot`, `no_worktree_command_of_the_pool_runs_beside_the_registration_of_a_new_slot`)

#### Scenario: An adopt that can't wait for the lock
- **WHEN** the move of an adopt can't wait for the lock
- **THEN** it is refused, naming it

### Requirement: Get: A get runs no worktree prune

A get MUST run no `worktree prune`: run beside the `worktree add` of another get, it deletes the admin directory that
add has just made, before git locks it, and that get fails. A new slot MUST be registered with `worktree add --force`,
which drops only a registration of its own path whose directory is gone (a slot deleted by hand).

#### Scenario: A slot deleted by hand
- **WHEN** a slot was deleted by hand
- **THEN** the next new slot takes that path
  [@test] crates/bd-lease/src/lease_test.rs (`a_slot_deleted_by_hand_gives_its_path_to_the_next_new_slot`)
  [@test] crates/git-ops/src/lease_slot_test.rs (`a_new_slot_takes_over_the_registration_of_a_slot_deleted_by_hand`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_of_a_new_slot_by_a_clone`)

### Requirement: Get: A get and a return take no repository lock

A get and a return MUST take no repository lock: a sync of the repository beside them (another agent's `brd sync`,
the app's) neither waits for them nor holds them, and the lease is cut at the HEAD that sync reads.

#### Scenario: A held sync
- **WHEN** a sync holds the repository beside a get and a return
- **THEN** they do not wait for it
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_and_a_return_beside_a_held_sync_do_not_wait_for_it`)

### Requirement: Get: The branch option creates the branch in the slot

`--branch <name>` (`-b`) MUST create the branch at the source HEAD and check it out in the slot; a branch that exists
MUST be `branch_exists` (exit 4), an invalid name `invalid_arguments` (exit 2).

#### Scenario: A lease on a new branch
- **WHEN** a get runs with `-b` and a new name
- **THEN** the slot has the branch checked out at the source HEAD, and the branch stays after the return
  [@test] crates/bd-lease/src/lease_test.rs (`a_lease_on_a_new_branch_keeps_the_branch_after_the_return`)

#### Scenario: A branch that exists or an invalid name
- **WHEN** the branch exists, or its name is invalid
- **THEN** the get fails with `branch_exists` (exit 4), or `invalid_arguments` (exit 2)

### Requirement: Get: A get seeds the ignored files that worktreeinclude names

The get MUST copy into the slot the ignored files and directories that `.worktreeinclude` at the root of the source
names (`#` comments; a literal path, or a glob of `*`, `?` and `[…]` in its last component; always relative to the
root), and record them in the lease (`seeded`). A tracked file or one that git does not ignore MUST NOT be seeded.

#### Scenario: The seed and the return
- **WHEN** `.worktreeinclude` names ignored files
- **THEN** the get copies them and records them in `seeded`, and the return removes them
  [@test] crates/bd-lease/src/lease_test.rs (`the_seed_copies_the_included_ignored_files_and_the_return_removes_them`)

#### Scenario: Literal paths and globs
- **WHEN** a line is a literal path, or a glob of `*`, `?` and `[…]` in its last component
- **THEN** it names the paths it matches in that one directory
  [@test] crates/lease-pool/src/lease_test.rs (`the_seed_candidates_are_literal_paths_and_globs_of_one_directory`, `a_glob_matches_by_stars_question_marks_and_classes`)

### Requirement: Get: A failed get gives the slot back

A get that fails after it recorded the lease MUST give the slot back (free, or dropped when it was never added) and say
so in the message; it MUST never leave the lease `preparing`. A `preparing` lease whose process (its pid and start
time) is gone MUST be taken as free by the next get, on Windows too, which has no process scan but tells the start of a
process.

#### Scenario: A get that fails
- **WHEN** a get fails after it recorded the lease
- **THEN** the slot is free again, or dropped when it was never added, and the message says so
  [@test] crates/bd-lease/src/lease_test.rs (`a_failed_get_rolls_the_lease_back`)

#### Scenario: A preparing lease of a gone process
- **WHEN** the process of a `preparing` lease is gone
- **THEN** the next get takes the slot as free
  [@test] crates/bd-lease/src/lease_test.rs (`a_preparing_lease_of_a_gone_process_is_taken_by_the_next_get`)

### Requirement: Get: A get of a free slot runs a fixed number of git processes

A get of a free slot MUST run a fixed number of git processes: the repository, the source HEAD, the pool config, one
`status` and the HEAD of the slot, plus one `reset` when the slot is at another commit.

#### Scenario: The git of a get and a return
- **WHEN** a get leases a free slot and its return runs
- **THEN** they run the fixed processes
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_and_return`)

### Requirement: Get: The process scan finds a process by its directory

The process scan MUST find a process by its working directory, compared by path components, or by the
`--workspace_directory` of a Bazel server, and leave out the session that runs `brd`: `brd`, its ancestors and every
process of a process group one of them is in (not the group of the init process). A recorded start time MUST tell a
process from a later one with the same pid.

#### Scenario: A process in a directory and this process
- **WHEN** the scan looks into a directory
- **THEN** a process whose working directory is inside it is found, and this process and its group are left out
- **AND** a process runs only with the start time it was recorded with
  [@test] crates/sync-utils/src/process_scan_test.rs (`a_process_in_a_directory_is_inside_it_and_this_process_and_its_group_are_left_out`, `a_process_runs_only_with_the_start_time_it_was_recorded_with`)

#### Scenario: The session of brd
- **WHEN** a process is in a group of `brd` or one of its ancestors (the `jq` of `brd lease return | jq`, the
  `caffeinate` of the agent)
- **THEN** it does not keep the slot busy
- **AND** a background task of the agent leads a group of its own and still counts
  [@test] crates/bd-lease/src/lease_test.rs (`a_process_of_the_group_of_bd_does_not_keep_the_slot_busy`)

### Requirement: Get: Background git, the fsmonitor daemon and caffeinate do not work in a slot

Three more processes MUST never count as working in a slot: git's background maintenance (a `git maintenance` or a
`git gc --auto` and every git below it in the parent chain, told by argv), which writes only the shared object store;
git's fsmonitor daemon (`git fsmonitor--daemon`, told by argv), which only watches the worktree; and `caffeinate`,
which holds a power assertion and opens no file.

#### Scenario: The ancestors, their groups and the three
- **WHEN** the scan finds git's maintenance, the fsmonitor daemon or `caffeinate` in a directory
- **THEN** they do not work in it, and a command `caffeinate` wraps is a process of its own and counts
  [@test] crates/sync-utils/src/process_scan_test.rs (`the_session_is_the_ancestors_and_their_groups`, `git_maintenance_the_fsmonitor_daemon_and_caffeinate_do_not_work_in_a_directory`)

### Requirement: Return: A return frees the lease of the worktree

`brd lease return` MUST free the lease of the worktree of `--workspace` (default: of `-C`, else of the current
directory): remove the seeded paths (`removed`, each moved into the trash as a whole), detach HEAD when it is on a
branch (`detached_branch`, the branch stays) and leave the slot at its commit.

#### Scenario: The seeded paths go
- **WHEN** a lease with seeded paths is returned
- **THEN** the return removes them and names them in `removed`
  [@test] crates/bd-lease/src/lease_test.rs (`the_seed_copies_the_included_ignored_files_and_the_return_removes_them`)

#### Scenario: A lease on a branch
- **WHEN** the HEAD of the returned slot is on a branch
- **THEN** the return detaches it (`detached_branch`) and the branch stays
  [@test] crates/bd-lease/src/lease_test.rs (`a_lease_on_a_new_branch_keeps_the_branch_after_the_return`)

### Requirement: Return: A lease id returns the slot that holds that lease

`--lease-id` without `--workspace` MUST return the slot that holds that lease when the worktree of `-C` (or of the
current directory) holds no lease or another one, so a coordinator returns the lease of another session from the
shared checkout; an id that no slot holds MUST keep the refusals below.

#### Scenario: A return by lease id from another checkout
- **WHEN** a coordinator runs `brd lease return --lease-id <id>` in the shared checkout
- **THEN** the slot that holds that lease is returned
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_by_lease_id_from_another_checkout_returns_the_slot_of_the_lease`)

#### Scenario: An id that no slot holds
- **WHEN** no slot holds the lease id
- **THEN** the refusals below apply
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot`)

### Requirement: Return: A return refuses what the slot still holds

The return MUST refuse, with nothing changed: a worktree without a lease (`no_lease`, exit 2), another `--lease-id`
(`lease_mismatch`, exit 2), a process of another session in the worktree (`slot_busy`, exit 4), HEAD with commits
that no branch, tag or remote branch holds (`unlanded_commits`, exit 4), an open resolve session of the slot
(`resolve_in_progress`, exit 4), and local changes (`dirty_worktree`, exit 4, `paths`).

#### Scenario: A busy slot
- **WHEN** a process of another session works in the worktree
- **THEN** the return refuses with `slot_busy`, `processes` [{`pid`, `command`, `argv` (at most 200 characters),
  `kind`: `bazel_server` or `other`, `workspace`?}]
- **AND** the message names each with its argv, so no `ps` is needed to tell them

#### Scenario: Unlanded commits
- **WHEN** HEAD has commits that no branch, tag or remote branch holds
- **THEN** the return refuses with `unlanded_commits` and `commits`, the message naming `brd -C <dir> lease pick`

#### Scenario: An open resolve session
- **WHEN** the slot has an open resolve session
- **THEN** the return refuses with `resolve_in_progress`, as the next lease of the slot would find it
- **AND** the message names `brd update --continue` and `--abort`, and `--confirm-preserved` does not pass it

### Requirement: Return: The commits of the leased commit never count as unlanded

The commits of the commit the slot was leased at MUST never count as unlanded, also after a rewrite of master made them
unreachable.

#### Scenario: Master rewritten past the leased commit
- **GIVEN** a rewrite of master made the commit the slot was leased at unreachable
- **WHEN** the lease is returned
- **THEN** its commits do not count as unlanded
  [@test] crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)

### Requirement: Return: The unlanded check asks whether the source branch has every change

The question of the unlanded check is whether the branch of the source has every change of the slot. The get MUST
record the source checkout and its branch (none for a detached HEAD). When reachability finds commits, each MUST be
proven on the tip of that branch, on git objects only, without the lock and without the index of the shared checkout
(`lease_pool::landed_work`, the one rule of the return, the takeover and the reclaim), and only the commits no proof
finds are unlanded.

#### Scenario: A follow and a rewrite of master
- **WHEN** the work of the lease landed by a follow and master was rewritten
- **THEN** the return finds the work landed by content
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_after_a_follow_and_a_rewrite_of_master_finds_the_work_landed_by_content`)

#### Scenario: Only the commit the source branch lacks
- **WHEN** one commit of the lease is not on the source branch
- **THEN** the return refuses only that commit
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_refuses_only_the_commit_that_the_source_branch_lacks`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot`)

#### Scenario: The git of a return proven by the ladder
- **WHEN** the commit of a lease landed and a proof finds it
- **THEN** the return runs the git of the proofs
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_return_whose_commit_landed_by_the_ladder`)

### Requirement: Return: The proofs of the unlanded check

Each proof MUST run for the commits the one before left, in this order: the dry run of the pick, the patch id, the
same identity, the twin of `brd update`, the same author and subject, the folded fixup, the mapping note of a sync.

#### Scenario: The dry run of the pick
- **WHEN** a commit is of the range of a dry run of the pick (the fork point in the reflog of the branch, `git replay`
  onto its tip) that it would not pick
- **THEN** it is landed
  [@test] crates/git-ops/src/pick_test.rs (`a_dry_run_names_its_range_and_the_commits_it_would_pick`)
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)

#### Scenario: A commit outside the range of the pick
- **WHEN** a fast-forward that a reset took back leaves the commit as its own fork point
- **THEN** the commit is outside the range of the pick and no commit the pick proved
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_after_a_pick_that_rolled_back_refuses_the_commit`, `a_lease_whose_home_was_reset_past_its_commits_is_not_landed`)

#### Scenario: The patch id
- **WHEN** a commit of the branch's side has the patch id of the commit
- **THEN** it is landed
  [@test] crates/bd-lease/src/lease_test.rs (`a_session_rewords_its_landed_commit_from_its_lease`, `a_pick_without_follow_finds_the_reworded_commit_present_by_patch_id`)

#### Scenario: The same identity
- **WHEN** a commit of the branch of the same identity (author, author time, subject without the prefix) is its
  cherry-pick or changes the same lines
- **THEN** it is landed
  [@test] crates/git-ops/src/landed_pairing_test.rs (`interdiffs_compare_multiple_local_commits_to_the_same_upstream`)

#### Scenario: The twin of brd update
- **WHEN** a commit of the branch of the same identity has every changed line of the commit with its fixups (a
  conflict resolution, an autosquash), or every line it adds while the lines it removes and the twin does not are in no
  line of the file of the twin's parent (a removal of lines that another commit had rewritten or removed already:
  nothing is lost)
- **THEN** it is the twin of `brd update`, and the commit is landed
- **AND** a file there that is gone, binary or over 8 MiB proves nothing
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_finds_a_commit_that_landed_with_a_conflict_resolution`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_finds_a_commit_whose_twin_lacks_only_a_removal_master_made_before`)

#### Scenario: The same author and subject at another author time
- **WHEN** a commit of the branch of the same author and subject at another author time (made anew, a rebase that
  reset the author; the three newest) has a change that passes that twin check
- **THEN** it is landed, the subject only picking it
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_finds_a_commit_made_anew_by_its_author_and_subject`)

#### Scenario: A folded fixup
- **WHEN** a remaining `fixup! <subject>` has a newest matching subject commit on the branch after the lease source
  head that has every changed line of the fixup
- **THEN** it is landed as `folded_fixup`, as `brd fold` leaves it
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_counts_two_folded_fixups_as_landed_without_confirmation`, `a_folded_fixup_stays_unlanded_when_its_target_lacks_a_changed_line`, `a_folded_fixup_proof_is_named_in_the_return_output`)
  [@test] crates/lease-pool/src/landed_work.rs (`newest_target_that_holds_every_changed_line_proves_a_folded_fixup`, `a_target_missing_a_changed_line_does_not_prove_a_folded_fixup`, `an_older_target_with_the_fixup_line_does_not_override_a_newer_target`)

#### Scenario: The mapping note of a sync
- **WHEN** the mapping note of a sync names a virtual copy of the commit on the branch, or one proven there so
- **THEN** it is landed
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_finds_a_commit_that_landed_by_its_virtual_copy`)

### Requirement: Return: Reachability decides alone where there is nothing to prove

Reachability MUST decide alone when it finds nothing (the fast path), when the source had no branch and when the branch
is gone; the output MUST say which check decided (`landed_check`, `landed_branch`) and name each commit the proofs
found landed with its proof (`landed`), and the refusal MUST say which check refused.

#### Scenario: The fast path, no branch and a branch that is gone
- **WHEN** reachability finds nothing, the source had no branch or the branch is gone
- **THEN** reachability decides alone, and `landed_check` says so

#### Scenario: The output of the proofs
- **WHEN** the proofs found commits landed
- **THEN** `landed` names each with its proof, and `landed_branch` the branch

### Requirement: Return: A return stops the Bazel servers of the slot

The return MUST find the Bazel servers of the slot (a process whose argv has `--workspace_directory=<dir>`, `<dir>` the
slot or inside it, such as `<slot>/community`), ask each to exit with SIGTERM once nothing else refuses, wait up to
30 s, and name them in `stopped_servers` (pid, workspace, output base). There MUST be no flag to keep a server: with
one, a return under a running server could never pass, the trap agents fell into.

#### Scenario: A server of the slot
- **WHEN** a Bazel server of the slot runs and nothing else refuses
- **THEN** the return stops it with SIGTERM and names it in `stopped_servers`
- **AND** the output base stays on disk: Bazel derives it from the workspace path, so a server serves only its slot,
  and the next lease of the slot gets the same one, with its disk action cache
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_stops_the_bazel_servers_of_the_slot`)
  [@test] crates/sync-utils/src/process_scan_test.rs (`the_argv_comes_from_the_procargs_buffer_without_the_environment`)

### Requirement: Return: A return signals no other process

The return MUST NOT signal any other process: any other process in the slot refuses as `slot_busy` before a server is
signalled, and the message says that the return stops the servers itself. A server that does not exit MUST keep the
slot busy (`slot_busy`, the message saying it did not exit on SIGTERM, on `bazel shutdown` on Windows).

#### Scenario: Another process and a server that does not exit
- **WHEN** another process is in the slot, or a server does not exit within 30 s
- **THEN** the return refuses as `slot_busy`, and no other process is signalled
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_signals_no_other_process_and_a_server_that_does_not_exit_keeps_the_slot_busy`)

### Requirement: Return: Without a process scan the servers are found by their output base

Where no process scan runs (Windows), the return, the get of a free slot, the takeover and the reclaim MUST find the
Bazel servers of the slot by their output base; other processes MUST stay unchecked (`processes_checked: false`).

#### Scenario: A server told by its output base
- **GIVEN** a workspace that is the slot or a directory right in it with `MODULE.bazel`, `WORKSPACE.bazel` or
  `WORKSPACE`
- **WHEN** its `bazel-out` link (`<ws>/bazel-out`, or `<ws>/out/bazel-out` with `--symlink_prefix=out/bazel-`; a
  junction or a symlink, never a directory) names `<output base>/execroot/<name>/bazel-out`
- **AND** the pid in `<output base>/server/server.pid.txt` runs and started before the file was written
- **THEN** that pid is a server of the slot
  [@test] crates/lease-pool/src/bazel_servers_test.rs (`a_server_is_told_by_the_pid_file_of_the_output_base_its_bazel_out_link_leads_to`)

#### Scenario: No running pid or no link
- **WHEN** the pid is gone, or the system gave it to a later process, or the link of a workspace is missing or named
  otherwise
- **THEN** there is no server, and a server of that workspace keeps running
  [@test] crates/lease-pool/src/bazel_servers_test.rs (`no_server_is_told_without_a_running_pid_of_the_pid_file`)

### Requirement: Return: A server told by its output base is asked to shut down

Each server told by its output base MUST be asked to exit with `bazel --output_base=<output base> shutdown` in its
workspace, without a console window and killed after the 30 s, the shutdowns of the servers of the slot side by side,
each with the whole 30 s, then waited for as SIGTERM is elsewhere; the pid itself MUST never be signalled.

#### Scenario: A server that exits
- **WHEN** a server exits on its shutdown
- **THEN** `stopped_servers` names it (pid, workspace, output base)
  [@test] crates/lease-pool/src/bazel_servers_test.rs (`the_stop_of_a_server_told_by_its_output_base_names_it_stopped`)

#### Scenario: A server that does not exit
- **WHEN** a server does not exit when asked
- **THEN** it is not stopped and keeps the slot busy
  [@test] crates/lease-pool/src/bazel_servers_test.rs (`a_server_that_does_not_exit_when_asked_is_not_stopped`)

#### Scenario: Several servers
- **WHEN** the slot has several servers
- **THEN** their shutdowns run side by side
  [@test] crates/lease-pool/src/bazel_servers_test.rs (`the_bazel_shutdowns_of_the_servers_run_side_by_side`)

### Requirement: Return: Bazel is found as cmd finds it

`bazel` MUST be found as `cmd` finds it: by the directories of the `PATH` and, on Windows, the extensions of `PATHEXT`
(`bazel.exe`, a `bazel.cmd` of an npm Bazelisk, a `bazel.bat`), which `CreateProcess` alone does not try.

#### Scenario: A bare program
- **WHEN** the stop runs `bazel`
- **THEN** it is found by the path and the extensions of `PATHEXT`
  [@test] crates/lease-pool/src/bazel_servers_test.rs (`a_bare_program_is_found_by_the_path_and_the_extensions_of_pathext`, `the_stop_of_a_server_told_by_its_output_base_names_it_stopped`)

### Requirement: Return: A bazel shutdown that can't run or fails is not waited for

A `bazel shutdown` that can't run (no `bazel` there, a program that does not start) or that fails (a non-zero exit,
killed at the timeout) MUST NOT be waited for: its server, unless it exited anyway, keeps the slot busy as one that
does not exit. Its pid MUST still never be signalled.

#### Scenario: A shutdown that cannot run or fails
- **WHEN** the shutdown of a server cannot run or fails
- **THEN** its process in the refusal (`slot_busy` of a return; the `busy` skip of a get or a reclaim) has
  `stop_error`, the error naming the program, which the message words too
  [@test] crates/lease-pool/src/bazel_servers_test.rs (`a_bazel_shutdown_that_cannot_run_or_fails_says_why_and_names_the_program`, `a_server_whose_bazel_shutdown_cannot_run_or_fails_is_named_running_with_why`)

### Requirement: Return: Background maintenance of git keeps no slot busy

git's background maintenance in the slot MUST keep neither a return nor a get from it, and the return names its pids
(`maintenance`): a commit in a slot starts `git maintenance run --auto --detach`, whose repack keeps the slot as its
working directory for minutes in a large repository (2 to 17 minutes in idea-1) and writes only the shared object
store.

#### Scenario: A repack in the slot
- **WHEN** a `git maintenance run --auto --detach` runs in the slot
- **THEN** the slot is not busy, and the return names its pids in `maintenance`
  [@test] crates/bd-lease/src/lease_test.rs (`git_maintenance_keeps_no_slot_busy_and_is_named`)

### Requirement: Return: The detach of HEAD is retried while gc holds HEAD.lock

While a `gc --auto` holds `HEAD.lock` of the slot (`pack-refs`, `reflog expire`), the detach of HEAD MUST be retried
for up to 10 s; the lock MUST be told by its file, never by the words of git's error.

#### Scenario: A gc holds HEAD.lock
- **WHEN** a `gc --auto` holds `HEAD.lock` of the slot as the return detaches HEAD
- **THEN** the return retries the detach for up to 10 s

### Requirement: Return: The return starts the maintenance the git config asks for

With `branchdeck.leaseMaintenance` true in the git config, the return MUST start `git maintenance run --auto --detach`
in the common dir once the slot is free, before it empties the trash, and say so (`maintenance_started`); a failed
start never fails the return. Without the setting the return MUST start none.

#### Scenario: A return with the setting
- **GIVEN** `branchdeck.leaseMaintenance` true, for a repository whose `maintenance.auto` is false, so that no commit
  or fetch in a slot starts a repack while a session works there (idea-1)
- **WHEN** a return frees the slot
- **THEN** it starts the maintenance in the common dir, and the repack runs between leases, only when its thresholds
  passed (`--auto`)
- **AND** the detached run keeps no slot as its working directory
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_starts_the_maintenance_that_branchdeck_lease_maintenance_asks_for`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_return_that_starts_the_maintenance`)
  [@test] crates/sync-utils/src/process_scan_test.rs (`git_maintenance_is_the_maintenance_and_the_gits_it_starts`)

### Requirement: Return: A return reserves the slot before it changes anything

Once its checks passed, the return MUST reserve the slot before it changes anything: under the pool lock, with CAS on
the lease id, the lease becomes `returning`, and a get MUST pass the slot over. A failed return gives the lease back as
`ready`; a `returning` lease whose process is gone is abandoned.

#### Scenario: A get beside a running return
- **WHEN** a get runs while a return acts on the slot
- **THEN** the get passes the slot over
  [@test] crates/bd-lease/src/lease_test.rs (`a_running_return_reserves_the_slot_against_a_get`)

### Requirement: Return: A repeated return joins the first

A repeat MUST join, never fail: a second return of the lease meanwhile waits for the first and answers `joined: true`
once it freed the slot (it returns the lease itself once the first failed or its process died); a return whose lease
another return gave back meanwhile answers `joined: true` and MUST change nothing of the next lease.

#### Scenario: A retry while the first acts on the slot
- **WHEN** a return is retried while the first acts on the slot
- **THEN** it waits for the first and answers `joined: true`
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_retried_while_the_first_acts_on_the_slot_waits_for_it_and_answers_joined`)

#### Scenario: A return that a retry overtook
- **WHEN** another return gave the lease back meanwhile (a get may have leased the slot again)
- **THEN** the return answers `joined: true` and changes nothing of the next lease, also when one of its checks refused
  the next holder's slot
- **AND** a `joined` answer has empty lists, and no `landed_check` when the return checked nothing
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_that_a_retry_overtook_answers_joined_and_leaves_the_next_lease_of_the_slot_alone`)

### Requirement: Return: Confirm preserved gives up the commits and discards the changes

`--confirm-preserved` MUST give up the unlanded commits (`given_up`) and discard the local changes (`discarded`) with
`reset --hard HEAD` and the removal of the untracked files: the ignored files stay. The slot MUST hold its commit's tree
and what the commit's own ignore rules ignore, so an untracked `.gitignore` goes before the `clean -fd`, which would
delete it and keep the file it hid.

#### Scenario: A confirmed return
- **WHEN** a return refused for unlanded commits or local changes runs again with `--confirm-preserved`
- **THEN** it gives up the commits and discards the changes, the ignored files staying
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot`)

#### Scenario: An untracked gitignore
- **WHEN** an untracked `.gitignore` hides a file
- **THEN** the discard removes the file it hid too
  [@test] crates/bd-lease/src/lease_test.rs (`a_confirmed_return_discards_the_file_an_untracked_gitignore_hid`)

### Requirement: Return: Every discard of a slot removes the untracked files in passes

Every discard of a slot (a return, a takeover, a reclaim, the give-back of a host slot) and the clean of a clone MUST
remove the untracked files so: a `status` in the untracked mode of the repository lists them, with none no `clean -fd`
runs, and after a `clean -fd` one more `status` MUST find none (at most four passes).

#### Scenario: The clean of a clone
- **WHEN** a template has an untracked `.gitignore`
- **THEN** the clone is clean
  [@test] crates/bd-lease/src/lease_test.rs (`a_clone_of_a_template_with_an_untracked_gitignore_is_clean`)

#### Scenario: The git of a discard
- **WHEN** a host slot is given back, or a new slot is cloned
- **THEN** the discard runs its `status` and runs no `clean -fd` when it lists none
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_host_slots_lend_give_back_and_shrink`, `a_lease_get_of_a_new_slot_by_a_clone`)

### Requirement: Pick: A lease pick lands the commits on the home branch and follows

`brd lease pick` inside a lease MUST be `brd pick --from <lease> --follow` run in the `home.checkout` of the lease, onto
`home.branch`: the commits the lease has and the branch lacks replayed onto its tip as it is then (whatever other
sessions landed meanwhile; no test runs again), and the lease moved onto the new tip, its answer that of `brd pick`
(`state`, `picked`, `followed`, `undo_token`).

#### Scenario: Commits to land
- **WHEN** `brd lease pick` runs in a lease with commits its home branch lacks
- **THEN** they are replayed onto the tip of `home.branch` as it is then, and the lease follows onto the new tip
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_lands_the_commits_on_the_home_branch_and_the_lease_follows`)

#### Scenario: Nothing to land
- **WHEN** the lease has nothing to land
- **THEN** the lease only follows (`state: up_to_date`, `followed.state: moved`), so it gets what the other sessions
  landed

#### Scenario: A dry run
- **WHEN** `brd lease pick` runs with --dry-run
- **THEN** it writes nothing but git objects

### Requirement: Pick: The home branch is checked under the repository lock

The branch MUST be checked under the repository lock with the rest of the pick's witness: when the home checkout has
another branch checked out, nothing lands, `home_branch_switched` (exit 4) names both branches, and the agent asks the
user. A lease cut from a detached HEAD has no branch to land on: `no_home_branch` (exit 4). Outside a leased slot:
`no_lease` (exit 2).

#### Scenario: A home that switched its branch
- **WHEN** the home checkout has another branch checked out
- **THEN** nothing lands and `home_branch_switched` names both branches
- **AND** inside the lease `brd workspace` shows the switch (`home.switched_to`)
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_home_that_switched_its_branch`)

#### Scenario: No home branch, or no lease
- **WHEN** the lease was cut from a detached HEAD, or `brd lease pick` runs outside a leased slot
- **THEN** it is refused with `no_home_branch` (exit 4), or `no_lease` (exit 2)
  [@test] crates/bd-lease/src/lease_pick_test.rs (`the_get_names_the_home_of_the_lease_and_the_commands_of_the_session`, `a_lease_pick_outside_a_lease_is_refused`)

### Requirement: Pick: A conflict lands nothing and comes back to the lease

A conflict MUST land nothing (exit 1, `state: conflict`) and come back to the lease: the lease is updated onto the tip
of its home branch without a fetch (`update`, what `brd update` prints, [brd update](../update/spec.md)), and the
conflict there opens a resolve session in the lease. While the session is open a `brd lease pick` MUST refuse
(`resolve_in_progress`, exit 4). The output is `lease_pick` of `brd schema`.

#### Scenario: A pick that conflicts
- **WHEN** the commits of the lease conflict with the tip of the home branch
- **THEN** nothing lands, and a resolve session opens in the lease, `conflict.next` saying so
- **AND** the lease moves only when `brd update --continue` finishes it, whose `next` is `brd -C <lease> lease pick`
  again, and `brd update --abort` closes it
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_pick_that_conflicts`)

#### Scenario: An update that applies at once
- **WHEN** the update applies at once (the words resolved the conflict)
- **THEN** it is picked again at once

### Requirement: Pick: A lease pick runs the git of the pick

Its git processes MUST be those of the pick (`a_pick`); the lease itself comes from the state of the pool, without git,
and one `show-ref` asks whether its resolve session is open.

#### Scenario: The git of a lease pick
- **WHEN** `brd lease pick` runs
- **THEN** it runs the git of the pick and one `show-ref`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_pick`)

### Requirement: Pick: A repository may require a green check of the tree

A repository whose committed `.branchdeck.toml` sets `pick_requires_check = true` in `[lease]` (false by default) MUST
land only a tree that a green build or test ran on. The gate is the pick's ([brd pick](../pick/spec.md#the-check-of-the-tree)):
`brd pick --from <lease>`, what `brd lease pick` runs, passes it too, so no path around it lands an unchecked tree.

#### Scenario: The setting of the lease table
- **WHEN** `.branchdeck.toml` sets `pick_requires_check` in `[lease]`
- **THEN** it is a boolean, false by default
  [@test] crates/lease-pool/src/check_record_test.rs (`pick_requires_check_is_a_boolean_of_the_lease_table`)

### Requirement: Pick: A check tool records its green runs in the git dir

A check tool (`bt` of the IntelliJ monorepo) appends one JSON line per green run to `bd-checks.jsonl` in the git dir
of the worktree (`git rev-parse --git-path`): `{"schema":1,"tree","head","tool","command","patterns","exit":0,"at"}`,
with `host` for a run on another machine. The tree is `HEAD^{tree}` of a clean worktree, else the tree of the worktree
with its local changes. brd MUST only read the file.

#### Scenario: Records that do not count
- **WHEN** a line is of another schema, has an `exit` other than 0 or does not parse
- **THEN** it counts as no record
  [@test] crates/lease-pool/src/check_record_test.rs (`only_green_records_of_schema_one_count`)

### Requirement: Pick: The pick reads the setting from the home checkout

The pick MUST read the setting from the home checkout, the file of the branch the commits land on, as the return reads
its clean rules; a file that does not parse is `invalid_arguments` (exit 2).

#### Scenario: A file that does not parse
- **WHEN** `.branchdeck.toml` of the home checkout does not parse
- **THEN** the pick fails with `invalid_arguments` (exit 2)

### Requirement: Pick: A tree no record names lands nothing

When no record names `HEAD^{tree}` of the lease, nothing MUST land: `unchecked_tree` (exit 4), whose message names the
tree and HEAD, the last check (its tree, time and command, how many files changed since and the first 10 of them, left
out when git no longer has that tree) and both ways on.

#### Scenario: An unchecked tree
- **GIVEN** `pick_requires_check = true`
- **WHEN** no record names `HEAD^{tree}` of the lease
- **THEN** the pick refuses with `unchecked_tree` and names the last check
- **AND** the message names both ways on: build and test, then `brd -C <lease> lease pick` again, or
  `brd -C <lease> lease pick --allow-unchecked '<reason>'`
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_tree_that_no_green_check_recorded`)

### Requirement: Pick: The pick expects the HEAD it checked

The pick MUST expect the lease's HEAD to be the commit it checked (`expect_source_head`, held under the lock by the
recheck of the pick): a lease that moved between the check and the lock is `head_moved` (exit 3), and the pick runs
again.

#### Scenario: A lease that moved after the check
- **WHEN** the lease moved between the check and the lock
- **THEN** the pick is refused with `head_moved` (exit 3)
  [@test] crates/git-ops/src/pick_test.rs (`a_source_that_moved_from_the_expected_head_is_refused`)

### Requirement: Pick: The gate stops a stale claim, not a merge break

The gate MUST stop a stale claim, a check that ran before the last edit; it does not stop a merge break, and it does
not judge coverage: any green build or test of the tree counts, whatever it built.

#### Scenario: A pick onto a tip that moved
- **WHEN** a pick lands onto a tip that moved, without a conflict
- **THEN** it lands a tree that no record saw

### Requirement: Pick: Allow unchecked lands the tree without a check

`--allow-unchecked <reason>` MUST land the tree without a check: the reason is required (a blank one is
`invalid_arguments`), the output repeats it (`unchecked`) and stderr names it. `--unchecked`, its name before, MUST be
the same flag. Without the setting the pick reads no record and prints no `check`.

#### Scenario: A pick with a reason
- **WHEN** `brd lease pick --allow-unchecked <reason>` runs
- **THEN** the tree lands without a check, the output repeats the reason in `unchecked` and stderr names it
  [@test] crates/bd-lease/src/lease_pick_test.rs (`allow_unchecked_lands_without_a_check_and_says_why`)

### Requirement: Pick: A dry run reports the check of the tree

`--dry-run` MUST report the check of the tree in `check` and never refuse for it, with or without the setting. With
the setting, a pick MUST report `check` too.

#### Scenario: The check of a dry run
- **WHEN** a pick runs with `--dry-run`
- **THEN** `check` has `state`: `ok`, a record names the tree; `stale`, records exist and none names it; `missing`, no
  record
- **AND** `record`: the newest record of the tree, else the newest one, with `changed_files` and the first 10 of them
  in `changed_paths`
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_tree_that_no_green_check_recorded`)

### Requirement: Pick: An update that applies after a conflict waits for a check of the new tree

With the setting, an update after a conflict that applies at once moves the lease onto a new tree: the pick again MUST
run only when a record names that tree too, else the answer stays `state: conflict` (exit 1) with the `update`, the
`check` of the new tree, and a `conflict.next` that says to build and test in the lease, then pick again.

#### Scenario: A new tree without a record
- **WHEN** an update that applies at once after a conflict leaves a tree no record names
- **THEN** the answer stays `state: conflict` with the `check` of the new tree
  [@test] crates/bd-lease/src/lease_pick_test.rs (`an_update_that_applies_after_a_conflict_waits_for_a_check_of_the_new_tree`)

### Requirement: Pick: The check costs one rev-parse

The check MUST cost one `rev-parse` (HEAD and its tree) and, for a stale tree, one `diff-tree`; without the setting and
without --dry-run the pick MUST run neither.

#### Scenario: The git of the check
- **WHEN** a lease pick checks its tree, or runs without the setting
- **THEN** it runs one `rev-parse` and for a stale tree one `diff-tree`, or neither
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_tree_check_of_a_lease_pick`, `a_lease_pick`)

### Requirement: Clean rules: The rules are the clean array of the lease table

The rules MUST be the `clean` array of the `[lease]` table of `.branchdeck.toml` at the root of the checkout (beside
`pick_requires_check` of [Pick](#pick)), committed with the build layout they clean (not git config, and not
`.worktreeinclude`, which Treehouse and Claude Code read too). There MUST be two kinds and no command, since a return
must not run arbitrary code.

#### Scenario: The two kinds and their defaults
- **WHEN** `.branchdeck.toml` names a `cargo` rule and a `delete` rule
- **THEN** they read with their defaults, `{ cargo = "<dir>", keep = 8, age = "1h" }`
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_rules_are_cargo_and_delete_with_their_defaults`)

### Requirement: Clean rules: A cargo rule prunes the stale units of its profile dirs

`{ cargo = "<dir>", keep = 8, age = "1h" }` (the defaults) MUST prune the profile dirs under `<dir>` (a dir with
`.cargo-lock`, at most 4 levels down, also of a cross target such as `aarch64-unknown-linux-musl/debug`): a unit stays
when it is among the `keep` newest of its group or younger than `age` (`s`, `m`, `h`, `d`), and the files of a unit go
together. Each profile dir MUST be pruned in the layout it has.

#### Scenario: The newest and the young units stay
- **WHEN** a cargo rule prunes a profile dir
- **THEN** the `keep` newest units of each group and the units younger than `age` stay
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_keeps_the_newest_and_the_young_units`)

#### Scenario: The build-dir layout
- **WHEN** the profile dir has the build-dir layout
- **THEN** a unit is `build/<package>/<hash>/`, built at the newest mtime of the dir and of the children of its
  `fingerprint` dir

#### Scenario: The classic layout
- **WHEN** the profile dir has the classic layout
- **THEN** a unit is `<name>-<16 hex digits>`, its group `<name>` (`-` and `_` the same, without the `lib` of a library
  file)
- **AND** its files are all of that name and hash in `deps/` (`lib<name>-<hash>.rlib` and `.rmeta`, `<name>-<hash>.d`,
  the object files, the test or bin executable and its `.dSYM`, `.pdb` or `.exe`), `.fingerprint/` and `build/` (a
  build script and its run, `out/`), built at the newest mtime of its files
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_prunes_the_classic_layout_by_units_of_name_and_hash`, `a_classic_unit_is_its_name_and_its_hash`)

#### Scenario: Incremental units
- **WHEN** the profile dir has `incremental/<crate>-<hash>/`, in either layout
- **THEN** it is a unit of its own, grouped by crate

#### Scenario: The uplifted artifacts
- **WHEN** the profile dir has uplifted artifacts at its top (`debug/libavl_vm.rlib`, `debug/<bin>`) and `.cargo-lock`
- **THEN** they are never touched

### Requirement: Clean rules: A delete rule removes a path or every entry but the kept ones

`{ delete = "<path>" }` MUST remove a path, a file, a directory or a symlink (the link, never its target);
`{ delete = "<dir>", except = ["<name>", …] }` MUST remove every entry of the directory but the named ones (names of
direct entries, no path), a whitelist: a new entry goes without an edit of the rules. A rule below a kept entry applies
to it (`out` but for `dev-run`, then `out/dev-run` but for `jar-cache`).

#### Scenario: A delete with kept entries and a link
- **WHEN** a `delete` with `except` runs on a directory with a link among its entries
- **THEN** it keeps the kept entries and removes the link as the link
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_delete_keeps_its_kept_entries_removes_a_link_and_goes_through_the_trash`)

#### Scenario: Nested deletes
- **WHEN** a rule lies below a kept entry of another
- **THEN** only the whitelist of each stays
  [@test] crates/lease-pool/src/lease_clean_test.rs (`nested_deletes_keep_only_the_whitelist`)

#### Scenario: A link of Windows
- **WHEN** the path is a junction or a directory symlink of Windows, such as Bazel's `out/bazel-*`
- **THEN** the removal neither goes into it nor takes it for a file held by a process
  [@test] crates/sync-utils/src/fs_retry_test.rs (`remove_entry_removes_a_directory_link_as_the_link_and_keeps_its_target`, `remove_entry_removes_a_file_an_empty_directory_and_a_file_symlink_as_themselves`)

### Requirement: Clean rules: A delete of the dir of a cargo rule names what it keeps

A `delete` of the dir of a `cargo` rule MUST name what it keeps in `except`
(`{ delete = "target", except = ["debug", "release", "CACHEDIR.TAG"] }`): it removes every other entry of the target
dir whole (the scratch an agent puts there: a second target dir, a log, a patch, a link as the link), which the cargo
rule at most prunes, never removes. The two rules MUST never take the same path, whatever their order in the file.

#### Scenario: A delete of a target dir
- **WHEN** a `delete` of a target dir keeps its profile dirs
- **THEN** the cargo rule of that dir prunes only the profile dirs at and below the kept entries that are directories,
  not links, at most 4 levels below the target dir as before
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_delete_of_a_target_dir_keeps_its_profile_dirs_for_the_cargo_rule`, `a_delete_of_the_dir_of_a_cargo_rule_needs_its_kept_entries`)

#### Scenario: A delete of a profile dir
- **WHEN** that dir is a profile dir itself (`{ cargo = "target/debug" }` with
  `{ delete = "target/debug", except = ["deps", "build", ".fingerprint", "incremental", ".cargo-lock"] }`, which drops
  the uplifted artifacts)
- **THEN** the cargo rule prunes the units in its kept dirs alone
- **AND** the except keeps `.cargo-lock`, without which the dir is no profile dir once the delete ran
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_delete_of_a_profile_dir_keeps_its_unit_dirs_for_the_cargo_rule`)

### Requirement: Clean rules: A kept entry is its exact name

A kept entry MUST be the one whose name is the `except` name byte for byte, for both rules.

#### Scenario: A volume that ignores case
- **WHEN** on a volume that ignores case (APFS, NTFS) the `except` name is `Debug`
- **THEN** it keeps no `debug`, so the delete removes it whole and the cargo rule prunes nothing of it
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_kept_entry_of_a_target_dir_is_its_exact_name`)

### Requirement: Clean rules: The delete and the cargo rule of a dir report on their own

Each of the delete and the cargo rule of a dir MUST be a rule of its own in `cleaned`, its `freed_bytes` its own paths,
which a dry run measures as the clean then frees them. Both MUST go through the trash of the run, under the `cleaning`
marker of a free slot, also in the shrink of a get, where the cargo rule keeps no unit of the kept profile dirs; a get
leaves the removed entries out of a clone.

#### Scenario: The report of the two rules
- **WHEN** a clean or a dry run applies a delete and a cargo rule of one dir
- **THEN** `cleaned` names each with the bytes of its own paths

### Requirement: Clean rules: A delete removes a link in place

A `delete` MUST remove a link (a symlink, a junction or a directory symlink of Windows) in place, as the link and never
its target, also in a return and `brd lease clean`, which move every other path into the trash: a link is no cheaper to
move than to delete, and a junction moved there stayed ("Access is denied"). It MUST go only once every other path
removed with it is moved, so a failed move never leaves a link lost, and its own bytes count in `freed_bytes` at once.

#### Scenario: Bazel links through the trash
- **WHEN** a delete through the trash comes to the `bazel-*` links
- **THEN** it deletes them in place and keeps their targets
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_delete_through_the_trash_deletes_bazel_links_in_place_and_keeps_their_targets`, `a_delete_keeps_its_kept_entries_removes_a_link_and_goes_through_the_trash`)

#### Scenario: A link an earlier version left in the trash
- **WHEN** the trash of a gone run holds a link an earlier version left there
- **THEN** it goes as the link with the trash of its run, and the trash holds no links
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_sweep_deletes_a_link_left_in_the_trash_of_a_gone_run`)

### Requirement: Clean rules: A bad file is an error of the file

An unknown key, an unknown kind, a wrong type or a file that does not parse MUST be an error of the file.

#### Scenario: Unknown keys and kinds
- **WHEN** the file has an unknown key, an unknown kind or a wrong type, or does not parse
- **THEN** it is an error of the file
  [@test] crates/lease-pool/src/lease_clean_test.rs (`unknown_keys_and_kinds_are_errors`)

### Requirement: Clean rules: The host table of the remote runs stays unread

The table `[host]` belongs to the remote runs of `brd-host` (openspec/specs/cli/host/spec.md), which checks its keys.
The rules and `pick_requires_check` MUST read past it, whatever it holds.

#### Scenario: A host table
- **WHEN** `.branchdeck.toml` has a `[host]` table
- **THEN** the rules read past it, whatever it holds
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_host_table_of_the_remote_runs_stays_unread`)

### Requirement: Clean rules: An unknown table leaves the rules in force

The rules and `pick_requires_check` MUST read past every other top-level table too (a table of another reader or of a
later version): its reader checks it, and a new table must not turn off the rules of an older brd. An unknown key
inside `[lease]` MUST stay an error of the file.

#### Scenario: An unknown table
- **WHEN** `.branchdeck.toml` has a top-level table that brd does not know
- **THEN** the lease rules stay in force
  [@test] crates/lease-pool/src/lease_clean_test.rs (`an_unknown_table_leaves_the_lease_rules_in_force`)

### Requirement: Clean rules: An unsafe rule is skipped and reported

A rule MUST be skipped and reported (`error`), never applied, when its path is absolute, leaves the checkout with `..`,
names `.git`, names the checkout itself, has a symlink on its way (a `cargo` dir that is one too), holds a file that
git tracks (one `ls-files -z` of the paths of all rules), or has an `except` entry that is no name of an entry. The
walk of a rule MUST never follow a symlink.

#### Scenario: An unsafe path
- **WHEN** a rule has an unsafe path
- **THEN** it is skipped and reported, never applied
  [@test] crates/lease-pool/src/lease_clean_test.rs (`an_unsafe_path_is_a_rule_that_is_reported`)
  [@test] crates/bd-lease/src/lease_clean_test.rs (`an_unsafe_rule_is_skipped_and_reported`)

### Requirement: Clean rules: A delete that would remove what a cargo rule keeps is reported

A `delete` whose path holds the dir of a `cargo` rule without naming the entry on its way in `except`, or is that dir
without `except` (no key or an empty array), MUST be skipped and reported, as it would remove what the cargo rule keeps.

#### Scenario: A delete over the dir of a cargo rule
- **WHEN** a `delete` holds the dir of a `cargo` rule and does not keep it, or is that dir without `except`
- **THEN** it is skipped and reported
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_delete_names_its_kept_entries_and_keeps_the_dirs_of_cargo_rules`, `a_delete_of_the_dir_of_a_cargo_rule_needs_its_kept_entries`)

### Requirement: Clean rules: A second delete of a path is reported

A `delete` of a path that an earlier `delete` has (normalized) MUST be skipped and reported, since a path has one
whitelist: two would both measure in a dry run an entry that the first then removes, and a clone would keep what the
last one keeps.

#### Scenario: Two deletes of one path
- **WHEN** two `delete` rules name the same path
- **THEN** the second is skipped and reported
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_second_delete_of_a_path_is_reported`)

### Requirement: Clean rules: The return applies the rules of the source checkout

The return MUST read the file once, from the source checkout that the lease recorded (the slot's own when the source
is gone), and apply the rules after all its checks passed, before it frees the slot: `cleaned` [{`rule`,
`freed_bytes`, `ms`, `error`?}]. A file that can't be read or parsed (`clean_error`) and a failed rule MUST never fail
the return.

#### Scenario: The rules of the source
- **WHEN** a return passed its checks
- **THEN** it applies the rules of the source and reports them in `cleaned`
  [@test] crates/bd-lease/src/lease_clean_test.rs (`the_return_applies_the_clean_rules_of_the_source`)

#### Scenario: A bad file
- **WHEN** the file can't be read or parsed
- **THEN** the return reports `clean_error` and does not fail
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_bad_file_does_not_fail_the_return`)

### Requirement: Clean rules: What a rule removes goes into the trash first

What a rule removes MUST go into the trash first, a rename into `$GIT_COMMON_DIR/branch-deck/trash/<pid>-<start
time>-<uuid>/` (in place on another volume): the paths of a `delete` but a link, and a stale unit of a `cargo` rule as
a whole, so a return frees the slot before the long delete of a build output and a timeout of the caller never leaves a
lease `returning` while files go.

#### Scenario: A return through the trash
- **WHEN** a return applies its rules
- **THEN** what they remove goes into the trash, and is deleted once the slot is free
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_return_deletes_through_the_trash_and_what_a_gone_run_left_there`)
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_delete_keeps_its_kept_entries_removes_a_link_and_goes_through_the_trash`)

### Requirement: Clean rules: What a process keeps stays whole in the checkout

What a process keeps MUST stay whole in the checkout, never be deleted in place: on Windows a rename fails while a
process works in the path or has a file of it open. The other paths of the rule MUST go on.

#### Scenario: A held unit or entry
- **WHEN** a rename still fails after a retry of about a second (`sync_utils::fs_retry`, which every removal and rename
  of the app goes through)
- **THEN** the files of the unit moved before it go back, and the `error` of the rule names the path and says that a
  process holds it (Windows has no process scan to name it)
- **AND** once the process let go, the next `brd lease clean` deletes it
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_held_unit_and_a_held_entry_stay_whole_and_the_next_clean_takes_them`)
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_held_entry_of_a_free_slot_stays_whole_until_its_process_exits`)

#### Scenario: The retry of a removal or a rename
- **WHEN** a removal or a rename meets a holder on Windows, or a read-only attribute, which fails a call with the same
  "Access is denied"
- **THEN** it waits out the holder for about a second, and the read-only attribute is cleared instead, as git clears it
  [@test] crates/sync-utils/src/fs_retry_test.rs (`a_process_whose_current_directory_is_in_the_directory_delays_its_removal_until_it_exits`, `a_program_that_keeps_a_file_open_fails_the_removal_on_windows_after_a_second`, `a_read_only_file_and_directory_are_renamed_and_removed_at_once`)
  [@test] crates/git-executor/src/fs_retry_scan_test.rs (`every_removal_and_rename_waits_out_a_holder_on_windows`)

### Requirement: Clean rules: The trash is deleted once the slots are free

Once the slot is free the return, `brd lease clean` once every slot it marked is free again, and `brd lease reclaim`
once every slot it reclaimed is free, MUST delete their trash, counting `freed_bytes` in the same walk, and the trash of
a run whose process is gone; a path that can't be deleted stays and is named in `trash_left`.

#### Scenario: The trash of a gone run
- **WHEN** the trash of a run whose process is gone is in the pool
- **THEN** the return deletes it with its own
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_return_deletes_through_the_trash_and_what_a_gone_run_left_there`)

#### Scenario: The bytes freed
- **WHEN** the trash is deleted
- **THEN** `freed_bytes` counts the blocks an APFS clone shares with another checkout too
- **AND** `brd lease clean` also prints `volume_freed_bytes`, what the volume gained (`statfs` before and after)

### Requirement: Clean rules: A read-only directory to delete is widened first

A directory the walk deletes that the owner may not read, write and search MUST be widened before it goes into it:
Bazel takes those rights from every directory of an output it wrote (`dr-xr-xr-x`), and without them no entry of it
goes ("Permission denied"), which left the trash of gone runs of such a repository growing unseen, a `WARN` its only
word. Only a directory that is to be deleted MUST be widened, never one that stays.

#### Scenario: A read-only directory of a Bazel output
- **WHEN** a delete comes to a read-only directory of a Bazel output
- **THEN** it goes with its entries, and the bytes it frees count once
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_read_only_directory_of_a_bazel_output_goes_with_the_bytes_it_freed`, `the_sweep_deletes_the_read_only_trash_of_a_gone_run`)

### Requirement: Clean rules: A path of a protected app bundle names its removal in Finder

If macOS refuses to remove a path inside a protected `.app` bundle with `EPERM`, the clean error and `trash_left` MUST
name the refused path and bundle, and tell the user to move the whole bundle to the Trash in Finder.

#### Scenario: EPERM in an app bundle
- **WHEN** macOS refuses to remove a path inside a protected `.app` bundle with `EPERM`
- **THEN** `trash_left` names the path and the bundle and tells the user to move the bundle to the Trash in Finder
  [@test] crates/lease-pool/src/lease_clean_test.rs (`an_eperm_in_an_app_bundle_names_finder_removal_in_trash_left`)

### Requirement: Clean rules: brd lease clean applies the rules of its checkout

`brd lease clean [--free] [--dry-run] [--keep <n>] [--age <duration>] [<dir>...]` MUST apply the rules of the checkout
it runs in, read once, to the free slots (`--free`), to the given worktrees, or to that checkout. A slot of the pool
with a lease or with a process in it is skipped (`skipped`); a checkout that is no slot is cleaned as asked.

#### Scenario: A busy and a leased slot, and a dry run
- **WHEN** `brd lease clean` comes to a slot with a lease or a process in it, or runs with `--dry-run`
- **THEN** it skips the slot, and `--dry-run` deletes nothing and says what the rules would free
  [@test] crates/bd-lease/src/lease_clean_test.rs (`clean_skips_a_busy_and_a_leased_slot_and_a_dry_run_deletes_nothing`)

#### Scenario: A file that does not parse
- **WHEN** the rules file does not parse
- **THEN** `brd lease clean` fails with `invalid_arguments` (exit 2)
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_bad_file_does_not_fail_the_return`)

### Requirement: Clean rules: A free slot in a clean carries the cleaning marker

A free slot `brd lease clean` cleans MUST carry a `cleaning` marker under the pool lock until the clean of it ends, so
no get leases it.

#### Scenario: A get beside a clean
- **WHEN** a get runs while `brd lease clean` cleans a free slot
- **THEN** the get does not lease that slot
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_slot_that_clean_cleans_is_reserved_against_a_get`)

### Requirement: Clean rules: Sizes are measured by the shared walk

`CleanMode::Measure` and `Pool::trash_bytes` MUST use `fs_walk::tree_size`, the shared filesystem walk rather than a
second recursive size implementation; its result includes directory blocks and counts links as themselves.

#### Scenario: The size of a tree
- **WHEN** a dry run measures a rule, or the pool measures its trash
- **THEN** the size comes from the walk, with directory blocks, each run of the trash measured
  [@test] crates/fs-walk/src/walk_test.rs (`tree_size_uses_the_walk_and_counts_directory_bytes`)
  [@test] crates/lease-pool/src/lease_clean_test.rs (`trash_bytes_measure_each_run_with_the_shared_walk`, `the_cargo_rule_keeps_the_newest_and_the_young_units`)

### Requirement: Clean rules: The rules apply by default with two overrides

The rules are configured on purpose, so they MUST apply by default: the return and `brd lease clean` delete what they
name, as the user decided. `brd lease return --no-clean` MUST apply none. `--keep <n>` and `--age <duration>`, on both
commands, MUST set `keep` and `age` of every cargo rule for that call without an edit of the file; a bad `--age` is
`invalid_arguments`. These are the only overrides: `except` of a `delete` rule is the file's.

#### Scenario: No clean and the overrides
- **WHEN** a return runs with `--no-clean`, or a call with `--keep <n>` or `--age <duration>`
- **THEN** the return cleans nothing, and the call prunes by the given `keep` and `age`
  [@test] crates/bd-lease/src/lease_clean_test.rs (`no_clean_and_the_overrides_change_what_a_call_cleans`)

### Requirement: Status: The status lists every slot from the state alone

`brd lease status` MUST list every slot from the state alone, with its `path`, `state` (`free`, `preparing`, `ready`,
`templating`, `returning`, `cleaning`, `abandoned`), `dirty`, `missing`, `lease` and `reclaimed`, the `free_bytes` of
the volume of the pool and the `trash_bytes` a killed run left; it MUST run no `git status`. `--processes` MUST add the
processes in each slot. It writes nothing.

#### Scenario: The status of the pool
- **WHEN** `brd lease status` runs
- **THEN** it lists every slot from the state without a `git status`
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_reads_the_state_without_a_git_status`)

#### Scenario: The holder of a lease
- **WHEN** a slot has a lease
- **THEN** `lease` has `holder_pid`, `holder_alive`, `idle` for a ready lease whose holder is gone (`idle`, or
  `idle_with_work` with what a check found), and `taking_over` while a get checks one
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`)

#### Scenario: A read
- **WHEN** `brd lease status` runs, with or without `--processes`
- **THEN** it writes no ref, config, index or working tree
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

### Requirement: Reclaim: Reclaim applies the rule of the takeover to every idle lease

`brd lease reclaim --idle [--dry-run] [--keep <n>] [--age <duration>]` MUST apply the rule of the takeover to every
idle lease of the pool, each checked anew: one whose work landed is returned as the takeover returns it, under a CAS on
its lease, its holder still gone and its slot as the check found it, its slot free with its record. The output is
`lease_reclaim` of `brd schema`, exit 0.

#### Scenario: An idle lease whose work landed
- **WHEN** reclaim finds an idle lease whose work landed
- **THEN** it returns it as the takeover returns it (`reclaimed`, each with `landed`, `stopped_servers`, `cleaned`)
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_reclaim`)

#### Scenario: An idle lease with work
- **WHEN** reclaim finds an idle lease with work
- **THEN** it keeps the lease and records its check (`kept`, as `idle_with_work`)
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)

#### Scenario: A lease it cannot judge
- **WHEN** a lease is not quiet for an hour, busy, changed meanwhile or its check failed
- **THEN** it is `skipped` with its reason

#### Scenario: A dry run
- **WHEN** reclaim runs with `--dry-run`
- **THEN** it checks the same and writes nothing

### Requirement: Reclaim: The trash is deleted once the last reclaimed slot is free

The trash of every reclaimed lease MUST be deleted once the last one is free (`trash_left`), so none waits for the
delete of another's build output.

#### Scenario: Several reclaimed leases
- **WHEN** reclaim returns several leases
- **THEN** it deletes their trash once every reclaimed slot is free
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_deletes_the_trash_once_every_reclaimed_slot_is_free`)

### Requirement: Trim: Trim keeps every pool of the machine within its limit

`brd lease trim [--max <n>] [--plan]` MUST trim every pool of this machine: the pool of `-C`, wherever it lives, and
each pool under `~/.branch-deck/pool` whose slots name their repository (the `.git` file of a slot). A pool keeps
`--max`, else `branchdeck.leaseMaxSlots` of its repository's git config (a whole number, the machine's, so never
`.branchdeck.toml`), else 10 slots; a value that does not parse is the default, named as `config_error`.

#### Scenario: The limit
- **WHEN** `branchdeck.leaseMaxSlots` is set, or does not parse
- **THEN** the pool keeps that whole number of slots, or the default with `config_error`
  [@test] crates/lease-pool/src/trim_test.rs (`the_limit_is_a_whole_number_of_slots`)

### Requirement: Trim: Past the limit the free slots a get would take last go first

Past the limit, the free slots of a pool MUST go one by one, the ones a get would take last first: the never returned
ones in reverse order of joining, then the ones returned longest ago. A leased slot, a host slot and a slot with a
lease of any state MUST count and stay.

#### Scenario: A pool past its limit
- **WHEN** a pool has more slots than its limit
- **THEN** its free slots past the limit go, the ones returned longest ago first, with their output bases
  [@test] crates/bd-lease/src/trim_test.rs (`a_trim_deletes_the_free_slots_past_the_limit_returned_longest_ago_first_with_their_output_bases`)

### Requirement: Trim: A slot goes only under the cleaning marker

A slot MUST go only under the `cleaning` marker, set under the pool lock while it is free, so no get reserves it: its
Bazel servers stopped as a get stops them. Then its directory moves into the trash of its pool, `git worktree remove
--force` drops its admin directory (`forget_slot`, refused while the directory is there), and its entry leaves the
state.

#### Scenario: A busy slot
- **WHEN** a slot has another process in it, or its server does not exit
- **THEN** it stays free and is named (`busy`, with the processes) and the next one goes instead
  [@test] crates/bd-lease/src/trim_test.rs (`a_free_slot_with_a_process_in_it_stays_and_the_next_one_goes`)

#### Scenario: The number directory of a slot
- **WHEN** the slot is the only directory of `<pool>/<n>`
- **THEN** `<pool>/<n>` moves into the trash, a log left beside it included; else the slot
  [@test] crates/lease-pool/src/trim_test.rs (`the_number_directory_of_a_slot_goes_with_it_when_the_slot_is_its_only_directory`)

### Requirement: Trim: The Bazel output bases of a slot go with it

The Bazel output bases of a slot that goes MUST go with it: the output user roots are found by the `bazel-out` link (or
`out/bazel-out`) of the workspaces of the source checkouts and of every slot (the checkout and each directory right in
it with a workspace file), and each output base of a root names its workspace in `DO_NOT_BUILD_HERE`; every one whose
workspace is in the slot goes (`output_bases`).

#### Scenario: The roots and the output bases
- **WHEN** the trim looks for the output bases of a slot
- **THEN** it finds the roots by the `bazel-out` links and reads the workspace of each output base
  [@test] crates/lease-pool/src/output_bases_test.rs (`the_roots_are_found_by_the_bazel_out_links_of_the_workspaces_of_a_checkout`, `the_output_bases_of_a_root_name_their_workspace_and_the_gone_ones_of_a_pool_are_orphans`)

### Requirement: Trim: An output base whose slot is gone goes too

An output base whose slot is gone (its workspace in a pool directory, `<pool>/<n>/<name>` missing) MUST go too
(`orphans`), unless its server runs; one of a slot that is there MUST stay, also when the commit of the slot lacks its
workspace.

#### Scenario: A pool within its limit
- **WHEN** a pool is within its limit and an output base names a workspace that is gone
- **THEN** no slot goes, but that output base does
  [@test] crates/bd-lease/src/trim_test.rs (`a_pool_within_its_limit_is_not_trimmed_but_the_output_bases_of_gone_workspaces_go`)

### Requirement: Trim: The deletes run side by side once every pool is done

The deletes MUST run once every pool is done, side by side (8 threads over the entries of what the trash holds): each
slot names the bytes its directory and its output bases freed (`freed_bytes`), the output the sum and the volume what
it gained (`volume_freed_bytes`, `statfs` before and after) and has free now (`free_bytes`). The output is `lease_trim`
of `brd schema`, exit 0.

#### Scenario: The deletes and their bytes
- **WHEN** the trim deletes
- **THEN** every path is deleted side by side and its bytes counted
  [@test] crates/lease-pool/src/trim_test.rs (`every_path_is_deleted_side_by_side_and_its_bytes_counted`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_trim`)

### Requirement: Trim: A plan names what would go and deletes nothing

`--plan` MUST name the same slots and output bases and write and measure nothing.

#### Scenario: A trim plan
- **WHEN** `brd lease trim --plan` runs
- **THEN** it names what would go and deletes nothing
  [@test] crates/bd-lease/src/trim_test.rs (`a_trim_plan_names_what_would_go_and_deletes_nothing`)

### Requirement: Disk: The disk report names the bytes of every slot of the machine

`brd lease disk [--cached] [--fresh]` MUST report every pool of this machine (as the trim finds them) without a `du` of
a slot: per slot its `state`, `holder`, `returned` and the bytes its builds wrote, the directories its clean rules name
and its Bazel output bases (`output_bases`), each with its bytes, their sum (`bytes`), `measured_at` and `cached`.

#### Scenario: The bytes of each slot
- **WHEN** `brd lease disk` runs
- **THEN** each slot names its build directories and output bases with their bytes, and keeps them until the slot is
  leased
  [@test] crates/bd-lease/src/trim_test.rs (`the_disk_report_names_the_bytes_of_each_slot_and_keeps_them_until_the_slot_is_leased`)

#### Scenario: The build directories
- **WHEN** the clean rules of a slot name nested directories
- **THEN** the report names the outermost of them, `build`
  [@test] crates/pool-disk/src/disk_test.rs (`the_build_dirs_of_a_slot_are_the_outermost_dirs_its_clean_rules_name`)

### Requirement: Disk: The report names what each pool holds and what a trim deletes next

Per pool the report MUST name `max_slots`, `past_limit` (the free slots a trim would delete now), `bytes`,
`trash_bytes` and the output bases of its main worktree (`checkout_bases`), which no trim deletes; each free slot a trim
deletes now is `trims_next`, by the order of the trim itself (`trim_order`); then the `orphans`, the `free_bytes` of the
volume and `min_free`.

#### Scenario: The pools of the report
- **WHEN** `brd lease disk` reports a pool past its limit
- **THEN** the free slots a trim deletes now are `trims_next`, in `trim_order`

### Requirement: Disk: Each slot names what its worktree holds of its own

Each slot MUST also have `worktree_bytes`, what its worktree holds of its own beside its build directories, which its
walk skips: its files rewritten since the clone it came from (each move by the diff writes the files it changes, which
then share no block with the template).

#### Scenario: The worktrees of idea-1
- **GIVEN** the slots of idea-1 on 2026-11-02
- **WHEN** the report measures their worktrees
- **THEN** each held 14 to 16 GB of its own

### Requirement: Disk: Every byte counted is what a delete frees

Every byte counted MUST be what a delete frees: on macOS the private size of a file (`ATTR_CMNEXT_PRIVATESIZE` of
`getattrlistbulk`, the blocks no APFS clone shares, one call per batch of entries), elsewhere its blocks (Linux) or its
length (Windows). A file with several hard links counts once, a link as itself, a directory nothing of its own; the
walks of the call run side by side (`fs_walk::walk_bytes`, 8 threads over one queue of directories).

#### Scenario: A clone
- **WHEN** a file of a slot is an APFS clone
- **THEN** it counts only the blocks it shares with no other file
  [@test] crates/pool-disk/src/walk_test.rs (`a_clone_counts_only_the_blocks_it_shares_with_no_other_file`)

#### Scenario: Roots side by side and hard links
- **WHEN** the walks of a call run
- **THEN** each root is counted side by side, what counts apart is skipped, and a hard-linked file counts once
  [@test] crates/fs-walk/src/walk_test.rs (`a_walk_counts_each_root_side_by_side_skips_what_counts_apart_and_a_hard_linked_file_once`, `walk_tree_nodes_and_root_remainder_add_up_to_the_walk`)

### Requirement: Disk: The bytes of a slot are kept and refreshed by what changed

The bytes of a slot MUST be kept in `$GIT_COMMON_DIR/branch-deck/disk.json` with its `returned`, whether it was leased,
its FSEvents mark and the sizes of disjoint nodes under its roots. On macOS the default refresh MUST ask FSEvents what
changed since that mark: an unchanged slot stays cached regardless of its age or lease; a changed path walks its
nearest saved node. A slot that left the pool leaves the file.

#### Scenario: A changed file
- **WHEN** FSEvents names a changed file of a slot
- **THEN** the refresh walks its nearest saved node, and a path no node holds selects a root
- **AND** the report streams the cached overview, then each finished slot
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_cached_overview_then_each_finished_slot`, `a_changed_file_selects_its_nearest_saved_node_and_uncached_paths_select_a_root`)

#### Scenario: A full walk
- **WHEN** the history is missing or incomplete, the roots changed, the slot is on another volume, or `--fresh` is given
- **THEN** the full slot is walked

#### Scenario: Linux, Windows and the output bases of the main worktree
- **WHEN** the report runs on Linux or Windows, or measures the output bases of the main worktree
- **THEN** Linux and Windows keep their returned/leased cache policy, with 15 minutes for a leased slot
- **AND** the output bases of the main worktree hold for 15 minutes on every platform

### Requirement: Disk: Cached reads the saved sizes without a walk

`--cached` MUST read the saved slot, trash and orphan sizes without a walk and conflict with `--fresh`; normal output
MUST name the cache source and measurement age per slot and the elapsed milliseconds. The report writes nothing else,
and counts as a read (`brd help`).

#### Scenario: Cached and fresh together
- **WHEN** `brd lease disk --cached --fresh` runs
- **THEN** it is refused, as the flags are mutually exclusive
  [@test] crates/bd-lease/src/disk_command.rs (`cached_and_fresh_disk_flags_are_mutually_exclusive`)

#### Scenario: The lines of the report
- **WHEN** the report prints a slot
- **THEN** its line names the cache source, the measurement age and the elapsed time
  [@test] crates/pool-disk/src/report_test.rs (`lines_name_the_slot_cache_source_age_and_elapsed_time`)

#### Scenario: The time of a report on the user's Mac
- **GIVEN** the user's Mac with 32 slots, 12 of them of idea-1 with 1.4M files each, 860 GB
- **WHEN** the report comes from the file, or walks every slot with its worktree
- **THEN** it takes half a second, or five minutes

### Requirement: Reclaimed: A command of another session in a taken-over slot is refused

A command of brd in a slot whose lease a get or a reclaim took over MUST be refused as `lease_reclaimed` (exit 4),
nothing changed, when another session than the one that holds it now runs it: the message names the old lease and
holder, the new one, the home branch and the old branch, which stays, and `next_steps` names
`brd lease get --holder <old holder> -C <home>`.

#### Scenario: The session of the old lease
- **WHEN** the session of the old lease runs a command of brd in the slot
- **THEN** it is refused as `lease_reclaimed`, nothing changed, and `next_steps` names a new get
  [@test] crates/bd-lease/src/takeover_test.rs (`a_command_of_another_session_in_a_taken_over_slot_is_refused_as_lease_reclaimed`)

### Requirement: Reclaimed: The session that holds the slot is told by its process

The session that holds it MUST be told by its process: refused unless the holder (pid and start time) is an ancestor of
bd. The check MUST read the state, no git, with a process scan only in such a slot.

#### Scenario: The holder is an ancestor
- **WHEN** the holder recorded with its pid and start time is an ancestor of bd
- **THEN** the command is not refused
  [@test] crates/sync-utils/src/process_scan_test.rs (`an_ancestor_is_a_process_above_this_one_with_its_start_time`)

### Requirement: Reclaimed: Reads pass and a coordinator may return the current lease

A read MUST pass, but `brd workspace`, the first command of a session; `brd lease status` MUST never refuse, and
`brd lease return --lease-id <current lease>` MUST pass, for a coordinator. A free slot with the record MUST refuse
every write, and a lease that a get is taking over every command.

#### Scenario: A read and the status
- **WHEN** another session runs a read, `brd workspace` or `brd lease status` in the slot
- **THEN** the read passes, `brd workspace` is refused, and `brd lease status` never refuses

#### Scenario: A free slot with the record and a slot being taken over
- **WHEN** a command runs in a free slot with the record, or in a lease that a get is taking over
- **THEN** every write is refused in the first, and every command in the second

### Requirement: Adopt: Adopt adds clean linked worktrees as free slots

`brd lease adopt <dir>...` MUST add each clean linked worktree of this repository as a free slot, where it is
(`adopted`), and list the refused ones with their code (`refused`, exit 4): not a linked worktree of this repository
(`not_a_slot`), held by the pool already (`slot_in_pool`), a process in it (`slot_busy`), local changes
(`dirty_worktree`).

#### Scenario: Clean worktrees and the rest
- **WHEN** `brd lease adopt` is given clean linked worktrees and others
- **THEN** it adds the clean ones and refuses the rest with their code
  [@test] crates/bd-lease/src/lease_test.rs (`adopt_adds_clean_linked_worktrees_and_refuses_the_rest`)

### Requirement: Adopt: Adopt with move moves each worktree into the pool

`brd lease adopt --move <dir>...` MUST, after the same checks, move each worktree to the next slot path
`<pool>/<n>/<repository name>` with `git worktree move` (never a plain rename), report `moved_from`, and remove the old
parent directory when it is left empty (`<treehouse pool>/<n>/`). The state file of Treehouse is not touched;
Treehouse sees the old path as missing.

#### Scenario: A move into the pool
- **WHEN** `brd lease adopt --move` moves a worktree
- **THEN** it is at the next slot path and the old empty parent directory is gone
  [@test] crates/bd-lease/src/lease_test.rs (`adopt_with_move_moves_the_worktree_into_the_pool`)

#### Scenario: The output base of a moved slot
- **WHEN** a slot was moved
- **THEN** it starts with a new, cold output base, as Bazel derives the output base from the workspace path
- **AND** the old one stays on disk until the user deletes it

### Requirement: Adopt: A worktree on another volume is refused

A worktree on another volume than the pool directory (`st_dev`) MUST be refused as `other_volume` (exit 4): a move
there copies every file.

#### Scenario: Another volume
- **WHEN** `brd lease adopt --move` is given a worktree on another volume
- **THEN** it is refused as `other_volume`

### Requirement: Adopt: The move holds no pool lock

The move MUST hold no pool lock: the target path is reserved first by a `preparing` entry of the adopt, so no get takes
it, and the free slot replaces the entry once the move is done (the entry goes when it failed).

#### Scenario: A get during the move
- **WHEN** a get runs while an adopt moves a worktree
- **THEN** the get does not wait for the move and does not take its target path
  [@test] crates/bd-lease/src/lease_test.rs (`adopt_with_move_holds_no_pool_lock_across_the_move`)

### Requirement: Host slots: The functions of the lend table

The functions of the table MUST be `lend` (a run, the pid of its process with its start, an affinity: `Lent` with the
slot, its lease id, `cleaned_at` and `generation`, or `Queued` with `ahead`, the lent runs and the slots in quarantine;
a repeat answers the same slot), `lend_waiting`, `checkout`, `give_back_run` (the slot of the run, else the run leaves
the queue), `fill`, `reclaim`, `shrink`, `set_limit`, and `read_host_slots`.

#### Scenario: The answers of a lend
- **WHEN** a run asks for a slot
- **THEN** `lend` answers `Lent` with the slot, or `Queued` with its place, and a repeat answers the same slot
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_run_gets_the_slot_of_its_affinity_else_the_one_used_longest_ago_else_waits_in_line`)

#### Scenario: The read of the table
- **WHEN** `read_host_slots` runs
- **THEN** it answers each slot, its state, the queue, the limit (`lend_waiting` and `checkout` below); `describe_state`
  and `is_abandoned` say a state in words

### Requirement: Host slots: A pid whose start a sandbox hides counts as alive

A pid whose start a sandbox hides MUST count as alive while it exists.

#### Scenario: A sandboxed process
- **WHEN** a sandbox hides the start of the process of a run
- **THEN** the run counts as alive while its pid exists

### Requirement: Host slots: The generation of a slot grows before each removal of files

The `generation` of a slot MUST grow by one with each give-back that discards changes and each clean, before their
removal of files, so also before a retry after a crash. So an agent that keys what its prepare steps made in the slot
by it never reuses a result that a give-back or a clean removed.

#### Scenario: A give-back after an abandoned lend
- **WHEN** `reclaim` gives back an abandoned lend before another run gets the slot
- **THEN** the `generation` of the slot grows, and the next run sees it
  [@test] crates/bd-lease/src/host_slots_test.rs (`an_abandoned_lend_is_given_back_by_reclaim_before_another_run_gets_the_slot`)

### Requirement: Host slots: A refusal of the table is a HostSlotRefusal

A refusal of the table MUST be a `HostSlotRefusal`: `UnknownSlot`, `NoSlot` or `NotLent` (no such slot, or not this
run's), `Busy` (another process acts on it) or `InvalidConfig` (a bad `branchdeck.hostSlots`).

#### Scenario: A refused call
- **WHEN** a call of the table names no such slot, a slot not this run's, a slot another process acts on, or the config
  is bad
- **THEN** it is refused with the matching `HostSlotRefusal`

### Requirement: Host slots: A waiting lend answers as soon as it gets a slot

`lend_waiting` MUST wait while the run is queued, as long as its process lives, and answer as soon as it gets a slot,
without a second call. It watches `branch-deck/` of the common dir before its first look, so no give-back between the
look and the wait is lost, wakes only on a change of `host-slots.json` or `leases.json`, and looks again at least every
5 s, as the end of a process makes no file event.

#### Scenario: A slot given back during the wait
- **WHEN** a slot is given back while a run waits in `lend_waiting`
- **THEN** the run gets it without a second call
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_lend_that_waits_gets_the_slot_once_it_is_given_back`)

#### Scenario: A long wait
- **WHEN** the run waits
- **THEN** it gives its caller the queue once a minute (`host_runs::queue_line`: `run <id> waits <s> s: <n> runs ahead, …`)
- **AND** once the time is up it answers `Queued` with its place, and the ticket stays while the process lives

#### Scenario: A process that ends while it waits
- **WHEN** the process of the run ends while it waits
- **THEN** the answer is `ProcessGone`, and its ticket leaves the queue

### Requirement: Host slots: A waiting lend gives back an abandoned slot itself

A queued `lend_waiting` MUST give back itself a slot that no process acts on (an abandoned lend or give-back, a
quarantine whose processes are gone), as `reclaim` gives it back and under the same CAS on its state, then look again at
once: the end of a process makes no file event, and the keeper's `reclaim` comes only once a minute.

#### Scenario: An abandoned slot during the wait
- **WHEN** a run waits and a slot is abandoned
- **THEN** the waiting lend gives it back itself and looks again
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_waiting_lend_gives_back_an_abandoned_slot_itself`)

#### Scenario: An upgrade of git or a failed give-back
- **WHEN** a live process upgrades git, or the give-back fails
- **THEN** the waiting lend skips this, and a give-back that fails is left to `reclaim`

### Requirement: Host slots: A give-back starts the maintenance the git config asks for

With `branchdeck.leaseMaintenance` true, a give-back MUST start `git maintenance run --auto --detach` in the common dir,
as a return does, also for a run without a slot, and say so (`maintenance_started`); a failed start never fails the
give-back. Without the setting it MUST start none.

#### Scenario: The maintenance of a give-back
- **GIVEN** `branchdeck.leaseMaintenance` true
- **WHEN** a run gives its slot back
- **THEN** the give-back starts the maintenance and says so
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_host_slots_lend_give_back_and_shrink`)

### Requirement: Host slots: A fill leases the host slots as plain ready leases

A fill MUST lease the missing host slots up to `branchdeck.hostSlots` (git config of the host, 4 by default) as plain
`ready` leases of the holders `brd-host/1`, `brd-host/2`, … at the HEAD of the host checkout, detached, through the get
of the pool. `leases.json` MUST keep its format, so another brd and the app read the pool as before.

#### Scenario: The leases of a fill and the other commands
- **WHEN** a fill leased the host slots
- **THEN** a plain `brd lease get` never takes a host slot, `brd lease clean --free` skips it, the hint at idle leases
  does not name it, and a get with a holder `brd-host/…` is refused as `invalid_arguments`
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_fill_leases_the_host_slots_as_ready_leases_that_the_other_commands_leave_alone`)

#### Scenario: The format of the leases
- **WHEN** another brd or the app reads `leases.json`
- **THEN** a host slot lease reads in the format of before
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_host_slot_lease_keeps_the_format_of_leases_json`)

### Requirement: Host slots: The gets of a fill run side by side

The gets of a fill MUST run side by side: a free slot is reused at once, and the clones lock makes the clones one at a
time. A get that fails frees its number, and the first failure is the error; the slots got stay.

#### Scenario: A fill of several slots
- **WHEN** a fill leases several slots
- **THEN** its gets run side by side, and the clones run one at a time
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_gets_of_a_fill_run_side_by_side`)

### Requirement: Host slots: The lend table is written only under a leaf lock

The lend table is `$GIT_COMMON_DIR/branch-deck/host-slots.json`, which MUST be written only under `host-slots.lock`, a
leaf lock: held while the table changes, never across git, a clone, a process scan or another lock of the pool. Every
state of a slot but `idle` and `quarantined` MUST name the process that acts on it (`lent`, `filling`, `giving_back`,
`cleaning`, `shrinking`).

#### Scenario: The process of a state is gone
- **WHEN** the process that a state of a slot names is gone
- **THEN** the state is abandoned

### Requirement: Host slots: A lend gives the slot of the affinity, else the one used longest ago

A lend MUST give the run the idle slot that its affinity (`checkout-<12 hex>` of the client's checkout) used last, else
the idle slot used longest ago, else put the run in a first-come queue; a run gets a slot when fewer runs wait before it
than slots are idle. A repeat of a lend MUST get the same slot. A ticket of a gone process leaves the queue.

#### Scenario: The affinity, the slot used longest ago and the queue
- **WHEN** runs ask for slots
- **THEN** each gets the slot of its affinity, else the one used longest ago, else waits in line
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_run_gets_the_slot_of_its_affinity_else_the_one_used_longest_ago_else_waits_in_line`)

#### Scenario: An abandoned lend and a waiting run
- **WHEN** a lend is abandoned, or a run waits
- **THEN** the abandoned slot is not lent again before a give-back (of `reclaim` or of a waiting lend)
- **AND** each waiting run is a process of its own (the supervisor of the run), since its ticket lasts as long as its
  process

### Requirement: Host slots: A give-back stops the processes of the run

A give-back MUST stop the processes in the slot that started after the lend (SIGTERM, up to 10 s, then SIGKILL, each
only while its pid has the start time of the scan), leave its Bazel servers, and never signal a process older than the
lend. Else the changes MUST be discarded (`reset --hard`, `clean -fd`, only when `status` shows any; the ignored files
stay as the warm cache). Without a process scan (Windows) it goes on and answers `processes_checked: false`.

#### Scenario: A process of the run and an older one
- **WHEN** a process of the run and a process older than the lend are in the slot
- **THEN** the give-back stops the process of the run and leaves the older one
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_give_back_stops_the_processes_of_the_run_and_quarantines_the_slot_for_an_older_one`)

### Requirement: Host slots: A process left after the give-back quarantines the slot

A process left after the stop MUST quarantine the slot: no run gets it, the queue answer names the process, and
`reclaim` gives the slot back once the process is gone.

#### Scenario: An older process in the slot
- **WHEN** a process is left in the slot after the give-back
- **THEN** the slot is in quarantine
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_give_back_stops_the_processes_of_the_run_and_quarantines_the_slot_for_an_older_one`)

#### Scenario: The process is gone
- **WHEN** the process of a quarantine is gone
- **THEN** `reclaim` gives the slot back before another run gets it
  [@test] crates/bd-lease/src/host_slots_test.rs (`an_abandoned_lend_is_given_back_by_reclaim_before_another_run_gets_the_slot`)

### Requirement: Host slots: The fsmonitor daemon of a host slot outlives the give-back

The `status` of a give-back MUST use the repository's own `core.fsmonitor` and `core.untrackedCache`. git's fsmonitor
daemon of the slot MUST outlive the give-back and never quarantine the slot, also when it started after the lend with
its working directory in the slot. With it, the `status` of the next give-back MUST still find every change of its run.

#### Scenario: The daemon started during the run
- **WHEN** git's fsmonitor daemon started in the slot after the lend
- **THEN** it outlives the give-back, and the next give-back finds every change of its run
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_fsmonitor_daemon_of_a_host_slot_outlives_the_give_back_and_the_changes_of_a_run_are_still_found`)

### Requirement: Host slots: The clean rules of a host slot run when they are due

After a give-back, the clean rules of the slot's own `.branchdeck.toml` MUST run when they are due: with
`branchdeck.hostSlotMaxSize`, when what they would free passes it, or when the volume has less free space than
`branchdeck.leaseMinFree` plus it; without it, after 50 runs or 7 days since the last clean, or when the volume has less
than `branchdeck.leaseMinFree`.

#### Scenario: By size, by free space and after fifty runs
- **WHEN** a give-back ends and the rules are due by size, by free space or by the number of runs
- **THEN** the clean rules of the slot run
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_host_slot_is_cleaned_by_size_by_free_space_and_after_fifty_runs`)

### Requirement: Host slots: Clean host cleans every idle host slot now

`brd lease clean --host` MUST clean every idle host slot now (`--dry-run` only measures) and skip a slot that is not
idle or has a process in it, with the reason; it MUST conflict with `--free` and `DIR`.

#### Scenario: A clean of the host slots
- **WHEN** `brd lease clean --host` runs
- **THEN** it cleans the idle host slots and skips the others with the reason

### Requirement: Host slots: The keeper keeps the minimum free space without a get

`host_slots::keep_free`, which the keeper of brd-host runs once a minute, MUST keep the minimum free space of the volume
of the pool without a get: below `branchdeck.leaseMinFree` the trash, the trim of every pool of the machine, the shrink
and the eviction, each when the one before left the volume below it, as a get takes them ("Get"). Those steps take
free slots only: a host slot is a lease and stays in the pool.

#### Scenario: The keeper below the minimum
- **WHEN** the volume of the pool is below `branchdeck.leaseMinFree` at a round of the keeper
- **THEN** it trims the pools of the machine without a get
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_trims_the_pools_below_the_minimum_free_space_without_a_get`)

#### Scenario: No minimum, no free space or an upgrade of git
- **WHEN** there is no minimum, the system tells no free space, or a live process upgrades git
- **THEN** the keeper deletes nothing

### Requirement: Host slots: The keeper relieves the idle host slots used longest ago first

Below the minimum still, the keeper MUST relieve the idle host slots of its repository (`relieve_host_slots`), one by
one, the one used longest ago first (a never lent one before them), until the volume has the minimum: in each,
`cleaning` meanwhile so no run gets it, its clean rules with keep 0 and age 0 as the shrink applies them, then, below
the minimum still, its Bazel output bases, its Bazel servers stopped first, as the trim removes them with a slot.

#### Scenario: The relief of the host slots
- **WHEN** the volume is below the minimum after the steps of the free slots
- **THEN** the keeper relieves the idle host slots, the one used longest ago first, until the volume has its minimum
- **AND** each relief moves the `generation` of the slot
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_keeper_relieves_the_idle_host_slots_used_longest_ago_first_until_the_volume_has_its_minimum`)

#### Scenario: A busy host slot and a server that does not exit
- **WHEN** a host slot is not idle, or a process of a run works in it, or a server of it does not exit
- **THEN** the slot is not touched, and a server that does not exit keeps every base

### Requirement: Host slots: The keeper answers what went, not low disk space

Below the minimum after every step the keeper MUST answer what went and the free space, not the `low_disk_space` of a
get, whose advice is for an agent that asked for a slot.

#### Scenario: Still below the minimum
- **WHEN** the volume is below the minimum after every step of the keeper
- **THEN** it answers what went and the free space

### Requirement: Host slots: A relief round takes each volume once

One `ReliefRound` MUST take each volume once for the steps of the pools: a pool of the round on a volume that it took
measures nothing for them, and relieves only its own host slots. `host_slots::trim` is the trim of `brd-host trim`
([brd-host](../host/spec.md#trim)), with the same guard.

#### Scenario: Two pools on one volume
- **WHEN** a round comes to a second pool on a volume it took
- **THEN** that pool measures nothing for the steps of the pools and relieves only its own host slots

### Requirement: Host slots: Only the measure of the rules runs while a slot is given back

Only the git of that measure (one `ls-files` of the paths of the rules) MAY run while the slot is `giving_back`, so an
upgrade of git waits for it; the walk of the sizes MUST run once the slot is idle, and the clean then takes the slot only
while it is still idle. A clean MUST leave the slot idle before it deletes its trash, as a return of the pool frees the
slot first.

#### Scenario: The walk and the clean of a give-back
- **WHEN** a give-back measures and cleans its slot
- **THEN** it walks the sizes and deletes the trash of the clean once the slot is idle
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_give_back_walks_the_sizes_and_a_clean_deletes_its_trash_once_the_slot_is_idle`)

### Requirement: Host slots: A shrink gives the host slots past a count back to the pool

A shrink to a count MUST give the host slots past it back to the pool and keep the count as the limit of later fills
until it is lifted. It takes a slot that is idle, in quarantine or abandoned, also without a running agent, and MUST
refuse a slot that a live process has. It signals no process (a process in the slot refuses it).

#### Scenario: A shrink without the agent
- **WHEN** a shrink runs while no agent runs
- **THEN** it returns the host slots past the count
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_shrink_returns_the_host_slots_past_the_count_also_without_the_agent`)

### Requirement: Host slots: A shrink returns the slots through the return of the pool

A shrink MUST discard the changes, check the slot out at the commit it was leased at, so the snapshot commits of the
runs count as landed, and return it through the return of the pool without `--confirm-preserved` and without a second
`status`. It MUST delete the trash of all the slots it returned once the last one is free.

#### Scenario: The trash of a shrink
- **WHEN** a shrink returned several slots
- **THEN** it deletes their trash once every slot it returns is free
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_shrink_deletes_the_trash_once_every_slot_it_returns_is_free`)

### Requirement: Host slots: Reclaim takes up what no process acts on

`reclaim` MUST take up what no process acts on: drop the record of a fill that died before its get, give back an
abandoned lend or give-back, run an abandoned clean or shrink again, and return a `brd-host/<n>` lease that the table
does not name and whose number no live fill holds.

#### Scenario: A dead fill and a lease the table does not name
- **WHEN** `reclaim` finds the record of a fill that died before its get, and a `brd-host/<n>` lease the table does not
  name
- **THEN** it drops the record and returns the lease
  [@test] crates/bd-lease/src/host_slots_test.rs (`reclaim_drops_a_dead_fill_and_returns_a_lease_the_table_does_not_name`)

### Requirement: Host slots: The steps of reclaim run side by side

The steps of `reclaim` MUST run side by side, each on a slot of its own under the CAS of its state: a give-back waits up
to 10 s for the processes of its run, and no other slot waits for it.

#### Scenario: Several abandoned slots
- **WHEN** `reclaim` finds several slots to take up
- **THEN** their steps run side by side
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_steps_of_reclaim_run_side_by_side`)

### Requirement: Host slots: A hand return of a host slot is refused

A hand `brd lease return` of a host slot (its holder is `brd-host/<n>`, or the table names its lease id) MUST be refused
as `host_slot` (exit 4, `paths`) before any check or git; the refusal names `brd-host slots --local --shrink`.

#### Scenario: A return of a host slot by hand
- **WHEN** `brd lease return` runs in a host slot
- **THEN** it is refused as `host_slot`, naming `brd-host slots --local --shrink`
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_hand_return_of_a_host_slot_is_refused`)

### Requirement: Host slots: A checkout moves the lent slot to the commit by the diff

`checkout` of a run and a commit MUST move the slot lent to that run to the commit, detached, by the diff, as the get of
a reused slot does but without a `status`: a give-back, a fill and a clean leave the slot clean (its index, HEAD and
files agree, the ignored files aside). It answers `moved`: `Kept`, `Switched`, or `Forced` with git's reason. A run
without a slot is `NoSlot`, another run's slot `NotLent`.

#### Scenario: A run checks out its commit
- **WHEN** a run checks out its commit in its slot
- **THEN** the slot moves by the diff and the answer says how
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_run_checks_out_its_commit_in_its_slot_by_the_diff`)

#### Scenario: Git refuses the diff
- **WHEN** git refused the diff (exit 128, never after a signal)
- **THEN** every file is checked out, only while the table still lends the slot to the same run with the same lend: a
  call of a supervisor that died may still run, and the slot may be another run's by then

### Requirement: Host slots: The git of the host slots is fixed

A lend MUST run no git, a checkout the config, HEAD and one `reset`, a give-back one `status` (plus `reset` and `clean`
for a dirty slot, and the `maintenance` of `branchdeck.leaseMaintenance`), a shrink one config read, one `status`, HEAD,
a `reset` when the slot is not at the commit it was leased at, and the processes of a return but its config read and
its `status`.

#### Scenario: The git of a lend, a give-back and a shrink
- **WHEN** a run is lent a slot, gives it back, and a shrink returns it
- **THEN** they run the fixed processes, and a kill signals only the process that has the start time
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_host_slots_lend_give_back_and_shrink`)
  [@test] crates/sync-utils/src/process_scan_test.rs (`a_kill_signals_only_the_process_that_has_the_start_time`)

### Requirement: Host objects: Have answers the commits the host has

`have` MUST answer the full ids of the commits that the repository has as commit objects, in their order, from one
`cat-file --batch-check`.

#### Scenario: The commits a host has
- **WHEN** `have` is asked for commits
- **THEN** it answers those the repository has as commit objects, in order
  [@test] crates/git-ops/src/host_objects_test.rs (`the_commits_a_host_has_are_its_commit_objects_in_order`)
  [@test] crates/bd-lease/src/host_objects_test.rs (`a_pack_lands_under_the_ref_of_its_run_until_the_give_back_of_the_run`)

### Requirement: Host objects: Ingest writes a pack and points the ref of the host at its commit

`ingest` of a ref (`HostRef::parse`: `run:<id>` or `base:<name>`) and a commit MUST write the pack of its input into the
object store and point `refs/branch-deck/host/run/<id>` or `refs/branch-deck/host/base/<name>` at the commit, in one ref
transaction. It answers the full ref, `objects` and `stored` (`Loose` or `Pack`). The pack MUST stream through, never
whole in memory.

#### Scenario: A small pack and a large one
- **WHEN** a pack has fewer objects than `receive.unpackLimit`, else `transfer.unpackLimit`, else 100
- **THEN** it goes to `unpack-objects`, which resolves the bases of a thin pack from the object store, as
  `receive-pack` does, and a larger one to `index-pack --stdin --fix-thin`
- **AND** it reads the 12-byte header across short reads
  [@test] crates/git-ops/src/host_objects_test.rs (`a_small_pack_lands_as_loose_objects_and_a_large_one_as_a_pack`)
  [@test] crates/git-ops/src/host_objects_test.rs (`the_unpack_limit_is_the_one_of_receive_else_transfer_else_100`)

#### Scenario: A stream that is no pack
- **WHEN** the stream is no pack, or git refuses the pack
- **THEN** it fails with git's words after the rest of the stream was read, so the sender is never blocked
  [@test] crates/git-ops/src/host_objects_test.rs (`a_stream_that_is_no_pack_is_refused_and_read_to_its_end`)

### Requirement: Host objects: The name of a host ref is one safe component

A name of 1 to 128 characters of `A-Z a-z 0-9 . _ -`, the first one a letter or a digit, without `..` and not ending in
`.lock`, and a full commit id MUST be required (`HostRefRefusal`).

#### Scenario: An unsafe name or a short id
- **WHEN** `ingest` gets another name or a commit id that is not full
- **THEN** it is refused with `HostRefRefusal`
  [@test] crates/sync-utils/src/host_ref_name_test.rs (`a_host_ref_name_is_one_safe_component`)

### Requirement: Host objects: The refs of the host go when no run needs them

When a pack last moved each ref of the host MUST be kept in `$GIT_COMMON_DIR/branch-deck/host-refs.json` (its lock a
leaf, as the one of the lend table). The give-back of a run MUST delete the run ref of its pack (`deleted_ref`), also
for a run that had no slot; a run that sent no pack costs no git. With `branchdeck.leaseMaintenance` true, an ingest
MUST start the maintenance after the ref (`maintenance_started`).

#### Scenario: The give-back of a run
- **WHEN** a run that sent a pack gives its slot back
- **THEN** the run ref of its pack is deleted
  [@test] crates/bd-lease/src/host_objects_test.rs (`a_pack_lands_under_the_ref_of_its_run_until_the_give_back_of_the_run`)
  [@test] crates/git-ops/src/host_objects_test.rs (`the_refs_of_the_host_move_and_go_in_one_transaction`)

#### Scenario: Refs that no run needs
- **WHEN** a run ref has no lend and no ticket of a live process an hour after its pack, or a base ref was moved by no
  pack for 14 days
- **THEN** `reclaim` deletes it (`deleted_refs`)
  [@test] crates/bd-lease/src/host_objects_test.rs (`reclaim_deletes_the_refs_that_no_run_needs`)

### Requirement: Host objects: Show answers the text of a file and the ids of paths

`show` of a commit, a blob path and id paths MUST answer the text of the file of the blob path (its path, id and text;
none when the commit has no file there) and the object id of each id path (a blob, the tree of a directory, the commit
of a gitlink; none for no object), from one `cat-file --batch`.

#### Scenario: A file and paths
- **WHEN** `show` is asked for a file and paths of a commit
- **THEN** it answers the text of the file and the ids of the paths
  [@test] crates/git-ops/src/host_objects_test.rs (`show_answers_the_text_of_a_file_and_the_ids_of_paths`)

#### Scenario: A refused show
- **WHEN** the repository lacks the commit, a path has an empty, `.` or `..` component, or the file is over 1 MiB
- **THEN** it is refused (`ShowRefusal`)

### Requirement: Host objects: The git of the runs is the one their PATH finds first

`find_git` of `run_path` MUST find the git that the `PATH` of the runs finds first, the `bin` of Homebrew before
`/usr/bin`, with its version. The floor of a host is `HOST_MIN_VERSION` (2.56.0, above the 2.54 of brd itself, a
constant of the code).

#### Scenario: The bin of Homebrew and the version
- **WHEN** `find_git` looks for git
- **THEN** the `bin` of Homebrew comes before `/usr/bin`
- **AND** a version parses as git prints it (`2.50.1 (Apple Git-155)`, `2.45.1.windows.1`), and a release candidate
  comes before its release
  [@test] crates/lease-pool/src/host_git_test.rs (`a_version_parses_releases_candidates_and_the_apple_git`, `the_bin_of_homebrew_comes_before_usr_bin`)

### Requirement: Host objects: The ensure brings the git of the host to the floor

`ensure_host_git` MUST bring that git to the floor: nothing at the floor; below it `brew install git` when the `bin` of
Homebrew has no git, else `brew upgrade git` (in `/`, `HOMEBREW_AUTO_UPDATE_SECS=0`, `HOMEBREW_NO_INSTALL_CLEANUP=1`,
at most 30 min), and `upgraded: true`. Its waits and Homebrew go to the progress of its caller.

#### Scenario: Below the floor
- **WHEN** the git of the host is below the floor
- **THEN** the ensure installs or upgrades git with Homebrew
  [@test] crates/bd-lease/src/host_git_test.rs (`ensure_installs_or_upgrades_git_with_homebrew_below_the_floor`)
  [@test] crates/lease-pool/src/host_git_test.rs (`the_plan_installs_upgrades_or_refuses_with_a_hint`)

#### Scenario: Another Homebrew process holds the lock of git
- **WHEN** another Homebrew process holds the lock of git (`has already locked`, `already in progress`)
- **THEN** the ensure tries again every 10 s and takes a git that meets the floor meanwhile as the end of that process
  [@test] crates/lease-pool/src/host_git_test.rs (`a_lock_of_another_homebrew_process_is_told_by_its_words`)

#### Scenario: No way to the floor
- **WHEN** the host has no Homebrew, a git comes before the one of Homebrew in the `PATH`, or an upgrade fails or
  leaves the git below the floor
- **THEN** the ensure refuses as `OldGit` with the hint: the install of Homebrew on macOS, a newer source of packages
  elsewhere

#### Scenario: A test
- **WHEN** a test sets `BD_TEST_HOST_BREW` or `BD_TEST_HOST_MIN_VERSION`
- **THEN** the first names a fake Homebrew (`none`: none), the second another floor

### Requirement: Host objects: No run uses git while Homebrew replaces it

A run MUST NOT use git while Homebrew replaces it, and the lend table is the lock: the ensure records the upgrade in the
table first (`git`: `upgrading`, its process and command), then waits until no slot is lent, filled, cleaned, shrunk or
given back by a live process and no host-object call of a live process runs (`users`, below), then runs Homebrew. The
wait MUST have no limit: Homebrew never starts under a live user of git, and a gone one does not count.

#### Scenario: Lent slots and lends during an upgrade
- **WHEN** an upgrade waits for the lent slots
- **THEN** Homebrew runs once they are given back
- **AND** while a live process upgrades, a lend only queues and answers `git_upgrading` with the command, and the queue
  line of a waiting lend says that git is upgrading
  [@test] crates/bd-lease/src/host_git_test.rs (`the_upgrade_waits_for_the_lent_slots_and_the_lends_queue_while_it_runs`)

#### Scenario: A live and a gone host-object call
- **WHEN** an upgrade finds a host-object call of a live process and one of a gone process
- **THEN** it waits for the live one only
- **AND** once a minute it says the busy slots, the host-object calls with their processes and the seconds it waited
  (`… waits for the users of git (<s> s so far): 1 is lent to …, 2 host-object calls of the processes …`)
  [@test] crates/bd-lease/src/host_git_test.rs (`the_upgrade_waits_for_a_live_host_object_call_and_not_for_a_gone_one`)

#### Scenario: A failed upgrade and a refusal
- **WHEN** an upgrade fails or panics before the end of Homebrew, or the ensure refuses
- **THEN** the failed upgrade removes its record, and a refusal stays in the table (`git`: `refused`): every lend
  refuses with it as `OldGit` until a later ensure finds the floor
  [@test] crates/bd-lease/src/host_git_test.rs (`without_homebrew_ensure_and_the_lends_refuse_until_git_meets_the_floor`)

### Requirement: Host objects: Each host-object call is a git user of the table

Each host-object call (`have`, `ingest`, `show`, and `snapshot` and `changes` of Host runs) MUST be a git user of the
table while it runs: it adds its process to `users` under the lock and removes it at its end, also on an error or a
panic. The library functions do this, so each caller gets it.

#### Scenario: A call during an upgrade
- **WHEN** a live process records an upgrade and no slot is busy
- **THEN** the call waits, without a limit, until the upgrade ends or its process is gone: a change of the table wakes
  it, and it looks again every 5 s
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_host_object_call_waits_for_an_upgrade_of_git_but_a_scan_of_a_lent_run_goes_on`)

#### Scenario: A scan of a lent run
- **WHEN** a slot is busy, as the scans and the `show` of a run come while its slot is lent
- **THEN** the call adds itself, since Homebrew can't start then, and the run and the upgrade would else wait for each
  other

### Requirement: Host objects: The steps of the keeper skip while git is upgrading

While a live process upgrades git, a step that would take a slot or run git for the keeper MUST skip, with the refusal
"The git of the host is upgrading" with the command (`GitUpgrading`). The give-back of a lent run MUST still run, as
Homebrew waits for it, but its run ref stays for `reclaim`, and `reclaim` deletes no ref meanwhile.

#### Scenario: A fill, a clean, a shrink and reclaim
- **WHEN** a fill, a clean, a shrink or `reclaim` runs while git is upgrading
- **THEN** a fill adds no slot and runs no git, a clean and the clean after a give-back skip, a shrink sets its limit
  and refuses each slot past it
- **AND** each step of `reclaim` but the queue and the dead fills has the refusal as its `error`, so a later `reclaim`
  takes it up
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_fill_a_clean_a_shrink_and_reclaim_skip_while_git_is_upgrading`)

#### Scenario: Clean host during an upgrade
- **WHEN** `brd lease clean --host` runs while git is upgrading
- **THEN** it skips every host slot with that reason, before any git

### Requirement: Host objects: Two ensures never both run Homebrew

The ensure MUST record its upgrade, its refusal or the ready git under the lock of the table, and only while no live
process records an upgrade, also another call of its own process. Else it leaves that record as it is, says once a
minute that another upgrade runs and its command, waits for its end and plans the git again.

#### Scenario: A second ensure
- **WHEN** an ensure runs while a live process records an upgrade
- **THEN** it never records over it, and waits for its end
  [@test] crates/bd-lease/src/host_git_test.rs (`a_second_ensure_never_records_over_the_upgrade_of_a_live_process`)

### Requirement: Host objects: The git of the host objects is fixed

`have` and `show` MUST run one `cat-file` each, `ingest` of a small pack the config, `unpack-objects` and one
`update-ref`, and the give-back of a run that sent a pack one more `update-ref`.

#### Scenario: The git of have, ingest and show
- **WHEN** `have`, `ingest` and `show` run
- **THEN** they run the fixed processes
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_host_objects_have_ingest_and_show`)

### Requirement: Host runs: A commit of the files of a worktree is what add all stages

A commit of the files of a worktree (`worktree_commit`) MUST be what `git add --all` stages, on HEAD, without a write of
the index, the files or the refs of the worktree: one `status --porcelain=v1 -z --no-renames` of its index, read only,
in the untracked mode of the repository (its `core.fsmonitor` and `core.untrackedCache` apply).

#### Scenario: The paths the status names
- **WHEN** the status names paths of the worktree
- **THEN** a file is written by `hash-object -w --stdin-paths` (its clean filter and end of line apply) with the mode
  of its executable bit, a link by its target, a directory with `.git` as the gitlink of its HEAD, a missing file as a
  deletion, an untracked directory by its files (`ls-files --others --exclude-standard`)
- **AND** one `fast-import` writes the commit from the tree of HEAD, and one ref transaction deletes its temporary ref
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`the_commit_of_a_worktree_is_what_add_all_stages_from_its_files_or_a_copy_of_its_index`)

#### Scenario: A wide change
- **WHEN** the change of the worktree is wide
- **THEN** the objects of the commit are loose (no pack file per run)
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`a_wide_change_leaves_loose_objects_and_no_pack`)

### Requirement: Host runs: Some worktrees take a copy of the index instead

A repository with `core.fileMode` or `core.symlinks` false, a path with a newline and a nested repository without a
commit MUST take a copy of the index instead (in a private directory of the git dir, removed afterwards): one add of the
paths, or of every path above 256 paths or when a path went away meanwhile, and one `write-tree`. Both MUST give the
same commit for the same tree.

#### Scenario: The copy of the index
- **WHEN** the repository has `core.fileMode` false, or many paths changed
- **THEN** the copy of the index gives the same commit for the same tree, and adds many paths at once
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`the_commit_of_a_worktree_is_what_add_all_stages_from_its_files_or_a_copy_of_its_index`, `a_clean_worktree_is_head_and_the_copy_adds_many_paths_at_once`)

### Requirement: Host runs: The snapshot of a run answers the tree of its slot

`snapshot_run` of a run MUST answer the tree of the files of the slot lent to the run as `git add --all` stages them
(`tree`: the tracked and the untracked files that git does not ignore, a deletion, a staged deletion and rename, a mode,
a link, a gitlink, a filtered file), with `n`, `path`, `head` and `changed` (the count of the status entries); the slot
index MUST stay byte for byte. A run without a slot is `NoSlot`.

#### Scenario: The scans of a slot
- **WHEN** a run scans its slot before and after its command
- **THEN** each scan names the tree of the slot, and the index of the slot stays
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_scans_of_a_slot_name_its_tree_and_the_patch_of_its_command`)
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`the_commit_of_a_worktree_is_what_add_all_stages_from_its_files_or_a_copy_of_its_index`, `a_clean_worktree_is_head_and_the_copy_adds_many_paths_at_once`)
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`a_wide_change_leaves_loose_objects_and_no_pack`)

### Requirement: Host runs: The changes of a run stream a patch beside the patch file

`changes_run` of a run, a base tree and a patch file MUST answer the tree as `snapshot_run` does and, when it differs
from the base, stream `diff-tree -r -p --binary --full-index --no-renames <base> <tree>` into a new file beside the
patch file, then rename it onto the patch file, so a link there is replaced, never followed (`patch_bytes`). A tree
equal to the base MUST write no file.

#### Scenario: The patch of a command
- **WHEN** the command of a run changed the slot
- **THEN** `changes_run` writes the patch between the two trees
  [@test] crates/bd-lease/src/host_slots_test.rs (`the_scans_of_a_slot_name_its_tree_and_the_patch_of_its_command`)

#### Scenario: A link at the path of the patch
- **WHEN** a link lies at the path of the patch file
- **THEN** the patch replaces it and applies
  [@test] crates/git-ops/src/worktree_snapshot_test.rs (`the_patch_between_two_trees_applies_and_replaces_a_link_at_its_path`)

### Requirement: Host runs: The git of a scan is fixed

A scan MUST run the config, HEAD and its tree (one `log`) and one `status`; when something changed, one `hash-object`,
one `fast-import` and one `update-ref`.

#### Scenario: The git of the scans of a lend
- **WHEN** a run scans its slot
- **THEN** it runs the fixed processes
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_host_slots_lend_give_back_and_shrink`)

### Requirement: Host runs: The snapshot of the client answers its commit and its candidates

`git_ops::host_client::snapshot` MUST answer `commit`, `head`, `changed` (the status entries; none for `Scan::Head`,
which reads no status), `candidates` and the `base` (the tip of `origin/master`, else of `master`, absent without both).
`repository_name` answers the directory of the main worktree without `.git`. The index, the files and the refs MUST
stay.

#### Scenario: The commit of the working tree
- **WHEN** the snapshot runs with `Scan::WorkingTree`
- **THEN** `commit` is HEAD when the tree is that of HEAD, else a commit on HEAD by
  `brd-host <brd-host@brd-host.invalid>`, dated as the committer of HEAD, so the same tree on the same HEAD is the same
  commit; else `commit` is HEAD
  [@test] crates/git-ops/src/host_client_test.rs (`the_commit_of_the_working_tree_is_head_or_the_same_commit_on_head_for_the_same_tree`)

#### Scenario: The candidates
- **WHEN** the snapshot names its candidates
- **THEN** they are HEAD and its first-parent ancestors at the distances 1, 2, 4 … up to the farthest distance
  (`FARTHEST`, 65536), then the merge bases of HEAD with `origin/master` and `master`, each once
  [@test] crates/git-ops/src/host_client_test.rs (`the_candidates_are_the_powers_of_two_and_the_merge_bases`)

#### Scenario: The name of the repository
- **WHEN** `repository_name` runs
- **THEN** it answers the directory of the main worktree
  [@test] crates/git-ops/src/host_client_test.rs (`the_name_of_a_repository_is_the_directory_of_its_main_worktree`)

#### Scenario: A snapshot of brd-host
- **WHEN** `brd-host run` takes the snapshot of its checkout
- **THEN** it reads what its scan names and ships its own commit
  [@test] crates/git-ops/src/host_client_test.rs (`a_snapshot_of_bd_host_reads_what_its_scan_names_and_ships_its_own_commit`)

### Requirement: Host runs: The pack of a commit is a private file

`pack_commit_until` MUST stream `pack-objects --revs --thin --stdout --progress` into a new file of mode 0600 in the
directory of its caller (mode 0700; `<git_dir>/branch-deck/host/packs/` for `brd-host run`), never a name of the shared
temporary directory, and answer its `path` and `bytes`; `commit_count` answers the commits (`rev-list --count`). Every
id MUST be full. The host takes the pack with `lease_pool::host_refs::ingest`.

#### Scenario: The thin pack
- **WHEN** the client packs a commit
- **THEN** the pack is a private file that the host takes
- **AND** each progress line of git goes to the caller, and a minute without a line kills git
- **AND** the caller removes the file; a pack file there older than a day goes at the next pack
  [@test] crates/git-ops/src/host_client_test.rs (`the_thin_pack_is_a_private_file_that_the_host_takes`)

#### Scenario: A stop of the caller
- **WHEN** the caller stops the pack
- **THEN** git is killed, the pack fails as cancelled and leaves no file
  [@test] crates/git-ops/src/host_client_test.rs (`a_cancelled_pack_fails_and_leaves_no_file`)

### Requirement: Host runs: The patch of a run applies to the files all or nothing

`apply_patch` MUST apply the patch of `changes_run` to the files of the worktree, not its index, all or nothing
(`apply --whitespace=nowarn`), and answer the count of the files (`apply --numstat`). It is the one write of the user's
working tree beside the guarded update: what the user asked a run for (`brd-host run --apply`).

#### Scenario: A patch that applies and one that does not
- **WHEN** the client applies the patch of a run
- **THEN** a patch that applies changes the files, and one that does not fails and changes nothing
  [@test] crates/git-ops/src/host_client_test.rs (`the_patch_of_a_run_applies_to_the_files_all_or_nothing`)

#### Scenario: The only write of the working tree beside the guarded update
- **WHEN** the safety scan reads the production code
- **THEN** the apply of `apply_patch` is the one write of the working tree beside the guarded update
  [@test] crates/git-executor/src/worktree_safety_test.rs (`production_code_never_writes_the_working_tree`)

### Requirement: Host runs: The git of the snapshot and the pack is fixed

The snapshot of a clean checkout MUST run HEAD (`log`), one `rev-list`, one `for-each-ref` and a `merge-base` per base
ref; `Scan::Status` adds the config and one `status`, and `Scan::WorkingTree` with changes the writes of a scan. The
pack MUST run one `pack-objects`, and the count one `rev-list`.

#### Scenario: The git of a snapshot and a pack
- **WHEN** `brd-host run` takes a snapshot and packs its commit
- **THEN** it runs the fixed processes
  [@test] crates/git-ops/src/host_client_test.rs (`a_snapshot_of_bd_host_reads_what_its_scan_names_and_ships_its_own_commit`, `the_thin_pack_is_a_private_file_that_the_host_takes`)

### Requirement: Error codes: Each error code has its exit code

The errors of `brd lease` MUST exit with the exit code of their code as the table of the scenario gives it.

#### Scenario: The exit of each code
- **WHEN** a command of `brd lease` fails with a code
- **THEN** it exits with the exit of its row

  | Exit | Codes |
  |---|---|
  | 2 | `invalid_arguments`, `no_lease`, `lease_mismatch`, `not_a_slot`, `slot_in_pool` |
  | 3 | `locked` (the pool lock, the lock of the `worktree` commands, a clones lock no record names a live holder of; retryable) |
  | 4 | `slot_busy`, `host_slot`, `unlanded_commits`, `dirty_worktree`, `branch_exists`, `other_volume`, `low_disk_space`, `no_baseline` (a source without commits), `resolve_in_progress`, `lease_reclaimed`; `brd lease pick`: `home_branch_switched`, `no_home_branch`, `unchecked_tree` and the refusals of `brd pick` |

### Requirement: Error codes: The outputs match brd schema

The outputs MUST match `brd schema` (`lease_get`, `lease_pick`, `lease_return`, `lease_status`, `lease_adopt`,
`lease_clean`, `lease_reclaim`, `lease_trim`, `lease_disk`).

#### Scenario: The outputs of the lease commands
- **WHEN** a command of `brd lease` prints its output
- **THEN** it matches its output of `brd schema`
  [@test] crates/bd-lease/src/lease_test.rs (`lease_outputs_match_the_schema`)
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_takes_over_an_idle_lease_whose_commits_landed`, `reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)
  [@test] crates/bd-lease/src/clones_lock_test.rs (`a_get_that_must_add_a_slot_waits_for_the_one_that_adds_a_slot`)
  [@test] crates/bd-lease/src/lease_clean_test.rs (`clean_skips_a_busy_and_a_leased_slot_and_a_dry_run_deletes_nothing`)

#### Scenario: Every output in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists every output
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)

### Requirement: Error codes: The slot writes stay in lease_slot.rs

The slot writes (`checkout` twice, `reset`, `clean` twice) MUST stay in `crates/git-ops/src/lease_slot.rs`, with their
exact counts in the worktree safety scan.

#### Scenario: The worktree safety scan
- **WHEN** the worktree safety scan reads the production code
- **THEN** it finds the slot writes only in `crates/git-ops/src/lease_slot.rs`, with their exact counts
  [@test] crates/git-executor/src/worktree_safety_test.rs (`production_code_never_writes_the_working_tree`)

## Testing / Local Run

- `cargo nextest run -p bd-lease lease_test` (and `takeover_test`, the idle leases) runs the commands end to end on `TestRepo` fixtures, the pool in a
  temporary home directory, `cargo nextest run -p bd-lease clones_lock_test` the gets beside a get that adds a slot
  (one held at its registration by a wrapped git, one holder a child process that the test kills),
  `cargo nextest run -p bd-lease host_slots_test` the host slots;
  `cargo nextest run -p lease-pool lease_test` runs the seed patterns and `cargo nextest run -p sync-utils
  process_scan_test` the process scan.

## Open Questions / Risks

- A git outside the pool that reads every worktree (the user's `git worktree list` or `git checkout` of a branch, the
  `worktree list` of `brd pick` and of a resolve session) beside the registration of a new slot can read its admin
  directory half written and fail once: git takes no lock for it, and `worktrees.lock` covers only the pool's own
  commands.

- A clone copies what the template has at that moment: an ignored file that the template writes during the clone can
  be copied half written, and the checkout and the clean keep ignored files. A free slot is the preferred template
  for this reason; the source checkout, where the user and the IDE write, is the last one.

- On Windows the scan of the processes is not available: a slot with a shell in it can be leased, and a Bazel server
  whose workspace has no `bazel-out` link where the return looks (another `--symlink_prefix`) keeps running.
- The clones lock is per pool: two repositories whose pools share a volume clone at the same time, and the minimum
  free space of one does not count the clone of the other until it is done.
- A get that waits behind a queue of clones can still run past the deadline of its caller; the queue makes the clones
  finish one after the other instead of all failing together, and a freed slot ends the wait of the next get.
- A seeded path that a later commit tracks is removed by the return, and the slot shows a deletion; the next get skips
  it as dirty.
- A free slot left dirty stays in the pool, also through the eviction of a get below the minimum free space (its build
  output goes in the shrink); a get that needs it pays one `status` of it once something touched it.
- The proof of a dirty slot reads only the top-level `.gitignore` and `info/exclude`: a nested `.gitignore` or a
  global `core.excludesFile` that comes to ignore the stray file, the file itself untouched, keeps the slot skipped
  until something touches a recorded path or the slot moves.
- A takeover can't tell a resumed session of the old holder from one of the new holder once the new holder is gone
  too, nor any session where no process scan runs (Windows): there `lease_reclaimed` refuses nothing.
- A brd older than `taking_over`, `idle_check` and `reclaimed` drops them when it writes the state: the get gives the
  idle lease back from its own copy, but a get that dies meanwhile beside such a brd leaves the slot free.
- The trash of a takeover stays until the next return or clean, or a get below the minimum free space; `brd lease
  status` names it (`trash_bytes`).
- The trim finds an output user root only through a `bazel-out` link that some checkout still has: a return deletes
  the links of a slot (`out/` of the clean rules), so with no source checkout or leased slot that built, the output
  bases of the free slots stay. A get below the minimum trims the pools of every volume, not only its own.
- A slot the trim deletes takes its warm build state with it; the next get past the limit clones a new one, cold.
- A resumed session that commits between the commit point of a takeover and its move loses the commit from HEAD; the
  reflog of the slot keeps it, and its next command is refused.

## References

- [Git Operations](../../../../docs/design/git-operations.md): "Lease" and "Pick".
- [Agent CLI guide](../../../../docs/contributing/agent-cli.md): `brd lease`.
