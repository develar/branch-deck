## MODIFIED Requirements

### Requirement: Get: The process scan finds a process by its directory

The process scan MUST find a process by its working directory, compared by path components, or by the
`--workspace_directory` of a Bazel server, and leave out the session that runs `brd`: `brd`, its ancestors and every
process of a process group one of them is in (not the group of the init process). A recorded start time MUST tell a
process from a later one with the same pid. It MUST read whether a process has a controlling terminal, and name the
processes between a process and an ancestor of it, by pid and start.

#### Scenario: A process in a directory and this process
- **WHEN** the scan looks into a directory
- **THEN** a process whose working directory is inside it is found, and this process and its group are left out
- **AND** a process runs only with the start time it was recorded with
  [@test] crates/sync-utils/src/process_scan_test.rs (`a_process_in_a_directory_is_inside_it_and_this_process_and_its_group_are_left_out`, `a_process_runs_only_with_the_start_time_it_was_recorded_with`)

#### Scenario: The session of brd
- **WHEN** a process is in a group of `brd` or one of its ancestors (the `jq` of `brd lease return | jq`, the
  `caffeinate` of the agent)
- **THEN** it does not keep the slot busy
- **AND** a background task of the agent leads a group of its own and still counts, as a command of the holder
  [@test] crates/bd-lease/src/lease_test.rs (`a_process_of_the_group_of_bd_does_not_keep_the_slot_busy`)

#### Scenario: A terminal and the path to an ancestor
- **WHEN** the scan reads a process in a session with a controlling terminal, one in a session without one, and the
  chain of parents of a process
- **THEN** only the first has a terminal
- **AND** the path to an ancestor names the processes between them, the parent first, and none for a process below
  another one with the pid of the ancestor
  [@test] crates/sync-utils/src/process_scan_test.rs (`a_process_has_a_terminal_only_when_its_session_has_one`, `the_path_to_an_ancestor_names_the_processes_between_them`)

### Requirement: Return: A return refuses what the slot still holds

The return MUST refuse, with nothing changed: a worktree without a lease (`no_lease`, exit 2), another `--lease-id`
(`lease_mismatch`, exit 2), a process in the worktree (`slot_busy`, exit 4), HEAD with commits that no branch, tag or
remote branch holds (`unlanded_commits`, exit 4), an open resolve session of the slot (`resolve_in_progress`, exit 4),
and local changes (`dirty_worktree`, exit 4, `paths`). A refusal after `--stop-own` ended the commands of the holder
names them in its message.

#### Scenario: A busy slot
- **WHEN** a process of another session works in the worktree
- **THEN** the return refuses with `slot_busy`, `processes` [{`pid`, `command`, `argv` (at most 200 characters),
  `kind`: `bazel_server`, `holder` or `other`, `workspace`?}]
- **AND** the message names each with its argv, so no `ps` is needed to tell them

#### Scenario: Unlanded commits
- **WHEN** HEAD has commits that no branch, tag or remote branch holds
- **THEN** the return refuses with `unlanded_commits` and `commits`, the message naming `brd -C <dir> lease pick`

#### Scenario: An open resolve session
- **WHEN** the slot has an open resolve session
- **THEN** the return refuses with `resolve_in_progress`, as the next lease of the slot would find it
- **AND** the message names `brd update --continue` and `--abort`, and `--confirm-preserved` does not pass it

### Requirement: Return: A return signals no other process

The return MUST NOT signal any other process: any other process in the slot refuses as `slot_busy` before a server is
signalled, and the message says that the return stops the servers itself. Only `--stop-own` MUST end a process, and
only a command of the holder. A server that does not exit MUST keep the slot busy (`slot_busy`, the message saying it
did not exit on SIGTERM, on `bazel shutdown` on Windows).

#### Scenario: Another process and a server that does not exit
- **WHEN** another process is in the slot, or a server does not exit within 30 s
- **THEN** the return refuses as `slot_busy`, and no other process is signalled
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_signals_no_other_process_and_a_server_that_does_not_exit_keeps_the_slot_busy`)

## ADDED Requirements

### Requirement: Return: `--stop-own` ends the commands of the holder

A process in the slot MUST be a command of the holder when the `brd` of the return runs below the holder process of the
lease (`holder_pid` with `holder_started`), the process runs below it too by the chain of its parents, its start is
known, and the root of its tree (the farthest process of the slot on its chain below the holder) has no controlling
terminal: a coding agent runs each command in a session of its own without one, a shell of a terminal tab has one and
is the user's. Every other process is `other`: of another session, an orphan, or seen by a `brd` that the holder does
not run (a coordinator, a resumed session). `slot_busy` MUST name a command of the holder `kind: holder`, and its
message `--stop-own`, once its output is no longer needed, when no process of another session is in the slot.
`brd lease return --stop-own` MUST end each tree of the commands of the holder, its root last, only once nothing else
refuses the return, then check the slot again without them, ending nothing more, and name what it ended in
`stopped_processes` [{`pid`, `command`, `argv`}]. With a process of another session in the slot it MUST end nothing.
Without a process scan (Windows) there are none.

#### Scenario: A background command of the holder
- **WHEN** a background command of the agent that holds the lease works in the slot, and the return runs without
  `--stop-own`
- **THEN** it refuses as `slot_busy`, the command `kind: holder`, the message naming `--stop-own`, nothing signalled
  [@test] crates/bd-lease/src/lease_test.rs (`a_background_command_of_the_holder_is_named_holder_and_keeps_the_slot_without_stop_own`)

#### Scenario: The return ends the command
- **WHEN** the return runs with `--stop-own`
- **THEN** the shell of the command and what it started end, and the slot is free, `stopped_processes` naming them
- **AND** the return checks the slot again after the kill, its git once more
  [@test] crates/bd-lease/src/lease_test.rs (`stop_own_ends_the_trees_of_the_holder_and_returns_the_slot`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_return_that_stops_a_command_of_the_holder`)
  [@test] crates/sync-utils/src/process_tree_test.rs (`a_running_tree_of_another_parent_is_killed_with_its_root`)

#### Scenario: What `--stop-own` never ends
- **WHEN** a process of another session, a command whose session has a controlling terminal, or a command seen by a
  `brd` that the holder does not run is in the slot, or another check refuses the return
- **THEN** the return refuses, `kind: other` for each of the first three, and no process is signalled
  [@test] crates/bd-lease/src/lease_test.rs (`stop_own_never_ends_a_process_of_another_session`, `a_command_with_a_terminal_stays_other`, `a_caller_that_is_not_the_holder_sees_other`, `stop_own_stops_nothing_when_a_check_refuses`)
