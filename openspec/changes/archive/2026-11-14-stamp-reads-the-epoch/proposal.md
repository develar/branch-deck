# Proposal

## Why

A window that showed a slot while the slot went back and an agent leased it again kept the session of the lease
before: its rows, its "Last synced", and a focus synced the new worktree of itself. The stack roots collapsed under the
lease before collapsed stacks of the same names of the next.

## What Changes

- A stamp of a watched session reads the epoch of the worktree with `synced_before`; under another known epoch the
  session opens anew for the new lease by the code of an open (`Actor::new`, `open`), in a new epoch of its view, not
  while a run is in flight. Each window gets one patch into the new epoch, which applies to no view.
- The page takes a patch into another epoch as a gap: it reads the snapshot and takes it as the view of another
  session, as on a switch, and forgets what it held of the rows before (`forgetRows`: the branches view mounts anew,
  the stores of the rows forget the selection, the forms, the preview, the card and the path of a stack).
- The mirror of `brd mcp` moves to the session of the repository on a patch into another epoch, as at a change of the
  prefix: one change of every part (`Server::resync`), served between the messages of air-ide.
- The writer of the settings tells the windows what it forgot of an earlier epoch (`on_external_change`), so the page
  drops "Last synced" of the lease before; the page drops the synced path of a view without a table.
- The collapsed stacks of a recent project are of the epoch of its worktree (`COLLAPSED_STACKS_EPOCH_FIELD`).
- Capabilities `repository/onboarding` (a window on a slot leased again, its rows, the collapsed stacks) and
  `repository/ide-embedding` (a patch into another epoch moves the mirror).
