import type { ReactNode } from "react"
import { cn } from "cn"
import { PopoverDescription, PopoverTitle } from "@/components/ui/popover"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { StackBaseLabel } from "@/components/branch/StackBaseLabel"
import type { OutgoingBranch, OutgoingState, PushSummary } from "@/lib/push-summary"
import { formatOutgoingCount, formatOutgoingTitle, getOutgoingLegend, getPushHue, outgoingBarFill } from "@/lib/push-summary"
import { useBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"

export interface OutgoingPopoverProps {
  summary: PushSummary
  /** Branches with an open pull request; those with nothing to push get a row after the outgoing ones */
  reviewBranches?: readonly string[]
  /** Closes the popover: a click on a branch name shows its row */
  onClose: () => void
  /** The pull request chip of a branch, between the state and the push button */
  renderPullRequest?: (branchName: string) => ReactNode
  /** Below the rows, after a rule: the update state of the baseline */
  footer?: ReactNode
}

/** Row word of a state; a plain push needs none */
const stateWord: Readonly<Record<OutgoingState, string>> = {
  push: "",
  create: "new",
  force: "force",
}

/**
 * The overview of what is waiting to be pushed: a row per branch with a bar proportional to the largest push (solid for
 * the user's commits), the mine/total count, the state and the push button of the branch row. Pushing stays per branch.
 * The branches with an open pull request and nothing to push follow, so the overview says what the work waits on.
 */
export function OutgoingPopover({ summary, reviewBranches = [], onClose, renderPullRequest, footer }: OutgoingPopoverProps) {
  const max = Math.max(1, ...summary.branches.map(branch => branch.count))
  const notes: string[] = []
  if (summary.mine !== summary.total) {
    notes.push(`${summary.mine} of them yours`)
  }
  if (summary.notLoaded > 0) {
    notes.push(`${summary.notLoaded} ${summary.notLoaded === 1 ? "branch" : "branches"} not checked yet`)
  }

  const outgoingNames = new Set(summary.branches.map(branch => branch.name))
  const reviewOnly = reviewBranches.filter(name => !outgoingNames.has(name))

  const reveal = (name: string) => {
    requestReveal({ branchName: name, commitIds: [] })
    onClose()
  }

  return (
    <>
      <div className="flex flex-col gap-1">
        <PopoverTitle className="text-sm">{formatOutgoingTitle(summary, reviewBranches.length)}</PopoverTitle>
        {notes.length > 0 && <PopoverDescription className="text-xs">{notes.join(" · ")}</PopoverDescription>}
      </div>

      <div
        role="list"
        className={cn(
          "grid items-center gap-x-3",
          renderPullRequest === undefined
            ? "grid-cols-[minmax(0,max-content)_5rem_auto_auto_auto]"
            : "grid-cols-[minmax(0,max-content)_5rem_auto_auto_auto_auto]",
        )}
      >
        {summary.branches.map(branch => (
          <OutgoingRow key={branch.name} branch={branch} max={max} onReveal={reveal} renderPullRequest={renderPullRequest} />
        ))}
        {reviewOnly.map(name => <ReviewRow key={name} name={name} onReveal={reveal} renderPullRequest={renderPullRequest} />)}
      </div>

      {summary.branches.length > 0 && <OutgoingLegend summary={summary} />}

      {/* hidden when every part renders nothing (a local baseline, no GitHub) */}
      {footer !== undefined && <div className="flex flex-col gap-2 border-t border-border pt-3 text-xs empty:hidden">{footer}</div>}
    </>
  )
}

interface OutgoingRowProps {
  branch: OutgoingBranch
  max: number
  onReveal: (name: string) => void
  renderPullRequest?: (branchName: string) => ReactNode
}

function OutgoingRow({ branch, max, onReveal, renderPullRequest }: OutgoingRowProps) {
  return (
    <div
      role="listitem"
      className="col-span-full grid min-h-7 grid-cols-subgrid items-center"
      data-testid="outgoing-branch"
      data-branch-name={branch.name}
      data-state={branch.state}
    >
      <BranchNameButton name={branch.name} onReveal={onReveal} />
      <OutgoingRowBar branch={branch} max={max} />
      <span className="text-xs whitespace-nowrap text-muted-foreground tabular-nums">{formatOutgoingCount(branch)}</span>
      <span className="text-xs text-muted-foreground">{stateWord[branch.state]}</span>
      {renderPullRequest !== undefined && <span className="flex items-center">{renderPullRequest(branch.name)}</span>}
      <span className="flex justify-end">
        <OutgoingPushButton name={branch.name} />
      </span>
    </div>
  )
}

/** A branch with nothing to push and an open pull request: the name and the chip, the other columns empty */
function ReviewRow({ name, onReveal, renderPullRequest }: { name: string, onReveal: (name: string) => void, renderPullRequest?: (branchName: string) => ReactNode }) {
  return (
    <div role="listitem" className="col-span-full grid min-h-7 grid-cols-subgrid items-center" data-testid="outgoing-review-branch" data-branch-name={name}>
      <BranchNameButton name={name} onReveal={onReveal} />
      <span />
      <span className="text-xs text-muted-foreground">pushed</span>
      <span />
      {renderPullRequest !== undefined && <span className="flex items-center">{renderPullRequest(name)}</span>}
      <span />
    </div>
  )
}

/** Shows the branch row */
function BranchNameButton({ name, onReveal }: { name: string, onReveal: (name: string) => void }) {
  return (
    <button
      type="button"
      className="min-w-0 truncate rounded-sm text-left text-sm outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
      title={`Show ${name}`}
      onClick={() => onReveal(name)}
    >
      {name}
      {/* the popover has no tree: a stacked branch says what it is built on */}
      <StackBaseLabel name={name} className="ml-1.5" />
    </button>
  )
}

/** On a `bg-muted` track, the commits of the push: the user's solid, the others' light; solid for a push of no commit. */
function OutgoingRowBar({ branch, max }: { branch: OutgoingBranch, max: number }) {
  const fill = outgoingBarFill(getPushHue(branch.behind))
  return (
    <span aria-hidden className="h-1.5 rounded-sm bg-muted">
      <span
        className={cn("flex h-full min-w-0.5 overflow-hidden rounded-sm", branch.count === 0 ? fill.mine : fill.others)}
        style={{ width: `${(branch.count / max) * 100}%` }}
      >
        {branch.count > 0 && <span className={cn("h-full", fill.mine)} style={{ width: `${(branch.mine / branch.count) * 100}%` }} />}
      </span>
    </span>
  )
}

/**
 * The push button of the branch row, following the branch record (pushing, syncing). Uncoloured: the bar of the row
 * carries the colour, and a column of coloured buttons would only repeat it.
 */
function OutgoingPushButton({ name }: { name: string }) {
  const branch = useBranch(name)
  return branch === undefined ? <span /> : <BranchPushButton branch={branch} colored={false} />
}

/** What the colours mean: a swatch per hue present, then solid and light when some commits are others'. */
function OutgoingLegend({ summary }: { summary: PushSummary }) {
  const entries = getOutgoingLegend(summary)
  const fill = outgoingBarFill(entries[0]?.hue ?? "primary")
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground" data-testid="outgoing-legend">
      {entries.map(entry => <LegendItem key={entry.label} swatch={outgoingBarFill(entry.hue).mine} label={entry.label} />)}
      {summary.mine < summary.total && (
        <>
          <span aria-hidden className="h-3 w-px bg-border" />
          <LegendItem swatch={fill.mine} label="yours" />
          <LegendItem swatch={fill.others} label="others'" />
        </>
      )}
    </div>
  )
}

function LegendItem({ swatch, label }: { swatch: string, label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span aria-hidden className={cn("size-2 rounded-sm", swatch)} />
      {label}
    </span>
  )
}
