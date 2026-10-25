/** Where agents work by the rule of the repository, `lease` (`branchdeck.agentLease`), once the Agent CLI is `installed` */
export function describeAgentsHere(lease: boolean, installed: boolean): string {
  if (lease) {
    return installed
      ? "You work in this checkout. Coding agents such as Claude Code lease worktrees of their own through bd and land each commit on your branch."
      : "You work in this checkout. With the Agent CLI, coding agents such as Claude Code lease worktrees of their own and land each commit on your branch."
  }
  return installed
    ? "Coding agents such as Claude Code work in this checkout beside you, through bd."
    : "With the Agent CLI, coding agents such as Claude Code work in this checkout beside you."
}
