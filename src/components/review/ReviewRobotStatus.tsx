import type { ReactNode } from "react"
import { RunningRowRobot } from "@/components/branch/BranchRowStatus"
import type { RunningRowRobotProps } from "@/components/branch/BranchRowStatus"
import { useRobotChip } from "@/hooks/useRowRobot"
import type { PatronusRobot } from "@/lib/bindings"
import type { LandingDescendant } from "@/lib/patronus"
import { formatLandingSubject } from "@/lib/patronus"
import { stackReadinessKey } from "@/lib/whole-stack"
import { useRowPart } from "@/model/store"
import { useBranch, useStackBranch } from "@/stores/branch-sync"
import { useBranchRobot, useLandingSpan, useStackRobot } from "@/stores/patronus"

/**
 * The robot that lands what the review window shows, while it runs, beside the stack bar of its title bar
 * (openspec/specs/branch/branch-review/spec.md, "The title bar shows the robot that lands the branch"), as the Status
 * cell of its row shows it (`RunningRowRobot`): the ring, the time ("1h 36m · ~23m"), the line under it, its details
 * with "Open in Patronus" on hover, a click opens it. Of a branch its own robot while the ring of its node says it runs
 * (`LandingNode.ring`), else the running Safe Push of a branch built on it or of its whole stack that lands it
 * (`useLandingSpan`), "lands with oauth" after it; of the whole stack its robot on the stack branch, while the ring of
 * its node says it runs. Nothing else: no robot runs, a kept branch, a list of commits, a pull request (whose window has
 * none of this).
 */
export function ReviewRobotStatus({ target, wholeStack }: { target: string; wholeStack: boolean }) {
  return wholeStack ? <StackRobotStatus root={target} /> : <BranchRobotStatus name={target} />
}

function BranchRobotStatus({ name }: { name: string }) {
  const robot = useBranchRobot(name)
  const ring = useRowPart(name, "landing")?.node.ring === true
  const span = useLandingSpan(name)
  if (robot !== undefined && ring) {
    return (
      <RobotLine>
        <BranchRobot name={name} robot={robot} room="alone" />
      </RobotLine>
    )
  }
  if (span === null) {
    return null
  }
  return (
    <RobotLine>
      {span.wholeStack === true ? <StackRobot root={span.branch} robot={span.robot} room="shared" /> : <BranchRobot name={span.branch} robot={span.robot} room="shared" />}
      <LandsWith span={span} />
    </RobotLine>
  )
}

function StackRobotStatus({ root }: { root: string }) {
  const robot = useStackRobot(root)
  const ring = useRowPart(stackReadinessKey(root), "landing")?.node.ring === true
  if (robot === undefined || !ring) {
    return null
  }
  return (
    <RobotLine>
      <StackRobot root={root} robot={robot} room="alone" />
    </RobotLine>
  )
}

/**
 * The room the stack bar leaves in the title bar, a container its words fit into (`RunningRowRobot.titleBar`): the
 * branches keep their names, and a narrow window keeps the ring and the line
 */
function RobotLine({ children }: { children: ReactNode }) {
  return (
    <span className="@container flex min-w-3.5 flex-1 items-center gap-2" data-testid="review-robot">
      {children}
    </span>
  )
}

/**
 * "lands with oauth", "lands with the whole air stack": the robot is another's; from 11rem of room, before the time left
 * (`RunningRowRobot.titleBar`), cut at its end; else read aloud alone
 */
function LandsWith({ span }: { span: LandingDescendant }) {
  return (
    <span className="sr-only min-w-0 text-muted-foreground @min-[11rem]:not-sr-only @min-[11rem]:truncate" data-testid="review-robot-lands-with">
      {`lands with ${formatLandingSubject(span)}`}
    </span>
  )
}

/** How the robot shares the room of the title bar (`RunningRowRobotProps.titleBar`) */
type Room = NonNullable<RunningRowRobotProps["titleBar"]>

/** The running robot of the branch `name` */
function BranchRobot({ name, robot, room }: { name: string; robot: PatronusRobot; room: Room }) {
  const branch = useBranch(name)
  const chip = useRobotChip(robot, name, branch?.commits[0]?.hash, branch?.remoteStatus ?? null)
  return <RunningRowRobot branchName={name} robot={robot} chip={chip} stackPath={false} titleBar={room} />
}

/** The running robot of the whole stack of `root`, on its stack branch: its commit is the stack branch's */
function StackRobot({ root, robot, room }: { root: string; robot: PatronusRobot; room: Room }) {
  const stack = useStackBranch(root)
  const chip = useRobotChip(robot, stackReadinessKey(root), stack?.tip ?? undefined, stack?.remoteStatus ?? null)
  return <RunningRowRobot branchName={root} robot={robot} chip={chip} stackPath={false} stackRoot={root} titleBar={room} />
}
