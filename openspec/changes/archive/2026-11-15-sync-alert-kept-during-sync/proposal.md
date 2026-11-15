# Proposal

## Why

The alert of a failed sync showed only while no sync ran: the error alert by `isSyncing` in `BranchCreator`, the
warning of the name and email by the code of the failed run, which a running view has not. In a repository whose syncs
fail, each sync took the alert away (the rows moved up by its height under a still pointer) and brought it back as it
failed again; "Sync Again" of the warning went from under the pointer that pressed it.

## What Changes

- The store keeps the failure the alerts show (`syncFailure`, its message and the code of a failed run) while the next
  sync runs, as "Last synced" stays, and drops it once a sync ended without one; both alerts read it. Sync turns red
  only after a failed run, as before.

## Capabilities

### Modified Capabilities

- `repository/onboarding`: the alert of a failed sync stays while the next sync runs.

## Impact

- `src/model/store.ts`, `src/stores/branch-sync.ts`, `src/components/branch/BranchCreator.tsx`,
  `src/components/repository/GitSetupAlert.tsx`, `src/model/store.test.ts`, `tests/e2e/onboarding/onboarding.spec.ts`,
  `docs/contributing/frontend.md`.
