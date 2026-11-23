# Proposal

## Why

"Checked just now" was a state of the page: the window whose focus found the inputs unchanged stamped its own clock,
and a run it heard of cleared it. Another window of the same session, or the tool window of the IDE, never heard of
the check, so two windows on one repository said "Checked just now" and "Last synced 18 minutes ago" (issue #260, the
states that diverge between windows move into the session).

## What Changes

- The time of the last check of a focus that found the inputs as the last sync read them is a part of the view of the
  session (`RepositoryView.inputsCheckedAt`, Unix seconds on the clock the time of the sync is recorded by), which a
  patch carries to every window; a run that ends after it takes it back.
- The page words it from the view; it no longer marks the check itself.

Capabilities: `repository/onboarding`.
