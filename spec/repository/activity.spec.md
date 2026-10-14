---
name: Activity
description: Requirements for the log of what Branch Deck did in a repository (pushes, updates, landed reports, Patronus robots, history rewrites, errors), opened from the header, with the Undo of a rewrite while its toast's lasts.
style: plain-1
targets:
  - ../../src/stores/activity.ts
  - ../../src/components/repository/ActivityButton.tsx
  - ../../src/components/repository/ActivityLog.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/LandedCard.tsx
  - ../../src/components/branch/DroppedCommitList.tsx
  - ../../src/lib/command.ts
  - ../../src/lib/errors.ts
  - ../../src/lib/landed.ts
  - ../../src/lib/git/push.ts
  - ../../src/lib/patronus-actions.ts
  - ../../src/stores/patronus.ts
  - ../../src/stores/update-project.ts
  - ../../src/stores/landed.ts
  - ../../src/stores/settings/app-store.ts
---

# Activity

Status: Implemented
Date: 2026-10-14

## Summary

What Branch Deck did lives in toasts of 5 to 10 seconds: the push that dropped remote commits, the update, what landed,
the Safe Push the user started, the squash and its Undo. Activity keeps them: a `HistoryIcon` button in the header opens
the log of the selected repository, newest first by day, each entry with what happened, the detail and when. A branch
name shows its row, a landed entry has the report of the island, a rewrite has its Undo while the toast's lasts.

## Goals

- Answer "what did I push yesterday", "what did the update do", "which robot did I start" after the toast is gone.
- One record of each event, in the words of its toast: Activity adds no news of its own.
- Undo from Activity exactly as from the toast, for as long, once.

## Non-goals

- No history of the repository (that is `git reflog`): only what this app did, as it said it.
- No sync and no fetch of their own in the log: they change nothing the user asked for, a failure is an error entry.
- No Undo after the toast's window or after a restart: the rewrite it would revert may be pushed or rewritten again.

## Requirements

### Recording

- Every event that toasts in the main window is recorded for its repository, the selected one by default, in the words
  of its toast; toasts are unchanged. A sub-window records nothing: it neither owns nor receives the log.
  [@test] ../../src/stores/activity.test.ts (`keeps the log of each repository apart, newest first`, `records nothing without a repository`)
- A push: "Pushed feature-auth" / "3 commits", "4 commits · new on the remote"; a force push "Force pushed ABC-123" /
  "1 commit · dropped 1 remote commit" with the dropped remote commits (`DroppedCommitList`), or "replaced the rebased
  copies"; a remote that moved since its status says so, nothing pushed. The words come from the remote status the push
  was made against.
  [@test] ../../src/stores/activity.test.ts (`says what the push did from the status it was made against`, `keeps the remote commits a force push dropped`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record the pushes with the remote commits a force push dropped, and show the branch`)
- An update: applied, with the toast's words ("Updated master: rebased 1 commit onto origin/master", or what landed
  first, with the report); refused, "Didn't update master" / "Conflicts in 1 file: src/app.txt. Nothing was changed.";
  an update with nothing to do is no entry.
  [@test] ../../src/stores/activity.test.ts (`records a refused update, not one with nothing to do`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`, `should record a refused update`)
- A landed report is recorded once: by the update that brought it, or by the sync after an update made elsewhere (the
  commits no entry has yet, "Found by a sync").
  [@test] ../../src/stores/activity.test.ts (`records a landed report once, whether the update or the sync after one brings it`)
- A Patronus robot started, restarted, canceled ("Safe Push started" / "auth → master") or merged ("auth merged into
  master" / "Safe Push"), with "Open in Patronus".
  [@test] ../../src/stores/activity.test.ts (`records a robot with its page`)
- A history rewrite with an Undo toast (squash, fixups, move, ungroup, rename, stack, unstack), and every error toast
  (`notifyError`, with its reason).
  [@test] ../../src/stores/activity.test.ts (`records the errors the user was told about`)

### Undo

- A rewrite entry shows "Undo" (`xs`) for the 10 s of its toast. Either Undo, the toast's or the entry's, reverts the
  rewrite once, closes the toast, and the entry says "undone"; after the window the button goes, and the entry says
  what was done, as its title.
  [@test] ../../src/stores/activity.test.ts (`offers the Undo of a rewrite for the window of its toast, once`, `marks a rewrite undone from its toast`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should offer the Undo of a squash while the toast's lasts`)
- The undo itself is not persisted: after a restart no entry has an Undo.
  [@test] ../../src/stores/activity.test.ts (`has no Undo for a rewrite of an earlier session`)

### Log

- The log is persisted by the main window (`createPersistentStore("activity", { isArray: true, isMainOnly: true })`)
  and parsed by its Zod schema on load; it is not sent to sub-windows.
  [@test] ../../src/stores/activity.test.ts (`starts from the stored log and parses what is stored or received`)
- At most 200 entries over all repositories, none older than 30 days; the oldest go first.
  [@test] ../../src/stores/activity.test.ts (`keeps at most 200 entries and none older than 30 days`)
- "Clear" in the footer forgets the log of the selected repository; the others stay.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should clear the log of the repository`)

### Landed entries

- A landed entry keeps the report: "What landed in origin/master" expands to the rows of the Landed island
  (`LandedBranchRow`), each expanding to its commits. While its commits are in the undismissed report (their chips and
  the "New" marks show, the island may be hidden since, see [Landed Commits](landed-commits.spec.md)), "Dismiss" clears
  them as the island's Dismiss.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island`)

## User Experience

- The button is a ghost `icon-sm` `HistoryIcon` after the outgoing indicator, named "Activity" with the tooltip
  "Activity" (an icon-only button, [UI Theme](../../docs/design/ui-theme.md)); it is hidden while the log of the
  repository is empty. The header stays on one line from 960 px.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`)
- A click opens a `Popover` (`w-96`): the title "Activity", the entries newest first grouped by day ("Today",
  "Yesterday", the date), the body scrolling within `max-h-[60vh]` with the day headings sticky, then "Clear".
  [@test] ../../src/stores/activity.test.ts (`groups by local day: today, yesterday, then the date`)
- An entry: the icon of its kind (`UploadIcon` push, `ArrowDownIcon` update, `ShieldCheckIcon` robot, `PenLineIcon`
  rewrite), `PackageCheckIcon` in `info` for what landed, `TriangleAlertIcon` in `warning` for a refusal, a dropping
  push or a moved remote, `OctagonXIcon` in `destructive` for an error; the title, the description muted, the relative
  time (`RelativeTime`). Commit subjects show through `CommitSubjectLine`, with their issue links.
- A word of the title or description that names a branch of the table is a button: it closes the popover and reveals
  the row (`requestReveal`), as a branch name of the outgoing overview.

## Testing / Local Run

- `pnpm test:unit`: `src/stores/activity.test.ts`.
- `pnpm e2e`: `tests/e2e/activity/activity.spec.ts` with the templates `issue_links` (pushes through `routePush`, a
  squash), `behind_upstream`, `update_conflict` and `landed_upstream`.

## Open Questions / Risks

- The log lives in `settings.json`, read by the main window at startup: 200 entries with landed reports stay small
  (a few hundred KB at worst), but a much larger cap would not.
- An entry that doesn't parse (written by a newer version) drops the whole log on load, as any invalid stored value.
- A cleared log records a still undismissed report again at the next sync, as "Found by a sync".

## References

- [Landed Commits](landed-commits.spec.md): the island and its lifetime.
- [Outgoing Commits](outgoing.spec.md), [Update Project](update-project.spec.md), [Patronus](../branch/patronus.spec.md),
  [Squash](../commit/squash.spec.md).
- [Distributed Settings](../../docs/design/distributed-settings.md): persistent and main-only stores.
