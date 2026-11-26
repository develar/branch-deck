# Proposal

## Why

The groups of the heavy queue (#279, `2026-11-26-279-heavy-queue-groups`) take turns, but by default a group is the
affinity, one checkout: the 6 agents of one orchestrating session, each in its own lease, are 6 groups and take 6
turns for one, unless the session sets `BRD_HOST_GROUP`, which agents drop after a few rounds. The hand-off of that
night also asked for what the groups leave out: a way to mark a gate that unblocks others as urgent, to change that for
a run that already waits, and an end to about 15 manual cancel-and-re-gate rounds after review amends (same lease,
same measure class, another commit), where the older gate kept its place and ran for nothing.

## What Changes

- **The default group is the session that holds the lease** of the checkout while it lives: `session-<12 hex>` of
  `holder_pid:holder_started` of its `brd lease` slot (the coding agent that took the lease; all its subagents share
  it). A checkout that is no slot, or whose holder is gone, keeps its affinity. `--group` and `BRD_HOST_GROUP` still win.
- **`brd-host run --priority high|normal|low`**, `RunSpec::priority` and `EstimateAsk::priority` behind the feature
  `priority`: after its turn places a run, the runs of its group and its class (short, exclusive or neither) are
  ordered by priority over their own places, first come first within one priority. The head keeps its place, the runs of
  other groups keep their turns, and a long run never takes the place of a short one. A run that supersedes keeps the
  priority of the run it replaces unless it names one. The priority lives in `groups.json`.
- **`brd-host prioritize <host> <run> <level>`** (`Request::Prioritize`): the same order for a run that waits; a holder
  answers that its priority orders nothing, an unknown run refuses with `unknown_run`.
- **The gate of an amended head supersedes the waiting one**: a run of the checkout of another commit with the same
  key of its measures (`--measure-as`, else of its command) supersedes a waiting run, whatever its command; a run of the
  same commit keeps the rule of the same command, so an A/B of two bases of one commit both run. The commit and the key
  live in `groups.json`. The estimate places a run by the same rules.
- `brd-host runs` names a priority that is not normal; an explicit `--priority` on an agent without the feature refuses
  with `bad_config`, as `--measure-as` does.

## Decisions

- **The session, not the affinity, not the home checkout**: by the affinity 6 leases are 6 shares; by the home checkout
  2 to 4 independent sessions of one repository share one. The session of the client outside a lease is not used: the
  runner scan does not know `bt`, so every run of it would be a group alone. An environment variable as the mechanism
  is left out: agents drop prefixes.
- **The priority orders, it does not admit**: the queue changes at a join and at a `prioritize` only, so a supervisor
  of an older build, which admits the head, reads it right, and the turns of the groups, the short lane and their
  budgets keep their bounds.
- **No join inherited by a priority**: as a run of the turns, a run that goes ahead by its priority keeps its own join;
  it never passes the head.

Capabilities: `cli/host`.
