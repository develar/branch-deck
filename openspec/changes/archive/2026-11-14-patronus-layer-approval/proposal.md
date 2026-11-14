# Proposal

## Why

Your Turn groups a chain of someone else's pull requests into a stack, and Review Stack approves the layer shown. The
spec left open whether Patronus counts the approval of a layer whose base is not the target. The sources of Patronus
answer it: a Safe Push from a pull request goes into the base of that pull request, and a base no protected group names
(the branch of the layer below) refuses the start; the approvals counted are those of that pull request alone, on the
paths of all it lands, the latest review of each reviewer whatever commit it was on. So the approval of a layer above
the bottom counts for nothing until its base is master, and the approval of the bottom counts for no layer above it. The
window said neither.

## What Changes

- A rule `domain_policy::approval_landing`: what an approval of a pull request to review counts for where Patronus
  merges, of its base and the target its Safe Push is offered by: its own Safe Push (`OwnSafePush`), or once its base is
  the target (`OnceIntoTarget`); nothing without Patronus, before the target is known, and for an own pull request.
- `RepositoryView.approvalLanding` (and `TableChanges`), by `pull:<number>`, beside the reviewer groups; the patch, the
  Air mirror and the mirror of `brd mcp` carry it.
- Approve says it before the groups it counts for (`presentApprovalLanding`): a layer into the branch below "Counts once
  it goes into master: …", the bottom of a stack "Counts for the Safe Push of #301 alone, not of the layers above it".
- Capability `repository/review-requests`: the requirement, and the open question answered.
