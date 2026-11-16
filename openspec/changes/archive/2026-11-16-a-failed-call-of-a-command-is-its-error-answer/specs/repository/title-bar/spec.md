## ADDED Requirements

### Requirement: Every platform: A failed call of a command is its error answer

A command whose answer is a Result MUST answer every failure of its call as its error, never as a rejection of the page
(`commandError` of `src/lib/api-error.ts`, which the export of the bindings writes into their `typedError`): the
`ApiError` the command answered as is; a refusal of Tauri itself (a string: an unknown command, an ACL refusal, invalid
arguments) and a failure of the IPC (an `Error`: a request WebKit failed as its page left, a window that closed) as its
`internal` error in their words, which its caller handles as any error answer. A command without a Result rejects as its
call fails, and every `void` of one MUST handle its rejection.

#### Scenario: The call of a command of a Result fails
- **WHEN** Tauri refuses the call, or its IPC fails, the read of the Safe Push statistics among them
- **THEN** the command answers its `internal` error in the words of the failure, the read ends with it, and the page has no unhandled rejection
  [@test] src/lib/api-error.test.ts (`resolves a refusal of Tauri (a string: an unknown command, an ACL refusal, invalid arguments) to an internal error`, `resolves a failure of the IPC (an Error: a fetch of the E2E bridge, the Air shim) to an internal error, never a rejection`, `resolves to the ApiError it answered`)
  [@test] src/stores/robot-stats.test.ts (`ends a read whose IPC failed with its error, never a rejection of the page: the entry stops loading and says it`)

#### Scenario: A command without a Result
- **WHEN** the page voids a call of a command without a Result
- **THEN** its chain handles the rejection (a `.catch`, a second argument of `.then`), which `pnpm lint:arch` (`void-command`) holds
  [@test] scripts/lint-arch.test.mjs (`a void of a command without a Result is refused unless its chain handles the rejection; of a command with one, a test or a comment not`)
  [@test] src/lib/air-launch.test.ts (`a stop whose call fails (the IPC, a refusal of Tauri) is a line of the log, never a rejection of the page`)
