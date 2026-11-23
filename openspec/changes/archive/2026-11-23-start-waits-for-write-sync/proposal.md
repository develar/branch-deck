# Proposal

## Why

On 2026-11-23 a Safe Push pressed while Update Project ran was refused: "What the Safe Push of acp-owner-start-read was
decided on changed before it began (develar/virtual/acp-owner-start-read moved); nothing was done: start it again".
The start waited behind the update, was admitted as the update ended with the request of its sync, and planned on the
table before the update (`SyncWait::Never`); while its check fetched the target for 13 s, the sync of the update
rebuilt the branch, and the witness refused the start. The session knew no fact "a sync a write of the session asked
has not ended", so every operation decided on a table it could know was stale; the push, the pull request and the
restart, which wait for a running sync, missed one asked while a focus check ran or queued as a follow-up. And a
branch another process rebuilt (`brd sync` beside the app) refused any of them the same way.

## What Changes

- The session tells a wait for the sync of its writes (`Sessions::after_writes`): the run that serves the last
  `AfterRewrite` not yet ended, also one a focus check or a run before it delays.
- Every operation that plans on the table waits for that sync (`run_frame::wait_for_facts`); a start waits for no other
  (`SyncWait::Writes`), so a start pressed during ⌘R or a focus sync still starts at once.
- A start fetches its target beside that wait, once (`StartFetch`): its checks replay onto it.
- A plan whose local branches alone moved before its first effect (a remote-tracking ref that moved still refuses it)
  waits for the sync that shows them (`run_frame::catch_up`) and is decided again once, on what the user accepted:
  a start, a push, a pull request intent, a restart; one plan decided again per start, shared with the retry after an
  update of its baseline.

- A start decided again after such a sync waits, within a minute, for the facts of its plan the click had answered and
  the sync made the session read again (the review of the rebuilt commits); the session asks the readiness of the
  subject of every start that runs, whatever its row shows.

Capabilities: `branch/patronus`, `branch/branch-row`, `branch/next-step`.
