# Proposal

## Why

On 2026-11-25 a Safe Push pressed 16 s into Update Project said "Cannot start the Safe Push of air-native-shipped:
air-native-shipped changed after the push: no robot started; the push stays". The update held nothing during its
fetch, so the start ran at once on the table before it and pushed the old copy; then the sync of the update rebuilt
the virtual branch onto the new master, and the late witness of the start refused the branch that moved, though
`range-diff` showed the same four commits, only on another base. The spec promised the opposite: a start pressed
while Update Project runs waits behind the update.

## What Changes

- A start and a restart wait behind an Update Project of the process admitted before them through its fetch too, not
  only behind the holder of HEAD (`registry::Ahead::head_and_update`), their step `Queued` behind it; a rewrite still
  runs during the fetch, a push and a pull request intent do not wait for it, nor a start that pushes into the target
  (it updates itself after its push, and holds the tracking ref the fetch of that update needs). Stop ends the wait.
- The late witness of a start lets a branch that moved since its push go when it is the pushed commits rebuilt onto
  another base, the same patches and versions in the same order (`git_ops::rebuilt_branch`, one `rev-list`): the
  start lands the pushed commits. Another change after the push still skips the start as `plan_changed`.

Capabilities: `branch/patronus`, `cli/agent-cli`, `repository/update-project`.
