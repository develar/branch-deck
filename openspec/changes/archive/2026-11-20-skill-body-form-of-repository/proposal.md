# Proposal

## Why

The skill gave every repository one form of the commit body, with `Tests:`, `Evidence:` and `Left out:` lists. In the
IntelliJ monorepo (2026-11-20) developers asked to drop these lists from master: a commit there compiles and passes its
tests, so a list of passed tests is noise in the history. The `commits` skill of the monorepo dropped them, but the
skill of Branch Deck still asked for them, so the agents with Branch Deck wrote them on.

## What Changes

- The body, as the subject, has the form of the repository: its agent instructions, else its `git log`.
- The code names in backticks and `Look closely:` stay: they serve the review of Branch Deck in every form.
- The lead, a paragraph per decision and the labels of the verification are the form of a repository with no form of
  its own.

Capabilities: `cli/agent-cli`.
