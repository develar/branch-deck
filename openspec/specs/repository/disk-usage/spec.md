---
name: Disk Usage
description: Requirements for the Disk Usage dialog of the main window - what the lease pools of this Mac and of the hosts of brd-host hold on disk, each slot with the bytes its builds wrote and the slots a trim deletes next marked, the free space against the minimum of each machine, and Trim to Limit of this Mac's pools.
style: plain-1
targets:
  - crates/app-api/src/app/disk.rs
  - crates/app-api/src/transport.rs
  - crates/app-api/src/commands.rs
  - crates/pool-disk/src/lib.rs
  - crates/pool-disk/src/disk_test.rs
  - crates/fs-walk/src/lib.rs
  - crates/fs-walk/src/changes.rs
  - crates/pool-disk/src/step.rs
  - crates/pool-disk/src/report.rs
  - crates/lease-pool/src/trim.rs
  - crates/test-server/src/disk_fixtures.rs
  - crates/test-server/src/host.rs
  - crates/app-api/src/app/services.rs
  - src/components/disk/DiskUsageDialog.tsx
  - src/hooks/useDiskUsage.ts
  - src/hooks/useTrimPools.ts
  - src/presenters/disk.ts
  - src/stores/disk-usage-dialog.ts
  - src/lib/command-palette.ts
  - src/components/command-palette/CommandPalette.tsx
  - src/pages/IndexPage.tsx
---

# Disk Usage

Status: Implemented
Date: 2026-11-02

## Purpose

The lease pools of agent worktrees take most of a developer Mac's disk: a slot keeps what its builds wrote, `target/`
inside it and its Bazel output bases outside it, tens of gigabytes each, and the hosts that run the remote runs keep
pools of their own. Disk Usage shows, for this Mac and each host of `brd-host`, what each pool holds and which free
slots a trim deletes next, the same report as `brd lease disk --all` and `brd-host disk` ([brd lease](../../cli/lease/spec.md#disk),
[brd-host](../../cli/host/spec.md#disk)), and trims this Mac's pools to their limit. "Show Disk Usage…" of the command
palette opens it.

## Goals

- See at a glance where the space went: per machine the free space against `branchdeck.leaseMinFree`, per pool its
  bytes and its limit, per slot what its builds wrote.
- Free the space of the free slots past the limit without a terminal, as a lease below the minimum does by itself.
- One decision for every surface: which slots a trim deletes is Rust's (`trim_order`), the dialog only marks them.

## Non-goals

- A trim of a host from the dialog: the keeper of its agent frees its space by itself below its minimum, and
  `brd-host trim <alias>` trims it now ([brd-host](../../cli/host/spec.md#trim)); a host has no `brd`.
- A change of the limit or of the minimum: both are the machine's git config.

## Requirements

### Requirement: Report: The dialog reads this Mac and the hosts when it opens

The dialog MUST read `disk_usage` when it opens: this Mac's pools walked in process (`pool_disk`, the bytes of
`disk/` while they hold, the home of the app) and the hosts of `brd-host` side by side (`AppHost::host_disks`: the
app runs `brd-host disk --json` of the binary it bundles, none without one).

#### Scenario: This Mac and the hosts are read side by side
- **WHEN** the dialog opens
- **THEN** it reads `disk_usage`: this Mac's pools walked in process (`pool_disk`, the bytes of `disk/` while they
  hold, the home of the app)
- **AND** beside them the hosts of `brd-host` (`AppHost::host_disks`)
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)

#### Scenario: The hosts are what the bundled `brd-host disk --json` answers
- **WHEN** the app reads the hosts
- **THEN** it runs `brd-host disk --json` of the binary it bundles, each host its report or its refusal
  [@test] crates/app-api/src/app/disk_test.rs (`the_hosts_of_bd_host_disk_are_its_reports_and_its_refusals`)

#### Scenario: No bundled binary
- **GIVEN** the app bundles no `brd-host` binary
- **WHEN** the dialog reads `disk_usage`
- **THEN** it reads no host

### Requirement: Report: Each machine shows its free space against the minimum

The dialog MUST show this Mac first, then each host by its alias: the free space and the minimum, in the warning colour
below the minimum.

#### Scenario: This Mac first, then the hosts, each with its free space and minimum
- **WHEN** the report is shown
- **THEN** this Mac comes first, then each host by its alias
- **AND** each says its free space and its minimum ("424.3 GiB free, a lease trims the pools below 200.0 GiB"), or
  "no minimum set (branchdeck.leaseMinFree)" without one
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)
  [@test] src/presenters/disk.test.ts (`names this Mac and a host, the free space against the minimum`)

#### Scenario: A machine below the minimum
- **WHEN** a machine has less free space than its minimum
- **THEN** its free space is in the warning colour
  [@test] src/presenters/disk.test.ts (`names this Mac and a host, the free space against the minimum`)

### Requirement: Report: Each pool shows its size, its slots, its limit and its trim

Each pool MUST be shown by the name of its repository with its size, its slots, its limit and how many go with a
trim, and the Bazel output bases of its main checkout.

#### Scenario: A pool past its limit
- **WHEN** a pool is shown
- **THEN** it is named by its repository, with its size and its slots, its limit and how many go with a trim ("11
  slots, limit 10, 1 goes with a trim")
- **AND** the Bazel output bases of its main checkout
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)
  [@test] src/presenters/disk.test.ts (`words a pool, its slots largest first, and what a trim deletes`)

### Requirement: Report: Each slot shows its state, its trim mark and its size

Each slot MUST be shown, the largest first, with its number, its state, "Goes with a trim" where a trim deletes it
next, what its size is made of and its size; the output bases of gone slots MUST be shown too.

#### Scenario: The slots of a pool, largest first, with their states
- **WHEN** the slots of a pool are shown
- **THEN** the largest comes first, each with its number and its state ("Free", "Leased by lane-3", "Busy")
  [@test] src/presenters/disk.test.ts (`words a pool, its slots largest first, and what a trim deletes`)

#### Scenario: The slot a trim deletes next
- **WHEN** a trim deletes a slot next
- **THEN** that slot says "Goes with a trim"
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)

#### Scenario: What the size of a slot is made of
- **WHEN** a slot is shown
- **THEN** it says what its size is made of ("worktree 15.1 GiB, build 21.0 GiB, Bazel 8.0 GiB": what its worktree
  holds of its own beside the clone it came from, its build directories, its Bazel output bases) and its size
  [@test] src/presenters/disk.test.ts (`words a pool, its slots largest first, and what a trim deletes`)

#### Scenario: The output bases of gone slots
- **WHEN** output bases of slots that are gone remain
- **THEN** the report shows them

### Requirement: Report: A host that answers nothing shows why

A host that answers nothing MUST show why; for an agent without the feature `disk` it MUST name `brd-host upgrade
<alias>`.

#### Scenario: An agent without the feature `disk`
- **WHEN** a host answers no report
- **THEN** it shows why
- **AND** an agent without the feature `disk` names `brd-host upgrade <alias>`
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)
  [@test] crates/app-api/src/app/disk_test.rs (`the_hosts_of_bd_host_disk_are_its_reports_and_its_refusals`)

### Requirement: Report: The first event is the cached overview

The first `disk_usage` event MUST be `Cached`, carrying the full saved overview before any tree walk; it shows each
slot's `measuredAt` age and whether its bytes are cached.

#### Scenario: The saved overview comes before any walk
- **WHEN** the dialog reads `disk_usage`
- **THEN** the first event is `Cached`, the full saved overview, before any tree walk
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`)
  [@test] crates/app-api/src/app/disk_test.rs (`disk_usage_events_serialize_their_channel_type`)

#### Scenario: The age of each slot's bytes
- **WHEN** the cached overview is shown
- **THEN** each slot shows its `measuredAt` age and whether its bytes are cached
  [@test] src/presenters/disk.test.ts (`words when slot bytes were measured and disk progress`)

### Requirement: Report: The progress counts the slots as their walks complete

After the check of the history, `Planning` MUST set the exact number of slots to walk and the number that keep their
saved bytes as the history came late; a `Slot` event MUST replace a slot as its walks complete and advance the measured
count. Host answers MUST arrive as `Machine`, and `Done` ends progress.

#### Scenario: Each finished slot replaces its row and advances the count
- **WHEN** the walk of a slot completes
- **THEN** a `Slot` event replaces that slot and advances the measured count of the slots `Planning` set
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`, `a_slot_event_counts_the_slots_that_the_history_check_left`)
  [@test] src/presenters/disk.test.ts (`words when slot bytes were measured and disk progress`)
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)

#### Scenario: The hosts and the end of the progress
- **WHEN** the hosts answer and the read ends
- **THEN** the host answers arrive as `Machine`, and `Done` ends progress
  [@test] crates/app-api/src/app/disk_test.rs (`disk_usage_events_serialize_their_channel_type`)

### Requirement: Report: Closing the dialog keeps the values received

Closing the dialog MUST NOT discard values already received.

#### Scenario: The dialog closes during a read
- **WHEN** the dialog closes while the events still come
- **THEN** the values already received are kept

### Requirement: Report: On macOS an unchanged slot stays cached

On macOS, a slot whose saved mark has no change in the history MUST stay cached regardless of its age, its lease, its
return or a reclaim since it was measured: the history names what they changed.

#### Scenario: No change since the saved mark
- **GIVEN** macOS
- **WHEN** the history names no change of a slot since its saved mark, whatever its age, its lease or its return
- **THEN** the slot stays cached, and its mark moves on to the one taken before the replay
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`)

### Requirement: Report: On macOS a changed path remeasures its nearest saved node

On macOS, a changed path MUST remeasure the deepest saved part it lies in, the other parts keeping their bytes: a
subtree of at most 20,000 entries is walked again; a directory above the subtrees is read alone, its own files, its
directories that no part holds walked as new parts and those that are gone dropped; a root that no saved measure holds
is walked whole.

#### Scenario: A changed file
- **GIVEN** macOS
- **WHEN** a path changed since the saved mark
- **THEN** the deepest saved part it lies in is measured again
- **AND** the other parts keep their bytes
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`, `a_changed_path_selects_the_deepest_saved_part_of_the_deepest_root`)

#### Scenario: A file of a directory above the subtrees
- **WHEN** a file of a directory above the subtrees changed, a directory was created in it or one under it is gone
- **THEN** that directory is read alone, the new directory is walked as a part of its own, the gone one leaves the
  measure
- **AND** the bytes agree with a full walk, and no walk lies in another
  [@test] crates/pool-disk/src/disk_test.rs (`a_directory_above_the_parts_is_read_alone_and_its_new_and_gone_directories_follow`, `the_targets_of_a_refresh_never_lie_in_one_another`)

#### Scenario: The parts of a walk
- **WHEN** a tree is walked
- **THEN** its parts are the subtrees of at most the limit of entries and the directories above them, each with the bytes
  of its own files, and they add up to the tree
  [@test] crates/fs-walk/src/walk_test.rs (`the_parts_of_a_tree_are_the_subtrees_of_at_most_the_node_entries_and_the_directories_above_them`)

#### Scenario: The changed paths of the events
- **WHEN** the history names changed paths
- **THEN** each goes to the measures under whose roots it lies and whose mark it follows
  [@test] crates/fs-walk/src/changes_test.rs (`canonical_path_keeps_the_nonexistent_suffix`, `an_event_goes_to_the_queries_under_whose_roots_it_lies_and_whose_mark_it_follows`)

### Requirement: Report: Other platforms cache a slot by its return and its lease

On other platforms, where there is no history, a slot MUST stay cached while it is returned and leased as when it was
measured, a leased one only for 15 minutes (`LEASED_STALE`: its builds write while it is leased); any other slot MUST be
walked in full.

#### Scenario: A platform without FSEvents
- **GIVEN** a platform other than macOS
- **WHEN** the dialog reads `disk_usage`
- **THEN** no mark of the history is taken and every query of the history is refused
  [@test] crates/fs-walk/src/changes_test.rs (`platforms_without_fsevents_keep_no_history`)

#### Scenario: A free slot returned as when it was measured
- **GIVEN** a platform other than macOS
- **WHEN** a reclaim returns another slot, and gives it a number of an earlier free period
- **THEN** the other free slots stay cached, the reclaimed one is walked, and a measure of another reclaimed lease is
  stale
  [@test] crates/pool-disk/src/disk_test.rs (`a_reclaim_leaves_the_measures_of_the_other_free_slots_valid`, `a_reclaimed_slot_with_the_number_of_an_earlier_free_period_is_measured_again`)

#### Scenario: A leased slot measured within 15 minutes
- **GIVEN** a platform other than macOS
- **WHEN** a slot, leased as when it was measured, was measured less than 15 minutes ago
- **THEN** it stays cached

#### Scenario: A leased slot measured longer ago
- **GIVEN** a platform other than macOS
- **WHEN** a leased slot was measured 15 minutes ago or longer
- **THEN** it is walked in full

### Requirement: Report: The history of every measure is read in one replay by a deadline

On macOS the report MUST read the history of every measure it checks in one replay of a volume from the oldest of their
marks (marks far apart replay in streams of their own, all at once), by a deadline of 10 s; each event MUST go only to
the measures under whose roots it lies and whose mark it follows, whatever the order of the ids. The new mark MUST be taken before the
replay and kept only with the measure: a write the history has not shown yet comes after it, in the next replay.

#### Scenario: Events of several slots
- **WHEN** a replay names changes under the roots of two slots and of none
- **THEN** each slot gets the changes under its roots after its mark, and a change before its mark or under no root
  goes nowhere; a lost subtree (`MustScanSubDirs`, with the drop that caused it) is that subtree to walk again
  [@test] crates/fs-walk/src/changes_test.rs (`an_event_goes_to_the_queries_under_whose_roots_it_lies_and_whose_mark_it_follows`)

### Requirement: Report: A full walk where the history cannot name the changes

A measure MUST be walked in full when it has no mark, a mark of another volume or one past the current event, when the
history lost events it names no subtree of (wrapped ids, a drop without `MustScanSubDirs`, a subtree of `/`), and when
nothing was saved; a lost subtree, a mount or a moved root is that subtree to walk again.

#### Scenario: A mark that cannot be replayed
- **GIVEN** macOS
- **WHEN** the saved mark is of another volume, past the current event or none
- **THEN** the history refuses it, and the slot is walked in full
  [@test] crates/fs-walk/src/changes_test.rs (`fsevents_refuses_a_mark_from_the_future_or_from_another_device_and_no_mark`)
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`)

#### Scenario: Another set of build directories
- **WHEN** the clean rules of a slot name another build directory inside its worktree
- **THEN** the saved parts of the worktree, which held it, are of no use: the worktree is walked whole, the build
  directory apart
  [@test] crates/pool-disk/src/disk_test.rs (`a_new_build_directory_inside_the_worktree_walks_the_worktree_again`)

#### Scenario: A history that lost events
- **WHEN** the history lost events under a slot
- **THEN** a lost subtree is walked again in the slots under it or above it, and a loss of the whole history walks every
  slot in full
  [@test] crates/fs-walk/src/changes_test.rs (`an_event_goes_to_the_queries_under_whose_roots_it_lies_and_whose_mark_it_follows`)
  [@test] crates/pool-disk/src/disk_test.rs (`a_late_history_keeps_the_saved_bytes_until_a_day_has_passed`)

### Requirement: Report: A late history keeps the saved bytes

A replay that does not end by its deadline MUST leave its measures with their saved bytes and their saved mark, shown
with their age and "the file history came late", and walk them in full in the budget the other walks leave, the least
recently measured first, so their next replay is short; a measure the history has not checked for a day MUST be walked
in full with the others.

#### Scenario: The history comes late
- **WHEN** the replay of the history does not end by its deadline
- **THEN** the slot keeps its saved bytes and its mark, and `Planning` counts it as late
- **AND** the budget left after the other walks walks it in full, with a new mark
  [@test] crates/pool-disk/src/disk_test.rs (`a_late_history_keeps_the_saved_bytes_until_a_day_has_passed`)

#### Scenario: A day without a history
- **WHEN** the history came late for a slot that it has not checked for a day
- **THEN** the slot is walked in full
  [@test] crates/pool-disk/src/disk_test.rs (`a_late_history_keeps_the_saved_bytes_until_a_day_has_passed`)

### Requirement: Report: The walks run in a budget and save each slot as it ends

The walks of a report MUST run in a budget of 90 s unless `fresh`: the changed parts first, then the full walks, the
least recently measured first, ending one after another in that order. Each slot MUST be saved as its walks end (a
file a slot in `disk/`); what the budget did not reach MUST keep its saved bytes, or none, and is walked by the next
report.

#### Scenario: The budget ends before the walks
- **WHEN** the budget of a report ends before the walks of a slot
- **THEN** the slot keeps its saved bytes, or shows none, and the next report walks it
- **AND** a slot whose walks ended is saved
  [@test] crates/pool-disk/src/disk_test.rs (`the_walk_budget_leaves_the_slots_it_did_not_reach_to_the_next_report`)
  [@test] crates/fs-walk/src/walk_test.rs (`each_walk_is_told_once_as_it_ends_and_a_stop_names_no_bytes_of_an_unfinished_one`)

### Requirement: Report: One report walks a pool at a time

A report MUST walk a pool only under `disk/walk.lock` of the pool, taken at once or not at all: while another report
holds it, the report MUST show the saved bytes of the pool and write nothing.

#### Scenario: Another report walks the pool
- **WHEN** a report reads a pool that another report walks
- **THEN** it shows the saved bytes, "another report measures now", and writes nothing
  [@test] crates/pool-disk/src/disk_test.rs (`another_report_that_walks_the_pool_leaves_this_one_the_saved_bytes`)

### Requirement: Report: A slot says why it shows saved bytes that may be stale

A slot whose saved bytes were kept though it may have changed MUST say why beside their age: the history came late,
the next report measures it (the budget), another report measures now.

#### Scenario: The reasons beside the age
- **WHEN** a slot keeps its saved bytes as the history came late, the budget ended, or another report walks
- **THEN** its line says "Measured 2 hours ago, the file history came late", "…, the next report measures it" or
  "…, another report measures now"
  [@test] src/presenters/disk.test.ts (`says why a slot shows saved bytes that may be stale`)

### Requirement: Report: Sizes come in MiB and are worded binary

Sizes MUST come in MiB (`u32`, up to 4 PiB: the bindings carry no `u64`, and an `f64` comes as `number | null`) and be
worded binary, as the minimum of the git config counts them.

#### Scenario: A size is worded in binary units
- **WHEN** a size is shown
- **THEN** it is worded in binary units, as the minimum of the git config counts them
  [@test] src/presenters/disk.test.ts (`words a size in binary units, MiB below a GiB`)

### Requirement: Report: Measure Again walks every slot in full

Measure Again MUST read with `fresh`: every slot walked again in full, whatever `disk/` holds and whatever the history
says, without the walk budget; the answer shown MUST stay until the new one comes.

#### Scenario: Measure Again reads with `fresh`
- **WHEN** the user chooses Measure Again
- **THEN** the dialog reads with `fresh`, every slot walked again in full, however long it takes
- **AND** the answer shown stays until the new one comes
  [@test] crates/pool-disk/src/disk_test.rs (`the_walk_budget_leaves_the_slots_it_did_not_reach_to_the_next_report`)

### Requirement: Report: A read that a later one overtook is dropped

A read that a later one overtook MUST be dropped.

#### Scenario: A read that a later one overtook
- **WHEN** a later read overtakes an earlier one
- **THEN** the earlier read is dropped

### Requirement: Report: A failed read says why with Retry

A failed read MUST say why with Retry.

#### Scenario: The read fails
- **WHEN** the read of the report fails
- **THEN** the dialog says why, with Retry

### Requirement: Trim: Trim to Limit trims every pool of this Mac and reads again

Trim to Limit MUST run `trim_pools`, the trim of every pool of this Mac as `brd lease trim` runs it (the free slots
past `branchdeck.leaseMaxSlots` with their Bazel output bases, and the output bases of gone slots), then read the
report again.

#### Scenario: The trim deletes the free slots past the limit
- **WHEN** the user clicks Trim to Limit
- **THEN** `trim_pools` deletes the free slots past `branchdeck.leaseMaxSlots` with their Bazel output bases, and the
  output bases of gone slots, in every pool of this Mac
- **AND** the dialog reads the report again
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)

### Requirement: Trim: The toast says what went

The toast of a trim MUST say what went; a failure MUST be the toast of `notifyError` ("Cannot trim the lease pools").

#### Scenario: A trim that deleted slots
- **WHEN** the trim ends
- **THEN** its toast says what went ("Trimmed 1 slot", "Freed 70.2 GiB; 1 slot stayed, a process works in it")
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)
  [@test] src/presenters/disk.test.ts (`words the toast of a trim`)

#### Scenario: A trim that fails
- **WHEN** the trim fails
- **THEN** the toast of `notifyError` says "Cannot trim the lease pools"

### Requirement: Trim: The footer says what a trim deletes

The footer MUST say what a trim deletes; with nothing past the limit the button MUST be disabled and the footer say
"No free slot is past the limit".

#### Scenario: Slots past the limit
- **WHEN** free slots are past the limit
- **THEN** the footer says what a trim deletes ("Trim deletes 1 free slot on this Mac")
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)
  [@test] src/presenters/disk.test.ts (`says why Trim to Limit does nothing, else what it deletes`)

#### Scenario: Nothing past the limit
- **WHEN** nothing is past the limit
- **THEN** the button is disabled and the footer says "No free slot is past the limit"
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)
  [@test] src/presenters/disk.test.ts (`says why Trim to Limit does nothing, else what it deletes`)

### Requirement: Opening: Show Disk Usage opens the dialog for the selected repository

"Show Disk Usage…" of the command palette MUST open the dialog for the selected repository, whose config gives the
minimum and whose pool is reported wherever it lives; without a repository the palette MUST offer no such entry.

#### Scenario: A repository is selected
- **GIVEN** a selected repository
- **WHEN** the user runs "Show Disk Usage…" of the command palette
- **THEN** the dialog opens for that repository: its config gives the minimum, its pool is reported wherever it lives
  [@test] src/lib/command-palette.test.ts (`offers Show Disk Usage… with a repository, before Open Repository…`)

#### Scenario: No repository is selected
- **WHEN** no repository is selected
- **THEN** the palette offers no such entry

### Requirement: Opening: The dialog loads on its first opening and stays mounted

The dialog MUST be loaded on its first opening and kept mounted while closed.

#### Scenario: The dialog closes
- **WHEN** the dialog has opened once and closes
- **THEN** it stays mounted

## Testing / Local Run

- `pnpm e2e tests/e2e/disk/`: the pool of the test repository lives in its fake home, where `POST
  /repositories/{id}/lease-slots` adds free slots by real gets and returns (three, past `branchdeck.leaseMaxSlots = 2`
  of the repository: a slot costs a get and a return), and the hosts are the fixture of `POST
  /repositories/{id}/host-disks`, held (`hostDisks` of `holdRemoteAnswers`) while the test reads the progress of this
  Mac.
- `cargo nextest run -p app-api -E 'test(/hosts_of/)'`, `pnpm test:unit src/presenters/disk.test.ts`.

## Open Questions / Risks

- A walk of a slot of idea-1 is cold and takes about 20 s on the user's Mac (2026-11-21, agents building): its files
  outnumber the vnode cache. The first report after an upgrade walks the slots in budgets of 90 s, a few each, and shows
  the bytes of brd before 1.20 until then; after that a report reads the history and walks the changed parts. Other
  platforms keep their cache policy, with the same budget.
- On a Mac whose `fseventsd` lags behind builds in 15 slots, the replay of more than a few minutes of history does not
  end in 10 s (20 minutes of it took over 30 s, 2026-11-21): a report then shows the saved bytes with their age and walks
  the least recently measured slots in its budget, about two minutes in all. A watcher that keeps the changes of the
  pools live, as accio's does for its index, would spare the replay.
- A host whose agent predates the feature `disk` shows only why; `brd-host upgrade <alias>` gives it the request.
