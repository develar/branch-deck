---
name: bd-host
description: Requirements for bd-host, the binary of the remote runs. The client runs a command of a checkout on a host, in a warm host slot that the host lends to that run only, streams its output, and brings back the fetched files, the outputs and the changes. The agent serves the runs under launchd, lends the slots through the lend table of the pool, and replaces its own binary on an upgrade. The CLI, the refusal codes with their exit codes, the report of a run on a descriptor and the hello as JSON.
style: plain-1
targets:
  - ../../crates/bd-host/src/main.rs
  - ../../crates/bd-host/src/cli.rs
  - ../../crates/host-link/src/lib.rs
  - ../../crates/host-client/src/lib.rs
  - ../../crates/host-agent/src/lib.rs
  - ../../crates/lease-pool/src/host_slots.rs
  - ../../crates/lease-pool/src/host_refs.rs
  - ../../crates/lease-pool/src/host_git.rs
  - ../../crates/git-ops/src/host_client.rs
  - ../../crates/git-ops/src/host_objects.rs
  - ../../crates/git-ops/src/worktree_snapshot.rs
  - ../../.branchdeck.toml
---

# bd-host

Status: Draft
Date: 2026-10-28

## Summary

`bd-host` runs a command of a checkout on another machine of the user, the host, and it is the agent on that host
too. It replaces yonder of the IntelliJ monorepo (`community/tools/yonder`). The client and the agent call the lease
pool and git in process, so no `bd` child runs on either end. The why, the slot model and the migration are in
[Host Runs](../../docs/design/host-runs.md). The lend table and the git of the host are in
[bd lease](lease.spec.md), "Host slots", "Host objects" and "Host runs".

A run ships the working tree as one commit, or the committed HEAD with `--head`. Only the commits that the host lacks
travel, as a thin pack. The output streams back. The exit code is the one of the command, or 75 when the host did not
run it. A caller such as bt reads one JSON report of the run on a descriptor of its choice, so stdout carries only
the stdout of the command.

This spec is a draft of the port. A requirement names the test that will cover it as `Test (planned): <file>
(<name>)`. The test lands with its code, and the line then becomes a test link.

## Goals

- One binary for both ends, outside `bd` and the app. The app binary does not grow.
- No `bd` process and no JSON of a child process between the agent and the pool, or between the client and git.
- A machine contract that a caller reads without parsing the words: the refusal codes, the exit codes, the report and
  the hello as JSON, and `bd-host schema` that prints them.
- A migration from yonderd without a new pairing and without ssh: both agents serve one host side by side and share its
  slots.

## Non-goals

- Linux and Windows hosts under launchd. The agent runs on Linux only by hand (`bd-host serve`), without `upgrade`.
- The removal of `bd lease host …` and `bd host …` from bd: yonderd and the yonder client still call them.
- A choice of the slot by the distance between trees, and a spare count of slots on the client Mac.

## Requirements

### The link

- The client and the agent must talk over TCP with TLS 1.3 only, on the rustls provider that the app links
  (`aws-lc-rs`). Each end has a self-signed certificate. The SHA-256 of the certificate is its fingerprint. The client
  must refuse an agent that shows another fingerprint than its host file (`untrusted`). The agent must refuse a client
  whose fingerprint its allow list does not name, and it reads the list at each handshake.
  [@test] ../../crates/host-link/src/channel_test.rs (`a_host_with_another_certificate_is_untrusted`, `a_client_outside_the_allow_list_gets_no_reply`, `a_kept_configuration_resumes_no_session_and_a_removed_client_is_refused`)
  [@test] ../../crates/host-link/src/identity_test.rs (`first_calls_at_the_same_time_answer_one_identity`, `an_identity_is_created_once_and_loaded_after`)
  [@test] ../../crates/host-link/src/allow_test.rs (`the_list_skips_comments_and_keeps_the_names`, `a_name_with_a_control_character_is_refused`)
- The default port is 7742. The protocol is host-link schema 1. One connection carries one hello and one request. A
  frame is a kind byte, a big-endian u32 length and the payload: a message of at most 1 MiB of JSON, or a byte chunk of
  at most 64 KiB. An empty chunk keeps a byte stream alive. An end frame completes it. A frame of an unknown kind, a
  frame over its limit, or a message that the protocol does not allow there is a protocol error.
  [@test] ../../crates/host-link/src/channel_test.rs (`a_frame_over_its_limit_or_of_an_unknown_kind_is_a_protocol_error`, `the_frames_go_to_tls_in_one_write_per_full_buffer_and_at_a_flush`)
  [@test] ../../crates/host-link/src/proto_test.rs (`a_request_and_a_reply_have_a_tagged_json_shape`, `a_start_carries_the_run_spec_inline_without_a_ref`)
- `Request::Pack` must name its target as `{"run": "<id>"}` or `{"base": "<name>"}`, never a ref name. The agent lands
  it as `refs/branch-deck/host/run/<id>` or `refs/branch-deck/host/base/<name>` (`lease_pool::host_refs`). `RunSpec`
  has no ref. A run id and a base name have the rules of a host ref name: 1 to 128 characters of `A-Z a-z 0-9 . _ -`,
  the first a letter or a digit, no `..`, no `.lock` at the end.
  [@test] ../../crates/host-link/src/proto_test.rs (`a_pack_names_a_run_or_a_base_and_never_a_ref`, `a_run_id_names_one_directory_and_one_ref`)
  [@test] ../../crates/sync-utils/src/host_ref_name_test.rs (`a_host_ref_name_is_one_safe_component`)
- A refusal of the agent must be `{"reply": "refused", "code", "message"}` with a code of the table "Refusal codes".
  A client that does not know the code takes it as `agent_error`, and the message names the code, so a code of a
  newer build is no protocol error. A connection that fails is `unreachable`, `untrusted`, `timed_out` or `link`.
  [@test] ../../crates/host-link/src/refusal_test.rs (`a_refusal_prints_its_code_and_an_unknown_code_of_an_agent_is_an_agent_error`)
  [@test] ../../crates/host-link/src/channel_test.rs (`a_port_without_an_agent_is_unreachable_and_a_silent_agent_times_out`)
- Every read and every write must have a deadline. A request that waits gets `Reply::Heartbeat` every 15 s, and the
  client resets its read deadline at each one. A TLS handshake has 3 s in all. The agent takes at most 128 handshakes
  at a time and 8 of one peer address.
  Test (planned): `crates/host-agent/src/serve_test.rs` (`a_waiting_request_gets_a_heartbeat_and_a_handshake_has_a_deadline`)
- On macOS a connection that Local Network privacy refuses (`EHOSTUNREACH`) must go again through `/usr/bin/nc`, which
  the client starts responsible for itself. TLS stays end to end. The process remembers the address, so its next
  connection takes the relay at once.
  [@test] ../../crates/host-link/src/channel_test.rs (`a_relayed_connection_carries_the_handshake_and_the_frames`, `a_remembered_refusal_of_an_address_takes_the_relay_at_once`)
  [@test] ../../crates/host-link/src/relay_test.rs (`the_relay_carries_bytes_both_ways_and_ends_with_the_peer`, `local_network_privacy_is_a_cause_only_of_host_unreachable_on_a_local_address_that_the_relay_cannot_pass`)
  [@test] ../../crates/host-link/src/disclaim_test.rs (`a_disclaimed_child_gets_its_arguments_and_its_pipes`, `a_command_that_changes_the_environment_is_refused`)

### The files

| End | File | Content |
|---|---|---|
| Client | `~/.config/bd-host/identity.crt`, `identity.key` | The certificate and the key of the client, made at the first call, mode 0600 |
| Client | `~/.config/bd-host/hosts/<alias>.json` | `{"alias", "fingerprint", "port", "hostname", "addresses", "repo"}` |
| Client | `~/.config/bd-host/bases/<alias>/<affinity>` | The commit of the base of this affinity on that host after the last base sync |
| Agent | `~/.config/bd-host/agent/identity.crt`, `identity.key` | The certificate and the key of the agent |
| Agent | `~/.config/bd-host/agent/clients` | The allow list: one `<64 hex> <name>` line per client |
| Agent | `~/.cache/bd-host/runs/<run>/` | `spec.json`, `events.log`, `exit.json`, `supervisor.log`, `outputs/`, `changes.patch` |
| Agent | `~/.cache/bd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared` |
| Agent | `~/Library/LaunchAgents/com.jetbrains.bd-host.plist`, `~/Library/Logs/bd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply` |

- Each directory that bd-host makes has the mode 0700, and each file 0600. A removal and a rename go through
  `sync_utils::fs_retry`. A record and a patch older than 7 days go at the next run of the checkout.
  Test (planned): `crates/host-client/src/record_test.rs` (`a_run_record_names_what_a_fetch_takes_and_old_records_go`)

### run

```sh
bd-host run <alias> [--head] [--apply] [--fetch <dir>]... [--no-outputs] [--nice <0-19>] [--repo <path>]
            [--affinity <key>] [--report-fd <n>] -- <command> <args>
```

- `run` must ship the working tree: HEAD plus the tracked and untracked changes that git does not ignore, as one
  commit (`git_ops::host_client::working_tree_commit`). Its author and committer are `bd-host
  <bd-host@bd-host.invalid>` at the date of the committer of HEAD, and its message names HEAD. So the same tree on the
  same HEAD is the same commit, and a second run of it sends no pack. The index, the files and the refs of the
  checkout stay as they are, and no ref names the commit.
  Test (planned): `crates/host-client/src/run_test.rs` (`the_working_tree_ships_as_one_commit_and_the_same_tree_sends_no_pack`)
- With `--head`, `run` must ship the committed HEAD and refuse a checkout with tracked or untracked changes
  (`local_changes`, exit 75).
  Test (planned): `crates/host-client/src/run_test.rs` (`head_refuses_a_checkout_with_local_changes`)
- The client must ask the host which commits it has (`Have`): HEAD and its first-parent ancestors at the distances 1,
  2, 4 … 16 first, then up to 65536 only when the host has none of them, and the merge bases of HEAD with `master` and
  `origin/master`. A host that has none refuses the run with `behind` (exit 75), and the message gives the commit
  count of the history.
  Test (planned): `crates/host-client/src/run_test.rs` (`a_host_without_a_near_commit_gets_the_far_candidates_then_refuses_behind`)
- The order of a run must be: the commit and `Have`; the run record; `Start`; `Follow` on a thread of its own; the pack
  on a connection of its own, with an empty chunk every 15 s while it builds; the base sync of the local
  `origin/master`, else `master`, once per run after the pack, a failure only one warning line; `Fetch` and `Forget`;
  with `--apply`, the apply of the patch. Up to `Start`, nothing runs on the host.
  Test (planned): `crates/host-client/src/run_test.rs` (`a_run_starts_follows_packs_syncs_the_base_and_fetches_in_order`)
- A failed pack must cancel the run, stop the follow and refuse with the code of the pack. The message says whether the
  cancel came.
  Test (planned): `crates/host-client/src/run_test.rs` (`a_failed_pack_cancels_the_run_and_refuses_with_its_code`)
- The run goes to the repository of the same name as the local one beside the repository of the host file, else to
  the one of the host file. `--repo` names another one. `--affinity` names the affinity of the lend, by default
  `checkout-<12 hex>` of the local checkout path. The base of the pack goes to `base:<affinity>`.
  Test (planned): `crates/host-client/src/run_test.rs` (`the_repository_of_a_run_is_the_one_of_the_same_name_beside_the_host_file`)
- `--nice` is 10 by default: the niceness of the prepare steps and the command. `--no-outputs` leaves the outputs of
  `[host]` alone. `--fetch <dir>` (repeatable) brings back the new entries of `<dir>`, relative to the slot root.
- Ctrl+C before `Start` must run nothing on the host and exit 130 (`interrupted`). Ctrl+C after it must send `Cancel`
  and stop the pack, and the output follows to the end of the run. A second press stops the follow at once with 130,
  and the run goes on on the host.
  Test (planned): `crates/bd-host/tests/cli.rs` (`ctrl_c_before_the_start_runs_nothing_and_after_it_cancels_the_run`)
- The stdout of `bd-host run` must hold only the stdout of the command, line by line as it comes. Every line of
  bd-host and every stderr line of the host go to the stderr. The lines of bd-host start with `bd-host: `:
  `bd-host: run <id> of <commit> on <alias>` first, `bd-host: running in <slot> (slot <n>): load …, nice …, … open
  files, git <version>, the login environment of <shell> from <age> ago` right before the command, and
  `bd-host: phases: pack 3.1s, upload 2.0s, slot 0.1s, … total 10m13s` at the end.
  Test (planned): `crates/bd-host/tests/cli.rs` (`stdout_holds_only_the_command_and_the_lines_of_bd_host_go_to_stderr`)
- A follow that lost its connection must connect again with waits from 1 s up to 30 s and go on from the last offset
  it showed. After 10 min without a reply it refuses with `timed_out` (exit 255) and names `bd-host follow`. A follow
  that the agent answers with `unknown_run` after the start exits 255, never 75: whether the command ran is unknown.
  Test (planned): `crates/host-client/src/run_test.rs` (`a_lost_follow_connects_again_from_its_offset_and_an_unknown_run_is_255`)

### The fetch, the outputs and the changes

- A fetch must take only what the run record asks for: the staged entries below a `--fetch` directory, the files of an
  output of `[host]` that the record names, and the patch of a record with `changes`. A path that is absolute, has
  `..` or `\`, is outside those, or is inside a `.git` directory in any case (`.GIT`, `.git.`, `git~1`) is skipped,
  and the fetch then fails with `fetch_failed` (exit 255). No write follows a local link. Each file goes to
  `<name>.bd-host-part` first and takes its name only at the size that the agent named. The run forgets itself on the
  host only when the client saw its end and every file came.
  Test (planned): `crates/host-client/src/fetch_test.rs` (`a_fetch_takes_only_what_the_record_names_and_never_writes_through_a_link`)
- The first file of an output must remove the local file or directory of that output. An output that the run did not
  make brings nothing, so the local copy of an earlier run stays.
  Test (planned): `crates/host-client/src/fetch_test.rs` (`the_first_file_of_an_output_replaces_its_local_copy`)
- With `--apply`, the patch of the changes that the command made goes to `<git dir>/branch-deck/host/runs/<run>.patch`
  and applies to the files of the working tree, not to the index, all or nothing (`git_ops::host_client::apply_patch`).
  A patch that does not apply refuses with `apply_failed` (exit 255), names the patch, and the working tree stays as it
  was.
  Test (planned): `crates/host-client/src/run_test.rs` (`apply_brings_the_changes_back_all_or_nothing`)

### The report of a run

- With `--report-fd <n>`, `run` must write one JSON object and a line end to the descriptor `n` on every way out after
  the parse of its command line: a run that exited, a refusal and an interrupt. It writes nothing else there, and it
  closes the descriptor after the object. A descriptor that is not open refuses with `usage` (exit 2) before anything
  runs.
  Test (planned): `crates/bd-host/tests/cli.rs` (`the_report_goes_to_its_descriptor_on_every_way_out`)
- The report must have the shape of the golden JSON below. `exit` is the exit code of `bd-host`. `command_exit` is the
  exit code of the command, `null` when it did not run or its end is unknown; a command that a signal ended answers
  128 plus the signal number. `refusal` is `null`, or `{"code", "message"}` with a code of the table below. `run`,
  `commit` and `head` are `null` before the client knows them. `phases` lists the steps in their order, each with its
  milliseconds. `fetched` lists the files that came back, relative to the checkout root with `/`. `applied_files` is
  the count of the files that `--apply` changed, `null` without `--apply`.
  Test (planned): `crates/bd-host/src/report_test.rs` (`the_report_of_a_run_is_the_golden_json_of_the_spec`)

```json golden=run-report
{
  "schema": 1,
  "alias": "studio",
  "run": "1793181600000-4242",
  "commit": "8d4b3790b5c0e7f1a2d3c4b5a6978877665544aa",
  "head": "8d4b3790b5c0e7f1a2d3c4b5a6978877665544aa",
  "working_tree": false,
  "repository": "~/projects/idea-1",
  "exit": 0,
  "command_exit": 0,
  "refusal": null,
  "phases": [
    { "name": "pack", "ms": 3100 },
    { "name": "upload", "ms": 2000 },
    { "name": "slot", "ms": 120 },
    { "name": "pack_wait", "ms": 0 },
    { "name": "checkout", "ms": 4200 },
    { "name": "prepare", "ms": 0 },
    { "name": "command", "ms": 600000 },
    { "name": "stage", "ms": 800 },
    { "name": "fetch", "ms": 900 },
    { "name": "total", "ms": 612000 }
  ],
  "fetched": ["out/bt-runs/20261028-100000-0001/summary.json"],
  "applied_files": null
}
```

- The steps are `pack` and `upload` of the client; `git`, `slot`, `pack_wait`, `checkout`, `prepare`, `command` and
  `stage` of the host; `fetch` and `total` of the client. A run that ends early has only the steps up to its last one.
  Only a run that waited for an upgrade of git has `git`.
  Test (planned): `crates/host-client/src/phases_test.rs` (`the_phases_are_the_steps_of_both_ends_in_their_order`)

### follow, cancel, fetch, forget, runs

| Command | Effect |
|---|---|
| `bd-host follow <alias> <run>` | Follows the output of a run from its first line, and exits with the exit code of the command at its end. Ctrl+C leaves, and the run goes on. |
| `bd-host cancel <alias> <run>` | Asks the agent to interrupt the run: SIGINT to the process group of the command, SIGKILL after 120 s, or no command when it has not started. It returns when the agent has the request. |
| `bd-host fetch <alias> <run> [--fetch <dir>]...` | Copies the files of a run whose fetch failed, by the run record of the checkout of the working directory. An explicit `--fetch` replaces the directories of the record. Without a record, only `--fetch` directories come, and no `--fetch` refuses with `usage`. |
| `bd-host forget <alias> <run>` | Removes a run and its files on the host. The host keeps an ended run for 24 h. |
| `bd-host runs <alias> [--json]` | Lists the runs that the agent keeps, each with its affinity, commit, command, start, exit and the steps of 0.1 s or more. |

- A fetch or a forget of a run that goes on must refuse with `bad_request`. A run id that the agent does not keep
  refuses with `unknown_run`. Each of these commands exits 0, or 255 with a refusal, or 2 for a usage error.
  Test (planned): `crates/bd-host/tests/cli.rs` (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

### hello

- `bd-host hello <alias> [--json]` must connect, send the hello and print the answer of the agent. With `--json` it
  prints one JSON object of the shape of the golden JSON below on stdout. `build` is the SHA-256 of the binary, which
  names a build. `version` is the version of Branch Deck. `same_build` tells whether both ends run the same build. A
  refusal prints `{"schema": 1, "alias", "refusal": {"code", "message"}}` with `--json` and exits 255.
  Test (planned): `crates/bd-host/src/report_test.rs` (`the_hello_is_the_golden_json_of_the_spec`)

```json golden=hello
{
  "schema": 1,
  "alias": "studio",
  "address": "192.168.1.20:7742",
  "agent": {
    "version": "0.37.0",
    "build": "5e1f0c3a9b7d4e2f8a6c1b3d5e7f9a0b2c4d6e8f1a3b5c7d9e0f2a4b6c8d0e1f",
    "schema": 1,
    "os": "macos",
    "arch": "aarch64",
    "features": ["upgrade"]
  },
  "client": {
    "version": "0.37.0",
    "build": "5e1f0c3a9b7d4e2f8a6c1b3d5e7f9a0b2c4d6e8f1a3b5c7d9e0f2a4b6c8d0e1f",
    "schema": 1
  },
  "same_build": true
}
```

- A hello of another schema must refuse with `schema` (exit 75 for a run). A hello of the same schema and another build
  must let the run go on, with one line that names `bd-host upgrade <alias>`.
  Test (planned): `crates/host-client/src/host_test.rs` (`another_schema_refuses_and_another_build_runs_with_a_hint`)

### slots

```sh
bd-host slots <alias> [--json]
bd-host slots --local [-C <repo>] [--shrink <n>] [--sizes] [--json]
```

- `bd-host slots <alias>` must ask the agent for the lend table of each repository that it served (`Request::Slots`):
  each slot, its state, its run, its affinity, its last use, and the queue. It writes nothing. The reply carries each
  table as the JSON of its `host-slots.json`, so the protocol keeps no second shape of the table.
  [@test] ../../crates/host-link/src/proto_test.rs (`the_slots_of_the_agent_carry_the_lend_table_as_the_pool_writes_it`)
- `--local` must read the table of the repository of `-C` on this machine, without the agent. `--shrink <n>` gives
  the host slots past `n` back to the pool in its own process (`lease_pool::host_slots::shrink`), also without a running
  `serve`, and keeps `n` as the limit of later fills. `--sizes` measures the directories that the clean rules of each
  slot name.
  Test (planned): `crates/bd-host/tests/cli.rs` (`slots_shows_the_lend_table_and_shrinks_it_without_the_agent`)

### setup

```sh
bd-host setup <alias> [--repo <path>] [--port 7742] [--name <name>] [--binary <path>]
bd-host setup --agent --client <fingerprint> --name <name> [--port 7742]
bd-host setup --import-yonder
bd-host setup --retire-yonderd
```

- `bd-host setup <alias>` must prepare a host over ssh, run by the user only: copy `--binary`, else this executable,
  to `~/.local/bin/bd-host` of the host when the host has another build there; run `~/.local/bin/bd-host setup --agent
  --client <the fingerprint of this client> --name <name>` there; and write the host file from its answer. Each ssh
  call has 30 s, the copy 120 s (`ssh_failed`).
  Test (planned): `crates/host-client/src/setup_test.rs` (`setup_copies_the_binary_runs_the_agent_setup_and_writes_the_host_file`)
- `bd-host setup --agent` must make the identity of the agent when it has none, add the client to the allow list,
  write the LaunchAgent `com.jetbrains.bd-host` (`KeepAlive`, `AbandonProcessGroup`, `~/.local/bin/bd-host serve
  --port <port>`), and bootstrap or restart it. It prints one JSON line `{"fingerprint", "port", "launchd":
  "bootstrapped" | "restarted" | "none"}`. A client name with a control character refuses with `bad_request`, and a
  failed `launchctl` with `launchd_failed`.
  Test (planned): `crates/host-agent/src/setup_test.rs` (`the_agent_setup_writes_the_identity_the_allow_list_and_the_launch_agent`)
- `bd-host setup --import-yonder` must take what yonder has on this machine, so no new pairing and no ssh is needed:
  the identity and the host files of the client from `~/.config/yonder` (each host file with the port 7742), and the
  identity and the allow list of the agent from `~/.config/yonderd`, then the LaunchAgent as `--agent` writes it. It
  copies, so yonder keeps working. An identity or a host file that bd-host has already stays. The clients of the allow
  list of yonderd join the allow list of bd-host, and a client that it names keeps its name. It prints what it took. A
  machine without yonder files refuses with `no_yonder`.
  [@test] ../../crates/host-link/src/identity_test.rs (`an_import_copies_the_identity_of_yonder_once_and_keeps_another_one`)
  [@test] ../../crates/host-link/src/allow_test.rs (`the_clients_of_another_list_join_and_a_known_client_keeps_its_name`)
  Test (planned): `crates/host-agent/src/setup_test.rs` (`import_yonder_takes_the_identities_the_hosts_and_the_allow_list`)
- `bd-host setup --retire-yonderd` must wait until no yonderd run holds its run lock, boot out
  `com.jetbrains.yonderd`, delete the refs `refs/yonder/*` of each repository of `~/.cache/yonderd/repos` through a ref
  transaction, then delete `~/.cache/yonderd` and `~/.config/yonderd`. The user only runs it.
  Test (planned): `crates/host-agent/src/setup_test.rs` (`retire_yonderd_waits_for_its_runs_and_removes_its_refs_and_files`)

### upgrade

```sh
bd-host upgrade <alias> [--binary <path>] [--force]
```

- `upgrade` must send its own executable, or `--binary`, over the link to an agent on macOS with the feature
  `upgrade`. It refuses a binary of another platform than the hello names before it sends it (the Mach-O header),
  and it sends nothing when the agent runs that build already, unless `--force`. A host that is not macOS refuses with
  `agent_failed`. An agent runs it only after the user agrees.
  Test (planned): `crates/host-client/src/upgrade_test.rs` (`upgrade_sends_this_executable_once_and_refuses_another_platform`)
- The agent must check the size and the SHA-256 of the binary (`bad_binary`), run `<staged> version --json` within
  10 s and refuse another schema or a binary without the feature `upgrade` (`bad_binary`), refuse a lower version
  without `force` (`downgrade`), and refuse another LaunchAgent than the one on the host (`plist_changed`). Then it
  keeps the binary as `~/.local/bin/bd-host.previous`, puts the new one at `~/.local/bin/bd-host` by a rename,
  answers, waits up to 20 s for its other requests, and exits. launchd starts the new binary.
  Test (planned): `crates/host-agent/src/upgrade_test.rs` (`the_agent_checks_the_binary_keeps_the_previous_one_and_swaps_by_rename`)
- A watchdog must wait for the new binary to listen, for the drain plus 60 s. Else it renames the previous binary
  back, restarts the LaunchAgent, and writes `~/.cache/bd-host/upgrade.json` with the outcome. The client connects
  again every 1 s to 5 s for up to 110 s: the previous build at the end refuses with `rolled_back`, no answer with
  `unreachable`. A run goes on through the restart.
  Test (planned): `crates/host-agent/src/upgrade_test.rs` (`the_watchdog_brings_back_the_previous_binary_when_the_new_one_does_not_listen`)
- `upgrade` must also replace an installed copy of the skill of another build that has no local edits, and say so.

### serve

- `bd-host serve [--port 7742]` must print one JSON line `{"listening": "<address>", "fingerprint": "<64 hex>"}`,
  then log on stderr. It answers the requests of the protocol, starts one supervisor per `Start`, and runs the keeper
  thread.
  Test (planned): `crates/host-agent/tests/serve.rs` (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)
- The supervisor must lend a slot through `lease_pool::host_slots::lend_waiting` with its own pid and the affinity of
  the spec, up to 1 h (`slot_timeout`). It writes the queue line to the log once a minute. A run with no slot whose
  last fill failed refuses at once (`lease_failed`). A lend that git refuses below its floor ends the run with
  `old_git` and the hint of Branch Deck.
  Test (planned): `crates/host-agent/tests/serve.rs` (`two_runs_of_one_affinity_run_in_parallel_and_a_third_waits_for_a_slot`)
- The supervisor must wait for the pack of its commit when the repository lacks it: up to 30 min, and up to 2 min
  without a chunk (`pack_missing`). It checks out the commit by the diff (`lease_pool::host_slots::checkout`), and
  writes the reason of a forced checkout to the log. On every way out, after `exit.json`, it gives the slot back
  (`give_back_run`), which also deletes the run ref. No signal cuts the give-back.
  Test (planned): `crates/host-agent/tests/serve.rs` (`a_run_waits_for_its_pack_checks_out_by_the_diff_and_gives_its_slot_back`)
- With changes asked for, the supervisor must take the tree of the slot after the prepare steps
  (`snapshot_run`), and after the command the patch from it (`changes_run`).
  Test (planned): `crates/host-agent/tests/serve.rs` (`the_changes_of_a_command_come_back_as_a_patch_without_the_prepare_steps`)
- The keeper must wake once a minute and at each `Start` of a repository, and run `ensure_host_git`, `reclaim`,
  `reclaim_refs` and `fill` in process, one repository at a time. A failed fill waits 1, 5, then 15 minutes.
  Test (planned): `crates/host-agent/tests/serve.rs` (`the_keeper_fills_the_slots_and_backs_off_after_a_failed_fill`)
- `serve` must hold the power assertions of `caffeinate -i -s -w <pid>` while a client has a connection after its
  hello, and each supervisor for its whole life. Elsewhere than on macOS nothing holds them.
  Test (planned): `crates/host-agent/src/awake_test.rs` (`serve_holds_the_assertions_while_a_client_is_connected`)

### The environment of a run and `[host]`

- A prepare step and the command must get, in this order: the environment of the agent; the login environment of the
  user (`<shell> -l -i -c 'exec bd-host dump-env <file>'`, 30 s, captured at the start of `serve` and again when an
  hour old); the directory of the git of the host right before `/usr/bin` in `PATH`; `BD_HOST_RUN`,
  `BD_HOST_AFFINITY`, `BD_HOST_COMMIT` and `BD_HOST_SLOT`; and `[host.env]`. The soft open-files limit is 65536, or
  the hard limit when that is lower.
  Test (planned): `crates/host-agent/src/run_env_test.rs` (`a_run_gets_the_login_environment_the_git_of_the_host_and_its_variables`)
- The agent must read `[host]` of `.branchdeck.toml` from the commit of the run, never from the files of the slot. A
  commit whose `.branchdeck.toml` has no `[host]` table gets its `.yonder.toml`, until the repositories moved. An
  unknown key or a value of the wrong shape ends the run with `bad_config` before the command. The other tables of
  `.branchdeck.toml` are not the agent's.
  [@test] ../../crates/host-link/src/config_test.rs (`the_host_table_of_branchdeck_toml_parses_and_an_unknown_key_refuses`, `a_commit_without_the_host_table_gets_its_yonder_toml`, `a_typo_or_a_bad_value_is_an_error_that_names_the_file`)

```toml
[host]
# the paths that the command makes and that come back; the agent removes each one before the command
outputs = ["tests/results", "playwright-report"]

[host.env]
RUST_BACKTRACE = "1"                   # a text, as it is
PATH = { prepend = ["~/.cargo/bin"] }  # also `append`; a relative entry is below the slot root
E2E_PORT = { port = 3100 }             # 3100 plus the slot number, or the next free TCP port
CI = { unset = true }                  # removed

[[host.prepare]]
name = "pnpm install"
run = ["pnpm", "install", "--frozen-lockfile"]
inputs = ["pnpm-lock.yaml"]            # the fingerprint: `run` and the object id of each input at the commit
creates = ["node_modules"]             # a missing path runs the step again
```

- A prepare step must run before the command when its fingerprint differs from the one of its last success in the
  slot, or when a path of `creates` is missing. A step without `inputs` runs once per slot. A step that exits with
  another code than 0 ends the run with `prepare_failed`. The fingerprints are keyed by the `generation` of the slot,
  so a give-back that discarded changes or a clean runs each step again.
  Test (planned): `crates/host-agent/src/prepare_test.rs` (`a_prepare_step_runs_when_its_inputs_change_or_a_created_path_is_missing`)

### version, schema, skill

- `bd-host version [--json]` must print the version of Branch Deck, the schema of the protocol, the features and the
  SHA-256 of the binary: `{"version", "schema", "features", "sha256"}`. The hello, `Request::Upgrade`,
  `Reply::Upgraded` and this line keep their shapes within the schema, so an agent takes the next binary over the link.
  [@test] ../../crates/host-link/src/proto_test.rs (`the_version_line_names_the_build_and_a_check_adds_the_launch_agent`, `a_hello_names_the_binary_and_the_features_of_this_build`, `an_upgrade_and_its_reply_have_a_tagged_json_shape`)
- `bd-host schema` must print one JSON object: the protocol schema and port, every refusal code of the table below
  with its exit code and words, the exit codes, and the JSON schemas of the report and of the hello.
  Test (planned): `crates/bd-host/src/report_test.rs` (`schema_lists_every_code_and_the_shapes_of_the_report_and_the_hello`)
- `bd-host skill install [--dir <dir>] [--force]` must write the skill `bd-host` into `~/.claude/skills/bd-host/`
  with its references, from the files that the binary carries. The marker `.bd-host-skill.sha256` lists the SHA-256 of
  each file. A copy whose files differ from the marker has local edits: an install refuses with `skill_edited`
  (exit 255), and `--force` replaces it. `bd-host skill print` prints `SKILL.md`.
  Test (planned): `crates/bd-host/src/skill_test.rs` (`skill_install_replaces_a_copy_of_another_build_and_keeps_local_edits`)

### Refusal codes

A refusal says `bd-host: <code>: <message>` on the stderr. A caller acts on the code and the exit code, never on the
message. Exit 75 means that the host did not run the command, so the caller can run it locally. A refusal of `run`
before `Start`, and a refusal of the agent whose phases have no `command`, exits 75. Any other refusal exits 255,
except `usage` (2) and `interrupted` (130).

| Code | Exit | When |
|---|---|---|
| `usage` | 2 | An alias that is not `[0-9A-Za-z._-]+`, a run without a command, a working directory in no git checkout, a descriptor of `--report-fd` that is not open, a fetch without a record and without `--fetch`. A command line that clap refuses exits 2 too. |
| `interrupted` | 130 | Ctrl+C before the start, or a second Ctrl+C during the follow. |
| `config` | 75 | No home directory, or an identity, a record or a file of bd-host that cannot be read or written. |
| `no_host` | 75 | No host file for the alias. The message names `bd-host setup <alias>`. |
| `bad_host_file` | 75 | A host file that is not JSON of its shape, or that names another alias. |
| `unreachable` | 75 | No address answers. The message names each address and the lookup, and Local Network privacy when the relay cannot pass it. |
| `untrusted` | 75 | The agent shows another certificate than the pin, or it refuses the certificate of the client. |
| `schema` | 75 | The agent speaks another schema of host-link. |
| `timed_out` | 75, 255 after the start | No reply within the deadline of the request; for a follow, 10 min over all reconnects. |
| `link` | 75, 255 after the start | The connection closed, or a reply came that the protocol does not allow there. |
| `local_changes` | 75 | `--head` with tracked or untracked changes. |
| `behind` | 75 | The host has none of the candidate commits. |
| `git_failed` | 75 | A git call of the client failed: the snapshot, the candidates or the pack. The message has the words of git. |
| `bad_request` | 75, 255 after the start | A request that the agent does not accept, such as a fetch of a run that goes on. |
| `bad_run_id` | 75 | A run id that cannot name a directory. |
| `run_exists` | 75 | A start with the id of a kept run. |
| `unknown_run` | 255 | No kept run has the id. After a start: the run is gone, and whether its command ran is unknown. |
| `repo_missing` | 75 | The repository path is no git repository on the host. |
| `pack_failed` | 75 | The agent cannot land the pack: no pack, or git refused it. |
| `pack_missing` | 75 | The pack of the commit did not come within 30 min, or sent no chunk for 2 min. |
| `slot_timeout` | 75 | The run got no host slot within an hour. |
| `lease_failed` | 75 | The lend failed, or no slot exists and the last fill failed. The message has the error of the pool. |
| `old_git` | 75 | The git of the host is below the floor of Branch Deck, and Homebrew cannot upgrade it. The message is the hint of Branch Deck. |
| `checkout_failed` | 75 | The commit cannot be checked out in the slot. |
| `bad_config` | 75 | `[host]` of `.branchdeck.toml` (or `.yonder.toml`) of the commit does not parse, has an unknown key or a value of the wrong shape. |
| `prepare_failed` | 75 | A prepare step exited with another code than 0. |
| `spawn_failed` | 75 | The command cannot start. |
| `agent_error` | 75, 255 after the command started | The agent cannot read or write its own files, or a supervisor got lost. |
| `fetch_failed` | 255 | The fetch did not copy every file. The run stays on the host for `bd-host fetch` and `bd-host forget`. |
| `apply_failed` | 255 | The changes do not apply to the working tree, or they did not come because the fetch failed. |
| `bad_binary` | 255 | The binary of an upgrade has another size or SHA-256, or `version --json` does not answer as an agent of this schema with `upgrade`. |
| `downgrade` | 255 | The binary of an upgrade has a lower version, and the request has no `force`. |
| `plist_changed` | 255 | The new binary would write another LaunchAgent than the one on the host: `bd-host setup` installs it. |
| `upgrade_failed` | 255 | The agent cannot stage, keep or swap its binary, or start the watchdog, or another upgrade runs. |
| `rolled_back` | 255 | The new agent did not listen in time, and the previous binary answers. |
| `agent_failed` | 255 | The binary for a host cannot be read or is of another platform, the host is not macOS for `upgrade`, or `setup --agent` on the host failed. |
| `ssh_failed` | 255 | An ssh call of `setup <alias>` failed or ran past its deadline. |
| `launchd_failed` | 255 | A `launchctl` call of `setup` failed. |
| `no_yonder` | 255 | `setup --import-yonder` found no file of yonder or yonderd. |
| `skill_edited` | 255 | `skill install` found a copy with local edits, without `--force`. |

- Each code must be one variant of the refusal type of `host-link`, with its name and exit in one table that
  `bd-host schema` prints, so a code cannot have two exits.
  [@test] ../../crates/host-link/src/refusal_test.rs (`every_refusal_code_has_one_name_and_one_exit`, `the_exit_of_a_code_follows_how_far_the_run_came`)
- The yonder code `lock_timeout` is `slot_timeout` here, and `bd_failed` is `git_failed`, because no `bd` process
  runs. `old_agent` is gone: every agent of host-link knows the upgrade request.

## Testing / Local Run

- `cargo nextest run -p host-link -p host-client -p host-agent -p bd-host` runs the unit tests and the process tests
  on 127.0.0.1, with `TestRepo` pools in a temporary home directory. No fake `bd` exists.
- The live check is `bd-host hello studio`, then a run with a dirty tree and `--apply`, a run with a base, and five
  parallel runs, from a lease of this repository.

## Open Questions / Risks

- A slot serves several checkouts. Ignored outputs of another branch stay, and a tool that trusts timestamps can build
  stale output.
- A daemon that leaves the slot directory escapes the process scan of the give-back.
- While both agents serve one host, a yonderd that upgrades the git of the host holds the lends of `bd-host` too,
  because they share the lend table. That is the intent: no run uses git while Homebrew replaces it.

## References

- [Host Runs](../../docs/design/host-runs.md): why, the slot model, the processes, the upgrade, the migration.
- [bd lease](lease.spec.md): "Host slots", "Host objects", "Host runs".
- [Git Operations](../../docs/design/git-operations.md): "Host slots".
