# Proposal

## Why

An agent asked to review a commit, a branch or a range had no recipe: the skill named `brd review-plan` only for the
code names of a commit body that resolve nothing, so an agent read the patch line by line, told formatting and moves
as changes, and seldom said that a change has no change of behaviour at all. The plan already proves what is left to
read (`reading`) and cuts the diff into the units a reviewer reads once, the mechanical ones apart.

## What Changes

- The skill says how to review by the plan: `brd review-plan`, its `reading` first (nothing to read is the report),
  then its `units` in the order printed, the `mechanical` ones passed by every kind the output prints, every other
  unit read; a move never quiet, read where it stands now; a rename no index proved checked; no removal skipped; a
  report of the changes of behaviour alone, each with its file, its lines and what observably changes, or that there
  is none.
- Its description names the review of a commit, a branch or a range among its triggers.
- The skill names only the fields its rules and the recipe act on; the details of an output stay in the binary.
- The line marks (formatting, renames, moves) stay out of the output of `brd` (Quiet Changes, non-goal), and the skill
  names no tool beside brd.

Capabilities: `cli/agent-cli`.
