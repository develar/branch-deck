import type { ReactNode } from "react"
import { cn } from "cn"
import { FoldVerticalIcon, GitPullRequestIcon, UnfoldVerticalIcon } from "lucide-react"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { Button } from "@/components/ui/button"
import { PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ActionTracks } from "@/components/branch/ActionTracks"
import { ColumnTrack } from "@/components/branch/BranchColumns"
import { CompactPushLabel, PushProgressDescription } from "@/components/branch/BranchPushButton"
import { LandingNodeHover } from "@/components/branch/LandingNodeHover"
import { GaplessAnchors, NextStepSlot, WholeStackRequestReviewAnchor } from "@/components/branch/NextStepSlot"
import { ClosePullRequestButton, OrphanedStackPullRequestLine } from "@/components/branch/OrphanedStackPullRequest"
import { RobotRing } from "@/components/branch/RobotProgress"
import { LaneNodeDot } from "@/components/branch/StackLanes"
import { WholeStackSlot } from "@/components/branch/WholeStackSlot"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { HoverDescription } from "@/components/shared/HoverDescription"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { menuSources, useMenuStores } from "@/hooks/useBranchContextActions"
import { useStackLandingNode, useStackNodeBranch } from "@/hooks/useLandingNode"
import { useStackBranchPushAlone } from "@/hooks/usePushButton"
import { useRobotTiming } from "@/hooks/useRobotTiming"
import { requestSafePush } from "@/hooks/useSafePush"
import { useSettledStackLine } from "@/hooks/useSettledLayout"
import { useStackStates } from "@/hooks/useStackStates"
import { getWholeStackMenuItems } from "@/hooks/useWholeStackActions"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { formatBranchCount, formatBranchesAndCommits } from "@/lib/branch"
import { pushWithChildren } from "@/lib/git/stack-push"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { formatPushMode } from "@/lib/patronus"
import { hasKeyModifier } from "@/lib/preview-keys"
import { openPullRequest } from "@/lib/pull-request"
import { formatStackPushLine } from "@/lib/push-up-to"
import { formatRobotProgress } from "@/lib/robot-eta"
import { formatAskFirst, formatPushWithChildrenLabel, stackPushSubject } from "@/lib/stack"
import type { StackIsland } from "@/lib/stack-collapse"
import type { StackStateChip } from "@/lib/stack-state"
import { getStackStateChips } from "@/lib/stack-state"
import { joinNames } from "@/lib/text"
import { transferLabel, transferPercent } from "@/lib/transfer-progress"
import { ghostButtonColorClass, textColorClassName } from "@/lib/ui-types"
import {
  STACK_BRANCH_PUSHING,
  describeOrphanedStackPullRequest,
  formatStackBranchPushLine,
  formatWholeStackApprovals,
  formatWholeStackBranch,
  stackReadinessKey,
} from "@/lib/whole-stack"
import { useRowPart } from "@/model/store"
import { actionBlocker } from "@/presenters/actions"
import { formatPushHoldReason } from "@/presenters/reasons"
import type { ChildrenPushOrder, StackBranch } from "@/stores/branch-sync"
import {
  isForkingRoot,
  mainLineOf,
  pushOrdersIn,
  structureIn,
  useBranch,
  useBranchPushProgress,
  useBranchSync,
  useIsStackBranchPushing,
  useStackBranch,
  useStackBranchPushProgress,
  useStackPush,
} from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { landedStacksStore, useStackLandedBy } from "@/stores/landed-stacks"
import { useBranchRobot, usePushHold, useStackRobot } from "@/stores/patronus"
import { useRepositoryPullRequestMap, useStackPullRequest } from "@/stores/pull-requests"
import { getReadinessTarget } from "@/stores/review-readiness"
import { toggleStackCollapsed } from "@/stores/stack-collapse"

interface IslandSummary {
  branchCount: number
  commitCount: number
  /** What a "Push with Children" of the root pushes, as Rust orders it (`ChildrenPush`); null with nothing stacked on it to push */
  children: ChildrenPushOrder | null
  /** The commits that push sends */
  pushCommits: number
  /**
   * A Safe Push runs in the stack: every one holds the root (`PushHold`), and the header offers no push nor Safe Push. A
   * merged one that landed the root is no such hold: the rest of the stack pushes and lands without it
   */
  rootHeld: boolean
  /** The main line, root first (`mainLineOf`): its last branch is the Safe Push of the header */
  mainLine: string[]
  /** The branches of the stack, the root first, parents before children */
  members: string[]
}

/**
 * The size of the stack on `root` and what its push sends, over the whole stack: a filter that shows a part of it
 * doesn't change what the stack holds; the states of its branches are `useStackStates`
 */
function useIslandSummary(root: string): IslandSummary | null {
  const rootHold = usePushHold(root)
  // a string, so the store notifies only when the summary changes
  const key = useBranchSync(state => {
    const descendants = structureIn(state, root).descendants
    const branches = [root, ...descendants].flatMap(name => state.branches[name] ?? [])
    if (state.branches[root] === undefined) {
      return ""
    }
    // what the rows offer, as Rust orders it: the landed, held and parked branches left out
    const children = pushOrdersIn(state, root).children
    const pushOrder = children?.order ?? []
    return JSON.stringify({
      branchCount: branches.length,
      commitCount: branches.reduce((sum, branch) => sum + branch.commitCount, 0),
      children,
      pushCommits: pushOrder.reduce((sum, name) => sum + (state.branches[name]?.remoteStatus?.commitsAhead ?? 0), 0),
      rootHeld: rootHold !== null && rootHold.kind !== "landed",
      mainLine: mainLineOf(state, root),
      members: branches.map(branch => branch.name),
    } satisfies IslandSummary)
  })
  return key === "" ? null : (JSON.parse(key) as IslandSummary)
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
 * the root), the Safe Push of the whole stack (one robot lands every branch of it) and "⋯"; none of the two while a Safe
 * Push runs in the stack. Its popover opens at the header, collapsed or not; S on the focused header opens it too. Then
 * the rows of the stack, the rows of the table: each shows its commits while expanded. It collapses into its header
 * (`collapsedStacks`, as a stack of the table into its root).
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
      onHeaderKeyDown={event => {
        // S on the focused header: the Safe Push of the whole stack, its popover at the header, Enter starts it
        if (event.code === "KeyS" && !event.repeat && !hasKeyModifier(event)) {
          event.preventDefault()
          void requestSafePush(stackReadinessKey(root), { mode: "rebaseAutosquash", via: "key", placement: "stack-header" })
        }
      }}
      actions={
        <>
          <StackPullRequestButton root={root} />
          <IslandStateChips root={root} />
          {/* the header has the card's wider padding: its actions stand where the Actions cell of the rows ends */}
          <div className="sm:-mr-2">
            {/* in the tracks of the Actions column: the push, the step and "⋯" stand where those of the rows stand */}
            <ActionTracks data-testid="stack-island-actions">
              <ColumnTrack track="push">
                <StackPushSlot root={root} summary={summary} />
              </ColumnTrack>
              <ColumnTrack track="step">
                <StackSafePushSlot root={root} summary={summary} />
              </ColumnTrack>
              <ColumnTrack track="more">
                <MoreActionsMenu name={`the ${root} stack`} menu={<StackIslandMenu root={root} collapsed={island.collapsed} />} />
              </ColumnTrack>
            </ActionTracks>
          </div>
        </>
      }
    >
      {children}
    </CollapsibleCard>
  )
}

/** Where the whole stack is on its way into master, as the node of a row says it of a branch; the hover says it (`LandingNodeHover`) */
function IslandLandingNode({ root }: { root: string }) {
  const node = useStackLandingNode(root)
  // a straight chain shows the node of its last branch, and the robot of that branch
  const leaf = useStackNodeBranch(root)
  if (node === null) {
    return null
  }
  return (
    <LandingNodeHover
      node={node}
      subject={leaf === null ? { kind: "stack", root } : { kind: "branch", name: leaf }}
      trigger={<span className="flex shrink-0" data-testid="stack-island-node" />}
    >
      <LaneNodeDot kind={node.kind} />
    </LandingNodeHover>
  )
}

function IslandHeader({ root, summary, landing }: { root: string; summary: IslandSummary; landing: ReadonlyMap<string, string> }) {
  const { linesRef, reserved } = useSettledStackLine(root)
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
        <IslandLandingNode root={root} />
        <h2 className="truncate text-sm font-semibold text-foreground" data-testid="stack-island-root">
          {root}
        </h2>
        <span className="shrink-0 text-sm text-muted-foreground tabular-nums" data-testid="stack-island-size">
          {`Stack · ${formatBranchesAndCommits(summary.branchCount, summary.commitCount)}`}
        </span>
      </div>
      {/* " · " between the items of the line, as between the parts of every line: each item may render nothing */}
      <div
        ref={linesRef}
        className={cn(
          "flex min-w-0 flex-wrap items-center gap-x-1 text-xs text-muted-foreground *:not-first:before:mr-1 *:not-first:before:text-muted-foreground *:not-first:before:content-['·']",
          reserved ? "min-h-lh" : "empty:hidden",
        )}
        data-testid="stack-island-lines"
        data-reserved={reserved || undefined}
      >
        <WholeStackPullRequestLine root={root} />
        <PullRequestSummary members={summary.members} />
        {[...landed].map(([branch, count]) => (
          <LandingLine key={branch} branch={branch} count={count} />
        ))}
        <WholeStackLandingLine root={root} />
        <OrphanedStackPullRequestLine root={root} />
      </div>
    </div>
  )
}

/**
 * "Approvals count in #12, the pull request of the whole stack" while the pull request of the whole stack is open and
 * matches it (one that no longer does says so in `OrphanedStackPullRequestLine`), with what GitHub says of its review;
 * counted apart from the pull requests of the branches (`PullRequestSummary`)
 */
function WholeStackPullRequestLine({ root }: { root: string }) {
  const pullRequest = useStackPullRequest(root)
  const stack = useStackBranch(root)
  const landedBy = useStackLandedBy(root)
  if (pullRequest?.state !== "open" || describeOrphanedStackPullRequest(root, pullRequest, stack, landedBy) !== null) {
    return null
  }
  const decision = pullRequest.reviewDecision === "approved" ? " · approved" : pullRequest.reviewDecision === "changesRequested" ? " · changes requested" : ""
  return <span data-testid="stack-island-stack-pull-request">{`${formatWholeStackApprovals(pullRequest.number)}${decision}`}</span>
}

/**
 * The pull request of the whole stack the header names, beside the actions (the header lines are inside its toggle):
 * "#12" opens it while it matches the stack, "Close Pull Request #12…" asks to close it once it no longer does
 */
function StackPullRequestButton({ root }: { root: string }) {
  const pullRequest = useStackPullRequest(root)
  const stack = useStackBranch(root)
  const landedBy = useStackLandedBy(root)
  if (pullRequest?.state !== "open") {
    return null
  }
  if (describeOrphanedStackPullRequest(root, pullRequest, stack, landedBy) !== null) {
    return <ClosePullRequestButton root={root} number={pullRequest.number} variant="ghost" className="text-warning" />
  }
  return (
    <Tooltip>
      <TooltipTrigger
        render={<Button variant="ghost" size="xs" className="tabular-nums" data-testid="stack-island-open-pull-request" onClick={() => openPullRequest(pullRequest)} />}
      >
        <GitPullRequestIcon data-icon="inline-start" />
        {`#${pullRequest.number}`}
      </TooltipTrigger>
      <TooltipContent>{`Open Pull Request #${pullRequest.number}`}</TooltipContent>
    </Tooltip>
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
      {progress.etaAlone !== null && ` · ${progress.etaAlone}`}
    </span>
  )
}

/**
 * "Safe Push of the whole stack lands 7 branches · ~23m left": the robot of the whole stack runs (on its stack branch),
 * and how long it takes; the "1 running" chip beside the name has its ring
 */
function WholeStackLandingLine({ root }: { root: string }) {
  const robot = useStackRobot(root)
  const members = useStackBranch(root)?.members.length ?? 0
  if (robot === undefined || members === 0 || robot.status !== "running") {
    return null
  }
  return <RunningWholeStack count={members} robot={robot} />
}

function RunningWholeStack({ count, robot }: { count: number; robot: PatronusRobot }) {
  const progress = formatRobotProgress(useRobotTiming(robot, true))
  return (
    <span data-testid="stack-island-landing">
      {`${formatPushMode(robot.pushMode)} of the whole stack lands ${formatBranchCount(count)}`}
      {progress.etaAlone !== null && ` · ${progress.etaAlone}`}
    </span>
  )
}

/**
 * Where the branches of the stack are on their way into master, by the nodes of their rows and a fix of the whole
 * stack (`useStackStates`), the strongest first in the colours of the nodes; a click shows the first branch in that state
 */
function IslandStateChips({ root }: { root: string }) {
  const states = useStackStates(root, { withRoot: true })
  const chips = states === null ? [] : getStackStateChips(states.state)
  if (states === null || chips.length === 0) {
    return null
  }
  return (
    <div className="hidden items-center gap-1 sm:flex" data-testid="stack-island-states">
      {chips.map(chip => (
        <StateChip key={chip.state} chip={chip} branch={states.first[chip.state]} />
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
            className={cn("tabular-nums", textColorClassName(chip.color))}
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
 * branches of the stack that have work for the remote, parents first, and the stack branch of the whole stack while it
 * goes along, as Rust orders them, `ChildrenPush`), in one push; a commit the branches and the stack branch
 * both send counts once. The hover names it and lists what it sends, and the branches that ask first in their rows; a
 * click pushes and leaves the hover as it is (`HoverDescription`), so it shows the progress while the pointer stays.
 * Disabled, "↑" with the reason in its hover, while it would push nothing beyond the root
 * (Rust's reason, `domain_policy::actions`: the stack is on the remote, the root row's push is the push, the branches with
 * work ask first, or only the stack branch has work and doesn't go along) and while a Safe Push runs in the stack: a push
 * won't land. While a push runs, the spinner and the percent, its name ("Pushing 7 branches 45%"; "Pushing the whole
 * stack 45%" for the stack branch alone) and the line of git on hover: the one indicator of the pushes of the stack; the
 * start of the Safe Push of the whole stack shows its push in its own button, this one disabled meanwhile.
 */
/** Why the push of a stack without branches to land is disabled: Rust offers no Push with Children of its root */
const NOTHING_TO_PUSH = "Nothing to push: the stack is on the remote"

function StackPushSlot({ root, summary }: { root: string; summary: IslandSummary }) {
  const stackPush = useStackPush(root)
  const current = stackPush === null ? null : stackPushSubject(root, stackPush)
  // the push of the stack branch alone (a step of the restart of its robot, of Request Review…, of the creation of its
  // pull request) shows here as the stack's pushes do; the Safe Push of the whole stack shows it in its own button, the
  // step of its start: one indicator, decided where the root's push button decides it
  const stackBranchPushing = useIsStackBranchPushing(root)
  const pushAlone = useStackBranchPushAlone(root)
  const alone = stackPush === null && pushAlone
  const busy = current !== null || alone
  // the one push of the stack reports its progress for each of its branches
  const branchProgress = useBranchPushProgress(stackPush?.[0] ?? root)
  const stackBranchProgress = useStackBranchPushProgress(root)
  const progress = alone ? stackBranchProgress : branchProgress
  const hold = usePushHold(root)
  // Push with Children of the root as Rust offers it (`domain_policy::actions`)
  const rootActions = useRowPart(root, "actions")
  const stack = useStackBranch(root)
  const { children } = summary
  const withStackBranch = children?.stackBranch?.kind === "along" && stack !== undefined
  const pushOrder = children?.order ?? []
  const askFirst = children?.askFirst ?? []
  let blocker: string | null = null
  if (!busy) {
    if (summary.rootHeld) {
      blocker = hold === null || hold.kind === "landed" ? "A Safe Push runs in the stack" : formatPushHoldReason(root, hold)
    } else {
      blocker = actionBlocker(rootActions, "push-with-children", { subject: root }) ?? (children?.offers === true ? null : NOTHING_TO_PUSH)
    }
  }
  let label = "Push with Children"
  if (current !== null) {
    label = transferLabel(`Pushing ${current}`, progress)
  } else if (alone) {
    label = transferLabel(STACK_BRANCH_PUSHING, progress)
  } else if (blocker === null) {
    label = formatPushWithChildrenLabel(pushOrder, withStackBranch)
  }
  // the stack branch holds the commits of the branches: what the remote lacks of both, each once
  const pushCommits = withStackBranch ? Math.max(summary.pushCommits, stack.remoteStatus?.commitsAhead ?? 0) : summary.pushCommits
  return (
    <HoverDescription>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={
          <Button
            size="xs"
            variant="ghost"
            className={cn("min-w-16 justify-start tabular-nums", ghostButtonColorClass(blocker === null ? "primary" : "neutral"), !busy && "aria-disabled:opacity-50")}
            // a push of the stack branch runs: Push with Children would take it along
            disabled={busy || blocker !== null || stackBranchPushing}
            // the reason stays reachable on hover while the button is disabled
            focusableWhenDisabled
            aria-busy={busy || undefined}
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
        <CompactPushLabel isPushing={busy} percent={transferPercent(progress)} count={blocker === null ? pushCommits : 0} />
      </PopoverTrigger>
      <PopoverContent data-testid="stack-island-push-description" className="w-auto max-w-sm gap-2 p-3 text-sm leading-snug" onClick={event => event.stopPropagation()}>
        {busy && <PushProgressDescription subject={current ?? formatWholeStackBranch(root)} progress={progress} />}
        {!busy && blocker !== null && (
          <>
            <div className="font-medium">{label}</div>
            <div className="break-words">{blocker}</div>
          </>
        )}
        {!busy && blocker === null && (
          <>
            <div className="font-medium">{label}</div>
            <StackPushLines order={pushOrder} stack={withStackBranch ? stack : null} />
            {askFirst.length > 0 && <div className="text-muted-foreground">{`${formatAskFirst(askFirst)}.`}</div>}
          </>
        )}
      </PopoverContent>
    </HoverDescription>
  )
}

/**
 * "Pushes 3 branches, parents first:", a line per push in the words of a push (`formatStackPushLine`), the stack branch of
 * the whole stack last (`formatStackBranchPushLine`)
 */
function StackPushLines({ order, stack }: { order: readonly string[]; stack: StackBranch | null }) {
  const branches = useBranchSync(useShallow(state => order.flatMap(name => (state.branches[name] === undefined ? [] : [formatStackPushLine(state.branches[name])]))))
  const lines = stack === null ? branches : [...branches, formatStackBranchPushLine(stack)]
  const text = lines.length === 1 ? `Pushes ${lines[0]}` : `Pushes ${formatBranchCount(lines.length)}, parents first:\n${lines.join("\n")}`
  return <div className="break-words whitespace-pre-line">{text}</div>
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
 * The Safe Push of the stack, the whole stack in one robot: of a stack that forks, the slot of its stack branch
 * (`WholeStackSlot`); of a straight chain, which has none, the slot of the last branch of its main line
 * (`mainLineOf`), whose one robot lands it all, its hover saying whose it is and what it lands. Its popover opens
 * at the header's slot (placement `stack-header`). Empty while a Safe Push runs in the stack: the landing line and "1
 * running" say it.
 */
function StackSafePushSlot({ root, summary }: { root: string; summary: IslandSummary }) {
  const forks = useBranchSync(state => isForkingRoot(state, root))
  const leaf = summary.mainLine.at(-1)
  const branch = useBranch(leaf ?? "")
  if (summary.rootHeld || leaf === undefined || branch === undefined) {
    // the Request Review… of the whole stack still opens here, from the "⋯"
    return (
      <GaplessAnchors>
        <WholeStackRequestReviewAnchor root={root} />
      </GaplessAnchors>
    )
  }
  if (forks) {
    return <WholeStackSlot root={root} leaf={leaf} />
  }
  const description = describeStackSafePush(leaf, summary.mainLine, summary.members)
  return <NextStepSlot branch={branch} stackHeader={{ description }} />
}

/** The items of the "⋯" of the header, built when it opens: Patronus, GitHub and the robots answer after the first render */
/** The "⋯" of the header: an open menu follows every store it reads, the pull requests that load after the sync too */
function StackIslandMenu({ root, collapsed }: { root: string; collapsed: boolean }) {
  useStore(landedStacksStore, () => getStackIslandMenuNow(root, collapsed))
  return <DropdownMenuItems groups={useMenuStores(() => getStackIslandMenuNow(root, collapsed))} />
}

/** By root: the menu of the header built last, and the store states it was built from (`getStackIslandMenuNow`) */
const shownIslandMenus = new Map<string, { sources: readonly unknown[]; groups: MenuItemGroups }>()

/**
 * The menu of the header as the stores are now (`getStackIslandMenuItems`), one object while the stores it reads stay: the
 * selector of `StackIslandMenu`, as `getBranchMenuNow` is of the branch menu
 */
function getStackIslandMenuNow(root: string, collapsed: boolean): MenuItemGroups {
  const sources = [...menuSources(), landedStacksStore.getState(), collapsed]
  const shown = shownIslandMenus.get(root)
  if (shown !== undefined && shown.sources.length === sources.length && shown.sources.every((source, index) => source === sources[index])) {
    return shown.groups
  }
  const groups = getStackIslandMenuItems(root, collapsed)
  shownIslandMenus.set(root, { sources, groups })
  return groups
}

/** The items of the first group of the "⋯" of the header: the starts of the whole stack, and the piece of its main line */
const ISLAND_START_ITEMS: ReadonlySet<string> = new Set(["safe-push-whole-stack", "dry-run-whole-stack", "safe-push-main-line"])

/**
 * The "⋯" of the header: the actions of the whole stack (`getWholeStackMenuItems`): its Safe Push and Dry Run (with the
 * Safe Push of the main line while the stack lands in pieces), then its pull request, Push with Children of the root,
 * Review Whole Stack, its robot; and the collapse
 */
function getStackIslandMenuItems(root: string, collapsed: boolean): MenuItemGroups {
  const items = getWholeStackMenuItems(root, { withPush: true, landInPieces: true })
  const isStart = (item: MenuItem) => item.id !== undefined && ISLAND_START_ITEMS.has(item.id)
  const collapse: MenuItem = collapsed
    ? { id: "expand-stack", label: "Expand Stack", icon: UnfoldVerticalIcon, onSelect: () => void toggleStackCollapsed(root, false) }
    : { id: "collapse-stack", label: "Collapse Stack", icon: FoldVerticalIcon, onSelect: () => void toggleStackCollapsed(root, true) }
  return [items.filter(isStart), items.filter(item => !isStart(item)), [collapse]].filter(group => group.length > 0)
}
