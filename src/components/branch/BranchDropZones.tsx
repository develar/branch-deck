import type { ComponentType } from "react"
import { cn } from "cn"
import { CirclePauseIcon, CirclePlayIcon, Layers2Icon, LayersIcon } from "lucide-react"
import { decidePark, decideUnpark, revealRow, UNPARK_TARGET, UNSTACK_TARGET, useBranchDropTarget, useParkDropTarget, useTakenNames } from "@/hooks/useBranchDropTarget"
import type { CommitDropTarget } from "@/hooks/useCommitDrag"
import { dropTargetState } from "@/hooks/useCommitDrag"
import { unparkBranches } from "@/hooks/useParkBranch"
import { restackBranch } from "@/hooks/useStackActions"
import { stackOnDrop, unstackDrop } from "@/lib/branch-drop"

interface DropZoneProps {
  target: CommitDropTarget
  icon: ComponentType<{ className?: string }>
  label: string
  "data-testid": string
}

/**
 * A dashed zone of the drag bar (`DragTargetBar`), shown only while a drag is for it: its label, which names what the
 * drop does ("Park docs"), or why it refuses the drag (`data-drop-target="refused"`, muted)
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
      data-drop-target={dropTargetState(target)}
      title={target.refusal ?? undefined}
      className={cn(
        "flex max-w-xs min-w-0 items-center gap-2 rounded-lg border border-dashed px-3 py-2 text-sm",
        refused ? "border-border text-muted-foreground" : "border-primary/50 text-primary",
        !refused && target.isOver && "border-primary bg-primary/10",
      )}
    >
      <Icon className="size-4 shrink-0" />
      {/* one line: the bar keeps its height, and the status line above the zones gives a long reason whole */}
      <span className="truncate">{target.refusal ?? label}</span>
    </div>
  )
}

/** "Park docs": the dragged branches that land are parked, without a note */
export function ParkDropZone() {
  const target = useParkDropTarget()
  const taken = useTakenNames(decidePark)
  return <DropZone target={target} icon={CirclePauseIcon} label={`Park ${taken}`} data-testid="park-drop-zone" />
}

/** "Unpark core" while parked branches are dragged: they are unparked, those parked with them follow */
export function UnparkDropZone() {
  const taken = useTakenNames(decideUnpark)
  const target = useBranchDropTarget(UNPARK_TARGET, decideUnpark, names => void unparkBranches(names).then(unparked => unparked && revealRow(names[0])))
  return <DropZone target={target} icon={CirclePlayIcon} label={`Unpark ${taken}`} data-testid="unpark-drop-zone" />
}

/** "Unstack auth" while a stacked branch is dragged: it becomes a root of its own */
export function UnstackDropZone() {
  const taken = useTakenNames(unstackDrop)
  const target = useBranchDropTarget(UNSTACK_TARGET, unstackDrop, ([branch]) => {
    if (branch !== undefined) {
      void restackBranch(branch, null).then(unstacked => unstacked && revealRow(branch))
    }
  })
  return <DropZone target={target} icon={Layers2Icon} label={`Unstack ${taken}`} data-testid="unstack-drop-zone" />
}

/**
 * "Stack on ‹name›" over the Actions cell of the row under a dragged branch, with stacked branches on: a drop there
 * stacks the branch on the row's branch instead of moving its commits into it. Shown while the row or the chip itself
 * is under the pointer; a refusal shows its reason in the chip and in the drag bar.
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
      data-drop-target={dropTargetState(target)}
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
