import type { ReactNode } from "react"
import { UploadIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RobotRing } from "@/components/branch/RobotProgress"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { useIsFullyLanded } from "@/hooks/useLandedIn"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import { formatBranchCount, formatBranchesAndCommits } from "@/lib/branch"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { pushWithChildren } from "@/lib/git/push"
import { formatPushMode } from "@/lib/patronus"
import { formatRobotProgress } from "@/lib/robot-eta"
import { formatPushWithChildrenLabel, stackChildrenPushOrder } from "@/lib/stack"
import type { StackIsland } from "@/lib/stack-collapse"
import type { StackAttention, StackState, StackStateChip } from "@/lib/stack-state"
import { firstBranchOfEachState, getStackStateChips, summarizeStackState } from "@/lib/stack-state"
import { stackDescendantsIn, useBranchSync, useStackPushCurrent } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { useBranchRobot, useFailingRobots, useRepositoryRobots, useRobotsNeedingReview } from "@/stores/patronus"
import { useRepositoryPullRequestMap } from "@/stores/pull-requests"
import { toggleStackCollapsed } from "@/stores/stack-collapse"

interface IslandSummary {
  branchCount: number
  commitCount: number
  state: StackState
  /** The first branch of the stack in each state: where a click on its chip goes */
  first: Partial<Record<keyof StackState, string>>
  /** What a "Push with Children" of the root pushes, parents first */
  pushOrder: string[]
  /** The branches of the stack, the root first, parents before children */
  members: string[]
}

/**
 * The size of the stack on `root` and what its branches need, each counted as the header and the filter chips count it
 * (`summarizeStackState`), over the whole stack: a filter that shows a part of it doesn't change what the stack holds
 */
function useIslandSummary(root: string): IslandSummary | null {
  const robots = useRepositoryRobots()
  const needsReview = useRobotsNeedingReview()
  const failing = useFailingRobots()
  const isLanded = useIsFullyLanded()
  // a string, so the store notifies only when the summary changes
  const key = useBranchSync((state) => {
    const descendants = stackDescendantsIn(state, root)
    const branches = [root, ...descendants].flatMap(name => state.branches[name] ?? [])
    if (state.branches[root] === undefined) {
      return ""
    }
    const sources = { robots, needsReview, failing, isLanded }
    return JSON.stringify({
      branchCount: branches.length,
      commitCount: branches.reduce((sum, branch) => sum + branch.commitCount, 0),
      state: summarizeStackState(branches, sources),
      first: firstBranchOfEachState(branches, sources),
      pushOrder: stackChildrenPushOrder(root, descendants, name => state.branches[name]),
      members: branches.map(branch => branch.name),
    } satisfies IslandSummary)
  })
  return key === "" ? null : JSON.parse(key) as IslandSummary
}

const chipClassNames: Readonly<Record<StackAttention["color"], string>> = {
  error: "text-destructive",
  warning: "text-warning",
  primary: "text-primary",
}

export interface StackIslandCardProps {
  island: StackIsland
  /** A filter shows its matches in their stacks: the island doesn't collapse */
  filtered: boolean
  /** The landing spans of the running robots, ancestor → the branch whose robot lands it (`useLandingSpans`) */
  landing: ReadonlyMap<string, string>
  /** The table of the stack's rows */
  children: ReactNode
}

/**
 * A big stack (`stackIslandRoots`) as an island of its own below the branch table: a header that sizes the stack and
 * says what it holds, each state a link to its first branch ("1 failed", "23 to push"), its pull requests and what a
 * running Safe Push lands, and the push of the whole stack ("Push with Children" of the root); then the rows of the
 * stack with their commits on the lanes. It collapses into its header (`collapsedStacks`, as a stack of the table into
 * its root).
 */
export function StackIslandCard({ island, filtered, landing, children }: StackIslandCardProps) {
  const { root } = island
  const summary = useIslandSummary(root)
  if (summary === null) {
    return null
  }
  const open = !island.collapsed
  return (
    <CollapsibleCard
      data-testid="stack-island"
      data-stack-root={root}
      open={open}
      onOpenChange={(next) => {
        // under a filter the island shows the matches of the stack
        if (!filtered) {
          toggleStackCollapsed(root, !next)
        }
      }}
      header={<IslandHeader root={root} summary={summary} landing={landing} />}
      actions={(
        <>
          <IslandStateChips summary={summary} />
          <PushStackButton root={root} pushOrder={summary.pushOrder} />
        </>
      )}
    >
      {children}
    </CollapsibleCard>
  )
}

function IslandHeader({ root, summary, landing }: { root: string, summary: IslandSummary, landing: ReadonlyMap<string, string> }) {
  // the branches whose robot lands a part of the stack, with the count of the branches it lands, itself included
  const landed = new Map<string, number>()
  for (const name of summary.members) {
    const by = landing.get(name)
    if (by !== undefined) {
      landed.set(by, (landed.get(by) ?? 1) + 1)
    }
  }
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <div className="flex min-w-0 items-center gap-2">
        <h2 className="truncate text-sm font-semibold text-foreground" data-testid="stack-island-root">{root}</h2>
        <span className="shrink-0 text-sm text-muted-foreground tabular-nums" data-testid="stack-island-size">
          {`Stack · ${formatBranchesAndCommits(summary.branchCount, summary.commitCount)}`}
        </span>
      </div>
      <div className="flex min-w-0 flex-wrap items-center gap-x-2 text-xs text-muted-foreground empty:hidden">
        <PullRequestSummary members={summary.members} />
        {[...landed].map(([branch, count]) => <LandingLine key={branch} branch={branch} count={count} />)}
      </div>
    </div>
  )
}

/** "4 pull requests open · 1 approved · 1 changes requested" of the open pull requests of the stack's branches */
function PullRequestSummary({ members }: { members: readonly string[] }) {
  const pullRequests = useRepositoryPullRequestMap()
  const open = members.flatMap(name => pullRequests[name]?.state === "open" ? [pullRequests[name]] : [])
  if (open.length === 0) {
    return null
  }
  const count = (decision: PullRequest["reviewDecision"]) => open.filter(it => it.reviewDecision === decision).length
  const parts = [`${open.length} ${open.length === 1 ? "pull request" : "pull requests"} open`]
  if (count("approved") > 0) {
    parts.push(`${count("approved")} approved`)
  }
  if (count("changesRequested") > 0) {
    parts.push(`${count("changesRequested")} changes requested`)
  }
  return <span data-testid="stack-island-pull-requests">{parts.join(" · ")}</span>
}

/** "Safe Push of oauth lands 4 branches · ~23m left": a running robot lands a part of the stack, how long it takes */
function LandingLine({ branch, count }: { branch: string, count: number }) {
  const robot = useBranchRobot(branch)
  if (robot === undefined) {
    return null
  }
  return <RunningLanding branch={branch} count={count} robot={robot} />
}

function RunningLanding({ branch, count, robot }: { branch: string, count: number, robot: PatronusRobot }) {
  const progress = formatRobotProgress(useRobotTiming(robot, true))
  return (
    <span className="inline-flex items-center gap-1" data-testid="stack-island-landing">
      <RobotRing className="size-3 text-primary" />
      {`${formatPushMode(robot.pushMode)} of ${branch} lands ${formatBranchCount(count)}`}
      {progress.eta !== null && ` · ${progress.eta}`}
    </span>
  )
}

/** The states the stack holds, the strongest first; a click shows the first branch in that state */
function IslandStateChips({ summary }: { summary: IslandSummary }) {
  const chips = getStackStateChips(summary.state)
  if (chips.length === 0) {
    return null
  }
  return (
    <div className="hidden items-center gap-1 sm:flex" data-testid="stack-island-states">
      {chips.map(chip => <StateChip key={chip.state} chip={chip} branch={summary.first[chip.state]} />)}
    </div>
  )
}

function StateChip({ chip, branch }: { chip: StackStateChip, branch: string | undefined }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <Button
            variant="ghost"
            size="xs"
            className={cn("tabular-nums", chipClassNames[chip.color])}
            disabled={branch === undefined}
            data-state-chip={chip.state}
            onClick={() => {
              if (branch !== undefined) {
                requestReveal({ branchName: branch, commitIds: [] })
              }
            }}
          />
        )}
      >
        {chip.ring && <RobotRing className="size-3" />}
        {chip.text}
      </TooltipTrigger>
      <TooltipContent>{branch === undefined ? chip.text : `Show ${branch}`}</TooltipContent>
    </Tooltip>
  )
}

/**
 * "Push with Children (21 branches)" of the root: the branches of the stack that have work for the remote, parents
 * first, as the item of the root's menu; one whose push drops remote commits asks first in its own row
 */
function PushStackButton({ root, pushOrder }: { root: string, pushOrder: readonly string[] }) {
  const current = useStackPushCurrent(root)
  // the root alone is the push of its row
  if (!pushOrder.some(name => name !== root) && current === null) {
    return null
  }
  return (
    <Button variant="default" size="sm" disabled={current !== null} data-testid="stack-island-push" onClick={() => void pushWithChildren(root)}>
      <UploadIcon data-icon="inline-start" />
      {current === null ? formatPushWithChildrenLabel(pushOrder.length) : `Pushing ${current}`}
    </Button>
  )
}
