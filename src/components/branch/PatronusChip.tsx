import { useState } from "react"
import type { ComponentType } from "react"
import { CircleXIcon, ExternalLinkIcon, RotateCcwIcon, ShieldCheckIcon, ShieldEllipsisIcon, ShieldIcon, ShieldOffIcon, ShieldXIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { StatusChip, StatusChipDot } from "@/components/branch/StatusChip"
import type { PatronusRobot } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import type { PatronusChipKind } from "@/lib/patronus"
import { usePatronusRobotDetails } from "@/hooks/usePatronusRobotDetails"
import { formatCheckCounts } from "@/lib/check-counts"
import { canCancelRobot, canRestartRobot, formatPushMode, getPatronusChipState, openRobot } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { statusChipTextColors } from "@/lib/status-chip"
import { formatTimestamp } from "@/lib/time"
import { useBranchRemoteTip, useBranchTip } from "@/stores/branch-sync"
import { cancelPatronusRobot, restartPatronusRobot, useBranchRobot } from "@/stores/patronus"

const icons: Readonly<Record<PatronusChipKind, ComponentType<{ className?: string }>>> = {
  running: ShieldEllipsisIcon,
  failed: ShieldXIcon,
  merged: ShieldCheckIcon,
  passed: ShieldCheckIcon,
  canceled: ShieldOffIcon,
  unknown: ShieldIcon,
}

/**
 * The Patronus robot of the branch: the shield and the state ("Running", "Failed", "Merged", "Dry run passed"), in its
 * hue; a `warning` dot when it tests another commit than the local branch. Hover shows the details, a click opens the
 * robot in Patronus. Nothing without a robot. `compact` (the outgoing overview, where the branch names need the room)
 * shows the shield without the state text.
 */
export function PatronusChip({ branchName, compact = false }: { branchName: string, compact?: boolean }) {
  const robot = useBranchRobot(branchName)
  const localTip = useBranchTip(branchName)
  const [open, setOpen] = useState(false)
  if (robot === undefined) {
    return null
  }

  const chip = getPatronusChipState(robot, localTip)
  const Icon = icons[chip.kind]
  return (
    <StatusChip
      label={`${formatPushMode(robot.pushMode)}: ${chip.label}`}
      color={chip.color}
      data={{
        "data-testid": "patronus-chip",
        "data-kind": chip.kind,
        "data-color": chip.color,
        "data-outdated": chip.outdated || undefined,
      }}
      details={<PatronusRobotDetails branchName={branchName} robot={robot} localTip={localTip} loadDetails={open} />}
      detailsTestId="patronus-popover"
      onActivate={() => openRobot(robot)}
      onOpenChange={setOpen}
    >
      <Icon className={cn("size-3.5", chip.kind === "running" && "animate-pulse")} />
      {!compact && chip.label}
      {chip.outdated && <StatusChipDot color="warning" testId="patronus-outdated-dot" />}
    </StatusChip>
  )
}

export interface PatronusRobotDetailsProps {
  branchName: string
  robot: PatronusRobot
  localTip: string | undefined
  /** Asks Patronus for the profile, the checks and the problems (the popover is open) */
  loadDetails: boolean
}

/**
 * The mode and the state, the name, the target, the profile, the commit and the times, the checks and the problems, a
 * notice when the robot tests another commit than the local branch, then Restart, Cancel and "Open in Patronus". The
 * chip popover and the branch preview show it.
 */
export function PatronusRobotDetails({ branchName, robot, localTip, loadDetails }: PatronusRobotDetailsProps) {
  const chip = getPatronusChipState(robot, localTip)
  const Icon = icons[chip.kind]
  const details = usePatronusRobotDetails(robot.id, robot.status, loadDetails)
  const remoteTip = useBranchRemoteTip(branchName)
  const mode = formatPushMode(robot.pushMode)
  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="patronus-details">
      <div className="flex flex-col gap-1">
        <div className="flex items-center gap-2 text-xs">
          <span className={cn("flex items-center gap-1 font-medium", statusChipTextColors[chip.color])}>
            <Icon className="size-3.5" />
            {formatPushMode(robot.pushMode)}
          </span>
          <span className={cn("whitespace-nowrap", statusChipTextColors[chip.color])} data-testid="patronus-state">{chip.label}</span>
        </div>
        <p className="leading-snug font-medium break-words" data-testid="patronus-name">{robot.name}</p>
      </div>

      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
        <dt className="text-muted-foreground">Into</dt>
        <dd className="break-words" data-testid="patronus-target">{robot.targetBranch}</dd>
        {details?.profileName != null && (
          <>
            <dt className="text-muted-foreground">Profile</dt>
            <dd className="break-words" data-testid="patronus-profile-name">{details.profileName}</dd>
          </>
        )}
        {robot.sourceRevision !== null && (
          <>
            <dt className="text-muted-foreground">Commit</dt>
            <dd className="font-mono" data-testid="patronus-revision">{formatShortHash(robot.sourceRevision)}</dd>
          </>
        )}
        {robot.startTime > 0 && (
          <>
            <dt className="text-muted-foreground">Started</dt>
            <dd className="text-muted-foreground">{formatTimestamp(robot.startTime)}</dd>
          </>
        )}
        {robot.finishTime !== null && (
          <>
            <dt className="text-muted-foreground">Finished</dt>
            <dd className="text-muted-foreground">{formatTimestamp(robot.finishTime)}</dd>
          </>
        )}
        {details?.checks != null && (
          <>
            <dt className="text-muted-foreground">Checks</dt>
            <dd className={cn("break-words", details.checks.failed > 0 && "text-destructive")} data-testid="patronus-checks">{formatCheckCounts(details.checks)}</dd>
          </>
        )}
        {robot.cancellationReason !== null && (
          <>
            <dt className="text-muted-foreground">Reason</dt>
            <dd className="break-words" data-testid="patronus-cancellation-reason">{robot.cancellationReason}</dd>
          </>
        )}
      </dl>

      {chip.outdated && robot.sourceRevision !== null && localTip !== undefined && (
        <p className="flex items-start gap-1.5 text-xs text-warning" data-testid="patronus-outdated">
          <TriangleAlertIcon className="mt-px size-3.5 shrink-0" />
          {`Tests ${formatShortHash(robot.sourceRevision)}; the branch is at ${formatShortHash(localTip)} now.`}
        </p>
      )}

      {details !== null && details.problems.length > 0 && (
        <ul className="flex flex-col gap-1 text-xs text-destructive" data-testid="patronus-problems">
          {details.problems.map(problem => (
            <li key={problem.title} className="flex items-start gap-1.5">
              <CircleXIcon className="mt-px size-3.5 shrink-0" />
              {problem.title}
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-wrap gap-2">
        {canRestartRobot(robot, branchName, remoteTip) && (
          <Button variant="outline" size="xs" onClick={() => void runRobotAction(restartPatronusRobot, robot, `${mode} restarted`, `Cannot restart the ${mode}`)}>
            <RotateCcwIcon data-icon="inline-start" />
            Restart
          </Button>
        )}
        {canCancelRobot(robot) && (
          <Button variant="outline" size="xs" onClick={() => void runRobotAction(cancelPatronusRobot, robot, `${mode} is canceling`, `Cannot cancel the ${mode}`)}>
            <ShieldOffIcon data-icon="inline-start" />
            Cancel
          </Button>
        )}
        <Button variant="outline" size="xs" onClick={() => openRobot(robot)}>
          <ExternalLinkIcon data-icon="inline-start" />
          Open in Patronus
        </Button>
      </div>
    </div>
  )
}

/** The robot of the branch in the branch preview, above the commits; nothing without one */
export function BranchPatronusSection({ branchName }: { branchName: string }) {
  const robot = useBranchRobot(branchName)
  const localTip = useBranchTip(branchName)
  if (robot === undefined) {
    return null
  }
  return (
    <section aria-label="Patronus" className="rounded-lg border border-border p-3" data-testid="branch-preview-patronus">
      <PatronusRobotDetails branchName={branchName} robot={robot} localTip={localTip} loadDetails />
    </section>
  )
}
