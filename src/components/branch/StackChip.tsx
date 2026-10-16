import { ChevronRightIcon } from "lucide-react"
import { cn } from "cn"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotRing } from "@/components/branch/RobotProgress"
import { formatBranchCount, formatBranchesAndCommits } from "@/lib/branch"
import { findBranchRobot, isRobotActive } from "@/lib/patronus"
import { isMac } from "@/lib/platform"
import { needsPush } from "@/lib/stack"
import { stackDescendantsIn, useBranchSync } from "@/stores/branch-sync"
import { useRepositoryRobots } from "@/stores/patronus"

/** "⌥←" on macOS, "Alt+←" elsewhere */
function stackKeyLabel(collapse: boolean): string {
  return `${isMac ? "⌥" : "Alt+"}${collapse ? "←" : "→"}`
}

interface StackSummary {
  branchCount: number
  commitCount: number
  toPush: number
  /** The branches stacked on the root, which a collapse hides */
  hidden: { toPush: number, conflicts: number, errors: number, blocked: number }
}

/** The size of the stack on `root` and what its branches need; null for a branch nothing is stacked on */
function useStackSummary(root: string): StackSummary | null {
  // a string, so the store notifies only when the summary changes
  const key = useBranchSync((state) => {
    const descendants = stackDescendantsIn(state, root).flatMap(name => state.branches[name] ?? [])
    const self = state.branches[root]
    if (descendants.length === 0 || self === undefined) {
      return ""
    }
    const count = (test: (branch: (typeof descendants)[number]) => boolean) => descendants.filter(test).length
    return [
      descendants.length + 1,
      descendants.reduce((sum, branch) => sum + branch.commitCount, self.commitCount),
      count(needsPush) + (needsPush(self) ? 1 : 0),
      count(needsPush),
      count(branch => branch.errorDetails?.MergeConflict != null),
      count(branch => branch.hasError && branch.errorDetails?.MergeConflict == null),
      count(branch => branch.status === "Blocked"),
    ].join(":")
  })
  if (key === "") {
    return null
  }
  const [branchCount, commitCount, toPush, hiddenToPush, conflicts, errors, blocked] = key.split(":").map(Number) as [number, number, number, number, number, number, number]
  return { branchCount, commitCount, toPush, hidden: { toPush: hiddenToPush, conflicts, errors, blocked } }
}

/** The robots running and failed on the branches stacked on `root` while they are hidden: "rrf" */
function useHiddenRobots(root: string, hidden: boolean): { running: number, failed: number } {
  const robots = useRepositoryRobots()
  const key = useBranchSync((state) => {
    if (!hidden) {
      return ""
    }
    return stackDescendantsIn(state, root).map((name) => {
      const branch = state.branches[name]
      const robot = findBranchRobot(robots, name, branch?.commits[0]?.hash, branch?.remoteStatus?.remoteTip ?? null)
      return robot === undefined ? "" : isRobotActive(robot) ? "r" : robot.status === "failure" ? "f" : ""
    }).join("")
  })
  return { running: key.split("r").length - 1, failed: key.split("f").length - 1 }
}

/** The strongest state the collapsed rows hide, in the order of the node colours, with its count */
function hiddenAttention(hidden: StackSummary["hidden"], robots: { running: number, failed: number }): { text: string, className: string, ring: boolean } | null {
  if (hidden.conflicts > 0) {
    return { text: `${hidden.conflicts} ${hidden.conflicts === 1 ? "conflict" : "conflicts"}`, className: "text-destructive", ring: false }
  }
  if (hidden.errors > 0) {
    return { text: `${hidden.errors} ${hidden.errors === 1 ? "error" : "errors"}`, className: "text-destructive", ring: false }
  }
  if (hidden.blocked > 0) {
    return { text: `${hidden.blocked} blocked`, className: "text-warning", ring: false }
  }
  if (robots.running > 0) {
    return { text: `${robots.running} running`, className: "text-primary", ring: true }
  }
  if (robots.failed > 0) {
    return { text: `${robots.failed} failed`, className: "text-destructive", ring: false }
  }
  if (hidden.toPush > 0) {
    return { text: `${hidden.toPush} to push`, className: "text-primary", ring: false }
  }
  return null
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
  const summary = useStackSummary(root)
  const robots = useHiddenRobots(root, collapsed)
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
  const attention = collapsed ? hiddenAttention(summary.hidden, robots) : null
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
          <span className={cn("ml-1 inline-flex items-center gap-1", attention.className)} data-testid="stack-chip-attention">
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
