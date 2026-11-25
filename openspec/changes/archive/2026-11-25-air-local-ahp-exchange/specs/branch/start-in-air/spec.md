## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: The IDE: Each IDE with Air is found by its instance file

Each IDE process with Air that admits local apps writes one instance file, `<pid>.json`, in
`~/Library/Application Support/JetBrains/air/ahp` (macOS), `%APPDATA%\JetBrains\air\ahp` (Windows),
`$XDG_DATA_HOME/JetBrains/air/ahp` or `~/.local/share/JetBrains/air/ahp` (Linux). A file whose process does not run, a
file that does not parse and a file of another `schema` than 2 MUST be skipped. A directory that does not exist MUST
read as no instance, not as an error.

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

### Requirement: The exchange: A launch sends the mode, Plan and plan effort only when set

Run MUST be `createSession` and Draft `x-air-ij/openDraft`, each with `projectDirectory` and the prompt (empty for a
draft without one). A launch MUST send the target and its surface only together, and `model`, `effort`, `mode`, `plan`,
`planEffort` and the title of Run only when set. Without a target the IDE starts the default of its New Session button.

#### Scenario: An accepted launch sends every field
- **WHEN** a launch with every field set is accepted
- **THEN** it sends every field and reads the local session id of the result
  [@test] crates/air-launch/src/client_test.rs (`the_launch_of_the_wire_lists_the_targets_starts_the_default_or_a_target_opens_a_draft_and_reads_each_refusal`)

#### Scenario: A draft without a model, an effort, a mode, Plan and a prompt
- **WHEN** a launch has no model, no effort, no mode, no Plan and no prompt
- **THEN** it leaves them out, and a draft sends an empty prompt and no title
  [@test] crates/air-launch/src/protocol_test.rs (`the_params_of_a_launch_send_an_option_only_when_set_and_a_surface_only_with_a_target`)

#### Scenario: A launch without a target
- **WHEN** a launch names no target
- **THEN** `createSession` has no `launchTargetId` and no `interactionSurfaceId`, and the IDE starts its default
  [@test] crates/air-launch/src/client_test.rs (`the_launch_of_the_wire_lists_the_targets_starts_the_default_or_a_target_opens_a_draft_and_reads_each_refusal`)
  [@test] crates/air-launch/src/protocol_test.rs (`the_params_of_a_launch_send_an_option_only_when_set_and_a_surface_only_with_a_target`)

### Requirement: The exchange: A refusal is an answer, never an error

An error of the IDE MUST be a refusal with the code of `error.data.code` (`DENIED`), and the message and the hint of
`error.data`, never an error of the command. A code this client does not know stays as its text.

#### Scenario: A refusal of a launch or of the targets
- **WHEN** the IDE refuses a launch or the targets
- **THEN** the answer is the refusal with its code, message and hint
  [@test] crates/air-launch/src/client_test.rs (`the_launch_of_the_wire_lists_the_targets_starts_the_default_or_a_target_opens_a_draft_and_reads_each_refusal`)

#### Scenario: An error without a code of its data
- **WHEN** an error of the IDE has no `data.code`
- **THEN** its code is the code of its number (`UNKNOWN_METHOD`, `BAD_PARAMS`, `BAD_REQUEST`, else `ERROR_<number>`)
  [@test] crates/air-launch/src/client_test.rs (`an_error_without_a_code_of_its_data_reads_the_code_of_its_number`)
  [@test] crates/air-launch/src/protocol_test.rs (`a_refusal_reads_the_code_message_and_hint_of_the_data_else_the_code_of_the_number`)

### Requirement: The exchange: A watch reads each status of the session until its end

A watch MUST subscribe the root (no consent), list the rows with `listSessions` page by page (its first call asks the
consent), then read each root notice without a timeout of the client: `root/sessionAdded`, `root/sessionRemoved` and
`root/sessionSummaryChanged`. It ends with a close of the IDE or a stop. A notice of another method MUST be skipped.

#### Scenario: The rows, then each change until the close
- **WHEN** the client watches the rows of an IDE
- **THEN** it subscribes the root, lists the rows page by page, and reads each change of a row until the close
  [@test] crates/air-launch/src/client_test.rs (`the_launch_of_the_wire_lists_the_targets_starts_the_default_or_a_target_opens_a_draft_and_reads_each_refusal`)

#### Scenario: A watch waits without a timeout
- **WHEN** the IDE sends no notice for long
- **THEN** the watch waits, with no timeout of the client
- **AND** it skips a notice of another method
  [@test] crates/air-launch/src/client_test.rs (`a_watch_waits_without_a_timeout_skips_a_notice_of_another_method_and_its_stopper_ends_it_from_another_thread`)

#### Scenario: A garbled notice of a watch
- **WHEN** a notice of the root does not parse
- **THEN** it is a bad response and ends the watch
  [@test] crates/air-launch/src/client_test.rs (`a_garbled_notice_of_a_watch_is_a_bad_response_and_ends_it`)

## REMOVED Requirements

### Requirement: The exchange: One line of response within 120 s, then the close

**Reason**: A connection of local AHP holds JSON-RPC with ids and notices, not one line.
**Migration**: "The exchange: The answer of each request within 120 s".

### Requirement: The exchange: An IDE before handshake v2 answers the opener

**Reason**: The client speaks only local AHP; an IDE without it gives no greeting, which is a bad response.
**Migration**: None.
