# Proposal

## Why

A row whose pull request awaits a review said nothing in its Status cell (its push state was up to date, quiet) and
"◷ Awaiting Review" among the buttons of its Actions: the state of the branch in the column of what the user can do,
beside the pull request chip of the same object in the Status column. The wait was a "quiet step", the one exception of
UX Writing to "a button is a verb": a press only opened the pull request, which its chip opens too. The same held for
Changes Requested, Draft, another base, the pull request of a whole stack, Merges when checks pass and the holds of a
merge.

## What Changes

- A status says where the work stands and whose move it is, and changes by itself; an action is what the user can do
  now. A wait is a status: `StepKind::is_wait` names the steps that are states, `NextStep.wait` carries it.
- The Status cell says the wait where no fact of the branch's own says more (`RowFact::Wait` of `row_status`, before a
  finished robot that covers nothing, never beside another fact), in sentence case: "Awaiting review", "Changes
  requested", "Draft", "Into release-252", "In #12", "Merges when checks pass"; a click and O open the pull request.
- The slot of the actions shows no step of a wait, in a row, in the header of an island (which says the wait of the
  whole stack beside its pull request) and in the landing overview (whose reason says it); `data-step` still names it.
- `brd status` names the fact `waits`.
