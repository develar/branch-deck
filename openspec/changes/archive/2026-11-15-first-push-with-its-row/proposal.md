# Proposal

## Why

In a repository on neither Space nor GitHub the first push of a branch showed only once the scheduler's answers of
Patronus and GitHub came, a patch after the rows, though git told them with no request: under load the push track grew
from 0 to 64 px and moved the status 52 px from under a still pointer, whose tooltip then never opened (the hover-back
test of `branch-row-layout.spec.ts`). A test that reads the layout of the rows had no way to wait for a first push.

## What Changes

- The session answers, as it takes the grouping of a run, that Patronus has no robots of a repository it doesn't know
  (`LandingRemote::patronus`) and that branches on no repository of GitHub (`LocalGitHub::None`) have no pull requests,
  with no request, in the patch with the rows; the scheduler's answers say the same later.
- The push track names the answer a first push or Push with Children waits for (`data-waits-for`); the push stays
  hidden until it in a repository Patronus or GitHub may land, as before.
- `waitForBranchLayout` waits for the push tracks too, `heldFacts` for a test that holds those answers; the fakes of a
  repository on no service hold the answers they give with no request.

## Capabilities

### Modified Capabilities

- `branch/branch-row`: the first push of a repository on no service comes with its row; a first push that waits names
  the answer on its track.
- `branch/landing-routes`: the session answers what git tells of the services with the grouping.

## Impact

- `crates/repository-model/src/runtime.rs`, `crates/test-server/src/session_routes.rs`,
  `src/components/branch/BranchActions.tsx`, `src/components/branch/BranchColumns.tsx`, `tests/e2e/helpers/`.
