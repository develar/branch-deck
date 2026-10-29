import type { CSSProperties, ReactNode } from "react"
import { cn } from "cn"
import { LandingNodeHover } from "@/components/branch/LandingNodeHover"
import { RobotRing } from "@/components/branch/RobotProgress"
import { useLandingNode } from "@/hooks/useLandingNode"
import type { LandingNodeKind } from "@/lib/bindings"
import { robotRingColor } from "@/lib/patronus"
import type { LaneSegments, StackLanes } from "@/lib/stack-lanes"
import { lanesBelow, segmentLanesBelow } from "@/lib/stack-lanes"
import type { UIColor } from "@/lib/ui-types"
import { landingNodeLooks } from "@/presenters/landing-node"
import { useBranchRobot, useRobotFailing } from "@/stores/patronus"
import { useIsPushedOnPath, useStackPathSegments } from "@/stores/stack-view"

/**
 * A lane and a branch row in the units of the SVG: 16px and 36px at 100% zoom (`--bd-lane-width`, the row around the
 * folder button). The SVG stretches over the actual row; its strokes keep 1px (`vector-effect: non-scaling-stroke`).
 */
const LANE = 16
const ROW = 36
/** The lines into the node end at its edge, so a hollow node stays empty */
const NODE_RADIUS = 4

function laneX(lane: number): number {
  return lane * LANE + LANE / 2
}

function cssVars(vars: Record<`--${string}`, number>): CSSProperties {
  return vars as CSSProperties
}

/** The text colour of a node, which its border and fill take (`currentColor`) */
const nodeColorClass: Record<UIColor, string> = {
  primary: "text-primary",
  secondary: "text-muted-foreground",
  success: "text-success",
  info: "text-info",
  warning: "text-warning",
  error: "text-destructive",
  neutral: "text-muted-foreground",
}

export interface LaneCellProps {
  lanes: StackLanes
  /** The branch of the row */
  name: string
  /** Under its issue: the guide of the family, and the folder button one step further right */
  familyIndent: 0 | 1
  /** The line from the node runs to the bottom of the row: the branch has children, or its commits are shown below */
  lineDown: boolean
  /** A collapsed stack root: a short line fading out below the node says that its stack goes on, hidden */
  collapsedBelow?: boolean
  /** The rotating ring around the node: a robot runs on the branch */
  ring?: boolean
  /** The segments of a landing span (`landingSegments`), drawn over the lines in the colour of the robot */
  highlight?: LaneSegments | null
}

/** The SVG path of the highlighted segments of a row whose node is on `lane`: the lanes passing it, the line up, the line down, the fork curve */
function segmentPath(segments: LaneSegments, lane: number, forkFrom: number | null): string {
  const x = laneX(lane)
  const middle = ROW / 2
  const parts = segments.through.map(it => `M${laneX(it)} 0V${ROW}`)
  if (segments.up) {
    parts.push(`M${x} 0V${middle - NODE_RADIUS}`)
  }
  if (segments.down) {
    parts.push(`M${x} ${middle + NODE_RADIUS}V${ROW}`)
  }
  if (segments.fork && forkFrom !== null) {
    // down from the parent's lane, then into the node from the left
    parts.push(`M${laneX(forkFrom)} 0Q${laneX(forkFrom)} ${middle} ${x - NODE_RADIUS} ${middle}`)
  }
  return parts.join("")
}

/** The SVG path of lanes running over a whole row below a branch row, in the units of `LaneLines` */
function verticalPath(lanes: readonly number[]): string {
  return lanes.map(it => `M${laneX(it)} 0V1`).join("")
}

/**
 * The start of the name cell of a branch row in a table with stacks: a spacer as wide as the lane column of the table
 * (so every name starts at the same x), and over the whole row height (the cell is `relative`) the lines of the lanes
 * and the node of the branch: the highlighted path to the root of the stack (`useStackPathSegments`) over them, the
 * landing span of a running robot on top.
 */
export function LaneCell({ lanes, name, familyIndent, lineDown, collapsedBelow = false, ring = false, highlight = null }: LaneCellProps) {
  const { lane, forkFrom, up, through } = lanes
  const path = useStackPathSegments(name)
  const lines = segmentPath({ up, down: lineDown, fork: forkFrom !== null, through }, lane, forkFrom)
  const drawn = Math.max(lane, ...through) + 1
  return (
    <>
      <span aria-hidden className={cn("shrink-0", familyIndent === 1 ? "w-[calc(var(--bd-lane-column)+var(--bd-family-step))]" : "w-(--bd-lane-column)")} />
      {familyIndent === 1 && <FamilyGuide />}
      <span aria-hidden className="pointer-events-none absolute inset-y-0 left-(--bd-lane-start)" style={cssVars({ "--bd-lanes-drawn": drawn, "--bd-node-lane": lane })}>
        {lines !== "" && (
          <svg
            className="h-full w-[calc(var(--bd-lanes-drawn)*var(--bd-lane-width))] text-(--bd-lane-line)"
            viewBox={`0 0 ${drawn * LANE} ${ROW}`}
            preserveAspectRatio="none"
            data-testid="lane-lines"
          >
            <path d={lines} fill="none" stroke="currentColor" strokeWidth={1} vectorEffect="non-scaling-stroke" />
            {path !== null && <HighlightPath d={segmentPath(path, lane, forkFrom)} tone="path" />}
            {highlight !== null && <HighlightPath d={segmentPath(highlight, lane, forkFrom)} tone="robot" />}
          </svg>
        )}
        {collapsedBelow && (
          <span
            className="absolute top-[calc(50%+0.375rem)] left-[calc((var(--bd-node-lane)+0.5)*var(--bd-lane-width))] h-2.5 w-px -translate-x-1/2 bg-linear-to-b from-(--bd-lane-line) to-transparent"
            data-testid="lane-collapsed"
          />
        )}
        <LaneNodeMark name={name} ring={ring} />
      </span>
    </>
  )
}

/** The hover target of a node: the lane's 16px square around its 8px circle */
const NODE_TARGET = "pointer-events-auto absolute top-1/2 left-[calc((var(--bd-node-lane)+0.5)*var(--bd-lane-width))] flex size-4 -translate-1/2 items-center justify-center"

/**
 * The node of a branch, an 8px circle on its lane: where the branch is on its way into master (`getLandingNode`), filled
 * while the next step is the user's, hollow while it waits for others or is done; the hover says it, with its links and
 * the robot it tells of (`LandingNodeHover`). The push has the dot of the status text.
 */
function LaneNodeMark({ name, ring }: { name: string; ring: boolean }) {
  const node = useLandingNode(name)
  // the hovered push button sends this branch: the node is circled in its colour
  const pushed = useIsPushedOnPath(name)
  const kind = node?.kind ?? "waiting"
  const dot = <LaneNodeDot kind={kind} ring={ring ? <LaneRobotRing branchName={name} /> : null} pushed={pushed} />
  if (node === null) {
    return (
      <span className={NODE_TARGET} data-testid="lane-node-target">
        {dot}
      </span>
    )
  }
  return (
    <LandingNodeHover node={node} subject={{ kind: "branch", name }} trigger={<span className={NODE_TARGET} data-testid="lane-node-target" />}>
      {dot}
    </LandingNodeHover>
  )
}

/**
 * The 8px circle of a landing state in the colour and form of its kind (`landingNodeLooks`), in a 16px box: the node of
 * a row, of an island header and of the legend; `ring` the ring of a running robot around it, `pushed` circled while
 * the hovered push button sends the branch
 */
export function LaneNodeDot({ kind, ring = null, pushed = false }: { kind: LandingNodeKind; ring?: ReactNode; pushed?: boolean }) {
  const { color, filled } = landingNodeLooks[kind]
  return (
    <span className="relative flex size-4 shrink-0 items-center justify-center">
      <span
        data-testid="lane-node"
        data-kind={kind}
        data-color={color}
        data-filled={filled}
        data-ring={ring !== null || undefined}
        data-pushed={pushed || undefined}
        className={cn(
          "size-2 rounded-full border-[1.5px] border-current",
          nodeColorClass[color],
          filled && "bg-current",
          pushed && "bd-path-in outline-[1.5px] outline-offset-2 outline-current",
        )}
      />
      {/* the ring of the Safe Push slot, around the node; its own box, as the ring turns */}
      {ring !== null && (
        <span className="absolute inset-0 flex" data-testid="lane-ring">
          {ring}
        </span>
      )}
    </span>
  )
}

/** The ring of the branch's running robot, `destructive` for one that will fail */
function LaneRobotRing({ branchName }: { branchName: string }) {
  const robot = useBranchRobot(branchName)
  const failing = useRobotFailing(robot?.id) && robot?.status === "running"
  return <RobotRing around className={cn("size-4", robotRingColor(failing))} />
}

/**
 * The highlighted segments of a stack graph: `path` the path to the root of the hovered or focused branch
 * (`--bd-lane-path`, fading in), `robot` the landing span of a running robot (`--color-primary`), drawn over it
 */
function HighlightPath({ d, tone }: { d: string; tone: "path" | "robot" }) {
  if (d === "") {
    return null
  }
  return tone === "path" ? (
    <path d={d} fill="none" stroke="var(--bd-lane-path)" strokeWidth={1.5} vectorEffect="non-scaling-stroke" className="bd-path-in" data-testid="stack-path-lanes" />
  ) : (
    <path d={d} fill="none" stroke="var(--color-primary)" strokeWidth={1.5} vectorEffect="non-scaling-stroke" data-testid="landing-lanes" />
  )
}

/**
 * The lanes that run on through a row below a branch row (its inline form, its expanded commits), over its whole
 * height; `path` and `highlighted` (of `lanes`) the lanes of the highlighted path and of the landing span of a running
 * robot
 */
export function LaneLines({ lanes, path = [], highlighted = [] }: { lanes: readonly number[]; path?: readonly number[]; highlighted?: readonly number[] }) {
  const all = [...lanes, ...path, ...highlighted]
  if (all.length === 0) {
    return null
  }
  const drawn = Math.max(...all) + 1
  return (
    <svg
      aria-hidden
      className="pointer-events-none absolute inset-y-0 left-(--bd-lane-start) h-full w-[calc(var(--bd-lanes-drawn)*var(--bd-lane-width))] text-(--bd-lane-line)"
      style={cssVars({ "--bd-lanes-drawn": drawn })}
      viewBox={`0 0 ${drawn * LANE} 1`}
      preserveAspectRatio="none"
      data-testid="lane-lines"
    >
      <path d={verticalPath(lanes)} fill="none" stroke="currentColor" strokeWidth={1} vectorEffect="non-scaling-stroke" />
      <HighlightPath d={verticalPath(path)} tone="path" />
      <HighlightPath d={verticalPath(highlighted)} tone="robot" />
    </svg>
  )
}

/**
 * The lanes below the branch row of `name` (`lanesBelow`), with the highlighted path and the landing span running on
 * beside its inline form, its amend conflict or its expanded commits (`commitsBelow`: the rail draws its own lane)
 */
export function LanesBelow({ name, lanes, commitsBelow, landing }: { name: string; lanes: StackLanes; commitsBelow: boolean; landing: LaneSegments | null }) {
  const path = useStackPathSegments(name)
  return <LaneLines lanes={lanesBelow(lanes, commitsBelow)} path={segmentLanesBelow(lanes, path)} highlighted={segmentLanesBelow(lanes, landing)} />
}

/**
 * In a table with stacks, the guide of an issue family over the whole height of a row (the cell is `relative`), under
 * the icon of the issue row above
 */
export function FamilyGuide() {
  return <span aria-hidden className="pointer-events-none absolute inset-y-0 left-[calc(var(--bd-tree-x)+0.5rem)] w-px bg-border" data-testid="family-guide" />
}

/** The lanes and the family guide beside a row below a branch row: its inline form, its amend conflict */
export function LaneGutter({
  name,
  lanes,
  commitsBelow,
  familyIndent,
  landing,
}: {
  name: string
  lanes: StackLanes
  commitsBelow: boolean
  familyIndent: 0 | 1
  landing: LaneSegments | null
}) {
  return (
    <>
      <LanesBelow name={name} lanes={lanes} commitsBelow={commitsBelow} landing={landing} />
      {familyIndent === 1 && <FamilyGuide />}
    </>
  )
}
