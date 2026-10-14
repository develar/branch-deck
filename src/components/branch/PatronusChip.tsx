import { useState } from "react"
import type { ComponentType } from "react"
import { CircleXIcon, ExternalLinkIcon, RotateCcwIcon, ShieldAlertIcon, ShieldCheckIcon, ShieldEllipsisIcon, ShieldIcon, ShieldOffIcon, ShieldXIcon, UploadIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { ReviewCommitNotice } from "@/components/branch/ReviewCommitNotice"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { StatusChip, StatusChipDot } from "@/components/branch/StatusChip"
import type { PatronusRobot } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import type { PatronusChipKind } from "@/lib/patronus"
import { usePatronusRobotDetails } from "@/hooks/usePatronusRobotDetails"
import { formatCheckCounts } from "@/lib/check-counts"
import { pushBranch } from "@/lib/git/push"
import { canCancelRobot, formatPushMode, formatRobotCommitNotice, getPatronusChipState, getRobotRestartAction, openRobot } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { statusChipTextColors } from "@/lib/status-chip"
import { formatTimestamp } from "@/lib/time"
import { useBranch, useBranchRemoteStatus, useBranchTip } from "@/stores/branch-sync"
import { cancelPatronusRobot, restartPatronusRobot, useBranchRobot, usePatronusAvailable, useRobotNeedsReview } from "@/stores/patronus"

const icons: Readonly<Record<PatronusChipKind, ComponentType<{ className?: string }>>> = {
  running: ShieldEllipsisIcon,
  failed: ShieldXIcon,
  needsReview: ShieldAlertIcon,
  merged: ShieldCheckIcon,
  passed: ShieldCheckIcon,
  canceled: ShieldOffIcon,
  unknown: ShieldIcon,
}

/**
 * The Patronus robot of the branch: the shield and the state ("Running", "Failed", "Merged", "Dry run passed"), in its
 * hue; a `warning` dot when the branch changed since the robot took its commit (a branch only rebuilt on a newer
 * baseline did not). Hover shows the details, a click opens the robot in Patronus. Nothing without a robot. `compact`
 * (the outgoing overview, where the branch names need the room) shows the shield without the state text.
 */
export function PatronusChip({ branchName, compact = false }: { branchName: string, compact?: boolean }) {
  const robot = useBranchRobot(branchName)
  const localTip = useBranchTip(branchName)
  const remote = useBranchRemoteStatus(branchName)
  const [open, setOpen] = useState(false)
  const needsReview = useRobotNeedsReview(robot?.id)
  if (robot === undefined) {
    return null
  }

  const chip = getPatronusChipState(robot, localTip, remote, needsReview)
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
      details={<PatronusRobotDetails branchName={branchName} robot={robot} loadDetails={open} />}
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
  /** Asks Patronus for the profile, the checks and the problems (the popover is open) */
  loadDetails: boolean
}

/**
 * The mode and the state, the name, the target, the profile, the commit and the times, the checks and the problems, a
 * notice when the branch changed since the robot took its commit (a quiet line when it was only rebuilt on a newer
 * baseline), then Restart (Push and Restart for a change not pushed yet), Cancel and "Open in Patronus". The chip
 * popover and the branch preview show it.
 */
export function PatronusRobotDetails({ branchName, robot, loadDetails }: PatronusRobotDetailsProps) {
  const needsReview = useRobotNeedsReview(robot.id)
  const localTip = useBranchTip(branchName)
  const remote = useBranchRemoteStatus(branchName)
  const chip = getPatronusChipState(robot, localTip, remote, needsReview)
  const Icon = icons[chip.kind]
  const details = usePatronusRobotDetails(robot.id, robot.status, loadDetails)
  const restart = getRobotRestartAction(robot, branchName, localTip, remote, needsReview)
  const notice = formatRobotCommitNotice(robot, chip, localTip, restart)
  const branch = useBranch(branchName)
  const patronusAvailable = usePatronusAvailable()
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

      <ReviewCommitNotice text={notice} outdated={chip.outdated} testId="patronus" />

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

      {needsReview && (
        <p className="text-xs text-warning" data-testid="patronus-needs-review">
          The code owners did not approve the changes. Get the approvals in a merge request, then start a new Safe Push: a
          restart would check the same merge request.
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        {/* the form is below the row: an open branch preview closes first (`SafePushButton`) */}
        {needsReview && patronusAvailable && branch !== undefined && <SafePushButton branch={branch} variant="outline" />}
        {/* named as in the branch menu: a bare "Cancel" reads like closing the popover */}
        {restart === "restart" && (
          <Button variant="outline" size="xs" onClick={() => void runRobotAction(restartPatronusRobot, robot, `${mode} restarted`, `Cannot restart the ${mode}`)}>
            <RotateCcwIcon data-icon="inline-start" />
            {`Restart ${mode}`}
          </Button>
        )}
        {restart === "pushAndRestart" && (
          // the restart takes the commit just pushed
          <Button variant="outline" size="xs" onClick={() => void pushBranch(branchName, { restartRobot: robot })}>
            <UploadIcon data-icon="inline-start" />
            Push and Restart
          </Button>
        )}
        {canCancelRobot(robot) && (
          <Button variant="outline" size="xs" onClick={() => void runRobotAction(cancelPatronusRobot, robot, `${mode} is canceling`, `Cannot cancel the ${mode}`)}>
            <ShieldOffIcon data-icon="inline-start" />
            {`Cancel ${mode}`}
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
  if (robot === undefined) {
    return null
  }
  return (
    <section aria-label="Patronus" className="rounded-lg border border-border p-3" data-testid="branch-preview-patronus">
      <PatronusRobotDetails branchName={branchName} robot={robot} loadDetails />
    </section>
  )
}
