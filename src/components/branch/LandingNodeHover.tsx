import type { ReactElement, ReactNode } from "react"
import { Button } from "@/components/ui/button"
import { PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { LinkedText } from "@/components/commit/LinkedText"
import { RobotRow } from "@/components/repository/RobotRows"
import { HoverDescription } from "@/components/shared/HoverDescription"
import { useLandedIn } from "@/hooks/useLandedIn"
import { getPatronusChipState } from "@/lib/patronus"
import type { RobotOverviewEntry } from "@/lib/robot-overview"
import { formatWholeStackStart } from "@/lib/whole-stack"
import { useRowPart } from "@/model/store"
import type { LandingNode } from "@/presenters/landing-node"
import { presentRowMark } from "@/presenters/row-status"
import { useBranch, useBranchRemoteStatus, useBranchTip, useParkedState, useStackBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
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
  /** Shows the row of a branch the label names; `requestReveal` by default, the landing overview closes first */
  onReveal?: (name: string) => void
}

/**
 * The hover of a landing node: its state in words, the issue keys as links, and for a state of its robot the robot with
 * its actions (`LandingNodeDetails`). A click on the node goes on to the row (or the island header): it never pins or
 * closes the popover (`HoverDescription`). Not a tab stop: the row has its own keys, and the same actions are in its slot, its menu and its robot chip.
 */
export function LandingNodeHover({ node, subject, trigger, children, onReveal }: LandingNodeHoverProps) {
  return (
    <HoverDescription lazy>
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
        <LandingNodeDetails node={node} subject={subject} onReveal={onReveal} />
      </PopoverContent>
    </HoverDescription>
  )
}

/**
 * The state of the node a line each, the issue keys as links; "Show auth" of the branch it names; the facts the Status
 * cell keeps beside what it says first; below it the robot whose state it tells
 */
export function LandingNodeDetails({ node, subject, onReveal = revealBranch }: { node: LandingNode; subject: LandingNodeSubject; onReveal?: (name: string) => void }) {
  const shows = node.shows
  return (
    <>
      <LinkedText text={node.label} className="break-words whitespace-pre-line" linkClassName="text-primary hover:underline" />
      {shows !== undefined && (
        <Button variant="outline" size="xs" className="self-start" onClick={() => onReveal(shows)}>
          {`Show ${shows}`}
        </Button>
      )}
      {subject.kind === "branch" && <BranchNodeAlso name={subject.name} />}
      {node.robot === true && (subject.kind === "branch" ? <BranchNodeRobot name={subject.name} /> : <StackNodeRobot root={subject.root} />)}
    </>
  )
}

/** The facts its Status cell keeps beside what it says first ("Blocked: parent auth has a merge conflict"), a line each */
function BranchNodeAlso({ name }: { name: string }) {
  const branch = useBranch(name)
  const landedIn = useLandedIn(name)
  const robot = useBranchRobot(name)
  const parked = useParkedState(name)
  const status = useRowPart(name, "landing")?.status
  const mark = branch === undefined ? null : presentRowMark(status, { branch, landedIn, robot, parked })
  if (mark === null) {
    return null
  }
  return (
    <p className="break-words whitespace-pre-line text-muted-foreground" data-testid="lane-node-also">
      {mark.lines.join("\n")}
    </p>
  )
}

function revealBranch(name: string): void {
  requestReveal({ branchName: name, commitIds: [] })
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
