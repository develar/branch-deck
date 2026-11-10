import type { CSSProperties, ReactNode } from "react"
import { cn } from "cn"
import { LandingNodeHover } from "@/components/branch/LandingNodeHover"
import { RING_AROUND_EDGE_REM, RobotRing } from "@/components/branch/RobotProgress"
import { useLandingNode } from "@/hooks/useLandingNode"
import type { LandingNodeKind } from "@/lib/bindings"
import { robotRingColor } from "@/lib/patronus"
import type { LaneSegments, StackLanes } from "@/lib/stack-lanes"
import { lanesBelow, segmentLanesBelow } from "@/lib/stack-lanes"
import type { UIColor } from "@/lib/ui-types"
import { landingNodeLooks } from "@/presenters/landing-node"
import { useBranchRobot, useRobotFailing } from "@/stores/patronus"
import { useRobotLineIntoRail } from "@/stores/robot-coverage"
import { useIsPushedOnPath, useStackPathSegments } from "@/stores/stack-view"

/**
 * A lane and a branch row in the units of the SVG of a fork curve: 16px and 36px at 100% zoom (`--bd-lane-width`, the
 * row around the folder button). The SVG stretches over the actual row; its stroke keeps its width (`vector-effect:
 * non-scaling-stroke`).
 */
const LANE = 16
const ROW = 36
/** The radius of the node, `size-2`, in the units of a lane: the fork curve comes in horizontally to its edge */
const NODE_RADIUS = 4

/**
 * Where the lines into the node stop, from its centre (`--bd-node-cut`): at its edge, so a hollow node stays empty, or
 * at the outer edge of the ring of a running robot around it. In rem as the node and the ring.
 */
const NODE_CUT_REM = NODE_RADIUS / 16

function cssVars(vars: Record<`--${string}`, number>): CSSProperties {
  return vars as CSSProperties
}

/** The height of a straight line: over the row, or from its top or to its bottom up to the edge of the node or its ring */
const BAR_SPAN = {
  row: "inset-y-0",
  up: "top-0 bottom-[calc(50%+var(--bd-node-cut))]",
  down: "top-[calc(50%+var(--bd-node-cut))] bottom-0",
} as const

/**
 * A straight part of a lane: a box as the rail draws its line (`bd-lane-x` in index.css), so the line of a branch row
 * and the line below it snap to the same device pixels; in the colour (`currentColor`) and width (`--bd-bar-width`)
 * of its tone
 */
function LaneBar({ lane, span }: { lane: number; span: keyof typeof BAR_SPAN }) {
  return <span className={cn("absolute bd-lane-x w-(--bd-bar-width) bg-current", BAR_SPAN[span])} style={cssVars({ "--bd-bar-lane": lane })} />
}

/**
 * The curve of a side child from its parent's lane at the top of the row into the left edge of its node, masked at the
 * node or its ring (`bd-node-cut`), in the colour and width of its tone. Its SVG starts where a box of the parent's lane
 * does (`bd-lane-x`), its stroke half the width further in: the SVG snaps to the device pixels as that box does, so the
 * curve leaves the line above it in the same pixels.
 */
function ForkCurve({ lane, forkFrom }: { lane: number; forkFrom: number }) {
  const lanes = lane - forkFrom
  const middle = ROW / 2
  return (
    <span className="absolute inset-0 bd-node-cut">
      <svg
        className="absolute inset-y-0 bd-lane-x h-full w-[calc(var(--bd-fork-lanes)*var(--bd-lane-width))] overflow-visible"
        style={cssVars({ "--bd-bar-lane": forkFrom, "--bd-fork-lanes": lanes })}
        viewBox={`0 0 ${lanes * LANE} ${ROW}`}
        preserveAspectRatio="none"
      >
        <path
          d={`M0 0Q0 ${middle} ${lanes * LANE - NODE_RADIUS} ${middle}`}
          className="fill-none stroke-current stroke-(length:--bd-bar-width)"
          style={{ transform: "translateX(calc(var(--bd-bar-width) / 2))" }}
          vectorEffect="non-scaling-stroke"
        />
      </svg>
    </span>
  )
}

function hasSegments(segments: LaneSegments, forkFrom: number | null): boolean {
  return segments.up || segments.down || (segments.fork && forkFrom !== null) || segments.through.length > 0
}

/** The segments of one tone in a branch row: the lanes passing it, the line up, the line down, the fork curve */
function RowSegments({ segments, lanes }: { segments: LaneSegments; lanes: StackLanes }) {
  const { lane, forkFrom } = lanes
  return (
    <>
      {segments.through.map(it => (
        <LaneBar key={it} lane={it} span="row" />
      ))}
      {segments.up && <LaneBar lane={lane} span="up" />}
      {segments.down && <LaneBar lane={lane} span="down" />}
      {segments.fork && forkFrom !== null && <ForkCurve lane={lane} forkFrom={forkFrom} />}
    </>
  )
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
  /**
   * The expanded commits of the branch are on its lane below the row: the line of the robot that covers the first of
   * them starts at the node (`RobotLanesIntoRail`)
   */
  commitsBelow?: boolean
}

/**
 * The start of the name cell of a branch row in a table with stacks: a spacer as wide as the lane column of the table
 * (so every name starts at the same x), and over the whole row height (the cell is `relative`) the lines of the lanes
 * and the node of the branch: the highlighted path to the root of the stack (`useStackPathSegments`) over them, the
 * landing span of a running robot on top. The lines stop at the edge of the node, or of the ring of its robot.
 */
export function LaneCell({ lanes, name, familyIndent, lineDown, collapsedBelow = false, ring = false, highlight = null, commitsBelow = false }: LaneCellProps) {
  const { lane, forkFrom, up, through } = lanes
  const path = useStackPathSegments(name)
  const lines: LaneSegments = { up, down: lineDown, fork: forkFrom !== null, through }
  const drawn = Math.max(lane, ...through) + 1
  const cut = ring ? RING_AROUND_EDGE_REM : NODE_CUT_REM
  return (
    <>
      <span aria-hidden className={cn("shrink-0", familyIndent === 1 ? "w-[calc(var(--bd-lane-column)+var(--bd-family-step))]" : "w-(--bd-lane-column)")} />
      {familyIndent === 1 && <FamilyGuide />}
      <span
        aria-hidden
        className="pointer-events-none absolute inset-y-0 left-(--bd-lane-start) [--bd-bar-width:var(--bd-lane-line-width)]"
        style={{ ...cssVars({ "--bd-lanes-drawn": drawn, "--bd-node-lane": lane }), "--bd-node-cut": `${cut}rem` } as CSSProperties}
      >
        {hasSegments(lines, forkFrom) && (
          <span className="absolute inset-y-0 left-0 w-[calc(var(--bd-lanes-drawn)*var(--bd-lane-width))] text-(--bd-lane-line)" data-testid="lane-lines">
            <RowSegments segments={lines} lanes={lanes} />
            {path !== null && hasSegments(path, forkFrom) && (
              <Highlight tone="path">
                <RowSegments segments={path} lanes={lanes} />
              </Highlight>
            )}
            {commitsBelow && highlight?.down !== true ? (
              <RobotLanesIntoRail name={name} lanes={lanes} highlight={highlight} />
            ) : (
              highlight !== null && <RobotSegments segments={highlight} lanes={lanes} failing={highlight.failing === true} />
            )}
          </span>
        )}
        {collapsedBelow && (
          <span
            className="absolute top-[calc(50%+var(--bd-node-cut)+0.125rem)] bd-lane-x h-2.5 w-(--bd-bar-width) bg-linear-to-b from-(--bd-lane-line) to-transparent"
            style={cssVars({ "--bd-bar-lane": lane })}
            data-testid="lane-collapsed"
          />
        )}
        <LaneNodeMark name={name} ring={ring} />
      </span>
    </>
  )
}

/** No highlighted segment: the robot's line into the rail of a row outside a landing span */
const NO_SEGMENTS: LaneSegments = { up: false, fork: false, down: false, through: [] }

/**
 * The landing span of an expanded branch row, with the line of the robot from the node down when it covers the first
 * expanded commit (`useRobotLineIntoRail`): the rail starts with that line, so the robot's line is one from the node,
 * of its own branch, of the branch a robot below lands, or of the whole stack. Its own component: only an expanded row
 * reads the coverage.
 */
function RobotLanesIntoRail({ name, lanes, highlight }: { name: string; lanes: StackLanes; highlight: LaneSegments | null }) {
  const tone = useRobotLineIntoRail(name, lanes.alone === true)
  if (tone === null) {
    return highlight === null ? null : <RobotSegments segments={highlight} lanes={lanes} failing={highlight.failing === true} />
  }
  return <RobotSegments segments={{ ...(highlight ?? NO_SEGMENTS), down: true }} lanes={lanes} failing={highlight === null ? tone === "failing" : highlight.failing === true} />
}

/** The landing span of a robot in a branch row (`landing-lanes`); nothing without a segment */
function RobotSegments({ segments, lanes, failing }: { segments: LaneSegments; lanes: StackLanes; failing: boolean }) {
  if (!hasSegments(segments, lanes.forkFrom)) {
    return null
  }
  return (
    <Highlight tone="robot" failing={failing}>
      <RowSegments segments={segments} lanes={lanes} />
    </Highlight>
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
 * The highlighted segments of a stack graph, `--bd-lane-span-width` wide in the colour their lines take
 * (`currentColor`): `path` the path to the root of the hovered or focused branch (`--bd-lane-path`, fading in), `robot`
 * the landing span of a running robot (`--bd-robot-line`: primary, destructive for one that will fail, as its ring),
 * drawn over it
 */
function Highlight({ tone, failing = false, children }: { tone: "path" | "robot"; failing?: boolean; children: ReactNode }) {
  const span = "absolute inset-0 [--bd-bar-width:var(--bd-lane-span-width)]"
  return tone === "path" ? (
    <span className={cn(span, "text-(--bd-lane-path) bd-path-in")} data-testid="stack-path-lanes">
      {children}
    </span>
  ) : (
    <span className={cn(span, "text-(--bd-robot-line)", failing && "bd-robot-failing")} data-testid="landing-lanes" data-failing={failing || undefined}>
      {children}
    </span>
  )
}

/**
 * The lanes that run on through a row below a branch row (its inline form, its expanded commits), over its whole
 * height; `path` and `highlighted` (of `lanes`) the lanes of the highlighted path and of the landing span of a running
 * robot, `failing` while that robot will fail
 */
export function LaneLines({
  lanes,
  path = [],
  highlighted = [],
  failing = false,
}: {
  lanes: readonly number[]
  path?: readonly number[]
  highlighted?: readonly number[]
  failing?: boolean
}) {
  const all = [...lanes, ...path, ...highlighted]
  if (all.length === 0) {
    return null
  }
  const bars = (of: readonly number[]) => of.map(it => <LaneBar key={it} lane={it} span="row" />)
  return (
    <span
      aria-hidden
      className="pointer-events-none absolute inset-y-0 left-(--bd-lane-start) w-[calc(var(--bd-lanes-drawn)*var(--bd-lane-width))] text-(--bd-lane-line) [--bd-bar-width:var(--bd-lane-line-width)]"
      style={cssVars({ "--bd-lanes-drawn": Math.max(...all) + 1 })}
      data-testid="lane-lines"
    >
      {bars(lanes)}
      {path.length > 0 && <Highlight tone="path">{bars(path)}</Highlight>}
      {highlighted.length > 0 && (
        <Highlight tone="robot" failing={failing}>
          {bars(highlighted)}
        </Highlight>
      )}
    </span>
  )
}

/**
 * The lanes below the branch row of `name` (`lanesBelow`), with the highlighted path and the landing span running on
 * beside its inline form, its amend conflict or its expanded commits (`commitsBelow`: the rail draws its own lane)
 */
export function LanesBelow({ name, lanes, commitsBelow, landing }: { name: string; lanes: StackLanes; commitsBelow: boolean; landing: LaneSegments | null }) {
  const path = useStackPathSegments(name)
  return (
    <LaneLines lanes={lanesBelow(lanes, commitsBelow)} path={segmentLanesBelow(lanes, path)} highlighted={segmentLanesBelow(lanes, landing)} failing={landing?.failing === true} />
  )
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
