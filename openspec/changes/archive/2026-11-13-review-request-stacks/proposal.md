# Proposal

## Why

A reviewer who finds twelve pull requests of one author in Your Turn in the morning sees twelve flat rows, though they
are one chain: each goes into the branch of the one below it, the bottom into master. Nothing says which belong
together or in which order they build on each other, and nothing shows the labels their author set on GitHub
("refactoring", "tests"), which are the first words of a triage.

## What Changes

- The query of the review requests (and the fragment of Reviews) asks the labels of each pull request,
  `labels(first: 20) { nodes { name } }`; `ReviewRequest.labels` carries their names. The search costs 3 points.
- Rule `domain_policy::review_request_chains::request_rows`: a request whose base is the head branch of another request
  of the repository is the layer above it; a chain that does not fork is one row, the bottom first, at the place of its
  layer that waits the longest. A branch two requests go into links neither, a request from a fork is never a layer
  below another, a request into the default branch is the bottom. `ReviewRequestsView.rows` carries the rows
  (`RequestRow`); the frame schema is 14.
- Your Turn and Waiting for You show a stack as one row, "Stack of 12 pull requests · #979–#991 · niyaz · +120 −30",
  collapsed; a click, Enter and → show its layers under it, indented, without the "→ base" their header says; ← hides
  them; ↓/↑ walk the header and the layers. A stack is one row of the three before "Show N More", unread with an unread
  layer; filtered, the stacks are open.
- A row shows the labels of its pull request as muted chips after the title, the first two and "+2"; the filter of
  Reviews matches them.
