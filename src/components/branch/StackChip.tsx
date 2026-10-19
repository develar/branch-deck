import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotRing } from "@/components/branch/RobotProgress"
import { useStackStates } from "@/hooks/useStackStates"
import { formatBranchCount, formatBranchesAndCommits } from "@/lib/branch"
import { isMac } from "@/lib/platform"
import { getStackStateChips } from "@/lib/stack-state"
import { textColorClassName } from "@/lib/ui-types"
import { stackDescendantsIn, useBranchSync } from "@/stores/branch-sync"

/** "⌥←" on macOS, "Alt+←" elsewhere */
function stackKeyLabel(collapse: boolean): string {
  return `${isMac ? "⌥" : "Alt+"}${collapse ? "←" : "→"}`
}

interface StackSummary {
  branchCount: number
  commitCount: number
}

/** The size of the stack on `root`; null for a branch nothing is stacked on */
function useStackSummary(root: string): StackSummary | null {
  // a string, so the store notifies only when the summary changes
  const key = useBranchSync(state => {
    const descendants = stackDescendantsIn(state, root).flatMap(name => state.branches[name] ?? [])
    const self = state.branches[root]
    if (descendants.length === 0 || self === undefined) {
      return ""
    }
    return JSON.stringify({
      branchCount: descendants.length + 1,
      commitCount: descendants.reduce((sum, branch) => sum + branch.commitCount, self.commitCount),
    } satisfies StackSummary)
  })
  return key === "" ? null : (JSON.parse(key) as StackSummary)
}

export interface StackChipProps {
  root: string
  /** The stack is collapsed into its root */
  collapsed: boolean
  /** The branches of the stack the table shows, the root included; under a filter some of them */
  shown: number
  /** A filter shows its matches in their stacks: the stack doesn't collapse */
  filtered: boolean
  onToggle: () => void
}

/**
 * The header of a stack on its root row, after the name: "22 branches", which collapses the stack into its root and
 * expands it ("22 branches · 1 to fix" while collapsed: the strongest state of the hidden rows by their nodes, in the
 * colour of the node); "3 of 22 branches" under a filter. The tooltip sizes the stack and tells where its branches are
 * on their way into master: "Stack of vm-lane-daemon · 22 branches · 152 commits · 3 your step · 18 waiting".
 */
export function StackChip({ root, collapsed, shown, filtered, onToggle }: StackChipProps) {
  const summary = useStackSummary(root)
  // the root row shows its own node: the states are of the branches stacked on it, which a collapse hides
  const states = useStackStates(root, { withRoot: false })
  if (summary === null) {
    return null
  }
  const size = formatBranchesAndCommits(summary.branchCount, summary.commitCount)
  const chips = states === null ? [] : getStackStateChips(states.state)
  const description = [`Stack of ${root}`, size, ...chips.map(chip => chip.text)].join(" · ")
  if (filtered) {
    return (
      <Tooltip>
        <TooltipTrigger
          render={<span className="inline-flex h-5 shrink-0 items-center px-1 text-xs text-muted-foreground tabular-nums" data-testid="stack-chip" data-stack="filtered" />}
        >
          {`${shown} of ${formatBranchCount(summary.branchCount)}`}
        </TooltipTrigger>
        <TooltipContent>{`${description} · The filter shows the matches of the stack`}</TooltipContent>
      </Tooltip>
    )
  }
  const attention = collapsed ? (chips[0] ?? null) : null
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            aria-expanded={!collapsed}
            aria-label={`Stack of ${root}: ${size}`}
            data-testid="stack-chip"
            data-stack={collapsed ? "collapsed" : "expanded"}
            className="inline-flex h-5 shrink-0 items-center gap-0.5 rounded-2xl px-1 text-xs text-muted-foreground tabular-nums outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30"
            onClick={event => {
              // the row toggles its commits on click
              event.stopPropagation()
              onToggle()
            }}
          />
        }
      >
        <ChevronRightIcon aria-hidden className={cn("size-3.5 shrink-0 transition-transform duration-150", !collapsed && "rotate-90")} />
        {formatBranchCount(summary.branchCount)}
        {attention !== null && (
          <span className={cn("ml-1 inline-flex items-center gap-1", textColorClassName(attention.color))} data-testid="stack-chip-attention">
            {"· "}
            {attention.ring && <RobotRing className="size-3" />}
            {attention.text}
          </span>
        )}
      </TooltipTrigger>
      <TooltipContent>
        {description}
        {" · "}
        {collapsed ? "Expand Stack" : "Collapse Stack"}
        <Kbd>{stackKeyLabel(!collapsed)}</Kbd>
      </TooltipContent>
    </Tooltip>
  )
}
