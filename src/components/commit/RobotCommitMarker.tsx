import { RobotActionButton } from "@/components/branch/PatronusChip"
import { StatusChip } from "@/components/branch/StatusChip"
import { getRobotItems, useRobotParts } from "@/hooks/useBranchNextStep"
import type { PatronusRobot } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { formatPushMode } from "@/lib/patronus"
import { patronusChipIcons } from "@/lib/patronus-icons"
import { formatTimestamp } from "@/lib/time"
import { useCoveringRobot } from "@/stores/robot-coverage"

export interface RobotCommitMarkerProps {
  /** The branch of the commit row */
  branchName: string
  /** Why the robot lacks the commit (`RobotRailMark.change`) */
  change: "added" | "changed"
}

/**
 * "not in Safe Push" after the subject of a commit that the running robot that tests the branch lacks: added or changed
 * since it started, so its result won't cover the commit. `warning`, as the outdated dot of the robot. Hover shows the
 * robot and its actions as its own details name them ("Restart Safe Push" tests the branch as it is).
 */
export function RobotCommitMarker({ branchName, change }: RobotCommitMarkerProps) {
  const robot = useCoveringRobot(branchName)
  if (robot === null) {
    return null
  }
  const mode = formatPushMode(robot.pushMode)
  return (
    <StatusChip
      label={`Not in ${mode}`}
      color="warning"
      data={{ "data-testid": "robot-commit-marker", "data-change": change }}
      details={<RobotCommitDetails robot={robot} change={change} />}
      detailsTestId="robot-commit-popover"
    >
      {`not in ${mode}`}
    </StatusChip>
  )
}

/** The robot that lacks the commit, why, and the actions of the robot */
function RobotCommitDetails({ robot, change }: { robot: PatronusRobot; change: "added" | "changed" }) {
  // the robot of a branch built on this one, or of the whole stack: its actions are the ones Rust offers of it
  const robotParts = useRobotParts(robot)
  const mode = formatPushMode(robot.pushMode)
  const started = robot.sourceRevision === null ? "" : ` on ${formatShortHash(robot.sourceRevision)}`
  const title = change === "added" ? `Added after the ${mode} started${started}` : `Changed since the ${mode} started${started}`
  // the robot of the whole stack tests the stack branch
  const what = robot.branchName === null && robot.stackRoot != null ? "stack" : "branch"
  const Icon = patronusChipIcons.running
  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="robot-commit-details">
      <div className="flex flex-col gap-1">
        <span className="flex items-center gap-1 text-xs font-medium text-warning">
          <Icon className="size-3.5" />
          {`Not in ${mode}`}
        </span>
        <p className="leading-snug font-medium break-words" data-testid="robot-commit-title">
          {title}
        </p>
      </div>
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
        <dt className="text-muted-foreground">Robot</dt>
        <dd className="break-words">{robot.name}</dd>
        {robot.startTime > 0 && (
          <>
            <dt className="text-muted-foreground">Started</dt>
            <dd className="text-muted-foreground">{formatTimestamp(robot.startTime)}</dd>
          </>
        )}
      </dl>
      <p className="text-xs text-muted-foreground">{`The result won't cover this commit. Restart ${mode} to test the current ${what}.`}</p>
      <div className="flex flex-wrap gap-2">
        {getRobotItems(robot, robotParts).map(action => (
          <RobotActionButton key={action.id} action={action} />
        ))}
      </div>
    </div>
  )
}
