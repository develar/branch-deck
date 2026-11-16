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
  the catch of `typedError`: the `ApiError` as is, anything else its `internal` error; a runtime of tauri-specta that
  writes another catch fails the export.
- `invokeCommand` is gone; the catches of the rejection take the error answer.
- The ten commands that answered their value as it is (`plain` and `now`: cancelConflictFix, validateBranchName,
  commitBody, unsubscribeView, setDemand, cancelOperation, stopAirWatch, agentCliStatus, cliToolsRunning,
  cancelCliToolAction) answer a Result, their value as `Ok` (`src-tauri/src/api.rs`), so no command rejects; the
  bindings test `every_command_of_the_bindings_answers_a_result` holds it, and their callers read the error answer.
- A failure of the e2e mock server is an error line of the log too, so a test fails on it whatever the page does.
- `typescript/no-floating-promises` is on.
- Capability `repository/title-bar`: the requirements that a failed call of a command is its error answer and that every
  command answers a Result.
