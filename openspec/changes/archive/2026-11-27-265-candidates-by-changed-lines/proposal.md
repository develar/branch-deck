# Proposal

## Why

GitHub squashes a pull request into one commit authored at the merge, with ` (#N)` after its title: no local commit
shares its identity (`CommitKey`). When a later commit of the pull request rewrote the lines of an earlier one (a fixup
of a review, or a plain follow-up), no local commit alone has the squash's patch, and the replay of the earlier one onto
the squash conflicts. `brd update` stopped on a round without `landed_as`, so `--landed` was refused
(`not_landed_round`), and an agent had to stop and ask; five such pull requests (17 commits) stopped one update
(branch-deck#265).

## What Changes

- After a conflict, a baseline commit of a local author that nothing took pairs with the one smallest set of local
  commits whose changed lines without context, summed in order, equal its own (modes composed, the last version of a
  binary file): a candidate by changed lines (`CandidateBy::ChangedLines`), one `diff-tree -p -U0`, none without a set
  of paths. A candidate by identity that a later commit completes becomes one too.
- A candidate is left out of the replay only when its change is already in the result, or in the upstream when the
  replay leaves out its whole unit; a set whose candidate fails is tried once more without it. `--landed` of a
  candidate by changed lines drops at once only when the upstream has its change as it is, else with `--user`.
- The round of each commit of the set prints `landed_as`; the report and `dropped` name the squash.
- The candidate names the commits it landed with (`landed_with`, once `fixups`) and what pairs it (`by`); the state of
  a session is version 5 and reads version 4.

Capabilities: `repository/landed-commits`, `cli/update`.
