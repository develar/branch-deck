# Proposal

## Why

In idea-1 the commit IJAI-6811 of apps-context-root conflicted because it needed seven commits of a2ui-v1. While the
Safe Push of git-worktrees, built on a2ui-v1, was landing them, the conflict view offered "Move … to a2ui-v1" but kept
Apply Fix disabled with "A Safe Push of a2ui-v1 is running". That told the user neither what to do nor what helps.

- The disable was wrong. A hold refuses a push, not a rewrite: the fix regroups local branches, and the robot lands
  the remote copy it took (`docs/design/landing.md`: "a fix doesn't disturb the robot"). No other rewrite but Park is
  refused by a hold.
- It named the branch of the fix, not the robot's.
- It said "is running" also for a Safe Push that had merged.
- It was a rule of the domain in TS (`usePushHolds` in `ConflictFixPanel`).

What helps depends on the robot that holds the branch that has what the conflict needs. If it lands that branch,
waiting fixes the conflict without a rewrite: after the merge and Update Project, the branch applies cleanly and lands
in one robot. If it is failing, waiting lands nothing. If it merged, Update Project brings what it landed.

## What Changes

- `FixComponent.holders`: the branches that hold what the steps bring. That is the target of a move or a restack and
  the sources of a pull in (`FixStep::holder`).
- `domain_policy::holds::fix_hold`: what the hold of a branch means for a fix (`FixHold`). A merged Safe Push gives
  `Landed`, a running one that will fail gives `Failing`, any other running one gives `Lands`. The view sends it per
  branch (`NextStepView.fixHold`), and `FRAME_SCHEMA` goes to 20.
- The conflict view:
  - `lands`: the title is "Wait for the Safe Push of git-worktrees", with what it lands and that Update Project then
    applies the branch cleanly. Below it comes "Or fix it now: <step>" with Apply Fix as the other choice, beside
    Open in Patronus and "Show <branch>" (the robot's row, where its cancel is).
  - `failing`: the fix stays the title and Apply Fix the primary action. A line says that the Safe Push is failing,
    and Restart Safe Push and Cancel Safe Push stand beside Apply Fix.
  - `landed`: "a2ui-v1 landed with git-worktrees" and Update Project replace the fix.
- No hold disables Apply Fix. It still waits until Patronus answered, so the offer is never made on an unknown hold.
