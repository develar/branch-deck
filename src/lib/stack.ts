import type { Branch } from "@/stores/branch-sync"

/** The branch that keeps a blocked branch from being rebuilt (the parent, or the ancestor that is missing); null if not blocked */
export function blockingBranchOf(branch: Branch): string | null {
  if (branch.status !== "Blocked") {
    return null
  }
  const stack = branch.stack
  return stack?.blockedBy?.parent ?? stack?.effectiveBase ?? stack?.declaredParent ?? null
}
