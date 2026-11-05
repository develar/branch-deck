import type { ComponentType, ReactNode } from "react"
import { cn } from "cn"
import { CirclePauseIcon, CirclePlayIcon, FolderInputIcon, LayersIcon, UndoDotIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { InlineBranchTarget } from "@/components/shared/InlineBranchTarget"
import { getBranchDropFacts, useDragStores } from "@/hooks/useCommitDrag"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import { parkBranches, unparkBranches } from "@/hooks/useParkBranch"
import { stackBranches } from "@/hooks/useStackActions"
import type { BranchDrop } from "@/lib/branch-drop"
import { parkDrop, showsParkDrop, showsUnparkDrop, stackTogetherDrop, unparkDrop } from "@/lib/branch-drop"
import { inTableOrder } from "@/lib/branch-selection"
import { useShownBranches } from "@/stores/branch-filter"
import { useBranchCommits, useBranchTableRows, useIssueGroups, useParkedTableRows } from "@/stores/branch-sync"
import { clearBranchSelection, setBranchTargetOpen, useCommitMove } from "@/stores/commit-move"
import { useRecentProjects } from "@/stores/settings/repositorySettings"
import { useShownBranchTables } from "@/stores/stack-collapse"

/** The selected branches in the order of the screen, their commits, and how many of them the view hides */
function useBranchSelection() {
  const selected = useCommitMove(state => state.selectedBranches)
  const rows = useBranchTableRows()
  // the rows of the Parked card are selected too, after the table
  const parkedRows = useParkedTableRows()
  const parkedNames = parkedRows.flatMap(row => (row.kind === "branch" ? [row.name] : []))
  const names = inTableOrder(selected, [...rows.flatMap(row => (row.kind === "branch" ? [row.name] : [])), ...parkedNames])
  const commits = useBranchCommits(names)
  // hidden by the filter, in a collapsed stack, a collapsed island or the collapsed Parked card
  const shown = useShownBranchTables(useShownBranches()).order
  const parkedOpen = useRecentProjects(projects => projects[0]?.parkedCollapsed !== true)
  const hiddenCount = names.filter(name => !shown.includes(name) && !(parkedOpen && parkedNames.includes(name))).length
  return { names, commits, hiddenCount }
}

/** The "Move to…" form of the selected branches, above the branch table while it is open */
export function BranchSelectionForm() {
  const isTargetOpen = useCommitMove(state => state.isBranchTargetOpen)
  const { names, commits } = useBranchSelection()
  if (names.length === 0 || !isTargetOpen) {
    return null
  }
  return (
    <div className="border-b border-border">
      <InlineBranchTarget commits={commits} sourceBranches={names} isActive onCancel={() => setBranchTargetOpen(false)} onSuccess={clearBranchSelection} />
    </div>
  )
}

/**
 * While branch rows are selected (Cmd/Ctrl-click, Shift-click): "N branches selected" (with "· 1 hidden" for the
 * selected branches the filter or a collapsed stack hides, which the actions take too), "Move to…" (the form that puts
 * the commits of all of them into one branch, existing or new, `BranchSelectionForm`), the way back, Park and Unpark (as
 * the targets of a drag of them, `branch-drop.ts`: those that land are parked, the parked ones unparked), and "N branches
 * selected" alone while the form is open. `overHead`: it takes the place of the column heads of the table, as high as
 * they are (`h-8`), so selecting or deselecting a branch never moves a row; otherwise (the table shows no rows) it is a
 * line above the islands.
 */
export function BranchSelectionBar({ overHead }: { overHead: boolean }) {
  const isTargetOpen = useCommitMove(state => state.isBranchTargetOpen)
  const issueGroups = useIssueGroups()
  const { names, commits, hiddenCount } = useBranchSelection()
  if (names.length === 0 || (isTargetOpen && !overHead)) {
    return null
  }

  const backLabel = getRemovePrefixLabel(commits, issueGroups)
  return (
    <div
      className={cn("flex items-center justify-end gap-2 bg-card px-4", overHead ? "absolute inset-x-0 top-0 h-8" : "border-b border-border py-2")}
      data-testid="branch-selection-actions"
    >
      <span className="text-xs text-muted-foreground">
        {names.length === 1 ? "1 branch selected" : `${names.length} branches selected`}
        {hiddenCount > 0 && ` · ${hiddenCount} hidden`}
      </span>
      {!isTargetOpen && (
        <>
          <Button size="xs" variant="outline" className="border-primary/50 text-primary hover:bg-primary/10 hover:text-primary" onClick={() => setBranchTargetOpen(true)}>
            <FolderInputIcon data-icon="inline-start" />
            Move to…
          </Button>
          {backLabel !== null && (
            <Button
              size="xs"
              variant="ghost"
              onClick={() => {
                void moveCommits(commits, null).then(success => {
                  if (success) {
                    clearBranchSelection()
                  }
                })
              }}
            >
              <UndoDotIcon data-icon="inline-start" />
              {backLabel}
            </Button>
          )}
          <SelectionShapeButton
            names={names}
            icon={LayersIcon}
            description="Stacks the selected branches, the oldest at the bottom"
            decide={stackTogetherSelection}
            act={stackBranches}
          >
            Stack Together
          </SelectionShapeButton>
          <SelectionShapeButton names={names} icon={CirclePauseIcon} decide={parkSelection} act={taken => parkBranches(taken, "")}>
            Park
          </SelectionShapeButton>
          <SelectionShapeButton names={names} icon={CirclePlayIcon} decide={unparkSelection} act={unparkBranches}>
            Unpark
          </SelectionShapeButton>
          <Tooltip>
            <TooltipTrigger render={<Button size="icon-xs" variant="ghost" aria-label="Clear the branch selection" onClick={clearBranchSelection} />}>
              <XIcon />
            </TooltipTrigger>
            <TooltipContent>Clear the selection (Esc)</TooltipContent>
          </Tooltip>
        </>
      )}
    </div>
  )
}

function parkSelection(names: readonly string[]): BranchDrop | null {
  const facts = getBranchDropFacts()
  return showsParkDrop(names, facts) ? parkDrop(names, facts) : null
}

function unparkSelection(names: readonly string[]): BranchDrop | null {
  const facts = getBranchDropFacts()
  return showsUnparkDrop(names, facts) ? unparkDrop(names, facts) : null
}

function stackTogetherSelection(names: readonly string[]): BranchDrop | null {
  return stackTogetherDrop(names, getBranchDropFacts())
}

interface SelectionShapeButtonProps {
  names: readonly string[]
  icon: ComponentType
  /** Whether the button shows for the selection (null hides it), and what it takes or why it can't now */
  decide: (names: readonly string[]) => BranchDrop | null
  act: (names: readonly string[]) => Promise<boolean>
  description?: string
  children: ReactNode
}

/** An action on the selected branches: disabled with the reason a drag of them would give, until it can */
function SelectionShapeButton({ names, icon: Icon, decide, act, description, children }: SelectionShapeButtonProps) {
  // "" hides the button, "\u0000" takes the selection, any other text is why it can't now
  const verdict = useDragStores(() => {
    const drop = decide(names)
    return drop === null ? "" : "refusal" in drop ? drop.refusal : "\u0000"
  })
  if (verdict === "") {
    return null
  }
  const reason = verdict === "\u0000" ? null : verdict
  return (
    <Tooltip disabled={reason === null && description === undefined}>
      <TooltipTrigger
        render={
          <Button
            size="xs"
            variant="ghost"
            disabled={reason !== null}
            onClick={() => {
              const drop = decide(names)
              if (drop !== null && "names" in drop) {
                void act(drop.names).then(done => done && clearBranchSelection())
              }
            }}
          />
        }
      >
        <Icon data-icon="inline-start" />
        {children}
      </TooltipTrigger>
      <TooltipContent>{reason ?? description}</TooltipContent>
    </Tooltip>
  )
}
