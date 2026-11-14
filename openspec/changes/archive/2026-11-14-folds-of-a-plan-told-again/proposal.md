# Proposal

## Why

The plan of git is sent before accio answers the modules, then again with them
(`2026-11-14-modules-from-the-index`). The window decided which groups start collapsed only when the plan came after
the files, so the plan told again kept the folds of the plan of git: their keys are by place (`groupRowKey`), and the
modules cut a large step into other groups. The Wiring of the step, at another place now, started open, and a row
folded as the build files of the plan of git could be another group, folded with no reason. Under load, where the
modules come after the window read the plan, the e2e test of the groups of a large step failed on it every time.

## What Changes

- The window keeps the groups of its tree that start collapsed (`foldedGroups`). A plan told again with other such
  groups folds those new to the tree, opens a row folded as such a group before that is none now, and keeps the
  reader's folds of every other row (`refoldGroups`).
- The fixture index of the e2e server holds the modules of a review plan (`HeldAnswer::Modules`), so a test sees the
  plan of git before them.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-plan`: the Mechanical group starts collapsed also when the plan is told again with other groups.

## Impact

- `src/components/commit/CommitReview.tsx` (`TreeSelection.folded`, `groupedSelection`), `src/lib/file-tree.ts`
  (`refoldGroups`).
- `crates/test-server/src/{remote_host,state}.rs`, `tests/e2e/helpers/test-repository.ts` (`modules` held).
