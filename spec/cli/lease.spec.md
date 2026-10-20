---
name: bd lease
description: Requirements for bd lease, the pool of worktrees that agent sessions lease, one session per slot: get moves a free slot to the HEAD of the source or adds one, return frees it and applies the clean rules of .branchdeck.toml, status reads the state, adopt adds existing worktrees, clean applies the rules on demand.
style: plain-1
targets:
  - ../../crates/bd-lease/src/lease_command.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/bd-core/src/error.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/sync-core/src/lease.rs
  - ../../crates/sync-core/src/lease_clean.rs
  - ../../.branchdeck.toml
  - ../../crates/sync-utils/src/process_scan.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/git-ops/src/lease_slot.rs
  - ../../crates/git-executor/src/worktree_safety_test.rs
---

# bd lease

Status: Implemented
Date: 2026-10-19

## Summary

Agent sessions that share a checkout each work in a worktree of their own and land their commits with `bd pick`
([bd pick](pick.spec.md)). `bd lease` keeps the pool of those worktrees. `bd lease get` leases a slot at the HEAD of
the source checkout: a free slot is moved there, so only the diff between the two commits is written, and a new slot
is added only when no slot is free. `bd lease return` frees the slot once its commits landed. The state in
`$GIT_COMMON_DIR/branch-deck/leases.json` is the authority. A free slot is pool property, and the next get replaces
its content, so no ancestry check decides whether a slot is free. How it works: "Lease" in
[Git Operations](../../docs/design/git-operations.md). Working beside other sessions: `bd help worktrees`
([Agent CLI](agent-cli.spec.md)).

## Goals

- A get in a repository of 1.4M files takes seconds when a slot is free: one `status` and one `checkout` of the
  slot, never a full checkout and never a scan of the other slots.
- A slot whose last commit a rewrite of master made unreachable is reused as any other free slot.
- `bd lease status` answers from the state in milliseconds: no `git status` of any slot.
- Concurrent gets never get the same slot, and never wait for each other's checkouts or for a `bd update`.
- A return never loses work silently: unlanded commits, local changes and a process in the slot refuse it.

## Non-goals

- A fetch: the slot moves to the HEAD of the source as it is.
- A reset of a returned slot to master: the next get moves it.
- A walk of the tree for `.worktreeinclude`: only literal paths and globs in one directory are seeded.
- The process scan on Windows: there it is reported as not checked, and a lease never waits for it.

## Requirements

### Get

- `bd lease get` must lease a free slot of the pool of this repository, moved to the exact HEAD commit of `-C` (or the
  current directory), detached, and print `path`, `lease_id` (a UUID), `holder` (`--holder`, default `agent-<uuid>`),
  `slot` (the worktree id), `source_head`, `leased_at`, `reused`, `seeded` and `processes_checked`. A slot whose
  commit master no longer holds, after a rewrite of master, must be reused like any free slot.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)
- A free slot must be a linked worktree of the same git common dir whose directory exists, and no process other than
  `bd` and its ancestors may have its working directory in it; a busy free slot is skipped (`skipped`, `reason:
  "busy"`). The source may itself be a leased slot.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_busy_free_slot_is_skipped`, `adopt_adds_clean_linked_worktrees_and_refuses_the_rest`)
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
- The checkout of a reused slot and the `worktree add` of a new one must run with `-c checkout.workers=0` (one
  worker per core), unless the repository config sets `checkout.workers`.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`every_checkout_of_the_pool_runs_with_a_worker_per_core_unless_the_repository_sets_it`)
- On macOS a new slot must be an APFS clone (`created_by: "clone"`, `cloned_from`) of a template: a free slot of this
  repository that the get passed over, marked `templating` while it is cloned, else the source checkout. The clone
  must get the `.git` file of a `worktree add --no-checkout`, the template's index refreshed with
  `core.checkStat=minimal`, then `checkout --force` to the source HEAD, `clean -fd` and a clean `status`: the
  template's local changes, staged files and untracked files never reach the lease, its ignored files do. The
  template must be free again afterwards, and the source unchanged.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_new_slot_is_a_clone_of_the_source_without_its_local_work`, `a_new_slot_is_a_clone_of_a_free_slot`)
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_get_of_a_new_slot_by_a_clone`)
- A clone that fails (another volume, no APFS, a file it can't read, not macOS) must leave nothing behind and fall
  back to `git worktree add` (`created_by: "checkout"`).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_clone_that_fails_falls_back_to_a_checkout`)
- A new slot must be `<pool>/<n>/<repository name>`, so the leaf directory keeps the name of the repository. The pool
  directory is `~/.branch-deck/pool/<repository name>-<8 hex digits of the SHA-256 of the canonical common dir>`, or
  `branchdeck.leasePool` of the git config.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)
  [@test] ../../crates/sync-core/src/lease_test.rs (`the_repository_name_is_the_directory_of_its_main_worktree`)
- Two gets at the same time must never lease the same slot: each records its lease as `preparing` under the pool lock
  (`leases.lock`, not the repository lock), then moves its slot without the lock.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`two_leases_never_get_the_same_slot`)
- `--branch <name>` (`-b`) must create the branch at the source HEAD and check it out in the slot; a branch that
  exists must be `branch_exists` (exit 4), an invalid name `invalid_arguments` (exit 2).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_lease_on_a_new_branch_keeps_the_branch_after_the_return`)
- The get must copy into the slot the ignored files and directories that `.worktreeinclude` at the root of the source
  names (`#` comments; a literal path, or a glob of `*`, `?` and `[…]` in its last component; always relative to the
  root), and record them in the lease (`seeded`). A tracked file or one that git does not ignore must not be seeded.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_seed_copies_the_included_ignored_files_and_the_return_removes_them`)
  [@test] ../../crates/sync-core/src/lease_test.rs (`the_seed_candidates_are_literal_paths_and_globs_of_one_directory`, `a_glob_matches_by_stars_question_marks_and_classes`)
- A get that fails after it recorded the lease must give the slot back (free, or dropped when it was never added) and
  say so in the message; it must never leave the lease `preparing`. A `preparing` lease whose process (its pid and
  start time) is gone must be taken as free by the next get.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_failed_get_rolls_the_lease_back`, `a_preparing_lease_of_a_gone_process_is_taken_by_the_next_get`)
- A get of a free slot must run a fixed number of git processes: the repository, the source HEAD, the pool config,
  one `status` and one `checkout` of the slot.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_lease_get_and_return`)
- The process scan must find a process by its working directory, compared by path components, and leave out `bd`
  and its ancestors; a recorded start time must tell a process from a later one with the same pid.
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`a_process_in_a_directory_is_inside_it_and_this_process_is_left_out`, `a_process_runs_only_with_the_start_time_it_was_recorded_with`)

### Return

- `bd lease return` must free the lease of the worktree of `--workspace` (default: of `-C`, else of the current
  directory): remove the seeded paths (`removed`), detach HEAD when it is on a branch (`detached_branch`, the branch
  stays) and leave the slot at its commit.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_seed_copies_the_included_ignored_files_and_the_return_removes_them`, `a_lease_on_a_new_branch_keeps_the_branch_after_the_return`)
- It must refuse, with nothing changed: a worktree without a lease (`no_lease`, exit 2), another `--lease-id`
  (`lease_mismatch`, exit 2), a process other than `bd` and its ancestors in the worktree (`slot_busy`, exit 4), HEAD
  with commits that no branch, tag or remote branch holds (`unlanded_commits`, exit 4, `commits`, the message naming
  `bd pick --from <dir> --follow`), and local changes (`dirty_worktree`, exit 4, `paths`). The commits of the commit
  the slot was leased at never count as unlanded, also after a rewrite of master made them unreachable.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_free_slot_is_reused_after_master_was_rewritten_past_its_commit`)
- The question of the unlanded check is whether the branch of the source has every change of the slot. The get must
  record the source checkout and its branch (none for a detached HEAD). When reachability finds commits, a dry run of
  the pick from the slot onto that branch must decide (`commits_to_pick`: the fork point in the reflog of the branch,
  `git replay` onto its tip, on git objects only, without the lock and without the index of the shared checkout), and
  only the commits it would pick are unlanded. Reachability decides when it finds nothing (the fast path), when the
  source had no branch, when the branch is gone, and when the pick can't tell; the output says which check decided
  (`landed_check`, `landed_branch`), and the refusal says so too.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_after_a_follow_and_a_rewrite_of_master_finds_the_work_landed_by_content`, `a_return_refuses_only_the_commit_that_the_source_branch_lacks`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_session_rewords_its_landed_commit_from_its_lease`, `a_pick_without_follow_finds_the_reworded_commit_present_by_patch_id`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot`)
- The return must find the Bazel servers of the slot (a process whose argv has `--workspace_directory=<dir>`, `<dir>`
  the slot or inside it, such as `<slot>/community`), ask each to exit with SIGTERM once nothing else refuses, wait up
  to 30 s, and name them in `stopped_servers` (pid, workspace, output base). A server that does not exit keeps the
  slot busy (`slot_busy`); `--keep-servers` refuses instead of stopping them. No other process is ever signalled: any
  other process in the slot refuses as `slot_busy` before a server is signalled. Bazel derives the output base from
  the workspace path, so a server serves only its slot, and the output base stays on disk: the next lease of the slot
  gets the same one, with its disk action cache. A get skips a free slot under a live server as busy, also when the
  server works elsewhere: a `--watchfs` server can miss the events of a large checkout, and its build could be stale.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_stops_the_bazel_servers_of_the_slot`, `a_return_stops_no_server_it_should_keep_and_signals_no_other_process`, `a_get_skips_a_free_slot_under_a_live_bazel_server`)
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`the_argv_comes_from_the_procargs_buffer_without_the_environment`)
- The return must wait up to 60 s for git's background maintenance in the slot, then go on: a `git maintenance` or a
  `git gc --auto` and every git below it in the parent chain (`git repack`, `git multi-pack-index write`), told by
  argv. A commit in a slot starts `git maintenance run --auto --detach`, whose repack keeps the slot as its working
  directory for about 30 s and writes only the shared object store. One still there after the wait refuses the return
  as `slot_busy` and is named. A get must not skip a free slot for them.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_return_waits_for_git_maintenance_and_a_get_ignores_it`)
  [@test] ../../crates/sync-utils/src/process_scan_test.rs (`git_maintenance_is_the_maintenance_and_the_gits_it_starts`)
- `--confirm-preserved` must give up the unlanded commits (`given_up`) and discard the local changes (`discarded`)
  with `reset --hard HEAD` and `clean -fd`: the ignored files stay.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_refuses_unlanded_commits_a_dirty_tree_and_a_busy_slot`)

### Clean rules

- The rules are the `clean` array of the `[lease]` table of `.branchdeck.toml` at the root of the checkout, committed
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
  never touched. `{ delete = "<path>" }` removes a path.
  [@test] ../../crates/sync-core/src/lease_clean_test.rs (`the_rules_are_cargo_and_delete_with_their_defaults`, `the_cargo_rule_keeps_the_newest_and_the_young_units`, `the_cargo_rule_prunes_the_classic_layout_by_units_of_name_and_hash`, `a_classic_unit_is_its_name_and_its_hash`)
- An unknown key, an unknown kind, a wrong type or a file that does not parse must be an error of the file.
  [@test] ../../crates/sync-core/src/lease_clean_test.rs (`unknown_keys_and_kinds_are_errors`)
- A rule must be skipped and reported (`error`), never applied, when its path is absolute, leaves the checkout with
  `..`, names `.git`, names the checkout itself, has a symlink on its way or is one, or holds a file that git tracks
  (one `ls-files -z` of the paths of all rules). The walk of a rule never follows a symlink.
  [@test] ../../crates/sync-core/src/lease_clean_test.rs (`an_unsafe_path_is_a_rule_that_is_reported`)
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`an_unsafe_rule_is_skipped_and_reported`)
- The return must read the file once, from the source checkout that the lease recorded (the slot's own when the
  source is gone), and apply the rules after all its checks passed, before it frees the slot: `cleaned` [{`rule`,
  `freed_bytes`, `ms`, `error`?}]. A file that can't be read or parsed (`clean_error`) and a failed rule never fail
  the return.
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`the_return_applies_the_clean_rules_of_the_source`, `a_bad_file_does_not_fail_the_return`)
- `bd lease clean [--free] [--dry-run] [--keep <n>] [--age <duration>] [<dir>...]` must apply the rules of the checkout
  it runs in, read once, to the free slots (`--free`), to the given worktrees, or to that checkout. A slot of the pool
  with a lease or with a process in it is skipped (`skipped`); a checkout that is no slot is cleaned as asked.
  `--dry-run` deletes nothing and says what the rules would free. A file that does not parse is `invalid_arguments`
  (exit 2).
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`clean_skips_a_busy_and_a_leased_slot_and_a_dry_run_deletes_nothing`, `a_bad_file_does_not_fail_the_return`)
- The rules are configured on purpose, so they apply by default: the return and `bd lease clean` delete what they
  name, as the user decided. `bd lease return --no-clean` applies none. `--keep <n>` and `--age <duration>`, on both
  commands, set `keep` and `age` of every cargo rule for that call without an edit of the file; a bad `--age` is
  `invalid_arguments`. These are the only overrides: a `delete` rule has no parameter.
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`no_clean_and_the_overrides_change_what_a_call_cleans`)

### Status

- `bd lease status` must list every slot from the state alone, with its `path`, `state` (`free`, `preparing`,
  `ready`, `templating`, `abandoned`), `dirty`, `missing` and `lease`; it must run no `git status`. `--processes` must add the
  processes in each slot. It writes nothing.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`the_status_reads_the_state_without_a_git_status`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

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

### Error codes

| Exit | Codes |
|---|---|
| 2 | `invalid_arguments`, `no_lease`, `lease_mismatch`, `not_a_slot`, `slot_in_pool` |
| 3 | `locked` (the pool lock, retryable) |
| 4 | `slot_busy`, `unlanded_commits`, `dirty_worktree`, `branch_exists`, `other_volume`, `no_baseline` (a source without commits) |

- The outputs must match `bd schema` (`lease_get`, `lease_return`, `lease_status`, `lease_adopt`, `lease_clean`).
  [@test] ../../crates/bd-lease/src/lease_test.rs (`lease_outputs_match_the_schema`)
  [@test] ../../crates/bd-lease/src/lease_clean_test.rs (`clean_skips_a_busy_and_a_leased_slot_and_a_dry_run_deletes_nothing`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)
- The slot writes (`checkout` twice, `reset`, `clean` twice) must stay in `crates/git-ops/src/lease_slot.rs`, with their
  exact counts in the worktree safety scan.
  [@test] ../../crates/git-executor/src/worktree_safety_test.rs (`production_code_never_writes_the_working_tree`)

## Testing / Local Run

- `cargo nextest run -p bd-cli lease_test` runs the commands end to end on `TestRepo` fixtures, the pool in a
  temporary home directory; `cargo nextest run -p sync-core lease_test process_scan_test` runs the seed patterns and
  the process scan.

## Open Questions / Risks

- `bd lease clean --free` reads the state again right before it cleans a free slot, but takes no pool lock while it
  deletes: a get that leases the slot meanwhile finds a cargo unit or a test artifact gone, which costs a rebuild.

- A clone copies what the template has at that moment: an ignored file that the template writes during the clone can
  be copied half written, and the checkout and the clean keep ignored files. A free slot is the preferred template
  for this reason; the source checkout, where the user and the IDE write, is the last one.

- On Windows the scan of the processes is not available: a slot with a shell in it can be leased, and a `preparing`
  lease of a get that died there stays until its record is edited. A lock file per lease would work on every platform.
- A seeded path that a later commit tracks is removed by the return, and the slot shows a deletion; the next get skips
  it as dirty.
- A free slot left dirty stays in the pool, and each get that needs it pays one `status` of it.

## References

- [Git Operations](../../docs/design/git-operations.md): "Lease" and "Pick".
- [Agent CLI guide](../../docs/guidelines/agent-cli.md): `bd lease`.
