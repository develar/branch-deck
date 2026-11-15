# Proposal

## Why

The review of e06a87938 found the empty landing indicator on the Welcome screen: the header group was gated only on a
valid path, so a disabled "Nothing on the way to master" stood there with no repository selected, where nothing can be
on its way and Activity, which comes into the same group with a repository, is absent too. And the empty indicator
showed the hollow node at half opacity, the same picture as the rows that still load ("checking…"), so empty did not
look empty.

## What Changes

- The indicator is of a selected repository, as Activity: none on the Welcome screen.
- Empty, it shows nothing: its node is hidden in its box, so it keeps its width; the node of the rows that load stays.
- The doc of `isLandingOverviewEmpty` says the button keeps its place.

## Capabilities

### Modified Capabilities

- `repository/landing-overview`: no indicator without a selected repository; the empty one shows nothing.

## Impact

- `src/components/repository/ConfigurationHeader.tsx`, `src/components/repository/LandingIndicator.tsx`,
  `src/lib/landing-overview.ts`, `tests/e2e/repository/browse.spec.ts`, three HTML snapshots of the header.
