---
name: Start in Air
description: Requirements for "Start in Air…" of a failed robot — the dialog that starts the prompt of the robot as an agent session in the IDE with Air that has the repository open, over the local AHP socket of Air as a local app (the local peer handshake v2, then JSON-RPC of `ahp-root://`, IJAI-7153) — the discovery of the IDE, the default agent and the agent, model, effort, mode, Plan and start pickers, the choice remembered per repository, the refusals of the IDE, and the toast that follows the row of the started session.
style: plain-1
targets:
  - crates/air-launch/src/lib.rs
  - crates/air-launch/src/discovery.rs
  - crates/air-launch/src/client.rs
  - crates/air-launch/src/protocol.rs
  - crates/air-launch/src/watch.rs
  - crates/app-api/src/app/air_launch.rs
  - src/lib/air-launch.ts
  - src/lib/air-session-toast.ts
  - src/stores/air-launch-dialog.ts
  - src/components/branch/StartInAirDialog.tsx
---

# Start in Air

Status: Implemented
Date: 2026-10-30

## Purpose

"Copy Prompt for Agent" puts the failure of a robot on the clipboard, and the user pastes it into an agent. Air, the
agent plugin of the JetBrains IDEs, has a local AHP socket: a local app names the project, the agent, the model and
the effort, and Air starts the session after the user agreed in the IDE. "Start in Air…" sends the same prompt there in
one dialog.

## Goals

- From a failed robot to a running agent session in the IDE in one dialog, without the clipboard.
- The last agent, model, effort, mode, Plan, plan effort and start of the repository come back the next time.
- After Run, a toast says what the session does until it is done, without a look at the IDE.
- When no IDE answers, the prompt is still one click away.

## Non-goals

- A consent or an identity of the client: the IDE asks the user and names the peer process by the kernel.
- A session in an IDE that does not have the repository open: the IDE refuses it (`NO_PROJECT`).
- The prompt with the CI details: the dialog starts the fast prompt, which tells the agent how to read the rest.
- A poll of the session status: the watches of the sessions of one IDE share one connection.

## Requirements

### Requirement: The IDE: Each IDE with Air is found by its instance file

Each IDE process with Air that admits local apps writes one instance file, `<pid>.json`, in
`<PathManager.getCommonDataPath()>/air/ahp` of the IDE, and Branch Deck MUST read that directory. A file whose process
does not run, a file that does not parse and a file of another `schema` than 2 MUST be skipped. A directory that does
not exist MUST read as no instance, not as an error.

#### Scenario: The files of running processes are read, the others skipped
- **GIVEN** the directory of instance files of the platform
- **WHEN** Branch Deck reads it
- **THEN** it reads the file of each running IDE process: its `pid`, `product`, `build` and `socket`
- **AND** it skips a file whose process does not run, a file that does not parse and a file of another `schema`
  [@test] crates/air-launch/src/discovery_test.rs (`reads_the_files_of_running_processes_and_skips_dead_garbled_and_later_ones`, `find_instance_checks_the_processes_of_the_files`)

#### Scenario: No directory of instance files
- **WHEN** Branch Deck reads a directory of instance files that does not exist
- **THEN** it reads no instance, and no error
  [@test] crates/air-launch/src/discovery_test.rs (`no_directory_is_no_instance`)

#### Scenario: The directory of the instance files of each platform
- **WHEN** Branch Deck finds the directory of the instance files on macOS, Windows and Linux
- **THEN** it is `~/Library/Application Support/JetBrains/air/ahp` on macOS
- **AND** `%APPDATA%\JetBrains\air\ahp` on Windows, else `~\AppData\Roaming\JetBrains\air\ahp`
- **AND** `$XDG_DATA_HOME/JetBrains/air/ahp` on Linux, else `~/.local/share/JetBrains/air/ahp`
- **AND** an empty variable is unset, as the IDE reads it, and so is a relative `XDG_DATA_HOME`
  [@test] crates/air-launch/src/discovery_test.rs (`the_directory_of_the_instance_files_is_the_common_data_directory_of_the_ide_on_each_platform`)

### Requirement: The IDE: Each running IDE names its hosts

Branch Deck MUST ask each running IDE over its socket for its hosts (`x-air-ij/hosts`, the result
`{"product","build","hosts":[{"projectDirectory"}],"focusedAt"}`), all IDEs at once, each within 2 s
(`HOSTS_TIMEOUT`). The IDE asks no consent for it. An IDE that does not answer in time, fails or refuses MUST be skipped
with a debug line.

#### Scenario: The hosts request and its answer
- **WHEN** the client asks an IDE for its hosts
- **THEN** the request has no params but the channel `ahp-root://`
- **AND** the answer names the absolute launch directories and the last focus, or a refusal
  [@test] crates/air-launch/src/client_test.rs (`the_local_app_side_of_the_corpus_of_the_ide_replays`, `the_handshake_writes_the_opener_then_the_envelope_with_the_echo_of_the_nonce_and_initialize_without_access`)

#### Scenario: An IDE that refuses, fails or does not answer in time
- **WHEN** an IDE refuses `x-air-ij/hosts`, has a socket nobody listens on, or does not answer in time
- **THEN** Branch Deck skips it and takes the hosts of the others
  [@test] crates/air-launch/src/discovery_test.rs (`each_ide_names_its_hosts_and_one_that_refuses_fails_or_does_not_answer_in_time_is_skipped`)

### Requirement: The IDE: The IDE of the repository has its directory or a parent open

The IDE of the repository MUST be the one whose `hosts` holds the directory of the repository or one of its parents,
compared by path components; the deepest host first, then the newest `focusedAt` (the last focus of a window of that
IDE, Unix milliseconds; an IDE that answers none is older than any), then the newest instance file.

#### Scenario: A parent is open and the deepest project wins
- **WHEN** IDEs host the directory of the repository or one of its parents, compared by path components
- **THEN** the IDE of the deepest host is the IDE of the repository
  [@test] crates/air-launch/src/discovery_test.rs (`the_ide_of_a_directory_has_it_or_a_parent_open_and_the_deepest_project_wins`)

#### Scenario: Of two IDEs with the same project the newest file wins
- **WHEN** two IDEs host the same directory and neither answers a `focusedAt`
- **THEN** the IDE of the newest instance file is picked
  [@test] crates/air-launch/src/discovery_test.rs (`of_two_ides_with_the_same_project_the_newest_file_wins`)

#### Scenario: Of two IDEs with the same project the last focused wins
- **WHEN** two IDEs host the same directory
- **THEN** the one with the newest `focusedAt` is picked
- **AND** an IDE that answers no `focusedAt` is older than any
  [@test] crates/air-launch/src/discovery_test.rs (`of_two_ides_with_the_same_project_the_last_focused_wins_and_one_without_a_focus_is_older_than_any`)

#### Scenario: The deepest project wins over the last focus
- **WHEN** one IDE hosts a deeper directory and another answers the newer `focusedAt`
- **THEN** the IDE of the deeper host is picked
  [@test] crates/air-launch/src/discovery_test.rs (`the_deepest_project_wins_over_the_last_focus`)

### Requirement: The IDE: A dead IDE is never picked

A dead IDE MUST NEVER be picked as the IDE of the repository.

#### Scenario: The file of a dead IDE is the newest
- **GIVEN** the newest instance file of the project belongs to a process that does not run
- **WHEN** Branch Deck picks the IDE of the repository
- **THEN** it does not pick that IDE
  [@test] crates/air-launch/src/discovery_test.rs (`a_dead_ide_is_never_picked_though_its_file_is_the_newest`)

### Requirement: The IDE: No test reaches an IDE of the user

The e2e server MUST have no directory of instance files (`AppHost::air_instances`): no test reaches an IDE of the
user.

#### Scenario: The e2e server looks for an IDE
- **WHEN** the e2e server looks for the IDE of a repository
- **THEN** it has no directory of instance files, and no IDE of the user is reached

### Requirement: The exchange: One connection per request, after the local peer handshake v2

Each request of the dialog MUST take one connection. The client runs the local peer handshake v2 first: it writes the
opener `{"proto":"local-peer","v":2}`, reads the greeting with its nonce within 10 s (`HANDSHAKE_TIMEOUT`, and never
later than the timeout of the answer), and writes the envelope `{"nonce":"<the echo>","request":{…}}`. The request is
`initialize` of `ahp-root://` with no `access`, so the IDE admits a local app, and the client reads its result.

#### Scenario: The opener, then the envelope with the echo of the nonce
- **WHEN** the client sends a request
- **THEN** it writes the opener and reads the greeting
- **AND** it writes the envelope with the echo of the nonce and `initialize`, without `access`, on one line
- **AND** after the result of `initialize` it sends the request with the id 2
  [@test] crates/air-launch/src/client_test.rs (`the_handshake_writes_the_opener_then_the_envelope_with_the_echo_of_the_nonce_and_initialize_without_access`)
  [@test] crates/air-launch/src/protocol_test.rs (`the_envelope_holds_the_echo_of_the_nonce_and_the_request_object_on_one_line`)

#### Scenario: The local-app side of the corpus of the IDE
- **WHEN** the client replays the frames that a local app sends and gets in the corpus of the IDE
  (`crates/air-launch/fixtures/local-ahp-apps.ndjson`)
- **THEN** it sends each frame of the corpus, and reads each answer, the refusal `UNAUTHORIZED` of `initialize` and the
  notice before an answer
  [@test] crates/air-launch/src/client_test.rs (`the_local_app_side_of_the_corpus_of_the_ide_replays`)

### Requirement: The exchange: The answer of each request within 120 s

Each request after `initialize` MUST have its own id, and the client MUST read frames until the answer of that id
within 120 s (`RESPONSE_TIMEOUT`: the consent dialog and the launch). A notice that comes before the answer MUST wait
for the watch, and the answer of another id and a request of the IDE MUST be skipped.

#### Scenario: The IDE does not answer in time
- **WHEN** the IDE sends no answer of the id within the timeout
- **THEN** the request fails with a timeout, an error of the command
  [@test] crates/air-launch/src/client_test.rs (`an_ide_that_does_not_answer_in_time_is_a_timeout`)

#### Scenario: A notice, the answer of another id and a request of the IDE before the answer
- **WHEN** a notice, the answer of another id or a request of the IDE comes before the answer of the request
- **THEN** the notice waits for the watch, and the other two are skipped
  [@test] crates/air-launch/src/client_test.rs (`a_notice_before_the_answer_waits_and_the_answer_of_another_id_and_a_request_of_the_ide_are_skipped`)
  [@test] crates/air-launch/src/protocol_test.rs (`a_request_names_the_channel_of_the_root_and_its_params_must_be_an_object`, `a_frame_is_an_answer_a_notice_or_a_request_of_the_ide`)

### Requirement: The exchange: Limits and bad answers are errors of the command

A request line longer than 1 MiB MUST NOT be sent; a frame longer than 4 MiB, one that is not a frame of JSON-RPC, a
result of another shape, a close without an answer, and no answer in time MUST be errors of the command. A socket
nobody listens on (the instance file names a socket that is gone) MUST be a connect error, also an error of the command.

#### Scenario: A request longer than the limit
- **WHEN** the line of a request is longer than 1 MiB
- **THEN** the request is not sent
  [@test] crates/air-launch/src/client_test.rs (`a_request_longer_than_the_limit_is_not_sent`)

#### Scenario: A response longer than the limit
- **WHEN** a frame of the IDE is longer than 4 MiB
- **THEN** it is an error of the command
  [@test] crates/air-launch/src/client_test.rs (`a_frame_longer_than_the_limit_is_refused`)

#### Scenario: A frame that is not JSON-RPC, or a result of another shape
- **WHEN** a frame of the IDE is not JSON, not a frame of JSON-RPC, or its result has another shape
- **THEN** it is an error of the command, a bad response
  [@test] crates/air-launch/src/client_test.rs (`a_garbled_frame_or_a_result_of_another_shape_is_a_bad_response`)

#### Scenario: A close without an answer
- **WHEN** the IDE closes the connection without an answer
- **THEN** it is an error of the command
  [@test] crates/air-launch/src/client_test.rs (`a_close_without_an_answer_is_closed_and_a_frame_without_its_end_is_read_whole`)

#### Scenario: No answer in time
- **WHEN** no answer comes in time
- **THEN** it is an error of the command
  [@test] crates/air-launch/src/client_test.rs (`an_ide_that_does_not_answer_in_time_is_a_timeout`)

#### Scenario: Nobody listens on the socket
- **WHEN** the client sends a request to a socket path nobody listens on
- **THEN** the request fails with a connect error (`ClientError::Connect`), an error of the command
  [@test] crates/air-launch/src/client_test.rs (`a_socket_nobody_listens_on_is_a_connect_error`)

### Requirement: The exchange: AF_UNIX on every platform

The connection MUST be AF_UNIX on every platform, on Windows through `uds_windows`.

#### Scenario: The client connects on Windows
- **WHEN** the client connects to the IDE on Windows
- **THEN** it uses AF_UNIX through `uds_windows`

### Requirement: The exchange: A target has its models, modes, Plan and listsLoading

A target SHALL have its models, its modes (`id`, `name`, `kind`: `plan`, `auto_review`, `standard`, `full_access`, a
kind of a later protocol as `other`), `plan` (a launch can start in Plan) and `listsLoading` (the IDE answered before
the agent listed its models and modes); an older IDE sends no modes, no `plan` and no `listsLoading`.

#### Scenario: The targets name the project and read the agents
- **WHEN** the client asks the IDE for its targets
- **THEN** the request names the project
- **AND** each agent is read with its models, its modes and whether its lists still load
  [@test] crates/air-launch/src/client_test.rs (`the_local_app_side_of_the_corpus_of_the_ide_replays`)

#### Scenario: A target of an older IDE
- **WHEN** a target comes without `listsLoading`
- **THEN** its lists are read as loaded
  [@test] crates/air-launch/src/protocol_test.rs (`a_target_without_lists_loading_has_its_lists`)

### Requirement: The exchange: A launch sends the mode, Plan and plan effort only when set

Run MUST be `createSession` and Draft `x-air-ij/openDraft`, each with `projectDirectory` and the prompt (empty for a
draft without one). A launch MUST send the target and its surface only together, and `model`, `effort`, `mode`, `plan`,
`planEffort` and the title of Run only when set. Without a target the IDE starts the default of its New Session button.

#### Scenario: An accepted launch sends every field
- **WHEN** a launch with every field set is accepted
- **THEN** it sends every field and reads the local session id of the result
  [@test] crates/air-launch/src/client_test.rs (`the_local_app_side_of_the_corpus_of_the_ide_replays`, `the_launch_of_the_wire_starts_a_route_with_every_field_reads_each_refusal_and_watches_the_rows`)

#### Scenario: A draft without a model, an effort, a mode, Plan and a prompt
- **WHEN** a launch has no model, no effort, no mode, no Plan and no prompt
- **THEN** it leaves them out, and a draft sends an empty prompt and no title
  [@test] crates/air-launch/src/protocol_test.rs (`the_params_of_a_launch_send_an_option_only_when_set_and_a_surface_only_with_a_target`)

#### Scenario: A launch without a target
- **WHEN** a launch names no target
- **THEN** `createSession` has no `launchTargetId` and no `interactionSurfaceId`, and the IDE starts its default
  [@test] crates/air-launch/src/client_test.rs (`the_local_app_side_of_the_corpus_of_the_ide_replays`)
  [@test] crates/air-launch/src/protocol_test.rs (`the_params_of_a_launch_send_an_option_only_when_set_and_a_surface_only_with_a_target`)

### Requirement: The exchange: A refusal is an answer, never an error

An error of the IDE MUST be a refusal with the code of `error.data.code` (`DENIED`), and the message and the hint of
`error.data`, never an error of the command. A code this client does not know stays as its text.

#### Scenario: A refusal of a launch or of the targets
- **WHEN** the IDE refuses a launch or the targets
- **THEN** the answer is the refusal with its code, message and hint
  [@test] crates/air-launch/src/client_test.rs (`the_local_app_side_of_the_corpus_of_the_ide_replays`, `the_launch_of_the_wire_starts_a_route_with_every_field_reads_each_refusal_and_watches_the_rows`)

#### Scenario: An error without a code of its data
- **WHEN** an error of the IDE has no `data.code`
- **THEN** its code is the code of its number (`UNKNOWN_METHOD`, `BAD_PARAMS`, `BAD_REQUEST`, else `ERROR_<number>`)
  [@test] crates/air-launch/src/client_test.rs (`an_error_without_a_code_of_its_data_reads_the_code_of_its_number`)
  [@test] crates/air-launch/src/protocol_test.rs (`a_refusal_reads_the_code_message_and_hint_of_the_data_else_the_code_of_the_number`)

### Requirement: The exchange: A refusal of the handshake is an error of the command

A refusal of the handshake (`{"proto":"local-peer","v":2,"error":"…"}`: `BAD_OPENER`, `BAD_NONCE`, `PEER_CHANGED`, a
code of a later handshake as its text) MUST be an error of the command: the IDE did not read the request. It comes
instead of the greeting or as the first line after the envelope, also of a watch.

#### Scenario: The refusal comes instead of the greeting or after the envelope
- **WHEN** the IDE refuses the handshake instead of the greeting, or as the first line after the envelope of a request
  or of a watch
- **THEN** it is a typed error of the command
  [@test] crates/air-launch/src/client_test.rs (`a_refusal_of_the_handshake_is_a_typed_error`)

#### Scenario: Every refusal code of handshake v2
- **WHEN** the refusal names `BAD_OPENER`, `BAD_NONCE`, `PEER_CHANGED` or a code of a later handshake
- **THEN** the code reads back as its text
  [@test] crates/air-launch/src/protocol_test.rs (`every_refusal_code_of_handshake_v2_reads_back_as_its_text`)

### Requirement: The exchange: A bad greeting is a bad response and no envelope is sent

A greeting of another version, without a nonce or with a nonce that is not base64url without padding MUST be a bad
response, and the client MUST send no envelope.

#### Scenario: A greeting of another version or without a nonce
- **WHEN** the greeting is of another version or has no nonce
- **THEN** it is a bad response and the client sends no envelope
- **AND** no greeting in time is a timeout
  [@test] crates/air-launch/src/client_test.rs (`a_greeting_of_another_version_without_a_nonce_or_of_no_handshake_is_a_bad_response_and_no_greeting_in_time_is_a_timeout`)

#### Scenario: A line of local-peer is a greeting or a refusal
- **WHEN** the client reads a line during the handshake
- **THEN** a line of local-peer is a greeting, a refusal, or a bad greeting (another version, no nonce, a nonce that is
  not base64url without padding)
- **AND** any other line is the application's
  [@test] crates/air-launch/src/protocol_test.rs (`a_line_of_local_peer_is_a_greeting_or_a_refusal_and_any_other_line_is_the_applications`)

### Requirement: The exchange: A watch reads each status of the session until its end

A watch MUST subscribe the root (no consent), list the rows with `listSessions` page by page (its first call asks the
consent), then read each root notice without a timeout of the client: `root/sessionAdded`, `root/sessionRemoved` and
`root/sessionSummaryChanged`. It ends with a close of the IDE or a stop. A notice of another method MUST be skipped.

#### Scenario: The rows, then each change until the close
- **WHEN** the client watches the rows of an IDE
- **THEN** it subscribes the root, lists the rows page by page, and reads each change of a row until the close
  [@test] crates/air-launch/src/client_test.rs (`the_launch_of_the_wire_starts_a_route_with_every_field_reads_each_refusal_and_watches_the_rows`)

#### Scenario: A watch waits without a timeout
- **WHEN** the IDE sends no notice for long
- **THEN** the watch waits, with no timeout of the client
- **AND** it skips a notice of another method
  [@test] crates/air-launch/src/client_test.rs (`a_watch_waits_without_a_timeout_skips_a_notice_of_another_method_and_its_stopper_ends_it_from_another_thread`)

#### Scenario: A garbled notice of a watch
- **WHEN** a notice of the root does not parse
- **THEN** it is a bad response and ends the watch
  [@test] crates/air-launch/src/client_test.rs (`a_garbled_notice_of_a_watch_is_a_bad_response_and_ends_it`)

### Requirement: The exchange: A stop from another thread ends a watch

A stop from another thread MUST close the connection of a watch, and the read it ends MUST give no line and no error
on every platform (Windows refuses a read after the shutdown, where Unix reads the end of the stream).

#### Scenario: The stopper ends the watch from another thread
- **WHEN** another thread stops a watch that waits for a status
- **THEN** the connection closes and the read gives no line and no error
  [@test] crates/air-launch/src/client_test.rs (`a_watch_waits_without_a_timeout_skips_a_notice_of_another_method_and_its_stopper_ends_it_from_another_thread`)

### Requirement: The action: A failed robot with Copy Prompt for Agent has Start in Air

A failed or failing robot that has "Copy Prompt for Agent" MUST also have "Start in Air…", before the prompts and after
the restart or the cancel, with the same blocker, in every form of the robot's actions: it is the primary way to fix
the failure (it opens a session in the IDE that has the repository open), the prompts are for a user who copies them
into an agent elsewhere.

#### Scenario: The actions of a failed robot
- **GIVEN** a failed robot that has "Copy Prompt for Agent"
- **WHEN** its actions are listed
- **THEN** "Start in Air…" stands after the restart and before the prompts
- **AND** it has the same blocker as "Copy Prompt for Agent"
  [@test] crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)

### Requirement: The action: The robot details show Start in Air as the primary button

The robot details MUST show "Start in Air…" as the primary button, the prompts beside it.

#### Scenario: The details of a failed robot
- **WHEN** the user opens the details of a failed robot that has "Start in Air…"
- **THEN** "Start in Air…" is the primary button and the prompts stand beside it

### Requirement: The dialog: The dialog asks the IDE for its agents and reads the prompt at once

The dialog MUST ask the IDE of the repository for its agents (`targets`) and read the fast prompt at once. While the
IDE answers it MUST say "Asking the IDE for its agents… It may ask you first.".

#### Scenario: The dialog opens
- **WHEN** the user chooses "Start in Air…"
- **THEN** the dialog asks the IDE of the repository for its agents and reads the fast prompt at once
- **AND** while the IDE answers it says "Asking the IDE for its agents… It may ask you first."

### Requirement: The dialog: No IDE, a refusal and an error are said in the dialog

No IDE, a refusal and an error MUST be said in the dialog with Try Again, and Copy Prompt MUST stay in the footer.

#### Scenario: No IDE, a refusal or an error
- **WHEN** the IDE of the repository is not found, refuses, or the request ends with an error
- **THEN** the dialog says so with Try Again
- **AND** Copy Prompt stays in the footer

### Requirement: The dialog: The default agent comes first

The agent picker MUST offer "Default Agent" first, before the agents of the IDE, also when the IDE lists none. Its
launch MUST name no target, so the IDE starts the default of its New Session button; it has no model, mode and Plan of
its own.

#### Scenario: The default agent stands first
- **WHEN** the IDE lists its agents, or none
- **THEN** the agent picker offers "Default Agent" first
  [@test] src/lib/air-launch.test.ts (`stands first before the agents of the IDE, also when the IDE lists none`)

#### Scenario: The launch of the default agent
- **WHEN** the user starts the default agent
- **THEN** the launch names no target, no model, no effort, no mode and no Plan
  [@test] src/lib/air-launch.test.ts (`sends no target, so the IDE starts the default of its New Session button`)

### Requirement: The dialog: The agent, model and effort pickers

The dialog MUST offer the agent; its model (off for an agent without models: the launch names none); the effort,
"Default" first (the launch names none) and the efforts of the model.

#### Scenario: An agent without models
- **WHEN** the agent has no models
- **THEN** the model picker is off and the launch names no model and no effort
  [@test] src/lib/air-launch.test.ts (`has no model and no effort for an agent without models, and no form without agents`)

#### Scenario: The efforts of the chosen model
- **WHEN** the user opens the effort picker
- **THEN** it offers "Default" first and the efforts of the chosen model, none for an agent without models
  [@test] src/lib/air-launch.test.ts (`offers the efforts of the chosen model, none for an agent without models`)

### Requirement: The dialog: The mode, Plan and plan effort pickers

The dialog MUST offer the mode only for an agent with modes, "Default" first (the launch names none) and its modes
without the one of kind `plan`; the Plan switch only for an agent that can start in Plan; the plan effort only while
Plan is on, "Default" first and the efforts of the model.

#### Scenario: The modes without the mode of Plan, and Plan only where the agent can start in it
- **WHEN** the agent has modes, one of them of kind `plan`
- **THEN** the mode picker offers "Default" first and the modes without the one of kind `plan`
- **AND** the Plan switch shows only for an agent that can start in Plan
  [@test] src/lib/air-launch.test.ts (`offers the modes without the mode of Plan, and Plan only for a target that can start in it`)

#### Scenario: The plan effort only while Plan is on
- **WHEN** the user turns Plan on
- **THEN** the plan effort starts at "Default"
- **AND** turning Plan off drops the plan effort
  [@test] src/lib/air-launch.test.ts (`keeps the plan effort only with Plan and while the model takes it`)

### Requirement: The dialog: Run or Draft, and an editable prompt

The dialog MUST offer Run or Draft, and the prompt MUST be editable.

#### Scenario: The user edits the prompt and picks the start
- **WHEN** the dialog shows the agents
- **THEN** the user picks Run or Draft and can edit the prompt

### Requirement: The dialog: Plan of a mode sends no mode

While Plan is on and Plan of the agent is a mode, the mode picker MUST be off and the launch MUST name no mode (else
`MODE_CONFLICT`).

#### Scenario: Plan of a mode and Plan that is not a mode
- **WHEN** Plan is on and Plan of the agent is a mode
- **THEN** the mode picker is off and the launch names no mode
- **AND** with an agent whose Plan is not a mode, the launch names the mode with Plan
  [@test] src/lib/air-launch.test.ts (`sends no other mode with Plan of a mode, and the mode with Plan of a target whose Plan is not a mode`)

### Requirement: The dialog: A plan effort is sent only with Plan

A plan effort MUST be sent only with Plan (else `BAD_REQUEST`).

#### Scenario: A plan effort without Plan
- **WHEN** Plan is off
- **THEN** the launch names no plan effort
  [@test] src/lib/air-launch.test.ts (`keeps the plan effort only with Plan and while the model takes it`)

### Requirement: The dialog: The pickers start from the choice remembered for the repository

The pickers MUST start from the choice remembered for the repository while the IDE still offers it, else the first
agent (the default agent in the dialog), its default model, the default effort, the default mode, Plan off and Run. A
choice remembered by an older version has no mode and no Plan.

#### Scenario: Nothing remembered
- **WHEN** the dialog opens with nothing remembered for the repository
- **THEN** the pickers start with the first agent, its default model, the default effort and Run
- **AND** in the dialog the first agent is the default agent, which also comes back as the remembered choice
  [@test] src/lib/air-launch.test.ts (`starts with the first agent, its default model, the default effort and Run`, `is the first choice with nothing remembered, and comes back as the remembered choice`)

#### Scenario: The remembered agent, model, effort and start
- **WHEN** the IDE still offers the remembered agent, model and effort
- **THEN** the pickers start with them and the remembered start
  [@test] src/lib/air-launch.test.ts (`starts with the remembered agent, model, effort and start while the IDE still offers them`)

#### Scenario: A remembered model or effort the agent no longer offers
- **WHEN** the agent no longer offers the remembered model or effort
- **THEN** the pickers drop it
  [@test] src/lib/air-launch.test.ts (`drops a remembered model or effort the agent no longer offers`)

#### Scenario: The remembered agent is gone
- **WHEN** the IDE no longer offers the remembered agent
- **THEN** the pickers take the first agent and keep the remembered start
  [@test] src/lib/air-launch.test.ts (`takes the first agent when the remembered one is gone, and keeps the remembered start`)

#### Scenario: No agents
- **WHEN** the list of the pickers has no agents
- **THEN** the dialog has no form of pickers
- **AND** the dialog never gets such a list: the default agent is always in it
  [@test] src/lib/air-launch.test.ts (`has no model and no effort for an agent without models, and no form without agents`, `stands first before the agents of the IDE, also when the IDE lists none`)

#### Scenario: The remembered mode, Plan and plan effort
- **WHEN** the IDE still offers the remembered mode and Plan
- **THEN** the pickers start with the remembered mode, Plan and plan effort
- **AND** without them in the IDE, the default mode and Plan off
- **AND** a choice remembered by an older version starts with no mode and no Plan
  [@test] src/lib/air-launch.test.ts (`starts with the remembered mode, Plan and plan effort while the IDE still offers them`)

### Requirement: The dialog: Another agent or model keeps a choice only while it offers it

Another agent MUST take its default model; an effort and a plan effort stay only while the model takes them, and a
mode and Plan only while the agent offers them.

#### Scenario: Another agent
- **WHEN** the user picks another agent
- **THEN** it takes its default model and keeps the effort only while that model takes it
  [@test] src/lib/air-launch.test.ts (`another agent takes its default model and keeps the effort only while that model takes it`)

#### Scenario: Another model
- **WHEN** the user picks another model
- **THEN** the effort stays only while the model takes it
  [@test] src/lib/air-launch.test.ts (`another model keeps the effort only while it takes it`)

#### Scenario: The plan effort and another model
- **WHEN** the user picks another model or turns Plan off
- **THEN** the plan effort stays only with Plan and while the model takes it
  [@test] src/lib/air-launch.test.ts (`keeps the plan effort only with Plan and while the model takes it`)

#### Scenario: The mode and Plan of another agent
- **WHEN** the user picks another agent
- **THEN** the mode and Plan stay only while that agent offers them
  [@test] src/lib/air-launch.test.ts (`another agent keeps the mode and Plan only while it offers them`)

### Requirement: The dialog: The dialog asks again while an agent still lists its models

While an agent of the list still lists its models and modes (`listsLoading`), the dialog MUST say "Some agents are
still listing their models and modes…" and ask the IDE for its agents again after 3 s, at most 4 times.

#### Scenario: An agent still lists its models
- **WHEN** an agent of the list has `listsLoading`
- **THEN** the dialog says "Some agents are still listing their models and modes…"
- **AND** it asks the IDE for its agents again after 3 s, at most 4 times
  [@test] src/lib/air-launch.test.ts (`is listing while an agent still lists its models`)

### Requirement: The dialog: A newer list keeps the choice made in the dialog

A newer list MUST replace the old one only when the IDE lists its agents. The pickers MUST keep the choice made in the
dialog while the newer list offers it; a target that was still listing takes its remembered model, effort, mode and
Plan.

#### Scenario: The newer list offers the choice of the dialog
- **WHEN** a newer list comes after the user chose in the dialog
- **THEN** the pickers keep the agent, the model, the effort and the start chosen while the newer list offers them
  [@test] src/lib/air-launch.test.ts (`a newer list keeps the agent, the model, the effort and the start chosen in the dialog while it offers them`)

#### Scenario: A target that was still listing
- **WHEN** a newer list gives the models of a target that was still listing
- **THEN** the target takes its remembered model and effort, with the start of the dialog
  [@test] src/lib/air-launch.test.ts (`a newer list gives a target that was still listing its remembered model and effort, with the start of the dialog`)

### Requirement: The dialog: Start sends the choice, the prompt and the title

Start (⌘↵; "Open Draft" for a draft) MUST send the target, the model id, the effort, the mode, Plan, the plan effort,
the prompt and the title "Fix the Safe Push of auth".

#### Scenario: The launch of the form
- **WHEN** the user presses Start (⌘↵)
- **THEN** the launch sends the target, the model id, the effort, the mode, Plan, the plan effort, the prompt and the
  title "Fix the Safe Push of auth"
  [@test] src/lib/air-launch.test.ts (`sends the target, the model, the effort, the prompt and the title of the form`)

#### Scenario: The commands of the targets and the launch
- **WHEN** the page asks for the targets and sends the launch
- **THEN** it gets the answer of each command, and an error of the command comes with its code
  [@test] src/lib/air-launch.test.ts (`answers the targets and the launch, and throws an error of the command with its code`)

### Requirement: The dialog: A draft sends no blank prompt and Run refuses one

A draft with a blank prompt MUST send none, and Run MUST refuse a blank prompt.

#### Scenario: A draft with a blank prompt
- **WHEN** the user starts a draft ("Open Draft") with a blank prompt
- **THEN** the launch sends no prompt
  [@test] src/lib/air-launch.test.ts (`sends no blank prompt of a draft`)

#### Scenario: Run with a blank prompt
- **WHEN** the start is Run and the prompt is blank
- **THEN** Start does not run the session
  [@test] src/lib/air-launch.test.ts (`runs no blank prompt, and sends again only after a change of what the IDE refused`)

### Requirement: The dialog: An accepted launch closes the dialog and remembers the choice

An accepted launch MUST close the dialog, remember the choice in the recent project (`airLaunch`) and nothing else.

#### Scenario: The IDE accepts the launch
- **WHEN** the IDE accepts the launch
- **THEN** the dialog closes
- **AND** the target, the model, the effort, the mode, Plan, the plan effort and the start are remembered in the recent
  project (`airLaunch`)
  [@test] src/lib/air-launch.test.ts (`remembers the target, the model, the effort, the mode, Plan, the plan effort and the start`)

### Requirement: The dialog: A launch without a session id says it started

A draft has no session to follow: a toast MUST say "Opened a draft for Codex in Air", with the title of the session
and the IDE.

#### Scenario: A draft is accepted without a session id
- **WHEN** the IDE opens a draft
- **THEN** a toast says "Opened a draft for Codex in Air"

### Requirement: The dialog: A refusal stays in the dialog in its words

A refusal MUST stay in the dialog in its words, the message of the IDE only for `LAUNCH_FAILED` and an unknown code. A
refusal of the consent (`DENIED`, `UNAVAILABLE`, `BUSY`, `RATE_LIMITED`), `NO_PROJECT` and `NO_DEFAULT` MUST say what
happened and what the user can do.

#### Scenario: The words of every refusal code
- **WHEN** the IDE refuses the launch
- **THEN** the dialog words every refusal code of the IDE
- **AND** it shows the message of the IDE only for `LAUNCH_FAILED` and an unknown code
  [@test] src/lib/air-launch.test.ts (`words every refusal code of the IDE, the message of a failed launch, and an unknown code with its message`)

### Requirement: The dialog: Start sends again only after a change

After a refusal, Start MUST send again only after a change of a picker or the prompt.

#### Scenario: The user changes what the IDE refused
- **GIVEN** the IDE refused the launch
- **WHEN** the user presses Start without a change
- **THEN** nothing is sent
- **AND** after a change of a picker or the prompt, Start sends again
  [@test] src/lib/air-launch.test.ts (`runs no blank prompt, and sends again only after a change of what the IDE refused`)

### Requirement: The dialog: An error of the exchange says it cannot start the session

An error of the exchange of the launch MUST be the error toast "Cannot start the session in Air" with the reason
(`notifyError`); the dialog stays open with its choice and prompt.

#### Scenario: The exchange fails
- **WHEN** the launch ends with an error of the exchange
- **THEN** an error toast says "Cannot start the session in Air" with the reason
- **AND** the dialog stays open, and Start can send again

### Requirement: The session toast: The toast follows the session in place

With a local session id, the toast "Started Codex in Air" MUST follow the row of the session in place: "Codex works in
Air…" (`processing`, an activity of a later IDE, and none), "Codex waits for you in Air" (`needs_input`, `draft`),
"Codex waits for your review in Air" (`reviewing`), with the title of the session.

#### Scenario: The words of each activity
- **WHEN** the session reports an activity
- **THEN** `processing`, an activity of a later IDE and none read "Codex works in Air…"
- **AND** `needs_input` and `draft` read "Codex waits for you in Air", `reviewing` reads "Codex waits for your review
  in Air"
- **AND** the row names its activity in `_meta["x-air-ij"].activity`, the name of `AgentSessionActivity` of the IDE
  (`NEEDS_INPUT`), read in any case and with or without `_`; the top-level `activity` of a row is not read
  [@test] src/lib/air-launch.test.ts (`words the activity as working, waiting for the user or done, and a later activity as working`)
  [@test] crates/air-launch/src/protocol_test.rs (`the_activity_of_a_row_is_read_in_any_case_with_or_without_underscores`, `a_row_names_its_activity_in_the_facts_of_air_of_its_meta`)

#### Scenario: The toast follows each status in place
- **WHEN** a launch is accepted with a local session id
- **THEN** the toast "Started Codex in Air" watches the session and shows each status in place, with the title of the
  session
  [@test] src/lib/air-session-toast.test.ts (`says it started, follows each status in place, and stops the watch once the session is done`)

### Requirement: The session toast: A done session stops the watch and closes the toast

At `ready` or `unread` the toast MUST say "Codex is done in Air", stop the watch and close after 5 s.

#### Scenario: The session is done
- **WHEN** the session reaches `ready` or `unread`
- **THEN** the toast says "Codex is done in Air" and stops the watch
- **AND** it closes after 5 s
  [@test] src/lib/air-launch.test.ts (`words the activity as working, waiting for the user or done, and a later activity as working`)
  [@test] src/lib/air-session-toast.test.ts (`says it started, follows each status in place, and stops the watch once the session is done`)

### Requirement: The session toast: The toast says how the watch ended

A removed row MUST read "The session of Codex is gone in Air"; a close, no IDE, a refusal and an error MUST say that the
toast stopped following, and why.

#### Scenario: The session is gone
- **WHEN** the watch ends because the row of the session is removed
- **THEN** the toast says "The session of Codex is gone in Air"
  [@test] src/lib/air-session-toast.test.ts (`says the session is gone, and a close of the toast stops the watch`)

#### Scenario: The end of a watch
- **WHEN** the watch ends with a close, a refusal or an error
- **THEN** the toast says that it stopped following, and why
- **AND** it says nothing for a watch the page stopped
  [@test] src/lib/air-launch.test.ts (`words the end of a watch, and nothing for a watch the page stopped`)

### Requirement: The session toast: A close of the toast stops the watch

A close of the toast MUST stop the watch.

#### Scenario: The user closes the toast
- **WHEN** the user closes the toast while the watch runs
- **THEN** the watch stops
  [@test] src/lib/air-session-toast.test.ts (`says the session is gone, and a close of the toast stops the watch`)

### Requirement: The session toast: One watch per window and session

The app MUST hold one watch per window and session (`air_watch_session`), and one connection per IDE for all of them: a
newer watch of the session stops the one before, `stop_air_watch` stops it, the page of its window that goes away stops
its watches, and the last watch of an IDE that ends closes its connection.

#### Scenario: A stop, a newer watch and a page that went away
- **WHEN** `stop_air_watch` names the session, a newer watch of the session starts, or the page of the window goes away
- **THEN** the watch before ends
- **AND** a newer watch keeps the connection of the IDE, and the stop of the last watch closes it
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_stop_a_newer_watch_of_the_session_and_a_page_that_went_away_end_the_watches_of_a_window`)

#### Scenario: A page that went away
- **WHEN** the page of the window went away (a reload, a close)
- **THEN** its watch stops at its first status, though its Tauri channel never fails its send
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_stop_a_newer_watch_of_the_session_and_a_page_that_went_away_end_the_watches_of_a_window`)

#### Scenario: The page watches a session and stops it
- **WHEN** the page watches a session through `air_watch_session`
- **THEN** it gets each status, then the end
- **AND** its stop names the local session id to `stop_air_watch`
  [@test] src/lib/air-launch.test.ts (`watches a session: each status to the callback, then the end; the stop names the session`)

### Requirement: The session toast: A watch tells the page each status and how it ended

A watch MUST tell the page the status of the row whose `resource` is its local session id, now and after each change
of its title or activity, and how it ended: `noIde`, `refused` with its code (the connection or the list), `gone` (the
row is removed), `closed`, or `stopped`. A watch whose row is not there yet MUST wait for it.

#### Scenario: Each status, then the end
- **WHEN** a watch runs to its end
- **THEN** the page gets each status, then `refused` with its code, `gone`, `closed`, or `stopped`
- **AND** a change of another row, or of another field of the row, tells the page nothing
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_watch_tells_the_page_the_status_of_its_row_now_and_after_each_change_then_that_it_is_gone`, `a_refusal_of_the_list_ends_each_watch_refused`)

#### Scenario: The IDE closes the connection
- **WHEN** the IDE closes the connection
- **THEN** each watch of the IDE ends `closed`
- **AND** the watches of one IDE share its connection, and a watch of a row that comes later follows it
  [@test] crates/app-api/src/app/air_launch_test.rs (`the_watches_of_one_ide_share_one_connection_and_a_row_that_comes_later_is_followed`)
