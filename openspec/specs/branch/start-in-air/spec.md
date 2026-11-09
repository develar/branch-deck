---
name: Start in Air
description: Requirements for "Start in Air…" of a failed robot — the dialog that starts the prompt of the robot as an agent session in the IDE with Air that has the repository open, over the local launch socket of Air (the local peer handshake v2 and the requests of protocol v1, IJAI-5465) — the discovery of the IDE, the agent, model, effort, mode, Plan and start pickers, the choice remembered per repository, the refusals of the IDE, and the toast that follows the started session.
style: plain-1
targets:
  - crates/air-launch/src/lib.rs
  - crates/air-launch/src/discovery.rs
  - crates/air-launch/src/client.rs
  - crates/air-launch/src/protocol.rs
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
agent plugin of the JetBrains IDEs, has a local launch socket: a client names the project, the agent, the model and
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
- A poll of the session status: one watch counts once against the rate limit of the IDE (10 connections in 60 s).

## Requirements

### Requirement: The IDE: Each IDE with Air is found by its instance file

Each IDE process with Air writes one instance file, `<pid>.json`, in `~/Library/Application Support/JetBrains/air/launch`
(macOS), `%APPDATA%\JetBrains\air\launch` (Windows), `$XDG_DATA_HOME/JetBrains/air/launch` or
`~/.local/share/JetBrains/air/launch` (Linux). A file whose process does not run, a file that does not parse and a file
of another `schemaVersion` MUST be skipped. A directory that does not exist MUST read as no instance, not as an error.

#### Scenario: The files of running processes are read, the others skipped
- **GIVEN** the directory of instance files of the platform
- **WHEN** Branch Deck reads it
- **THEN** it reads the file of each running IDE process
- **AND** it skips a file whose process does not run, a file that does not parse and a file of another `schemaVersion`
  [@test] crates/air-launch/src/discovery_test.rs (`reads_the_files_of_running_processes_and_skips_dead_garbled_and_later_ones`, `find_instance_checks_the_processes_of_the_files`)

#### Scenario: No directory of instance files
- **WHEN** Branch Deck reads a directory of instance files that does not exist
- **THEN** it reads no instance, and no error
  [@test] crates/air-launch/src/discovery_test.rs (`no_directory_is_no_instance`)

### Requirement: The IDE: The IDE of the repository has its directory or a parent open

The IDE of the repository MUST be the one whose `projects` holds the directory of the repository or one of its
parents, compared by path components; the deepest project first, then the newest `focusedAt` (the last focus of a
window of that IDE, Unix milliseconds; a file without it is older than any), then the newest file.

#### Scenario: A parent is open and the deepest project wins
- **WHEN** IDEs have the directory of the repository or one of its parents open, compared by path components
- **THEN** the IDE of the deepest project is the IDE of the repository
  [@test] crates/air-launch/src/discovery_test.rs (`the_ide_of_a_directory_has_it_or_a_parent_open_and_the_deepest_project_wins`)

#### Scenario: Of two IDEs with the same project the newest file wins
- **WHEN** two IDEs have the same project open and neither file has a `focusedAt`
- **THEN** the IDE of the newest file is picked
  [@test] crates/air-launch/src/discovery_test.rs (`of_two_ides_with_the_same_project_the_newest_file_wins`)

#### Scenario: Of two IDEs with the same project the last focused wins
- **WHEN** two IDEs have the same project open
- **THEN** the one with the newest `focusedAt` is picked
- **AND** a file without `focusedAt` is older than any
  [@test] crates/air-launch/src/discovery_test.rs (`of_two_ides_with_the_same_project_the_last_focused_wins_and_one_without_a_focus_is_older_than_any`)

#### Scenario: The deepest project wins over the last focus
- **WHEN** one IDE has a deeper project and another the newer `focusedAt`
- **THEN** the IDE of the deeper project is picked
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

Each request MUST take one connection. The client runs the local peer handshake v2 first: it writes the opener
`{"proto":"local-peer","v":2}`, reads the greeting `{"proto":"local-peer","v":2,"nonce":"…"}` within 10 s
(`HANDSHAKE_TIMEOUT`, and never later than the timeout of the response), and writes the envelope
`{"nonce":"<the echo>","request":{…}}`. The request object is the request of v1 (`"v":1`, the method and its fields).

#### Scenario: The opener, then the envelope with the echo of the nonce
- **WHEN** the client sends a request
- **THEN** it writes the opener and reads the greeting
- **AND** it writes the envelope with the echo of the nonce and the request object of v1, on one line
  [@test] crates/air-launch/src/client_test.rs (`the_handshake_writes_the_opener_then_the_envelope_with_the_echo_of_the_nonce_and_the_request`)
  [@test] crates/air-launch/src/protocol_test.rs (`the_envelope_holds_the_echo_of_the_nonce_and_the_request_object_on_one_line`)

### Requirement: The exchange: One line of response within 120 s, then the close

After the envelope the client MUST read one line within 120 s (`RESPONSE_TIMEOUT`: the consent dialog and the launch),
and close the connection.

#### Scenario: The IDE does not answer in time
- **WHEN** the IDE sends no line within the timeout
- **THEN** the request fails with a timeout, an error of the command
  [@test] crates/air-launch/src/client_test.rs (`an_ide_that_does_not_answer_in_time_is_a_timeout`)

### Requirement: The exchange: Limits and bad answers are errors of the command

A request longer than 1 MiB in its envelope MUST NOT be sent; a response longer than 1 MiB, one that is not JSON or
has an unknown `status`, a close without an answer, and no answer in time MUST be errors of the command. A socket
nobody listens on (the instance file names a socket that is gone) MUST be a connect error, also an error of the command.

#### Scenario: A request longer than the limit
- **WHEN** the envelope of a request is longer than 1 MiB
- **THEN** the request is not sent
  [@test] crates/air-launch/src/client_test.rs (`a_request_longer_than_the_limit_is_not_sent`)

#### Scenario: A response longer than the limit
- **WHEN** the response is longer than 1 MiB
- **THEN** it is an error of the command
  [@test] crates/air-launch/src/client_test.rs (`a_response_longer_than_the_limit_is_refused`)

#### Scenario: A response that is not JSON or has an unknown status
- **WHEN** the response is not JSON or has an unknown `status`
- **THEN** it is an error of the command, a bad response
  [@test] crates/air-launch/src/client_test.rs (`a_garbled_response_or_an_unknown_status_is_a_bad_response`)

#### Scenario: A close without an answer
- **WHEN** the IDE closes the connection without an answer
- **THEN** it is an error of the command
  [@test] crates/air-launch/src/client_test.rs (`a_close_without_an_answer_is_closed_and_a_line_without_its_end_is_read_whole`)

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
  [@test] crates/air-launch/src/client_test.rs (`the_targets_round_trip_names_the_project_and_reads_the_agents_with_their_models_modes_and_whether_they_still_load`)

#### Scenario: A target of an older IDE
- **WHEN** a target comes without `listsLoading`
- **THEN** its lists are read as loaded
  [@test] crates/air-launch/src/protocol_test.rs (`a_target_without_lists_loading_has_its_lists`)

### Requirement: The exchange: A launch sends the mode, Plan and plan effort only when set

A launch MUST send `mode`, `plan` and `planEffort` only when set.

#### Scenario: An accepted launch sends every field
- **WHEN** a launch with every field set is accepted
- **THEN** it sends every field and reads the session id
  [@test] crates/air-launch/src/client_test.rs (`an_accepted_launch_sends_every_field_and_reads_the_session_id`)

#### Scenario: A draft without a model, an effort, a mode, Plan and a prompt
- **WHEN** a launch has no model, no effort, no mode, no Plan and no prompt
- **THEN** it leaves them out
  [@test] crates/air-launch/src/client_test.rs (`a_draft_without_a_model_an_effort_a_mode_plan_and_a_prompt_leaves_them_out`)

### Requirement: The exchange: A refusal is an answer, never an error

A refusal (`"status":"refused"`) MUST be an answer with its code and message, never an error; a code of a later
protocol stays as its text.

#### Scenario: A refusal of a launch or of the targets
- **WHEN** the IDE refuses a launch or the targets
- **THEN** the answer is the refusal with its code and message
  [@test] crates/air-launch/src/client_test.rs (`a_refusal_of_a_launch_or_of_the_targets_reads_its_code_and_message`)

#### Scenario: A refusal code of a later protocol
- **WHEN** the code of a refusal is one of a later protocol
- **THEN** it stays as its text
  [@test] crates/air-launch/src/client_test.rs (`a_refusal_code_of_a_later_protocol_stays_as_its_text`)

#### Scenario: Every refusal code of protocol v1
- **WHEN** a refusal names a code of protocol v1
- **THEN** the code reads back as its text
  [@test] crates/air-launch/src/protocol_test.rs (`every_refusal_code_of_protocol_v1_reads_back_as_its_text`)

### Requirement: The exchange: A refusal of the handshake is an error of the command

A refusal of the handshake (`{"proto":"local-peer","v":2,"error":"…"}`: `BAD_OPENER`, `BAD_NONCE`, `PEER_CHANGED`, a
code of a later handshake as its text) MUST be an error of the command: the IDE did not read the request. It comes
instead of the greeting or as the first line after the envelope, also of a watch.

#### Scenario: The refusal comes instead of the greeting or after the envelope
- **WHEN** the IDE refuses the handshake instead of the greeting, or as the first line after the envelope of a request
  or of a watch
- **THEN** it is a typed error of the command
  [@test] crates/air-launch/src/client_test.rs (`a_refusal_of_the_handshake_is_a_typed_error_of_the_exchange_and_of_a_watch`)

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
  [@test] crates/air-launch/src/client_test.rs (`a_greeting_of_another_version_or_without_a_nonce_is_a_bad_response_and_no_greeting_in_time_is_a_timeout`)

#### Scenario: A line of local-peer is a greeting or a refusal
- **WHEN** the client reads a line during the handshake
- **THEN** a line of local-peer is a greeting, a refusal, or a bad greeting (another version, no nonce, a nonce that is
  not base64url without padding)
- **AND** any other line is the application's
  [@test] crates/air-launch/src/protocol_test.rs (`a_line_of_local_peer_is_a_greeting_or_a_refusal_and_any_other_line_is_the_applications`)

### Requirement: The exchange: An IDE before handshake v2 answers the opener

An IDE before handshake v2 reads the opener as a request and answers it at once (`UNSUPPORTED_VERSION`): that line MUST
be the answer of the request or the first line of the watch, and the client MUST send no envelope.

#### Scenario: The opener is answered at once
- **WHEN** an IDE before handshake v2 answers the opener with `UNSUPPORTED_VERSION`
- **THEN** that line is the response of the request, or the first line of the watch
- **AND** the client sends no envelope
  [@test] crates/air-launch/src/client_test.rs (`an_ide_before_handshake_v2_answers_the_opener_and_its_answer_is_the_response`)

### Requirement: The exchange: A watch reads each status of the session until its end

A watch (`watch`, the project and the session id) MUST hold its connection without a timeout of the client: the IDE
sends the status of the session (its title and activity) now and after each change, and ends with `end` (`timeout`
after 4 h), with the refusal `NO_SESSION`, or with a close. Nothing MUST be read after the end.

#### Scenario: Each status until the end
- **WHEN** the client watches a session
- **THEN** the watch names the session
- **AND** it reads each status until `end`, and nothing after it
  [@test] crates/air-launch/src/client_test.rs (`a_watch_names_the_session_and_reads_each_status_until_the_end`)

#### Scenario: The session is gone, or the IDE closes the connection
- **WHEN** the IDE answers the watch with `NO_SESSION`, or closes the connection after a status
- **THEN** the watch ends with the refusal `NO_SESSION`, or, at the close, without another line
  [@test] crates/air-launch/src/client_test.rs (`a_watch_of_a_session_that_is_gone_ends_with_no_session_and_a_close_ends_it_without_a_line`)

#### Scenario: A watch waits without a timeout
- **WHEN** the session sends no status for long
- **THEN** the watch waits, with no timeout of the client
  [@test] crates/air-launch/src/client_test.rs (`a_watch_waits_without_a_timeout_and_its_stopper_ends_it_from_another_thread`)

#### Scenario: A garbled line of a watch
- **WHEN** a line of a watch does not parse
- **THEN** it is a bad response and ends the watch
  [@test] crates/air-launch/src/client_test.rs (`a_garbled_line_of_a_watch_is_a_bad_response_and_ends_it`)

### Requirement: The exchange: A refusal of v1 can come before the first status

A refusal of v1 SHALL be read when it comes before the first status of a watch (`BUSY` when 8 watches are open).

#### Scenario: Eight watches are open
- **WHEN** the IDE answers a watch with `BUSY` (8 watches are open) before the first status
- **THEN** the watch reads it as a refusal and ends
  [@test] crates/air-launch/src/client_test.rs (`a_watch_of_a_session_that_is_gone_ends_with_no_session_and_a_close_ends_it_without_a_line`)

### Requirement: The exchange: A stop from another thread ends a watch

A stop from another thread MUST close the connection of a watch, and the read it ends MUST give no line and no error
on every platform (Windows refuses a read after the shutdown, where Unix reads the end of the stream).

#### Scenario: The stopper ends the watch from another thread
- **WHEN** another thread stops a watch that waits for a status
- **THEN** the connection closes and the read gives no line and no error
  [@test] crates/air-launch/src/client_test.rs (`a_watch_waits_without_a_timeout_and_its_stopper_ends_it_from_another_thread`)

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
agent, its default model, the default effort, the default mode, Plan off and Run. A choice remembered by an older
version has no mode and no Plan.

#### Scenario: Nothing remembered
- **WHEN** the dialog opens with nothing remembered for the repository
- **THEN** the pickers start with the first agent, its default model, the default effort and Run
  [@test] src/lib/air-launch.test.ts (`starts with the first agent, its default model, the default effort and Run`)

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
- **WHEN** the IDE offers no agents
- **THEN** the dialog has no form of pickers
  [@test] src/lib/air-launch.test.ts (`has no model and no effort for an agent without models, and no form without agents`)

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

Without a session id (a draft, an agent of a terminal that has no id yet) a toast MUST say "Started Codex in Air" for
Run and "Opened a draft for Codex in Air" for a draft, with the title of the session and the IDE.

#### Scenario: Run is accepted without a session id
- **WHEN** the IDE accepts a launch with Run and answers no session id
- **THEN** a toast says "Started Codex in Air"

#### Scenario: A draft is accepted without a session id
- **WHEN** the IDE accepts a launch with Draft and answers no session id
- **THEN** a toast says "Opened a draft for Codex in Air"

### Requirement: The dialog: A refusal stays in the dialog in its words

A refusal MUST stay in the dialog in its words, the message of the IDE only for `LAUNCH_FAILED` and an unknown code.

#### Scenario: The words of every refusal code
- **WHEN** the IDE refuses the launch
- **THEN** the dialog words every code of protocol v1
- **AND** it shows the message of the IDE only for `LAUNCH_FAILED` and an unknown code
  [@test] src/lib/air-launch.test.ts (`words every code of protocol v1, the message of a failed launch, and an unknown code with its message`)

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

With a session id, the toast "Started Codex in Air" MUST follow the session through `watch` in place: "Codex works in
Air…" (`processing`, and an activity of a later protocol), "Codex waits for you in Air" (`needs_input`, `draft`),
"Codex waits for your review in Air" (`reviewing`), with the title of the session.

#### Scenario: The words of each activity
- **WHEN** the session reports an activity
- **THEN** `processing` and an activity of a later protocol read "Codex works in Air…"
- **AND** `needs_input` and `draft` read "Codex waits for you in Air", `reviewing` reads "Codex waits for your review
  in Air"
  [@test] src/lib/air-launch.test.ts (`words the activity as working, waiting for the user or done, and a later activity as working`)

#### Scenario: The toast follows each status in place
- **WHEN** a launch is accepted with a session id
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

`NO_SESSION` MUST read "The session of Codex is gone in Air"; the end after 4 h, a close, no IDE, a refusal and an
error MUST say that the toast stopped following, and why.

#### Scenario: The session is gone
- **WHEN** the watch ends with `NO_SESSION`
- **THEN** the toast says "The session of Codex is gone in Air"
  [@test] src/lib/air-session-toast.test.ts (`says the session is gone, and a close of the toast stops the watch`)

#### Scenario: The end of a watch
- **WHEN** the watch ends after 4 h, with a close, a refusal or an error
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

The app MUST hold one watch per window and session (`air_watch_session`): a newer watch of the session stops the one
before, `stop_air_watch` stops it, and the page of its window that goes away (a reload, a close) stops its watches, as
a Tauri channel of such a page never fails its send.

#### Scenario: A stop, a newer watch and a page that went away
- **WHEN** `stop_air_watch` names the session, a newer watch of the session starts, or the page of the window goes away
- **THEN** the watch before ends
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_stop_a_newer_watch_of_the_session_and_a_page_that_went_away_end_the_watches_of_a_window`)

#### Scenario: A page that went away
- **WHEN** the page of the window went away (a reload, a close)
- **THEN** its watch stops at its first status, though its Tauri channel never fails its send
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_page_that_went_away_stops_the_watch_at_its_first_status`)

#### Scenario: The page watches a session and stops it
- **WHEN** the page watches a session through `air_watch_session`
- **THEN** it gets each status, then the end
- **AND** its stop names the session to `stop_air_watch`
  [@test] src/lib/air-launch.test.ts (`watches a session: each status to the callback, then the end; the stop names the session`)

### Requirement: The session toast: A watch tells the page each status and how it ended

A watch MUST tell the page each status and how it ended: `noIde` (no IDE with Air has the repository open), `refused`
with its code, `ended` with its reason, `closed`, or `stopped`.

#### Scenario: Each status, then the end
- **WHEN** a watch runs to its end
- **THEN** the page gets each status, then `refused` with its code, `ended` with its reason, `closed`, or `stopped`
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_watch_tells_the_page_each_status_then_how_it_ended`)

#### Scenario: A close the page stopped
- **WHEN** the connection closes
- **THEN** the end is `closed`, unless the page stopped the watch, which is `stopped`
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_close_is_closed_unless_the_page_stopped_the_watch`)
