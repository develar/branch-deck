# Proposal

## Why

Two failures of the page in the IDE, left open by the change that gave each frame one view of Air:

- brd answers a subscription it refuses with its `ApiError` in `error.data.error`, and Air passes the error on, but the
  shim read every failure of the frame link as `internal`. So a repository without a branch prefix showed an error
  alert in the IDE, not the welcome card that `no_branch_prefix` is.
- After Air ended a mirror (`x-air-app/subscriptionEnded`), the page asked brd for the snapshot. When brd still held a
  session of the repository, the page installed that snapshot and kept a subscription that nothing changed any more: a
  sync after it never reached the view.

And the contract of Air changed meanwhile.

## What Changes

- Page (`src/platform/air/shim.ts`): a failure of the frame link is the `ApiError` of its `data.error`, else
  `internal`; after the end of a mirror the snapshot of its repository is none until the page subscribes to it again, so
  the page subscribes again, as when its session went away in the desktop app.
- The fake IDE (`tests/e2e/air-host/`): a subscribe with the params of the view the frame holds answers `held` without a
  state, as Air does; `ide.closeMirror` ends a mirror for a cause of Air (`closed`) while brd keeps the session.
- The copy `docs/design/ide-embedding/protocol/` is byte-identical to `plugins/air/native/air-apps/protocol/` of idea-1
  again, with `corpus/branch-deck-app.ndjson`: a `viewOpen` with the params of the view that the connection holds
  answers at once with that subscription (`held: true`, no snapshot), and `invalidExecutable` is a problem of an app
  entry. The IDE hosts the apps only with its registry key `air.apps.enabled`, off by default, and reads the executable
  of an app only after the user answered the consent of its command.
- Capability `repository/ide-embedding`: the requirement "Page: The shim maps the Tauri calls onto x-air-app" gets the
  scenario of a refused subscription, and the end of a mirror subscribes the page again; the concepts name the held
  answer, the registry key and the consent of the IDE.
