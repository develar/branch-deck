import { useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { PopoverDescription, PopoverSectionTitle, PopoverTitle } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { LandingNodeSubject } from "@/components/branch/LandingNodeHover"
import { LandingNodeHover } from "@/components/branch/LandingNodeHover"
import { StepButton } from "@/components/branch/NextStepSlot"
import { LaneNodeDot } from "@/components/branch/StackLanes"
import { RobotList, RobotRow } from "@/components/repository/RobotRows"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import { useStackNextStep } from "@/hooks/useStackNextStep"
import { openActivity } from "@/lib/activity-window"
import { formatBranchCount } from "@/lib/branch"
import { formatCommitCount } from "@/lib/commit"
import type { LandingOverview, LandingUnit } from "@/lib/landing-overview"
import { formatFinishedSummary, formatLandingSectionTitle, formatLandingTitle, formatMoveUnit, urgentKindOf } from "@/lib/landing-overview"
import { FROM_MENU } from "@/lib/menu"
import { getRobotTiming } from "@/lib/robot-eta"
import type { RobotOverviewEntry } from "@/lib/robot-overview"
import { orderLandingRobots } from "@/lib/robot-overview"
import { useShowReviewReadiness } from "@/model/demand"
import type { LandingNode } from "@/presenters/landing-node"
import type { BranchNextStep } from "@/presenters/next-step"
import { requestReveal } from "@/stores/commit-move"
import { showMainView } from "@/stores/main-view"
import { getRobotEstimate } from "@/stores/patronus"
import { toggleStackCollapsed } from "@/stores/stack-collapse"

export interface LandingPopoverProps {
  overview: LandingOverview
  onClose: () => void
  renderPullRequest: (branchName: string) => ReactNode
  footer: ReactNode
}

/**
 * The overview of the work on its way into master: the user's moves with the next step of each (the same as the slot
 * of its row), the robots that land as a queue, what waits for others, what is in master until the update; a stack is
 * one row, its next step that of the whole stack. Only the present: the robots of the last day are one line, their
 * history is Activity. The title stays at the top and the line and the footer at the bottom, the sections scroll
 * between them, so the overview fits the window.
 */
export function LandingPopover({ overview, onClose, renderPullRequest, footer }: LandingPopoverProps) {
  const reveal = (name: string) => {
    requestReveal({ branchName: name, commitIds: [] })
    onClose()
  }
  // a branch whose robot failed on it as it is: the robot is the move, with its actions (restart, the prompts, open)
  const failedRobotOf = new Map(overview.robotFailures.map(entry => [entry.stackRoot ?? entry.branchName, entry]))
  const unitProps = { onClose, onReveal: reveal, renderPullRequest, failedRobotOf }
  const moveCount = overview.yourMove.length + overview.failedRobots.length + overview.failingRobots.length
  const queue = useLandingQueue(overview.running)
  const finished = formatFinishedSummary(overview.finished)
  const moveUnit = formatMoveUnit(overview)
  return (
    <>
      <div className="flex shrink-0 flex-col gap-1">
        <PopoverTitle className="text-sm">{formatLandingTitle(overview)}</PopoverTitle>
        {moveUnit !== null && (
          <PopoverDescription className="text-xs" data-testid="landing-move-unit">
            {moveUnit}
          </PopoverDescription>
        )}
        {overview.checking > 0 && (
          <PopoverDescription className="text-xs" data-testid="landing-checking">
            {`Checking ${formatBranchCount(overview.checking)}: Patronus and GitHub answer…`}
          </PopoverDescription>
        )}
      </div>
      {/*
        one grid of the sections: the pull request chips and the steps of the rows in their columns across the sections,
        as wide as their widest; a robot row spans it. It scrolls between the title and the bottom; the gutter keeps the
        focus rings and the scrollbar off the rows
      */}
      <div
        className="-mx-3 grid min-h-0 grid-cols-[minmax(0,1fr)_auto_auto] content-start gap-y-3 overflow-y-auto overscroll-contain px-3 empty:hidden"
        data-testid="landing-sections"
      >
        {moveCount > 0 && (
          <Section title={`Your move · ${overview.moves}`} testId="landing-your-move">
            {overview.yourMove.map(unit => (
              <UnitRow key={unit.name} unit={unit} {...unitProps} />
            ))}
            {/* a failed robot of no row and a running Safe Push that will fail: its actions are the move */}
            {[...overview.failedRobots, ...overview.failingRobots].map(entry => (
              <RobotRow key={entry.robot.id} entry={entry} onReveal={reveal} />
            ))}
          </Section>
        )}
        <div className="col-span-full empty:hidden">
          <RobotList title={formatLandingSectionTitle(overview)} testId="landing-running" entries={queue} onReveal={reveal} />
        </div>
        {overview.waiting.length > 0 && (
          <Section title={`Waiting · ${overview.waiting.length}`} testId="landing-waiting">
            {overview.waiting.map(unit => (
              <UnitRow key={unit.name} unit={unit} {...unitProps} />
            ))}
          </Section>
        )}
        {overview.inMaster.length > 0 && (
          <Section title={`In master · ${overview.inMaster.length}`} testId="landing-in-master">
            {overview.inMaster.map(unit => (
              <UnitRow key={unit.name} unit={unit} {...unitProps} />
            ))}
          </Section>
        )}
      </div>
      <FinishedLine summary={finished} onClose={onClose} />
      {/* hidden when every part renders nothing (a local baseline, no GitHub) */}
      <div className="flex min-w-0 shrink-0 flex-col gap-2 border-t border-border pt-3 text-xs empty:hidden">{footer}</div>
    </>
  )
}

/**
 * The running robots as a queue, the nearest to finishing first (`orderLandingRobots`), in the order they had when the
 * overview opened: a row never jumps under the pointer; a robot started since comes last, one that finished goes
 */
function useLandingQueue(running: readonly RobotOverviewEntry[]): readonly RobotOverviewEntry[] {
  const [order] = useState(() => {
    const now = Date.now()
    return orderLandingRobots(running, entry => getRobotTiming(entry.robot, getRobotEstimate(entry.robot), now)).map(entry => entry.robot.id)
  })
  const rank = new Map(order.map((id, index) => [id, index]))
  return running.toSorted((a, b) => (rank.get(a.robot.id) ?? rank.size) - (rank.get(b.robot.id) ?? rank.size))
}

/**
 * The robots of the last day in one line, their history in Activity ("Show in Activity", its Robots), and how the
 * landings went (Show Statistics, the Landing view)
 */
function FinishedLine({ summary, onClose }: { summary: string | null; onClose: () => void }) {
  return (
    <div className="flex shrink-0 items-center gap-2 text-xs text-muted-foreground" data-testid="landing-finished">
      <span className="min-w-0 flex-1">{summary}</span>
      <Button
        variant="ghost"
        size="xs"
        className="shrink-0"
        onClick={() => {
          onClose()
          showMainView("landing")
        }}
      >
        Show Statistics
      </Button>
      {summary !== null && (
        <Button
          variant="ghost"
          size="xs"
          className="shrink-0"
          onClick={() => {
            onClose()
            openActivity({ kind: "robots" })
          }}
        >
          Show in Activity
        </Button>
      )}
    </div>
  )
}

function Section({ title, testId, children }: { title: string; testId: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="col-span-full grid grid-cols-subgrid gap-y-1" data-testid={testId}>
      <PopoverSectionTitle className="col-span-full">{title}</PopoverSectionTitle>
      <ul className="col-span-full grid grid-cols-subgrid gap-y-1">{children}</ul>
    </section>
  )
}

interface UnitRowProps {
  unit: LandingUnit
  /** The robots that failed on their branch as it is, by branch (the root for the whole stack) */
  failedRobotOf: ReadonlyMap<string, RobotOverviewEntry>
  onClose: () => void
  onReveal: (name: string) => void
  renderPullRequest: (branchName: string) => ReactNode
}

/** A unit: a stack takes the step of its whole stack, its branches that need a fix below it */
function UnitRow(props: UnitRowProps) {
  const { unit, failedRobotOf, onReveal } = props
  const failed = unit.node.kind === "needsFix" ? failedRobotOf.get(unit.name) : undefined
  if (failed !== undefined) {
    return <RobotRow entry={failed} onReveal={onReveal} />
  }
  return unit.branchCount > 1 ? <StackUnitRow {...props} /> : <BranchUnitRow {...props} />
}

function BranchUnitRow({ unit, onClose, onReveal, renderPullRequest }: UnitRowProps) {
  const step = useBranchNextStep(unit.name)
  return (
    <LandingRow
      name={unit.name}
      node={unit.node}
      size={formatCommitCount(unit.commitCount)}
      step={step}
      onClose={onClose}
      onReveal={onReveal}
      pullRequest={renderPullRequest(unit.name)}
    />
  )
}

/** The step of the whole stack (`useStackNextStep`: of its stack branch, or of the last branch of a straight chain) */
function StackUnitRow({ unit, failedRobotOf, onClose, onReveal, renderPullRequest }: UnitRowProps) {
  const next = useStackNextStep(unit.name)
  // the stack is the row: its name shows the stack, expanded
  const revealStack = (name: string) => {
    toggleStackCollapsed(name, false)
    onReveal(name)
  }
  // the popover shows the step of the last branch of a chain: its readiness is asked as its row's
  useShowReviewReadiness(next?.leaf ?? "")
  return (
    <>
      <LandingRow
        name={unit.name}
        node={unit.node}
        size={`${formatBranchCount(unit.branchCount)} · ${formatCommitCount(unit.commitCount)}`}
        subject={next == null || next.leaf === null ? { kind: "stack", root: unit.name } : { kind: "branch", name: next.leaf }}
        step={next?.step ?? null}
        stepOwner={next?.leaf ?? unit.name}
        onClose={onClose}
        onReveal={revealStack}
        pullRequest={next?.leaf == null ? null : renderPullRequest(next.leaf)}
        kind={urgentKindOf(unit) === "needsFix" && unit.node.kind !== "needsFix" ? unit.node.kind : undefined}
      />
      {unit.fixes.map(fix => {
        const failed = failedRobotOf.get(fix.name)
        return failed === undefined ? (
          <FixRow key={fix.name} name={fix.name} node={fix.node} onClose={onClose} onReveal={onReveal} renderPullRequest={renderPullRequest} />
        ) : (
          <li key={fix.name} className="col-span-full pl-6">
            <ul>
              <RobotRow entry={failed} onReveal={onReveal} />
            </ul>
          </li>
        )
      })}
    </>
  )
}

/** A branch of a stack that needs a fix of its own, below its stack */
function FixRow({ name, node, onClose, onReveal, renderPullRequest }: { name: string; node: LandingNode } & Omit<UnitRowProps, "unit" | "failedRobotOf">) {
  const step = useBranchNextStep(name)
  return <LandingRow name={name} node={node} size={null} step={step} nested onClose={onClose} onReveal={onReveal} pullRequest={renderPullRequest(name)} />
}

interface LandingRowProps {
  name: string
  node: LandingNode
  /** Whose node it is: the branch, the stack branch of a forking stack, the last branch of a straight chain */
  subject?: LandingNodeSubject
  /** "2 commits", "5 branches · 37 commits" */
  size: string | null
  step: BranchNextStep | null
  /** The branch whose row the step belongs to: shown before the step runs (its popovers open at its slot) */
  stepOwner?: string
  /** A branch of the stack above */
  nested?: boolean
  /** The kind of the node of the row when it differs from the node of the unit */
  kind?: LandingNode["kind"]
  pullRequest: ReactNode
  onClose: () => void
  onReveal: (name: string) => void
}

/**
 * The node and the name (a click shows its row), the size, the first line of the node's hover (the hover of the node on
 * hover, `LandingNodeHover`: its links, "Show auth", the robot), the pull request chip and the next step: a click
 * closes the overview, shows the row and runs the step there, as the slot of the row would
 */
function LandingRow({ name, node, subject = { kind: "branch", name }, size, step, stepOwner = name, nested = false, kind, pullRequest, onClose, onReveal }: LandingRowProps) {
  const reason = node.label.split("\n")[0] ?? ""
  const item = step?.item ?? null
  return (
    <li className="col-span-full grid grid-cols-subgrid items-center" data-testid="landing-row" data-branch-name={name} data-kind={node.kind}>
      {/* the node and the name; a branch of the stack above is indented one step */}
      <span className={cn("flex min-w-0 items-center gap-2", nested && "pl-6")}>
        <LaneNodeDot kind={kind ?? node.kind} />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="flex min-w-0 items-baseline gap-2">
            <Tooltip>
              <TooltipTrigger
                render={
                  <button
                    type="button"
                    className="min-w-0 truncate rounded-sm text-left text-sm outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
                    onClick={() => onReveal(name)}
                  />
                }
              >
                {name}
              </TooltipTrigger>
              <TooltipContent>{`Show ${name}`}</TooltipContent>
            </Tooltip>
            {size !== null && <span className="shrink-0 text-xs text-muted-foreground tabular-nums">{size}</span>}
          </span>
          {/* the hover of its node: what it names opens from there, a branch it shows closes the overview */}
          <LandingNodeHover
            node={node}
            subject={subject}
            onReveal={onReveal}
            trigger={<span className="min-w-0 truncate text-xs text-muted-foreground" data-testid="landing-row-reason" />}
          >
            {reason}
          </LandingNodeHover>
        </span>
      </span>
      <span className="flex items-center gap-1 not-empty:ms-2">{pullRequest}</span>
      {item !== null && (
        <span className="ms-2 flex justify-self-end">
          <StepButton
            action={{
              ...item,
              onSelect: () => {
                onClose()
                // the step runs at the row: its popovers (Safe Push, Request Review…) open at its slot
                requestReveal({ branchName: stepOwner, commitIds: [] })
                item.onSelect(FROM_MENU)
              },
            }}
            tone={step!.tone}
            showKeys={false}
          />
        </span>
      )}
    </li>
  )
}
