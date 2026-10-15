---
name: Landed Commits
description: Requirements for telling the user that their commits reached the baseline (a Safe Push robot, a merged pull request, a cherry-pick): before the update the header and the rows name what the fetched baseline already has, the update toast leads with what landed, and an island keeps the report until the user has seen it or dismissed it, also for an update made outside Branch Deck; Activity keeps it after.
style: plain-1
targets:
  - ../../crates/sync-types/src/landed.rs
  - ../../crates/sync-types/src/upstream_commit.rs
  - ../../crates/sync-core/src/landed.rs
  - ../../crates/sync-core/src/update_project.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/git-ops/src/update_branch.rs
  - ../../crates/bd-cli/src/sync_command.rs
  - ../../src-tauri/src/commands/update_project.rs
  - ../../src/lib/landed.ts
  - ../../src/stores/landed.ts
  - ../../src/stores/update-project.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/components/repository/LandedCard.tsx
  - ../../src/components/repository/ActivityLog.tsx
  - ../../src/stores/settings/repositorySettings.ts
  - ../../src/components/repository/UpdateProjectButton.tsx
  - ../../src/components/branch/LandedChip.tsx
  - ../../src/hooks/useLandedIn.ts
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/hooks/usePushSummary.ts
  - ../../src/lib/push-summary.ts
  - ../../src/components/shared/CopyButton.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchCreator.tsx
  - ../../src/components/commit/CommitList.tsx
---

# Landed Commits

Status: Implemented
Date: 2026-10-14

## Summary

A Safe Push robot rebases the commits of a virtual branch onto `master`; the next Update Project drops them locally.
Branch Deck says so at three moments: before the update, the header and the branch rows name what the fetched baseline
already has; the update toast leads with what landed; afterwards an island lists it, with the baseline commits and the
robot or pull request, until the user has seen it or dismisses it, and Activity keeps the report. An update made elsewhere (the IDE, `git pull --rebase`) is reported
by the next sync the same way.

## Goals

- Landing is the user's news: never hide it behind "rebased 116 commits" or a silent move into the archive.
- Name the user's own branches and commits, with the baseline commit that carries each, and part of a branch too.
- Know it whoever updated, and keep it until the user has seen it.

## Non-goals

- No count of the upstream's commits; only the user's own landed commits are named.
- No notification of its own: the moment a robot merges is the Patronus toast (`spec/branch/patronus.spec.md`).

## Requirements

### Detection

- A local commit landed when the baseline since the merge base has a commit with the same author email, author time and
  subject without the `(prefix)`; the landed commit names it with its author, committer and time.
  [@test] ../../crates/sync-core/src/landed_test.rs (`whole_branch_landed_by_a_rebase_upstream`)
  [@test] ../../crates/sync-core/src/landed_test.rs (`same_author_and_time_with_another_subject_is_not_landed`)
- The commits are grouped by branch, unassigned ones apart, each branch with the commits that stay local.
  [@test] ../../crates/sync-core/src/landed_test.rs (`part_of_a_branch_landed`)
- A fixup whose target landed landed with it once the replay drops it; without a replay (a conflict) only key matches
  count, so a fixup made after the push is never reported as landed.
  [@test] ../../crates/sync-core/src/landed_test.rs (`fixups_folded_upstream_landed_with_their_target`)
  [@test] ../../crates/sync-core/src/landed_test.rs (`fixup_made_after_the_push_stays_local`)
- Where the replay onto the upstream is known, a commit it drops landed even without a counterpart (squashed upstream,
  no baseline commit).
  [@test] ../../crates/sync-core/src/landed_test.rs (`branch_squashed_upstream_landed_without_an_upstream_commit`)
- A conflicting update still reports the key matches.
  [@test] ../../crates/sync-core/src/landed_test.rs (`landed_commits_are_reported_while_the_update_conflicts`)
- Detection costs three read-only `log` processes; the check inside a sync reuses them for an unchanged state, and a
  failure never fails the check or the update.

### Update

- When the replay conflicts, the check replays again without the commits with a counterpart together with their
  fixups, then without those commits alone, as a drop does (a run of kept commits per replay). A result counts only if
  it keeps every local change: `merge-tree --merge-base=<merge base> <result> HEAD` gives the result's tree. So a robot
  that folded the fixups into their target no longer makes the update conflict, while a commit amended after the push,
  or a fixup made after it, is never left out; the update then conflicts as before.
  [@test] ../../crates/sync-core/src/landed_test.rs (`fixups_folded_upstream_landed_with_their_target`)
  [@test] ../../crates/sync-core/src/landed_test.rs (`fixup_made_after_the_push_stays_local`)
  [@test] ../../crates/sync-core/src/landed_test.rs (`commit_amended_after_the_push_is_never_left_out`)
- The preview of every sync and the update make the same decision, so the header shows such an update as clean.

### Report

- An applied update adds what it landed to the report in `branch-deck/landed.json` of the worktree's git directory and
  returns it; the next sync doesn't add it again.
  [@test] ../../crates/sync-core/src/landed_test.rs (`whole_branch_landed_by_a_rebase_upstream`)
- Each sync remembers where the branch is; when the merge base moved since the previous sync, the commits that were local
  then and match the upstream now are added, once.
  [@test] ../../crates/sync-core/src/landed_test.rs (`update_made_elsewhere_is_reported_by_the_next_sync_once`)
- Amend, reword, squash and drop between two syncs report nothing.
  [@test] ../../crates/sync-core/src/landed_test.rs (`reword_squash_or_amend_between_syncs_is_not_landed`)
- The sync sends the landing (`BaselineStatus.landing`) and the undismissed report (`BaselineStatus.landed`).
  [@test] ../../crates/sync-core/src/landed_test.rs (`sync_sends_the_landing_and_the_report`)
- Dismiss clears the report for every window and `bd`; a report is kept at most 7 days. A new report for a known branch
  adds its new commits and takes the new count of the ones that stay.
  [@test] ../../src/lib/landed.test.ts (`adds new commits to a known branch and takes its new count of local commits`)
- `bd sync` prints the landing as `update.landing` and the report as `landed`, with short ids.
  [@test] ../../crates/bd-cli/src/sync_test.rs (`reports_the_commits_the_baseline_already_has`)

### Wording

- Every surface (the Update tooltip, the footer of the outgoing overview, the toast, the island, the chip) says it in
  one grammar: each number has its verb, "3 commits of IJPL-257337 landed · 9 stay local", never a ratio like "3 of
  12", which hides the number that landed. What stays is "1 stays local" / "9 stay local" ("9 local" on the chip).
  [@test] ../../src/lib/landed.test.ts (`gives each number its verb`)
- A commit subject shows its issue links and its dimmed conventional-commit prefix, as in the commit rows: in the
  island, in the popover of the chip.
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`, `should lead the toast with what landed and keep the island until it is dismissed`)

### Before the update

- The tooltip of the Update button adds what the baseline brings in: "Rebases cleanly onto origin/master · brings in
  your feature-auth and 1 commit of search"; a clean update shows the `info` package instead of the check, on the
  button and in the footer line of the outgoing overview. The footer line says "your wsm landed", "40 of your commits
  landed" (a part of a branch, or commits without one), "your 2 branches landed".
  [@test] ../../src/lib/landed.test.ts (`names the user's branches, a part as a count of the branch`)
  [@test] ../../src/lib/landed.test.ts (`says it in a few words for the footer`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
- The branch row shows an `info` chip "Landed" (all its commits) or "1 landed · 1 local" in the third column of its
  Status cell ([Branch Row](../branch/branch-row.spec.md)). Its popover has the anatomy of the
  pull request and Patronus details: the state line (the chip text), "Already in origin/master · the update removes the
  local copies", then the rows "Commits" (each subject with its baseline commit) and "Stays local" ("1 commit"). A
  landed commit shows "in origin/master" after its subject; its popover says "Already in origin/master · the update
  removes the local copy" and has the row "Landed as".
  [@test] ../../src/lib/landed.test.ts (`says it in fewer words on the chip, "Landed" when the baseline has all of the branch`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
- A branch the fetched baseline has all of has nothing left to push: the update archives it. Its status reads
  "landed" after an `info` dot, its push button keeps its label (`Create Remote (1)`) but is neutral, and both
  descriptions say "Already in origin/master. Update to archive the branch."; it is not outgoing, so the indicator of
  [Outgoing Commits](outgoing.spec.md#what-is-outgoing) leaves it out ("3 commits to push in 2 branches"). A part that
  landed changes nothing of this: the branch still has commits to push.
  [@test] ../../src/lib/landed.test.ts (`has nothing to push for a branch the fetched baseline has all of, before the update`)
  [@test] ../../src/hooks/useStatusBadge.test.ts (`says landed, in the merged hue, for a branch the baseline has all of: its push state no longer matters`)
  [@test] ../../src/hooks/usePushButton.test.ts (`is quiet for a branch the baseline has all of: the update archives it`)
  [@test] ../../src/lib/push-summary.test.ts (`skips a branch the fetched baseline has all of: the update archives it`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

### After the update

- The toast leads with what landed, then how the branch was updated: "wsm landed in origin/master" / "Updated master:
  rebased 116 commits"; "3 branches landed in origin/master" / "wsm, IJPL-257337 and IJAI-3828 · rebased 98 commits";
  "40 commits of IJPL-257337 landed · 12 stay local" / "Updated master: rebased 113 commits"; "2 of your commits landed
  in origin/master" for commits without a branch; ", squashed" when no commit of the branch has a counterpart. Without
  anything landed it stays "Updated master: rebased 2 commits onto origin/master".
  [@test] ../../src/lib/landed.test.ts (`leads with what landed, then how the branch was updated`)
  [@test] ../../src/lib/landed.test.ts (`keeps the fast-forward wording`)
- The Landed island sits above the branches: "Landed in origin/master" and when, a row per branch with "1 landed",
  "40 landed · 12 stay local" or "3 landed · squashed", the pull request and Patronus chips and the committer; a row
  expands to the commits, each with the short baseline commit (copy), its committer and time, or "squashed upstream".
  When every row has the same committer, the header names it once instead ("Landed in origin/master · by Space Team ·
  just now"). Dismiss (`XIcon`) closes it and clears the report, the chips and the "New" marks of the archive.
  [@test] ../../src/lib/landed.test.ts (`names the committer once when every branch has the same one`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should lead the toast with what landed and keep the island until it is dismissed`)
- The island shows from the report until the first successful sync that completes while the window has focus, at least
  10 s after the user saw the island (it showed while the window had focus): the sync that follows the update keeps it,
  a later one hides what was seen. Hidden is remembered for the repository (`landedHiddenUntil`, the newest `landedAt`
  seen), so a restart doesn't show it again; what lands later shows again. The "N landed" chips and the "New" marks stay
  until Dismiss, which the Activity entry of the report offers once the island is hidden, and the entry keeps the
  report ([Activity](activity.spec.md)).
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island`)
- The copy button of a baseline commit (the full hash) shows while its commit row is hovered or the button has keyboard
  focus, as the copy button of the commit that merged an archived branch.
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should show the copy button of a baseline commit on hover`)
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should show the copy button of the merged-by commit on hover`)
- A branch that stays active shows "1 landed · 1 local" until Dismiss; a fully landed branch moves to the archive,
  marked "New".

## User Experience

- `info` (the hue of merged) everywhere, `PackageCheckIcon`; the island is a stock `Card` with a one-time
  `animate-in fade-in-0 slide-in-from-top-2`, no celebration beyond it.
- The chip and the marker are `StatusChip`s without a click action: hover or a click opens the details.

## Testing / Local Run

- `cargo nextest run --workspace`: `crates/sync-core/src/landed_test.rs`, the `landed_upstream` template
  in `update_project_test.rs`, `crates/bd-cli/src/sync_test.rs`.
- `pnpm test:unit`: `src/lib/landed.test.ts`.
- `pnpm e2e`: `tests/e2e/update-project/landed.spec.ts` with the template `landed_upstream`: a robot pushed `feature-auth`
  and the first `search` commit, committer `Patronus`.

## Open Questions / Risks

- A fixup of a prefix branch is pushed without its marker (the virtual commit takes the subject without the `(prefix)`
  and the marker before it), so a robot can't fold it: it lands as a commit of its own and matches by key. Only the
  fixups of an issue or unassigned branch keep the marker and are folded.
- Leaving the landed commits out is tried only when the plain replay conflicts, and the no-loss proof is strict: an
  upstream commit that changed the landed lines again after the robot makes it fail, and the update conflicts.
- Two commits of one author in the same second with the same subject are one key.
- The shared-element move of a row into the island (`ViewTransition`) is not done: the rows change on a later sync
  event than the report, and zustand updates are not transitions.

## References

- [Integration Detection](../../docs/design/integration-detection.md): decision 9, "Landed commits".
- [Update Project](update-project.spec.md), [Patronus](../branch/patronus.spec.md).
- [Branch Row](../branch/branch-row.spec.md): the Status cell and the anatomy of the chip details.
