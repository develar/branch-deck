import { cn } from "cn"
import { CirclePauseIcon } from "lucide-react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchStatusBadge } from "@/components/branch/BranchStatusBadge"
import { RobotStatusChip } from "@/components/branch/PatronusChip"
import { RobotProgressBar, RobotRing } from "@/components/branch/RobotProgress"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { useLandedIn } from "@/hooks/useLandedIn"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import { useRowRobotChip } from "@/hooks/useRowRobot"
import type { PatronusRobot } from "@/lib/bindings"
import type { ParkedState } from "@/lib/parked"
import { formatParkedLabel } from "@/lib/parked"
import type { PatronusChipState } from "@/lib/patronus"
import { formatPushMode, isFailingChip, robotRingColor } from "@/lib/patronus"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { countRender } from "@/lib/render-counts"
import { formatRobotProgress, formatSpokenRobotProgress } from "@/lib/robot-eta"
import { useRowPart } from "@/model/store"
import type { RowMark } from "@/presenters/row-status"
import { presentRowCell, presentRowMark } from "@/presenters/row-status"
import type { Branch } from "@/stores/branch-sync"
import { useBranch, useParkedState } from "@/stores/branch-sync"
import { useBranchRobot } from "@/stores/patronus"

/**
 * The robot reads as the status text it replaces: `text-sm`, its icon where the status dot is; the hover underline and
 * the details of a chip stay
 */
const ROW_ROBOT = "h-6 gap-2 px-0 text-sm"

/**
 * The first column of the Status cell of a branch row, what happens to the branch, as Rust decides it (`RowStatus`,
 * `presentRowCell`): its robot while one runs (its ring, its time, the progress line under it; "Failing" in
 * `destructive` for one that will fail) or finished on the branch as it is ("Failed", "Needs review", "Merged", "Dry run
 * passed", "Canceled"), what landed it, a failed sync, parked, a block by its parent, else the push state. The facts the
 * first covers stay beside it as a mark whose hover names them: a running robot never hides a block or a conflict. A
 * sync running lays over the cell.
 *
 * A leaf of its row: it reads the branch by its name, the status of its sync included, which the row leaves out
 * (`Branch.row`), so a sync that changes only the status of a branch renders this alone.
 */
export function BranchRowStatus({ name }: { name: string }) {
  countRender(`status:${name}`)
  const branch = useBranch(name)
  return branch === undefined ? null : <RowStatusCell branch={branch} />
}

/** The first column and the mark beside it; the robot's hooks only for a row that shows its robot, a few of the rows */
function RowStatusCell({ branch }: { branch: Branch }) {
  const landedIn = useLandedIn(branch.name)
  const robot = useBranchRobot(branch.name)
  const parked = useParkedState(branch.name)
  const status = useRowPart(branch.name, "landing")?.status
  const context = { branch, landedIn, robot, parked }
  const cell = presentRowCell(status, context)
  const mark = presentRowMark(status, context)
  const first =
    cell.kind === "robot" && robot !== undefined ? (
      <RobotRowStatus branch={branch} robot={robot} running={cell.running} />
    ) : cell.kind === "parked" && parked !== null ? (
      <ParkedStatus state={parked} />
    ) : cell.kind === "text" ? (
      <BranchStatusBadge status={cell.texts} />
    ) : null
  if (mark === null) {
    return first
  }
  return (
    <div className="flex min-w-0 items-center gap-2">
      {first}
      <RowStatusMark mark={mark} />
    </div>
  )
}

/** The robot of the branch, running or finished on the branch as it is */
function RobotRowStatus({ branch, robot, running }: { branch: Branch; robot: PatronusRobot; running: boolean }) {
  const chip = useRowRobotChip(branch, robot)
  return running ? (
    <RunningRowRobot branchName={branch.name} robot={robot} chip={chip} />
  ) : (
    <RobotStatusChip branchName={branch.name} robot={robot} chip={chip} label={formatRobotLabel(robot, chip)} className={ROW_ROBOT} stackPath>
      <FinishedRobotIcon chip={chip} />
      <span className="truncate">{chip.label}</span>
      {chip.outdated && <StatusChipDot color="warning" testId="patronus-outdated-dot" />}
    </RobotStatusChip>
  )
}

/** The facts the first column covers ("blocked by auth" beside a running Safe Push): a dot, their lines on hover */
function RowStatusMark({ mark }: { mark: RowMark }) {
  const label = mark.lines.join("\n")
  return (
    <Tooltip lazy>
      <TooltipTrigger delay={300} render={<span role="img" aria-label={label} className="flex size-4 shrink-0 items-center justify-center" data-testid="row-status-mark" />}>
        <StatusChipDot color={mark.color} testId="row-status-mark-dot" />
      </TooltipTrigger>
      <TooltipContent className="max-w-sm break-words whitespace-pre-line">{label}</TooltipContent>
    </Tooltip>
  )
}

/** "Parked", "Parked with auth": muted, the note in the hover (the name cell of the parked branch shows it too) */
function ParkedStatus({ state }: { state: ParkedState }) {
  const what = state.with === null ? "Parked" : `Parked with ${state.with}`
  return (
    <div className="min-w-0">
      <Tooltip lazy>
        <TooltipTrigger delay={300} render={<span className="flex h-6 max-w-full items-center gap-2 text-sm text-muted-foreground" data-testid="parked-status" />}>
          <CirclePauseIcon aria-hidden className="size-3.5 shrink-0" />
          <span className="truncate">{what}</span>
        </TooltipTrigger>
        <TooltipContent className="max-w-sm whitespace-pre-line">{formatParkedLabel(state)}</TooltipContent>
      </Tooltip>
    </div>
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

export interface RunningRowRobotProps {
  /** The branch the robot ran on; the root for the robot of the whole stack (`stackRoot`) */
  branchName: string
  robot: PatronusRobot
  chip: PatronusChipState
  /** The robot of a branch row, whose hover draws its path in the stack graph (`RobotStatusChipProps.stackPath`) */
  stackPath?: boolean
  /** The robot of the whole stack of this root (`RobotStatusChipProps.stackRoot`) */
  stackRoot?: string
  /**
   * In the title bar of a window, inside a `@container` of the room the stack bar leaves: the time while it fits, how
   * long it runs from 5rem, how long it still takes from 10rem, so a narrow window keeps the ring and the line, its
   * accessible name the time; `shared` with words after it in the same room ("lands with oauth"), which show first: the
   * time left from 20rem
   */
  titleBar?: "alone" | "shared"
}

/**
 * A running robot: the ring, how long it runs and how long it still takes ("18m · ~22m", past its usual time
 * "55m · usually ~40m"), and under it the line of the share of its usual time, indeterminate without history, past it
 * and while it cancels; one that will fail says so instead of the time left ("18m · Failing"), no use waiting for it.
 * Its own component, as its time ticks: the Status cell of a branch row and the title bar of the review window show it.
 */
export function RunningRowRobot({ branchName, robot, chip, stackPath = true, stackRoot, titleBar }: RunningRowRobotProps) {
  const timing = useRobotTiming(robot, true)
  const canceling = robot.status === "canceling"
  const failing = isFailingChip(chip)
  const progress = formatRobotProgress(timing)
  // "Safe Push: Failing": the time left is no use for a robot that will fail
  const label = canceling || failing ? formatRobotLabel(robot, chip) : `${formatRobotLabel(robot, chip)}, ${formatSpokenRobotProgress(timing)}`
  const rest = failing ? ` · ${chip.label}` : progress.eta === null ? null : ` · ${progress.eta}`
  return (
    // as wide as "59m · ~1h 59m" while it runs, so its time ticks without moving the columns; a robot past its usual
    // time ("55m · usually ~40m") widens its column once
    <div className={cn("relative flex", titleBar === undefined ? "min-w-[13ch]" : "min-w-0")}>
      <RobotStatusChip
        branchName={branchName}
        robot={robot}
        chip={chip}
        label={label}
        className={cn(ROW_ROBOT, titleBar !== undefined && "min-w-0 shrink")}
        stackPath={stackPath}
        {...(stackRoot !== undefined && { stackRoot })}
      >
        <RobotRing className={cn("size-3.5 shrink-0", robotRingColor(failing))} />
        <span className={cn("truncate", titleBar !== undefined && "hidden @min-[5rem]:inline")} data-testid="robot-time">
          {canceling ? (
            chip.label
          ) : (
            <>
              <span className={failing ? undefined : "text-foreground"}>{progress.elapsed}</span>
              {titleBar === undefined || rest === null ? rest : <span className={titleBar === "alone" ? "hidden @min-[10rem]:inline" : "hidden @min-[20rem]:inline"}>{rest}</span>}
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
