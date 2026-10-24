import { lazy, Suspense, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { RobotRing } from "@/components/branch/RobotProgress"
import { LaneNodeDot } from "@/components/branch/StackLanes"
import { useLandingOverview } from "@/hooks/useLandingOverview"
import type { LandingOverview } from "@/lib/landing-overview"
import { formatLandingTitle, getLandingBarSegments, isLandingOverviewEmpty, landingBarFill } from "@/lib/landing-overview"
import { useBranchSync } from "@/stores/branch-sync"
import { useSelectedProjectPath, useSelectedRepositoryState } from "@/stores/repository"

const loadLandingPopover = () => import("@/components/repository/LandingPopover")
const LandingPopover = lazy(() => loadLandingPopover().then(it => ({ default: it.LandingPopover })))

export interface LandingIndicatorProps {
  /** The pull request chip of a row of the overview */
  renderPullRequest: (branchName: string) => ReactNode
  /** Below the rows of the overview: the update state of the baseline, the pull requests and Patronus checked */
  footer: ReactNode
}

/**
 * The header button of the work on its way into master ([Landing Overview](../../../spec/repository/landing-overview.spec.md)):
 * the node of the most urgent move (in the ring of a running Safe Push), the moves that are the user's, a bar of the
 * work by where it is on its way. Hover or click opens the overview; hidden while nothing is on the way and nothing ran
 * in the last day. As wide whatever the answers say: the place of the count stays at 0 and the bar shows one kind too,
 * so an answer of Patronus or GitHub moves no control of the header.
 */
export function LandingIndicator({ renderPullRequest, footer }: LandingIndicatorProps) {
  const overview = useLandingOverview()
  const [open, setOpen] = useState(false)
  const empty = isLandingOverviewEmpty(overview)
  const loadingRows = useIsLoadingRows()
  // nothing left to show: the next change must not reopen the popover
  if (empty && open) {
    setOpen(false)
  }
  if (empty) {
    // the rows of a repository synced before are still to come: the button holds its place meanwhile
    return loadingRows ? <LandingIndicatorLoading /> : null
  }
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={
          <Button
            variant="ghost"
            size="sm"
            aria-label={formatLandingTitle(overview)}
            data-testid="landing-indicator"
            data-urgent={overview.urgent ?? undefined}
            // the overview is loaded while the hover delay runs
            onPointerEnter={() => void loadLandingPopover()}
            onFocus={() => void loadLandingPopover()}
          />
        }
      >
        <span aria-hidden className="relative flex size-4 items-center justify-center" data-testid="landing-indicator-node">
          {overview.safePushRuns && <RobotRing className="absolute inset-0 size-4 text-primary" />}
          <LaneNodeDot kind={overview.urgent ?? "waiting"} />
        </span>
        {overview.moves > 0 ? (
          <span className="tabular-nums" data-testid="landing-moves">
            {overview.moves}
          </span>
        ) : (
          // the place of one digit: the first move that comes moves nothing
          <span aria-hidden className="invisible tabular-nums">
            0
          </span>
        )}
        <LandingBar overview={overview} />
      </PopoverTrigger>
      {/*
        at most the height left below the button, as a dropdown: the sections scroll, the title and the footer stay; it
        never moves beside the button, where a tall overview would go
      */}
      <PopoverContent
        align="end"
        collisionAvoidance={{ fallbackAxisSide: "none" }}
        className="max-h-(--available-height) w-auto max-w-2xl min-w-96 gap-3 p-3"
        data-testid="landing-popover"
      >
        <Suspense fallback={null}>
          <LandingPopover overview={overview} onClose={() => setOpen(false)} renderPullRequest={renderPullRequest} footer={footer} />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}

/**
 * The sync of a repository that had branches the last time hasn't given the rows yet (the table loads where it will be,
 * `BranchTableLoading`): the header button will come
 */
function useIsLoadingRows(): boolean {
  const selected = useSelectedProjectPath()
  const lastBranchCount = useSelectedRepositoryState("lastBranchCount") ?? 0
  return useBranchSync(state => state.branchOrder.length === 0 && state.syncedRepositoryPath !== selected) && lastBranchCount > 0
}

/** The button while the rows load: as wide as it will be, nothing to open yet */
function LandingIndicatorLoading() {
  return (
    <Button variant="ghost" size="sm" disabled aria-busy="true" aria-label="On the way to master: checking…" data-testid="landing-indicator-loading">
      <span aria-hidden className="relative flex size-4 items-center justify-center">
        <LaneNodeDot kind="waiting" />
      </span>
      <span aria-hidden className="invisible tabular-nums">
        0
      </span>
      <span aria-hidden className="h-1.5 w-16" />
    </Button>
  )
}

/** A segment per kind of the work on its way, its width the commits; one kind fills it, nothing on the way leaves it empty */
function LandingBar({ overview }: { overview: LandingOverview }) {
  const segments = getLandingBarSegments(overview.onTheWay)
  return (
    <span aria-hidden className="flex h-1.5 w-16 gap-0.5 overflow-hidden" data-testid="landing-bar">
      {segments.map(segment => (
        <span
          key={segment.kind}
          data-kind={segment.kind}
          className={cn("h-full min-w-0.5 basis-0 rounded-sm", landingBarFill(segment.kind))}
          style={{ flexGrow: segment.weight }}
        />
      ))}
    </span>
  )
}
