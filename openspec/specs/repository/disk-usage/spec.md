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
  - crates/pool-disk/src/report.rs
  - crates/lease-pool/src/trim.rs
  - crates/test-server/src/disk_fixtures.rs
  - crates/test-server/src/host.rs
  - src-tauri/src/services.rs
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
slots a trim deletes next, the same report as `brd lease disk` and `brd-host disk` ([brd lease](../../cli/lease/spec.md#disk),
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
`disk.json` while they hold, the home of the app) and the hosts of `brd-host` side by side (`AppHost::host_disks`: the
app runs `brd-host disk --json` of the binary it bundles, none without one).

#### Scenario: This Mac and the hosts are read side by side
- **WHEN** the dialog opens
- **THEN** it reads `disk_usage`: this Mac's pools walked in process (`pool_disk`, the bytes of `disk.json` while they
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
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_cached_overview_then_each_finished_slot`)
  [@test] crates/app-api/src/app/disk_test.rs (`disk_usage_events_serialize_their_channel_type`)

#### Scenario: The age of each slot's bytes
- **WHEN** the cached overview is shown
- **THEN** each slot shows its `measuredAt` age and whether its bytes are cached
  [@test] src/presenters/disk.test.ts (`words when slot bytes were measured and disk progress`)

### Requirement: Report: The progress counts the slots as their walks complete

After FSEvents planning, `Planning` MUST set the exact number of slots to walk; a `Slot` event MUST replace that slot
as its walk completes and advance the measured count. Host answers MUST arrive as `Machine`, and `Done` ends progress.

#### Scenario: Each finished slot replaces its row and advances the count
- **WHEN** the walk of a slot completes
- **THEN** a `Slot` event replaces that slot and advances the measured count of the slots `Planning` set
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_cached_overview_then_each_finished_slot`)
  [@test] src/presenters/disk.test.ts (`words when slot bytes were measured and disk progress`)
  [@test] tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)

#### Scenario: The hosts and the end of the progress
- **WHEN** the hosts answer and the read ends
- **THEN** the host answers arrive as `Machine`, and `Done` ends progress
  [@test] crates/app-api/src/app/disk_test.rs (`disk_usage_events_serialize_their_channel_type`)

### Requirement: Report: Measure Again walks everything, closing keeps what came

Measure Again MUST always request a full walk; closing the dialog MUST NOT discard values already received.

#### Scenario: Measure Again
- **WHEN** the user chooses Measure Again
- **THEN** a full walk is requested

#### Scenario: The dialog closes during a read
- **WHEN** the dialog closes while the events still come
- **THEN** the values already received are kept

### Requirement: Report: On macOS an unchanged slot stays cached

On macOS, a slot whose saved volume mark has no changes MUST stay cached regardless of its age or lease.

#### Scenario: No change since the saved mark
- **GIVEN** macOS
- **WHEN** a slot's saved volume mark has no changes, whatever its age or its lease
- **THEN** the slot stays cached
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_cached_overview_then_each_finished_slot`)

### Requirement: Report: On macOS a changed path remeasures its nearest saved node

On macOS, a changed path MUST remeasure the nearest saved node (through depth 3), preserving the other node sizes.

#### Scenario: A changed file
- **GIVEN** macOS
- **WHEN** a path changed since the saved mark
- **THEN** the nearest saved node (through depth 3) is measured again
- **AND** the other node sizes are preserved
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_cached_overview_then_each_finished_slot`, `a_changed_file_selects_its_nearest_saved_node_and_uncached_paths_select_a_root`)

#### Scenario: The changed paths of the events
- **WHEN** FSEvents names changed paths
- **THEN** the changed paths of the slot select the saved nodes measured again
  [@test] crates/fs-walk/src/changes_test.rs (`canonical_path_keeps_the_nonexistent_suffix`, `changed_event_paths_are_filtered_to_requested_roots`)

### Requirement: Report: A full walk where a partial sum could double-count

A node or root with hardlinks, a newly created hardlink or directory, missing marks, a different root set or volume,
incomplete FSEvents history, and Measure Again MUST require a full walk so a partial sum never double-counts a linked
file.

#### Scenario: A node or root with hardlinks
- **WHEN** a changed path lies in a node or root with hardlinks
- **THEN** the slot is walked in full
  [@test] crates/pool-disk/src/disk_test.rs (`a_changed_file_selects_its_nearest_saved_node_and_uncached_paths_select_a_root`)

#### Scenario: A newly created hardlink or directory
- **WHEN** a hardlink or a directory is newly created
- **THEN** the slot is walked in full
  [@test] crates/fs-walk/src/changes_test.rs (`a_new_hardlink_requires_a_full_slot_measurement`)

#### Scenario: Missing marks
- **WHEN** the saved mark of a slot is missing
- **THEN** the slot is walked in full
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_cached_overview_then_each_finished_slot`)

#### Scenario: A different root set or volume
- **WHEN** the mark is of a different volume, or the roots are a different set
- **THEN** the slot is walked in full
  [@test] crates/fs-walk/src/changes_test.rs (`fsevents_refuses_a_mark_from_another_device`)

#### Scenario: Incomplete FSEvents history
- **WHEN** the FSEvents history since the mark is incomplete
- **THEN** the slot is walked in full
  [@test] crates/fs-walk/src/changes_test.rs (`fsevents_refuses_a_mark_from_the_future`)

#### Scenario: Measure Again walks in full
- **WHEN** the user chooses Measure Again
- **THEN** every slot is walked in full

### Requirement: Report: Other platforms keep the returned and leased cache rules

On other platforms, the system MUST retain the current returned/leased cache rules because there is no FSEvents
history.

#### Scenario: A platform without FSEvents
- **GIVEN** a platform other than macOS
- **WHEN** the dialog reads `disk_usage`
- **THEN** the current returned/leased cache rules decide what is walked, as there is no FSEvents history
  [@test] crates/fs-walk/src/changes_test.rs (`platforms_without_fsevents_always_require_a_full_walk`)
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_cached_overview_then_each_finished_slot`)

### Requirement: Report: Sizes come in MiB and are worded binary

Sizes MUST come in MiB (`u32`, up to 4 PiB: the bindings carry no `u64`, and an `f64` comes as `number | null`) and be
worded binary, as the minimum of the git config counts them.

#### Scenario: A size is worded in binary units
- **WHEN** a size is shown
- **THEN** it is worded in binary units, as the minimum of the git config counts them
  [@test] src/presenters/disk.test.ts (`words a size in binary units, MiB below a GiB`)

### Requirement: Report: Measure Again reads fresh, the shown answer stays until the new one

Measure Again MUST read with `fresh`, every slot walked again; the answer shown MUST stay until the new one comes, and
a read that a later one overtook MUST be dropped.

#### Scenario: Measure Again reads with `fresh`
- **WHEN** the user chooses Measure Again
- **THEN** the dialog reads with `fresh`, every slot walked again
- **AND** the answer shown stays until the new one comes

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

- A walk of every slot with its worktree takes minutes the first time (five on the user's Mac, 860 GB in 32 slots); the
  dialog shows the last answer immediately and updates it with progress as each slot finishes. A refresh on macOS walks
  only changed nodes, while other platforms keep their previous cache policy.
- A host whose agent predates the feature `disk` shows only why; `brd-host upgrade <alias>` gives it the request.
