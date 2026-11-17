# Proposal

## Why

After every sync that moved a branch tip the session computes the landing needs of the new tips, and until they came
the step of every row waited for them ("Checking…"): 0.1–0.2 s in the E2E fixtures, about 0.2 s warm and 0.7–1.2 s
cold on the monorepo, two patches of every next step per sync, which rendered the step of every row. The answer is the
same almost always: one more commit on one branch changes the needs of nobody. A stale readiness already keeps the
step of the answer before, and the needs of the cached frame are values; only a start must read fresh facts.

## What Changes

- The needs of tips that moved on the same baseline stand until the new ones come (`PatronusPart.landingNeeds`
  `stale`): each branch keeps the answer of the tips before while it was in that table, built on the same branches, and
  every branch its refusals name is still a branch of the table the fetched baseline doesn't have whole. Its step, its
  node and the counts stay, so a step whose needs did not change gets no patch. A move of the baseline (Update
  Project, a fetch), a commit a merged Safe Push landed, a new branch, a branch built on others now and a need of a
  branch gone wait as for the first answer.
- The step carries no mark of it: a start reads only fresh needs. Its decision waits for them (`LandFact::LandingNeeds`
  in `RepositoryView.lands` and in the plan of `prepare`), a press (the slot, S, the popover, "⋯", Dry Run, Start
  Anyway) waits as it waits for any fact, in the repository it was pressed in, and starts nothing once another one
  opens, also when the first one shows again before its plan came; the engine plans a start on fresh needs on every route, its checks run once they are.
- The fix of row 1′ ("Stack lane-run on stills-codec") runs on fresh needs: its rewrite names the refusal it fixes
  (`RewriteIntent::fixes`), and the engine waits for the fresh needs, then runs it while they still refuse with it,
  else refuses it (`plan_changed`) and the Safe Push popover opens where the start opens (the whole stack at the
  header of its island): never a second rewrite on needs the first one
  changed. A declared need refuses a start at once, and the start still waits for the gate beside it.
- Start Anyway is bound to the refusals it showed and those it lifted before (`Land.anyway`): the overrides lift those
  only, a refusal of newer needs refuses the start, whose toast asks again; the flags of `brd safe-push` lift every
  refusal of their kind. It accepts only the confirmations it showed: a newer one opens the popover. It is settled
  and decided as a click is, one path for both.
- The page exposes how the needs stand, with no word: `data-landing-needs` of Sync, `aria-busy` and
  `data-landing-needs` of the landing indicator, whose counts are those of the nodes the rows show; the E2E helper that
  waits for the facts of the steps waits for fresh needs too.
- `BranchTable.baseline_commit`: the commit of the baseline a sync built the copies on, which the needs keep with their
  answer.
- `perf:table` counts the steps a sync of one more commit on one branch renders (`oneBranchSyncSteps`).
