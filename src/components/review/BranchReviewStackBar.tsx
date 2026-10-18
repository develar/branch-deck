import { Fragment, useState } from "react"
import type { MouseEvent } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { BranchReviewSegment } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import { selectSegment, stackBarItems } from "@/lib/branch-review"
import type { SegmentRange } from "@/lib/branch-review"
import { formatWholeStackStart } from "@/lib/whole-stack"

export interface BranchReviewStackBarProps {
  baselineBranch: string
  /** Root first, the reviewed branch last */
  segments: readonly BranchReviewSegment[]
  range: SegmentRange
  onRangeChange: (range: SegmentRange) => void
  /** The review of the whole stack: its one part is the stack branch of the root, named "Whole air stack" */
  wholeStack?: boolean
}

/**
 * The stack of the reviewed branch from the baseline: a segment per branch with its commits. A click shows one branch
 * on top of its parent, Shift+click extends the range to the clicked branch, "From Root" shows everything from the
 * baseline to the branch (root..branch, what its Safe Push lands and Patronus checks). A long stack folds its middle
 * into "… 14 more", which unfolds it. The whole stack (`wholeStack`) is one part, its stack branch.
 */
export function BranchReviewStackBar({ baselineBranch, segments, range, onRangeChange, wholeStack = false }: BranchReviewStackBarProps) {
  const isWhole = range.from === 0 && range.to === segments.length - 1
  const select = (index: number) => (event: MouseEvent) => onRangeChange(selectSegment(range, index, event.shiftKey))
  const commitCount = segments.reduce((sum, segment) => sum + segment.commits.length, 0)
  const [unfolded, setUnfolded] = useState(false)
  return (
    // one line, a title bar's: the other branches give way before the shown ones and the last, which give way too
    // rather than run under the actions
    <div className="flex min-w-0 items-center gap-1.5" data-testid="branch-review-stack">
      <span className="min-w-8 shrink-[2] truncate text-muted-foreground">{baselineBranch}</span>
      {stackBarItems(segments.length, range, unfolded).map(item => {
        if (item.kind === "fold") {
          const folded = segments.slice(item.from, item.from + item.count)
          const selected = item.from <= range.to && item.from + item.count - 1 >= range.from
          return (
            <Fragment key={`fold:${item.from}`}>
              <ChevronRightIcon aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      size="xs"
                      variant="ghost"
                      data-testid="branch-review-fold"
                      className={cn("shrink-0 text-muted-foreground", selected && "text-primary hover:text-primary")}
                      onClick={() => setUnfolded(true)}
                    />
                  }
                >
                  {`… ${item.count} more`}
                </TooltipTrigger>
                <TooltipContent>{`${formatBranchesAndCommits(
                  folded.length,
                  folded.reduce((sum, it) => sum + it.commits.length, 0),
                )} · Click to show them`}</TooltipContent>
              </Tooltip>
            </Fragment>
          )
        }
        const index = item.index
        const segment = segments[index]!
        const selected = index >= range.from && index <= range.to
        return (
          <Fragment key={segment.name}>
            <ChevronRightIcon aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
            <Button
              size="xs"
              variant="outline"
              aria-pressed={selected}
              data-testid="branch-review-segment"
              data-name={segment.name}
              title={segment.name}
              className={cn(
                "max-w-64",
                selected || index === segments.length - 1 ? "min-w-20 shrink" : "min-w-12 shrink-[8]",
                selected && "border-primary/50 bg-primary/10 text-primary hover:bg-primary/15 hover:text-primary",
              )}
              onClick={segments.length > 1 ? select(index) : undefined}
            >
              <span className="truncate">{wholeStack ? formatWholeStackStart(segment.name) : segment.name}</span>
              <span className="text-muted-foreground tabular-nums">{segment.commits.length}</span>
            </Button>
          </Fragment>
        )
      })}
      {segments.length > 1 && (
        <Tooltip>
          <TooltipTrigger
            render={
              <Button
                size="xs"
                variant="ghost"
                aria-pressed={isWhole}
                className={cn("ml-1 shrink-0", isWhole && "text-primary hover:text-primary")}
                onClick={() => onRangeChange({ from: 0, to: segments.length - 1 })}
              />
            }
          >
            From Root
          </TooltipTrigger>
          <TooltipContent>{`${segments[0]!.name} to ${segments.at(-1)!.name} on ${baselineBranch}: ${formatBranchesAndCommits(segments.length, commitCount)}, what the Safe Push of ${segments.at(-1)!.name} lands · Shift+click a branch to extend`}</TooltipContent>
        </Tooltip>
      )}
    </div>
  )
}
