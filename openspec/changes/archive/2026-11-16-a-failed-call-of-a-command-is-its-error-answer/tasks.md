# Tasks

- [x] The catch of `typedError` written by the export, `commandError` and its tests (a string, an `Error`, an `ApiError`)
- [x] `invokeCommand` removed; the catches of a rejection turned into the error answer
- [x] Every command answers a Result (`plain`, `now`), its bindings test; their callers read the error answer
- [x] A failure of the e2e mock server is an error line of the log
- [x] `typescript/no-floating-promises` on; the test of a read whose IPC failed (Safe Push statistics)
