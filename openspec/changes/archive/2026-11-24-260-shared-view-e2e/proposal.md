# Proposal

## Why

The acceptance of #260 is that the app and the IDE on one repository show the same last sync, Landed island and
estimates. The check time is tested on one backend; the Landed island (landed-seen) and the robot estimate (robot-eta)
moved into the session since, and are tested the same way now.

## What Changes

- The desktop window and the IDE view of one test repository show one Landed island, which the sight of the desktop
  window hides in both, and the same estimate of a running robot (`tests/e2e/air/one-backend.spec.ts`).

Capabilities: `repository/backend-protocol`.
