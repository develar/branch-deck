import { useEffect, useLayoutEffect, useRef, useState } from "react"
import type { CSSProperties, Dispatch, ReactNode, SetStateAction } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotRing } from "@/components/branch/RobotProgress"
import { LaneNodeDot } from "@/components/branch/StackLanes"
import { PriceLine } from "@/components/landing/PriceLine"
import { RunningTime } from "@/components/repository/RobotRows"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { edgeRunners } from "@/hooks/useBranchNextStep"
import { useLandingOverview } from "@/hooks/useLandingOverview"
import type { LandTogether, LandingEdge, LandingPlan, LandingUnit as PlanUnit } from "@/lib/bindings"
import type { LandingOverview, LandingUnit } from "@/lib/landing-overview"
import type { PlanBox, PlanCurve } from "@/lib/landing-plan-layout"
import { edgeCurve, layoutPlan } from "@/lib/landing-plan-layout"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { observeSize } from "@/lib/observe-size"
import { robotRingColor } from "@/lib/patronus"
import type { RobotOverviewEntry } from "@/lib/robot-overview"
import { isFailingEntry } from "@/lib/robot-overview"
import { describeEdge } from "@/presenters/landing"
import {
  describeTogetherPrice,
  formatCheckCount,
  formatEdgeEvidence,
  formatEdgeFiles,
  formatEdgeNeeded,
  formatEdgeState,
  formatUnitSize,
  formatWaveTitle,
} from "@/presenters/landing-plan"
import { edgeActionItems } from "@/presenters/next-step"
import { useBranchSync } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { useIndicatorClaim, useSurfaceBusy } from "@/stores/indicators"
import type { ShownEdge } from "@/stores/landing-section"
import { showLandingEdge, useShownEdge } from "@/stores/landing-section"
import { toggleStackCollapsed } from "@/stores/stack-collapse"

/**
 * The narrowest and the widest column of a wave, and the room between two for the curves and their chips: a plan of few
 * units keeps its cards narrow and leaves the rest of the island empty, a frame hugs its pair; past the room of the
 * island the columns keep their narrowest and the picture scrolls across
 */
const COLUMN_MIN_REM = 10
const COLUMN_MAX_REM = 18
const COLUMN_GAP_REM = 4.5
/** The middle of the first line of a card from its top (its `py-2` and a 20px line): where the curves meet it */
const HEADER_ANCHOR_PX = 18
/** The padding of the picture inside its island, each side */
const PADDING_REM = 1.5

/**
 * The plan of Landing (openspec/specs/branch/landing-plan/spec.md, "Picture"): the units on their way into master in the
 * waves Rust decides, from left to right, the edges between them as curves with their evidence on hover, each offer to
 * land two in one robot as a dashed frame around the pair with its price and its buttons, the parked branches apart.
 * A stale plan says so, offers nothing, and keeps what it showed dimmed. It decides nothing: the units, the edges, the
 * waves, the offers, their price and the actions of each edge are Rust's (`RepositoryView.landingPlan`), the page lays
 * them out (`layoutPlan`) and words them (`src/presenters/landing-plan.ts`).
 */
export function LandingPlanSection() {
  const plan = useBranchSync(state => state.landingPlan)
  const units = useBranchSync(state => state.landingUnits)
  const overview = useLandingOverview()
  if (plan === null) {
    return <LoadingLine className="px-1">Reading the plan…</LoadingLine>
  }
  return (
    <div className="flex flex-col gap-2" data-testid="landing-plan" data-stale={plan.stale || undefined}>
      {plan.stale && (
        <p className="flex items-center gap-2 px-1 text-xs text-muted-foreground" data-testid="landing-plan-stale">
          <Spinner className="size-3.5" />
          Computing what each start needs: the waves may change, and nothing is offered to land together until they are known
        </p>
      )}
      {plan.waves.length > 0 ? (
        <PlanCard plan={plan} units={units} overview={overview} />
      ) : (
        <Card className="px-4 py-3 text-sm text-muted-foreground sm:px-6" data-testid="landing-plan-empty">
          Nothing is on its way into master
        </Card>
      )}
      {plan.parked.length > 0 && <ParkedCard names={plan.parked} />}
    </div>
  )
}

/** An edge the picture draws: from the unit that waits to the unit it needs, both in a wave */
interface DrawnEdge {
  readonly edge: LandingEdge
  readonly index: number
  readonly needed: string
}

/** A drawn edge with its curve between the measured cards */
interface EdgeOnCurve extends DrawnEdge {
  readonly curve: PlanCurve
}

/** The curves of the drawn edges between the cards as measured; none of a card not measured yet */
function curvesOf(drawn: readonly DrawnEdge[], boxes: ReadonlyMap<string, PlanBox>): EdgeOnCurve[] {
  return drawn.flatMap(it => {
    const from = boxes.get(it.edge.from)
    const to = boxes.get(it.needed)
    return from === undefined || to === undefined ? [] : [{ ...it, curve: edgeCurve(to, from, HEADER_ANCHOR_PX) }]
  })
}

/** The unit `name` shows its row in the branches: a stack expanded, its root focused */
function revealUnit(name: string, stack: boolean): void {
  if (stack) {
    toggleStackCollapsed(name, false)
  }
  requestReveal({ branchName: name, commitIds: [] })
}

/** The edge shown by the Status cell of a start: of its branch or of the root it is lifted to, to the branch it needs */
function isShownEdge(edge: LandingEdge, shown: ShownEdge | null): boolean {
  return shown !== null && edge.needed === shown.needed && (edge.from === shown.branch || edge.refuses.includes(shown.branch))
}

/** The offer that lands the unit of `edge` with the unit it needs, of its own edge only */
function proposalOf(edge: LandingEdge, proposals: readonly LandTogether[]): LandTogether | undefined {
  return edge.kind === "build" ? proposals.find(proposal => proposal.from === edge.from && proposal.onto === edge.needed) : undefined
}

function PlanCard({ plan, units, overview }: { plan: LandingPlan; units: Readonly<Record<string, PlanUnit>>; overview: LandingOverview }) {
  const { edge: shown, shows } = useShownEdge()
  const [hovered, setHovered] = useState<number | null>(null)
  const unitOf = new Map<string, string>()
  for (const unit of Object.values(units)) {
    for (const branch of unit.branches) {
      unitOf.set(branch, unit.name)
    }
  }
  const pairs = plan.proposals.map(proposal => ({ from: proposal.from, onto: proposal.ontoUnit }))
  const needsOf = (unit: string) => plan.edges.flatMap(edge => (edge.from === unit ? [unitOf.get(edge.needed) ?? edge.needed] : []))
  const layout = layoutPlan(
    plan.waves.map(wave => wave.units),
    pairs,
    needsOf,
  )
  const drawn: DrawnEdge[] = plan.edges.flatMap((edge, index) => {
    const needed = unitOf.get(edge.needed)
    return needed !== undefined && needed !== edge.from && layout.cells.has(needed) && layout.cells.has(edge.from) ? [{ edge, index, needed }] : []
  })
  const nodes = new Map(overview.onTheWay.map(unit => [unit.name, unit]))
  const robots = [...overview.running, ...overview.failingRobots]
  const drawnEdges = new Set(drawn.map(it => it.edge))
  const robotOf = (unit: PlanUnit) => robots.find(entry => unit.branches.includes(entry.stackRoot ?? entry.branchName))
  // what moves a card: its cell, the frames, the lines of its edges and its reason
  const layoutKey = JSON.stringify([
    [...layout.cells],
    layout.frames,
    plan.edges.map(edge => [edge.from, edge.needed, edge.state.kind]),
    [...nodes.values()].map(unit => unit.node.label),
  ])
  const { container, cards, boxes } = useCardBoxes(layoutKey)
  const curves = curvesOf(drawn, boxes)
  const shownIndex = plan.edges.findIndex(edge => isShownEdge(edge, shown))
  const shownUnit = shownIndex === -1 ? null : plan.edges[shownIndex]!.from
  // a show of an edge scrolls the card of the unit that waits into view, again for the same edge shown twice
  useEffect(() => {
    if (shows > 0 && shownUnit !== null) {
      cards.current.get(shownUnit)?.scrollIntoView({ block: "nearest", inline: "nearest" })
    }
  }, [shows, shownUnit, cards])
  const emphasized = hovered ?? (shownIndex === -1 ? null : shownIndex)
  // the ends of the emphasized edge stay as they are, the rest fades
  const emphasizedEdge = emphasized === null ? null : (plan.edges[emphasized] ?? null)
  const ends = emphasizedEdge === null ? null : new Set([emphasizedEdge.from, unitOf.get(emphasizedEdge.needed) ?? emphasizedEdge.needed])
  const style: CSSProperties = {
    gridTemplateColumns: `repeat(${layout.columns}, minmax(${COLUMN_MIN_REM}rem, ${COLUMN_MAX_REM}rem))`,
    justifyContent: "start",
    gridTemplateRows: `auto repeat(${layout.rows}, auto)`,
    columnGap: `${COLUMN_GAP_REM}rem`,
    padding: `0.75rem ${PADDING_REM}rem`,
    minWidth: `${layout.columns * COLUMN_MIN_REM + (layout.columns - 1) * COLUMN_GAP_REM + 2 * PADDING_REM}rem`,
    alignItems: "start",
  }
  return (
    <Card className="gap-0 py-0" data-testid="landing-plan-card">
      <ScrollAcross dimmed={plan.stale}>
        <div ref={container} role="group" aria-label="Landing plan" className="relative grid gap-y-2" style={style}>
          {plan.waves.map((wave, column) => (
            <div
              key={`wave:${column}`}
              className="flex min-w-0 items-baseline gap-1.5 pb-1"
              style={{ gridColumn: column + 1, gridRow: 1 }}
              data-testid="plan-wave"
              data-kind={wave.kind.kind}
            >
              <h2 className="min-w-0 truncate text-xs font-semibold text-foreground" title={formatWaveTitle(wave)}>
                {formatWaveTitle(wave)}
              </h2>
              <span className="shrink-0 text-xs text-muted-foreground tabular-nums">{wave.units.length}</span>
            </div>
          ))}
          {layout.frames.map(frame => {
            const proposal = plan.proposals[frame.pair]!
            const edge = plan.edges.find(it => proposalOf(it, [proposal]) !== undefined)
            return (
              <ProposalFrame
                key={`frame:${proposal.from}`}
                proposal={proposal}
                edge={edge ?? null}
                columnStart={frame.columnStart}
                columnEnd={frame.columnEnd}
                row={frame.row}
                faded={ends !== null && !(ends.has(proposal.from) && ends.has(proposal.ontoUnit))}
              />
            )
          })}
          <EdgeCurves curves={curves} emphasized={emphasized} shownIndex={shownIndex} />
          {[...layout.cells].map(([name, cell]) => {
            const unit = units[name]
            if (unit === undefined) {
              return null
            }
            const own = plan.edges.filter(edge => edge.from === name && proposalOf(edge, plan.proposals) === undefined)
            return (
              <UnitCard
                key={name}
                unit={unit}
                node={nodes.get(name) ?? null}
                robot={robotOf(unit) ?? null}
                edges={own}
                drawn={drawnEdges}
                shown={shownUnit === name}
                faded={ends !== null && !ends.has(name)}
                style={{ gridColumn: cell.column + 1, gridRow: cell.row + 2 }}
                cardRef={element => {
                  if (element !== null) {
                    cards.current.set(name, element)
                  }
                  return () => {
                    cards.current.delete(name)
                  }
                }}
              />
            )
          })}
          <EdgeChips curves={curves} shownIndex={shownIndex} emphasized={emphasized} onHover={setHovered} />
        </div>
      </ScrollAcross>
    </Card>
  )
}

/**
 * The picture's own scroller across, past the room of the island at the narrowest window: a fade at each side that has
 * more says so, and the focus a key moves stays clear of the fades (`scroll-padding`), the browser scrolling it into view
 */
function ScrollAcross({ dimmed, children }: { dimmed: boolean; children: ReactNode }) {
  const scroller = useRef<HTMLDivElement>(null)
  const [more, setMore] = useState<{ readonly left: boolean; readonly right: boolean }>({ left: false, right: false })
  useLayoutEffect(() => {
    const element = scroller.current
    if (element === null) {
      return
    }
    const read = () => {
      const left = element.scrollLeft > 1
      const right = element.scrollLeft + element.clientWidth < element.scrollWidth - 1
      setMore(kept => (kept.left === left && kept.right === right ? kept : { left, right }))
    }
    read()
    const sizes = observeSize(read)
    sizes.observe(element)
    if (element.firstElementChild !== null) {
      sizes.observe(element.firstElementChild)
    }
    element.addEventListener("scroll", read, { passive: true })
    return () => {
      sizes.disconnect()
      element.removeEventListener("scroll", read)
    }
  }, [])
  return (
    <div className="relative" data-testid="plan-scroll" data-more-left={more.left || undefined} data-more-right={more.right || undefined}>
      <div ref={scroller} className={cn("scroll-px-10 overflow-x-auto overscroll-x-contain transition-opacity", dimmed && "opacity-60")}>
        {children}
      </div>
      {more.left && <div aria-hidden className="pointer-events-none absolute inset-y-0 left-0 z-30 w-16 bg-linear-to-r from-card to-transparent" />}
      {more.right && <div aria-hidden className="pointer-events-none absolute inset-y-0 right-0 z-30 w-16 bg-linear-to-l from-card to-transparent" />}
    </div>
  )
}

/**
 * The boxes of the cards of the picture in its coordinates, measured after each render that lays them out anew
 * (`layoutKey`: the cells, the frames, what the cards say) and as the picture resizes (the curves need them; the grid
 * places the cards); the same map while no box moved
 */
function useCardBoxes(layoutKey: string) {
  const container = useRef<HTMLDivElement>(null)
  const cards = useRef(new Map<string, HTMLElement>())
  const [boxes, setBoxes] = useState<ReadonlyMap<string, PlanBox>>(new Map())
  useLayoutEffect(() => measureCards(cards.current, setBoxes), [layoutKey])
  useLayoutEffect(() => {
    const element = container.current
    if (element === null) {
      return
    }
    const sizes = observeSize(() => measureCards(cards.current, setBoxes))
    sizes.observe(element)
    return () => sizes.disconnect()
  }, [])
  return { container, cards, boxes }
}

/** The boxes of `cards` now, kept when none moved */
function measureCards(cards: ReadonlyMap<string, HTMLElement>, setBoxes: Dispatch<SetStateAction<ReadonlyMap<string, PlanBox>>>): void {
  const next = new Map<string, PlanBox>()
  for (const [name, element] of cards) {
    next.set(name, { left: element.offsetLeft, top: element.offsetTop, width: element.offsetWidth, height: element.offsetHeight })
  }
  setBoxes(kept => (sameBoxes(kept, next) ? kept : next))
}

function sameBoxes(kept: ReadonlyMap<string, PlanBox>, next: ReadonlyMap<string, PlanBox>): boolean {
  if (kept.size !== next.size) {
    return false
  }
  for (const [name, box] of next) {
    const old = kept.get(name)
    if (old === undefined || old.left !== box.left || old.top !== box.top || old.width !== box.width || old.height !== box.height) {
      return false
    }
  }
  return true
}

/** The colour of the curve of an edge by its state: a wait muted, a need nothing lands in warning, a failed robot in red */
function edgeTone(edge: LandingEdge): string {
  if (edge.state.kind === "neededFailed") {
    return "text-destructive"
  }
  if (edge.state.kind === "open" && edge.kind === "build") {
    return "text-warning"
  }
  return "text-muted-foreground"
}

/**
 * The curves of the edges, under the cards; the emphasized one (hovered, or shown by a Status cell) stronger, the rest
 * faded; the shown one in primary
 */
function EdgeCurves({ curves, emphasized, shownIndex }: { curves: readonly EdgeOnCurve[]; emphasized: number | null; shownIndex: number }) {
  return (
    <svg aria-hidden className="pointer-events-none absolute inset-0 z-0 size-full overflow-visible">
      {curves.map(({ edge, index, curve }) => {
        const strong = emphasized === index
        return (
          <path
            key={`${edge.from}>${edge.needed}:${edge.kind}`}
            d={curve.path}
            fill="none"
            stroke="currentColor"
            strokeWidth={strong ? 2.5 : 1.5}
            strokeDasharray={edge.kind === "build" ? undefined : "4 3"}
            strokeLinecap="round"
            className={cn(index === shownIndex ? "text-primary" : edgeTone(edge), "transition-opacity", emphasized !== null && !strong && "opacity-30")}
            data-testid="plan-edge-curve"
          />
        )
      })}
    </svg>
  )
}

/**
 * The chip of each drawn edge halfway along its curve: its evidence in a few words ("Effect, Input +3"), the whole edge
 * in its hover (the names, where the branch it needs stands, the files); a click shows it as a Status cell does
 */
function EdgeChips({
  curves,
  shownIndex,
  emphasized,
  onHover,
}: {
  curves: readonly EdgeOnCurve[]
  shownIndex: number
  emphasized: number | null
  onHover: (index: number | null) => void
}) {
  return curves.map(({ edge, index, curve }) => {
    const { middle } = curve
    const sentence = describeEdge(edge)
    const files = formatEdgeFiles(edge)
    return (
      <Tooltip key={`${edge.from}>${edge.needed}:${edge.kind}`}>
        <TooltipTrigger
          render={
            <button
              type="button"
              aria-label={`${edge.from} needs ${edge.needed}: ${formatEdgeEvidence(edge)}`}
              className={cn(
                "absolute z-20 max-w-[4.5rem] -translate-x-1/2 -translate-y-1/2 truncate rounded-full border bg-card px-1.5 text-[11px] leading-[18px] outline-none hover:text-foreground focus-visible:bd-focus",
                shownIndex === index ? "border-primary text-foreground" : "border-border text-muted-foreground",
                emphasized !== null && emphasized !== index && "opacity-40",
              )}
              style={{ left: middle.x, top: middle.y }}
              onPointerEnter={() => onHover(index)}
              onPointerLeave={() => onHover(null)}
              onFocus={() => onHover(index)}
              onBlur={() => onHover(null)}
              onClick={() => showLandingEdge({ branch: edge.from, needed: edge.needed })}
              data-testid="plan-edge"
              data-from={edge.from}
              data-needed={edge.needed}
              data-state={edge.state.kind}
              data-kind={edge.kind}
              data-shown={shownIndex === index || undefined}
            />
          }
        >
          {formatEdgeEvidence(edge)}
        </TooltipTrigger>
        <TooltipContent className="max-w-sm break-words whitespace-pre-line" data-testid="plan-edge-hover">
          {`${sentence}${files === "" ? "" : `\n${files}`}`}
        </TooltipContent>
      </Tooltip>
    )
  })
}

/**
 * A unit as a card: its node (the ring of its robot around it), its name (a click anywhere on the card shows its row
 * in the branches), the time of its running robot, its size and the required checks it starts; then the edges no curve
 * draws (an unassigned commit, a cycle, a companion, a branch in no wave) in a line each, and the actions of its edges
 * that are not offers to land together (Restart, Land … First, Group, the start of a cycle)
 */
function UnitCard({
  unit,
  node,
  robot,
  edges,
  drawn,
  shown,
  faded,
  style,
  cardRef,
}: {
  unit: PlanUnit
  node: LandingUnit | null
  robot: RobotOverviewEntry | null
  edges: readonly LandingEdge[]
  drawn: ReadonlySet<LandingEdge>
  shown: boolean
  /** Another edge is emphasized: the card fades, as the curves off it do */
  faded: boolean
  style: CSSProperties
  cardRef: (element: HTMLElement | null) => () => void
}) {
  const kind = node?.node.kind ?? unit.kind
  const reason = node?.node.label.split("\n")[0] ?? ""
  const running = robot !== null && robot.chip.kind === "running"
  const checks = formatCheckCount(unit.checks)
  const stack = unit.branches.length > 1
  return (
    <div
      ref={cardRef}
      className={cn(
        "relative z-10 flex min-w-0 flex-col gap-1 rounded-lg border bg-card px-3 py-2 transition-[border-color,opacity] hover:border-muted-foreground/40",
        shown ? "border-primary" : "border-border",
        faded && "opacity-40",
      )}
      style={style}
      data-testid="plan-unit"
      data-unit={unit.name}
      data-kind={kind}
      data-shown={shown || undefined}
      data-faded={faded || undefined}
    >
      <div className="flex min-w-0 items-center gap-2">
        <LaneNodeDot kind={kind} ring={running ? <RobotRing around className={robotRingColor(isFailingEntry(robot))} /> : null} />
        <Tooltip>
          <TooltipTrigger
            render={
              <button
                type="button"
                className="min-w-0 truncate rounded-sm text-left text-sm font-medium outline-none after:absolute after:inset-0 after:rounded-lg hover:underline focus-visible:bd-focus"
                onClick={() => revealUnit(unit.name, stack)}
              />
            }
          >
            {unit.name}
          </TooltipTrigger>
          <TooltipContent>{`Show ${unit.name}`}</TooltipContent>
        </Tooltip>
        {running && (
          <span className="ms-auto shrink-0 text-xs whitespace-nowrap text-muted-foreground tabular-nums" data-testid="plan-unit-robot">
            <RunningTime entry={robot} />
          </span>
        )}
      </div>
      <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
        <span className="truncate tabular-nums">{formatUnitSize(unit.branches.length, unit.commitCount)}</span>
        {checks !== null && <ChecksChip checks={unit.checks ?? []} label={checks} />}
      </div>
      {reason !== "" && (
        <span className="truncate text-xs text-muted-foreground" title={reason} data-testid="plan-unit-reason">
          {reason}
        </span>
      )}
      {edges.map(edge => (
        <EdgeLine key={`${edge.needed}:${edge.kind}`} edge={edge} drawn={drawn.has(edge)} />
      ))}
    </div>
  )
}

/** "7 checks": the required checks of Patronus its changes start, each named in its hover */
function ChecksChip({ checks, label }: { checks: readonly string[]; label: string }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            className="relative z-20 shrink-0 rounded-sm border border-border px-1 leading-4 outline-none hover:text-foreground focus-visible:bd-focus"
            data-testid="plan-unit-checks"
          />
        }
      >
        {label}
      </TooltipTrigger>
      <TooltipContent className="max-w-sm whitespace-pre-line">{checks.length === 0 ? "Starts no required check" : checks.join("\n")}</TooltipContent>
    </Tooltip>
  )
}

/**
 * The actions of an edge of the unit that are no offer to land together, from the one builder of Rust
 * (`LandingEdge.actions`): beside a curve only its buttons; of an edge no curve draws its words first ("Needs
 * 1a2b3c4d (unassigned) · unassigned"), the whole edge in their hover. Land Together is offered only with its price, in
 * the frame of an offer
 */
function EdgeLine({ edge, drawn }: { edge: LandingEdge; drawn: boolean }) {
  const surface = `landing-plan:${edge.from}`
  useIndicatorClaim(surface)
  const busy = useSurfaceBusy(surface)
  const sentence = describeEdge(edge)
  const items = edgeActionItems(edge, [], sentence, edgeRunners(edge.from, surface, { reveal: false }))
  if (drawn && items.length === 0) {
    return null
  }
  return (
    <div className="relative z-20 flex flex-col gap-1" data-testid="plan-unit-edge" data-needed={edge.needed} data-state={edge.state.kind}>
      {!drawn && (
        <span className="truncate text-xs text-muted-foreground" title={sentence}>
          {`Needs ${formatEdgeNeeded(edge)} · ${formatEdgeState(edge)}`}
        </span>
      )}
      <ActionButtons items={items} busy={busy} />
    </div>
  )
}

/** The actions as small outline buttons, in the order of the builder; a running one of the surface disables them */
function ActionButtons({ items, busy }: { items: readonly MenuItem[]; busy: string | null }) {
  if (items.length === 0) {
    return null
  }
  return (
    <div className="flex flex-wrap gap-1.5">
      {items.map(item => {
        const Icon = item.icon
        return (
          <Button
            key={`${item.id}:${item.label}`}
            size="xs"
            variant="outline"
            disabled={busy !== null || item.disabled === true}
            onClick={() => item.onSelect(FROM_MENU)}
            data-testid="plan-action"
            data-action={item.id}
          >
            {busy !== null && item.id === "landing-fix" ? <Spinner data-icon="inline-start" /> : Icon !== undefined && <Icon data-icon="inline-start" />}
            {item.label}
          </Button>
        )
      })}
    </div>
  )
}

/**
 * An offer to land two units in one robot: a dashed frame around the pair, its price below as its risks one by one,
 * then the actions of its edge (Land Together, Land … First); Land Together restacks on fresh needs, one undo puts it
 * back, and the plan follows the view
 */
function ProposalFrame({
  proposal,
  edge,
  columnStart,
  columnEnd,
  row,
  faded,
}: {
  proposal: LandTogether
  edge: LandingEdge | null
  columnStart: number
  columnEnd: number
  row: number
  faded: boolean
}) {
  const surface = `landing-plan:${proposal.from}`
  useIndicatorClaim(surface)
  const busy = useSurfaceBusy(surface)
  const risks = describeTogetherPrice(proposal.price, proposal.forks)
  const items = edge === null ? [] : edgeActionItems(edge, [proposal.fixes], describeEdge(edge), edgeRunners(proposal.from, surface, { reveal: false }))
  const span = `${columnStart + 1} / ${columnEnd + 2}`
  return (
    <>
      <div
        aria-hidden
        className={cn("pointer-events-none -m-1.5 self-stretch rounded-xl border border-dashed border-muted-foreground/50 transition-opacity", faded && "opacity-40")}
        style={{ gridColumn: span, gridRow: `${row + 2} / ${row + 4}` }}
        data-testid="plan-proposal-frame"
      />
      <div
        className={cn("relative z-10 flex min-w-0 flex-col gap-1.5 px-1 pt-0.5 pb-1 transition-opacity", faded && "opacity-40")}
        style={{ gridColumn: span, gridRow: row + 3 }}
        data-testid="plan-proposal"
        data-from={proposal.from}
        data-onto={proposal.onto}
      >
        <PriceLine risks={risks} />
        <ActionButtons items={items} busy={busy} />
      </div>
    </>
  )
}

/** The parked branches apart: kept local, they land with nothing; a click shows the row */
function ParkedCard({ names }: { names: readonly string[] }) {
  return (
    <Card className="gap-1.5 px-4 py-3 sm:px-6" data-testid="plan-parked">
      <h2 className="text-xs font-semibold text-foreground">
        {"Parked "}
        <span className="font-normal text-muted-foreground tabular-nums">{names.length}</span>
      </h2>
      <ul className="flex flex-wrap gap-1.5">
        {names.map(name => (
          <li key={name}>
            <ParkedName name={name} />
          </li>
        ))}
      </ul>
    </Card>
  )
}

function ParkedName({ name }: { name: string }): ReactNode {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            className="flex items-center gap-1.5 rounded-md border border-border px-2 py-0.5 text-xs text-muted-foreground outline-none hover:text-foreground focus-visible:bd-focus"
            onClick={() => revealUnit(name, false)}
          />
        }
      >
        <LaneNodeDot kind="parked" />
        {name}
      </TooltipTrigger>
      <TooltipContent>{`Show ${name}`}</TooltipContent>
    </Tooltip>
  )
}
