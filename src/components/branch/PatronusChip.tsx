import { useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { CircleXIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ReviewCommitNotice } from "@/components/branch/ReviewCommitNotice"
import { RobotRing } from "@/components/branch/RobotProgress"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { StatusChip, StatusChipDot } from "@/components/branch/StatusChip"
import { getGatedRobotActions } from "@/hooks/useBranchNextStep"
import { usePatronusRobotDetails } from "@/hooks/usePatronusRobotDetails"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import type { PatronusRobot } from "@/lib/bindings"
import { formatCheckCounts } from "@/lib/check-counts"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import type { PatronusChipState } from "@/lib/patronus"
import { formatPushMode, formatRobotCommitNotice, getPatronusChipState, getRobotRestartAction, isFailingChip, openRobot, robotRingColor } from "@/lib/patronus"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { openPullRequestUrl } from "@/lib/pull-request"
import { formatCheckProgress, formatRobotTimingDetails } from "@/lib/robot-eta"
import { statusChipTextColors } from "@/lib/status-chip"
import { formatTimestamp } from "@/lib/time"
import { useBranch, useBranchRemoteStatus, useBranchTip } from "@/stores/branch-sync"
import { useOperations } from "@/stores/operations"
import { useBranchRobot, usePatronusAvailable, useRobotFailing, useRobotNeedsReview } from "@/stores/patronus"

/**
 * The Patronus robot of the branch: the shield and the state ("Failed", "Merged", "Dry run passed"), in its hue, the
 * rotating ring while it runs; a `warning` dot when the branch changed since the robot took its commit (a branch only
 * rebuilt on a newer baseline did not). Hover shows the details, a click opens the robot in Patronus. Nothing without a
 * robot. `compact` (the landing overview, where the branch names need the room) shows the icon without the state text.
 * The branch row has the robot in its Status cell and its next step in the slot instead (`NextStepSlot`); the preview, the overview and the Landed
 * island show this chip.
 */
export function PatronusChip({ branchName, compact = false }: { branchName: string; compact?: boolean }) {
  const robot = useBranchRobot(branchName)
  const localTip = useBranchTip(branchName)
  const remote = useBranchRemoteStatus(branchName)
  const needsReview = useRobotNeedsReview(robot?.id)
  const failing = useRobotFailing(robot?.id)
  if (robot === undefined) {
    return null
  }

  const chip = getPatronusChipState(robot, localTip, remote, { needsReview, failing })
  const Icon = patronusChipIcons[chip.kind]
  return (
    <RobotStatusChip branchName={branchName} robot={robot} chip={chip} label={`${formatPushMode(robot.pushMode)}: ${chip.label}`}>
      {chip.kind === "running" ? <RobotRing className={cn("size-3.5", robotRingColor(isFailingChip(chip)))} /> : <Icon className="size-3.5" />}
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
  /** The look of the chip in the Status cell of the row (`StatusChip`) */
  className?: string
  /**
   * The robot of a branch row: hovered or focused, the stack graph draws the path to the root and the landing span
   * (`data-stack-path`, `stack-view.ts`)
   */
  stackPath?: boolean
  /** The icon, the text and the dots */
  children: ReactNode
}

/**
 * The `StatusChip` of a robot: hover opens its details (its actions, `getGatedRobotActions`), asked for while open; a
 * click opens the robot in Patronus. `patronus-chip` with its kind, colour and the outdated mark for the E2E tests.
 */
export function RobotStatusChip({ branchName, robot, chip, label, className, stackPath = false, children }: RobotStatusChipProps) {
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
        "data-stack-path": stackPath ? "landing" : undefined,
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
 * baseline), then the actions of the robot as the branch menu names them (`getGatedRobotActions`: "Restart Safe Push",
 * which pushes a change not pushed yet first, "Cancel Safe Push", "Open in Patronus", "Open Pull Request #12").
 * The chip popover and the branch preview show it.
 */
export function PatronusRobotDetails({ branchName, robot, loadDetails }: PatronusRobotDetailsProps) {
  const needsReview = useRobotNeedsReview(robot.id)
  const failing = useRobotFailing(robot.id)
  // a restart or a cancel that runs shows in its button
  const operations = useOperations()
  const localTip = useBranchTip(branchName)
  const remote = useBranchRemoteStatus(branchName)
  const chip = getPatronusChipState(robot, localTip, remote, { needsReview, failing })
  const Icon = patronusChipIcons[chip.kind]
  const details = usePatronusRobotDetails(robot.id, robot.status, loadDetails)
  const restart = getRobotRestartAction(robot, branchName, localTip, remote, needsReview)
  const notice = formatRobotCommitNotice(robot, chip, localTip, restart)
  const branch = useBranch(branchName)
  const patronusAvailable = usePatronusAvailable()
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
          <span className={cn("whitespace-nowrap", statusChipTextColors[chip.color])} data-testid="patronus-state">
            {chip.label}
          </span>
        </div>
        <p className="leading-snug font-medium break-words" data-testid="patronus-name">
          {robot.name}
        </p>
      </div>

      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
        <dt className="text-muted-foreground">Into</dt>
        <dd className="break-words" data-testid="patronus-target">
          {robot.targetBranch}
        </dd>
        {profileName !== null && (
          <>
            <dt className="text-muted-foreground">Profile</dt>
            <dd className="break-words" data-testid="patronus-profile-name">
              {profileName}
            </dd>
          </>
        )}
        {robot.sourceRevision !== null && (
          <>
            <dt className="text-muted-foreground">Commit</dt>
            <dd className="font-mono" data-testid="patronus-revision">
              {formatShortHash(robot.sourceRevision)}
            </dd>
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
            <dd className={cn("break-words", details.checks.failed > 0 && "text-destructive")} data-testid="patronus-checks">
              {formatCheckCounts(details.checks)}
            </dd>
          </>
        )}
        {robot.cancellationReason !== null && (
          <>
            <dt className="text-muted-foreground">Reason</dt>
            <dd className="break-words" data-testid="patronus-cancellation-reason">
              {robot.cancellationReason}
            </dd>
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
          {robot.pullRequest == null ? (
            "The code owners did not approve the changes. Get their approvals in the pull request of the branch, then start a new Safe Push from it: a restart has no pull request to count them in."
          ) : (
            <>
              {"The code owners did not approve the changes in pull request "}
              <PullRequestLink pullRequest={robot.pullRequest} />
              {". Restart once they approve: the restart counts the approvals the pull request has by then."}
            </>
          )}
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        {/* the form is below the row: an open branch preview closes first (`SafePushButton`) */}
        {needsReview && robot.pullRequest == null && patronusAvailable && branch !== undefined && <SafePushButton branch={branch} variant="outline" />}
        {/* named as in the branch menu: a bare "Cancel" reads like closing the popover */}
        {getGatedRobotActions(robot, branch ?? null, needsReview, failing, operations).map(action => (
          <RobotActionButton key={action.id} action={action} />
        ))}
      </div>
    </div>
  )
}

/** "#1078" in a sentence, a link that opens the pull request the robot was started from */
function PullRequestLink({ pullRequest }: { pullRequest: { readonly number: number; readonly url: string } }) {
  return (
    <button
      type="button"
      className="rounded-sm underline-offset-2 outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
      onClick={() => openPullRequestUrl(pullRequest.url)}
    >
      {`#${pullRequest.number}`}
    </button>
  )
}

/**
 * An action of the robot as an `outline` button; a disabled one says why on hover. `data-action` names it: a restart
 * that pushes first has the name of a restart
 */
export function RobotActionButton({ action }: { action: MenuItem }) {
  // a restart or a cancel that runs: the spinner in place of its icon (`operationItem`)
  const running = action.running === true
  const content = (
    <>
      {running ? <Spinner data-icon="inline-start" /> : action.icon && <action.icon data-icon="inline-start" />}
      {action.label}
    </>
  )
  if (action.description === undefined) {
    return (
      <Button variant="outline" size="xs" data-action={action.id} disabled={action.disabled} aria-busy={running || undefined} onClick={() => action.onSelect(FROM_MENU)}>
        {content}
      </Button>
    )
  }
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          // the hover says why; Base UI keeps the button focusable, so the dimming is ours
          <Button
            variant="outline"
            size="xs"
            data-action={action.id}
            disabled={action.disabled}
            aria-busy={running || undefined}
            focusableWhenDisabled
            className={cn(action.disabled && "opacity-50")}
            onClick={() => action.onSelect(FROM_MENU)}
          />
        }
      >
        {content}
      </TooltipTrigger>
      <TooltipContent>{action.description}</TooltipContent>
    </Tooltip>
  )
}

/** " · usually ~40 min · about 22 min left" after the start time of a running robot; it ticks, so it is its own component */
function RobotTimingDetails({ robot, remaining }: { robot: PatronusRobot; remaining: boolean }) {
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
