## MODIFIED Requirements

### Requirement: The link: Every read and every write has a deadline

Every read and every write MUST have a deadline. The deadline MUST be a timeout of the channel (`tokio::time::timeout`),
not one of the socket, so it holds through the relay too. A request that waits MUST get `Reply::Heartbeat` every 15 s,
and the client resets its read deadline at each one. A heartbeat MUST be a message of the protocol, not a keepalive of
TCP: the relay through `nc` carries no keepalive of the socket, and a run that is quiet must differ from a link that is
dead.

#### Scenario: A request that waits
- **WHEN** a request waits on the agent, such as a follow whose thread is held in a file read or a `Have` while git
  upgrades
- **THEN** it gets `Reply::Heartbeat` every 15 s, and the client resets its read deadline at each one
  [@test] crates/host-agent/tests/serve/link.rs (`heartbeats_go_on_while_the_follow_thread_is_held_in_a_file_read`)
  [@test] crates/host-agent/tests/serve/lend.rs (`a_have_sends_heartbeats_while_it_waits_for_an_upgrade_of_git`)
  [@test] crates/host-client/src/run_test.rs (`heartbeats_keep_a_waiting_request_alive`)

### Requirement: The link: A connection that Local Network privacy refuses goes through nc

On macOS a connection that Local Network privacy refuses (`EHOSTUNREACH`) MUST go again through `/usr/bin/nc`, which
the client starts responsible for itself. The client MUST read the stdout and write the stdin of `nc` as the connection,
and the end of the connection MUST end `nc`. TLS stays end to end.

#### Scenario: A refused connection
- **WHEN** a connection on macOS fails with `EHOSTUNREACH`
- **THEN** it goes again through `/usr/bin/nc`, which the client starts responsible for itself
- **AND** the relay carries the handshake and the frames over the pipes of `nc`, so TLS stays end to end
  [@test] crates/host-link/src/channel_test.rs (`a_relayed_connection_carries_the_handshake_and_the_frames`)
  [@test] crates/host-link/src/relay_test.rs (`the_relay_carries_bytes_both_ways_and_ends_with_the_peer`, `local_network_privacy_is_a_cause_only_of_host_unreachable_on_a_local_address_that_the_relay_cannot_pass`)
  [@test] crates/host-link/src/disclaim_test.rs (`a_disclaimed_child_gets_its_arguments_and_its_pipes`, `a_command_that_changes_the_environment_is_refused`)

### Requirement: run: The steps of a run come in their order

The order of a run MUST be: the commit and `Have`; the run record; `Start`; `Follow` beside the other steps; the pack
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
  [@test] crates/host-client/src/run_test.rs (`the_pack_builds_after_the_start_and_keeps_its_connection_alive`)
