---
name: Start in Air
description: Requirements for "Start in Air" of a failed robot, which opens the New Session tab of Air with the fast prompt in the IDE that has the repository open, over the local AHP socket of Air as a local app (the local peer handshake v2, then JSON-RPC of `ahp-root://`, IJAI-7153): the discovery of the IDE, the draft with the project and the prompt alone, the refusals of the IDE and the toast of the answer.
style: plain-1
targets:
  - crates/air-launch/src/lib.rs
  - crates/air-launch/src/discovery.rs
  - crates/air-launch/src/client.rs
  - crates/air-launch/src/protocol.rs
  - crates/app-api/src/app/air_launch.rs
  - src/lib/air-launch.ts
  - src/lib/robot-prompt-copy.ts
---

# Start in Air

Status: Implemented
Date: 2026-11-27

## Purpose

"Copy Prompt for Agent" puts the failure of a robot on the clipboard, and the user pastes it into an agent. Air, the
agent plugin of the JetBrains IDEs, has a local AHP socket: a local app names the project and the prompt, and Air opens
its New Session tab with them after the user agreed in the IDE. "Start in Air" sends the same prompt there in one click,
and the user picks the agent, the model and the rest in the composer of Air.

## Goals

- From a failed robot to the New Session tab of Air with the prompt in one click, without the clipboard.
- One place to pick the agent, the model, the effort and the mode: the composer of Air.
- When no IDE answers, the prompt is still one click away.

## Non-goals

- A consent or an identity of the client: the IDE asks the user and names the peer process by the kernel.
- A session in an IDE that does not have the repository open: the IDE refuses it (`NO_PROJECT`).
- The prompt with the CI details: Start in Air sends the fast prompt, which tells the agent how to read the rest.
- A pick of the agent in Branch Deck, a start of the session, or a watch of it: the user does them in Air.

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
  [@test] tests/e2e/patronus/robot-prompt.spec.ts (`should say that no IDE with Air has the repository open on Start in Air, and copy the prompt from its toast`)

### Requirement: The exchange: One connection per request, after the local peer handshake v2

Each request MUST take one connection. The client runs the local peer handshake v2 first: it writes the
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
- **WHEN** the client replays the frames of `initialize`, of the hosts and of a draft that a local app sends and gets in
  the corpus of the IDE (`crates/air-launch/fixtures/local-ahp-apps.ndjson`), the draft with the project and the prompt
  alone
- **THEN** it reads each answer and the refusal `UNAUTHORIZED` of `initialize`
  [@test] crates/air-launch/src/client_test.rs (`the_local_app_side_of_the_corpus_of_the_ide_replays`)

### Requirement: The exchange: The answer of each request within 120 s

Each request after `initialize` MUST have its own id, and the client MUST read frames until the answer of that id
within 120 s (`RESPONSE_TIMEOUT`: the consent dialog and the New Session tab). A notice, the answer of another id and a
request of the IDE that come before the answer MUST be skipped.

#### Scenario: The IDE does not answer in time
- **WHEN** the IDE sends no answer of the id within the timeout
- **THEN** the request fails with a timeout, an error of the command
  [@test] crates/air-launch/src/client_test.rs (`an_ide_that_does_not_answer_in_time_is_a_timeout`)

#### Scenario: A notice, the answer of another id and a request of the IDE before the answer
- **WHEN** a notice, the answer of another id or a request of the IDE comes before the answer of the request
- **THEN** the client skips each, and reads the answer
  [@test] crates/air-launch/src/client_test.rs (`a_notice_the_answer_of_another_id_and_a_request_of_the_ide_before_the_answer_are_skipped`)
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

### Requirement: The exchange: A refusal is an answer, never an error

An error of the IDE MUST be a refusal with the code of `error.data.code` (`DENIED`), and the message and the hint of
`error.data`, never an error of the command. A code this client does not know stays as its text.

#### Scenario: A refusal of a draft
- **WHEN** the IDE refuses a draft or `initialize`
- **THEN** the answer is the refusal with its code, message and hint
  [@test] crates/air-launch/src/client_test.rs (`the_local_app_side_of_the_corpus_of_the_ide_replays`, `a_draft_names_only_the_project_and_the_prompt_and_reads_each_refusal`)

#### Scenario: An error without a code of its data
- **WHEN** an error of the IDE has no `data.code`
- **THEN** its code is the code of its number (`UNKNOWN_METHOD`, `BAD_PARAMS`, `BAD_REQUEST`, else `ERROR_<number>`)
  [@test] crates/air-launch/src/client_test.rs (`an_error_without_a_code_of_its_data_reads_the_code_of_its_number`)
  [@test] crates/air-launch/src/protocol_test.rs (`a_refusal_reads_the_code_message_and_hint_of_the_data_else_the_code_of_the_number`)

### Requirement: The exchange: A refusal of the handshake is an error of the command

A refusal of the handshake (`{"proto":"local-peer","v":2,"error":"…"}`: `BAD_OPENER`, `BAD_NONCE`, `PEER_CHANGED`, a
code of a later handshake as its text) MUST be an error of the command: the IDE did not read the request. It comes
instead of the greeting or as the first line after the envelope.

#### Scenario: The refusal comes instead of the greeting or after the envelope
- **WHEN** the IDE refuses the handshake instead of the greeting, or as the first line after the envelope of a request
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

### Requirement: The exchange: A draft names only the project and the prompt

A draft MUST be `x-air-ij/openDraft` with `projectDirectory` and `prompt` alone, and no route, model, effort, mode or
Plan, so the IDE takes the default agent of its New Session button. Its answer is `{}`, and the IDE sends nothing to
the agent.

#### Scenario: The IDE opens the tab, or refuses it
- **WHEN** the client opens a draft
- **THEN** the request names the project and the prompt alone, and the answer `{}` is the open tab
- **AND** a refusal (`DENIED`, `NO_DEFAULT`) is read with its code, message and hint
  [@test] crates/air-launch/src/client_test.rs (`a_draft_names_only_the_project_and_the_prompt_and_reads_each_refusal`)
  [@test] crates/app-api/src/app/air_launch_test.rs (`the_ide_of_the_repository_opens_the_new_session_tab_with_the_project_and_the_prompt_alone`)

#### Scenario: No IDE of the repository
- **WHEN** the host has no directory of instance files, or no IDE has the repository or a parent of it open
- **THEN** `air_open_draft` answers `noIde`, and sends no draft
  [@test] crates/app-api/src/app/air_launch_test.rs (`no_directory_of_instance_files_and_no_ide_of_the_repository_are_no_ide`)

#### Scenario: A refusal and a close without an answer
- **WHEN** the IDE refuses the draft, or closes the connection without an answer
- **THEN** `air_open_draft` answers `refused` with the IDE, the code and the message
- **AND** a close without an answer is an error of the command
  [@test] crates/app-api/src/app/air_launch_test.rs (`a_refusal_of_the_ide_is_an_answer_and_a_close_without_an_answer_an_error`)

### Requirement: The action: A failed robot with Copy Prompt for Agent has Start in Air

A failed or failing robot that has "Copy Prompt for Agent" MUST also have "Start in Air", before the prompts and after
the restart or the cancel, with the same blocker, in every form of the robot's actions: it is the primary way to fix
the failure (it opens the New Session tab in the IDE that has the repository open), the prompts are for a user who
copies them into an agent elsewhere.

#### Scenario: The actions of a failed robot
- **GIVEN** a failed robot that has "Copy Prompt for Agent"
- **WHEN** its actions are listed
- **THEN** "Start in Air" stands after the restart and before the prompts
- **AND** it has the same blocker as "Copy Prompt for Agent"
  [@test] crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)

### Requirement: The action: The robot details show Start in Air as the primary button

The robot details MUST show "Start in Air" as the primary button, the prompts beside it.

#### Scenario: The details of a failed robot
- **WHEN** the user opens the details of a failed robot that has "Start in Air"
- **THEN** "Start in Air" is the primary button and the prompts stand beside it
  [@test] tests/e2e/patronus/robot-prompt.spec.ts (`should offer both prompts in the details of the failed robot, the one with the CI details in the menu of its split button`)

### Requirement: The action: Start in Air opens a prefilled New Session tab with the fast prompt

"Start in Air" MUST read the fast prompt of the robot and send it with the directory of the repository to the IDE
(`air_open_draft`), with no dialog of Branch Deck. While the IDE answers, the toast "Opening in Air…" MUST stay with no
timeout, as the IDE can ask the user first; then it MUST say the answer in place.

#### Scenario: The IDE opens the tab
- **WHEN** the user chooses "Start in Air" of the failed robot of `auth`
- **THEN** the toast "Opening in Air…" / "New Session for auth" shows with no timeout
- **AND** the page sends the fast prompt and the directory of the repository alone
- **AND** once the IDE opened the tab, the toast turns in place into "Opened in IU-271.1" / "New Session for auth"
  [@test] src/lib/robot-prompt-copy.test.ts (`says it opens while the IDE may ask first, sends the fast prompt alone, then says the IDE opened the New Session tab`)
  [@test] src/lib/air-launch.test.ts (`sends the project and the prompt alone, answers the answer of the IDE, and throws an error of the command with its code`, `names the IDE by its build, else by its product`)

### Requirement: The action: No IDE, a refusal and an error are said in the toast

No IDE MUST be the toast "No IDE with Air has idea open" with "Copy Prompt", which copies the fast prompt. A refusal
MUST be the error toast "Cannot open the New Session for auth in Air" in the words of its code, and an error of the
command MUST go through `notifyError`.

#### Scenario: No IDE has the repository open
- **WHEN** no IDE with Air has the repository open
- **THEN** the toast says "No IDE with Air has idea open" with "Copy Prompt"
- **AND** "Copy Prompt" closes it, copies the fast prompt and says "Copied the prompt for auth"
  [@test] src/lib/robot-prompt-copy.test.ts (`says no IDE has the repository open, and its Copy Prompt copies the fast prompt`)
  [@test] tests/e2e/patronus/robot-prompt.spec.ts (`should say that no IDE with Air has the repository open on Start in Air, and copy the prompt from its toast`)

#### Scenario: The IDE refuses, or the command fails
- **WHEN** the IDE refuses the draft, or the command ends with an error
- **THEN** a refusal is the error toast "Cannot open the New Session for auth in Air" in the words of its code
- **AND** an error of the command is the same title through `notifyError`, with the reason
  [@test] src/lib/robot-prompt-copy.test.ts (`says a refusal of the IDE in its words, and an error of the command as an error`)

#### Scenario: The words of every refusal code
- **WHEN** the IDE refuses a draft
- **THEN** the toast words every refusal code of a draft
- **AND** it shows the message of the IDE only for `LAUNCH_FAILED` and an unknown code
  [@test] src/lib/air-launch.test.ts (`words every refusal code of a draft, the message of a failed open, and an unknown code with its message`)
