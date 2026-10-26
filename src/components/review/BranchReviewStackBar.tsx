import { Fragment, useState } from "react"
import type { MouseEvent, ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { BranchReviewSegment } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import { selectSegment, stackBarItems } from "@/lib/branch-review"
import type { SegmentRange } from "@/lib/branch-review"
import { formatWholeStackStart } from "@/lib/whole-stack"
import type { ReviewedBranch, WholeStackScope } from "@/types/sub-window"

export interface BranchReviewStackBarProps {
  baselineBranch: string
  /** Root first, the reviewed branch last */
  segments: readonly BranchReviewSegment[]
  range: SegmentRange
  onRangeChange: (range: SegmentRange) => void
  /** The review of the whole stack: its one part is the stack branch of the root, named "Whole air stack" */
  wholeStack?: boolean
  /** "Whole Stack" after "From Root", the whole stack of the root (`WholeStackScope`); null where it holds nothing more */
  wholeStackScope?: WholeStackScope | null
  onWholeStack?: () => void
  /** The review of the whole stack: the chain it came from, each segment back to the review of its branch */
  from?: readonly ReviewedBranch[]
  onBranch?: (name: string) => void
}

/**
 * The stack of the reviewed branch from the baseline: a segment per branch with its commits. A click shows one branch
 * on top of its parent, Shift+click extends the range to the clicked branch, "From Root" shows everything from the
 * baseline to the branch (root..branch, what its Safe Push lands and Patronus checks). A long stack folds its middle
 * into "… 14 more", which unfolds it. The whole stack (`wholeStack`) is one part, its stack branch.
 */
export function BranchReviewStackBar({
  baselineBranch,
  segments,
  range,
  onRangeChange,
  wholeStack = false,
  wholeStackScope = null,
  onWholeStack,
  from = [],
  onBranch,
}: BranchReviewStackBarProps) {
  const isWhole = range.from === 0 && range.to === segments.length - 1
  const select = (index: number) => (event: MouseEvent) => onRangeChange(selectSegment(range, index, event.shiftKey))
  const commitCount = segments.reduce((sum, segment) => sum + segment.commits.length, 0)
  const [unfolded, setUnfolded] = useState(false)
  if (wholeStack && from.length > 0) {
    return <WholeStackBar baselineBranch={baselineBranch} from={from} onBranch={onBranch} />
  }
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
      {(segments.length > 1 || wholeStackScope !== null) && (
        // the scopes of the stack, one option set: the branch with what it is built on, every branch of the stack
        <ScopeToggle value={isWhole ? FROM_ROOT : null} onValueChange={value => (value === FROM_ROOT ? onRangeChange({ from: 0, to: segments.length - 1 }) : onWholeStack?.())}>
          {segments.length > 1 && (
            <Tooltip>
              <TooltipTrigger render={<ToggleGroupItem value={FROM_ROOT} className="shrink-0" />}>From Root</TooltipTrigger>
              <TooltipContent>{`${segments[0]!.name} to ${segments.at(-1)!.name} on ${baselineBranch}: ${formatBranchesAndCommits(segments.length, commitCount)}, what the Safe Push of ${segments.at(-1)!.name} lands · Shift+click a branch to extend`}</TooltipContent>
            </Tooltip>
          )}
          {wholeStackScope !== null && <WholeStackScopeItem scope={wholeStackScope} />}
        </ScopeToggle>
      )}
    </div>
  )
}

const FROM_ROOT = "from-root"
const WHOLE_STACK = "whole-stack"

/**
 * The scopes of the stack bar as one option set (`ToggleGroup`), the shown one pressed: a press of the pressed one keeps
 * it, a press of another goes there
 */
function ScopeToggle({ value, onValueChange, children }: { value: string | null; onValueChange?: (value: string) => void; children: ReactNode }) {
  return (
    <ToggleGroup
      aria-label="Scope"
      variant="outline"
      size="sm"
      spacing={0}
      value={value === null ? [] : [value]}
      onValueChange={next => {
        const chosen = next.find(it => it !== value)
        if (chosen !== undefined) {
          onValueChange?.(chosen)
        }
      }}
      className="ml-1 shrink-0"
    >
      {children}
    </ToggleGroup>
  )
}

/**
 * The stack bar of the review of the whole stack: the chain it came from (the main line from the island), each
 * segment back to the review of its branch, then "Whole Stack", pressed
 */
function WholeStackBar({ baselineBranch, from, onBranch }: { baselineBranch: string; from: readonly ReviewedBranch[]; onBranch: ((name: string) => void) | undefined }) {
  return (
    <div className="flex min-w-0 items-center gap-1.5" data-testid="branch-review-stack">
      <span className="min-w-8 shrink-[2] truncate text-muted-foreground">{baselineBranch}</span>
      {from.map(branch => (
        <Fragment key={branch.name}>
          <ChevronRightIcon aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
          <Button
            size="xs"
            variant="outline"
            aria-pressed={false}
            data-testid="branch-review-segment"
            data-name={branch.name}
            title={`Review ${branch.name}`}
            className="max-w-64 min-w-12 shrink"
            onClick={() => onBranch?.(branch.name)}
          >
            <span className="truncate">{branch.name}</span>
            <span className="text-muted-foreground tabular-nums">{branch.commits}</span>
          </Button>
        </Fragment>
      ))}
      <ScopeToggle value={WHOLE_STACK}>
        <WholeStackScopeItem scope={null} />
      </ScopeToggle>
    </div>
  )
}

/**
 * "Whole Stack": the scope of every branch of the stack, what its Safe Push lands; pressed while it is shown (`scope`
 * null). Its tooltip is the item itself, a child of the group, so the group keeps its joined edges. A disabled toggle
 * of Base UI takes neither the pointer nor the focus, so one that can't go now (the stack branch is not built) stays
 * enabled for its tooltip: announced as unavailable (`aria-disabled`), dimmed, its press canceled, the reason on hover
 */
function WholeStackScopeItem({ scope }: { scope: WholeStackScope | null }) {
  const disabled = scope?.disabled === true
  const item = (
    <ToggleGroupItem
      value={WHOLE_STACK}
      data-testid="branch-review-whole-stack"
      aria-disabled={disabled || undefined}
      className={cn("shrink-0", disabled && "opacity-50")}
      onPressedChange={disabled ? (_pressed, details) => details.cancel() : undefined}
    >
      Whole Stack
    </ToggleGroupItem>
  )
  if (scope === null) {
    return item
  }
  return (
    <Tooltip>
      <TooltipTrigger render={item} />
      <TooltipContent>{scope.description}</TooltipContent>
    </Tooltip>
  )
}
