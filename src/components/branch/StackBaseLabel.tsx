import { cn } from "cn"
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
