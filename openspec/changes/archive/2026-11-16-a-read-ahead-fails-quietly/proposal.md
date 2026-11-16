# A read ahead fails quietly

## Why

`void commands.prefetchReview(review).then(...)` of src/lib/review-window.ts handled an error answer of the command
but not a rejection: the bindings rethrow an `Error` of the IPC (`typedError`), so a request WebKit failed as its page
left ("Load failed") was an unhandled rejection of the page, which failed moved-lines.spec.ts:292 in WebKit, and in the
app an error line for a read nobody asked for. A read ahead is best effort: the read of the window tells its own failure.
The reads ahead of the page had no one way: some logged an error answer as an error, some dropped it, none handled the
IPC.

## What Changes

- `readAhead(what, run)` of src/lib/command.ts: `invokeCommand`, then a warning of the log for a failure of the command
  or of the IPC, never an error line or a rejection; resolves to whether it read.
- The read ahead of a review as its window opens runs through it; a failed one is read ahead again on the next open.
- Capability `branch/branch-review`: a requirement that a read ahead that fails is a warning, with its scenario.
