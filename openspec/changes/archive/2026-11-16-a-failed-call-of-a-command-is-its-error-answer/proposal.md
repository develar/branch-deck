# A failed call of a command is its error answer

## Why

The class of ce7c60747 (the read ahead of a review): the catch of `typedError`, which tauri-specta writes into the
bindings, rethrew an `Error` of the IPC (a request WebKit failed as its page left, a window that closed), so a command
whose answer is a Result rejected where its transport failed: an unhandled rejection wherever its call was voided, a
read that never ended. Any other rejection it answered as the error as is, so a refusal of Tauri itself ("Command not
found", an ACL refusal, invalid arguments: a string) was an error without a code. A few callers caught the rejection by
hand (`invokeCommand`, nine try/catch around an await, a few `.catch` and second arguments of `.then`).

## What Changes

- The export of the bindings (`src-tauri/src/bindings_test.rs`) writes `commandError(e)` of `src/lib/api-error.ts` into
  `typedError`, which takes the name of its command: the `ApiError` as is, anything else its `internal` error with a
  warning of the log ("Cannot run <command>", the backend logged nothing of it); a runtime of tauri-specta that writes
  another `typedError` fails the export.
- `useSafePush`: the `.catch` of the `execute` of a start is gone; the e2e test of a merge whose call fails declares no
  "Cannot update the project" line, the toast tells the error and the log has the warning.
- `invokeCommand` is gone; the catches of the rejection take the error answer.
- The ten commands that answered their value as it is (`plain` and `now`: cancelConflictFix, validateBranchName,
  commitBody, unsubscribeView, setDemand, cancelOperation, stopAirWatch, agentCliStatus, cliToolsRunning,
  cancelCliToolAction) answer a Result, their value as `Ok` (`src-tauri/src/api.rs`), so no command rejects; the
  bindings test `every_command_of_the_bindings_answers_a_result` holds it, and their callers read the error answer.
- A failure of the e2e mock server is an error line of the log too, so a test fails on it whatever the page does.
- `typescript/no-floating-promises` is on.
- Capability `repository/title-bar`: the requirements that a failed call of a command is its error answer and that every
  command answers a Result.
