---
name: brd-host
description: Requirements for brd-host, the binary of the remote runs. The client runs a command of a checkout on a host, in a warm host slot that the host lends to that run only, streams its output, and brings back the fetched files, the outputs and the changes. The agent serves the runs under launchd, lends the slots through the lend table of the pool, keeps the minimum free space of the host and trims its pools on request, so the host needs no app and no bd, and replaces its own binary on an upgrade. The CLI, the refusal codes with their exit codes, the report of a run on a descriptor and the hello as JSON.
style: plain-1
targets:
  - crates/bd-host/src/main.rs
  - crates/bd-host/src/cli.rs
  - crates/bd-host/src/schema.rs
  - crates/bd-host/src/skill.rs
  - crates/bd-host/skill/SKILL.md
  - crates/host-agent/src/table.rs
  - crates/host-link/src/lib.rs
  - crates/host-client/src/lib.rs
  - crates/host-agent/src/lib.rs
  - crates/lease-pool/src/host_slots.rs
  - crates/lease-pool/src/host_refs.rs
  - crates/lease-pool/src/host_git.rs
  - crates/lease-pool/src/host_runs.rs
  - crates/git-ops/src/host_client.rs
  - crates/git-ops/src/host_objects.rs
  - crates/git-ops/src/worktree_snapshot.rs
  - .branchdeck.toml
  - src-tauri/tauri.macos-bd-host.conf.json
  - scripts/build-bd-host.mjs
  - crates/bd-host/src/report_fd.rs
  - crates/lease-pool/src/host_brew.rs
  - crates/pool-disk/src/report.rs
  - crates/pool-disk/src/trim_report.rs
  - crates/host-agent/src/keeper.rs
---

# brd-host

Status: Draft
Date: 2026-10-28

## Purpose

`brd-host` runs a command of a checkout on another machine of the user, the host, and it is the agent on that host
too. It replaced yonder of the IntelliJ monorepo, whose agent yonderd was retired on 2026-10-28. The client and the
agent call the lease pool and git in process, so no `brd` child runs on either end. The why and the slot model are in
[Host Runs](../../../../docs/design/host-runs.md). The lend table and the git of the host are in
[brd lease](../lease/spec.md), "Host slots", "Host objects" and "Host runs".

A run ships the working tree as one commit, or the committed HEAD with `--head`. Only the commits that the host lacks
travel, as a thin pack. The output streams back. The exit code is the one of the command, or 75 when the host did not
run it. A caller such as bt reads one JSON report of the run on a descriptor of its choice, so stdout carries only
the stdout of the command.

This spec is a draft of the port. A requirement names the test that will cover it as `Test (planned): <file>
(<name>)`. The test lands with its code, and the line then becomes a test link.

## Goals

- One binary for both ends, outside `brd` and the app. The app binary does not grow.
- A host needs no app and no `brd`: the agent keeps the minimum free space of its pools by itself and trims them on
  request (`brd-host trim`), so nothing sends an agent there for the app.
- No `brd` process and no JSON of a child process between the agent and the pool, or between the client and git.
- A machine contract that a caller reads without parsing the words: the refusal codes, the exit codes, the report and
  the hello as JSON, and `brd-host schema` that prints them.

## Non-goals

- Linux and Windows hosts under launchd. The agent runs on Linux only by hand (`brd-host serve`), without `upgrade`.
- Windows: brd-host, its client and its agent run on macOS and Linux only. The Windows app does not bundle it.
- A choice of the slot by the distance between trees, and a spare count of slots on the client Mac.

## Requirements

### Requirement: The link: The ends talk over TLS 1.3 with self-signed certificates

The client and the agent MUST talk over TCP with TLS 1.3 only, on the rustls provider that the app links
(`aws-lc-rs`). Each end has a self-signed certificate. The SHA-256 of the certificate is its fingerprint.

#### Scenario: Each end has one identity
- **WHEN** an end needs its certificate for the first time, also from several calls at the same time
- **THEN** one self-signed identity is created, and later calls load it
  [@test] crates/host-link/src/identity_test.rs (`first_calls_at_the_same_time_answer_one_identity`, `an_identity_is_created_once_and_loaded_after`)

### Requirement: The link: The client refuses an agent of another fingerprint

The client MUST refuse an agent that shows another fingerprint than its host file (`untrusted`).

#### Scenario: A host with another certificate
- **WHEN** the agent shows another fingerprint than the host file of the client
- **THEN** the client refuses it with `untrusted`
  [@test] crates/host-link/src/channel_test.rs (`a_host_with_another_certificate_is_untrusted`)

### Requirement: The link: The agent refuses a client outside its allow list

The agent MUST refuse a client whose fingerprint its allow list does not name, and it reads the list at each
handshake.

#### Scenario: A client outside the allow list
- **WHEN** a client whose fingerprint the allow list does not name connects
- **THEN** the agent refuses it
  [@test] crates/host-link/src/channel_test.rs (`a_client_outside_the_allow_list_gets_no_reply`)

#### Scenario: A client removed from the list
- **GIVEN** a client that the allow list named at an earlier handshake
- **WHEN** the list no longer names it and it connects again
- **THEN** the agent reads the list at this handshake and refuses it
  [@test] crates/host-link/src/channel_test.rs (`a_kept_configuration_resumes_no_session_and_a_removed_client_is_refused`)

#### Scenario: The lines of the allow list
- **WHEN** the agent reads its allow list
- **THEN** it takes the fingerprints and the names of the lines
  [@test] crates/host-link/src/allow_test.rs (`the_list_skips_comments_and_keeps_the_names`, `a_name_with_a_control_character_is_refused`)

### Requirement: The link: One connection carries one hello and one request

The default port SHALL be 7742, and the protocol host-link schema 1. One connection MUST carry one hello and one
request.

#### Scenario: A connection of a request
- **WHEN** a client connects to the agent
- **THEN** the connection carries one hello and one request, and their messages have the shapes of the protocol
  [@test] crates/host-link/src/proto_test.rs (`a_request_and_a_reply_have_a_tagged_json_shape`, `a_start_carries_the_run_spec_inline_without_a_ref`)

### Requirement: The link: A frame is a kind byte, a length and a payload of a limit

A frame MUST be a kind byte, a big-endian u32 length and the payload: a message of at most 1 MiB of JSON, or a byte
chunk of at most 64 KiB. An empty chunk keeps a byte stream alive. An end frame completes it.

#### Scenario: A byte stream
- **WHEN** an end sends a byte stream
- **THEN** it goes as chunks of at most 64 KiB, an empty chunk keeps it alive, and an end frame completes it
  [@test] crates/host-link/src/channel_test.rs (`the_frames_go_to_tls_in_one_write_per_full_buffer_and_at_a_flush`)
  [@test] crates/host-agent/tests/serve/follow.rs (`a_fetch_of_many_small_files_takes_few_tls_records`)

### Requirement: The link: A frame outside the protocol is a protocol error

A frame of an unknown kind, a frame over its limit, or a message that the protocol does not allow there MUST be a
protocol error.

#### Scenario: A frame over its limit or of an unknown kind
- **WHEN** an end reads a frame of an unknown kind, a frame over its limit, or a message that the protocol does not
  allow there
- **THEN** it is a protocol error
  [@test] crates/host-link/src/channel_test.rs (`a_frame_over_its_limit_or_of_an_unknown_kind_is_a_protocol_error`)

### Requirement: The link: A pack names a run or a base, never a ref

`Request::Pack` MUST name its target as `{"run": "<id>"}` or `{"base": "<name>"}`, never a ref name. The agent lands
it as `refs/branch-deck/host/run/<id>` or `refs/branch-deck/host/base/<name>` (`lease_pool::host_refs`). `RunSpec`
has no ref.

#### Scenario: The target of a pack
- **WHEN** the client sends a pack
- **THEN** `Request::Pack` names `{"run": "<id>"}` or `{"base": "<name>"}`, and the agent lands it as
  `refs/branch-deck/host/run/<id>` or `refs/branch-deck/host/base/<name>`
- **AND** neither the request nor `RunSpec` carries a ref name
  [@test] crates/host-link/src/proto_test.rs (`a_pack_names_a_run_or_a_base_and_never_a_ref`)

### Requirement: The link: A run id and a base name are host ref names

A run id and a base name MUST have the rules of a host ref name: 1 to 128 characters of `A-Z a-z 0-9 . _ -`, the
first a letter or a digit, no `..`, no `.lock` at the end.

#### Scenario: A safe and an unsafe name
- **WHEN** a run id or a base name is checked
- **THEN** 1 to 128 characters of `A-Z a-z 0-9 . _ -` that start with a letter or a digit pass
- **AND** a name with `..` or `.lock` at the end is refused
  [@test] crates/host-link/src/proto_test.rs (`a_run_id_names_one_directory_and_one_ref`)
  [@test] crates/sync-utils/src/host_ref_name_test.rs (`a_host_ref_name_is_one_safe_component`)

### Requirement: The link: A refusal of the agent carries a code of the table

A refusal of the agent MUST be `{"reply": "refused", "code", "message"}` with a code of the table "Refusal codes". A
client that does not know the code MUST take it as `agent_error`, and the message names the code, so a code of a newer
build is no protocol error.

#### Scenario: A refusal of a known code
- **WHEN** the agent refuses a request
- **THEN** its reply is `{"reply": "refused", "code", "message"}` with a code of the table
  [@test] crates/host-link/src/refusal_test.rs (`a_refusal_prints_its_code_and_an_unknown_code_of_an_agent_is_an_agent_error`)

#### Scenario: A code of a newer build
- **WHEN** the reply names a code that the client does not know
- **THEN** the client takes it as `agent_error`, and the message names the code
  [@test] crates/host-link/src/refusal_test.rs (`a_refusal_prints_its_code_and_an_unknown_code_of_an_agent_is_an_agent_error`)

### Requirement: The link: A connection that fails has one of four codes

A connection that fails MUST be `unreachable`, `untrusted`, `timed_out` or `link`.

#### Scenario: No agent on the port, or a silent agent
- **WHEN** no agent listens on the port, or the agent does not answer
- **THEN** the connection is `unreachable`, or `timed_out`
  [@test] crates/host-link/src/channel_test.rs (`a_port_without_an_agent_is_unreachable_and_a_silent_agent_times_out`)

### Requirement: The link: Every read and every write has a deadline

Every read and every write MUST have a deadline. A request that waits MUST get `Reply::Heartbeat` every 15 s, and
the client resets its read deadline at each one.

#### Scenario: A request that waits
- **WHEN** a request waits on the agent, such as a follow whose thread is held in a file read or a `Have` while git
  upgrades
- **THEN** it gets `Reply::Heartbeat` every 15 s, and the client resets its read deadline at each one
  [@test] crates/host-agent/tests/serve/link.rs (`heartbeats_go_on_while_the_follow_thread_is_held_in_a_file_read`)
  [@test] crates/host-agent/tests/serve/lend.rs (`a_have_sends_heartbeats_while_it_waits_for_an_upgrade_of_git`)

### Requirement: The link: The handshakes of the agent have a deadline and a cap

A TLS handshake MUST have 3 s in all. The agent MUST take at most 128 handshakes at a time and 8 of one peer
address.

#### Scenario: A slow handshake and a socket over the cap
- **WHEN** a handshake takes longer than 3 s, or a socket comes over the cap
- **THEN** the handshake is cut at the deadline, and the socket over the cap closes at once
  [@test] crates/host-agent/tests/serve/link.rs (`a_slow_handshake_is_cut_at_the_deadline_and_a_socket_over_the_cap_closes_at_once`)

### Requirement: The link: A write of the client after the hello has the read deadline

After the hello, each write of the client MUST have the read deadline, so a large pack to a slow reader does not fail
at the short connect deadline.

#### Scenario: A large pack to a slow reader
- **WHEN** the client writes a large pack to an agent that reads it slowly
- **THEN** the pack does not fail at the connect deadline
  [@test] crates/host-client/src/run_test.rs (`a_slow_agent_reader_takes_a_large_pack`)

### Requirement: The link: A connection that Local Network privacy refuses goes through nc

On macOS a connection that Local Network privacy refuses (`EHOSTUNREACH`) MUST go again through `/usr/bin/nc`, which
the client starts responsible for itself. TLS stays end to end.

#### Scenario: A refused connection
- **WHEN** a connection on macOS fails with `EHOSTUNREACH`
- **THEN** it goes again through `/usr/bin/nc`, which the client starts responsible for itself
- **AND** the relay carries the handshake and the frames, so TLS stays end to end
  [@test] crates/host-link/src/channel_test.rs (`a_relayed_connection_carries_the_handshake_and_the_frames`)
  [@test] crates/host-link/src/relay_test.rs (`the_relay_carries_bytes_both_ways_and_ends_with_the_peer`, `local_network_privacy_is_a_cause_only_of_host_unreachable_on_a_local_address_that_the_relay_cannot_pass`)
  [@test] crates/host-link/src/disclaim_test.rs (`a_disclaimed_child_gets_its_arguments_and_its_pipes`, `a_command_that_changes_the_environment_is_refused`)

### Requirement: The link: The process remembers an address that takes the relay

The process MUST remember the address that Local Network privacy refused, so its next connection takes the relay at
once.

#### Scenario: A second connection to a refused address
- **GIVEN** a connection to an address that Local Network privacy refused
- **WHEN** the process connects to that address again
- **THEN** the connection takes the relay at once
  [@test] crates/host-link/src/channel_test.rs (`a_remembered_refusal_of_an_address_takes_the_relay_at_once`)

### Requirement: The bundle: brd-host is a binary of its own

`brd-host` MUST be a binary of its own: neither `brd` nor the app binary links a crate of the remote runs, so the app
binary does not grow (the architecture test, `crates/bd-cli/src/architecture_test.rs`).

#### Scenario: The crates of the binaries
- **WHEN** `brd` or the app binary is built
- **THEN** it links no crate of the remote runs

### Requirement: The bundle: The release build of the app on macOS carries brd-host

On macOS the release build of the app MUST carry it as `BranchDeck.app/Contents/MacOS/brd-host`: the Tauri config
`src-tauri/tauri.macos-bd-host.conf.json` names it as `externalBin`, and its build command
(`scripts/build-bd-host.mjs`) builds it for the target triple of the build. CI and `pnpm install-app` pass that
config with `--config`.

#### Scenario: A release build on macOS
- **WHEN** CI or `pnpm install-app` builds the app on macOS with `--config src-tauri/tauri.macos-bd-host.conf.json`
- **THEN** `scripts/build-bd-host.mjs` builds `brd-host` for the target triple of the build
- **AND** the bundle has it as `BranchDeck.app/Contents/MacOS/brd-host`

### Requirement: The bundle: The config of brd-host is not the macOS config of every build

The config that names `brd-host` MUST NOT be `tauri.macos.conf.json`, because Tauri merges that file into every macOS
build, `cargo build` of the app too, and that build then needs the binary first. The Windows build keeps its own
config for `brd.exe`.

#### Scenario: A build of the app without the config
- **WHEN** `cargo build` builds the app on macOS
- **THEN** it does not need `brd-host` first
- **AND** the Windows build keeps its own config for `brd.exe`

### Requirement: The bundle: brd install links brd-host to the binary of the bundle

`brd install` MUST link `~/.local/bin/brd-host` to that binary ([Agent CLI](../agent-cli/spec.md), "`brd install`").

#### Scenario: An install from the app bundle
- **WHEN** `brd install` runs from the app bundle
- **THEN** `~/.local/bin/brd-host` links to the `brd-host` of the bundle
  [@test] crates/bd-cli/src/install_test.rs (`install_links_the_brd_host_of_the_app_bundle_and_keeps_a_binary_there`)

### Requirement: The files: Each end keeps its files at the paths of the table

brd-host MUST keep the files of the client, of the agent and of a checkout at the paths of the table below, each with
the content that the table names.

#### Scenario: The files of each end
- **WHEN** brd-host writes a file of its own
- **THEN** the file is at its path of the table, with its content

| End | File | Content |
|---|---|---|
| Client | `~/.config/brd-host/identity.crt`, `identity.key` | The certificate and the key of the client, made at the first call, mode 0600 |
| Client | `~/.config/brd-host/hosts/<alias>.json` | `{"alias", "fingerprint", "port", "hostname", "addresses", "repo"}` |
| Client | `~/.config/brd-host/bases/<alias>/<affinity>` | The commit of the base of this affinity on that host after the last base sync |
| Agent | `~/.config/brd-host/agent/identity.crt`, `identity.key` | The certificate and the key of the agent |
| Agent | `~/.config/brd-host/agent/clients` | The allow list: one `<64 hex> <name>` line per client |
| Agent | `~/.cache/brd-host/runs/<run>/` | `spec.json`, `events.log`, `exit.json`, `supervisor.log`, `outputs/`, `changes.patch` |
| Agent | `~/.cache/brd-host/` | `repos`, `login-env.json`, `listening.json`, `upgrade.json`, `upgrade.lock`, `slots/<lease id>.prepared` |
| Agent | `~/Library/LaunchAgents/com.jetbrains.brd-host.plist`, `~/Library/Logs/brd-host.log` | The LaunchAgent and its log |
| Checkout | `<git dir>/branch-deck/host/runs/<run>.json`, `<run>.patch` | The run record and the patch of `--apply` |

### Requirement: The files: The directories and files of brd-host are private

Each directory that brd-host makes MUST have the mode 0700, and each file 0600. A removal and a rename MUST go
through `sync_utils::fs_retry`.

#### Scenario: A new directory and a new file
- **WHEN** brd-host makes a directory or a file
- **THEN** the directory has the mode 0700, and the file 0600

### Requirement: The files: The records and patches of old runs go

A record and a patch older than 7 days MUST go at the next run of the checkout.

#### Scenario: A record older than 7 days
- **GIVEN** a run record and a patch of the checkout older than 7 days
- **WHEN** the next run of the checkout starts
- **THEN** they go
  [@test] crates/host-client/src/record_test.rs (`a_run_record_names_what_a_fetch_takes_and_old_records_go`)

### Requirement: run: The command line of run

`brd-host run` MUST take the alias of a host, the options of its synopsis and, after `--`, the command with its
arguments. `--fetch` MUST be repeatable, and `--nice` MUST take 0 to 19.

#### Scenario: The synopsis of run
- **WHEN** a caller runs `brd-host run`
- **THEN** it takes this command line

```sh
brd-host run <alias> [--head] [--apply] [--fetch <dir>]... [--no-outputs] [--nice <0-19>] [--repo <path>]
            [--affinity <key>] [--report-fd <n>] -- <command> <args>
```

### Requirement: run: The working tree ships as one commit

`run` MUST ship the working tree: HEAD plus the tracked and untracked changes that git does not ignore, as one commit
(`git_ops::host_client::working_tree_commit`). The index, the files and the refs of the checkout MUST stay as they
are, and no ref names the commit.

#### Scenario: A run of a checkout with changes
- **GIVEN** a checkout with tracked and untracked changes
- **WHEN** `run` ships it
- **THEN** HEAD and the changes that git does not ignore go as one commit
- **AND** the index, the files and the refs of the checkout stay as they are, and no ref names the commit
  [@test] crates/host-client/src/run_test.rs (`the_working_tree_ships_as_one_commit_and_the_same_tree_sends_no_pack`)

### Requirement: run: The same tree on the same HEAD is the same commit

The author and the committer of that commit MUST be `brd-host <brd-host@brd-host.invalid>` at the date of the
committer of HEAD, and its message names HEAD. So the same tree on the same HEAD is the same commit, and a second run
of it sends no pack.

#### Scenario: A second run of the same tree
- **GIVEN** a run of a tree on a HEAD
- **WHEN** a second run ships the same tree on the same HEAD
- **THEN** it is the same commit, and the second run sends no pack
  [@test] crates/host-client/src/run_test.rs (`the_working_tree_ships_as_one_commit_and_the_same_tree_sends_no_pack`)

### Requirement: run: With --head a run ships the committed HEAD of a clean checkout

With `--head`, `run` MUST ship the committed HEAD and refuse a checkout with tracked or untracked changes
(`local_changes`, exit 75).

#### Scenario: --head on a checkout with local changes
- **WHEN** `run --head` starts in a checkout with tracked or untracked changes
- **THEN** it refuses with `local_changes`, exit 75
  [@test] crates/host-client/src/run_test.rs (`head_refuses_a_checkout_with_local_changes`)

### Requirement: run: The client asks the host which commits it has

The client MUST ask the host which commits it has (`Have`): HEAD and its first-parent ancestors at the distances 1,
2, 4 … 16 first, then up to 65536 only when the host has none of them, and the merge bases of HEAD with `master` and
`origin/master`.

#### Scenario: A host without a near commit
- **WHEN** the host has none of HEAD and its ancestors at the distances 1, 2, 4 … 16
- **THEN** the client asks again with the ancestors up to 65536 and the merge bases of HEAD with `master` and
  `origin/master`
  [@test] crates/host-client/src/run_test.rs (`a_host_without_a_near_commit_gets_the_far_candidates_then_refuses_behind`)

### Requirement: run: A host without a candidate commit refuses the run with behind

A host that has none of the candidate commits MUST refuse the run with `behind` (exit 75), and the message gives the
commit count of the history.

#### Scenario: A host that has none of the candidates
- **WHEN** the host has none of the far candidates either
- **THEN** it refuses the run with `behind`, exit 75, and the message gives the commit count of the history
  [@test] crates/host-client/src/run_test.rs (`a_host_without_a_near_commit_gets_the_far_candidates_then_refuses_behind`)

### Requirement: run: The steps of a run come in their order

The order of a run MUST be: the commit and `Have`; the run record; `Start`; `Follow` on a thread of its own; the pack
on a connection of its own, with an empty chunk every 15 s while it builds; the base sync; `Fetch` and `Forget`; with
`--apply`, the apply of the patch. Up to `Start`, nothing runs on the host.

#### Scenario: A run from its commit to its fetch
- **WHEN** a run goes through
- **THEN** the commit and `Have`, the run record, `Start`, `Follow`, the pack, the base sync, `Fetch` and `Forget`
  come in this order, and with `--apply` the apply of the patch comes last
  [@test] crates/host-client/src/run_test.rs (`a_run_starts_follows_packs_syncs_the_base_and_fetches_in_order`)

#### Scenario: A pack that takes long to build
- **WHEN** the pack builds for longer than 15 s
- **THEN** its connection gets an empty chunk every 15 s

### Requirement: run: The base syncs once per run after the pack

The base sync MUST send the local `origin/master`, else `master`, once per run after the pack. A failure of the base
sync MUST be only one warning line.

#### Scenario: The base of a run
- **WHEN** the pack of a run went
- **THEN** the base sync sends the local `origin/master`, else `master`, once
  [@test] crates/host-client/src/run_test.rs (`a_run_starts_follows_packs_syncs_the_base_and_fetches_in_order`)

#### Scenario: A failed base sync
- **WHEN** the base sync fails
- **THEN** it is one warning line, and the run goes on

### Requirement: run: A failed pack cancels the run

A failed pack MUST cancel the run, stop the follow and refuse with the code of the pack. The message says whether the
cancel came. A connection that breaks during the pack MUST refuse with `link` or `timed_out` (exit 75).

#### Scenario: The agent refuses the pack
- **WHEN** the pack fails
- **THEN** the client cancels the run, stops the follow and refuses with the code of the pack
- **AND** the message says whether the cancel came
  [@test] crates/host-client/src/run_test.rs (`a_failed_pack_cancels_the_run_and_refuses_with_its_code`)

#### Scenario: The link breaks during the pack
- **WHEN** the connection breaks during the pack
- **THEN** the client cancels the started run and refuses with `link` or `timed_out` (exit 75)
  [@test] crates/host-client/src/run_test.rs (`a_link_lost_during_the_pack_cancels_the_started_run_with_a_link_refusal`)

### Requirement: run: A run goes to the repository of the same name

The run MUST go to the repository of the same name as the local one beside the repository of the host file, else to
the one of the host file. `--repo` names another one.

#### Scenario: A repository of the same name beside the one of the host file
- **WHEN** the host has a repository of the same name as the local one beside the repository of the host file
- **THEN** the run goes to it, else to the one of the host file
  [@test] crates/host-client/src/run_test.rs (`the_repository_of_a_run_is_the_one_of_the_same_name_beside_the_host_file`)

#### Scenario: --repo
- **WHEN** `run` has `--repo <path>`
- **THEN** the run goes to that repository

### Requirement: run: The affinity of a run names its lend and its base

`--affinity` SHALL name the affinity of the lend, by default `checkout-<12 hex>` of the local checkout path. The base
of the pack MUST go to `base:<affinity>`.

#### Scenario: A run without --affinity
- **WHEN** `run` has no `--affinity`
- **THEN** the affinity of the lend is `checkout-<12 hex>` of the local checkout path
- **AND** the base of the pack goes to `base:<affinity>`

### Requirement: run: The niceness of a run is 10 by default

`--nice` SHALL be 10 by default: the niceness of the prepare steps and the command.

#### Scenario: A run without --nice
- **WHEN** `run` has no `--nice`
- **THEN** the prepare steps and the command run with the niceness 10

### Requirement: run: --no-outputs leaves the outputs of [host] alone

`--no-outputs` MUST leave the outputs of `[host]` alone.

#### Scenario: A run with --no-outputs
- **WHEN** `run` has `--no-outputs`
- **THEN** the outputs of `[host]` are left alone: none comes back, and no local copy is replaced

### Requirement: run: --fetch brings back the new entries of a directory

`--fetch <dir>` (repeatable) MUST bring back the new entries of `<dir>`, relative to the slot root.

#### Scenario: A run with --fetch
- **WHEN** `run` has one or more `--fetch <dir>`
- **THEN** the new entries of each `<dir>`, relative to the slot root, come back

### Requirement: run: Ctrl+C before the start runs nothing on the host

Ctrl+C before `Start` MUST run nothing on the host and exit 130 (`interrupted`).

#### Scenario: Ctrl+C before Start
- **WHEN** the user presses Ctrl+C before `Start`
- **THEN** nothing runs on the host, and `run` exits 130 (`interrupted`)
  [@test] crates/host-client/src/run_test.rs (`ctrl_c_before_the_start_reaches_no_host`)

### Requirement: run: Ctrl+C after the start cancels the run and follows it

Ctrl+C after `Start` MUST send `Cancel` and stop the pack, and the output follows to the end of the run. A `Cancel`
whose connection fails MUST connect again for up to 120 s.

#### Scenario: Ctrl+C after Start
- **WHEN** the user presses Ctrl+C after `Start`
- **THEN** the client sends `Cancel` and stops the pack
- **AND** the output follows to the end of the run
  [@test] crates/host-client/src/run_test.rs (`ctrl_c_cancels_the_run_and_follows_it_to_its_end`)

#### Scenario: A failed Cancel
- **WHEN** the connection of a `Cancel` fails
- **THEN** it connects again for up to 120 s
  [@test] crates/host-client/src/run_test.rs (`a_failed_cancel_connects_again`)

### Requirement: run: A second Ctrl+C stops the follow at once

A second press of Ctrl+C MUST stop the follow at once with 130, and the run goes on on the host.

#### Scenario: A second Ctrl+C
- **GIVEN** a run whose Ctrl+C sent `Cancel`
- **WHEN** the user presses Ctrl+C again
- **THEN** the follow stops at once with 130, and the run goes on on the host
  [@test] crates/host-client/src/run_test.rs (`a_second_ctrl_c_stops_the_follow_at_once`)

### Requirement: run: stdout holds only the stdout of the command

The stdout of `brd-host run` MUST hold only the stdout of the command, line by line as it comes. Every line of
brd-host and every stderr line of the host MUST go to the stderr.

#### Scenario: The streams of a run
- **WHEN** a run prints
- **THEN** stdout holds only the stdout of the command, line by line as it comes
- **AND** every line of brd-host and every stderr line of the host go to the stderr
  [@test] crates/bd-host/tests/cli.rs (`stdout_holds_only_the_command_and_the_lines_of_brd_host_go_to_stderr`)

### Requirement: run: The lines of brd-host name the run, the slot and the phases

The lines of brd-host MUST start with `brd-host: `: the line of the run first, the line of the slot right before the
command, and the line of the phases at the end.

#### Scenario: The lines of a run on its stderr
- **WHEN** a run goes through
- **THEN** `brd-host: run <id> of <commit> on <alias>` comes first
- **AND** `brd-host: running in <slot> (slot <n>): load …, nice …, … open files, git <version>, the login environment of <shell> from <age> ago` comes right before the command
- **AND** `brd-host: phases: pack 3.1s, upload 2.0s, slot 0.1s, … total 10m13s` comes at the end
  [@test] crates/bd-host/tests/cli.rs (`stdout_holds_only_the_command_and_the_lines_of_brd_host_go_to_stderr`)

### Requirement: run: A lost follow connects again from its offset

A follow that lost its connection MUST connect again with waits from 1 s up to 30 s and go on from the last offset it
showed.

#### Scenario: A follow that lost its connection
- **WHEN** the connection of a follow breaks
- **THEN** it connects again with waits from 1 s up to 30 s
- **AND** it goes on from the last offset it showed
  [@test] crates/host-client/src/run_test.rs (`a_lost_follow_connects_again_from_its_offset_and_an_unknown_run_is_255`)
  [@test] crates/host-agent/tests/serve/follow.rs (`a_follow_from_an_offset_in_the_middle_of_a_run_goes_on_from_there`)

### Requirement: run: A follow without a reply for 10 min times out with 255

After 10 min without a reply a follow MUST refuse with `timed_out` (exit 255) and name `brd-host follow`, also when
each new connection was `unreachable`.

#### Scenario: An agent that ends during the follow
- **WHEN** the follow gets no reply for 10 min, also when each new connection was `unreachable`
- **THEN** it refuses with `timed_out`, exit 255 and not 75, and names `brd-host follow`
  [@test] crates/host-client/src/run_test.rs (`an_agent_that_ends_during_the_follow_times_out_with_255_and_not_75`)

### Requirement: run: An unknown run after the start exits 255

A follow that the agent answers with `unknown_run` after the start MUST exit 255, never 75: whether the command ran is
unknown.

#### Scenario: The agent no longer keeps the run
- **WHEN** the agent answers a follow after the start with `unknown_run`
- **THEN** the follow exits 255, never 75
  [@test] crates/host-client/src/run_test.rs (`a_lost_follow_connects_again_from_its_offset_and_an_unknown_run_is_255`)

### Requirement: run: A refusal of the client after the start never exits 75

Any other refusal of the client after the start, before it saw the end of the run, MUST exit 255. A code whose exit is
always 75, such as `untrusted` from a new connection, becomes `link` and keeps its name in the message. Only a second
Ctrl+C keeps `interrupted` (exit 130).

#### Scenario: A refusal after the start
- **WHEN** the client refuses after the start, before it saw the end of the run
- **THEN** it exits 255
  [@test] crates/host-client/src/run_test.rs (`a_refusal_of_the_client_after_the_start_never_exits_75`)

#### Scenario: A code whose exit is always 75
- **WHEN** a new connection after the start refuses with a code whose exit is always 75, such as `untrusted`
- **THEN** the refusal becomes `link`, exit 255, and the message keeps the name of the code
  [@test] crates/host-client/src/run_test.rs (`a_refusal_of_the_client_after_the_start_never_exits_75`)

#### Scenario: A second Ctrl+C after the start
- **WHEN** a second Ctrl+C stops the follow after the start
- **THEN** the refusal stays `interrupted` (exit 130)

### Requirement: The fetch, the outputs and the changes: A fetch takes only what the run record asks for

A fetch MUST take only what the run record asks for: the staged entries below a `--fetch` directory, the files of an
output of `[host]` that the record names, and the patch of a record with `changes`.

#### Scenario: The files of a fetch
- **WHEN** the client fetches the files of a run
- **THEN** it takes the staged entries below a `--fetch` directory, the files of an output of `[host]` that the record
  names, and the patch of a record with `changes`, and nothing else
  [@test] crates/host-client/src/fetch_test.rs (`a_fetch_takes_only_what_the_record_names_and_never_writes_through_a_link`)

### Requirement: The fetch, the outputs and the changes: A path outside the fetch is skipped

A path that is absolute, has `..` or `\`, is outside those, or is inside a `.git` directory in any case (`.GIT`,
`.git.`, `git~1`) MUST be skipped, and the fetch then fails with `fetch_failed` (exit 255).

#### Scenario: A path that the fetch must not write
- **WHEN** the agent names a path that is absolute, has `..` or `\`, is outside what the record asks for, or is inside
  a `.git` directory in any case (`.GIT`, `.git.`, `git~1`)
- **THEN** the fetch skips it and then fails with `fetch_failed` (exit 255)
  [@test] crates/host-client/src/fetch_test.rs (`a_fetch_takes_only_what_the_record_names_and_never_writes_through_a_link`)

### Requirement: The fetch, the outputs and the changes: A write of a fetch follows no link and is whole

A write of a fetch MUST NOT follow a local link. Each file MUST go to `<name>.bd-host-part` first and take its name only
at the size that the agent named.

#### Scenario: A local link on the path of a file
- **WHEN** a path of the fetch goes through a local link
- **THEN** no write follows the link
  [@test] crates/host-client/src/fetch_test.rs (`a_fetch_takes_only_what_the_record_names_and_never_writes_through_a_link`)

#### Scenario: A file that comes
- **WHEN** a file of the fetch comes
- **THEN** it goes to `<name>.bd-host-part` first, and takes its name only at the size that the agent named

### Requirement: The fetch, the outputs and the changes: The run forgets itself after every file came

The run MUST forget itself on the host only when the client saw its end and every file came.

#### Scenario: A fetch that did not take every file
- **WHEN** the client did not see the end of the run, or a file did not come
- **THEN** the run stays on the host

### Requirement: The fetch, the outputs and the changes: The first file of an output replaces its local copy

The first file of an output MUST remove the local file or directory of that output. An output that the run did not
make brings nothing, so the local copy of an earlier run stays.

#### Scenario: An output that the run made
- **WHEN** the first file of an output comes
- **THEN** the local file or directory of that output is removed first
  [@test] crates/host-client/src/fetch_test.rs (`the_first_file_of_an_output_replaces_its_local_copy`)

#### Scenario: An output that the run did not make
- **WHEN** the run did not make an output
- **THEN** it brings nothing, and the local copy of an earlier run stays

### Requirement: The fetch, the outputs and the changes: --apply applies the changes all or nothing

With `--apply`, the patch of the changes that the command made MUST go to `<git dir>/branch-deck/host/runs/<run>.patch`
and apply to the files of the working tree, not to the index, all or nothing (`git_ops::host_client::apply_patch`). A
patch that does not apply MUST refuse with `apply_failed` (exit 255), name the patch, and leave the working tree as it
was.

#### Scenario: A patch that applies
- **WHEN** a run with `--apply` brings back the changes of the command
- **THEN** the patch goes to `<git dir>/branch-deck/host/runs/<run>.patch` and applies to the files of the working
  tree, not to the index
  [@test] crates/host-client/src/run_test.rs (`apply_brings_the_changes_back_all_or_nothing`)

#### Scenario: A patch that does not apply
- **WHEN** the patch does not apply
- **THEN** `run` refuses with `apply_failed` (exit 255) and names the patch
- **AND** the working tree stays as it was
  [@test] crates/host-client/src/run_test.rs (`apply_brings_the_changes_back_all_or_nothing`)

### Requirement: The report of a run: The report goes to its descriptor on every way out

With `--report-fd <n>`, `run` MUST write one JSON object and a line end to the descriptor `n` on every way out after
the parse of its command line: a run that exited, a refusal and an interrupt. It MUST write nothing else there, and it
closes the descriptor after the object.

#### Scenario: Every way out of a run
- **WHEN** a run with `--report-fd <n>` exits, refuses or is interrupted after the parse of its command line
- **THEN** the descriptor `n` gets one JSON object and a line end, and nothing else
- **AND** the descriptor is closed after the object
  [@test] crates/bd-host/tests/cli.rs (`the_report_goes_to_its_descriptor_on_every_way_out`)

### Requirement: The report of a run: A descriptor that is not open refuses with usage

A descriptor of `--report-fd` that is not open MUST refuse with `usage` (exit 2) before anything runs.

#### Scenario: A closed descriptor
- **WHEN** `run --report-fd <n>` starts and the descriptor `n` is not open
- **THEN** it refuses with `usage` (exit 2), and nothing runs
  [@test] crates/bd-host/tests/cli.rs (`the_report_goes_to_its_descriptor_on_every_way_out`)

### Requirement: The report of a run: A machine caller names its run options in full

A machine caller such as bt MUST name its run options in full, because the defaults of `run` are the ones of a
person: `brd-host run <alias> --head --nice 0 --no-outputs --fetch out/bt-runs --report-fd 3 -- <command…>`, with `--repo <path>` when it names a repository.

#### Scenario: The command line of bt
- **WHEN** bt runs a command on a host
- **THEN** it passes `--head --nice 0 --no-outputs --fetch out/bt-runs --report-fd 3`, and `--repo <path>` when it
  names a repository
- **AND** niceness 0 keeps the CPU for the run
- **AND** without `--no-outputs`, an output of `[host]` would replace its local copy
  [@test] crates/bd-host/src/cli_test.rs (`the_command_line_of_bt_gives_niceness_0_and_no_outputs`)

### Requirement: The report of a run: A caller reads the exit of the command from the report

A machine caller MUST read `command_exit` of the report, because a failed fetch exits 255 after the command ran.

#### Scenario: A failed fetch after the command
- **WHEN** the command ran and the fetch failed
- **THEN** `brd-host` exits 255, and `command_exit` of the report is the exit code of the command

### Requirement: The report of a run: The report has the shape of the golden JSON

The report MUST have the shape of the golden JSON below.

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

#### Scenario: The report of a run that exited
- **WHEN** a run exits
- **THEN** its report has the shape of the golden JSON
  [@test] crates/host-client/src/report_test.rs (`the_report_of_a_run_is_the_golden_json_of_the_spec`)

### Requirement: The report of a run: The exits of the report

`exit` MUST be the exit code of `brd-host`. `command_exit` MUST be the exit code of the command, `null` when it did
not run or its end is unknown; a command that a signal ended answers 128 plus the signal number.

#### Scenario: A command that ran to its end
- **WHEN** the command exited
- **THEN** `exit` is the exit code of `brd-host`, and `command_exit` the exit code of the command
  [@test] crates/host-client/src/report_test.rs (`the_report_of_a_run_is_the_golden_json_of_the_spec`)

#### Scenario: A command that did not run or whose end is unknown
- **WHEN** the command did not run, or its end is unknown
- **THEN** `command_exit` is `null`

#### Scenario: A command that a signal ended
- **WHEN** a signal ended the command
- **THEN** `command_exit` is 128 plus the signal number

### Requirement: The report of a run: The refusal and the ids of the report

`refusal` MUST be `null`, or `{"code", "message"}` with a code of the table below. `run`, `commit` and `head` MUST be
`null` before the client knows them.

#### Scenario: A refusal before the client knew the run
- **WHEN** a run refuses before the client knows all of its run, commit and head
- **THEN** `refusal` is `{"code", "message"}` with a code of the table
- **AND** each of `run`, `commit` and `head` that the client did not know is `null`

### Requirement: The report of a run: The phases, the fetched files and the applied files

`phases` MUST list the steps in their order, each with its milliseconds. `fetched` MUST list the files that came back,
relative to the checkout root with `/`. `applied_files` MUST be the count of the files that `--apply` changed, `null`
without `--apply`.

#### Scenario: The lists of a report
- **WHEN** a run exits
- **THEN** `phases` lists its steps in their order with their milliseconds, and `fetched` the files that came back,
  relative to the checkout root with `/`
- **AND** `applied_files` is the count of the files that `--apply` changed, `null` without `--apply`
  [@test] crates/host-client/src/report_test.rs (`the_report_of_a_run_is_the_golden_json_of_the_spec`)

### Requirement: The report of a run: The phases are the steps of both ends in their order

The steps MUST be `pack` and `upload` of the client; `git`, `slot`, `pack_wait`, `checkout`, `prepare`, `command` and
`stage` of the host; `fetch` and `total` of the client. A run that ends early has only the steps up to its last one.
Only a run that waited for an upgrade of git has `git`.

#### Scenario: The steps of a run
- **WHEN** a run goes through
- **THEN** its phases are `pack`, `upload`, `slot`, `pack_wait`, `checkout`, `prepare`, `command`, `stage`, `fetch`
  and `total`, with `git` only when it waited for an upgrade of git
  [@test] crates/host-client/src/phases_test.rs (`the_phases_are_the_steps_of_both_ends_in_their_order`)

#### Scenario: A run that ends early
- **WHEN** a run ends before its last step
- **THEN** its phases have only the steps up to its last one
  [@test] crates/host-client/src/phases_test.rs (`the_phases_are_the_steps_of_both_ends_in_their_order`)

### Requirement: follow, cancel, fetch, forget, runs: follow shows a run from its first line

`brd-host follow <alias> <run>` MUST follow the output of a run from its first line, and exit with the exit code of
the command at its end. Ctrl+C MUST leave the follow, and the run goes on.

#### Scenario: A follow of a run
- **WHEN** `brd-host follow <alias> <run>` runs
- **THEN** it shows the output of the run from its first line
- **AND** it exits with the exit code of the command at the end of the run

#### Scenario: Ctrl+C during a follow
- **WHEN** the user presses Ctrl+C during `brd-host follow`
- **THEN** the follow leaves, and the run goes on on the host

### Requirement: follow, cancel, fetch, forget, runs: cancel interrupts a run on the host

`brd-host cancel <alias> <run>` MUST ask the agent to interrupt the run: SIGINT to the process group of the command,
SIGKILL after 120 s, or no command when it has not started. It MUST return when the agent has the request.

#### Scenario: A cancel of a running command
- **WHEN** `brd-host cancel <alias> <run>` runs while the command runs
- **THEN** the agent sends SIGINT to the process group of the command
- **AND** `cancel` returns when the agent has the request
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

#### Scenario: A command that outlives SIGINT
- **WHEN** the command still runs 120 s after the SIGINT of a cancel
- **THEN** the agent sends SIGKILL to its process group

#### Scenario: A cancel before the command started
- **WHEN** `brd-host cancel <alias> <run>` runs before the command started
- **THEN** no command starts

### Requirement: follow, cancel, fetch, forget, runs: fetch copies the files of a failed fetch

`brd-host fetch <alias> <run> [--fetch <dir>]...` MUST copy the files of a run whose fetch failed, by the run record
of the checkout of the working directory. An explicit `--fetch` MUST replace the directories of the record.

#### Scenario: A fetch by the run record
- **WHEN** `brd-host fetch <alias> <run>` runs in the checkout of the run
- **THEN** it copies the files of the run that its run record names

#### Scenario: A fetch with --fetch
- **WHEN** `brd-host fetch` has `--fetch <dir>`
- **THEN** those directories replace the directories of the record

#### Scenario: A fetch without a run record
- **WHEN** the checkout has no run record of the run
- **THEN** only the `--fetch` directories come, and without `--fetch` the fetch refuses with `usage`

### Requirement: follow, cancel, fetch, forget, runs: forget removes a run, and an ended run stays 24 h

`brd-host forget <alias> <run>` MUST remove a run and its files on the host. Without it, the host MUST keep an ended
run for 24 h.

#### Scenario: A forget of an ended run
- **WHEN** `brd-host forget <alias> <run>` runs for an ended run
- **THEN** the run and its files are gone from the host

#### Scenario: An ended run that nobody forgets
- **WHEN** a run ended and nobody forgot it
- **THEN** the host keeps it for 24 h

### Requirement: follow, cancel, fetch, forget, runs: runs lists the runs that the agent keeps

`brd-host runs <alias> [--json]` MUST list the runs that the agent keeps, each with its affinity, commit, command,
start, exit and the steps of 0.1 s or more.

#### Scenario: The runs of a host
- **WHEN** `brd-host runs <alias>` runs while a run goes on
- **THEN** it lists that run with its affinity, commit, command, start and exit, and the steps of 0.1 s or more
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

### Requirement: follow, cancel, fetch, forget, runs: A fetch or a forget of a running run refuses

A fetch or a forget of a run that goes on MUST refuse with `bad_request`. A run id that the agent does not keep MUST
refuse with `unknown_run`.

#### Scenario: A run that goes on
- **WHEN** a caller fetches or forgets a run that goes on
- **THEN** the command refuses with `bad_request`
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

#### Scenario: A run that the agent does not keep
- **WHEN** a command names a run id that the agent does not keep
- **THEN** it refuses with `unknown_run`
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

### Requirement: follow, cancel, fetch, forget, runs: The exit codes of these commands

Each of these commands MUST exit 0, or 255 with a refusal, or 2 for a usage error.

#### Scenario: The exits of a command
- **WHEN** one of these commands ends
- **THEN** it exits 0, or 255 with a refusal, or 2 for a usage error
  [@test] crates/bd-host/tests/cli.rs (`follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent`)

### Requirement: hello: hello prints the answer of the agent

`brd-host hello <alias> [--json]` MUST connect, send the hello and print the answer of the agent. With `--json` it
MUST print one JSON object of the shape of the golden JSON below on stdout.

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
    "features": ["upgrade", "disk", "trim"]
  },
  "client": {
    "version": "0.37.0",
    "build": "5e1f0c3a9b7d4e2f8a6c1b3d5e7f9a0b2c4d6e8f1a3b5c7d9e0f2a4b6c8d0e1f",
    "schema": 1
  },
  "same_build": true
}
```

#### Scenario: hello --json
- **WHEN** `brd-host hello <alias> --json` gets the answer of the agent
- **THEN** it prints one JSON object of the shape of the golden JSON on stdout
  [@test] crates/host-client/src/report_test.rs (`the_hello_is_the_golden_json_of_the_spec`)

### Requirement: hello: The hello names the build and the version of each end

`build` MUST be the SHA-256 of the binary, which names a build. `version` MUST be the version of Branch Deck.
`same_build` MUST tell whether both ends run the same build.

#### Scenario: Both ends of one build
- **WHEN** the client and the agent run the same binary
- **THEN** both `build` are its SHA-256, and `same_build` is `true`
  [@test] crates/host-client/src/report_test.rs (`the_hello_is_the_golden_json_of_the_spec`)

### Requirement: hello: A refusal of hello --json has its own shape

A refusal MUST print `{"schema": 1, "alias", "refusal": {"code", "message"}}` with `--json` and exit 255.

#### Scenario: A hello that the host refuses
- **WHEN** `brd-host hello <alias> --json` gets no answer of the agent, or a refusal
- **THEN** it prints `{"schema": 1, "alias", "refusal": {"code", "message"}}` and exits 255
  [@test] crates/host-client/src/report_test.rs (`the_hello_is_the_golden_json_of_the_spec`)

### Requirement: hello: A hello of another schema refuses

A hello of another schema MUST refuse with `schema` (exit 75 for a run).

#### Scenario: An agent of another schema
- **WHEN** the agent answers the hello with another schema
- **THEN** the client refuses with `schema`, exit 75 for a run
  [@test] crates/host-client/src/host_test.rs (`another_schema_refuses_and_another_build_runs_with_a_hint`)

### Requirement: hello: A hello of another build runs with a hint

A hello of the same schema and another build MUST let the run go on, with one line that names
`brd-host upgrade <alias>`.

#### Scenario: An agent of another build
- **WHEN** the agent answers the hello with the same schema and another build
- **THEN** the run goes on, with one line that names `brd-host upgrade <alias>`
  [@test] crates/host-client/src/host_test.rs (`another_schema_refuses_and_another_build_runs_with_a_hint`)

### Requirement: slots: The command line of slots

`brd-host slots` MUST take either the alias of a host or `--local`. `-C <repo>`, `--shrink <n>` and `--sizes`
MUST come only with `--local`, and `--json` with either.

#### Scenario: The synopsis of slots
- **WHEN** a caller runs `brd-host slots`
- **THEN** it takes one of these command lines

```sh
brd-host slots <alias> [--json]
brd-host slots --local [-C <repo>] [--shrink <n>] [--sizes] [--json]
```

### Requirement: slots: slots asks the agent for its lend tables

`brd-host slots <alias>` MUST ask the agent for the lend table of each repository that it served since its first
`Start` (`Request::Slots`): each slot, its state, its run, its affinity, its last use, and the queue. It MUST write
nothing.

#### Scenario: The slots of a host
- **WHEN** `brd-host slots <alias>` runs
- **THEN** it shows, for each repository that the agent served since its first `Start`, each slot with its state, its
  run, its affinity and its last use, and the queue
- **AND** nothing is written
  [@test] crates/host-link/src/proto_test.rs (`the_slots_of_the_agent_carry_the_lend_table_as_the_pool_writes_it`)

### Requirement: slots: The reply carries each table as the pool writes it

The reply MUST carry each table as the JSON of its `host-slots.json`, so the protocol keeps no second shape of the
table.

#### Scenario: The reply of Request::Slots
- **WHEN** the agent answers `Request::Slots`
- **THEN** each table is the JSON of its `host-slots.json`
  [@test] crates/host-link/src/proto_test.rs (`the_slots_of_the_agent_carry_the_lend_table_as_the_pool_writes_it`)

### Requirement: slots: --local reads the table without the agent

`--local` MUST read the table of the repository of `-C` on this machine, without the agent.

#### Scenario: The table of this machine
- **WHEN** `brd-host slots --local -C <repo>` runs
- **THEN** it shows the lend table of that repository on this machine, without the agent
  [@test] crates/bd-host/tests/cli.rs (`slots_shows_the_lend_table_and_shrinks_it_without_the_agent`)
  [@test] crates/host-agent/src/table_test.rs (`the_lines_of_a_table_name_the_limit_the_queue_and_each_slot`)

### Requirement: slots: --shrink gives the slots past a limit back in its own process

`--shrink <n>` MUST give the host slots past `n` back to the pool in its own process
(`lease_pool::host_slots::shrink`), also without a running `serve`, and keep `n` as the limit of later fills.

#### Scenario: A shrink without the agent
- **WHEN** `brd-host slots --local --shrink <n>` runs, also without a running `serve`
- **THEN** the host slots past `n` go back to the pool
- **AND** `n` stays the limit of later fills
  [@test] crates/bd-host/tests/cli.rs (`slots_shows_the_lend_table_and_shrinks_it_without_the_agent`)

### Requirement: slots: --sizes measures the directories of the clean rules

`--sizes` MUST measure the directories that the clean rules of each slot name.

#### Scenario: The sizes of the slots
- **WHEN** `brd-host slots --local --sizes` runs
- **THEN** each slot shows the sizes of the directories that its clean rules name
  [@test] crates/host-agent/src/table_test.rs (`the_sizes_measure_the_directories_of_the_clean_rules_of_each_slot`)

### Requirement: disk: The command line of disk

`brd-host disk` MUST take an optional alias of a host, `--fresh` and `--json`.

#### Scenario: The synopsis of disk
- **WHEN** a caller runs `brd-host disk`
- **THEN** it takes this command line

```sh
brd-host disk [<alias>] [--fresh] [--json]
```

### Requirement: disk: disk asks the agent what the pools of its host hold

`brd-host disk <alias>` MUST ask the agent what the lease pools of its host hold on disk (`Request::Disk`, feature
`disk`, every agent of this build has it): the agent answers the report of `brd lease disk` for its home
(`pool_disk`, by the first repository of its list that opens), and heartbeats while it walks.

#### Scenario: The disk of a host
- **WHEN** `brd-host disk <alias>` runs against an agent of this build
- **THEN** the agent answers the report of `brd lease disk` for its home, and heartbeats while it walks
  [@test] crates/host-link/src/proto_test.rs (`a_request_and_a_reply_have_a_tagged_json_shape`, `an_agent_on_macos_has_the_upgrade_and_every_agent_the_disk_and_the_trim`)
  [@test] crates/host-client/src/run_test.rs (`runs_cancel_slots_disk_and_trim_answer_the_replies_of_the_agent`)

### Requirement: disk: The reply carries the disk report as JSON

The reply MUST carry the report as JSON (`pool_disk::report::DiskReport`), so the protocol keeps no second shape of
it.

#### Scenario: The reply of Request::Disk
- **WHEN** the agent answers `Request::Disk`
- **THEN** the reply is the JSON of `pool_disk::report::DiskReport`
  [@test] crates/host-link/src/proto_test.rs (`a_request_and_a_reply_have_a_tagged_json_shape`)

### Requirement: disk: --fresh walks every slot again

`--fresh` MUST walk every slot again.

#### Scenario: disk --fresh
- **WHEN** `brd-host disk <alias> --fresh` runs
- **THEN** the agent walks every slot again

### Requirement: disk: Without an alias disk asks every host

Without an alias `disk` MUST ask every host of the client (`hosts/<alias>.json`), one after the other.

#### Scenario: disk without an alias
- **WHEN** `brd-host disk` runs without an alias
- **THEN** it asks every host of `hosts/<alias>.json`, one after the other

### Requirement: disk: An agent without the feature gets no request

A client MUST check the feature in the hello first: an agent without it gets no request (it would close the
connection on one it does not know) and its host names `agent_failed` with `brd-host upgrade <alias>`.

#### Scenario: An agent of an older build
- **WHEN** the hello of an agent has no feature `disk`
- **THEN** the client sends it no request, and the host names `agent_failed` with `brd-host upgrade <alias>`
  [@test] crates/host-client/src/run_test.rs (`disk_and_trim_of_an_agent_without_the_feature_send_no_request`)

### Requirement: disk: disk prints the reports of the hosts

`--json` MUST print `{"schema", "hosts": [{"alias", "report"} or {"alias", "refusal": {"code", "message"}}]}`;
without it each host MUST be its lines: the free space and the minimum, each pool with its bytes, its slots, its limit
and the free slots past it, the output bases of the main worktree and of gone slots.

#### Scenario: disk --json
- **WHEN** `brd-host disk --json` runs
- **THEN** it prints `{"schema", "hosts": [...]}` with the report or the refusal of each host
  [@test] crates/bd-host/tests/cli.rs (`disk_reports_the_pools_of_the_host`)

#### Scenario: disk without --json
- **WHEN** `brd-host disk` runs without `--json`
- **THEN** each host is its lines: the free space and the minimum, each pool with its bytes, its slots, its limit and
  the free slots past it, the output bases of the main worktree and of gone slots
  [@test] crates/bd-host/tests/cli.rs (`disk_reports_the_pools_of_the_host`)

### Requirement: disk: A host that answers nothing does not stop the others

A host that answers nothing MUST be named and the others go on; the command fails with its refusal only when no host
answered.

#### Scenario: One host of several answers nothing
- **WHEN** one host answers nothing and another answers
- **THEN** the silent host is named, and the others go on

#### Scenario: No host answers
- **WHEN** no host answers
- **THEN** the command fails with its refusal

### Requirement: trim: The command line of trim

`brd-host trim` MUST take an optional alias of a host, `--max <n>`, `--plan` and `--json`.

#### Scenario: The synopsis of trim
- **WHEN** a caller runs `brd-host trim`
- **THEN** it takes this command line

```sh
brd-host trim [<alias>] [--max <n>] [--plan] [--json]
```

### Requirement: trim: trim asks the agent to trim every pool of its host

`brd-host trim <alias>` MUST ask the agent to trim every pool of its host (`Request::Trim`, feature `trim`, every
agent of this build has it), as `brd lease trim` trims the pools of this machine ([brd lease](../lease/spec.md#trim)).

#### Scenario: A trim of a host
- **WHEN** `brd-host trim <alias>` runs against an agent of this build
- **THEN** the agent trims every pool of its host, as `brd lease trim` trims the pools of this machine
  [@test] crates/host-client/src/run_test.rs (`runs_cancel_slots_disk_and_trim_answer_the_replies_of_the_agent`)

### Requirement: trim: A trim deletes the free slots past the limit

A trim MUST delete the free slots past `--max`, else `branchdeck.leaseMaxSlots` of each repository there, the ones
returned longest ago first, with their Bazel output bases, and the output bases of gone workspaces
(`lease_pool::host_slots::trim`). A host slot is a lease and stays.

#### Scenario: The free slots past the limit
- **WHEN** a pool of the host has more free slots than `--max`, else `branchdeck.leaseMaxSlots` of its repository
- **THEN** the trim deletes the ones past it, returned longest ago first, with their Bazel output bases
- **AND** it deletes the output bases of gone workspaces
- **AND** a host slot stays
  [@test] crates/bd-host/tests/cli.rs (`trim_plans_and_deletes_the_free_slots_of_the_host`)

### Requirement: trim: --plan names what would go and deletes nothing

`--plan` MUST name what would go and delete nothing.

#### Scenario: trim --plan
- **WHEN** `brd-host trim <alias> --plan` runs
- **THEN** it names what would go, and nothing is deleted
  [@test] crates/bd-host/tests/cli.rs (`trim_plans_and_deletes_the_free_slots_of_the_host`)

### Requirement: trim: The agent heartbeats, refuses during a git upgrade and logs a trim

The agent MUST heartbeat while it deletes, refuse while a live process upgrades git (`GitUpgrading`), and write one
line of its log with the peer and what went.

#### Scenario: A trim that deletes
- **WHEN** the agent deletes the slots of a trim
- **THEN** it heartbeats while it deletes, and writes one line of its log with the peer and what went

#### Scenario: A trim while git upgrades
- **WHEN** a live process upgrades git on the host
- **THEN** the agent refuses the trim (`GitUpgrading`)

### Requirement: trim: The reply carries the trim report as JSON

The reply MUST carry `pool_disk::trim_report::TrimReport` as JSON.

#### Scenario: The reply of Request::Trim
- **WHEN** the agent answers `Request::Trim`
- **THEN** the reply is the JSON of `pool_disk::trim_report::TrimReport`
  [@test] crates/host-client/src/run_test.rs (`runs_cancel_slots_disk_and_trim_answer_the_replies_of_the_agent`)

### Requirement: trim: Without an alias trim asks every host

Without an alias `trim` MUST ask every host of the client, one after the other; an agent without the feature gets no
request and its host names `agent_failed` with `brd-host upgrade <alias>`, as for `disk`.

#### Scenario: trim without an alias
- **WHEN** `brd-host trim` runs without an alias
- **THEN** it asks every host of the client, one after the other

#### Scenario: An agent without the feature trim
- **WHEN** the hello of an agent has no feature `trim`
- **THEN** the client sends it no request, and the host names `agent_failed` with `brd-host upgrade <alias>`
  [@test] crates/host-client/src/run_test.rs (`disk_and_trim_of_an_agent_without_the_feature_send_no_request`)

### Requirement: trim: trim prints the reports of the hosts

`--json` MUST print `{"schema", "hosts": [{"alias", "report"} or {"alias", "refusal": {"code", "message"}}]}`, the
shape of `disk`; without it each host MUST be its lines: what went (with `--plan`: what would go) and the free space
after, then each pool it touched with its slots, its limit, each deleted slot with its bytes and each busy one.

#### Scenario: trim without --json
- **WHEN** `brd-host trim` runs without `--json`
- **THEN** each host is its lines: what went (with `--plan`: what would go) and the free space after
- **AND** each pool it touched with its slots, its limit, each deleted slot with its bytes and each busy one
  [@test] crates/pool-disk/src/trim_report_test.rs (`a_trim_report_counts_each_slot_with_its_output_bases_and_names_only_the_pools_it_touched`)

#### Scenario: trim --json
- **WHEN** `brd-host trim --json` runs
- **THEN** it prints the shape of `disk`, with the report or the refusal of each host

### Requirement: setup: The command line of setup

`brd-host setup` MUST take either the alias of a host, with `--repo <path>`, `--port`, `--name <name>` and
`--binary <path>`, or `--agent` with `--client <fingerprint>`, `--name <name>` and `--port`. `--port` MUST be
7742 by default.

#### Scenario: The synopsis of setup
- **WHEN** a caller runs `brd-host setup`
- **THEN** it takes one of these command lines

```sh
brd-host setup <alias> [--repo <path>] [--port 7742] [--name <name>] [--binary <path>]
brd-host setup --agent --client <fingerprint> --name <name> [--port 7742]
```

### Requirement: setup: setup prepares a host over ssh

`brd-host setup <alias>` MUST prepare a host over ssh, run by the user only: copy `--binary`, else this executable, to
`~/.local/bin/brd-host` of the host when the host has another build there; run `~/.local/bin/brd-host setup --agent --client <the fingerprint of this client> --name <name>` there; and write the host file from its answer.

#### Scenario: A setup of a host
- **WHEN** the user runs `brd-host setup <alias>`
- **THEN** it copies `--binary`, else this executable, to `~/.local/bin/brd-host` of the host when the host has another
  build there
- **AND** it runs the agent setup there and writes the host file from its answer
  [@test] crates/host-client/src/setup_test.rs (`setup_copies_the_binary_runs_the_agent_setup_and_writes_the_host_file`)

### Requirement: setup: Each ssh call of setup has a deadline

Each ssh call of `setup` MUST have 30 s, the copy 120 s (`ssh_failed`).

#### Scenario: An ssh call past its deadline
- **WHEN** an ssh call runs past 30 s, or the copy past 120 s
- **THEN** the setup refuses with `ssh_failed`
  [@test] crates/host-client/src/setup_test.rs (`setup_copies_the_binary_runs_the_agent_setup_and_writes_the_host_file`)

### Requirement: setup: The agent setup writes the identity, the allow list and the LaunchAgent

`brd-host setup --agent` MUST make the identity of the agent when it has none, add the client to the allow list, write
the LaunchAgent `com.jetbrains.brd-host` (`KeepAlive`, `AbandonProcessGroup`, `~/.local/bin/brd-host serve --port <port>`),
and bootstrap or restart it. It MUST print one JSON line
`{"fingerprint", "port", "launchd": "bootstrapped" | "restarted" | "none"}`.

#### Scenario: The setup of the agent
- **WHEN** `brd-host setup --agent --client <fingerprint> --name <name>` runs on the host
- **THEN** the agent has its identity, the allow list names the client, and the LaunchAgent `com.jetbrains.brd-host`
  is written and bootstrapped or restarted
- **AND** it prints one JSON line of the fingerprint, the port and what launchd did
  [@test] crates/host-agent/src/setup_test.rs (`the_agent_setup_writes_the_identity_the_allow_list_and_the_launch_agent`)

### Requirement: setup: The agent setup refuses a bad name and a failed launchctl

A client name with a control character MUST refuse with `bad_request`, and a failed `launchctl` with
`launchd_failed`.

#### Scenario: A client name with a control character
- **WHEN** the name of the client has a control character
- **THEN** the agent setup refuses with `bad_request`
  [@test] crates/host-agent/src/setup_test.rs (`the_agent_setup_writes_the_identity_the_allow_list_and_the_launch_agent`)

#### Scenario: A failed launchctl
- **WHEN** a `launchctl` call of the agent setup fails
- **THEN** it refuses with `launchd_failed`

### Requirement: setup: The agent setup puts the binary in place by a rename

On macOS, when `~/.local/bin/brd-host` is missing, the agent setup MUST first copy this executable there by a rename.
So launchd never runs a file that cargo writes again, that a shrink of a slot deletes, or that is in the bundle of the
app. A file that is there stays.

#### Scenario: No binary at the path of the LaunchAgent
- **WHEN** the agent setup runs on macOS and `~/.local/bin/brd-host` is missing
- **THEN** it first copies this executable there by a rename

#### Scenario: A binary at the path of the LaunchAgent
- **WHEN** `~/.local/bin/brd-host` is there
- **THEN** the file stays

### Requirement: upgrade: The command line of upgrade

`brd-host upgrade` MUST take the alias of a host, `--binary <path>` and `--force`.

#### Scenario: The synopsis of upgrade
- **WHEN** a caller runs `brd-host upgrade`
- **THEN** it takes this command line

```sh
brd-host upgrade <alias> [--binary <path>] [--force]
```

### Requirement: upgrade: upgrade sends this executable over the link

`upgrade` MUST send its own executable, or `--binary`, over the link to an agent on macOS with the feature `upgrade`.
It MUST refuse a binary of another platform than the hello names before it sends it (the Mach-O header), and send
nothing when the agent runs that build already, unless `--force`.

#### Scenario: An upgrade of an agent of another build
- **WHEN** `brd-host upgrade <alias>` runs against an agent on macOS with the feature `upgrade` and another build
- **THEN** it sends its own executable, or `--binary`, over the link
  [@test] crates/host-client/src/upgrade_test.rs (`upgrade_sends_this_executable_once_and_refuses_another_platform`)

#### Scenario: A binary of another platform
- **WHEN** the Mach-O header of the binary names another platform than the hello
- **THEN** `upgrade` refuses before it sends it
  [@test] crates/host-client/src/upgrade_test.rs (`upgrade_sends_this_executable_once_and_refuses_another_platform`)

#### Scenario: An agent that runs that build already
- **WHEN** the agent runs that build already
- **THEN** `upgrade` sends nothing, unless `--force`
  [@test] crates/host-client/src/upgrade_test.rs (`upgrade_sends_this_executable_once_and_refuses_another_platform`)

### Requirement: upgrade: A host that is not macOS refuses the upgrade

An `upgrade` of a host that is not macOS MUST refuse with `agent_failed`.

#### Scenario: An upgrade of a Linux host
- **WHEN** `brd-host upgrade <alias>` runs against a host that is not macOS
- **THEN** it refuses with `agent_failed`

### Requirement: upgrade: An agent runs upgrade only after the user agrees

An agent MUST run `upgrade` only after the user agrees.

#### Scenario: An agent that wants a new build on a host
- **WHEN** an agent wants to upgrade a host
- **THEN** it runs `brd-host upgrade <alias>` only after the user agrees

### Requirement: upgrade: The agent checks the binary before the swap

The agent MUST check the size and the SHA-256 of the binary (`bad_binary`), run `<staged> version --json` within 10 s
and refuse another schema or a binary without the feature `upgrade` (`bad_binary`), refuse a lower version without
`force` (`downgrade`), and refuse another LaunchAgent than the one on the host (`plist_changed`).

#### Scenario: A binary that fails a check
- **WHEN** the binary of an upgrade has another size or SHA-256, or its `version --json` does not answer within 10 s
  as an agent of this schema with `upgrade`
- **THEN** the agent refuses with `bad_binary`
  [@test] crates/host-agent/src/upgrade_test.rs (`the_agent_checks_the_binary_keeps_the_previous_one_and_swaps_by_rename`)

#### Scenario: A lower version or another LaunchAgent
- **WHEN** the binary has a lower version and the request has no `force`, or it would write another LaunchAgent
- **THEN** the agent refuses with `downgrade`, or with `plist_changed`
  [@test] crates/host-agent/src/upgrade_test.rs (`the_agent_checks_the_binary_keeps_the_previous_one_and_swaps_by_rename`)

### Requirement: upgrade: The agent keeps the previous binary and swaps by a rename

After the checks the agent MUST keep the binary as `~/.local/bin/brd-host.previous`, put the new one at
`~/.local/bin/brd-host` by a rename, answer, wait up to 20 s for its other requests, and exit. launchd starts the new
binary.

#### Scenario: A binary that passes the checks
- **WHEN** the binary passes the checks
- **THEN** the agent keeps the binary as `~/.local/bin/brd-host.previous` and puts the new one at
  `~/.local/bin/brd-host` by a rename
- **AND** it answers, waits up to 20 s for its other requests, and exits, and launchd starts the new binary
  [@test] crates/host-agent/src/upgrade_test.rs (`the_agent_checks_the_binary_keeps_the_previous_one_and_swaps_by_rename`)
  [@test] crates/host-agent/tests/serve/upgrade.rs (`an_upgrade_swaps_the_binary_and_ends_serve_and_a_run_and_the_watchdog_see_the_next_serve`)

### Requirement: upgrade: A watchdog brings back the previous binary

A watchdog MUST wait for the new binary to listen, for the drain plus 60 s. Else it MUST rename the previous binary
back, restart the LaunchAgent, and write `~/.cache/brd-host/upgrade.json` with the outcome.

#### Scenario: A new binary that does not listen
- **WHEN** the new binary does not listen within the drain plus 60 s
- **THEN** the watchdog renames the previous binary back and restarts the LaunchAgent
- **AND** it writes `~/.cache/brd-host/upgrade.json` with the outcome
  [@test] crates/host-agent/src/upgrade_test.rs (`the_watchdog_brings_back_the_previous_binary_when_the_new_one_does_not_listen`)

#### Scenario: A new binary that listens
- **WHEN** the new binary listens in time
- **THEN** the watchdog sees the next `serve`
  [@test] crates/host-agent/tests/serve/upgrade.rs (`an_upgrade_swaps_the_binary_and_ends_serve_and_a_run_and_the_watchdog_see_the_next_serve`)

### Requirement: upgrade: The client connects again after the upgrade

The client MUST connect again every 1 s to 5 s for up to 110 s: the previous build at the end refuses with
`rolled_back`, no answer with `unreachable`. A run goes on through the restart.

#### Scenario: The previous build answers at the end
- **WHEN** the previous build answers at the end of the wait
- **THEN** the upgrade refuses with `rolled_back`

#### Scenario: No answer at the end
- **WHEN** no agent answers within 110 s
- **THEN** the upgrade refuses with `unreachable`

#### Scenario: A run during the upgrade
- **WHEN** a run goes on while the agent restarts
- **THEN** the run goes on through the restart
  [@test] crates/host-agent/tests/serve/upgrade.rs (`an_upgrade_swaps_the_binary_and_ends_serve_and_a_run_and_the_watchdog_see_the_next_serve`)

### Requirement: upgrade: upgrade replaces an unedited copy of the skill

`upgrade` MUST also replace an installed copy of the skill of another build that has no local edits, and say so.

#### Scenario: A copy of the skill of another build
- **WHEN** `upgrade` finds an installed copy of the skill of another build without local edits
- **THEN** it replaces the copy and says so
  [@test] crates/bd-host/src/skill_test.rs (`a_refresh_replaces_only_an_unedited_copy_of_another_build`)

### Requirement: serve: The commands of the agent are commands of brd-host

`serve` and the commands that the agent starts itself (`supervise`, `check --plist`, `watchdog`, `dump-env`, all
hidden) MUST be commands of `brd-host` on macOS and Linux (`host_agent::command`). A refusal of one of them MUST exit
by its code before any run, such as 75 for `lease_failed`.

#### Scenario: A refusal of a command of the agent
- **WHEN** `serve`, `supervise`, `check --plist`, `watchdog` or `dump-env` refuses
- **THEN** it exits by the exit code of its refusal before any run, such as 75 for `lease_failed`

### Requirement: serve: serve prints where it listens

`brd-host serve [--port 7742]` MUST print one JSON line `{"listening": "<address>", "fingerprint": "<64 hex>"}`, then
log on stderr.

#### Scenario: The start of serve
- **WHEN** `brd-host serve` starts
- **THEN** it prints one JSON line `{"listening": "<address>", "fingerprint": "<64 hex>"}`, then logs on stderr
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)

### Requirement: serve: serve answers the protocol, supervises each start and keeps the slots

`serve` MUST answer the requests of the protocol, start one supervisor per `Start`, and run the keeper thread.

#### Scenario: A run on the loopback
- **WHEN** a client starts a run against `serve`
- **THEN** `serve` answers its requests and starts one supervisor for the `Start`
  [@test] crates/host-agent/tests/serve.rs (`serve_answers_a_run_on_the_loopback_with_a_real_pool`)

### Requirement: serve: A follow gets a new event as the supervisor writes it

A follow MUST send a new event of its run as the supervisor writes it: on macOS a change of the run directory wakes it
(a kqueue), elsewhere it looks every 200 ms.

#### Scenario: A new event of a run
- **WHEN** the supervisor writes a new event of a run
- **THEN** the follow sends it: on macOS a change of the run directory wakes it (a kqueue), elsewhere it looks every
  200 ms
  [@test] crates/host-agent/tests/serve/link.rs (`a_new_event_is_pushed_to_the_follow_on_macos_and_polled_elsewhere`)

### Requirement: serve: A follow ends a run that lost its supervisor

A run that lost its supervisor changes no file, so the follow MUST look at it at least every 1 s and end it.

#### Scenario: A killed supervisor
- **WHEN** the supervisor of a run is killed
- **THEN** the follow looks at the run at least every 1 s and ends it, for a cancel too
  [@test] crates/host-agent/tests/serve/signals.rs (`a_killed_supervisor_ends_the_run_for_cancel_and_follow`)

### Requirement: serve: The supervisor lends a slot and waits for one up to 1 h

The supervisor MUST lend a slot through `lease_pool::host_slots::lend_waiting` with its own pid and the affinity of
the spec, up to 1 h (`slot_timeout`). It MUST write the queue line to the log once a minute
(`lease_pool::host_runs::queue_line`).

#### Scenario: Runs of one affinity
- **WHEN** two runs of one affinity start and a third comes while no slot is free
- **THEN** the two run in parallel, and the third waits for a slot
  [@test] crates/host-agent/tests/serve.rs (`two_runs_of_one_affinity_run_in_parallel_and_a_third_waits_for_a_slot`)

#### Scenario: A run in the queue
- **WHEN** a run waits in the queue
- **THEN** the supervisor writes the queue line to the log once a minute

#### Scenario: No slot within 1 h
- **WHEN** the run gets no slot within 1 h
- **THEN** it ends with `slot_timeout`

#### Scenario: A failed lend
- **WHEN** the lend fails
- **THEN** the run refuses with `lease_failed`
  [@test] crates/host-agent/tests/serve/lend.rs (`a_failed_lend_refuses_the_run_with_lease_failed`)

### Requirement: serve: A git below the floor ends the run with old_git

A lend that git refuses below its floor MUST end the run with `old_git` and the hint of Branch Deck.

#### Scenario: A git below the floor
- **WHEN** the git of the host is below the floor, and the lend refuses it
- **THEN** the run ends with `old_git` and the hint of Branch Deck
  [@test] crates/host-agent/tests/serve/lend.rs (`a_git_below_the_floor_ends_the_run_with_old_git_and_the_hint_of_bd`)

#### Scenario: Homebrew upgrades git
- **WHEN** Homebrew upgrades the git of the host while a run lends a slot
- **THEN** the run waits, and its command gets the git of the host
  [@test] crates/host-agent/tests/serve/lend.rs (`a_run_waits_while_homebrew_upgrades_git_and_its_command_gets_the_git_of_the_host`)

### Requirement: serve: A cancel in the queue ends the wait within 2 s

A cancel of a run in the queue MUST end its wait within 2 s.

#### Scenario: A cancel of a run that waits for a slot
- **WHEN** a run waits in the queue of the host slots and gets a cancel
- **THEN** its wait ends within 2 s, before its command
  [@test] crates/host-agent/tests/serve.rs (`a_cancel_in_the_queue_of_the_host_slots_ends_the_run_before_its_command`)

### Requirement: serve: The supervisor waits for the pack of its commit

The supervisor MUST wait for the pack of its commit when the repository lacks it: up to 30 min, and up to 2 min
without a chunk (`pack_missing`).

#### Scenario: A commit that the repository lacks
- **WHEN** the repository lacks the commit of a run
- **THEN** the supervisor waits for its pack, and the chunks of the pack keep the run waiting
  [@test] crates/host-agent/tests/serve.rs (`a_run_waits_for_its_pack_checks_out_by_the_diff_and_gives_its_slot_back`)
  [@test] crates/host-agent/tests/serve/pack.rs (`the_chunks_of_a_pack_keep_its_run_waiting_and_a_start_without_them_ends_at_the_stall`)

#### Scenario: A pack that never comes or stalls
- **WHEN** the pack does not come within 30 min, or sends no chunk for 2 min
- **THEN** the run ends with `pack_missing`
  [@test] crates/host-agent/tests/serve/pack.rs (`a_start_whose_pack_never_comes_ends_with_pack_missing`, `the_chunks_of_a_pack_keep_its_run_waiting_and_a_start_without_them_ends_at_the_stall`)

### Requirement: serve: The supervisor checks out the commit by the diff

The supervisor MUST check out the commit by the diff (`lease_pool::host_slots::checkout`), and write the reason of a
forced checkout to the log.

#### Scenario: The checkout of a run
- **WHEN** the commit of a run is in the repository
- **THEN** the supervisor checks it out in the slot by the diff
- **AND** a forced checkout writes its reason to the log
  [@test] crates/host-agent/tests/serve.rs (`a_run_waits_for_its_pack_checks_out_by_the_diff_and_gives_its_slot_back`)

### Requirement: serve: The supervisor gives the slot back on every way out

On every way out, after `exit.json`, the supervisor MUST give the slot back (`give_back_run`), which also deletes the
run ref. No signal cuts the give-back.

#### Scenario: The end of a run
- **WHEN** a run ends in any way
- **THEN** after `exit.json` the supervisor gives the slot back and deletes the run ref, and no signal cuts it
  [@test] crates/host-agent/tests/serve.rs (`a_run_waits_for_its_pack_checks_out_by_the_diff_and_gives_its_slot_back`)

#### Scenario: A pack after the end of its run
- **WHEN** a pack comes after its run ended
- **THEN** it lands, and it marks no run
  [@test] crates/host-agent/tests/serve/pack.rs (`a_pack_after_the_end_of_its_run_lands_and_marks_no_run`)

### Requirement: serve: The changes of a command leave out the prepare steps

With changes asked for, the supervisor MUST take the tree of the slot after the prepare steps (`snapshot_run`), and
after the command the patch from it (`changes_run`).

#### Scenario: A run with changes asked for
- **WHEN** a run asks for its changes
- **THEN** the supervisor takes the tree of the slot after the prepare steps, and after the command the patch from it
  [@test] crates/host-agent/tests/serve.rs (`the_changes_of_a_command_come_back_as_a_patch_without_the_prepare_steps`)

### Requirement: serve: The keeper wakes once a minute and at each start

The keeper MUST wake once a minute and at each `Start` of a repository, and run `ensure_host_git`, `reclaim`,
`reclaim_refs` (`lease_pool::host_runs::reclaim_all`) and `fill` in process, one repository at a time, then one
relief (`keep_free`) over the repositories.

#### Scenario: A round of the keeper
- **WHEN** a minute passed, or a `Start` of a repository came
- **THEN** the keeper runs `ensure_host_git`, `reclaim`, `reclaim_refs` and `fill` in process for each repository, one
  at a time, then one relief over the repositories
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_fills_the_slots_and_backs_off_after_a_failed_fill`)

### Requirement: serve: A failed fill backs off

A failed fill MUST wait 1, 5, then 15 minutes.

#### Scenario: Fills that fail
- **WHEN** a fill fails, and again
- **THEN** the next fill waits 1, 5, then 15 minutes
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_fills_the_slots_and_backs_off_after_a_failed_fill`)

### Requirement: serve: The relief raises the free space once per volume

Below `branchdeck.leaseMinFree` of the host, the relief MUST raise the free space by the steps of a get there, without
a get ([brd lease](../lease/spec.md), "Host slots"), once per volume: the repositories of a host share one, so the
first repository on it measures and relieves its pools, and the others of the round take none of those steps.

#### Scenario: Repositories on one volume
- **GIVEN** a host whose free space is below `branchdeck.leaseMinFree`, with several repositories on one volume
- **WHEN** the relief runs
- **THEN** the first repository on the volume measures and relieves its pools by the steps of a get, without a get
- **AND** the others of the round take none of those steps
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_keeps_the_minimum_free_space_once_per_volume_and_waits_after_a_relief_that_left_it_low`)

### Requirement: serve: The relief then relieves the idle host slots

Below the minimum still, each repository MUST relieve its idle host slots, used longest ago first: their clean rules
with keep 0, then their Bazel output bases, until the volume has the minimum.

#### Scenario: A volume still below the minimum
- **WHEN** the volume is still below the minimum after those steps
- **THEN** each repository relieves its idle host slots, used longest ago first: their clean rules with keep 0, then
  their Bazel output bases, until the volume has the minimum
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_keeps_the_minimum_free_space_once_per_volume_and_waits_after_a_relief_that_left_it_low`)

### Requirement: serve: The relief runs alone and logs what it deleted

The relief MUST run on a thread of its own, one at a time, and write one line of the log per repository whose relief
took a step, with what each step deleted (each host slot with what it freed) and the free space after.

#### Scenario: A relief that took a step
- **WHEN** the relief of a repository took a step
- **THEN** the log gets one line for that repository with what each step deleted, each host slot with what it freed,
  and the free space after
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_keeps_the_minimum_free_space_once_per_volume_and_waits_after_a_relief_that_left_it_low`)

### Requirement: serve: A relief that left the volume low makes the next one wait

A relief that leaves a volume below the minimum after every step MUST say so, and the next one waits 15 minutes;
without a minimum it deletes nothing. So a host that only runs keeps its disk without the app and without `brd`.

#### Scenario: A volume below the minimum after every step
- **WHEN** a relief leaves a volume below the minimum after every step
- **THEN** it says so, and the next relief waits 15 minutes
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_keeps_the_minimum_free_space_once_per_volume_and_waits_after_a_relief_that_left_it_low`)

#### Scenario: A host without a minimum
- **WHEN** the host has no `branchdeck.leaseMinFree`
- **THEN** the relief deletes nothing

### Requirement: serve: serve keeps the host awake

`serve` MUST hold the power assertions of `caffeinate -i -s -w <pid>` while a client has a connection after its
hello, and each supervisor for its whole life. Elsewhere than on macOS nothing holds them.

#### Scenario: A connection and a run on macOS
- **WHEN** a client has a connection after its hello, or a supervisor runs
- **THEN** on macOS `caffeinate -i -s -w <pid>` holds the power assertions for them
- **AND** elsewhere nothing holds them
  [@test] crates/host-agent/tests/serve/system.rs (`serve_and_the_supervisor_keep_the_host_awake`)

### Requirement: The environment of a run and `[host]`: A prepare step and the command get the environment in order

A prepare step and the command MUST get, in this order: the environment of the agent; the login environment of the
user; the directory of the git of the host right before `/usr/bin` in `PATH`; `BD_HOST_RUN`, `BD_HOST_AFFINITY`,
`BD_HOST_COMMIT` and `BD_HOST_SLOT`; and `[host.env]`.

#### Scenario: The environment of a command
- **WHEN** a prepare step or the command of a run starts
- **THEN** it gets the environment of the agent, then the login environment of the user, then the directory of the git
  of the host right before `/usr/bin` in `PATH`, then `BD_HOST_RUN`, `BD_HOST_AFFINITY`, `BD_HOST_COMMIT` and
  `BD_HOST_SLOT`, then `[host.env]`
  [@test] crates/host-agent/tests/serve/system.rs (`a_run_gets_the_login_environment_a_raised_open_files_limit_and_its_niceness`)

### Requirement: The environment of a run and `[host]`: The login environment is captured at the start and when old

The agent MUST capture the login environment of the user by `<shell> -l -i -c 'exec brd-host dump-env <file>'` within
30 s, at the start of `serve` and again when an hour old.

#### Scenario: The capture of the login environment
- **WHEN** `serve` starts, or its login environment is an hour old
- **THEN** it captures the login environment by `<shell> -l -i -c 'exec brd-host dump-env <file>'`, within 30 s
  [@test] crates/host-agent/tests/serve/system.rs (`a_run_gets_the_login_environment_a_raised_open_files_limit_and_its_niceness`)

### Requirement: The environment of a run and `[host]`: The soft open-files limit is raised

The soft open-files limit of a prepare step and the command MUST be 65536, or the hard limit when that is lower.

#### Scenario: The limits of a command
- **WHEN** a prepare step or the command of a run starts
- **THEN** its soft open-files limit is 65536, or the hard limit when that is lower
  [@test] crates/host-agent/tests/serve/system.rs (`a_run_gets_the_login_environment_a_raised_open_files_limit_and_its_niceness`)

### Requirement: The environment of a run and `[host]`: The agent reads [host] from the commit of the run

The agent MUST read `[host]` of `.branchdeck.toml` from the commit of the run, never from the files of the slot. A
commit whose `.branchdeck.toml` has no `[host]` table has no prepare steps, no environment of its own and no outputs.
The other tables of `.branchdeck.toml` are not the agent's.

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

#### Scenario: A commit with a [host] table
- **WHEN** a run starts on a commit whose `.branchdeck.toml` has a `[host]` table
- **THEN** the agent takes the table of that commit, never the one of the files of the slot
  [@test] crates/host-link/src/config_test.rs (`the_host_table_of_branchdeck_toml_parses_and_an_unknown_key_refuses`, `a_commit_gets_the_host_table_of_its_branchdeck_toml_else_the_default`)
  [@test] crates/host-link/src/config_test.rs (`the_host_table_of_this_repository_parses`)

#### Scenario: A commit without a [host] table
- **WHEN** the `.branchdeck.toml` of the commit has no `[host]` table
- **THEN** the run has no prepare steps, no environment of its own and no outputs
  [@test] crates/host-link/src/config_test.rs (`a_commit_gets_the_host_table_of_its_branchdeck_toml_else_the_default`)

### Requirement: The environment of a run and `[host]`: A [host] table that does not parse ends the run

An unknown key or a value of the wrong shape in `[host]` MUST end the run with `bad_config` before the command.

#### Scenario: An unknown key or a bad value
- **WHEN** the `[host]` table of the commit has an unknown key or a value of the wrong shape
- **THEN** the run ends with `bad_config` before the command
  [@test] crates/host-link/src/config_test.rs (`the_host_table_of_branchdeck_toml_parses_and_an_unknown_key_refuses`, `a_typo_or_a_bad_value_is_an_error_that_names_the_file`)

### Requirement: The environment of a run and `[host]`: A prepare step runs when its fingerprint changed

A prepare step MUST run before the command when its fingerprint differs from the one of its last success in the slot,
or when a path of `creates` is missing. A step without `inputs` runs once per slot.

#### Scenario: Changed inputs or a missing created path
- **WHEN** the fingerprint of a step differs from the one of its last success in the slot, or a path of its `creates`
  is missing
- **THEN** the step runs before the command
  [@test] crates/host-agent/src/prepare_test.rs (`a_prepare_step_runs_when_its_inputs_change_or_a_created_path_is_missing`)

#### Scenario: A step without inputs
- **WHEN** a step has no `inputs`
- **THEN** it runs once per slot

### Requirement: The environment of a run and `[host]`: A failed prepare step ends the run

A prepare step that exits with another code than 0 MUST end the run with `prepare_failed`.

#### Scenario: A step that fails
- **WHEN** a prepare step exits with another code than 0
- **THEN** the run ends with `prepare_failed`

### Requirement: The environment of a run and `[host]`: The fingerprints are keyed by the generation of the slot

The fingerprints MUST be keyed by the `generation` of the slot, so a give-back that discarded changes or a clean runs
each step again.

#### Scenario: A slot of a new generation
- **WHEN** a give-back discarded changes of the slot, or a clean ran in it
- **THEN** each step runs again at the next run

### Requirement: version, schema, skill: version prints the build

`brd-host version [--json]` MUST print the version of Branch Deck, the schema of the protocol, the features and the
SHA-256 of the binary: `{"version", "schema", "features", "sha256"}`.

#### Scenario: version --json
- **WHEN** `brd-host version --json` runs
- **THEN** it prints `{"version", "schema", "features", "sha256"}` of this build
  [@test] crates/host-link/src/proto_test.rs (`the_version_line_names_the_build_and_a_check_adds_the_launch_agent`)

### Requirement: version, schema, skill: The shapes of an upgrade stay within the schema

The hello, `Request::Upgrade`, `Reply::Upgraded` and the version line MUST keep their shapes within the schema, so an
agent takes the next binary over the link.

#### Scenario: An agent of an older build of the schema
- **WHEN** a client of a newer build of the same schema upgrades an agent
- **THEN** the hello, `Request::Upgrade`, `Reply::Upgraded` and the version line have the shapes that the agent knows
  [@test] crates/host-link/src/proto_test.rs (`a_hello_names_the_binary_and_the_features_of_this_build`, `an_upgrade_and_its_reply_have_a_tagged_json_shape`)

### Requirement: version, schema, skill: schema prints the machine contract

`brd-host schema` MUST print one JSON object: the protocol schema and port, every refusal code of the table below with
its exit code and words, the exit codes, and the JSON schemas of the report and of the hello.

#### Scenario: brd-host schema
- **WHEN** `brd-host schema` runs
- **THEN** it prints one JSON object with the protocol schema and port, every refusal code with its exit code and
  words, the exit codes, and the JSON schemas of the report and of the hello
  [@test] crates/bd-host/src/schema_test.rs (`schema_lists_every_code_and_the_shapes_of_the_report_and_the_hello`)
  [@test] crates/bd-host/tests/cli.rs (`schema_and_version_answer_one_json_object`)

### Requirement: version, schema, skill: skill install writes the skill from the binary

`brd-host skill install [--dir <dir>] [--force]` MUST write the skill `brd-host` into `~/.claude/skills/brd-host/`
from the files that the binary carries: `SKILL.md` (`crates/bd-host/skill/SKILL.md`), and this spec and
[Host Runs](../../../../docs/design/host-runs.md) as `references/spec.md` and `references/design.md`. The marker
`.brd-host-skill.sha256` lists the SHA-256 of each file.

#### Scenario: An install of the skill
- **WHEN** `brd-host skill install` runs
- **THEN** `~/.claude/skills/brd-host/` gets `SKILL.md`, `references/spec.md` and `references/design.md` of this
  build, and the marker `.brd-host-skill.sha256` with the SHA-256 of each file
  [@test] crates/bd-host/src/skill_test.rs (`skill_install_replaces_a_copy_of_another_build_and_keeps_local_edits`)

### Requirement: version, schema, skill: An install keeps a copy with local edits

A copy whose files differ from the marker has local edits: an install MUST refuse with `skill_edited` (exit 255), and
`--force` replaces it.

#### Scenario: A copy with local edits
- **WHEN** the files of the installed copy differ from its marker
- **THEN** `brd-host skill install` refuses with `skill_edited` (exit 255)
  [@test] crates/bd-host/src/skill_test.rs (`skill_install_replaces_a_copy_of_another_build_and_keeps_local_edits`, `a_copy_without_a_marker_or_with_a_missing_file_counts_as_edited`)

#### Scenario: --force
- **WHEN** `brd-host skill install --force` finds a copy with local edits
- **THEN** it replaces the copy

### Requirement: version, schema, skill: skill print prints SKILL.md

`brd-host skill print` MUST print `SKILL.md`.

#### Scenario: brd-host skill print
- **WHEN** `brd-host skill print` runs
- **THEN** it prints `SKILL.md` of this build

### Requirement: Refusal codes: A refusal names its code on the stderr

A refusal MUST say `brd-host: <code>: <message>` on the stderr. A caller MUST act on the code and the exit code, never
on the message.

#### Scenario: A refusal on the stderr
- **WHEN** brd-host refuses
- **THEN** the stderr says `brd-host: <code>: <message>`
- **AND** a caller acts on the code and the exit code, never on the message

### Requirement: Refusal codes: The exit of a refusal says whether the host ran the command

Exit 75 MUST mean that the host did not run the command, so the caller can run it locally. A refusal of `run` before
`Start`, and a refusal of the agent whose phases have no `command`, MUST exit 75. Any other refusal MUST exit 255,
except `usage` (2) and `interrupted` (130).

#### Scenario: A refusal before the command
- **WHEN** `run` refuses before `Start`, or the agent refuses a run whose phases have no `command`
- **THEN** it exits 75, and the caller can run the command locally
  [@test] crates/host-link/src/refusal_test.rs (`the_exit_of_a_code_follows_how_far_the_run_came`)

#### Scenario: Any other refusal
- **WHEN** any other refusal comes
- **THEN** it exits 255, except `usage` (2) and `interrupted` (130)
  [@test] crates/host-link/src/refusal_test.rs (`the_exit_of_a_code_follows_how_far_the_run_came`)

### Requirement: Refusal codes: Each code has one name and one exit

Each code MUST be one variant of the refusal type of `host-link`, with its name and exit in one table that
`brd-host schema` prints, so a code cannot have two exits.

#### Scenario: The table of the codes
- **WHEN** brd-host refuses with a code
- **THEN** the code has the exit and the words of its row of the table
  [@test] crates/host-link/src/refusal_test.rs (`every_refusal_code_has_one_name_and_one_exit`)

| Code | Exit | When |
|---|---|---|
| `usage` | 2 | An alias that is not `[0-9A-Za-z._-]+`, a run without a command, a working directory in no git checkout, a descriptor of `--report-fd` that is not open, a fetch without a record and without `--fetch`. A command line that clap refuses exits 2 too. |
| `interrupted` | 130 | Ctrl+C before the start, or a second Ctrl+C during the follow. |
| `config` | 75 | No home directory, or an identity, a record or a file of brd-host that cannot be read or written. |
| `no_host` | 75 | No host file for the alias. The message names `brd-host setup <alias>`. |
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
| `lease_failed` | 75 | The lend failed. The message has the error of the pool. |
| `old_git` | 75 | The git of the host is below the floor of Branch Deck, and Homebrew cannot upgrade it. The message is the hint of Branch Deck. |
| `checkout_failed` | 75 | The commit cannot be checked out in the slot. |
| `bad_config` | 75 | `[host]` of `.branchdeck.toml` of the commit does not parse, has an unknown key or a value of the wrong shape. |
| `prepare_failed` | 75 | A prepare step exited with another code than 0. |
| `spawn_failed` | 75 | The command cannot start. |
| `agent_error` | 75, 255 after the command started | The agent cannot read or write its own files, or a supervisor got lost. |
| `fetch_failed` | 255 | The fetch did not copy every file. The run stays on the host for `brd-host fetch` and `brd-host forget`. |
| `apply_failed` | 255 | The changes do not apply to the working tree, or they did not come because the fetch failed. |
| `bad_binary` | 255 | The binary of an upgrade has another size or SHA-256, or `version --json` does not answer as an agent of this schema with `upgrade`. |
| `downgrade` | 255 | The binary of an upgrade has a lower version, and the request has no `force`. |
| `plist_changed` | 255 | The new binary would write another LaunchAgent than the one on the host: `brd-host setup` installs it. |
| `upgrade_failed` | 255 | The agent cannot stage, keep or swap its binary, or start the watchdog, or another upgrade runs. |
| `rolled_back` | 255 | The new agent did not listen in time, and the previous binary answers. |
| `agent_failed` | 255 | The binary for a host cannot be read or is of another platform, the host is not macOS for `upgrade`, or `setup --agent` on the host failed. |
| `ssh_failed` | 255 | An ssh call of `setup <alias>` failed or ran past its deadline. |
| `launchd_failed` | 255 | A `launchctl` call of `setup` failed. |
| `skill_edited` | 255 | `skill install` found a copy with local edits, without `--force`. |

#### Scenario: The exit after the start
- **WHEN** a code whose row has two exits refuses
- **THEN** its exit is the one of how far the run came
  [@test] crates/host-link/src/refusal_test.rs (`the_exit_of_a_code_follows_how_far_the_run_came`)

## Testing / Local Run

- `cargo nextest run -p host-link -p host-client -p host-agent -p brd-host` runs the unit tests and the process tests
  on 127.0.0.1, with `TestRepo` pools in a temporary home directory. No fake `brd` exists.
- The live check is `brd-host hello studio`, then a run with a dirty tree and `--apply`, a run with a base, and five
  parallel runs, from a lease of this repository.

## Open Questions / Risks

- A slot serves several checkouts. Ignored outputs of another branch stay, and a tool that trusts timestamps can build
  stale output.
- A daemon that leaves the slot directory escapes the process scan of the give-back.
- A run with no slot whose last fill failed waits in the queue like any other run. A refusal at once
  (`lease_failed`) needs a fact of the last fill in the lend table.
- A cancel during the checkout or the scan of a slot waits for that step to end, because the pool runs that git in
  process.

## References

- [Host Runs](../../../../docs/design/host-runs.md): why, the slot model, the processes, the upgrade.
- [brd lease](../lease/spec.md): "Host slots", "Host objects", "Host runs".
- [Git Operations](../../../../docs/design/git-operations.md): "Host slots".
