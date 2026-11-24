# Proposal

## Why

The field of Reviews filtered only the open pull requests the session read: a pull request the user reviewed and that
closed yesterday was found nowhere by its words. A merged or closed one opened by its link failed its review fetch (the
session remembered only an open one) and offered Approve as an open one does.

## What Changes

- While the field holds words of three characters or more, the session searches GitHub for the closed pull requests
  the user reviewed or opened whose titles have them, once the words stood 400 ms, one search at a time, an answer kept
  two minutes; the view shows them last, "Closed and Merged", the newest closed first, each marked merged or closed.
- A merged or closed pull request is read only: Approve, Request Changes and the Safe Push are disabled for it, its
  readiness and what its approval counts for are never asked; its review window says it, and shows it from where it
  forked from its base.
- A merged or closed link is remembered for its review window, never kept as a link.
- A filter that leaves the view empty says so once the closed search answered, and always offers Search on GitHub.

Capabilities: `repository/reviews`.
