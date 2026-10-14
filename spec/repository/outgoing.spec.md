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
  - ../../src/lib/push-summary.ts
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

- A branch is outgoing when its row offers a push that is not disabled as up to date: the branch synced without an
  error, has commits, and its remote status is loaded and not up to date.
- A branch not on the remote is outgoing with all its commits; the sync reports no commits ahead for it.
  [@test] ../../src/lib/push-summary.test.ts (`counts every commit of a branch not on the remote`)
- A branch with a conflict, a sync error or no commits must not count; a branch whose remote status is not loaded yet
  must not count either, but the overview says how many there are.
  [@test] ../../src/lib/push-summary.test.ts (`skips the branches whose row offers no push and counts the ones not loaded`)
- The state of a branch must follow the label of its push button:
  - "create" for a branch not on the remote (`Create Remote`);
  - "force" for a branch the remote has commits of that it lacks, or that the sync rewrote (`Force Push`);
  - "push" otherwise (`Push (2)`).
  [@test] ../../src/lib/push-summary.test.ts (`follows the rules of the push button label`)
- The total and the mine counts must sum the outgoing branches; "mine" uses the remote status count of the user's
  commits.
  [@test] ../../src/lib/push-summary.test.ts (`sums the outgoing branches in order and counts them by state`)

### Indicator

- The indicator is a ghost button in the header, before the time of the last sync: an up arrow, the total commits and
  a bar.
- Its accessible name must be the title of the overview, e.g. "12 commits to push in 7 branches".
- The bar must have a segment per outgoing branch, in the order of the rows, its width proportional to the commits of
  the branch; a force push without commits keeps a segment of one commit.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show the commits to push with a bar segment per branch`)
- With more than 8 outgoing branches, the bar must have a segment per state instead, so the segments stay readable.
  [@test] ../../src/lib/push-summary.test.ts (`has a segment per state for many branches`)
- The indicator must be hidden when nothing is outgoing, also right after the last push.
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
- A legend must name the colours of the states present.
- A click on a branch name must close the overview, expand the branch row and scroll it into view.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show and expand the row of a branch clicked in the overview`)
- The push button must push as the button of the row does, with the same label, description and disabled states, but
  in the text colour: the bar of the row carries the state.
- The overview must stay open across a push and follow its result: a pushed branch leaves the list, a branch whose
  remote diverged turns into a force push.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should push a branch from the overview and follow the push results`)

## User Experience

- The indicator button is `ghost` `sm`, the count in tabular figures, the bar `w-16 h-1.5`.
- The colours are the semantic tokens: `primary` push, `info` new remote, `warning` force push. The segments are
  separated by a 2px gap of the surface and rounded by `rounded-sm`, derived from `--radius`.
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

- Step 6 (Update Project) passes the update state of the baseline as `footer` of `OutgoingIndicator`; the overview shows
  it below the rows, after a rule.
- Step 8 (pull requests) passes `renderPullRequest(branchName)` for the chip of a row, between the state word and the
  push button, and adds a dot to the indicator when a pull request has changes requested or failing checks. The
  indicator must then also show when a pull request is open and nothing is outgoing.
- The push button of a branch row keeps its own colours (`primary` to push, `info` when the remote diverged, muted
  without the user's commits), which differ from the state colours of the bars; the overview shows it uncoloured so the
  two do not clash.
- The `primary` green has a 2.2:1 contrast on the light card; the counts and the state words carry the values in text.
- No test covers a branch whose remote status is not loaded yet, or the bar of more than 8 branches in the page.

## References

- [Remote Status for Virtual Branches](../../docs/design/remote-status.md): how ahead, behind and mine are computed.
- [Branch Preview](../branch/branch-preview.spec.md): the same push button and labels.
- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
