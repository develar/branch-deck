import { cn } from "cn"
import { RobotStatusChip } from "@/components/branch/PatronusChip"
import { RobotProgressBar, RobotRing } from "@/components/branch/RobotProgress"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import type { PatronusRobot } from "@/lib/bindings"
import type { PatronusChipState } from "@/lib/patronus"
import { canStartRobot, formatPushMode, getSafePushSlot, isRobotActive } from "@/lib/patronus"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { formatRobotProgress, formatSpokenRobotProgress } from "@/lib/robot-eta"
import type { Branch } from "@/stores/branch-sync"
import { useBranchRobot, useLandingAncestor, useLandingSpan, usePatronusAvailable, useRobotNeedsReview } from "@/stores/patronus"

/**
 * The robot in the slot looks like the buttons of the Actions cell (an `xs` ghost button with a `size-3` icon), so the
 * icon of every state sits in one column down the table; it shrinks, its text truncates, the slot keeps its width
 */
const SLOT_CHIP = "h-6 min-w-0 shrink pr-2.5 pl-2 hover:bg-muted hover:no-underline"

/**
 * Hidden while the row is hovered or has the keyboard focus: a finished robot's text gives way to "Safe Push…". So also
 * while the robot's details are open: the pointer on its way into them leaves the row, and the slot must not change
 * under the popover.
 */
const HIDDEN_ON_ROW_HOVER = "group-hover:hidden group-focus-visible:hidden group-has-focus-visible:hidden in-data-popup-open:hidden"
const SHOWN_ON_ROW_HOVER = "hidden group-hover:inline-flex group-focus-visible:inline-flex group-has-focus-visible:inline-flex group-has-data-popup-open/slot:inline-flex"

/**
 * The Safe Push slot of a branch row, after the push button, one fixed width in every row, so the push buttons and "⋯"
 * line up down the table (`getSafePushSlot`):
 * - "Safe Push…" without a robot, and for a robot of an earlier state of the branch;
 * - while a robot runs, the robot: the rotating ring, its time and how long it has left ("18m · ~22m left"), past its
 *   usual time that time ("55m · usually ~40m");
 * - a finished robot of the branch as it is: its state in its hue ("Failed"); the row hover swaps the text for "Safe
 *   Push…" when a start is possible, the shield stays and opens the details.
 * Hovering the robot opens its details, a click opens it in Patronus. Nothing in a repository that is not on Space.
 */
export function SafePushSlot({ branch }: { branch: Branch }) {
  const patronusAvailable = usePatronusAvailable()
  const robot = useBranchRobot(branch.name)
  const needsReview = useRobotNeedsReview(robot?.id)
  const landingAncestor = useLandingAncestor(branch.name)
  const landingSpan = useLandingSpan(branch.name)
  const slot = getSafePushSlot({
    robot,
    localTip: branch.commits[0]?.hash,
    remote: branch.remoteStatus,
    needsReview,
    canOffer: patronusAvailable && !branch.hasError && branch.commitCount > 0,
    canStart: canStartRobot(branch, robot, landingAncestor, landingSpan).ok,
  })
  if (!patronusAvailable && slot.kind === "none") {
    return null
  }
  return (
    <div className="group/slot flex w-40 items-center" data-testid="safe-push-slot" data-slot-kind={slot.kind} data-stack-path="landing">
      {slot.kind === "start" && <SafePushButton branch={branch} shortcut="S" />}
      {slot.kind === "running" && robot !== undefined && <RunningRobotChip branchName={branch.name} robot={robot} chip={slot.chip} />}
      {slot.kind === "finished" && robot !== undefined && (
        <>
          <RobotStatusChip branchName={branch.name} robot={robot} chip={slot.chip} label={formatRobotLabel(robot, slot.chip)} className={cn(SLOT_CHIP, slot.restart && "group-hover:pr-1 group-focus-visible:pr-1 group-has-focus-visible:pr-1 data-popup-open:pr-1")}>
            <FinishedRobotIcon chip={slot.chip} />
            <span className={cn("truncate", slot.restart && HIDDEN_ON_ROW_HOVER)}>{slot.chip.label}</span>
            {slot.chip.outdated && <StatusChipDot color="warning" testId="patronus-outdated-dot" />}
          </RobotStatusChip>
          {slot.restart && <SafePushButton branch={branch} icon={false} shortcut="S" className={cn(SHOWN_ON_ROW_HOVER, "pl-0")} />}
        </>
      )}
    </div>
  )
}

function FinishedRobotIcon({ chip }: { chip: PatronusChipState }) {
  const Icon = patronusChipIcons[chip.kind]
  return <Icon className="size-3 shrink-0" />
}

/** "Safe Push: Failed", the mode, then the state */
function formatRobotLabel(robot: PatronusRobot, chip: PatronusChipState): string {
  return `${formatPushMode(robot.pushMode)}: ${chip.label}`
}

/** A running robot: the ring, how long it runs and how long it usually takes; its own component, as its time ticks */
function RunningRobotChip({ branchName, robot, chip }: { branchName: string, robot: PatronusRobot, chip: PatronusChipState }) {
  const timing = useRobotTiming(robot, true)
  const canceling = robot.status === "canceling"
  const progress = formatRobotProgress(timing)
  const label = canceling ? formatRobotLabel(robot, chip) : `${formatRobotLabel(robot, chip)}, ${formatSpokenRobotProgress(timing)}`
  return (
    <RobotStatusChip branchName={branchName} robot={robot} chip={chip} label={label} className={SLOT_CHIP}>
      <RobotRing className="size-3 shrink-0 text-primary" />
      <span className="truncate" data-testid="robot-time">
        {canceling
          ? chip.label
          : (
              <>
                <span className="text-foreground">{progress.elapsed}</span>
                {progress.eta !== null && ` · ${progress.eta}`}
              </>
            )}
      </span>
      {chip.outdated && <StatusChipDot color="warning" testId="patronus-outdated-dot" />}
    </RobotStatusChip>
  )
}

export interface RobotProgressRowProps {
  name: string
  columnCount: number
  /** In a table with stacks, the inset of the rows below a branch row (`laneContentInsetClass`): the bar starts right of the lanes */
  insetClass?: string
}

/**
 * The thin progress line under the row of a branch whose robot runs: the share of its usual duration, indeterminate
 * without history and past it. A row of no height after the branch row, its bar over the border between the rows (a
 * `tr` can't position its own bar in WebKit).
 */
export function RobotProgressRow({ name, columnCount, insetClass }: RobotProgressRowProps) {
  const robot = useBranchRobot(name)
  if (robot === undefined || !isRobotActive(robot)) {
    return null
  }
  return (
    <tr aria-hidden data-testid="robot-progress-row" className="border-0">
      <td colSpan={columnCount} className={cn("relative h-0 p-0", insetClass)}>
        <RobotRowProgressBar robot={robot} className={insetClass === undefined ? undefined : "left-(--bd-row-inset)"} />
      </td>
    </tr>
  )
}

function RobotRowProgressBar({ robot, className }: { robot: PatronusRobot, className?: string }) {
  const timing = useRobotTiming(robot, true)
  return <RobotProgressBar progress={robot.status === "canceling" ? null : timing.progress} className={cn("absolute inset-x-0 -top-px h-0.5", className)} />
}
