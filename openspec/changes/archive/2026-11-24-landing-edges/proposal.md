# Proposal

## Why

Agents cut work into logical virtual branches, and the landing gate refuses a start once per name: on idea-1 of
2026-11-24 one need (the stack of `air-home-watch`, with `hosted-exit` and `claude-folds`, uses five names of
`acp-load-replay`) read as fifteen refusals, every row "Needs acp-load-replay" with a restack, while `acp-load-replay`
was landing in its own robot and a start beside it failed. A restack that would close a cycle offered the move of a
commit between the logical pieces of an agent.

## What Changes

- `domain_policy::landing_edges`: the refusals of a start merged into edges `from → needed`, by the branch they need
  and their kind (build, declared, companion), the names and the files their evidence; of every branch lifted to the
  root of its stack, a need the stack meets itself dropped. Each edge has a state: `Waits` (a running Safe Push lands
  what it needs), `NeededFailed`, `Open`, `Unassigned`, `Cycle`.
- `DanglingReference` names the running Safe Push of the branch of the commit that changes the use (`landing_in`), as
  `MissingDeclaration` and `UnlandedNeed` do.
- `LandingFix::for_need`: a cycle is the start of the branch built on both, or of the whole stack (`LandingFix::Start`),
  never a move; the move (the group) is only of an unassigned commit. The landing gate, the fix of a copy to
  regenerate and `brd` follow.

Capabilities: `branch/landing-plan` (new), `branch/branch-row`, `cli/agent-cli`.
