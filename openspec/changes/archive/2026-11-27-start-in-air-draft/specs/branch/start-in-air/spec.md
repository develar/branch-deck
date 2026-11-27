## ADDED Requirements

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

## MODIFIED Requirements

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
- **WHEN** the client replays the frames of the hosts and of a draft that a local app sends and gets in the corpus of
  the IDE (`crates/air-launch/fixtures/local-ahp-apps.ndjson`), the draft with the project and the prompt alone
- **THEN** it reads each answer and the refusal `UNAUTHORIZED` of `initialize`, and skips the notices before an answer
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

## REMOVED Requirements

### Requirement: The exchange: A target has its models, modes, Plan and listsLoading

**Reason**: The page sends no target: the IDE takes the default agent of its New Session button, and the user picks an agent in the tab.

**Migration**: "The exchange: A draft names only the project and the prompt".

### Requirement: The exchange: A launch sends the mode, Plan and plan effort only when set

**Reason**: Start in Air sends no route, no model, no effort, no mode and no Plan: it opens a draft with the project and the prompt alone.

**Migration**: "The exchange: A draft names only the project and the prompt".

### Requirement: The exchange: A watch reads each status of the session until its end

**Reason**: Start in Air starts no session, so nothing is followed: the user sends the prompt from the New Session tab.

**Migration**: None.

### Requirement: The exchange: A stop from another thread ends a watch

**Reason**: No watch is left to stop.

**Migration**: None.

### Requirement: The dialog: The dialog asks the IDE for its agents and reads the prompt at once

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: No IDE, a refusal and an error are said in the dialog

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: The default agent comes first

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: The agent, model and effort pickers

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: The mode, Plan and plan effort pickers

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: Run or Draft, and an editable prompt

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: Plan of a mode sends no mode

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: A plan effort is sent only with Plan

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: The pickers start from the choice remembered for the repository

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: Another agent or model keeps a choice only while it offers it

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: The dialog asks again while an agent still lists its models

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: A newer list keeps the choice made in the dialog

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: Start sends the choice, the prompt and the title

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: A draft sends no blank prompt and Run refuses one

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: An accepted launch closes the dialog and remembers the choice

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: A launch without a session id says it started

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: A refusal stays in the dialog in its words

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: Start sends again only after a change

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The dialog: An error of the exchange says it cannot start the session

**Reason**: The dialog is gone: Start in Air opens the New Session tab of Air with the prompt, and the user picks the agent, the model and the rest in the composer of Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The session toast: The toast follows the session in place

**Reason**: Start in Air starts no session, so no toast follows one: the user sends the prompt from the New Session tab and sees the session in Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The session toast: A done session stops the watch and closes the toast

**Reason**: Start in Air starts no session, so no toast follows one: the user sends the prompt from the New Session tab and sees the session in Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The session toast: The toast says how the watch ended

**Reason**: Start in Air starts no session, so no toast follows one: the user sends the prompt from the New Session tab and sees the session in Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The session toast: A close of the toast stops the watch

**Reason**: Start in Air starts no session, so no toast follows one: the user sends the prompt from the New Session tab and sees the session in Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The session toast: One watch per window and session

**Reason**: Start in Air starts no session, so no toast follows one: the user sends the prompt from the New Session tab and sees the session in Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".

### Requirement: The session toast: A watch tells the page each status and how it ended

**Reason**: Start in Air starts no session, so no toast follows one: the user sends the prompt from the New Session tab and sees the session in Air.

**Migration**: "The action: Start in Air opens a prefilled New Session tab with the fast prompt".
