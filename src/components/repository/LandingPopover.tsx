import type { ReactNode } from "react"
import { PopoverDescription, PopoverSectionTitle, PopoverTitle } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { StepButton } from "@/components/branch/NextStepSlot"
import { LaneNodeDot } from "@/components/branch/StackLanes"
import { RobotList, RobotRow } from "@/components/repository/RobotRows"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import { useStackNextStep } from "@/hooks/useStackNextStep"
import { formatBranchCount } from "@/lib/branch"
import type { BranchNextStep } from "@/lib/branch-next-step"
import { formatCommitCount } from "@/lib/commit"
import type { LandingNode } from "@/lib/landing-node"
import type { LandingOverview, LandingUnit } from "@/lib/landing-overview"
import { formatLandingTitle, urgentKindOf } from "@/lib/landing-overview"
import type { RobotOverviewEntry } from "@/lib/robot-overview"
import { requestReveal } from "@/stores/commit-move"
import { useShowReviewReadiness } from "@/stores/review-readiness"
import { toggleStackCollapsed } from "@/stores/stack-collapse"

export interface LandingPopoverProps {
  overview: LandingOverview
  onClose: () => void
  renderPullRequest: (branchName: string) => ReactNode
  footer: ReactNode
}

/**
 * The overview of the work on its way into master: the user's moves with the next step of each (the same as the slot
 * of its row), the robots that land, what waits for others, what is in master until the update, the robots of the last
 * day; a stack is one row, its next step that of the whole stack
 */
export function LandingPopover({ overview, onClose, renderPullRequest, footer }: LandingPopoverProps) {
  const reveal = (name: string) => {
    requestReveal({ branchName: name, commitIds: [] })
    onClose()
  }
  // a branch whose robot failed on it as it is: the robot is the move, with its actions (restart, the prompts, open)
  const failedRobotOf = new Map(overview.robotFailures.map(entry => [entry.stackRoot ?? entry.branchName, entry]))
  const unitProps = { onClose, onReveal: reveal, renderPullRequest, failedRobotOf }
  const moveCount = overview.yourMove.length + overview.failedRobots.length
  return (
    <>
      <div className="flex flex-col gap-1">
        <PopoverTitle className="text-sm">{formatLandingTitle(overview)}</PopoverTitle>
        {overview.checking > 0 && (
          <PopoverDescription className="text-xs" data-testid="landing-checking">
            {`Asking Patronus about ${formatBranchCount(overview.checking)}…`}
          </PopoverDescription>
        )}
      </div>
      {moveCount > 0 && (
        <Section title={`Your move · ${overview.moves}`} testId="landing-your-move">
          {overview.yourMove.map(unit => (
            <UnitRow key={unit.name} unit={unit} {...unitProps} />
          ))}
          {/* a failed robot of no row: its actions are the move */}
          {overview.failedRobots.map(entry => (
            <RobotRow key={entry.robot.id} entry={entry} onReveal={reveal} />
          ))}
        </Section>
      )}
      <RobotList title={`Landing · ${overview.running.length}`} testId="landing-running" entries={overview.running} onReveal={reveal} />
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
      <RobotList title="Last 24 hours" testId="landing-finished" entries={overview.finished} onReveal={reveal} />
      {/* hidden when every part renders nothing (a local baseline, no GitHub) */}
      <div className="flex min-w-0 flex-col gap-2 border-t border-border pt-3 text-xs empty:hidden">{footer}</div>
    </>
  )
}

function Section({ title, testId, children }: { title: string; testId: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="flex flex-col gap-1" data-testid={testId}>
      <PopoverSectionTitle>{title}</PopoverSectionTitle>
      <ul className="flex flex-col gap-1">{children}</ul>
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
          <li key={fix.name} className="pl-6">
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
 * The node and the name (a click shows its row), the size, the first line of the node's hover (all of it on hover), the
 * pull request chip and the next step: a click closes the overview, shows the row and runs the step there, as the slot
 * of the row would
 */
function LandingRow({ name, node, size, step, stepOwner = name, nested = false, kind, pullRequest, onClose, onReveal }: LandingRowProps) {
  const reason = node.label.split("\n")[0] ?? ""
  const item = step?.item ?? null
  return (
    <li className={nested ? "flex items-center gap-2 pl-6" : "flex items-center gap-2"} data-testid="landing-row" data-branch-name={name} data-kind={node.kind}>
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
        <Tooltip>
          <TooltipTrigger render={<span className="min-w-0 truncate text-xs text-muted-foreground" data-testid="landing-row-reason" />}>{reason}</TooltipTrigger>
          <TooltipContent className="max-w-sm whitespace-pre-line">{node.label}</TooltipContent>
        </Tooltip>
      </span>
      <span className="flex shrink-0 items-center gap-1">{pullRequest}</span>
      {item !== null && (
        <span className="flex w-40 shrink-0 justify-end">
          <StepButton
            action={{
              ...item,
              onSelect: () => {
                onClose()
                // the step runs at the row: its popovers (Safe Push, Request Review…) open at its slot
                requestReveal({ branchName: stepOwner, commitIds: [] })
                item.onSelect()
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
