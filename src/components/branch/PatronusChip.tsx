import { useState } from "react"
import type { ReactNode } from "react"
import { CircleXIcon, ExternalLinkIcon, GitPullRequestIcon, RotateCcwIcon, ShieldOffIcon, UploadIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { ReviewCommitNotice } from "@/components/branch/ReviewCommitNotice"
import { RobotRing } from "@/components/branch/RobotProgress"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { StatusChip, StatusChipDot } from "@/components/branch/StatusChip"
import type { PatronusRobot } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import type { PatronusChipState } from "@/lib/patronus"
import { usePatronusRobotDetails } from "@/hooks/usePatronusRobotDetails"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import { formatCheckCounts } from "@/lib/check-counts"
import { pushBranch } from "@/lib/git/push"
import { canCancelRobot, formatPushMode, formatRobotCommitNotice, getPatronusChipState, getRobotRestartAction, openRobot } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { openPullRequestUrl } from "@/lib/pull-request"
import { formatCheckProgress, formatRobotTimingDetails } from "@/lib/robot-eta"
import { statusChipTextColors } from "@/lib/status-chip"
import { formatTimestamp } from "@/lib/time"
import { useBranch, useBranchRemoteStatus, useBranchTip } from "@/stores/branch-sync"
import { cancelPatronusRobot, restartPatronusRobot, useBranchRobot, usePatronusAvailable, useRobotNeedsReview } from "@/stores/patronus"

/**
 * The Patronus robot of the branch: the shield and the state ("Failed", "Merged", "Dry run passed"), in its hue, the
 * rotating ring while it runs; a `warning` dot when the branch changed since the robot took its commit (a branch only
 * rebuilt on a newer baseline did not). Hover shows the details, a click opens the robot in Patronus. Nothing without a
 * robot. `compact` (the outgoing overview, where the branch names need the room) shows the icon without the state text.
 * The branch row has the robot in its Safe Push slot instead (`SafePushSlot`); the preview, the overview and the Landed
 * island show this chip.
 */
export function PatronusChip({ branchName, compact = false }: { branchName: string, compact?: boolean }) {
  const robot = useBranchRobot(branchName)
  const localTip = useBranchTip(branchName)
  const remote = useBranchRemoteStatus(branchName)
  const needsReview = useRobotNeedsReview(robot?.id)
  if (robot === undefined) {
    return null
  }

  const chip = getPatronusChipState(robot, localTip, remote, needsReview)
  const Icon = patronusChipIcons[chip.kind]
  return (
    <RobotStatusChip branchName={branchName} robot={robot} chip={chip} label={`${formatPushMode(robot.pushMode)}: ${chip.label}`}>
      {chip.kind === "running" ? <RobotRing className="size-3.5 text-primary" /> : <Icon className="size-3.5" />}
      {!compact && chip.label}
      {chip.outdated && <StatusChipDot color="warning" testId="patronus-outdated-dot" />}
    </RobotStatusChip>
  )
}

export interface RobotStatusChipProps {
  branchName: string
  robot: PatronusRobot
  chip: PatronusChipState
  /** The accessible name: "Safe Push: Failed" */
  label: string
  /** The look of the Safe Push slot of the row (`StatusChip`) */
  className?: string
  /** The icon, the text and the dots */
  children: ReactNode
}

/**
 * The `StatusChip` of a robot: hover opens its details (Restart, Cancel, Open in Patronus), asked for while open; a
 * click opens the robot in Patronus. `patronus-chip` with its kind, colour and the outdated mark for the E2E tests.
 */
export function RobotStatusChip({ branchName, robot, chip, label, className, children }: RobotStatusChipProps) {
  const [open, setOpen] = useState(false)
  return (
    <StatusChip
      label={label}
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
      className={className}
    >
      {children}
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
  const Icon = patronusChipIcons[chip.kind]
  const details = usePatronusRobotDetails(robot.id, robot.status, loadDetails)
  const restart = getRobotRestartAction(robot, branchName, localTip, remote, needsReview)
  const notice = formatRobotCommitNotice(robot, chip, localTip, restart)
  const branch = useBranch(branchName)
  const patronusAvailable = usePatronusAvailable()
  const mode = formatPushMode(robot.pushMode)
  // the details name it; a poll or the start may have named it before
  const profileName = details?.profileName ?? robot.profile?.name ?? null
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
        {profileName !== null && (
          <>
            <dt className="text-muted-foreground">Profile</dt>
            <dd className="break-words" data-testid="patronus-profile-name">{profileName}</dd>
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
            <dd className="text-muted-foreground" data-testid="patronus-started">
              {formatTimestamp(robot.startTime)}
              {robot.status === "running" && <RobotTimingDetails robot={robot} remaining={details?.progress?.remainingSeconds == null} />}
            </dd>
          </>
        )}
        {robot.finishTime !== null && (
          <>
            <dt className="text-muted-foreground">Finished</dt>
            <dd className="text-muted-foreground">{formatTimestamp(robot.finishTime)}</dd>
          </>
        )}
        {robot.status === "running" && details?.progress != null && (
          <>
            <dt className="text-muted-foreground">Progress</dt>
            <dd className="break-words tabular-nums" data-testid="patronus-progress">
              {formatCheckProgress(details.progress)}
              {details.progress.probablyHanging && <span className="text-warning">{" · probably hanging"}</span>}
            </dd>
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
          {robot.pullRequest == null
            ? "The code owners did not approve the changes. Get the approvals in a merge request, then start a new Safe Push: a restart would check the same merge request."
            : `The code owners did not approve the changes in pull request #${robot.pullRequest.number}. Restart once they approve: the restart counts the approvals the pull request has by then.`}
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        {/* the form is below the row: an open branch preview closes first (`SafePushButton`) */}
        {needsReview && robot.pullRequest == null && patronusAvailable && branch !== undefined && <SafePushButton branch={branch} variant="outline" />}
        {/* named as in the branch menu: a bare "Cancel" reads like closing the popover */}
        {restart === "restart" && (
          <Button variant="outline" size="xs" onClick={() => void runRobotAction(restartPatronusRobot, robot, `${mode} restarted`, `Cannot restart the ${mode}`)}>
            <RotateCcwIcon data-icon="inline-start" />
            {`Restart ${mode}`}
          </Button>
        )}
        {restart === "pushAndRestart" && (
          // the restart takes the commit just pushed
          <Button variant="outline" size="xs" onClick={() => void pushBranch(branchName, { restartRobot: robot, announce: false })}>
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
        {robot.pullRequest != null && (
          <Button variant="outline" size="xs" onClick={() => openPullRequestUrl(robot.pullRequest!.url)} data-testid="patronus-open-pull-request">
            <GitPullRequestIcon data-icon="inline-start" />
            {`Open Pull Request #${robot.pullRequest.number}`}
          </Button>
        )}
      </div>
    </div>
  )
}

/** " · usually ~40 min · about 22 min left" after the start time of a running robot; it ticks, so it is its own component */
function RobotTimingDetails({ robot, remaining }: { robot: PatronusRobot, remaining: boolean }) {
  const timing = useRobotTiming(robot)
  return ` · ${formatRobotTimingDetails(robot, timing, { remaining })}`
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
