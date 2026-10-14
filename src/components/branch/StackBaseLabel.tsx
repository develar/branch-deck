import { cn } from "cn"
import { usePushButton } from "@/hooks/usePushButton"
import type { Branch } from "@/stores/branch-sync"
import { useBranch } from "@/stores/branch-sync"

/** "on auth" for a stacked branch, where the tree of the branch table isn't visible; nothing for a root */
export function StackBaseLabel({ name, className }: { name: string, className?: string }) {
  const stack = useBranch(name)?.stack
  const parent = stack?.effectiveBase ?? stack?.declaredParent ?? null
  if (parent === null) {
    return null
  }
  return <span className={cn("shrink-0 text-xs text-muted-foreground", className)} data-testid="stack-base">{`on ${parent}`}</span>
}

/**
 * "on login" in the Status cell of a stacked branch while its push button offers Push Stack: the parent has work for the
 * remote, so a pull request of the branch alone would include the parent's commits
 */
export function UnpushedParentLabel({ branch }: { branch: Branch }) {
  const push = usePushButton(branch, false, false)
  return push.action === null ? null : <StackBaseLabel name={branch.name} />
}
