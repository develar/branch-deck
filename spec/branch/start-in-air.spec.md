---
name: Start in Air
description: Requirements for "Start in Air…" of a failed robot — the dialog that starts the prompt of the robot as an agent session in the IDE with Air that has the repository open, over the local launch socket of Air (protocol v1, IJAI-5465) — the discovery of the IDE, the agent, model, effort and start pickers, the choice remembered per repository, and the refusals of the IDE.
style: plain-1
targets:
  - ../../crates/air-launch/src/lib.rs
  - ../../crates/air-launch/src/discovery.rs
  - ../../crates/air-launch/src/client.rs
  - ../../crates/air-launch/src/protocol.rs
  - ../../crates/app-api/src/app/air_launch.rs
  - ../../src/lib/air-launch.ts
  - ../../src/stores/air-launch-dialog.ts
  - ../../src/components/branch/StartInAirDialog.tsx
---

# Start in Air

Status: Implemented
Date: 2026-10-30

## Summary

"Copy Prompt for Agent" puts the failure of a robot on the clipboard, and the user pastes it into an agent. Air, the
agent plugin of the JetBrains IDEs, has a local launch socket: a client names the project, the agent, the model and
the effort, and Air starts the session after the user agreed in the IDE. "Start in Air…" sends the same prompt there in
one dialog.

## Goals

- From a failed robot to a running agent session in the IDE in one dialog, without the clipboard.
- The last agent, model, effort and start of the repository come back the next time.
- When no IDE answers, the prompt is still one click away.

## Non-goals

- A consent or an identity of the client: the IDE asks the user and names the peer process by the kernel.
- A session in an IDE that does not have the repository open: the IDE refuses it (`NO_PROJECT`).
- The prompt with the CI details: the dialog starts the fast prompt, which tells the agent how to read the rest.

## Requirements

### The IDE

- Each IDE process with Air writes one instance file, `<pid>.json`, in `~/Library/Application Support/JetBrains/air/launch`
  (macOS), `%APPDATA%\JetBrains\air\launch` (Windows), `$XDG_DATA_HOME/JetBrains/air/launch` or
  `~/.local/share/JetBrains/air/launch` (Linux). A file whose process does not run, a file that does not parse and a
  file of another `schemaVersion` are skipped.
  [@test] ../../crates/air-launch/src/discovery_test.rs (`reads_the_files_of_running_processes_and_skips_dead_garbled_and_later_ones`, `no_directory_is_no_instance`, `find_instance_checks_the_processes_of_the_files`)
- The IDE of the repository is the one whose `projects` holds the directory of the repository or one of its parents,
  compared by path components; the deepest project first, then the newest file. A dead IDE is never picked.
  [@test] ../../crates/air-launch/src/discovery_test.rs (`the_ide_of_a_directory_has_it_or_a_parent_open_and_the_deepest_project_wins`, `of_two_ides_with_the_same_project_the_newest_file_wins`, `a_dead_ide_is_never_picked_though_its_file_is_the_newest`)
- The e2e server has no directory of instance files (`AppHost::air_instances`): no test reaches an IDE of the user.

### The exchange

- One connection per request: the client writes one JSON line (`"v":1`, the method and its fields), reads one line
  within 120 s (`RESPONSE_TIMEOUT`: the consent dialog and the launch), and closes. A request longer than 1 MiB is not
  sent; a response longer than 1 MiB, one that is not JSON or has an unknown `status`, a close without an answer, and
  no answer in time are errors of the command. AF_UNIX on every platform, on Windows through `uds_windows`.
  [@test] ../../crates/air-launch/src/client_test.rs (`the_targets_round_trip_names_the_project_and_reads_the_agents_with_their_models`, `an_accepted_launch_sends_every_field_and_reads_the_session_id`, `a_draft_without_a_model_an_effort_and_a_prompt_leaves_them_out`, `an_ide_that_does_not_answer_in_time_is_a_timeout`, `a_response_longer_than_the_limit_is_refused`, `a_garbled_response_or_an_unknown_status_is_a_bad_response`, `a_close_without_an_answer_is_closed_and_a_line_without_its_end_is_read_whole`, `a_request_longer_than_the_limit_is_not_sent`, `a_socket_nobody_listens_on_is_a_connect_error`)
- A refusal (`"status":"refused"`) is an answer with its code and message, never an error; a code of a later protocol
  stays as its text.
  [@test] ../../crates/air-launch/src/client_test.rs (`a_refusal_of_a_launch_or_of_the_targets_reads_its_code_and_message`, `a_refusal_code_of_a_later_protocol_stays_as_its_text`)
  [@test] ../../crates/air-launch/src/protocol_test.rs (`every_refusal_code_of_protocol_v1_reads_back_as_its_text`)

### The action

- A failed or failing robot that has "Copy Prompt for Agent" also has "Start in Air…", after the prompts and before
  "Open in Patronus", with the same blocker, in every form of the robot's actions; the robot details show it as a
  button beside the prompts.
  [@test] ../../crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)

### The dialog

- The dialog asks the IDE of the repository for its agents (`targets`) and reads the fast prompt at once. While the
  IDE answers it says "Asking the IDE for its agents… It may ask you first."; no IDE, a refusal and an error are said in
  the dialog with Try Again, and Copy Prompt stays in the footer.
- The pickers: the agent; its model (off for an agent without models: the launch names none); the effort, "Default"
  first (the launch names none) and the efforts of the model; Run or Draft. The prompt is editable. The pickers start
  from the choice remembered for the repository while the IDE still offers it, else the first agent, its default
  model, the default effort and Run; another agent takes its default model, and an effort stays only while the model
  takes it.
  [@test] ../../src/lib/air-launch.test.ts (`starts with the first agent, its default model, the default effort and Run`, `starts with the remembered agent, model, effort and start while the IDE still offers them`, `drops a remembered model or effort the agent no longer offers`, `takes the first agent when the remembered one is gone, and keeps the remembered start`, `has no model and no effort for an agent without models, and no form without agents`, `offers the efforts of the chosen model, none for an agent without models`, `another agent takes its default model and keeps the effort only while that model takes it`, `another model keeps the effort only while it takes it`)
- Start (⌘↵; "Open Draft" for a draft) sends the target, the model id, the effort, the prompt and the title "Fix the
  Safe Push of auth"; a draft with a blank prompt sends none, and Run refuses a blank prompt. An accepted launch closes
  the dialog with "Started Codex in Air", remembers the choice in the recent project (`airLaunch`) and nothing else.
  [@test] ../../src/lib/air-launch.test.ts (`sends the target, the model, the effort, the prompt and the title of the form`, `sends no blank prompt of a draft`, `remembers the target, the model, the effort and the start`, `answers the targets and the launch, and throws an error of the command with its code`)
- A refusal stays in the dialog in its words, the message of the IDE only for `LAUNCH_FAILED` and an unknown code;
  Start sends again only after a change of the agent, the model, the effort or the prompt. An error of the exchange is
  "Cannot start the session in Air" with the reason.
  [@test] ../../src/lib/air-launch.test.ts (`runs no blank prompt, and sends again only after a change of what the IDE refused`, `words every code of protocol v1, the message of a failed launch, and an unknown code with its message`)
