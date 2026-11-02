---
name: Disk Usage
description: Requirements for the Disk Usage dialog of the main window - what the lease pools of this Mac and of the hosts of bd-host hold on disk, each slot with the bytes its builds wrote and the slots a trim deletes next marked, the free space against the minimum of each machine, and Trim to Limit of this Mac's pools.
style: plain-1
targets:
  - ../../crates/app-api/src/app/disk.rs
  - ../../crates/app-api/src/transport.rs
  - ../../crates/app-api/src/commands.rs
  - ../../crates/pool-disk/src/lib.rs
  - ../../crates/pool-disk/src/report.rs
  - ../../crates/lease-pool/src/trim.rs
  - ../../crates/test-server/src/disk_fixtures.rs
  - ../../crates/test-server/src/host.rs
  - ../../src-tauri/src/services.rs
  - ../../src/components/disk/DiskUsageDialog.tsx
  - ../../src/hooks/useDiskUsage.ts
  - ../../src/hooks/useTrimPools.ts
  - ../../src/presenters/disk.ts
  - ../../src/stores/disk-usage-dialog.ts
  - ../../src/lib/command-palette.ts
  - ../../src/components/command-palette/CommandPalette.tsx
  - ../../src/pages/IndexPage.tsx
---

# Disk Usage

Status: Implemented
Date: 2026-11-02

## Summary

The lease pools of agent worktrees take most of a developer Mac's disk: a slot keeps what its builds wrote, `target/`
inside it and its Bazel output bases outside it, tens of gigabytes each, and the hosts that run the remote runs keep
pools of their own. Disk Usage shows, for this Mac and each host of `bd-host`, what each pool holds and which free
slots a trim deletes next, the same report as `bd lease disk` and `bd-host disk` ([bd lease](../cli/lease.spec.md#disk),
[bd-host](../cli/host.spec.md#disk)), and trims this Mac's pools to their limit. "Show Disk Usage…" of the command
palette opens it.

## Goals

- See at a glance where the space went: per machine the free space against `branchdeck.leaseMinFree`, per pool its
  bytes and its limit, per slot what its builds wrote.
- Free the space of the free slots past the limit without a terminal, as a lease below the minimum does by itself.
- One decision for every surface: which slots a trim deletes is Rust's (`trim_order`), the dialog only marks them.

## Non-goals

- A trim of a host from the dialog: a host trims itself on its next lease below its minimum, or by `bd lease trim`
  there.
- A change of the limit or of the minimum: both are the machine's git config.

## Requirements

### Report

- The dialog must read `disk_usage` when it opens: this Mac's pools walked in process (`pool_disk`, the bytes of
  `disk.json` while they hold, the home of the app) and the hosts of `bd-host` side by side (`AppHost::host_disks`: the
  app runs `bd-host disk --json` of the binary it bundles, none without one). It shows this Mac first, then each host
  by its alias: the free space and the minimum ("424.3 GiB free, a lease trims the pools below 200.0 GiB", "no minimum
  set (branchdeck.leaseMinFree)"), in the warning colour below the minimum; each pool by the name of its repository with
  its size, its slots, its limit and how many go with a trim ("11 slots, limit 10, 1 goes with a trim"), the Bazel
  output bases of its main checkout; each slot, the largest first, with its number, its state ("Free", "Leased by
  lane-3", "Busy"), "Goes with a trim" where a trim deletes it next, what its size is made of ("worktree 15.1 GiB,
  build 21.0 GiB, Bazel 8.0 GiB": what its worktree holds of its own beside the clone it came from, its build
  directories, its Bazel output bases) and its size; the output
  bases of gone slots. A host that answers nothing shows why (an agent without the feature `disk` names `bd-host
  upgrade <alias>`).
  [@test] ../../tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)
  [@test] ../../src/presenters/disk.test.ts (`words a size in binary units, MiB below a GiB`, `names this Mac and a host, the free space against the minimum`, `words a pool, its slots largest first, and what a trim deletes`)
  [@test] ../../crates/app-api/src/app/disk_test.rs (`the_hosts_of_bd_host_disk_are_its_reports_and_its_refusals`)
- Sizes come in MiB (`u32`, up to 4 PiB: the bindings carry no `u64`, and an `f64` comes as `number | null`) and are
  worded binary, as the minimum of the git config counts them. Measure Again reads with `fresh`, every slot walked
  again; the answer shown stays until the new one comes, and a read that a later one overtook is dropped. A failed
  read says why with Retry.

### Trim

- Trim to Limit must run `trim_pools`, the trim of every pool of this Mac as `bd lease trim` runs it (the free slots
  past `branchdeck.leaseMaxSlots` with their Bazel output bases, and the output bases of gone slots), then read the
  report again; its toast says what went ("Trimmed 1 slot", "Freed 70.2 GiB; 1 slot stayed, a process works in it"),
  a failure is the toast of `notifyError` ("Cannot trim the lease pools"). The footer says what a trim deletes ("Trim
  deletes 1 free slot on this Mac"); with nothing past the limit the button is disabled and the footer says "No free
  slot is past the limit".
  [@test] ../../tests/e2e/disk/disk-usage.spec.ts (`should show the pools of this Mac and of the hosts, and trim the free slots of this Mac past the limit`)
  [@test] ../../src/presenters/disk.test.ts (`says why Trim to Limit does nothing, else what it deletes`, `words the toast of a trim`)

### Opening

- "Show Disk Usage…" of the command palette opens the dialog for the selected repository, whose config gives the
  minimum and whose pool is reported wherever it lives; without a repository the palette offers no such entry. The
  dialog is loaded on its first opening and kept mounted while closed.
  [@test] ../../src/lib/command-palette.test.ts (`offers Show Disk Usage… with a repository, before Open Repository…`)

## Testing / Local Run

- `pnpm e2e tests/e2e/disk/`: the pool of the test repository lives in its fake home, where `POST
  /repositories/{id}/lease-slots` adds free slots by real gets and returns, and the hosts are the fixture of `POST
  /repositories/{id}/host-disks`.
- `cargo nextest run -p app-api -E 'test(/hosts_of/)'`, `pnpm test:unit src/presenters/disk.test.ts`.

## Open Questions / Risks

- A walk of every slot with its worktree takes minutes the first time (five on the user's Mac, 860 GB in 32 slots); the
  report answers from `disk.json` after it, and a leased slot is walked again after 15 minutes. The dialog shows the
  last answer while Measure Again walks.
- A host whose agent predates the feature `disk` shows only why; `bd-host upgrade <alias>` gives it the request.
