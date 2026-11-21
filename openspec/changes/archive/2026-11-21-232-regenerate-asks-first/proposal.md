# Proposal

## Why

The change `232-generated-merged-by-words` makes a sync tell which generated files the words merged in the copy of a
branch and the commit the copy lacks ([develar/branch-deck#232](https://github.com/develar/branch-deck/issues/232)).
That copy is what a push sends and a robot or a merge lands, while the user's tree builds: the row must say so, and
the push and the start must not send it without the user's word.

## What Changes

- `domain_policy::regenerate`: the fact of the row (`RowFact::Regenerate`, none for a branch in master or parked),
  its fix (`LandingFix::for_need`, moved out of the landing gate: the restack of the root of what lands onto the branch
  of the lacking commit, else the move of the commit) and the confirmation (`Confirmation::Regenerate`).
- The Status cell says "regenerate Cargo.lock" in `warning`, after a block, before a finished robot, beside a running
  one; its hover names the file, why it is generated, the commit the copy lacks and the fix.
- The push of a branch alone, of a stack (each copy it sends and the stack branch going along) and of the stack branch
  of a whole stack asks first, also when forced; Push Anyway pushes. Every start (Patronus, a Dry Run, the push and
  the merge routes) confirms it; Start Anyway starts. Nothing remembers an answer: each push and start asks again
  until the copy no longer holds the file.

Capabilities: `branch/branch-row`, `repository/outgoing`, `branch/patronus`.
