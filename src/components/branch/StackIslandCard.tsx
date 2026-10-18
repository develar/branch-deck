import { lazy, Suspense } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { FileDiffIcon, FoldVerticalIcon, ShieldCheckIcon, UnfoldVerticalIcon } from "lucide-react"
import { useShallow } from "zustand/react/shallow"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CompactPushLabel, PushProgressDescription } from "@/components/branch/BranchPushButton"
import { NextStepSlot } from "@/components/branch/NextStepSlot"
import { RobotRing } from "@/components/branch/RobotProgress"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { RowActions } from "@/components/shared/RowActions"
import { getPushWithChildrenItem, getSafePushBlocker } from "@/hooks/useBranchContextActions"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import { useStackStateSources } from "@/hooks/useStackStateSources"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { formatBranchCount, formatBranchesAndCommits } from "@/lib/branch"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReviewWindow } from "@/lib/branch-review-window"
import { pushWithChildren } from "@/lib/git/push"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { formatPushMode } from "@/lib/patronus"
import { dropsRemoteCommits } from "@/lib/push-summary"
import { formatStackPushLine } from "@/lib/push-up-to"
import { formatRobotProgress } from "@/lib/robot-eta"
import { formatPushWithChildrenLabel, needsPush, offersPushWithChildren, stackChildrenPushOrder } from "@/lib/stack"
import type { StackIsland } from "@/lib/stack-collapse"
import type { StackAttention, StackState, StackStateChip } from "@/lib/stack-state"
import { firstBranchOfEachState, getStackStateChips, summarizeStackState } from "@/lib/stack-state"
import { joinNames } from "@/lib/text"
import { transferLabel } from "@/lib/transfer-progress"
import { ghostButtonColorClass } from "@/lib/ui-types"
import { getBranch, stackDescendantsIn, stackMainLineIn, useBranch, useBranchPushProgress, useBranchSync, useStackPushCurrent } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { openInline, useStackHeaderInline } from "@/stores/inline-row-action"
import { isPatronusAvailable, useBranchRobot } from "@/stores/patronus"
import { useRepositoryPullRequestMap } from "@/stores/pull-requests"
import { getReadinessTarget } from "@/stores/review-readiness"
import { toggleStackCollapsed } from "@/stores/stack-collapse"

const InlinePatronusStart = lazy(() => import("@/components/branch/InlinePatronusStart").then(it => ({ default: it.InlinePatronusStart })))

interface IslandSummary {
  branchCount: number
  commitCount: number
  state: StackState
  /** The first branch of the stack in each state: where a click on its chip goes */
  first: Partial<Record<keyof StackState, string>>
  /** What a "Push with Children" of the root pushes, parents first, without the branches a Safe Push holds */
  pushOrder: string[]
  /** The commits that push sends */
  pushCommits: number
  /** The branches with work whose push drops remote commits: they ask first in their rows */
  askFirst: string[]
  /** A Safe Push runs in the stack: every one holds the root (`PushHold`), and the header offers no push nor Safe Push */
  rootHeld: boolean
  /** The main line, root first (`stackMainLineIn`): its last branch is the Safe Push of the header */
  mainLine: string[]
  /** The branches of the stack, the root first, parents before children */
  members: string[]
}

/**
 * The size of the stack on `root` and what its branches need, each counted as the header and the filter chips count it
 * (`summarizeStackState`), over the whole stack: a filter that shows a part of it doesn't change what the stack holds
 */
function useIslandSummary(root: string): IslandSummary | null {
  const sources = useStackStateSources()
  const { isLanded, isHeld } = sources
  // a string, so the store notifies only when the summary changes
  const key = useBranchSync(state => {
    const descendants = stackDescendantsIn(state, root)
    const branches = [root, ...descendants].flatMap(name => state.branches[name] ?? [])
    if (state.branches[root] === undefined) {
      return ""
    }
    // the push counts as the "to push" chip: what the rows offer, the landed branches left out
    const pushOrder = stackChildrenPushOrder(
      root,
      descendants,
      name => state.branches[name],
      name => isHeld(name) || isLanded(name),
    )
    return JSON.stringify({
      branchCount: branches.length,
      commitCount: branches.reduce((sum, branch) => sum + branch.commitCount, 0),
      state: summarizeStackState(branches, sources),
      first: firstBranchOfEachState(branches, sources),
      pushOrder,
      pushCommits: pushOrder.reduce((sum, name) => sum + (state.branches[name]?.remoteStatus?.commitsAhead ?? 0), 0),
      askFirst: branches
        .filter(
          branch =>
            branch.name !== root && branch.remoteStatus != null && needsPush(branch) && dropsRemoteCommits(branch.remoteStatus) && !isHeld(branch.name) && !isLanded(branch.name),
        )
        .map(branch => branch.name),
      rootHeld: isHeld(root),
      mainLine: stackMainLineIn(state, root),
      members: branches.map(branch => branch.name),
    } satisfies IslandSummary)
  })
  return key === "" ? null : (JSON.parse(key) as IslandSummary)
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
 * running Safe Push lands, then the actions of a branch row for the whole stack: the push ("↑ 41", Push with Children of
 * the root), the Safe Push of the main line (its last branch) and "⋯"; none of the two while a Safe Push runs in the
 * stack. The form of that Safe Push opens under the header, collapsed or not. Then the rows of the stack with their
 * commits on the lanes. It collapses into its header (`collapsedStacks`, as a stack of the table into its root).
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
      onOpenChange={next => {
        // under a filter the island shows the matches of the stack
        if (!filtered) {
          toggleStackCollapsed(root, !next)
        }
      }}
      header={<IslandHeader root={root} summary={summary} landing={landing} />}
      below={<StackHeaderForm leaf={summary.mainLine.at(-1)} />}
      actions={
        <>
          <IslandStateChips summary={summary} />
          {/* the header has the card's wider padding: its actions stand where the Actions cell of the rows ends */}
          <div className="sm:-mr-2" data-testid="stack-island-actions">
            <RowActions>
              <StackPushSlot root={root} summary={summary} />
              <StackSafePushSlot summary={summary} />
              <MoreActionsMenu name={`the ${root} stack`} menu={<DropdownMenuItems groups={getStackIslandMenuItems(root, summary.mainLine, island.collapsed)} />} />
            </RowActions>
          </div>
        </>
      }
    >
      {children}
    </CollapsibleCard>
  )
}

function IslandHeader({ root, summary, landing }: { root: string; summary: IslandSummary; landing: ReadonlyMap<string, string> }) {
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
        <h2 className="truncate text-sm font-semibold text-foreground" data-testid="stack-island-root">
          {root}
        </h2>
        <span className="shrink-0 text-sm text-muted-foreground tabular-nums" data-testid="stack-island-size">
          {`Stack · ${formatBranchesAndCommits(summary.branchCount, summary.commitCount)}`}
        </span>
      </div>
      <div className="flex min-w-0 flex-wrap items-center gap-x-2 text-xs text-muted-foreground empty:hidden">
        <PullRequestSummary members={summary.members} />
        {[...landed].map(([branch, count]) => (
          <LandingLine key={branch} branch={branch} count={count} />
        ))}
      </div>
    </div>
  )
}

/** "4 pull requests open · 1 approved · 1 changes requested" of the open pull requests of the stack's branches */
function PullRequestSummary({ members }: { members: readonly string[] }) {
  const pullRequests = useRepositoryPullRequestMap()
  const open = members.flatMap(name => (pullRequests[name]?.state === "open" ? [pullRequests[name]] : []))
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

/**
 * "Safe Push of oauth lands 4 branches · ~23m left": a running robot lands a part of the stack, how long it takes; no
 * ring, the "1 running" chip beside the name has it
 */
function LandingLine({ branch, count }: { branch: string; count: number }) {
  const robot = useBranchRobot(branch)
  if (robot === undefined) {
    return null
  }
  return <RunningLanding branch={branch} count={count} robot={robot} />
}

function RunningLanding({ branch, count, robot }: { branch: string; count: number; robot: PatronusRobot }) {
  const progress = formatRobotProgress(useRobotTiming(robot, true))
  return (
    <span data-testid="stack-island-landing">
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
      {chips.map(chip => (
        <StateChip key={chip.state} chip={chip} branch={summary.first[chip.state]} />
      ))}
    </div>
  )
}

function StateChip({ chip, branch }: { chip: StackStateChip; branch: string | undefined }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
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
        }
      >
        {chip.ring && <RobotRing className="size-3" />}
        {chip.text}
      </TooltipTrigger>
      <TooltipContent>{branch === undefined ? chip.text : `Show ${branch}`}</TooltipContent>
    </Tooltip>
  )
}

/**
 * The push of the stack, as the compact push of a row: "↑ 41", the commits of Push with Children of the root (the
 * branches of the stack that have work for the remote, parents first, `stackChildrenPushOrder`); the hover names it and
 * lists what it sends, and the branches that ask first in their rows. Its place stays empty while it would push only the
 * root (the root row's push is that push) and while a Safe Push runs in the stack: a push won't land.
 */
function StackPushSlot({ root, summary }: { root: string; summary: IslandSummary }) {
  const current = useStackPushCurrent(root)
  const progress = useBranchPushProgress(current ?? root)
  const { pushOrder, askFirst } = summary
  if (current === null && (summary.rootHeld || !offersPushWithChildren(root, pushOrder))) {
    return <span aria-hidden className="w-16 shrink-0" data-testid="stack-island-push-placeholder" />
  }
  const label = current === null ? formatPushWithChildrenLabel(pushOrder) : transferLabel(`Pushing ${current}`, progress)
  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={
          <Button
            size="xs"
            variant="ghost"
            className={cn("min-w-16 justify-start tabular-nums", ghostButtonColorClass("primary"))}
            disabled={current !== null}
            focusableWhenDisabled
            aria-busy={current !== null || undefined}
            aria-label={label}
            data-testid="stack-island-push"
            onClick={event => {
              // the header toggles the island on a click
              event.stopPropagation()
              void pushWithChildren(root)
            }}
          />
        }
      >
        <CompactPushLabel isPushing={current !== null} percent={progress?.percent ?? null} count={summary.pushCommits} />
      </PopoverTrigger>
      <PopoverContent data-testid="stack-island-push-description" className="w-auto max-w-sm gap-2 p-3 text-sm leading-snug" onClick={event => event.stopPropagation()}>
        {current === null ? (
          <>
            <div className="font-medium">{label}</div>
            <StackPushLines order={pushOrder} />
            {askFirst.length > 0 && (
              <div className="text-muted-foreground">{`${joinNames(askFirst)} ${askFirst.length === 1 ? "asks" : "ask"} first in ${askFirst.length === 1 ? "its row" : "their rows"}: the push drops remote commits.`}</div>
            )}
          </>
        ) : (
          <PushProgressDescription branchName={current} progress={progress} />
        )}
      </PopoverContent>
    </Popover>
  )
}

/** "Pushes 3 branches, parents first:", a line per push in the words of a push (`formatStackPushLine`) */
function StackPushLines({ order }: { order: readonly string[] }) {
  const lines = useBranchSync(useShallow(state => order.flatMap(name => (state.branches[name] === undefined ? [] : [formatStackPushLine(state.branches[name])]))))
  return <div className="break-words whitespace-pre-line">{`Pushes ${formatBranchCount(lines.length)}, parents first:\n${lines.join("\n")}`}</div>
}

/**
 * "Safe Push deploy · lands 5 of 7 branches into master · side-fix and side-test stay: not on the main line": whose
 * Safe Push it is, what its one robot lands, and the side branches it doesn't land
 */
function describeStackSafePush(leaf: string, mainLine: readonly string[], members: readonly string[]): string {
  const rest = members.filter(name => !mainLine.includes(name))
  const count = rest.length === 0 ? formatBranchCount(mainLine.length) : `${mainLine.length} of ${formatBranchCount(members.length)}`
  const lands = `Safe Push ${leaf} · lands ${count} into ${getReadinessTarget()}`
  return rest.length === 0 ? lands : `${lands} · ${joinNames(rest)} ${rest.length === 1 ? "stays" : "stay"}: not on the main line`
}

/**
 * The Safe Push of the stack: the Safe Push slot of the last branch of its main line (`stackMainLineIn`), whose one robot
 * lands the whole line; its form opens under the header (`openStackSafePush`). The hover says whose it is, what it
 * lands and which side branches stay. Empty while a Safe Push runs in the stack: the landing line and "1 running" say it.
 */
function StackSafePushSlot({ summary }: { summary: IslandSummary }) {
  const leaf = summary.mainLine.at(-1)
  const branch = useBranch(leaf ?? "")
  if (summary.rootHeld || leaf === undefined || branch === undefined) {
    return <div className="w-40" />
  }
  const description = describeStackSafePush(leaf, summary.mainLine, summary.members)
  return <NextStepSlot branch={branch} stackHeader={{ description, onOpen: () => openStackSafePush(leaf) }} />
}

/**
 * The Safe Push form of the last branch of the main line, under the header of the island (`stack-header`): it shows
 * there also while the island is collapsed, so nothing expands and nothing scrolls to the end of the line
 */
function openStackSafePush(leaf: string): void {
  openInline("safe-push", leaf, { placement: "stack-header" })
}

/**
 * The form `openStackSafePush` opened, the one of the last branch of the main line (its key, processing and holds are
 * the branch's); nothing otherwise. Its own border below gives way to the divider of the card.
 */
function StackHeaderForm({ leaf }: { leaf: string | undefined }) {
  const activeInline = useStackHeaderInline(leaf)
  if (leaf === undefined || activeInline?.type !== "safe-push") {
    return null
  }
  return (
    <div className="relative" data-testid="stack-island-form">
      {activeInline.processing && <InlineRowProgressBar />}
      <Suspense fallback={null}>
        <InlinePatronusStart
          key={`safe-push:${leaf}:${activeInline.bypassReview === true ? "bypass" : ""}`}
          branchName={leaf}
          mode="rebaseAutosquash"
          bypassReview={activeInline.bypassReview === true}
          className="border-b-0"
        />
      </Suspense>
    </div>
  )
}

/**
 * The "⋯" of the header, the actions of the whole stack: the Safe Push of the main line, Push with Children of the root
 * (`getPushWithChildrenItem`, disabled with the reason while a Safe Push holds the root), the review of the whole main
 * line, and the collapse
 */
function getStackIslandMenuItems(root: string, mainLine: readonly string[], collapsed: boolean): MenuItemGroups {
  const leaf = mainLine.at(-1) ?? root
  const branch = getBranch(leaf)
  const blocker = branch === undefined ? "The branch is not in the table" : getSafePushBlocker(branch)
  const remote: MenuItem[] = []
  if (isPatronusAvailable()) {
    remote.push({
      id: "safe-push",
      label: `Safe Push ${leaf}…`,
      icon: ShieldCheckIcon,
      disabled: blocker !== null,
      description: blocker ?? `Lands the main line, ${formatBranchCount(mainLine.length)}, into ${getReadinessTarget()} in one Safe Push`,
      onSelect: () => openStackSafePush(leaf),
    })
  }
  const push = getPushWithChildrenItem(root)
  if (push !== null) {
    remote.push(push)
  }
  // as the branch menu's: a leaf without commits, failed or blocked has nothing to review
  const reviewBlocker = branch === undefined ? "The branch is not in the table" : branchReviewBlocker(branch)
  const review: MenuItem = {
    id: "review-changes",
    label: "Review Changes",
    icon: FileDiffIcon,
    disabled: reviewBlocker !== null,
    description: reviewBlocker ?? `The diff of ${formatBranchCount(mainLine.length)} of the main line, the files of each code owner`,
    onSelect: () => void openBranchReviewWindow(leaf, { range: "all" }),
  }
  const collapse: MenuItem = collapsed
    ? { id: "expand-stack", label: "Expand Stack", icon: UnfoldVerticalIcon, onSelect: () => void toggleStackCollapsed(root, false) }
    : { id: "collapse-stack", label: "Collapse Stack", icon: FoldVerticalIcon, onSelect: () => void toggleStackCollapsed(root, true) }
  return [remote, [review, collapse]].filter(group => group.length > 0)
}
