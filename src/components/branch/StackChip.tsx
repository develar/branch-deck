import { ChevronRightIcon } from "lucide-react"
import { cn } from "cn"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotRing } from "@/components/branch/RobotProgress"
import { useStackStateSources } from "@/hooks/useStackStateSources"
import { formatBranchCount, formatBranchesAndCommits } from "@/lib/branch"
import { isMac } from "@/lib/platform"
import { summarizePushState } from "@/lib/push-summary"
import type { StackAttention, StackState } from "@/lib/stack-state"
import { getStackAttention, summarizeStackState } from "@/lib/stack-state"
import { stackDescendantsIn, useBranchSync } from "@/stores/branch-sync"

/** "⌥←" on macOS, "Alt+←" elsewhere */
function stackKeyLabel(collapse: boolean): string {
  return `${isMac ? "⌥" : "Alt+"}${collapse ? "←" : "→"}`
}

interface StackSummary {
  branchCount: number
  commitCount: number
  /** The branches of the stack with work for the remote, the root included */
  toPush: number
  /** What the branches stacked on the root hold, which a collapse hides; null while the stack is expanded */
  hidden: StackState | null
}

/**
 * The size of the stack on `root` and what its branches need, each counted as the header and the filter chips count it
 * (`summarizeStackState`); null for a branch nothing is stacked on
 */
function useStackSummary(root: string, collapsed: boolean): StackSummary | null {
  const sources = useStackStateSources()
  // a string, so the store notifies only when the summary changes
  const key = useBranchSync((state) => {
    const descendants = stackDescendantsIn(state, root).flatMap(name => state.branches[name] ?? [])
    const self = state.branches[root]
    if (descendants.length === 0 || self === undefined) {
      return ""
    }
    const hidden = collapsed ? summarizeStackState(descendants, sources) : null
    return JSON.stringify({
      branchCount: descendants.length + 1,
      commitCount: descendants.reduce((sum, branch) => sum + branch.commitCount, self.commitCount),
      toPush: summarizePushState([self, ...descendants], sources.isLanded, sources.isHeld).branches.length,
      hidden,
    } satisfies StackSummary)
  })
  return key === "" ? null : JSON.parse(key) as StackSummary
}

const attentionClassNames: Readonly<Record<StackAttention["color"], string>> = {
  error: "text-destructive",
  warning: "text-warning",
  primary: "text-primary",
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
 * expands it ("22 branches · 4 to push" while collapsed: the strongest state of the hidden rows); "3 of 22 branches"
 * under a filter. The tooltip sizes the stack: "Stack of vm-lane-daemon · 22 branches · 152 commits · 4 to push".
 */
export function StackChip({ root, collapsed, shown, filtered, onToggle }: StackChipProps) {
  const summary = useStackSummary(root, collapsed && !filtered)
  if (summary === null) {
    return null
  }
  const size = formatBranchesAndCommits(summary.branchCount, summary.commitCount)
  const toPush = summary.toPush > 0 ? ` · ${summary.toPush} to push` : ""
  const description = `Stack of ${root} · ${size}${toPush}`
  if (filtered) {
    return (
      <Tooltip>
        <TooltipTrigger render={<span className="inline-flex h-5 shrink-0 items-center px-1 text-xs text-muted-foreground tabular-nums" data-testid="stack-chip" data-stack="filtered" />}>
          {`${shown} of ${formatBranchCount(summary.branchCount)}`}
        </TooltipTrigger>
        <TooltipContent>{`${description} · The filter shows the matches of the stack`}</TooltipContent>
      </Tooltip>
    )
  }
  const attention = summary.hidden === null ? null : getStackAttention(summary.hidden)
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <button
            type="button"
            aria-expanded={!collapsed}
            aria-label={`Stack of ${root}: ${size}`}
            data-testid="stack-chip"
            data-stack={collapsed ? "collapsed" : "expanded"}
            className="inline-flex h-5 shrink-0 items-center gap-0.5 rounded-2xl px-1 text-xs text-muted-foreground tabular-nums outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30"
            onClick={(event) => {
              // the row toggles its commits on click
              event.stopPropagation()
              onToggle()
            }}
          />
        )}
      >
        <ChevronRightIcon aria-hidden className={cn("size-3.5 shrink-0 transition-transform duration-150", !collapsed && "rotate-90")} />
        {formatBranchCount(summary.branchCount)}
        {attention !== null && (
          <span className={cn("ml-1 inline-flex items-center gap-1", attentionClassNames[attention.color])} data-testid="stack-chip-attention">
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
