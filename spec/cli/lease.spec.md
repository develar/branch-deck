---
name: bd lease
description: Requirements for bd lease, the pool of worktrees that agent sessions lease, one session per slot: get moves a free slot to the HEAD of the source, else takes over an idle lease whose work landed, else adds one and names where its commits land, pick lands them on the branch of the source, return frees it and applies the clean rules of .branchdeck.toml, status reads the state, adopt adds existing worktrees, clean applies the rules on demand; the host slots are leases of the pool that a lend table lends to one run at a time, which the agent of bd-host drives in process.
style: plain-1
targets:
  - ../../crates/bd-lease/src/lease_command.rs
  - ../../crates/bd-cli/src/lib.rs
  - ../../crates/bd-update/src/pick_command.rs
  - ../../crates/update-core/src/pick.rs
  - ../../crates/git-ops/src/pick.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/lease-pool/src/lease.rs
  - ../../crates/lease-pool/src/landed_work.rs
  - ../../crates/lease-pool/src/agent_workspace.rs
  - ../../crates/bd-lease/src/workspace_command.rs
  - ../../crates/git-ops/src/landed_pairing.rs
  - ../../crates/lease-pool/src/host_slots.rs
  - ../../crates/lease-pool/src/host_refs.rs
  - ../../crates/lease-pool/src/host_git.rs
  - ../../crates/git-ops/src/host_objects.rs
  - ../../crates/git-ops/src/worktree_snapshot.rs
  - ../../crates/git-ops/src/host_client.rs
  - ../../crates/lease-pool/src/state_file.rs
  - ../../crates/lease-pool/src/lease_clean.rs
  - ../../crates/lease-pool/src/check_record.rs
  - ../../crates/lease-pool/src/clone_dir.rs
  - ../../crates/lease-pool/src/free_space.rs
  - ../../crates/lease-pool/src/bazel_servers.rs
  - ../../crates/bd-core/src/error.rs
  - ../../.branchdeck.toml
  - ../../crates/sync-utils/src/process_scan.rs
  - ../../crates/sync-utils/src/size.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/git-ops/src/lease_slot.rs
  - ../../crates/git-executor/src/worktree_safety_test.rs
---

# bd lease

Status: Implemented
Date: 2026-10-19

## Summary

Agent sessions that share a checkout each work in a worktree of their own and land their commits with `bd lease pick`
([bd pick](pick.spec.md)). `bd lease` keeps the pool of those worktrees. `bd lease get` leases a slot at the HEAD of
the source checkout: a free slot is moved there, so only the diff between the two commits is written, and a new slot
is added only when no slot is free and no idle lease, one whose holder process is gone, holds only landed work. Its
answer names where the commits of the lease land (`home`) and what the session runs (`next`), so an agent never works
out the landing itself. `bd lease pick` in the lease lands each commit on that
branch, onto its tip as it is then, and the lease follows. `bd lease return` frees the slot once its commits landed. The state in
`$GIT_COMMON_DIR/branch-deck/leases.json` is the authority. A free slot is pool property, and the next get replaces
its content, so no ancestry check decides whether a slot is free. How it works: "Lease" in
[Git Operations](../../docs/design/git-operations.md). Working beside other sessions: `bd help worktrees`
([Agent CLI](agent-cli.spec.md)). Whether an agent leases at all, `branchdeck.agentLease`, says
[bd workspace](workspace.spec.md), which it runs before its first edit.

## Goals

- A get in a repository of 1.4M files takes seconds when a slot is free: one `status` and a move of the slot by the
  diff, never a full checkout and never a scan of the other slots.
- A slot whose last commit a rewrite of master made unreachable is reused as any other free slot.
- `bd lease status` answers from the state in milliseconds: no `git status` of any slot.
- Concurrent gets never get the same slot and never wait for a `bd update`; a get that reuses a slot never waits for
  another's checkout, and a pool adds one slot at a time, so a burst of gets never clones a large checkout many times
  at once and a slot freed meanwhile wins over a new one.
- A return never loses work silently: unlanded commits, local changes and a process of another session in the slot
  refuse it. Nothing that cannot hurt the slot refuses it: the session that returns it, git's background maintenance,
  the Bazel servers of the slot, which the return stops itself.
- A get never starts on a volume that the builds of the slot would fill (`branchdeck.leaseMinFree`), and a new slot
  never carries the build output of the checkout it was cloned from.
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

## Requirements

### Get

- The answer of a get must name where the commits of the lease land, `home`: `checkout` (the source checkout, its top
  level) and `branch` (the branch it had checked out, `null` for a detached HEAD), and `next`, what the session runs in
  order: `bd -C <path> lease pick` after each commit that builds and passes its tests, `bd lease return --lease-id
  <id>` once the task is done. With no branch in `home` there is nothing to pick onto, and `next` is only the return.
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`the_get_names_the_home_of_the_lease_and_the_commands_of_the_session`)
- `bd lease get` must lease a free slot of the pool of this repository, moved to the exact HEAD commit of `-C` (or the
  current directory), detached, and print `path`, `lease_id` (a UUID), `holder` (`--holder`, default `agent-<uuid>`),
  `slot` (the worktree id), `source_head`, `leased_at`, `reused`, `seeded` and `processes_checked`. A slot whose
  commit master no longer holds, after a rewrite of master, must be reused like any free slot.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)
- With a minimum free space, `--min-free <size>` or else `branchdeck.leaseMinFree` of the git config (a number and a
  binary unit, `200G` or `200GiB`, K to P, as `df -h` counts; `0` and no setting check nothing), a get must refuse
  before it reserves anything when the volume of the pool has less free space (`f_bavail` of `statfs`):
  `low_disk_space` (exit 4), the message naming the free space and the minimum in GiB, the free slots and the leases
  whose holder process is gone. A size that does not parse is `invalid_arguments`. The minimum is the machine's, so it
  is git config, never `.branchdeck.toml`. A successful get prints `free_bytes`; where the system does not tell it
  (Windows) nothing is checked. A get that adds a slot must check again under the clones lock (below), before its
  reserve there, so a queue of gets never adds slots past the minimum: a get that waited for the lock reads
  `branchdeck.leaseMinFree` again, and refuses with nothing reserved.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_free_space_refuses_and_names_it`)
  [@test] ../../crates/sync-utils/src/size_test.rs (`a_size_is_a_number_and_a_binary_unit`)
  [@test] ../../crates/bd-lease/src/clones_lock_test.rs (`a_get_that_waited_for_the_clones_lock_checks_the_minimum_free_space_under_it`)
- A free slot must be a linked worktree of the same git common dir whose directory exists, and no process of another
  session may work in it; a busy free slot is skipped (`skipped`, `reason: "busy"`, `processes` [{`pid`, `command`,
  `argv`, `kind`, `workspace`?}]). The source may itself be a leased slot.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_busy_free_slot_is_skipped`, `adopt_adds_clean_linked_worktrees_and_refuses_the_rest`)
- A get must stop the Bazel servers of a free slot it reserved (`--workspace_directory` in the slot, wherever their
  working directory is) with SIGTERM, wait up to 30 s, name them in `stopped_servers` and lease the slot: the slot is
  the pool's, a server serves only it, and a `--watchfs` server could miss the events of the checkout and build
  stale. A server that does not exit gives the slot back free and skips it as busy. On Windows the get finds and
  stops them by their output base, as the return does ([Return](#return)).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_get_stops_the_bazel_server_of_a_free_slot_and_leases_it`)
- A get must record the process it runs for, the nearest ancestor that is no shell or wrapper (the coding agent, the
  terminal), with its start time (`holder_pid`, `holder_started`): the `pid` of a lease is of the get, which exits at
  once. A gone holder alone frees no lease: its slot may hold unlanded work, and a resumed session runs as another
  process. A get that adds a new slot names the `ready` leases whose holder process is gone (`idle_leases`, each with
  its `lease_id` and `idle`), and so does `low_disk_space`.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`)
- After the free slots, before it adds a slot, a get must take over an idle lease: `ready`, its holder process gone,
  no host slot, a linked worktree of this repository, no process in it but its Bazel servers, and quiet for an hour,
  its get and the last move of its HEAD (the mtime of the slot's `logs/HEAD`), since a holder recorded as a wrapper
  that exits at once (`cargo run`) must not lose the lease its agent just began; the oldest first. It must take the
  lease over only when every commit of the lease that its home branch lacks (`HEAD ^refs/heads/<home> ^<leased
  commit>`, whatever other branch, its own `-b` one too, holds it) is proven on that branch by the landed rule of the
  return ([Return](#return)), the slot has no local changes, no git operation in progress (`rebase-merge`,
  `rebase-apply`, `MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`, `sequencer`, `BISECT_LOG` in its git dir) and no
  open resolve session. A lease without a home branch is taken over only without commits of its own.
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`a_get_takes_over_an_idle_lease_whose_commits_landed`, `a_get_takes_over_an_idle_lease_whose_commit_landed_with_a_conflict_resolution`)
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`an_idle_lease_on_its_own_branch_is_taken_over_only_when_home_has_its_commits`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_takes_over_an_idle_lease`)
- The answer of a takeover must name the lease it took over (`took_over`: `lease_id`, `holder`, `leased_at`, its
  `branch`, which stays, and `landed`, each commit with its proof) and the slot keep its record (`reclaimed` of
  `bd lease status`) until the next get of the free slot. The takeover is a return of the old lease by the pool: its
  seeded paths removed, HEAD detached, the clean rules of the source applied into the trash, which the get leaves for
  the next return or clean to delete (the trash of a run that is gone), then the move of a reused slot.
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`a_get_takes_over_an_idle_lease_whose_commits_landed`)
- An idle lease with work its home branch lacks must never be taken over: the get skips it (`skipped`, `reason:
  "idle_with_work"`, `idle` with its `commits`, `paths` or `operation`), records the check in the lease
  (`idle_check`) and adds a slot, and `bd lease status` shows it as `idle_with_work`. A later get checks it again only
  when what the verdict rests on moved: the tree work while the slot (its `HEAD`, the length and the mtime of its
  reflog) is unchanged, the commits also while the home tip is; else it costs one `rev-parse`. One get checks at most
  two idle leases in full.
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`an_idle_lease_with_unlanded_work_is_kept_and_named`, `a_kept_idle_lease_is_checked_again_only_when_its_slot_or_home_moved`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_passes_over_an_idle_lease_it_checked`)
- While it checks, the get must hold the slot by a `preparing` lease that keeps the idle one (`taking_over`); a check
  that finds work, an error and a get whose process is gone give it back (from the copy of the get, so a bd that wrote
  the state without the field loses nothing), in the reserve of the next get, the mark of `bd lease clean`, a return
  and the status alike. Its commit point is under the pool lock: the lease still the get's and the slot as the check
  found it (a commit of a resumed session meanwhile stops the takeover); from there a get that dies leaves the slot
  free, as its work landed.
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`a_get_killed_while_it_checks_an_idle_lease_gives_the_lease_back`)
  [@test] ../../crates/lease-pool/src/lease_test.rs (`a_commit_during_the_check_stops_the_takeover`)
- A get whose volume is below the minimum free space with trash in the pool must delete the trash of runs that are
  gone first (what a takeover left there) and measure again; `low_disk_space` names what the trash of a running
  process keeps.
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`a_get_below_the_minimum_free_space_deletes_the_trash_of_gone_runs_first`)
- A free slot with local changes (one `status --porcelain=v1`, the repository's own `core.fsmonitor` applies) must be
  skipped (`skipped`, `reason: "dirty"`, `paths`), stay free, and be marked dirty;
  later gets must try a dirty slot after the clean ones. With no free slot left, the get must add a new slot
  (`reused: false`).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_dirty_free_slot_is_skipped_and_a_new_slot_is_added`)
- Every `status` of the pool (a get, the check of a clone, a return, an adopt) must list untracked files with
  `--untracked-files=all` when the repository's `status.showUntrackedFiles` is `all`, else with `normal`, also for
  `no`: git's untracked cache answers only in the mode it was built for, and git builds it only in the configured mode.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`every_status_of_the_pool_lists_untracked_files_in_the_mode_the_repository_configures`)
- Every `status` of the pool must be allowed to write the slot's index (`GIT_OPTIONAL_LOCKS=1`), so its refreshed
  stat data, fsmonitor token and untracked cache stay: after a get and its return, a `status` that may not write must
  find the untracked cache filled.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_status_of_a_slot_leaves_its_untracked_cache_warm`)
- Among the clean free slots a get must take the one returned last first, then the ones never returned in the order
  they joined the pool: the slot returned last has the warmest index and caches and the commit closest to the source.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_free_slot_returned_last_is_leased_first`)
- Every checkout and move of the pool (a reused slot, a clone, a new slot after its registration) must run with
  `-c checkout.workers=0` (one worker per core), unless the repository config sets `checkout.workers`.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`every_checkout_of_the_pool_runs_with_a_worker_per_core_unless_the_repository_sets_it`)
- A get of a reused slot, after its clean `status`, must move the slot to the source HEAD by the diff of the two
  commits: `reset --merge` of its detached HEAD (a HEAD on a branch is detached first, the branch stays), which reads
  and writes only the paths of the diff; a slot at the source HEAD writes nothing. The new branch of `-b` is created
  and HEAD attached to it in one ref transaction, which reads no file. Every file is checked out (`checkout --force`)
  only when git refuses the diff (an untracked file or a local change in a path of it), with git's reason among the
  steps on stderr. The move keeps the repository's `core.fsmonitor` (a git that writes the index without it drops the
  token, and the next `status` walks every file). An `index.lock` that no process owns (a git killed in the slot) is
  removed, and one of a live process is waited for.
  [@test] ../../crates/git-ops/src/lease_slot_test.rs (`a_clean_slot_moves_by_the_diff_and_one_at_the_commit_stays`, `a_slot_on_a_branch_is_detached_before_the_move_and_its_branch_stays`)
  [@test] ../../crates/git-ops/src/lease_slot_test.rs (`an_untracked_file_in_a_path_of_the_diff_refuses_the_move`, `a_new_branch_is_attached_in_one_transaction_and_an_existing_one_fails_it`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_get_removes_the_index_lock_a_killed_git_left_in_the_slot`)
- On macOS a new slot must be an APFS clone (`created_by: "clone"`, `cloned_from`) of a template: a free slot of this
  repository that the get passed over, marked `templating` while it is cloned, else the source checkout. The clone
  must get the `.git` file of a `worktree add --no-checkout --lock` (unlocked once the clone is at its path, so a
  `worktree prune` beside it, the user's or a gc's, never deletes the registration), the template's index refreshed
  with `core.checkStat=minimal`, then `checkout --force` to the source HEAD, `clean -fd` and a clean `status`: the
  template's local changes, staged files and untracked files never reach the lease, its ignored files do, but for
  what the `delete` clean rules of the source delete: those paths are left out of the clone, but for their `except`
  entries (a rule below a kept entry applies too), a link among them as the link, and the `.git` of the template with
  them, so a new slot starts as a
  returned one and a clone of a main checkout never copies its build output (in idea-1 188 GB and 760,000 files of
  `out/`, its `bazel-*` links into the output base of the main checkout among them). The directories on the way to a
  left-out path are made anew and the rest of each is cloned entry by entry (`clonefile` with `CLONE_NOFOLLOW`);
  without such rules the whole tree is one `clonefile`. A tracked file under a left-out path comes back with the
  checkout. A rules file that can't be read clones everything. The template must be free again afterwards, and the
  source unchanged.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_new_slot_is_a_clone_of_the_source_without_its_local_work`, `a_new_slot_is_a_clone_of_a_free_slot`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_get_of_a_new_slot_by_a_clone`)
  [@test] ../../crates/git-ops/src/lease_slot_test.rs (`a_prune_while_a_cloned_slot_is_away_keeps_its_registration_until_the_unlock`)
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`a_cloned_slot_leaves_out_what_the_delete_rules_delete`)
  [@test] ../../crates/lease-pool/src/clone_dir_test.rs (`a_clone_leaves_out_the_skipped_paths_but_for_their_kept_entries`, `a_rule_below_a_kept_entry_applies_too`)
- A clone that fails (another volume, no APFS, a file it can't read, not macOS) must leave nothing behind and fall
  back to `git worktree add --no-checkout` and a full checkout (`created_by: "checkout"`).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_clone_that_fails_falls_back_to_a_checkout`)
- A new slot must be `<pool>/<n>/<repository name>`, so the leaf directory keeps the name of the repository. The pool
  directory is `~/.branch-deck/pool/<repository name>-<8 hex digits of the SHA-256 of the canonical common dir>`, or
  `branchdeck.leasePool` of the git config.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)
  [@test] ../../crates/lease-pool/src/lease_test.rs (`the_repository_name_is_the_directory_of_its_main_worktree`)
- Two gets at the same time must never lease the same slot: each records its lease as `preparing` under the pool lock
  (`leases.lock`, not the repository lock), then moves its slot without the lock.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`two_leases_never_get_the_same_slot`)
- A pool must add one slot at a time: a get that finds no free slot takes the clones lock (`clones.lock` beside the
  state, an exclusive file lock), reserves again under it, so that a slot a return freed meanwhile wins over a new one
  (`reused: true`), and holds it until its new slot is ready or given back: the clone of a template or of the source,
  or the registration and the full checkout, and the seed. On 2026-10-24 fourteen gets cloned the idea-1 checkout at
  once, several ran past the deadline of their caller, and the disk stalled for 1.5 h. A get that reuses a slot never
  takes the lock. A get that waits must say so on stderr when the wait starts, when the holder or the slot it adds
  changes and once a minute, naming the holder's command, pid and how long it holds the lock, and the new slot and
  the holder of its lease once it reserved one, and must print how long it waited (`waited_for_clone_ms`, omitted when
  it waited for none). The wait lasts while the record of the holder (`clones.lock.holder`, as the repository lock
  writes it) names a live process: a holder that is killed frees the lock with its process, on every platform; a lock
  no record names a live holder of is `locked` after the lock timeout. The order of the locks is `clones.lock`,
  `leases.lock`, `worktrees.lock`; a return and a clean never take the clones lock.
  [@test] ../../crates/bd-lease/src/clones_lock_test.rs (`a_get_that_must_add_a_slot_waits_for_the_one_that_adds_a_slot`)
  [@test] ../../crates/bd-lease/src/clones_lock_test.rs (`a_slot_freed_while_a_get_waits_for_the_clones_lock_wins_over_a_new_one`)
  [@test] ../../crates/bd-lease/src/clones_lock_test.rs (`a_killed_holder_frees_the_clones_lock_with_its_process`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`two_leases_never_get_the_same_slot`)
- A get must print each step on stderr as it starts (`bd lease get: <step>`): the reserve of a free slot, under the
  clones lock the reserve of a slot freed meanwhile or a new one, the wait for the clones lock, the stop of the Bazel
  servers, the `status` of a slot, the clone of a template, the refresh of its index, the checkout and the seed; the
  JSON goes to stdout at the end. The core tells the same steps to its caller (`LeaseGetParams::progress`).
  [@test] ../../crates/bd-lease/src/clones_lock_test.rs (`the_steps_of_a_get_arrive_in_order`)
- The `worktree` commands of the pool (the registration of a new slot, its unlock, the move of an adopt) must run one
  at a time, under `worktrees.lock` beside the state; a registration and an unlock without the pool lock, which no
  git holds for a get: git writes the admin directory of a new worktree in several files, and a `worktree` command
  beside it reads them half written (an empty `commondir`) and fails. A new slot is registered with
  `worktree add --no-checkout` and gets its files after the lock, by a checkout or a clone; the move of an adopt that
  can't wait for the lock is refused, naming it. Two gets never register beside each other: the clones lock comes
  first.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`two_leases_never_get_the_same_slot`, `no_worktree_command_of_the_pool_runs_beside_the_registration_of_a_new_slot`)
- A get must run no `worktree prune`: run beside the `worktree add` of another get, it deletes the admin directory that
  add has just made, before git locks it, and that get fails. A new slot is registered with `worktree add --force`,
  which drops only a registration of its own path whose directory is gone (a slot deleted by hand): the next new slot
  takes that path.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_slot_deleted_by_hand_gives_its_path_to_the_next_new_slot`)
  [@test] ../../crates/git-ops/src/lease_slot_test.rs (`a_new_slot_takes_over_the_registration_of_a_slot_deleted_by_hand`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_get_of_a_new_slot_by_a_clone`)
- A get and a return must take no repository lock: a sync of the repository beside them (another agent's `bd sync`,
  the app's) neither waits for them nor holds them, and the lease is cut at the HEAD that sync reads.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_get_and_a_return_beside_a_held_sync_do_not_wait_for_it`)
- `--branch <name>` (`-b`) must create the branch at the source HEAD and check it out in the slot; a branch that
  exists must be `branch_exists` (exit 4), an invalid name `invalid_arguments` (exit 2).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_lease_on_a_new_branch_keeps_the_branch_after_the_return`)
- The get must copy into the slot the ignored files and directories that `.worktreeinclude` at the root of the source
  names (`#` comments; a literal path, or a glob of `*`, `?` and `[…]` in its last component; always relative to the
  root), and record them in the lease (`seeded`). A tracked file or one that git does not ignore must not be seeded.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_seed_copies_the_included_ignored_files_and_the_return_removes_them`)
  [@test] ../../crates/lease-pool/src/lease_test.rs (`the_seed_candidates_are_literal_paths_and_globs_of_one_directory`, `a_glob_matches_by_stars_question_marks_and_classes`)
- A get that fails after it recorded the lease must give the slot back (free, or dropped when it was never added) and
  say so in the message; it must never leave the lease `preparing`. A `preparing` lease whose process (its pid and
  start time) is gone must be taken as free by the next get, on Windows too, which has no process scan but tells the
  start of a process.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_failed_get_rolls_the_lease_back`, `a_preparing_lease_of_a_gone_process_is_taken_by_the_next_get`)
- A get of a free slot must run a fixed number of git processes: the repository, the source HEAD, the pool config,
  one `status` and the HEAD of the slot, plus one `reset` when the slot is at another commit.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_get_and_return`)
- The process scan must find a process by its working directory, compared by path components, or by the
  `--workspace_directory` of a Bazel server, and leave out the session that runs `bd`: `bd`, its ancestors and every
  process of a process group one of them is in (the `jq` of `bd lease return | jq`, the `caffeinate` of the agent;
  not the group of the init process). A background task of the agent leads a group of its own and still counts.
  Three more never work in a slot: git's background maintenance (a `git maintenance` or a `git gc --auto` and every
  git below it in the parent chain, told by argv), which writes only the shared object store; git's fsmonitor daemon
  (`git fsmonitor--daemon`, told by argv), which only watches the worktree; and `caffeinate`, which holds a power
  assertion and opens no file (a command it wraps is a process of its own and counts). A recorded start
  time must tell a process from a later one with the same pid.
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`a_process_in_a_directory_is_inside_it_and_this_process_and_its_group_are_left_out`, `a_process_runs_only_with_the_start_time_it_was_recorded_with`)
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`the_session_is_the_ancestors_and_their_groups`, `git_maintenance_the_fsmonitor_daemon_and_caffeinate_do_not_work_in_a_directory`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_process_of_the_group_of_bd_does_not_keep_the_slot_busy`)

### Return

- `bd lease return` must free the lease of the worktree of `--workspace` (default: of `-C`, else of the current
  directory): remove the seeded paths (`removed`, each moved into the trash as a whole), detach HEAD when it is on a
  branch (`detached_branch`, the branch stays) and leave the slot at its commit.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_seed_copies_the_included_ignored_files_and_the_return_removes_them`, `a_lease_on_a_new_branch_keeps_the_branch_after_the_return`)
- `--lease-id` without `--workspace` must return the slot that holds that lease when the worktree of `-C` (or of the
  current directory) holds no lease or another one, so a coordinator returns the lease of another session from the
  shared checkout; an id that no slot holds keeps the refusals below.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_by_lease_id_from_another_checkout_returns_the_slot_of_the_lease`, `a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot`)
- It must refuse, with nothing changed: a worktree without a lease (`no_lease`, exit 2), another `--lease-id`
  (`lease_mismatch`, exit 2), a process of another session in the worktree (`slot_busy`, exit 4, `processes`
  [{`pid`, `command`, `argv` (at most 200 characters), `kind`: `bazel_server` or `other`, `workspace`?}], the message
  naming each with its argv, so no `ps` is needed to tell them), HEAD
  with commits that no branch, tag or remote branch holds (`unlanded_commits`, exit 4, `commits`, the message naming
  `bd -C <dir> lease pick`), an open resolve session of the slot (`resolve_in_progress`, exit 4: the next lease of
  the slot would find it; the message names `bd update --continue` and `--abort`, and `--confirm-preserved` does not
  pass it), and local changes (`dirty_worktree`, exit 4, `paths`). The commits of the commit
  the slot was leased at never count as unlanded, also after a rewrite of master made them unreachable.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)
- The question of the unlanded check is whether the branch of the source has every change of the slot. The get must
  record the source checkout and its branch (none for a detached HEAD). When reachability finds commits, each must be
  proven on the tip of that branch, on git objects only, without the lock and without the index of the shared
  checkout (`lease_pool::landed_work`, the one rule of the return, the takeover and the reclaim), and only the commits
  no proof finds are unlanded. The proofs, each for the commits the one before left: a commit of the range of a dry
  run of the pick (the fork point in the reflog of the branch, `git replay` onto its tip) that it would not pick; the
  patch id of a commit of the branch's side; a commit of the branch of the same identity (author, author time, subject
  without the prefix) that is its cherry-pick or changes the same lines; the twin of `bd update`, a commit of the
  branch of the same identity that has every changed line of the commit with its fixups (a conflict resolution, an
  autosquash), or every line it adds while the lines it removes and the twin does not are in no line of the file of the
  twin's parent (a removal of lines that another commit had rewritten or removed already: nothing is lost; a file there
  that is gone, binary or over 8 MiB proves nothing); a commit of the branch of the same author and subject at another
  author time (made anew, a rebase that reset the author; the three newest), whose change passes that twin check, the
  subject only picking it; the mapping note of a sync, its virtual copy on the branch or proven there so. A commit outside the
  range of the pick is no commit the pick proved: a fast-forward that a reset took back leaves the commit as its own
  fork point. Reachability decides alone when it finds nothing (the fast path), when the source had no branch and when
  the branch is gone; the output says which check decided (`landed_check`, `landed_branch`) and names each commit the
  proofs found landed with its proof (`landed`), and the refusal says which check refused.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_after_a_follow_and_a_rewrite_of_master_finds_the_work_landed_by_content`, `a_return_refuses_only_the_commit_that_the_source_branch_lacks`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_session_rewords_its_landed_commit_from_its_lease`, `a_pick_without_follow_finds_the_reworded_commit_present_by_patch_id`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_after_a_pick_that_rolled_back_refuses_the_commit`, `a_lease_whose_home_was_reset_past_its_commits_is_not_landed`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_finds_a_commit_that_landed_with_a_conflict_resolution`, `a_return_finds_a_commit_that_landed_by_its_virtual_copy`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_finds_a_commit_whose_twin_lacks_only_a_removal_master_made_before`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_finds_a_commit_made_anew_by_its_author_and_subject`)
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_dry_run_names_its_range_and_the_commits_it_would_pick`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_return_whose_commit_landed_by_the_ladder`)
- The return must find the Bazel servers of the slot (a process whose argv has `--workspace_directory=<dir>`, `<dir>`
  the slot or inside it, such as `<slot>/community`), ask each to exit with SIGTERM once nothing else refuses, wait up
  to 30 s, and name them in `stopped_servers` (pid, workspace, output base). A server that does not exit keeps the
  slot busy (`slot_busy`, the message saying it did not exit on SIGTERM, on `bazel shutdown` on Windows). No other
  process is ever signalled: any other process in the slot refuses as `slot_busy` before a server is signalled, and
  the message says that the return stops the servers itself. Bazel derives the output base from the workspace path,
  so a server serves only its slot, and the output base stays on disk: the next lease of the slot gets the same one,
  with its disk action cache.
  There is no flag to keep a server: with one, a return under a running server could never pass, the trap agents
  fell into.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_stops_the_bazel_servers_of_the_slot`, `a_return_signals_no_other_process_and_a_server_that_does_not_exit_keeps_the_slot_busy`)
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`the_argv_comes_from_the_procargs_buffer_without_the_environment`)
- Where no process scan runs (Windows), the return, the get of a free slot, the takeover and the reclaim must find
  the Bazel servers of the slot by their output base: a workspace is the slot or a directory right in it with
  `MODULE.bazel`, `WORKSPACE.bazel` or `WORKSPACE`; its `bazel-out` link (`<ws>/bazel-out`, or `<ws>/out/bazel-out`
  with `--symlink_prefix=out/bazel-`; a junction or a symlink, never a directory) names `<output
  base>/execroot/<name>/bazel-out`; the pid in `<output base>/server/server.pid.txt` runs and started before the file
  was written (a pid that is gone, or that the system gave to a later process, is no server). Each is asked to exit
  with `bazel --output_base=<output base> shutdown` in its workspace, without a console window and killed after the
  30 s, then waited for as SIGTERM is elsewhere: `stopped_servers` names the ones that exited (pid, workspace, output
  base), one that does not exit keeps the slot busy, and the pid itself is never signalled. A workspace whose link is
  missing or named otherwise shows no server, and its server keeps running. Other processes stay unchecked
  (`processes_checked: false`).
  [@test] ../../crates/lease-pool/src/bazel_servers_test.rs (`a_server_is_told_by_the_pid_file_of_the_output_base_its_bazel_out_link_leads_to`, `no_server_is_told_without_a_running_pid_of_the_pid_file`)
  [@test] ../../crates/lease-pool/src/bazel_servers_test.rs (`the_stop_of_a_server_told_by_its_output_base_names_it_stopped`, `a_server_that_does_not_exit_when_asked_is_not_stopped`)
- `bazel` must be found as `cmd` finds it: by the directories of the `PATH` and, on Windows, the extensions of
  `PATHEXT` (`bazel.exe`, a `bazel.cmd` of an npm Bazelisk, a `bazel.bat`), which `CreateProcess` alone does not try.
  A `bazel shutdown` that can't run (no `bazel` there, a program that does not start) or that fails (a non-zero exit,
  killed at the timeout) must not be waited for: its server, unless it exited anyway, keeps the slot busy as one that
  does not exit, and its process in the refusal (`slot_busy` of a return; the `busy` skip of a get or a reclaim) has
  `stop_error`, the error naming the program, which the message words too. Its pid is still never signalled.
  [@test] ../../crates/lease-pool/src/bazel_servers_test.rs (`a_bazel_shutdown_that_cannot_run_or_fails_says_why_and_names_the_program`, `a_server_whose_bazel_shutdown_cannot_run_or_fails_is_named_running_with_why`)
  [@test] ../../crates/lease-pool/src/bazel_servers_test.rs (`a_bare_program_is_found_by_the_path_and_the_extensions_of_pathext`, `the_stop_of_a_server_told_by_its_output_base_names_it_stopped`)
- git's background maintenance in the slot must keep neither a return nor a get from it, and the return names its
  pids (`maintenance`): a commit in a slot starts `git maintenance run --auto --detach`, whose repack keeps the slot as
  its working directory for minutes in a large repository (2 to 17 minutes in idea-1) and writes only the shared
  object store. While a `gc --auto` holds `HEAD.lock` of the slot (`pack-refs`, `reflog expire`), the detach of HEAD
  must be retried for up to 10 s; the lock is told by its file, never by the words of git's error.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`git_maintenance_keeps_no_slot_busy_and_is_named`)
- With `branchdeck.leaseMaintenance` true in the git config, the return must start `git maintenance run --auto
  --detach` in the common dir once the slot is free, before it empties the trash, and say so
  (`maintenance_started`); a failed start never fails the return. It is for a repository whose `maintenance.auto` is
  false, so that no commit or fetch in a slot starts a repack while a session works there (idea-1): the repack runs
  between leases, and only when its thresholds passed (`--auto`). The detached run keeps no slot as its working
  directory. Without the setting the return starts none.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_starts_the_maintenance_that_branchdeck_lease_maintenance_asks_for`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_return_that_starts_the_maintenance`)
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`git_maintenance_is_the_maintenance_and_the_gits_it_starts`)
- Once its checks passed, the return must reserve the slot before it changes anything: under the pool lock, with CAS on
  the lease id, the lease becomes `returning`, and a get must pass the slot over. A repeat must join, never fail: a
  second return of the lease meanwhile waits for the first and answers `joined: true` once it freed the slot (it
  returns the lease itself once the first failed or its process died); a return whose lease another return gave back
  meanwhile (a get may have leased the slot again) answers `joined: true` and must change nothing of the next lease,
  also when one of its checks refused the next holder's slot. A `joined` answer has empty lists, and no
  `landed_check` when the return checked nothing. A failed return gives the lease back as `ready`; a `returning` lease
  whose process is gone is abandoned.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_that_a_retry_overtook_answers_joined_and_leaves_the_next_lease_of_the_slot_alone`, `a_return_retried_while_the_first_acts_on_the_slot_waits_for_it_and_answers_joined`, `a_running_return_reserves_the_slot_against_a_get`)
- `--confirm-preserved` must give up the unlanded commits (`given_up`) and discard the local changes (`discarded`)
  with `reset --hard HEAD` and `clean -fd`: the ignored files stay.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot`)

### Pick

- `bd lease pick` inside a lease must be `bd pick --from <lease> --follow` run in the `home.checkout` of the lease,
  onto `home.branch`: the commits the lease has and the branch lacks replayed onto its tip as it is then (whatever
  other sessions landed meanwhile; no test runs again), and the lease moved onto the new tip, its answer that of
  `bd pick` (`state`, `picked`, `followed`, `undo_token`). With nothing to land the lease only follows
  (`state: up_to_date`, `followed.state: moved`), so it gets what the other sessions landed. --dry-run writes nothing
  but git objects.
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_lands_the_commits_on_the_home_branch_and_the_lease_follows`)
- The branch is checked under the repository lock with the rest of the pick's witness: when the home checkout has
  another branch checked out, nothing lands, `home_branch_switched` (exit 4) names both branches, and the agent asks
  the user. Inside the lease `bd workspace` shows the switch (`home.switched_to`). A lease cut from a detached HEAD has
  no branch to land on: `no_home_branch` (exit 4). Outside a leased slot: `no_lease` (exit 2).
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_home_that_switched_its_branch`)
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`the_get_names_the_home_of_the_lease_and_the_commands_of_the_session`, `a_lease_pick_outside_a_lease_is_refused`)
- A conflict must land nothing (exit 1, `state: conflict`) and come back to the lease: the lease is updated onto the
  tip of its home branch without a fetch (`update`, what `bd update` prints, [bd update](update.spec.md)), and the
  conflict there opens a resolve session in the lease; the lease moves only when `bd update --continue` finishes it,
  whose `next` is `bd -C <lease> lease pick` again, and `bd update --abort` closes it. `conflict.next` says so. An
  update that applies at once (the words resolved the conflict) is picked again at once. While the session is open a
  `bd lease pick` refuses (`resolve_in_progress`, exit 4). The output is `lease_pick` of `bd schema`.
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_pick_that_conflicts`)
- Its git processes are those of the pick (`a_pick`); the lease itself comes from the state of the pool, without git,
  and one `show-ref` asks whether its resolve session is open.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_pick`)
- A repository whose committed `.branchdeck.toml` sets `pick_requires_check = true` in `[lease]` (false by default)
  must land only a tree that a green build or test ran on. A check tool (`bt` of the IntelliJ monorepo) appends one
  JSON line per green run to `bd-checks.jsonl` in the git dir of the worktree (`git rev-parse --git-path`):
  `{"schema":1,"tree","head","tool","command","patterns","exit":0,"at"}`, with `host` for a run on another machine.
  The tree is `HEAD^{tree}` of a clean worktree, else the tree of the worktree with its local changes. bd only reads
  the file: a line of another schema, with an `exit` other than 0 or that does not parse counts as no record. The
  pick reads the setting from the home checkout, the file of the branch the commits land on, as the return reads its
  clean rules; a file that does not parse is `invalid_arguments` (exit 2). When no record names `HEAD^{tree}` of the
  lease, nothing lands: `unchecked_tree` (exit 4), whose message names the tree and HEAD, the last check (its tree,
  time and command, how many files changed since and the first 10 of them, left out when git no longer has that
  tree) and both ways on: build and test, then `bd -C <lease> lease pick` again, or `bd -C <lease> lease pick
  --unchecked '<reason>'`. The pick expects the lease's HEAD to be the commit it checked (`expect_source_head`, held
  under the lock by the recheck of the pick): a lease that moved between the check and the lock is `head_moved`
  (exit 3), and the pick runs again. The gate stops a stale claim, a check that ran before the last edit; it does not
  stop a merge break: a pick onto a tip that moved, without a conflict, lands a tree that no record saw. It does not
  judge coverage either: any green build or test of the tree counts, whatever it built.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_source_that_moved_from_the_expected_head_is_refused`)
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_tree_that_no_green_check_recorded`)
  [@test] ../../crates/lease-pool/src/check_record_test.rs (`only_green_records_of_schema_one_count`, `pick_requires_check_is_a_boolean_of_the_lease_table`)
- `--unchecked <reason>` must land the tree without a check: the reason is required (a blank one is
  `invalid_arguments`), the output repeats it (`unchecked`) and stderr names it. Without the setting the pick reads no
  record and prints no `check`.
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`unchecked_lands_without_a_check_and_says_why`)
- `--dry-run` must report the check of the tree in `check` (`state`: `ok`, a record names the tree; `stale`, records
  exist and none names it; `missing`, no record; `record`: the newest record of the tree, else the newest one, with
  `changed_files` and the first 10 of them in `changed_paths`), and never refuse for it, with or without the setting. With the setting, a pick reports `check` too.
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_tree_that_no_green_check_recorded`)
- With the setting, an update after a conflict that applies at once moves the lease onto a new tree: the pick again
  runs only when a record names that tree too, else the answer stays `state: conflict` (exit 1) with the `update`, the
  `check` of the new tree, and a `conflict.next` that says to build and test in the lease, then pick again.
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`an_update_that_applies_after_a_conflict_waits_for_a_check_of_the_new_tree`)
- The check costs one `rev-parse` (HEAD and its tree) and, for a stale tree, one `diff-tree`; without the setting and
  without --dry-run the pick runs neither.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`the_tree_check_of_a_lease_pick`, `a_lease_pick`)

### Clean rules

- The rules are the `clean` array of the `[lease]` table of `.branchdeck.toml` at the root of the checkout (beside
  `pick_requires_check` of [Pick](#pick)), committed
  with the build layout they clean (not git config, and not `.worktreeinclude`, which Treehouse and Claude Code read
  too). There are two kinds and no command, since a return must not run arbitrary code:
  `{ cargo = "<dir>", keep = 8, age = "1h" }` (the defaults) prunes the profile dirs under `<dir>` (a dir with
  `.cargo-lock`, at most 4 levels down, also of a cross target such as `aarch64-unknown-linux-musl/debug`): a unit
  stays when it is among the `keep` newest of its group or younger than `age` (`s`, `m`, `h`, `d`), and the files of a
  unit go together. Each profile dir is pruned in the layout it has:
  - the build-dir layout: a unit is `build/<package>/<hash>/`, built at the newest mtime of the dir and of the children
    of its `fingerprint` dir;
  - the classic layout: a unit is `<name>-<16 hex digits>`, its group `<name>` (`-` and `_` the same, without the
    `lib` of a library file), and its files are all of that name and hash in `deps/` (`lib<name>-<hash>.rlib` and
    `.rmeta`, `<name>-<hash>.d`, the object files, the test or bin executable and its `.dSYM`, `.pdb` or `.exe`),
    `.fingerprint/` and `build/` (a build script and its run, `out/`), built at the newest mtime of its files;
  - in both, `incremental/<crate>-<hash>/` is a unit of its own, grouped by crate.
  The uplifted artifacts at the top of the profile dir (`debug/libavl_vm.rlib`, `debug/<bin>`) and `.cargo-lock` are
  never touched. `{ delete = "<path>" }` removes a path, a file, a directory or a symlink (the link, never its
  target; a junction or a directory symlink of Windows too, such as Bazel's `out/bazel-*`, which a removal neither goes
  into nor takes for a file held by a process); `{ delete = "<dir>", except = ["<name>", …] }` removes every entry of
  the directory but the named ones (names of direct entries, no path), a whitelist: a new entry goes without an edit of
  the rules. A rule below a kept entry applies to it (`out` but for `dev-run`, then `out/dev-run` but for `jar-cache`).
  [@test] ../../crates/lease-pool/src/lease_clean_test.rs (`the_rules_are_cargo_and_delete_with_their_defaults`, `the_cargo_rule_keeps_the_newest_and_the_young_units`, `the_cargo_rule_prunes_the_classic_layout_by_units_of_name_and_hash`, `a_classic_unit_is_its_name_and_its_hash`)
  [@test] ../../crates/lease-pool/src/lease_clean_test.rs (`a_delete_keeps_its_kept_entries_removes_a_link_and_goes_through_the_trash`, `nested_deletes_keep_only_the_whitelist`)
  [@test] ../../crates/sync-utils/src/fs_retry_test.rs (`remove_entry_removes_a_directory_link_as_the_link_and_keeps_its_target`, `remove_entry_removes_a_file_an_empty_directory_and_a_file_symlink_as_themselves`)
- A `delete` must remove a link (a symlink, a junction or a directory symlink of Windows) in place, as the link and
  never its target, also in a return and `bd lease clean`, which move every other path into the trash: a link is no
  cheaper to move than to delete, and a junction moved there stayed ("Access is denied"). It goes only once every
  other path removed with it is moved, so a failed move never leaves a link lost, and its own bytes count in
  `freed_bytes` at once. The trash holds no links; one an earlier version left there goes as the link with the trash
  of its run.
  [@test] ../../crates/lease-pool/src/lease_clean_test.rs (`a_delete_through_the_trash_deletes_bazel_links_in_place_and_keeps_their_targets`, `the_sweep_deletes_a_link_left_in_the_trash_of_a_gone_run`, `a_delete_keeps_its_kept_entries_removes_a_link_and_goes_through_the_trash`)
- An unknown key, an unknown kind, a wrong type or a file that does not parse must be an error of the file.
  [@test] ../../crates/lease-pool/src/lease_clean_test.rs (`unknown_keys_and_kinds_are_errors`)
- The table `[host]` belongs to the remote runs of `bd-host` (spec/cli/host.spec.md), which checks its keys. The rules
  and `pick_requires_check` must read past it, whatever it holds. A bd before this rule refuses a file with `[host]`
  and then applies no clean rule.
  [@test] ../../crates/lease-pool/src/lease_clean_test.rs (`the_host_table_of_the_remote_runs_stays_unread`)
- A rule must be skipped and reported (`error`), never applied, when its path is absolute, leaves the checkout with
  `..`, names `.git`, names the checkout itself, has a symlink on its way (a `cargo` dir that is one too), holds a file
  that git tracks (one `ls-files -z` of the paths of all rules), or has an `except` entry that is no name of an entry;
  and a `delete` whose path holds the dir of a `cargo` rule without naming the entry on its way in `except`, which
  would remove what the cargo rule keeps. The walk of a rule never follows a symlink.
  [@test] ../../crates/lease-pool/src/lease_clean_test.rs (`an_unsafe_path_is_a_rule_that_is_reported`, `a_delete_names_its_kept_entries_and_keeps_the_dirs_of_cargo_rules`)
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`an_unsafe_rule_is_skipped_and_reported`)
- The return must read the file once, from the source checkout that the lease recorded (the slot's own when the
  source is gone), and apply the rules after all its checks passed, before it frees the slot: `cleaned` [{`rule`,
  `freed_bytes`, `ms`, `error`?}]. A file that can't be read or parsed (`clean_error`) and a failed rule never fail
  the return.
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`the_return_applies_the_clean_rules_of_the_source`, `a_bad_file_does_not_fail_the_return`)
- What a rule removes must go into the trash first, a rename into `$GIT_COMMON_DIR/branch-deck/trash/<pid>-<start
  time>-<uuid>/` (in place on another volume): the paths of a `delete` but a link, and a stale unit of a `cargo` rule
  as a whole, so a return frees the slot before the long delete of a build output and a timeout of the caller never
  leaves a lease `returning` while files go. What a process keeps must stay whole in the checkout, never be deleted in
  place: on Windows a rename fails while a process works in the path or has a file of it open; after a retry of about a second
  (`sync_utils::fs_retry`, which every removal and rename of the app goes through; a read-only attribute, which fails a
  call with the same "Access is denied", is cleared instead, as git clears it) the files of the unit moved before it go
  back, and the `error` of the rule names the path and says that a process holds it (Windows has no process scan
  to name it): once the process let go, the next `bd lease clean` deletes it. The other paths of the rule go on. Once
  the slot is free the return, `bd lease clean` once every slot it marked is free again, and `bd lease reclaim` once
  every slot it reclaimed is free, delete their trash, counting `freed_bytes` in the same walk, and the trash of a run
  whose process is gone; a path that can't be deleted stays and is named in `trash_left`. `freed_bytes` counts the blocks an APFS clone shares with another checkout too;
  `bd lease clean` also prints `volume_freed_bytes`, what the volume gained (`statfs` before and after).
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`a_return_deletes_through_the_trash_and_what_a_gone_run_left_there`)
  [@test] ../../crates/lease-pool/src/lease_clean_test.rs (`a_delete_keeps_its_kept_entries_removes_a_link_and_goes_through_the_trash`)
  [@test] ../../crates/lease-pool/src/lease_clean_test.rs (`a_held_unit_and_a_held_entry_stay_whole_and_the_next_clean_takes_them`)
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`a_held_entry_of_a_free_slot_stays_whole_until_its_process_exits`)
  [@test] ../../crates/sync-utils/src/fs_retry_test.rs (`a_process_whose_current_directory_is_in_the_directory_delays_its_removal_until_it_exits`, `a_program_that_keeps_a_file_open_fails_the_removal_on_windows_after_a_second`, `a_read_only_file_and_directory_are_renamed_and_removed_at_once`)
  [@test] ../../crates/git-executor/src/fs_retry_scan_test.rs (`every_removal_and_rename_waits_out_a_holder_on_windows`)
- `bd lease clean [--free] [--dry-run] [--keep <n>] [--age <duration>] [<dir>...]` must apply the rules of the checkout
  it runs in, read once, to the free slots (`--free`), to the given worktrees, or to that checkout. A slot of the pool
  with a lease or with a process in it is skipped (`skipped`); a checkout that is no slot is cleaned as asked. A free
  slot it cleans must carry a `cleaning` marker under the pool lock until the clean of it ends, so no get leases it.
  `--dry-run` deletes nothing and says what the rules would free. A file that does not parse is `invalid_arguments`
  (exit 2).
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`clean_skips_a_busy_and_a_leased_slot_and_a_dry_run_deletes_nothing`, `a_bad_file_does_not_fail_the_return`)
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`a_slot_that_clean_cleans_is_reserved_against_a_get`)
- The rules are configured on purpose, so they apply by default: the return and `bd lease clean` delete what they
  name, as the user decided. `bd lease return --no-clean` applies none. `--keep <n>` and `--age <duration>`, on both
  commands, set `keep` and `age` of every cargo rule for that call without an edit of the file; a bad `--age` is
  `invalid_arguments`. These are the only overrides: `except` of a `delete` rule is the file's.
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`no_clean_and_the_overrides_change_what_a_call_cleans`)

### Status

- `bd lease status` must list every slot from the state alone, with its `path`, `state` (`free`, `preparing`,
  `ready`, `templating`, `returning`, `cleaning`, `abandoned`), `dirty`, `missing`, `lease` (with `holder_pid`,
  `holder_alive`, `idle` for a ready lease whose holder is gone: `idle`, or `idle_with_work` with what a check found,
  and `taking_over` while a get checks one) and `reclaimed`, the `free_bytes` of the volume of the pool and the `trash_bytes` a killed run left; it must run no
  `git status`. `--processes` must add the processes in each slot. It writes nothing.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_status_reads_the_state_without_a_git_status`, `the_status_tells_whether_the_holder_of_a_lease_is_alive`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

### Reclaim

- `bd lease reclaim --idle [--dry-run] [--keep <n>] [--age <duration>]` must apply the rule of the takeover to every
  idle lease of the pool, each checked anew: one whose work landed is returned as the takeover returns it, under a CAS
  on its lease, its holder still gone and its slot as the check found it (`reclaimed`, each with `landed`,
  `stopped_servers`, `cleaned`), its slot free with its record, and the trash of every reclaimed lease deleted once the
  last one is free (`trash_left`), so none waits for the delete of another's build output; one with work is kept and
  its check recorded (`kept`, as `idle_with_work`); one not quiet for an hour, busy, changed meanwhile or whose check
  failed is `skipped` with its reason. `--dry-run` checks the same and writes nothing. The output is `lease_reclaim` of
  `bd schema`, exit 0.
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`reclaim_deletes_the_trash_once_every_reclaimed_slot_is_free`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_reclaim`)

### Reclaimed

- A command of bd in a slot whose lease a get or a reclaim took over must be refused as `lease_reclaimed` (exit 4),
  nothing changed, when another session than the one that holds it now runs it: the message names the old lease and
  holder, the new one, the home branch and the old branch, which stays, and `next_steps` names `bd lease get --holder
  <old holder> -C <home>`. The session that holds it is told by its process: refused unless the holder (pid and start
  time) is an ancestor of bd. A read passes, but `bd workspace`, the first command of a session; `bd lease status`
  never refuses, and `bd lease return --lease-id <current lease>` passes, for a coordinator. A free slot with the record
  refuses every write, and a lease that a get is taking over every command. Checked from the state, no git, with a
  process scan only in such a slot.
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`a_command_of_another_session_in_a_taken_over_slot_is_refused_as_lease_reclaimed`)
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`an_ancestor_is_a_process_above_this_one_with_its_start_time`)

### Adopt

- `bd lease adopt <dir>...` must add each clean linked worktree of this repository as a free slot, where it is
  (`adopted`), and list the refused ones with their code (`refused`, exit 4): not a linked worktree of this repository
  (`not_a_slot`), held by the pool already (`slot_in_pool`), a process in it (`slot_busy`), local changes
  (`dirty_worktree`).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`adopt_adds_clean_linked_worktrees_and_refuses_the_rest`)
- `bd lease adopt --move <dir>...` must, after the same checks, move each worktree to the next slot path
  `<pool>/<n>/<repository name>` with `git worktree move` (never a plain rename), report `moved_from`, and remove the
  old parent directory when it is left empty (`<treehouse pool>/<n>/`). A worktree on another volume than the pool
  directory (`st_dev`) must be refused as `other_volume` (exit 4): a move there copies every file. The state file of
  Treehouse is not touched; Treehouse sees the old path as missing. Bazel derives the output base from the workspace
  path, so a moved slot starts with a new, cold output base, and the old one stays on disk until the user deletes it.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`adopt_with_move_moves_the_worktree_into_the_pool`)

### Host slots

The slots that the agent of `bd-host` keeps for the runs of other machines (spec/cli/host.spec.md): it calls the
functions of `lease_pool::host_slots` in process on the host. A remote slot holds no state of its own, since the commit
comes from the client and the outputs go back to it, so a slot is lent to one run for the duration of that run only.

- The functions of the table are `lend` (a run, the pid of its process with its start, an affinity: `Lent` with the
  slot, its lease id, `cleaned_at` and `generation`, or `Queued` with `ahead`, the lent runs and the slots in
  quarantine; a repeat answers the same slot), `lend_waiting` (below), `checkout` (below), `give_back_run` (the slot
  of the run, else the run leaves the queue), `fill`, `reclaim`, `shrink`, `set_limit`, and `read_host_slots` (each
  slot, its state, the queue, the limit; `describe_state` and `is_abandoned` say a state in words). A pid whose start a
  sandbox hides counts as alive while it exists. The `generation` of a slot grows by one with each give-back that
  discards changes and each clean, before their removal of files, so also before a retry after a crash. So an agent
  that keys what its prepare steps made in the slot by it never reuses a result that a give-back or a clean removed. A
  refusal of the table is a `HostSlotRefusal`: `UnknownSlot`, `NoSlot` or `NotLent` (no such slot, or not this run's),
  `Busy` (another process acts on it) or `InvalidConfig` (a bad `branchdeck.hostSlots`).
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_run_gets_the_slot_of_its_affinity_else_the_one_used_longest_ago_else_waits_in_line`)
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`an_abandoned_lend_is_given_back_by_reclaim_before_another_run_gets_the_slot`)
- `lend_waiting` must wait while the run is queued, as long as its process lives, and answer as soon as it gets a
  slot, without a second call. It watches `branch-deck/` of the common dir before its first look, so no give-back
  between the look and the wait is lost, wakes only on a change of `host-slots.json` or `leases.json`, and looks again
  at least every 5 s, as the end of a process makes no file event. It gives its caller the queue once a minute
  (`host_runs::queue_line`: `run <id> waits <s> s: <n> runs ahead, …`). Once the time is up it answers `Queued` with
  its place; the ticket stays while the process lives. A process that ends while it waits is `ProcessGone`, and its
  ticket leaves the queue.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_lend_that_waits_gets_the_slot_once_it_is_given_back`)
- With `branchdeck.leaseMaintenance` true, a give-back must start `git maintenance run --auto --detach` in the common
  dir, as a return does, also for a run without a slot, and say so (`maintenance_started`); a failed start never fails
  the give-back. Without the setting it starts none.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`the_host_slots_lend_give_back_and_shrink`)

- A fill must lease the missing host slots up to `branchdeck.hostSlots` (git config of the host, 4 by default) as plain
  `ready` leases of the holders `bd-host/1`, `bd-host/2`, … at the HEAD of the host checkout, detached, through the get
  of the pool (a free slot reused, one clone at a time). `leases.json` keeps its format, so another bd and the app read
  the pool as before. A plain `bd lease get` never takes a host slot, `bd lease clean --free` skips it, the hint at idle
  leases does not name it, and a get with a holder `bd-host/…` is refused as `invalid_arguments`.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_fill_leases_the_host_slots_as_ready_leases_that_the_other_commands_leave_alone`)
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_host_slot_lease_keeps_the_format_of_leases_json`)
- The lend table is `$GIT_COMMON_DIR/branch-deck/host-slots.json`, written only under `host-slots.lock`, a leaf lock:
  held while the table changes, never across git, a clone, a process scan or another lock of the pool. Every state of
  a slot but `idle` and `quarantined` names the process that acts on it (`lent`, `filling`, `giving_back`, `cleaning`,
  `shrinking`); once that process is gone, the state is abandoned.
- A lend must give the run the idle slot that its affinity (`checkout-<12 hex>` of the client's checkout) used last,
  else the idle slot used longest ago, else put the run in a first-come queue; a run gets a slot when fewer runs wait
  before it than slots are idle. A repeat of a lend gets the same slot. A ticket of a gone process leaves the queue.
  An abandoned lend is not lent again before `reclaim` gave it back. Each waiting run must be a process of its own
  (the supervisor of the run), since its ticket lasts as long as its process.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_run_gets_the_slot_of_its_affinity_else_the_one_used_longest_ago_else_waits_in_line`)
- A give-back must stop the processes in the slot that started after the lend (SIGTERM, up to 10 s, then SIGKILL, each
  only while its pid has the start time of the scan), leave its Bazel servers, and never signal a process older than
  the lend. A process left after that quarantines the slot: no run gets it, the queue answer names the process, and
  `reclaim` gives the slot back once the process is gone. Else the changes are discarded (`reset --hard`, `clean -fd`,
  only when `status` shows any; the ignored files stay as the warm cache). Without a process scan (Windows) it goes on
  and answers `processes_checked: false`.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`the_give_back_stops_the_processes_of_the_run_and_quarantines_the_slot_for_an_older_one`)
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`an_abandoned_lend_is_given_back_by_reclaim_before_another_run_gets_the_slot`)
- The `status` of a give-back must use the repository's own `core.fsmonitor` and `core.untrackedCache`. git's fsmonitor
  daemon of the slot must outlive the give-back and never quarantine the slot, also when it started after the lend
  with its working directory in the slot. With it, the `status` of the next give-back must still find every change of
  its run.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`the_fsmonitor_daemon_of_a_host_slot_outlives_the_give_back_and_the_changes_of_a_run_are_still_found`)
- After a give-back, the clean rules of the slot's own `.branchdeck.toml` must run when they are due: with
  `branchdeck.hostSlotMaxSize`, when what they would free passes it, or when the volume has less free space than
  `branchdeck.leaseMinFree` plus it; without it, after 50 runs or 7 days since the last clean, or when the volume has
  less than `branchdeck.leaseMinFree`. `bd lease clean --host` cleans every idle host slot now (`--dry-run` only
  measures) and skips a slot that is not idle or has a process in it, with the reason; it conflicts with `--free` and
  `DIR`.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_host_slot_is_cleaned_by_size_by_free_space_and_after_fifty_runs`)
- A shrink to a count must give the host slots past it back to the pool and keep the count as the limit of later
  fills until it is lifted. It takes a slot that is idle, in quarantine or abandoned, also without a running
  agent, and refuses a slot that a live process has. It signals no process (a process in the slot refuses
  it), discards the changes, checks the slot out at the commit it was leased at, so the snapshot commits of the runs
  count as landed, and returns it through the return of the pool without `--confirm-preserved`.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_shrink_returns_the_host_slots_past_the_count_also_without_the_agent`)
- `reclaim` must take up what no process acts on: drop the record of a fill that died before its get, give back an
  abandoned lend or give-back, run an abandoned clean or shrink again, and return a `bd-host/<n>` lease that the table
  does not name and whose number no live fill holds.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`reclaim_drops_a_dead_fill_and_returns_a_lease_the_table_does_not_name`)
- A hand `bd lease return` of a host slot (its holder is `bd-host/<n>`, or the table names its lease id) must be refused
  as `host_slot` (exit 4, `paths`) before any check or git; the refusal names `bd-host slots --local --shrink`.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_hand_return_of_a_host_slot_is_refused`)
- `checkout` of a run and a commit must move the slot lent to that run to the commit, detached, by the diff, as the
  get of a reused slot does but without a `status`: a give-back, a fill and a clean leave the slot clean (its index,
  HEAD and files agree, the ignored files aside). It answers `moved`: `Kept`, `Switched`, or `Forced` with git's
  reason. Every file is checked out only after git refused the diff (exit 128, never after a signal), and only while
  the table still lends the slot to the same run with the same lend: a call of a supervisor that died may still run,
  and the slot may be another run's by then. A run without a slot is `NoSlot`, another run's slot `NotLent`.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_run_checks_out_its_commit_in_its_slot_by_the_diff`)
- A lend runs no git, a checkout the config, HEAD and one `reset`, a give-back one `status` (plus `reset` and `clean`
  for a dirty slot, and the `maintenance` of `branchdeck.leaseMaintenance`), a shrink one `status`, HEAD, a `reset` when the slot is not at the commit it was leased at, and
  the processes of a return.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`the_host_slots_lend_give_back_and_shrink`)
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`a_kill_signals_only_the_process_that_has_the_start_time`)

### Host objects

The git of the runs on the host goes through these functions too (`lease_pool::host_refs`, `git_ops::host_objects`),
so the agent starts no git of its own: which commits the host has, the packs of the clients, the refs that keep their
commits, and what a commit holds.

- `have` must answer the full ids of the commits that the repository has as commit objects, in their order, from one
  `cat-file --batch-check`.
  [@test] ../../crates/bd-lease/src/host_objects_test.rs (`a_pack_lands_under_the_ref_of_its_run_until_the_give_back_of_the_run`)
  [@test] ../../crates/git-ops/src/host_objects_test.rs (`the_commits_a_host_has_are_its_commit_objects_in_order`)
- `ingest` of a ref (`HostRef::parse`: `run:<id>` or `base:<name>`) and a commit must write the pack of its input into
  the object store and point `refs/branch-deck/host/run/<id>` or `refs/branch-deck/host/base/<name>` at the
  commit, in one ref transaction. It reads the 12-byte header across short reads; a pack of fewer objects than
  `receive.unpackLimit`, else `transfer.unpackLimit`, else 100, goes to `unpack-objects`, which resolves the bases of
  a thin pack from the object store, as `receive-pack` does, and a larger one to `index-pack --stdin --fix-thin`. It
  answers the full ref, `objects` and `stored` (`Loose` or `Pack`). The pack streams through, never whole in memory. A
  stream that is no pack, and a pack git refuses, fail with git's words after the rest of the stream was read, so the
  sender is never blocked. A name of 1 to 128 characters of `A-Z a-z 0-9 . _ -`, the first one a letter or a digit,
  without `..` and not ending in `.lock`, and a full commit id are required (`HostRefRefusal`).
  [@test] ../../crates/git-ops/src/host_objects_test.rs (`a_small_pack_lands_as_loose_objects_and_a_large_one_as_a_pack`, `a_stream_that_is_no_pack_is_refused_and_read_to_its_end`)
  [@test] ../../crates/git-ops/src/host_objects_test.rs (`the_unpack_limit_is_the_one_of_receive_else_transfer_else_100`)
  [@test] ../../crates/sync-utils/src/host_ref_name_test.rs (`a_host_ref_name_is_one_safe_component`)
- When a pack last moved each ref of the host must be kept in `$GIT_COMMON_DIR/branch-deck/host-refs.json` (its lock
  a leaf, as the one of the lend table). The give-back of a run must delete the run ref of its pack (`deleted_ref`),
  also for a run that had no slot; a run that sent no pack costs no git. `reclaim` must delete a run ref whose run has
  no lend and no ticket of a live process an hour after its pack, and a base ref that no pack moved for 14 days
  (`deleted_refs`). With `branchdeck.leaseMaintenance` true, an ingest must start the maintenance after the ref
  (`maintenance_started`).
  [@test] ../../crates/bd-lease/src/host_objects_test.rs (`a_pack_lands_under_the_ref_of_its_run_until_the_give_back_of_the_run`, `reclaim_deletes_the_refs_that_no_run_needs`)
  [@test] ../../crates/git-ops/src/host_objects_test.rs (`the_refs_of_the_host_move_and_go_in_one_transaction`)
- `show` of a commit, a blob path and id paths must answer the text of the file of the blob path (its path, id and
  text; none when the commit has no file there) and the object id of each id path (a blob, the tree of a directory, the
  commit of a gitlink; none for no object), from one `cat-file --batch`. A commit that the repository lacks, a path with
  an empty, `.` or `..` component and a file over 1 MiB are refused (`ShowRefusal`).
  [@test] ../../crates/git-ops/src/host_objects_test.rs (`show_answers_the_text_of_a_file_and_the_ids_of_paths`)
- `find_git` of `run_path` must find the git that the `PATH` of the runs finds first, the `bin` of Homebrew before
  `/usr/bin`, with its version. The floor of a host is `HOST_MIN_VERSION` (2.56.0, above the 2.54 of bd itself, a
  constant of the code). A version parses as git prints it (`2.50.1 (Apple Git-155)`, `2.45.1.windows.1`), and a
  release candidate comes before its release.
  [@test] ../../crates/lease-pool/src/host_git_test.rs (`a_version_parses_releases_candidates_and_the_apple_git`, `the_bin_of_homebrew_comes_before_usr_bin`)
- `ensure_host_git` must bring that git to the floor: nothing at the floor; below it `brew install git`
  when the `bin` of Homebrew has no git, else `brew upgrade git` (in `/`, `HOMEBREW_AUTO_UPDATE_SECS=0`,
  `HOMEBREW_NO_INSTALL_CLEANUP=1`, at most 30 min), and `upgraded: true`. While another Homebrew process holds the
  lock of git (`has already locked`, `already in progress`), it tries again every 10 s and takes a git that meets the
  floor meanwhile as the end of that process. A host without Homebrew, a git before the one of Homebrew in the
  `PATH`, and an upgrade that fails or leaves the git below the floor refuse as `OldGit` with the hint: the install of
  Homebrew on macOS, a newer source of packages elsewhere. Its waits and Homebrew go to the progress of its caller.
  `BD_TEST_HOST_BREW` names a fake Homebrew (`none`: none) and `BD_TEST_HOST_MIN_VERSION` another floor, for a test.
  [@test] ../../crates/bd-lease/src/host_git_test.rs (`ensure_installs_or_upgrades_git_with_homebrew_below_the_floor`)
  [@test] ../../crates/lease-pool/src/host_git_test.rs (`the_plan_installs_upgrades_or_refuses_with_a_hint`, `a_lock_of_another_homebrew_process_is_told_by_its_words`)
- No run may use git while Homebrew replaces it, and the lend table is the lock: the ensure records the upgrade in
  the table first (`git`: `upgrading`, its process and command), then waits until no slot is lent, filled, cleaned,
  shrunk or given back by a live process and no host-object call of a live process runs (`users`, below), then runs
  Homebrew. The wait has no limit: Homebrew never starts under a live user of git, and a gone one does not count.
  Once a minute it says the busy slots, the host-object calls with their processes and the seconds it waited (`… waits
  for the users of git (<s> s so far): 1 is lent to …, 2 host-object calls of the processes …`). An upgrade that
  fails or panics before the end of Homebrew removes its record. While a live process upgrades, a lend only queues and
  answers `git_upgrading` with the command, and the queue line of a waiting lend says that git is upgrading. A refusal
  stays in the table (`git`: `refused`): every lend refuses with it as `OldGit` until a later ensure finds the floor.
  [@test] ../../crates/bd-lease/src/host_git_test.rs (`the_upgrade_waits_for_the_lent_slots_and_the_lends_queue_while_it_runs`, `without_homebrew_ensure_and_the_lends_refuse_until_git_meets_the_floor`)
  [@test] ../../crates/bd-lease/src/host_git_test.rs (`the_upgrade_waits_for_a_live_host_object_call_and_not_for_a_gone_one`)
- Each host-object call (`have`, `ingest`, `show`, and `snapshot` and `changes` of Host runs) must be a git user of
  the table while it runs: it adds its process to `users` under the lock and removes it at its end, also on an error
  or a panic. It adds itself only when no live process records an upgrade, or while a slot is busy, since Homebrew
  can't start then: the scans and the `show` of a run come while its slot is lent, and the run and the upgrade would
  else wait for each other. Else it waits, without a limit, until the upgrade ends or its process is gone: a change of
  the table wakes it, and it looks again every 5 s. The library functions do this, so each caller gets it.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_host_object_call_waits_for_an_upgrade_of_git_but_a_scan_of_a_lent_run_goes_on`)
- While a live process upgrades git, a step that would take a slot or run git for the keeper must skip, with the
  refusal "The git of the host is upgrading" with the command (`GitUpgrading`): a fill adds no slot and runs no git, a clean and the clean after a give-back skip, a shrink sets its limit and refuses each slot
  past it, and each step of `reclaim` but the queue and the dead fills has the refusal as its `error`, so a later
  `reclaim` takes it up. `bd lease clean --host` skips every host slot with that reason, before any git. The give-back
  of a lent run still runs, as Homebrew waits for it, but its run ref stays for `reclaim`, and `reclaim` deletes no
  ref meanwhile.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`a_fill_a_clean_a_shrink_and_reclaim_skip_while_git_is_upgrading`)
- The ensure must record its upgrade, its refusal or the ready git under the lock of the table, and only while no live
  process records an upgrade, also another call of its own process. Else it leaves that record as it is, says once a
  minute that another upgrade runs and its command, waits for its end and plans the git again. So two ensures never
  both run Homebrew.
  [@test] ../../crates/bd-lease/src/host_git_test.rs (`a_second_ensure_never_records_over_the_upgrade_of_a_live_process`)
- `have` and `show` run one `cat-file` each, `ingest` of a small pack the config, `unpack-objects` and one
  `update-ref`, and the give-back of a run that sent a pack one more `update-ref`.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`the_host_objects_have_ingest_and_show`)

### Host runs

The files of a run: on the host the scans of the slot before and after its command, on the client (`bd-host run`,
`git_ops::host_client`) the commit the run ships, its pack and the apply of its changes. A commit of the files of a worktree
(`worktree_commit`) is what `git add --all` stages, on HEAD, without a write of the index, the files or the refs of the
worktree: one `status --porcelain=v1 -z --no-renames` of its index, read only, in the untracked mode of the repository
(its `core.fsmonitor` and `core.untrackedCache` apply); for the paths it names, a file by `hash-object -w
--stdin-paths` (its clean filter and end of line apply) with the mode of its executable bit, a link by its target, a
directory with `.git` as the gitlink of its HEAD, a missing file as a deletion, an untracked directory by its files
(`ls-files --others --exclude-standard`); then one `fast-import` of the commit from the tree of HEAD, its objects
loose also for a wide change (no pack file per run), and one ref transaction that deletes its temporary ref. A repository with `core.fileMode` or `core.symlinks` false, a path with a
newline and a nested repository without a commit take a copy of the index instead (in a private directory of the git
dir, removed afterwards): one add of the paths, or of every path above 256 paths or when a path went away meanwhile,
and one `write-tree`. Both give the same commit for the same tree.

- `snapshot_run` of a run must answer the tree of the files of the slot lent to the run as `git add --all` stages them
  (`tree`: the tracked and the untracked files that git does not ignore, a deletion, a staged deletion and rename, a
  mode, a link, a gitlink, a filtered file), with `n`, `path`, `head` and `changed` (the count of the status entries);
  the slot index stays byte for byte. A run without a slot is `NoSlot`.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`the_scans_of_a_slot_name_its_tree_and_the_patch_of_its_command`)
  [@test] ../../crates/git-ops/src/worktree_snapshot_test.rs (`the_commit_of_a_worktree_is_what_add_all_stages_from_its_files_or_a_copy_of_its_index`, `a_clean_worktree_is_head_and_the_copy_adds_many_paths_at_once`)
  [@test] ../../crates/git-ops/src/worktree_snapshot_test.rs (`a_wide_change_leaves_loose_objects_and_no_pack`)
- `changes_run` of a run, a base tree and a patch file must answer the tree as `snapshot_run` does and, when it differs
  from the base, stream `diff-tree -r -p --binary --full-index --no-renames <base> <tree>` into a new file beside the
  patch file, then rename it onto the patch file, so a link there is replaced, never followed (`patch_bytes`). A tree
  equal to the base writes no file.
  [@test] ../../crates/bd-lease/src/host_slots_test.rs (`the_scans_of_a_slot_name_its_tree_and_the_patch_of_its_command`)
  [@test] ../../crates/git-ops/src/worktree_snapshot_test.rs (`the_patch_between_two_trees_applies_and_replaces_a_link_at_its_path`)
- A scan runs the config, HEAD and its tree (one `log`) and one `status`; when something changed, one `hash-object`,
  one `fast-import` and one `update-ref`.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`the_host_slots_lend_give_back_and_shrink`)
- `git_ops::host_client::snapshot` must answer `commit` (HEAD; with `Scan::WorkingTree` the commit of the working
  tree: HEAD when the tree is that of HEAD, else a commit on HEAD by `bd-host <bd-host@bd-host.invalid>`, dated as the
  committer of HEAD, so the same tree on the same HEAD is the same commit), `head`, `changed` (the status entries; none
  for `Scan::Head`, which reads no status) and `candidates`: HEAD and its first-parent ancestors at the distances 1, 2,
  4 … up to the farthest distance (`FARTHEST`, 65536), then the merge bases of HEAD with `origin/master` and `master`,
  each once, and the `base` (the tip of `origin/master`, else of `master`, absent without both). `repository_name`
  answers the directory of the main worktree without `.git`. The index, the files and the refs stay.
  [@test] ../../crates/git-ops/src/host_client_test.rs (`the_commit_of_the_working_tree_is_head_or_the_same_commit_on_head_for_the_same_tree`, `the_candidates_are_the_powers_of_two_and_the_merge_bases`)
  [@test] ../../crates/git-ops/src/host_client_test.rs (`the_name_of_a_repository_is_the_directory_of_its_main_worktree`)
  [@test] ../../crates/git-ops/src/host_client_test.rs (`a_snapshot_of_bd_host_reads_what_its_scan_names_and_ships_its_own_commit`)
- `pack_commit_until` must stream `pack-objects --revs --thin --stdout --progress` into a new file of mode 0600 in the
  directory of its caller (mode 0700; `<git_dir>/branch-deck/host/packs/` for `bd-host run`), never a name of the
  shared temporary directory, and answer its `path` and `bytes`; `commit_count` answers the commits (`rev-list
  --count`). Each progress line of git goes to the caller, and a minute without a line kills git. A stop of the caller
  kills git too, fails as cancelled and leaves no file. The caller removes the file; a pack file there older than a day
  goes at the next pack. Every id must be full. The host takes the pack with `lease_pool::host_refs::ingest`.
  [@test] ../../crates/git-ops/src/host_client_test.rs (`the_thin_pack_is_a_private_file_that_the_host_takes`, `a_cancelled_pack_fails_and_leaves_no_file`)
- `apply_patch` must apply the patch of `changes_run` to the files of the worktree, not its index, all or nothing (`apply
  --whitespace=nowarn`), and answer the count of the files (`apply --numstat`). A patch that does not apply fails and
  changes nothing. It is the one write of the user's working tree beside the guarded update: what the user asked a run
  for (`bd-host run --apply`).
  [@test] ../../crates/git-ops/src/host_client_test.rs (`the_patch_of_a_run_applies_to_the_files_all_or_nothing`)
  [@test] ../../crates/git-executor/src/worktree_safety_test.rs (`production_code_never_writes_the_working_tree`)
- The snapshot of a clean checkout runs HEAD (`log`), one `rev-list`, one `for-each-ref` and a `merge-base` per base
  ref; `Scan::Status` adds the config and one `status`, and `Scan::WorkingTree` with changes the writes of a scan. The
  pack runs one `pack-objects`, and the count one `rev-list`.
  [@test] ../../crates/git-ops/src/host_client_test.rs (`a_snapshot_of_bd_host_reads_what_its_scan_names_and_ships_its_own_commit`, `the_thin_pack_is_a_private_file_that_the_host_takes`)

### Error codes

| Exit | Codes |
|---|---|
| 2 | `invalid_arguments`, `no_lease`, `lease_mismatch`, `not_a_slot`, `slot_in_pool` |
| 3 | `locked` (the pool lock, the lock of the `worktree` commands, a clones lock no record names a live holder of; retryable) |
| 4 | `slot_busy`, `host_slot`, `unlanded_commits`, `dirty_worktree`, `branch_exists`, `other_volume`, `low_disk_space`, `no_baseline` (a source without commits), `resolve_in_progress`, `lease_reclaimed`; `bd lease pick`: `home_branch_switched`, `no_home_branch`, `unchecked_tree` and the refusals of `bd pick` |

- The outputs must match `bd schema` (`lease_get`, `lease_pick`, `lease_return`, `lease_status`, `lease_adopt`, `lease_clean`, `lease_reclaim`).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`lease_outputs_match_the_schema`)
  [@test] ../../crates/bd-lease/src/takeover_test.rs (`a_get_takes_over_an_idle_lease_whose_commits_landed`, `reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)
  [@test] ../../crates/bd-lease/src/clones_lock_test.rs (`a_get_that_must_add_a_slot_waits_for_the_one_that_adds_a_slot`)
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`clean_skips_a_busy_and_a_leased_slot_and_a_dry_run_deletes_nothing`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)
- The slot writes (`checkout` twice, `reset`, `clean` twice) must stay in `crates/git-ops/src/lease_slot.rs`, with their
  exact counts in the worktree safety scan.
  [@test] ../../crates/git-executor/src/worktree_safety_test.rs (`production_code_never_writes_the_working_tree`)

## Testing / Local Run

- `cargo nextest run -p bd-lease lease_test` (and `takeover_test`, the idle leases) runs the commands end to end on `TestRepo` fixtures, the pool in a
  temporary home directory, `cargo nextest run -p bd-lease clones_lock_test` the gets beside a get that adds a slot
  (one held at its registration by a wrapped git, one holder a child process that the test kills),
  `cargo nextest run -p bd-lease host_slots_test` the host slots;
  `cargo nextest run -p lease-pool lease_test` runs the seed patterns and `cargo nextest run -p sync-utils
  process_scan_test` the process scan.

## Open Questions / Risks

- A git outside the pool that reads every worktree (the user's `git worktree list` or `git checkout` of a branch, the
  `worktree list` of `bd pick` and of a resolve session) beside the registration of a new slot can read its admin
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
- A free slot left dirty stays in the pool, and each get that needs it pays one `status` of it.
- A takeover can't tell a resumed session of the old holder from one of the new holder once the new holder is gone
  too, nor any session where no process scan runs (Windows): there `lease_reclaimed` refuses nothing.
- A bd older than `taking_over`, `idle_check` and `reclaimed` drops them when it writes the state: the get gives the
  idle lease back from its own copy, but a get that dies meanwhile beside such a bd leaves the slot free.
- The trash of a takeover stays until the next return or clean, or a get below the minimum free space; `bd lease
  status` names it (`trash_bytes`).
- A resumed session that commits between the commit point of a takeover and its move loses the commit from HEAD; the
  reflog of the slot keeps it, and its next command is refused.

## References

- [Git Operations](../../docs/design/git-operations.md): "Lease" and "Pick".
- [Agent CLI guide](../../docs/contributing/agent-cli.md): `bd lease`.
