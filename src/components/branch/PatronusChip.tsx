import type { ComponentType } from "react"
import { ExternalLinkIcon, ShieldCheckIcon, ShieldEllipsisIcon, ShieldIcon, ShieldOffIcon, ShieldXIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { StatusChip, StatusChipDot } from "@/components/branch/StatusChip"
import type { PatronusRobot } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import type { PatronusChipKind } from "@/lib/patronus"
import { formatPushMode, getPatronusChipState, openRobot } from "@/lib/patronus"
import { statusChipTextColors } from "@/lib/status-chip"
import { formatTimestamp } from "@/lib/time"
import { useBranchTip } from "@/stores/branch-sync"
import { useBranchRobot } from "@/stores/patronus"

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
      details={<PatronusRobotDetails robot={robot} localTip={localTip} />}
      detailsTestId="patronus-popover"
      onActivate={() => openRobot(robot)}
    >
      <Icon className={cn("size-3.5", chip.kind === "running" && "animate-pulse")} />
      {!compact && chip.label}
      {chip.outdated && <StatusChipDot color="warning" testId="patronus-outdated-dot" />}
    </StatusChip>
  )
}

export interface PatronusRobotDetailsProps {
  robot: PatronusRobot
  localTip: string | undefined
}

/**
 * The mode and the state, the name, the target, the commit and the times, a notice when the robot tests another commit
 * than the local branch, and "Open in Patronus". The chip popover and the branch preview show it.
 */
export function PatronusRobotDetails({ robot, localTip }: PatronusRobotDetailsProps) {
  const chip = getPatronusChipState(robot, localTip)
  const Icon = icons[chip.kind]
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

      <div>
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
      <PatronusRobotDetails robot={robot} localTip={localTip} />
    </section>
  )
}
