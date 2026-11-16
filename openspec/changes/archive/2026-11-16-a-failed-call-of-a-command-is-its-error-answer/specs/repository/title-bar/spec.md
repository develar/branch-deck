## ADDED Requirements

### Requirement: Every platform: A failed call of a command is its error answer

A command MUST answer every failure of its call as its error, never as a rejection of the page (`commandError` of
`src/lib/api-error.ts` in the `typedError` of the bindings): the `ApiError` it answered as is; a refusal of Tauri (a
string) and a failure of the IPC (an `Error`) as its `internal` error in their words, with one warning of the log,
"Cannot run <command>", as the backend logged nothing of it.

#### Scenario: The call of a command fails
- **WHEN** Tauri refuses the call, or its IPC fails: the read of the Safe Push statistics, the check of a typed branch name, the stop of the watch of an Air session
- **THEN** the command answers its `internal` error in the words of the failure, the log has one warning "Cannot run <command>" (none for an error the command answered), the read ends with it, a typed name says why and its submit asks the check again, and the page has no unhandled rejection
  [@test] src/lib/api-error.test.ts (`resolves a refusal of Tauri (a string: an unknown command, an ACL refusal, invalid arguments) to an internal error`, `resolves a failure of the IPC (an Error: a fetch of the E2E bridge, the Air shim) to an internal error, never a rejection`, `resolves to the ApiError it answered, which the backend logged: no warning of the page`, `answers a failure of no words with words of its own, never an empty message`)
  [@test] src/stores/robot-stats.test.ts (`ends a read whose IPC failed with its error, never a rejection of the page: the entry stops loading and says it`)
  [@test] src/presenters/branch-name.test.ts (`says a check whose call failed and marks the name invalid, so no form submits a name nobody checked`)
  [@test] src/lib/debounced-answers.test.ts (`shows a failed answer without keeping it: no pending one, and asked again, a later answer of the same input kept`)
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should ask the check of a typed name again on the submit when its call failed, and rename once it answers`)
  [@test] src/lib/air-launch.test.ts (`a stop whose call fails (the IPC, a refusal of Tauri) is a line of the log, never a rejection of the page`)

### Requirement: Every platform: Every command answers a Result

Every command MUST answer a Result in the bindings, the value of a body that cannot fail (`plain`, `now`) as its `ok`
answer (`src-tauri/src/api.rs`), so that the failed call of any command is its error answer.

#### Scenario: A command whose body cannot fail
- **WHEN** the bindings are exported
- **THEN** every command of them answers a Result (`typedError`), a body that cannot fail too (`plain`, `now`), so no call of the page rejects
  [@test] src-tauri/src/bindings_test.rs (`every_command_of_the_bindings_answers_a_result`)
