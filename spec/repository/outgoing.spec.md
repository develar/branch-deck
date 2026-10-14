---
name: Outgoing Commits
description: Requirements for the header indicator of the commits waiting to be pushed and its overview popover, with a row per branch and the push button of the branch row.
style: plain-1
targets:
  - ../../src/components/repository/OutgoingIndicator.tsx
  - ../../src/components/repository/OutgoingPopover.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/hooks/usePushSummary.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/lib/push-summary.ts
  - ../../src/lib/remote-status.ts
  - ../../crates/sync-core/src/remote_status.rs
  - ../../src/stores/commit-move.ts
---

# Outgoing Commits

Status: Implemented
Date: 2026-10-14

## Summary

The header shows how many commits wait to be pushed, over all virtual branches, next to a tiny bar with a segment per
branch. Hovering or clicking it opens an overview: a row per branch with the size of its push, what the push does and
the push button of the branch row.
The overview answers "what is my work waiting on" without scanning the branch table.

## Goals

- See at a glance that something is not pushed, and how much.
- Tell a plain push from a first push and from a force push before pushing.
- Push a branch, or go to its row, from the overview.

## Non-goals

- There is no "Push all": a push stays deliberate and per branch.
- The overview does not fetch: it shows the remote status of the last sync or push.
- The indicator does not count the commits the remote has and the branch lacks; a force push drops them, and its row
  says how many.

## Requirements

### What is outgoing

The sync rebuilds a virtual branch on the latest baseline, so its hashes change while its work stays the same. What
is outgoing compares content, not hashes: the question is whether the remote has the user's work as it is now.

- A branch is outgoing when its row offers a push that is not disabled as up to date: the branch synced without an
  error, has commits, and its remote status is loaded and not up to date.
- The commits of a push must be the commits whose content the remote lacks. The baseline commits must not count (the
  remote has them), nor the rebased copies of pushed commits (patch-equivalent). A branch not on the remote counts all
  its commits, the baseline excluded; mine counts only the user's commits, also for a first push.
  [@test] ../../src/lib/push-summary.test.ts (`counts the commits whose content the remote lacks`)
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_new_branch_counts_commits_by_author`)
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_new_commit_after_baseline_merge`)
- A branch only rebased onto a newer baseline must be up to date: the remote has all its commits by content. Its row
  says so, and it is not outgoing.
  [@test] ../../src/lib/push-summary.test.ts (`is up to date for a branch only rebased onto a newer baseline: the remote has its work`)
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_rebased_branch_shows_zero_unpushed`)
- A branch with a conflict, a sync error or no commits must not count; a branch whose remote status is not loaded yet
  must not count either, but the overview says how many there are.
  [@test] ../../src/lib/push-summary.test.ts (`skips the branches whose row offers no push and counts the ones not loaded`)
- The state of a branch must follow the label of its push button:
  - "create" for a branch not on the remote (`Create Remote (3)`);
  - "force" for a push that is not a fast-forward: the remote has commits the branch lacks, or the branch was rebased
    onto a newer baseline (`Force Push`). It follows the remote branch and the local one, not the result of the last
    sync, so it stays the same across syncs and restarts;
  - "push" otherwise (`Push (2)`).
  [@test] ../../src/lib/push-summary.test.ts (`follows the rules of the push button label`)
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_rebased_branch_with_new_commit_is_not_fast_forward`)
- The remote status must name the remote commit it was computed against (`remoteTip`, null without a remote branch)
  and list the newest ten of the commits a push drops (`droppedCommits`: id, subject, author, time), newest first;
  `commitsBehind` counts them all.
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_dropped_commits_are_listed_newest_first_up_to_the_limit`)
- The total and the mine counts must sum the outgoing branches; "mine" uses the remote status count of the user's
  commits.
  [@test] ../../src/lib/push-summary.test.ts (`sums the outgoing branches in order and counts them by state`)

### Indicator

- The indicator is a ghost button in the header, before the time of the last sync: an up arrow, the total commits and
  a bar.
- Its accessible name must be the title of the overview, e.g. "12 commits to push in 7 branches".
- The bar must have a segment per outgoing branch, in the order of the rows, its width proportional to the commits of
  the branch; a force push without commits keeps a segment of one commit.
- A segment must be stacked: the user's commits solid, the others' commits light, as `mine (total)↑` reads. The one
  commit of a force push without commits is solid.
  [@test] ../../src/lib/push-summary.test.ts (`has a segment per branch, a force push without commits still visible`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show the commits to push with a bar segment per branch`)
- With more than 8 outgoing branches, the bar must have a segment per hue instead, so the segments stay readable.
  [@test] ../../src/lib/push-summary.test.ts (`has a segment per hue for many branches`)
- The indicator must be hidden when nothing is outgoing, also right after the last push, unless a pull request is open
  ([Pull Requests](../branch/pull-requests.spec.md)).
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should hide when every branch is pushed`)
- It must follow every sync and push result without a reload.

### Overview

- Hover (after a short delay) or a click must open the overview; it is loaded while the hover delay runs.
- The title must say "12 commits to push in 4 branches"; a muted line below says how many of them are the user's when
  that differs, and how many branches are not checked yet.
- The overview must list a row per outgoing branch in the order of the branch table, a branch nested under its issue
  where the table shows it.
- A row must show the branch name, a bar proportional to the largest push, `mine (total)↑` as the status text of the
  branch row (with `· N↓` for the remote commits a force push drops), the state word ("new" for a first push, "force"
  for a force push, none for a plain push) and the push button of the branch row.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should list the outgoing branches in the overview on hover`)
- A row bar must be stacked as the segments of the indicator.
- A legend must name the hues present ("push" for a safe push, "drops remote commits"), and, when some commits are not
  the user's, the solid "yours" and the light "others'".
  [@test] ../../src/lib/push-summary.test.ts (`has a swatch per colour present`)
- Below the rows, after a rule, the overview shows the update state of the baseline with an Update button
  ([Update Project](update-project.spec.md)).
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show the update state in the footer of the outgoing overview`)
- A click on a branch name must close the overview, expand the branch row and scroll it into view.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show and expand the row of a branch clicked in the overview`)
- The push button must push as the button of the row does, with the same label, description and disabled states, but
  in the text colour: the bar of the row carries the colour, and a column of coloured buttons would only repeat it.

### Push colours

One rule colours a push wherever it shows: the push button and the status text of a branch row, the segments of the
indicator and the row bars of the overview. The hue says how risky the push is, the lightness whose commits it sends.

- The hue must follow the risk of the push: `warning` when it drops remote commits the branch lacks (`· N↓`), `primary`
  otherwise. A force push of a rebased branch only replaces the remote commits with their rebased copies: it is safe,
  `primary`, and keeps the `Force Push` label, which says what git does.
  [@test] ../../src/lib/push-summary.test.ts (`takes the hue from the risk: warning only for a push that drops remote commits`)
- Up to date is neutral. A safe push of no commit of the user is neutral too (muted button, grey status dot): there is
  nothing of theirs to push. A first push and a push that drops remote commits keep their hue whoever authored the
  commits.
  [@test] ../../src/lib/push-summary.test.ts (`is neutral for a safe push of no commit of the user`)
- The status text of a push state is muted text with a dot of the push colour, never a badge: a force push after a sync
  is routine. Only an error or a conflict is a badge.
- The overview must stay open across a push and follow its result: a pushed branch leaves the list, a branch whose
  remote diverged turns into a force push.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should push a branch from the overview and follow the push results`)

## User Experience

- The indicator button is `ghost` `sm`, the count in tabular figures, the bar `w-16 h-1.5`.
- The colours are the semantic tokens `bg-primary` and `bg-warning` for the user's commits, `bg-primary/40` and
  `bg-warning/40` for the others'. `info` is not a push colour. The segments of the branches are separated by a 2px gap
  of the surface, the two parts of a segment touch; both are rounded by `rounded-sm`, derived from `--radius`.
- Text never takes a state colour: the counts, the state words and the legend use `text-muted-foreground`; the colour
  is carried by the bars and the legend swatches.
- The overview is a `Popover` as wide as its content up to `max-w-lg`, aligned to the end of the button. The rows form a
  grid, so the bars and the counts line up; a long branch name is truncated; a row bar sits on a `bg-muted` track.
- The push buttons are aligned to the end of the rows.
- The branch name is a plain button that underlines on hover.
- The push button opens its description on hover, as in the branch row.

## Testing / Local Run

- `pnpm test:unit` covers the summary (`src/lib/push-summary.test.ts`).
- `pnpm e2e` covers the indicator and the overview (`tests/e2e/outgoing`). The E2E server has no remote: the tests
  answer `push_branch` in the page to turn branches into a plain push, a force push or up to date.

## Open Questions / Risks

- The `primary` green has a 2.2:1 contrast on the light card, the light tints 1.4–2.7:1; the counts and the state words
  carry the values in text.
- Solid green and solid amber are 6.4 ΔE apart for deuteranopia (validator floor band): the 2px gaps, the state words
  and the button labels carry the state too. The two light tints are only 4–5 ΔE apart under CVD (13 normal vision);
  they touch only across a gap, when a branch with others' commits is followed by one with none of the user's.
- No test covers a branch whose remote status is not loaded yet, or the bar of more than 8 branches in the page.

## References

- [Remote Status for Virtual Branches](../../docs/design/remote-status.md): how ahead, behind and mine are computed.
- [Branch Preview](../branch/branch-preview.spec.md): the same push button and labels.
- [Pull Requests](../branch/pull-requests.spec.md): the chip column, the attention dot and the pull request footer line.
- [Patronus](../branch/patronus.spec.md): the robot chip in the same column, the attention dot of a failed robot and the
  Patronus footer line.
- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
