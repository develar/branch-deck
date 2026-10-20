import { cn } from "cn"
import { BranchStatusBadge } from "@/components/branch/BranchStatusBadge"
import { RobotStatusChip } from "@/components/branch/PatronusChip"
import { RobotProgressBar, RobotRing } from "@/components/branch/RobotProgress"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { useLandedIn } from "@/hooks/useLandedIn"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import { useRowRobot } from "@/hooks/useRowRobot"
import { statusOutranksRobot } from "@/hooks/useStatusBadge"
import type { PatronusRobot } from "@/lib/bindings"
import type { PatronusChipState } from "@/lib/patronus"
import { formatPushMode, getRowRobotChip, isFailingChip, robotRingColor } from "@/lib/patronus"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { formatRobotProgress, formatSpokenRobotProgress } from "@/lib/robot-eta"
import type { Branch } from "@/stores/branch-sync"

/**
 * The robot reads as the status text it replaces: `text-sm`, its icon where the status dot is; the hover underline and
 * the details of a chip stay
 */
const ROW_ROBOT = "h-6 gap-2 px-0 text-sm"

/**
 * The first column of the Status cell of a branch row, its state in the life of the branch: the robot of the branch
 * while one runs (its ring, its time, the progress line under it; "Failing" in `destructive` for one that will fail) or
 * finished on the branch as it is ("Failed", "Needs review", "Merged", "Dry run passed", "Canceled"), else the status
 * text (`BranchStatusBadge`). A sync running, an error, a conflict, a block and a landed branch say more than the robot.
 */
export function BranchRowStatus({ branch }: { branch: Branch }) {
  const landedIn = useLandedIn(branch.name)
  const { robot, slot } = useRowRobot(branch)
  const chip = getRowRobotChip(slot)
  if (robot === undefined || chip === null || statusOutranksRobot(branch, landedIn)) {
    return <BranchStatusBadge branch={branch} />
  }
  return slot.kind === "running" ? (
    <RunningRowRobot branchName={branch.name} robot={robot} chip={chip} />
  ) : (
    <RobotStatusChip branchName={branch.name} robot={robot} chip={chip} label={formatRobotLabel(robot, chip)} className={ROW_ROBOT} stackPath>
      <FinishedRobotIcon chip={chip} />
      <span className="truncate">{chip.label}</span>
      {chip.outdated && <StatusChipDot color="warning" testId="patronus-outdated-dot" />}
    </RobotStatusChip>
  )
}

function FinishedRobotIcon({ chip }: { chip: PatronusChipState }) {
  const Icon = patronusChipIcons[chip.kind]
  return <Icon className="size-3.5 shrink-0" />
}

/** "Safe Push: Failed", the mode, then the state */
function formatRobotLabel(robot: PatronusRobot, chip: PatronusChipState): string {
  return `${formatPushMode(robot.pushMode)}: ${chip.label}`
}

/**
 * A running robot: the ring, how long it runs and how long it usually takes ("18m · ~22m left", past its usual time
 * "55m · usually ~40m"), and under it the line of the share of its usual time, indeterminate without history, past it
 * and while it cancels; one that will fail says so instead of the time left ("18m · Failing"), no use waiting for it.
 * Its own component, as its time ticks.
 */
function RunningRowRobot({ branchName, robot, chip }: { branchName: string; robot: PatronusRobot; chip: PatronusChipState }) {
  const timing = useRobotTiming(robot, true)
  const canceling = robot.status === "canceling"
  const failing = isFailingChip(chip)
  const progress = formatRobotProgress(timing)
  // "Safe Push: Failing": the time left is no use for a robot that will fail
  const label = canceling || failing ? formatRobotLabel(robot, chip) : `${formatRobotLabel(robot, chip)}, ${formatSpokenRobotProgress(timing)}`
  return (
    // as wide as "55m · usually ~40m" while it runs, so its time ticks without moving the columns
    <div className="relative flex min-w-[18ch]">
      <RobotStatusChip branchName={branchName} robot={robot} chip={chip} label={label} className={ROW_ROBOT} stackPath>
        <RobotRing className={cn("size-3.5 shrink-0", robotRingColor(failing))} />
        <span className="truncate" data-testid="robot-time">
          {canceling ? (
            chip.label
          ) : (
            <>
              <span className={failing ? undefined : "text-foreground"}>{progress.elapsed}</span>
              {failing ? ` · ${chip.label}` : progress.eta !== null && ` · ${progress.eta}`}
            </>
          )}
        </span>
        {chip.outdated && <StatusChipDot color="warning" testId="patronus-outdated-dot" />}
      </RobotStatusChip>
      {/* in the padding of the cell, under the text: the row keeps its height */}
      <RobotProgressBar progress={canceling ? null : timing.progress} failing={failing && !canceling} className="absolute inset-x-0 -bottom-1 h-0.5" />
    </div>
  )
}
