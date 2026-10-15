import type { CSSProperties } from "react"
import { cn } from "cn"
import { RobotRing } from "@/components/branch/RobotProgress"
import { useLaneNode } from "@/hooks/useStatusBadge"
import type { LandingSegments, StackLanes } from "@/lib/stack-lanes"
import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

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
  branch: Branch
  /** Under its issue: the guide of the family, and the folder button one step further right */
  familyIndent: 0 | 1
  /** The line from the node runs to the bottom of the row: the branch has children, or its commits are shown below */
  lineDown: boolean
  /** The rotating ring around the node: a robot runs on the branch */
  ring?: boolean
  /** The segments of a landing span (`landingSegments`), drawn over the lines in the colour of the robot */
  highlight?: LandingSegments | null
}

/**
 * The start of the name cell of a branch row in a table with stacks: a spacer as wide as the lane column of the table
 * (so every name starts at the same x), and over the whole row height (the cell is `relative`) the lines of the lanes
 * and the node of the branch.
 */
export function LaneCell({ lanes, branch, familyIndent, lineDown, ring = false, highlight = null }: LaneCellProps) {
  const { lane, forkFrom, up, through } = lanes
  const x = laneX(lane)
  const middle = ROW / 2
  const upPath = `M${x} 0V${middle - NODE_RADIUS}`
  const downPath = `M${x} ${middle + NODE_RADIUS}V${ROW}`
  // down from the parent's lane, then into the node from the left
  const forkPath = forkFrom === null ? "" : `M${laneX(forkFrom)} 0Q${laneX(forkFrom)} ${middle} ${x - NODE_RADIUS} ${middle}`
  const paths = through.map(it => `M${laneX(it)} 0V${ROW}`)
  if (up) {
    paths.push(upPath)
  }
  if (lineDown) {
    paths.push(downPath)
  }
  if (forkFrom !== null) {
    paths.push(forkPath)
  }
  const highlighted = highlight === null
    ? []
    : [
        ...highlight.through.map(it => `M${laneX(it)} 0V${ROW}`),
        ...(highlight.up ? [upPath] : []),
        ...(highlight.down ? [downPath] : []),
        ...(highlight.fork ? [forkPath] : []),
      ]
  const drawn = Math.max(lane, ...through) + 1
  return (
    <>
      <span aria-hidden className={cn("shrink-0", familyIndent === 1 ? "w-[calc(var(--bd-lane-column)+var(--bd-family-step))]" : "w-(--bd-lane-column)")} />
      {familyIndent === 1 && <FamilyGuide />}
      <span aria-hidden className="pointer-events-none absolute inset-y-0 left-(--bd-lane-start)" style={cssVars({ "--bd-lanes-drawn": drawn, "--bd-node-lane": lane })}>
        {paths.length > 0 && (
          <svg className="h-full w-[calc(var(--bd-lanes-drawn)*var(--bd-lane-width))] text-(--bd-lane-line)" viewBox={`0 0 ${drawn * LANE} ${ROW}`} preserveAspectRatio="none" data-testid="lane-lines">
            <path d={paths.join("")} fill="none" stroke="currentColor" strokeWidth={1} vectorEffect="non-scaling-stroke" />
            {highlighted.length > 0 && <LandingPath d={highlighted.join("")} />}
          </svg>
        )}
        <LaneNodeMark branch={branch} ring={ring} />
      </span>
    </>
  )
}

/**
 * The node of a branch, an 8px circle on its lane: the colour of its status (the push colour, `destructive` an error
 * or a conflict, `warning` blocked, `info` landed), filled while the branch has work for the remote, hollow when up to
 * date. It is the dot of the status text, which a table with stacks drops.
 */
function LaneNodeMark({ branch, ring }: { branch: Branch, ring: boolean }) {
  const { color, filled } = useLaneNode(branch)
  return (
    <>
      <span
        data-testid="lane-node"
        data-color={color}
        data-filled={filled}
        data-ring={ring || undefined}
        className={cn(
          "absolute top-1/2 left-[calc((var(--bd-node-lane)+0.5)*var(--bd-lane-width))] size-2 -translate-1/2 rounded-full border-[1.5px] border-current",
          nodeColorClass[color],
          filled && "bg-current",
        )}
      />
      {/* the ring of the Safe Push slot, around the node; its own box, as the ring turns and the box is translated */}
      {ring && (
        <span className="absolute top-1/2 left-[calc((var(--bd-node-lane)+0.5)*var(--bd-lane-width))] flex size-4 -translate-1/2" data-testid="lane-ring">
          <RobotRing around className="size-4 text-primary" />
        </span>
      )}
    </>
  )
}

/** The segments of a landing span: the lines of the stack, in the colour of the running robot that lands them */
function LandingPath({ d }: { d: string }) {
  return <path d={d} fill="none" stroke="var(--color-primary)" strokeWidth={1.5} vectorEffect="non-scaling-stroke" data-testid="landing-lanes" />
}

/**
 * The lanes that run on through a row below a branch row (its inline form, its expanded commits), over its whole
 * height; `highlighted` (of `lanes`) in the colour of the running robot that lands them
 */
export function LaneLines({ lanes, highlighted = [] }: { lanes: readonly number[], highlighted?: readonly number[] }) {
  if (lanes.length === 0) {
    return null
  }
  const drawn = Math.max(...lanes) + 1
  return (
    <svg
      aria-hidden
      className="pointer-events-none absolute inset-y-0 left-(--bd-lane-start) h-full w-[calc(var(--bd-lanes-drawn)*var(--bd-lane-width))] text-(--bd-lane-line)"
      style={cssVars({ "--bd-lanes-drawn": drawn })}
      viewBox={`0 0 ${drawn * LANE} 1`}
      preserveAspectRatio="none"
      data-testid="lane-lines"
    >
      <path d={lanes.map(it => `M${laneX(it)} 0V1`).join("")} fill="none" stroke="currentColor" strokeWidth={1} vectorEffect="non-scaling-stroke" />
      {highlighted.length > 0 && <LandingPath d={highlighted.map(it => `M${laneX(it)} 0V1`).join("")} />}
    </svg>
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
export function LaneGutter({ lanes, familyIndent, highlighted }: { lanes: readonly number[], familyIndent: 0 | 1, highlighted?: readonly number[] }) {
  return (
    <>
      <LaneLines lanes={lanes} highlighted={highlighted} />
      {familyIndent === 1 && <FamilyGuide />}
    </>
  )
}
