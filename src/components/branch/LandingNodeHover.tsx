import type { ReactElement, ReactNode } from "react"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { LinkedText } from "@/components/commit/LinkedText"
import { RobotRow } from "@/components/repository/RobotRows"
import type { LandingNode } from "@/lib/landing-node"
import { getPatronusChipState } from "@/lib/patronus"
import type { RobotOverviewEntry } from "@/lib/robot-overview"
import { formatWholeStackStart } from "@/lib/whole-stack"
import { useBranchRemoteStatus, useBranchTip, useStackBranch } from "@/stores/branch-sync"
import { useBranchRobot, useRobotFailing, useRobotNeedsReview, useStackRobot } from "@/stores/patronus"

/** Whose node it is: a branch of the table, or the whole stack of `root` (the header of its island) */
export type LandingNodeSubject = { readonly kind: "branch"; readonly name: string } | { readonly kind: "stack"; readonly root: string }

export interface LandingNodeHoverProps {
  node: LandingNode
  subject: LandingNodeSubject
  /** The hover target, a `span` (`lane-node-target`, `stack-island-node`) */
  trigger: ReactElement
  /** The node itself (`LaneNodeDot`) */
  children: ReactNode
}

/**
 * The hover of a landing node: its state in words, the issue keys as links, and for a state of its robot the robot with
 * its actions (`LandingNodeDetails`). A click on the node goes on to the row (or the island header): it never pins the
 * popover. Not a tab stop: the row has its own keys, and the same actions are in its slot, its menu and its robot chip.
 */
export function LandingNodeHover({ node, subject, trigger, children }: LandingNodeHoverProps) {
  return (
    <Popover
      onOpenChange={(_open, eventDetails) => {
        if (eventDetails.reason === "trigger-press") {
          eventDetails.cancel()
        }
      }}
    >
      <PopoverTrigger openOnHover delay={300} nativeButton={false} tabIndex={-1} render={trigger}>
        {children}
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-max max-w-96 gap-3 p-3 text-xs"
        data-testid="lane-node-details"
        data-kind={node.kind}
        // the popup is in a portal, but React bubbles its events through the row: a click here must not toggle it
        onClick={event => event.stopPropagation()}
        onContextMenu={event => event.stopPropagation()}
      >
        <LandingNodeDetails node={node} subject={subject} />
      </PopoverContent>
    </Popover>
  )
}

/** The state of the node a line each, the issue keys as links; below it the robot whose state it tells */
export function LandingNodeDetails({ node, subject }: { node: LandingNode; subject: LandingNodeSubject }) {
  return (
    <>
      <LinkedText text={node.label} className="break-words whitespace-pre-line" linkClassName="text-primary hover:underline" />
      {node.robot === true && (subject.kind === "branch" ? <BranchNodeRobot name={subject.name} /> : <StackNodeRobot root={subject.root} />)}
    </>
  )
}

function BranchNodeRobot({ name }: { name: string }) {
  const robot = useBranchRobot(name)
  const localTip = useBranchTip(name)
  const remote = useBranchRemoteStatus(name)
  const needsReview = useRobotNeedsReview(robot?.id)
  const failing = useRobotFailing(robot?.id)
  if (robot === undefined) {
    return null
  }
  const entry: RobotOverviewEntry = { branchName: name, hasRow: true, robot, chip: getPatronusChipState(robot, localTip, remote, { needsReview, failing }) }
  return <NodeRobot entry={entry} />
}

function StackNodeRobot({ root }: { root: string }) {
  const robot = useStackRobot(root)
  const stack = useStackBranch(root)
  const needsReview = useRobotNeedsReview(robot?.id)
  const failing = useRobotFailing(robot?.id)
  if (robot === undefined) {
    return null
  }
  const entry: RobotOverviewEntry = {
    branchName: formatWholeStackStart(root),
    hasRow: true,
    stackRoot: root,
    robot,
    chip: getPatronusChipState(robot, stack?.tip ?? undefined, stack?.remoteStatus ?? null, { needsReview, failing }),
  }
  return <NodeRobot entry={entry} />
}

/** The robot as the landing overview lists it, without the name: the node is of its branch */
function NodeRobot({ entry }: { entry: RobotOverviewEntry }) {
  return (
    <ul className="flex flex-col" data-testid="lane-node-robot">
      <RobotRow entry={entry} />
    </ul>
  )
}
