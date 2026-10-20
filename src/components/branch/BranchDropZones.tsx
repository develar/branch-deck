import type { ComponentType } from "react"
import { cn } from "cn"
import { CirclePauseIcon, CirclePlayIcon, Layers2Icon, LayersIcon } from "lucide-react"
import { Card } from "@/components/ui/card"
import type { CommitDropTarget } from "@/hooks/useCommitDrag"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { parkBranches, unparkBranches } from "@/hooks/useParkBranch"
import { restackBranch } from "@/hooks/useStackActions"
import type { BranchDrop, BranchDropFacts } from "@/lib/branch-drop"
import { parkDrop, showsParkDrop, showsUnparkDrop, stackOnDrop, unparkDrop, unstackDrop } from "@/lib/branch-drop"
import { branchSyncStore, getBranch, getParkedState, getStackDescendants, isStacksEnabled } from "@/stores/branch-sync"
import { getPushHold } from "@/stores/patronus"

/** Drop target keys that no list has, so a drag from every list may come */
const PARK_TARGET = "\u0000park"
const UNPARK_TARGET = "\u0000unpark"
const UNSTACK_TARGET = "\u0000unstack"

function getBranchDropFacts(): BranchDropFacts {
  return {
    branch: getBranch,
    parked: getParkedState,
    hold: getPushHold,
    descendants: getStackDescendants,
    stacksEnabled: isStacksEnabled(),
    isSyncing: branchSyncStore.getState().isSyncing,
  }
}

/**
 * A target of a drag of whole branches (`branch-drop.ts`): `decide` says whether it shows (null hides it), takes the
 * branches or refuses them with the reason it shows; a drop runs `act` on the branches it took. A drag of commit rows
 * passes it by.
 */
function useBranchDropTarget(
  targetKey: string,
  decide: (names: readonly string[], facts: BranchDropFacts) => BranchDrop | null,
  act: (names: readonly string[]) => void,
): CommitDropTarget {
  return useCommitDropTarget(
    targetKey,
    drag => {
      const verdict = decide(drag.branches, getBranchDropFacts())
      if (verdict !== null && "names" in verdict) {
        act(verdict.names)
      }
    },
    drag => {
      if (drag.branches.length === 0) {
        return false
      }
      const verdict = decide(drag.branches, getBranchDropFacts())
      return verdict === null ? false : "refusal" in verdict ? verdict.refusal : true
    },
  )
}

interface DropZoneProps {
  target: CommitDropTarget
  icon: ComponentType<{ className?: string }>
  label: string
  "data-testid": string
}

/**
 * A dashed nested panel that a drag drops on, shown only while it is dragged: its label, or why it refuses the drag
 * (`data-drop-target="refused"`, muted)
 */
export function DropZone({ target, icon: Icon, label, "data-testid": testId }: DropZoneProps) {
  if (!target.canDrop && target.refusal === null) {
    return null
  }
  const refused = target.refusal !== null
  return (
    <div
      {...target.dropProps}
      data-testid={testId}
      data-drop-target={refused ? "refused" : target.isOver ? "over" : "ready"}
      className={cn(
        "flex flex-1 items-center justify-center gap-2 rounded-lg border border-dashed px-3 py-4 text-center text-sm",
        refused ? "border-border text-muted-foreground" : "border-primary/50 text-primary",
        !refused && target.isOver && "border-primary bg-primary/10",
      )}
    >
      <Icon className="size-4 shrink-0" />
      {target.refusal ?? label}
    </div>
  )
}

function useParkDropTarget(): CommitDropTarget {
  return useBranchDropTarget(
    PARK_TARGET,
    (names, facts) => (showsParkDrop(names, facts) ? parkDrop(names, facts) : null),
    names => void parkBranches(names, ""),
  )
}

/** The park zone at the top of the Parked card, also while it is collapsed: dropped branches are parked, without a note */
export function ParkedCardDropZone() {
  const target = useParkDropTarget()
  if (!target.canDrop && target.refusal === null) {
    return null
  }
  return (
    <div className="border-t border-border p-3">
      <DropZone target={target} icon={CirclePauseIcon} label="Drop here to park" data-testid="park-drop-zone" />
    </div>
  )
}

/** The park zone where the Parked card would be, while no branch is parked: a card of its own */
export function ParkDropCard() {
  const target = useParkDropTarget()
  if (!target.canDrop && target.refusal === null) {
    return null
  }
  return (
    <Card className="gap-0 p-3">
      <DropZone target={target} icon={CirclePauseIcon} label="Drop here to park" data-testid="park-drop-zone" />
    </Card>
  )
}

/** Above the branch table while parked branches are dragged: they are unparked, those parked with them follow */
export function UnparkDropZone() {
  const target = useBranchDropTarget(
    UNPARK_TARGET,
    (names, facts) => (showsUnparkDrop(names, facts) ? unparkDrop(names, facts) : null),
    names => void unparkBranches(names),
  )
  return <DropZone target={target} icon={CirclePlayIcon} label="Drop here to unpark" data-testid="unpark-drop-zone" />
}

/** Above the branch table while a stacked branch is dragged: it becomes a root of its own */
export function UnstackDropZone() {
  const target = useBranchDropTarget(UNSTACK_TARGET, unstackDrop, ([branch]) => {
    if (branch !== undefined) {
      void restackBranch(branch, null)
    }
  })
  return <DropZone target={target} icon={Layers2Icon} label="Drop here to unstack" data-testid="unstack-drop-zone" />
}

/**
 * "Stack on ‹name›" over the Actions cell of the row under a dragged branch, with stacked branches on: a drop there
 * stacks the branch on the row's branch instead of moving its commits into it. Shown while the row or the chip itself
 * is under the pointer; a refusal shows its reason in the chip.
 */
export function StackOnDropChip({ name, rowOver }: { name: string; rowOver: boolean }) {
  const target = useBranchDropTarget(
    `\u0000stack-on:${name}`,
    (names, facts) => stackOnDrop(names, name, facts),
    ([branch]) => {
      if (branch !== undefined) {
        void restackBranch(branch, name)
      }
    },
  )
  if ((!target.canDrop && target.refusal === null) || (!rowOver && !target.isOver)) {
    return null
  }
  const refused = target.refusal !== null
  return (
    <div
      {...target.dropProps}
      data-testid="stack-on-drop-chip"
      data-drop-target={refused ? "refused" : target.isOver ? "over" : "ready"}
      title={target.refusal ?? undefined}
      className={cn(
        "absolute inset-y-0 right-2 my-auto flex h-6 max-w-md items-center gap-1.5 rounded-md border border-dashed bg-background px-2 text-xs",
        refused ? "border-border text-muted-foreground" : "border-primary text-primary",
        !refused && target.isOver && "border-solid bg-primary text-primary-foreground",
      )}
    >
      <LayersIcon className="size-3 shrink-0" />
      <span className="truncate">{target.refusal ?? `Stack on ${name}`}</span>
    </div>
  )
}
