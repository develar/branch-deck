# Proposal

## Why

A report that walks a pool for minutes kept nothing until its end: `disk.json`, one file of 26 MB for the 16 slots of
idea-1, was written once, after the last walk, so a report stopped after 18 minutes kept no slot, and the next one walked
them all again. The walks of all slots ran mixed in one stack, so the first slot ended near the end. Two reports at once
(the app, `brd`, the agent of a host) both walked everything. The output bases of the main worktree (136 GiB, a million
files) were walked in full every 15 minutes, the orphans and the trash on every report.

## What Changes

- The walks of a report run in a budget of 90 s unless `--fresh`: the changed parts first, then the full walks, the
  least recently measured first; what the budget does not reach keeps its saved bytes and is walked by the next report.
- The walks end one after another in that order, and each slot is saved as its walk ends, a file a slot in
  `$GIT_COMMON_DIR/branch-deck/disk/`; `pool.json` there keeps the output bases of the main worktree, the orphans and
  the trash, which the history checks as it checks a slot. The `disk.json` of brd before stays for that brd; a slot
  not saved yet shows its bytes from it until its first walk.
- One report walks a pool at a time (`disk/walk.lock`); another one shows the saved bytes and writes nothing.
- Each slot says where its bytes come from (`walk`: `parts`, `unchanged`, `budget`, `history_late`, `busy`, why a full
  walk), in the steps of `brd lease disk`, its JSON and the dialog.

Capabilities: `repository/disk-usage`, `cli/lease`.
