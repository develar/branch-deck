import { cn } from "cn"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useBaselineOnRemote } from "@/hooks/useBaselineOnRemote"
import { usePushButton } from "@/hooks/usePushButton"
import { describeParentState, passedParentOf } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"
import { useBranch } from "@/stores/branch-sync"

/**
 * "on auth" for a stacked branch, where the tree of the branch table isn't visible: the branch it is built on, "on
 * master" when its declared parent has no commits, and the declared parent of a branch that isn't built (blocked, or
 * with a stack problem); nothing for a root
 */
export function StackBaseLabel({ name, className }: { name: string, className?: string }) {
  const stack = useBranch(name)?.stack
  const baseline = useBaselineOnRemote()
  const base = stack?.effectiveBase ?? (stack?.prBaseChanged === true ? baseline : stack?.declaredParent ?? null)
  if (base === null) {
    return null
  }
  return <span className={cn("shrink-0 text-xs text-muted-foreground", className)} data-testid="stack-base">{`on ${base}`}</span>
}

/**
 * The base in the Status cell of a stacked branch. "on login" while its push button offers Push Stack: the parent has
 * work for the remote, so a pull request of the branch alone would include the parent's commits. Else "on master" when
 * the declared parent has no commits, with why in its tooltip ("air-misc landed · IJAI-1184 is built on master").
 */
export function UnpushedParentLabel({ branch }: { branch: Branch }) {
  const push = usePushButton(branch, false, false)
  return push.action === null ? <PassedParentLabel branch={branch} /> : <StackBaseLabel name={branch.name} />
}

function PassedParentLabel({ branch }: { branch: Branch }) {
  const baseline = useBaselineOnRemote()
  const passed = passedParentOf(branch)
  if (passed === null) {
    return null
  }
  const base = branch.stack?.effectiveBase ?? baseline
  return (
    <Tooltip>
      <TooltipTrigger render={<span className="shrink-0 text-xs text-muted-foreground" data-testid="stack-base" />}>{`on ${base}`}</TooltipTrigger>
      <TooltipContent>{`${describeParentState(passed.parent, passed.state)} · ${branch.name} is built on ${base}`}</TooltipContent>
    </Tooltip>
  )
}
