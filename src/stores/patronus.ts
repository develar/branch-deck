import { useMemo } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { toast } from "@/components/ui/toast"
import type {
  LandingAllowances,
  LandingRefusal,
  PatronusAvailability,
  PatronusCommandSubject,
  PatronusMergeRequest,
  PatronusPushMode,
  PatronusRobot,
  PatronusRobotStatus,
  PatronusView,
  PullRequestStartBranch,
  RemoteFacts,
  RobotMatch,
  RobotOf,
  FactsChanges,
  FactsView,
  RobotsChange,
  RobotsView,
} from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { commandFailure, getErrorDetails } from "@/lib/errors"
import { storeKeyOf } from "@/lib/git/vcs-request"
import type { VcsRequest } from "@/lib/git/vcs-request"
import { closeLandedPullRequests, describeLandedLeftOpen } from "@/lib/landed-stack"
import { announce, resetOsNotification } from "@/lib/os-notification"
import type { LandedBy, LandingAncestor, LandingDescendant, PushHold } from "@/lib/patronus"
import {
  REVIEW_APPROVAL_SUBSYSTEM,
  findBranchRobot,
  findMergedRobots,
  findStackRobot,
  formatLandedWholeReason,
  formatMergedRobotTitle,
  formatPushMode,
  formatStackPushHoldReason,
  isHoldingRobot,
  isLandedWhole,
  isRobotActive,
  openRobot,
} from "@/lib/patronus"
import { isRemoteUpToDate } from "@/lib/push-summary"
import { withoutKeys } from "@/lib/record"
import type { RobotEstimate } from "@/lib/robot-eta"
import { estimateRobotDuration, robotSample } from "@/lib/robot-eta"
import { stackAncestors } from "@/lib/stack"
import { lastFetchTime } from "@/lib/update-state"
import { watchRobotInDemand } from "@/model/demand"
import { refreshSessionFacts } from "@/model/store"
import { recordRobot } from "@/stores/activity"
import type { BranchSyncState, RemoteStatus } from "@/stores/branch-sync"
import { branchSyncStore, getStackBranch, stackDescendantsIn, useBranchRemoteTip, useBranchSync, useBranchTip, useStackBranch, whenSynced } from "@/stores/branch-sync"
import { refreshPullRequests } from "@/stores/pull-requests"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"
import type { RobotHistoryEntry } from "@/stores/robot-history"
import { historySamples, recordFinishedRobots, robotHistoryStore } from "@/stores/robot-history"
import { getRepositoryState } from "@/stores/settings/repositoryState"
import { runFetchBaseline, runUpdateProjectOf } from "@/stores/update-project"

/**
 * A robot a `/patronus` comment started may have started this long before the comment was posted (the clocks of the
 * app and of Patronus differ)
 */
const COMMENT_ROBOT_SLACK_MS = 120_000
/** After a `/patronus` comment, how often the bot's reaction and the robot are asked for */
export const COMMAND_REPLY_INTERVAL_MS = 3_000
/** How long a `/patronus` comment waits for the bot to start the robot */
export const COMMAND_REPLY_TIMEOUT_MS = 60_000

/**
 * The robots of one repository and branch prefix, as the session of the repository listed and polled them
 * (`facts.robots` of its view, applied by `applyRobotsFacts`), with what the page derives of them
 */
export interface RepositoryPatronus {
  /** null until the first answer */
  readonly availability: PatronusAvailability | null
  /** By robot id: the session's, a robot an action of the page answered until the session's patch has it */
  readonly robots: Readonly<Record<string, PatronusRobot>>
  /** Patronus answered with robots once: what they hold is known from the answer of the view on */
  readonly listed: boolean
  /** `Date.now()` of the last answer, 0 = never */
  readonly fetchedAt: number
  /** The refresh button asked the robots and the session hasn't answered yet */
  readonly isRefreshing: boolean
  /** By robot id, of the failed robots of the branches: failed for missing code-owner approvals */
  readonly needsReview: Readonly<Record<string, boolean>>
  /**
   * `owner/name` of the GitHub repository the branches are mirrored to (the mirror of the committed Space settings, or
   * `branchdeck.githubRepository`), null without
   * one: the branches are reviewed in its pull requests, and a Safe Push of a branch with a pull request is a
   * `/patronus` comment of it
   */
  readonly githubRepository: string | null
  /**
   * By robot id, the robots of no branch the user started from the app (a Safe Push of a reviewed pull request) with
   * what they test ("#101 Rework the session store"): the session polls them while they run (`watchedRobots` of the
   * demand), their merge is told, the header lists them
   */
  readonly watched: Readonly<Record<string, string>>
  /**
   * By robot id, the running robots a failed required check dooms: Patronus warns that they will fail, although they run
   * on (for an hour, maybe). Asked after each answer of a list or a poll (`loadRobotFacts`) until known; a robot stays
   * failing until it finishes.
   */
  readonly failing: Readonly<Record<string, true>>
  /**
   * What the robots land, landed and hold of the branch table of the last sync, as Rust decides it for `bd` too
   * (`get_repository_view`, `askRepositoryView`); null before the first answer
   */
  readonly view: PatronusView | null
  /**
   * The robots (their record) the last answer of the view was asked with: while it is not `robots`, the view lags a
   * change of the robots, which the Dock waits for (`useRepositoryViewCurrent`)
   */
  readonly viewOf: Readonly<Record<string, PatronusRobot>> | null
  /**
   * The view answered (or failed) for the robots of an answered list: what they hold and land is known from then on (a
   * later change of the robots lags a moment, which no row waits for). Before it, the rows wait for Patronus
   * (`isPatronusPending`)
   */
  readonly viewListed: boolean
  /**
   * By robot id, the failed robots whose problems Patronus didn't give (`loadReviewProblems` failed), with why: why they
   * failed stays unknown, so the step of the branch offers no restart (`UnavailableFact`); asked again after the next
   * answer of a list or a poll, and by Check Approvals Again (`askRobotProblemsAgain`)
   */
  readonly problemsFailed: Readonly<Record<string, string>>
  /**
   * The view said that the landing needs of a branch are unknown and Rust computes them now (`waitForLandingNeeds`
   * runs); false once the computation answered, or ended without needs for these inputs (failed, too slow)
   */
  readonly landingNeedsComputing: boolean
  /**
   * Why the last ask of the view failed; null once one answers. Over a last view the steps stand on that one (its holds,
   * landing ancestors and needs stay, stale); without one nothing says what the robots hold (`getPatronusViewFailure`)
   */
  readonly viewFailure: string | null
}

export interface PatronusState {
  /** By `repositoryKey`; in memory only, Patronus is asked again after a restart */
  readonly repositories: Readonly<Record<string, RepositoryPatronus>>
}

export const patronusStore = createStore<PatronusState>(() => ({ repositories: {} }))

const EMPTY: RepositoryPatronus = {
  availability: null,
  robots: {},
  listed: false,
  fetchedAt: 0,
  isRefreshing: false,
  needsReview: {},
  githubRepository: null,
  watched: {},
  failing: {},
  view: null,
  viewOf: null,
  viewListed: false,
  problemsFailed: {},
  landingNeedsComputing: false,
  viewFailure: null,
}

/** The key of a repository and branch prefix in the stores of Patronus answers (the robots, the review readiness) */
export function repositoryKey(repositoryPath: string, branchPrefix: string): string {
  return `${repositoryPath}\n${branchPrefix}`
}

/** The key of the selected repository (`repositoryKey`); null while none is selected */
export function currentKey(): string | null {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const branchPrefix = getEffectiveBranchPrefix()
  return repositoryPath === "" || branchPrefix === "" ? null : repositoryKey(repositoryPath, branchPrefix)
}

// ---- hooks ----

function useRepositoryKey(): string {
  return repositoryKey(useSelectedProjectPath() ?? "", useEffectiveBranchPrefix())
}

/** The robots of the selected repository, for the status line */
export function useRepositoryPatronus(): RepositoryPatronus {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key] ?? EMPTY)
}

/** The robot of the branch (see `findBranchRobot`); re-renders only when it changes */
export function useBranchRobot(branchName: string): PatronusRobot | undefined {
  const key = useRepositoryKey()
  const localTip = useBranchTip(branchName)
  const remoteTip = useBranchRemoteTip(branchName)
  return useStore(patronusStore, state => findBranchRobot(state.repositories[key]?.robots, branchName, localTip, remoteTip))
}

/**
 * The robot of the whole stack of `root` (`findStackRobot`, on its stack branch `{prefix}/stack/<root>`); re-renders
 * only when it changes
 */
export function useStackRobot(root: string): PatronusRobot | undefined {
  const key = useRepositoryKey()
  const stack = useStackBranch(root)
  const localTip = stack?.tip
  const remoteTip = stack?.remoteStatus?.remoteTip ?? null
  return useStore(patronusStore, state => findStackRobot(state.repositories[key]?.robots, root, localTip, remoteTip))
}

/** Non-hook variant of `useStackRobot`, for menus built on open */
export function getStackRobot(root: string): PatronusRobot | undefined {
  const key = currentKey()
  return key === null ? undefined : stackRobotIn(patronusStore.getState().repositories[key]?.robots, root)
}

function stackRobotIn(robots: Readonly<Record<string, PatronusRobot>> | undefined, root: string): PatronusRobot | undefined {
  const stack = getStackBranch(root)
  return findStackRobot(robots, root, stack?.tip, stack?.remoteStatus?.remoteTip ?? null)
}

/**
 * What the robots land, landed and hold of the branches, by branch, as the repository view says it (`view`, the rule
 * `bd` reads too), each fact with its robot object: the same objects while nothing of them changes, so a row that reads
 * one renders again only when it does. A fact whose robot no longer does what the view saw (a hold of a robot that
 * stopped running, a landing of a robot no longer merged) waits for the next view, which the change of the robots asks.
 */
interface PatronusFacts {
  readonly spans: ReadonlyMap<string, LandingDescendant>
  readonly landed: ReadonlyMap<string, LandedBy>
  readonly ancestors: ReadonlyMap<string, LandingAncestor>
  readonly holds: ReadonlyMap<string, PushHold>
  /** By root, the first branch of the stack whose robot lands a part of it */
  readonly members: ReadonlyMap<string, LandingAncestor>
  /** By root, the robot of the whole stack that runs on its stack branch */
  readonly stackHolds: ReadonlyMap<string, PushHold>
}

const NO_FACTS: PatronusFacts = {
  spans: new Map(),
  landed: new Map(),
  ancestors: new Map(),
  holds: new Map(),
  members: new Map(),
  stackHolds: new Map(),
}

let lastFacts: PatronusFacts = NO_FACTS
/** The view and the robots `lastFacts` was computed from */
let lastFactsOf: { readonly view: PatronusView | null; readonly robots: Readonly<Record<string, PatronusRobot>> | null } = { view: null, robots: null }

/** The fields of two facts are the same (their robot the same object, `after` the same commits) */
function sameFact(a: object, b: object): boolean {
  const entries = Object.entries(a)
  return (
    entries.length === Object.keys(b).length &&
    entries.every(([field, value]) => {
      const other = (b as Record<string, unknown>)[field]
      return Array.isArray(value) && Array.isArray(other) ? value.length === other.length && value.every((item, index) => item === other[index]) : value === other
    })
  )
}

/** `next` with the facts equal to those of `previous` kept as the same objects, the same map while all of them are */
function keptFacts<T extends object>(previous: ReadonlyMap<string, T>, next: Map<string, T>): ReadonlyMap<string, T> {
  for (const [name, fact] of next) {
    const before = previous.get(name)
    if (before !== undefined && sameFact(before, fact)) {
      next.set(name, before)
    }
  }
  return previous.size === next.size && [...next].every(([name, fact]) => previous.get(name) === fact) ? previous : next
}

/** The facts of `repository`: computed once per view and robots, kept as `lastFacts` */
function factsOf(repository: RepositoryPatronus | undefined): PatronusFacts {
  const view = repository?.view ?? null
  const robots = repository?.robots ?? EMPTY.robots
  if (view === null) {
    return NO_FACTS
  }
  const last = lastFacts
  if (lastFactsOf.view === view && lastFactsOf.robots === robots) {
    return last
  }
  lastFactsOf = { view, robots }
  const robotOf = (id: string): PatronusRobot | undefined => robots[id]
  const by = (of: RobotOf, robot: PatronusRobot) => ({ branch: of.branch, robot, ...(of.wholeStack && { wholeStack: true }) })
  const spans = new Map<string, LandingDescendant>()
  const landed = new Map<string, LandedBy>()
  const ancestors = new Map<string, LandingAncestor>()
  const holds = new Map<string, PushHold>()
  for (const [name, facts] of Object.entries(view.branches)) {
    const spanRobot = facts.span === null ? undefined : robotOf(facts.span.by.robotId)
    if (facts.span !== null && spanRobot !== undefined) {
      const commits = facts.span.carried?.commits.length
      spans.set(name, { ...by(facts.span.by, spanRobot), ...(commits !== undefined && { commits }) })
    }
    const landedRobot = facts.landedBy === null ? undefined : robotOf(facts.landedBy.by.robotId)
    if (facts.landedBy !== null && landedRobot !== undefined && landedRobot.status === "success") {
      landed.set(name, { ...by(facts.landedBy.by, landedRobot), ...(facts.landedBy.own && { own: true }), after: facts.landedBy.after })
    }
    const ancestorRobot = facts.landingAncestor === null ? undefined : robotOf(facts.landingAncestor.robotId)
    if (facts.landingAncestor !== null && ancestorRobot !== undefined && isRobotActive(ancestorRobot)) {
      ancestors.set(name, by(facts.landingAncestor, ancestorRobot))
    }
    const hold = facts.hold
    const holdRobot = hold === null ? undefined : robotOf(hold.by.robotId)
    if (hold !== null && holdRobot !== undefined && (hold.kind === "landed" ? holdRobot.status === "success" : isHoldingRobot(holdRobot))) {
      const commits = hold.carried?.commits.length
      holds.set(name, { kind: hold.kind, ...by(hold.by, holdRobot), ...(commits !== undefined && { commits }) })
    }
  }
  const members = new Map<string, LandingAncestor>()
  const stackHolds = new Map<string, PushHold>()
  for (const [root, facts] of Object.entries(view.stacks)) {
    const memberRobot = facts.landingMember === null ? undefined : robotOf(facts.landingMember.robotId)
    if (facts.landingMember !== null && memberRobot !== undefined && isRobotActive(memberRobot)) {
      members.set(root, by(facts.landingMember, memberRobot))
    }
    const stackRobot = facts.holdRobotId === null ? undefined : robotOf(facts.holdRobotId)
    if (stackRobot !== undefined && isHoldingRobot(stackRobot)) {
      stackHolds.set(root, { kind: "own", branch: root, robot: stackRobot, wholeStack: true })
    }
  }
  const next: PatronusFacts = {
    spans: keptFacts(last.spans, spans),
    landed: keptFacts(last.landed, landed),
    ancestors: keptFacts(last.ancestors, ancestors),
    holds: keptFacts(last.holds, holds),
    members: keptFacts(last.members, members),
    stackHolds: keptFacts(last.stackHolds, stackHolds),
  }
  // the same facts (another robot changed, a view equal by content): the same object, no hook renders again
  const same = (Object.keys(next) as (keyof PatronusFacts)[]).every(field => next[field] === last[field])
  lastFacts = same ? last : next
  return lastFacts
}

/** The facts of the selected repository; renders again only when the view or the robots change */
function useRepositoryFacts(): PatronusFacts {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => factsOf(state.repositories[key]))
}

/** The facts of the selected repository now, for menus built on open, the next step and the push itself */
function currentFacts(): PatronusFacts {
  return factsOf(currentRepository())
}

/**
 * The first branch of the stack of `root` whose own robot lands a part of it: the robot of the whole stack waits for
 * it; null while none runs. For menus built on open.
 */
export function getStackLandingMember(root: string): LandingAncestor | null {
  return currentFacts().members.get(root) ?? null
}

/**
 * The hold of the stack branch of `root`: its own robot runs and pushes into the target, so a push of the stack branch
 * makes it fail at the end; its restart, which cancels it first, is the way through. Null while none holds it.
 */
export function getStackPushHold(root: string): PushHold | null {
  return currentFacts().stackHolds.get(root) ?? null
}

/**
 * The nearest ancestor of a stacked branch whose robot lands it, or that a running robot of a side branch lands; never
 * the branch's own robot, which its Safe Push replaces: a Safe Push of the branch waits for it; null for a root and while
 * none runs
 */
export function useLandingAncestor(branchName: string): LandingAncestor | null {
  return useRepositoryFacts().ancestors.get(branchName) ?? null
}

/** Non-hook variant of `useLandingAncestor`, for menus built on open */
export function getLandingAncestor(branchName: string): LandingAncestor | null {
  return currentFacts().ancestors.get(branchName) ?? null
}

/**
 * The branch built on this one, or the whole stack, whose running robot lands it: its row says so, and a Safe Push of
 * the branch waits for it; null while none runs
 */
export function useLandingSpan(branchName: string): LandingDescendant | null {
  return useRepositoryFacts().spans.get(branchName) ?? null
}

/**
 * Every landing span of a branch of the selected repository, ancestor → the branch whose running robot lands it, for
 * the stack graph (`landingSegments`); renders again only when a span changes. The robot of the whole stack lands no
 * path of the graph: every branch of the stack says it in its row.
 */
export function useLandingSpans(): ReadonlyMap<string, string> {
  const spans = useRepositoryFacts().spans
  const key = [...spans]
    .filter(([, span]) => span.wholeStack !== true)
    .map(([name, span]) => `${name}\t${span.branch}`)
    .join("\n")
  return useMemo(() => new Map(key === "" ? [] : key.split("\n").map(line => line.split("\t") as [string, string])), [key])
}

/** Non-hook variant of `useLandingSpan`, for menus built on open */
export function getLandingSpan(branchName: string): LandingDescendant | null {
  return currentFacts().spans.get(branchName) ?? null
}

/**
 * The branch or the whole stack whose merged Safe Push landed this branch, its own included: until the update archives
 * the branch, its Status cell says "Landed with oauth" (with the commits after it: "· 1 commit after it"), and while all
 * of it landed it offers no Safe Push, Dry Run or push; null for a branch no merged robot landed
 */
export function useLandedBy(branchName: string): LandedBy | null {
  return useRepositoryFacts().landed.get(branchName) ?? null
}

/** Every branch a merged Safe Push landed (`useLandedBy`), branch → who landed it; the same map while none changes */
export function useLandedBranches(): ReadonlyMap<string, LandedBy> {
  return useRepositoryFacts().landed
}

/** Non-hook variant of `useLandedBy`, for menus built on open and the next step */
export function getLandedBy(branchName: string): LandedBy | null {
  return currentFacts().landed.get(branchName) ?? null
}

/**
 * The branches a Safe Push of the branch lands now: the branch and those it is built on, less the ones a merged Safe
 * Push landed whole already (`getLandedBy`): their commits are in the target ("Safe Push · 3"; 1 for a lone one). One
 * with commits after the landed ones lands them.
 */
export function getLandingBranchCount(branchName: string): number {
  const branches = branchSyncStore.getState().branches
  return stackAncestors(branchName, name => branches[name]).filter(ancestor => !isLandedWhole(getLandedBy(ancestor.name))).length + 1
}

/**
 * Why Push with Children of `name` has nothing to push (`formatLandedWholeReason`): merged Safe Pushes landed the branch
 * and every branch built on it whole ("Landed in master with deploy and side-test"); null while one of them is left, and
 * for a branch without children. While the rest has work, the landed ones are only left out of the push.
 */
export function useLandedWithChildrenReason(name: string): string | null {
  const landed = useRepositoryFacts().landed
  return useBranchSync(state => landedWithChildrenIn(landed, state, name))
}

/** Non-hook variant of `useLandedWithChildrenReason`, for menus built on open */
export function getLandedWithChildrenReason(name: string): string | null {
  return landedWithChildrenIn(currentFacts().landed, branchSyncStore.getState(), name)
}

function landedWithChildrenIn(landed: ReadonlyMap<string, LandedBy>, state: BranchSyncState, name: string): string | null {
  const descendants = stackDescendantsIn(state, name)
  if (descendants.length === 0) {
    return null
  }
  return formatLandedWholeReason(
    [name, ...descendants].map(it => landed.get(it)),
    name,
  )
}

/**
 * Why the whole stack of `root` has nothing left to push or land: merged Safe Pushes landed every branch of it whole
 * (`formatLandedWholeReason`), "Merged by Patronus" of its own robot, else "Landed in master with oauth and the whole
 * auth stack"; null while one of them is left, and for a stack that doesn't fork
 */
export function getStackLandedReason(root: string): string | null {
  return stackLandedIn(currentFacts().landed, branchSyncStore.getState(), root)
}

function stackLandedIn(landed: ReadonlyMap<string, LandedBy>, state: BranchSyncState, root: string): string | null {
  const members = state.stackBranches[root]?.members ?? []
  return formatLandedWholeReason(
    members.map(name => landed.get(name)),
    root,
  )
}

/**
 * Why the stack branch of the whole stack of `root` is not pushed (`StackBranchAlong` `held`): merged Safe Pushes landed
 * every branch of the stack whole, so it carries only commits that are in the target ("Landed in master with deploy and
 * side-test: nothing of it is left to push", `getStackLandedReason`); or its own robot runs and pushes into the target,
 * which a push of it would fail at the end (`formatStackPushHoldReason`). Null while neither holds it.
 */
export function useStackBranchHoldReason(root: string): string | null {
  const facts = useRepositoryFacts()
  return useBranchSync(state => stackBranchHoldReasonIn(facts, state, root))
}

/** Non-hook variant of `useStackBranchHoldReason`, for menus built on open and the push itself */
export function getStackBranchHoldReason(root: string): string | null {
  return stackBranchHoldReasonIn(currentFacts(), branchSyncStore.getState(), root)
}

function stackBranchHoldReasonIn(facts: PatronusFacts, state: BranchSyncState, root: string): string | null {
  const stack = state.stackBranches[root]
  if (stack === undefined || stack.members.length === 0) {
    return null
  }
  // the stack branch holds the commits of every member: a parked one keeps it local
  const parked = stack.members.find(member => state.parked[member] !== undefined)
  if (parked !== undefined) {
    return `It holds parked ${parked}: land the stack in pieces`
  }
  const landed = stackLandedIn(facts.landed, state, root)
  if (landed !== null) {
    return `${landed}: nothing of it is left to push`
  }
  const hold = facts.stackHolds.get(root)
  return hold === undefined ? null : formatStackPushHoldReason(root, hold.robot)
}

/** The robots a view needs: the active ones (their matches, their starts, what they land) and the merged ones (what they landed) */
function viewRobots(robots: Readonly<Record<string, PatronusRobot>>): PatronusRobot[] {
  return Object.values(robots).filter(robot => isRobotActive(robot) || (robot.status === "success" && robot.pushMode !== "dryRun"))
}

/** The robot of each branch and whole stack, as the rows match them (`findBranchRobot`, the cases Rust passes too) */
function robotMatchIn(robots: Readonly<Record<string, PatronusRobot>>, state: BranchSyncState): RobotMatch {
  const branches: Record<string, string> = {}
  for (const name of state.branchOrder) {
    const branch = state.branches[name]
    const robot = findBranchRobot(robots, name, branch?.commits[0]?.hash, branch?.remoteStatus?.remoteTip ?? null)
    if (robot !== undefined) {
      branches[name] = robot.id
    }
  }
  const stacks: Record<string, string> = {}
  for (const stack of Object.values(state.stackBranches)) {
    const robot = findStackRobot(robots, stack.root, stack.tip, stack.remoteStatus?.remoteTip ?? null)
    if (robot !== undefined) {
      stacks[stack.root] = robot.id
    }
  }
  return { branches, stacks }
}

/** What the page knows of the remote branches: a push moves them without a sync, the backend's table doesn't know it */
function remoteFactsIn(state: BranchSyncState): RemoteFacts {
  const fact = (remote: RemoteStatus | null | undefined) => ({ remoteTip: remote?.remoteTip ?? null, upToDate: remote != null && isRemoteUpToDate(remote) })
  return {
    branches: Object.fromEntries(state.branchOrder.map(name => [name, fact(state.branches[name]?.remoteStatus)])),
    stacks: Object.fromEntries(Object.values(state.stackBranches).map(stack => [stack.root, fact(stack.remoteStatus)])),
  }
}

/** Counts the questions of the view: an answer to an older one is dropped */
let viewRevision = 0
/**
 * What the last question was asked with: the same inputs are not asked again, and while the branches and the stack
 * branches are the same objects of the same table they are not even computed (a store change of something else)
 */
let askedView: {
  readonly key: string
  readonly robots: Readonly<Record<string, PatronusRobot>>
  readonly branches: BranchSyncState["branches"]
  readonly stacks: BranchSyncState["stackBranches"]
  /** The table the backend answers from: its revision in the store (`tableRevision`) */
  readonly table: number
  readonly inputs: string
} | null = null
/** The question in flight, and whether another one waits for it */
let viewInFlight: Promise<void> | null = null

/**
 * Asks `get_repository_view` when its inputs changed: the robots (their record, which `withRobots` keeps while nothing
 * changed), the matches and remote facts of the branches, and the table the session holds (by its revision: a sync that
 * changed nothing of it asks nothing). During a sync only a change of the robots asks (the backend answers from the table of the last sync), so a merge says what it landed at
 * once. One question in flight; one that changes meanwhile is asked once it answered. Resolves once the view is of the
 * inputs as they are.
 */
async function askRepositoryView(): Promise<void> {
  const key = currentKey()
  const project = getSelectedProject()
  const state = branchSyncStore.getState()
  const repository = key === null ? undefined : patronusStore.getState().repositories[key]
  if (key === null || project === null || repository === undefined) {
    return
  }
  if (viewInFlight !== null) {
    await viewInFlight
    return askRepositoryView()
  }
  const sameRobots = askedView?.key === key && askedView.robots === repository.robots
  // the table of the backend is the one of the session: a sync that changed nothing of it leaves the same revision
  const table = state.tableRevision
  if (sameRobots && (state.isSyncing || (askedView?.branches === state.branches && askedView.stacks === state.stackBranches && askedView.table === table))) {
    return
  }
  const matched = robotMatchIn(repository.robots, state)
  const remote = remoteFactsIn(state)
  const inputs = JSON.stringify([table, matched, remote])
  if (sameRobots && askedView?.inputs === inputs) {
    askedView = { ...askedView, branches: state.branches, stacks: state.stackBranches }
    return
  }
  const asking = repository.robots
  askedView = { key, robots: asking, branches: state.branches, stacks: state.stackBranches, table, inputs }
  const revision = ++viewRevision
  const asked = commands
    .getRepositoryView({ repositoryPath: project.path, robots: viewRobots(asking), matched, remote })
    .then(result => {
      if (result.status === "error") {
        console.warn("Cannot read the repository view", result.error)
        // the same inputs are not asked again (each change of the store would ask, a failure without end): a sync, a
        // change of the robots or a refresh of Patronus does. The view stays as it was, stale: nothing waits for an
        // answer that doesn't come; without one the steps say why
        update(key, entry => ({ viewOf: asking, viewListed: entry.viewListed || isOfListedRobots(entry, asking), viewFailure: getErrorDetails(result.error) }))
        return
      }
      const entry = patronusStore.getState().repositories[key]
      if (revision === viewRevision && key === currentKey() && entry !== undefined) {
        // the same view of the same robots (a sync that changed nothing): no store change, no selector of a row runs
        const view = keptView(entry.view, result.data.patronus)
        const viewListed = entry.viewListed || isOfListedRobots(entry, asking)
        // the landing needs of a branch of the table are computed in the background meanwhile, unless the same view of the
        // same count of computations said that none runs for them (failed, too slow): no step waits then
        const unknown = state.branchOrder.some(name => view.branches[name]?.landingNeeds.state !== "known")
        const computing = unknown && !(view === entry.view && landingSettled.get(key) === result.data.landingGeneration)
        if (view !== entry.view || entry.viewOf !== asking || viewListed !== entry.viewListed || (!computing && entry.landingNeedsComputing) || entry.viewFailure !== null) {
          update(key, () => ({ view, viewOf: asking, viewListed, viewFailure: null, ...(!computing && { landingNeedsComputing: false }) }))
        }
        if (computing) {
          waitForLandingNeeds(key, project.path, result.data.landingGeneration)
        }
      }
    })
    .finally(() => {
      viewInFlight = null
    })
  viewInFlight = asked
  await asked
}

/**
 * A view asked with `asking` is of the robots Patronus listed: a list answered, and the robots are still the ones it was
 * asked with (a list that changed them asks again)
 */
function isOfListedRobots(entry: RepositoryPatronus, asking: Readonly<Record<string, PatronusRobot>>): boolean {
  return entry.listed && entry.robots === asking
}

/** The repository whose landing needs the page waits for, one wait at a time */
let landingWait: string | null = null
/** By repository key, the count of computations of the landing needs after which no computation ran (`waitForLandingNeeds`) */
const landingSettled = new Map<string, number>()

/**
 * While the landing needs of a branch are unknown, Rust computes them in the background: waits until the computation
 * after the view's (`after`) finished (`wait_landing_needs`), then asks the view again. A wait that ends without one
 * (none ran, or it failed for these inputs) asks nothing: the needs stay unknown until the inputs change
 */
function waitForLandingNeeds(key: string, repositoryPath: string, after: number): void {
  if (!(patronusStore.getState().repositories[key]?.landingNeedsComputing ?? false)) {
    update(key, () => ({ landingNeedsComputing: true }))
  }
  if (landingWait === key) {
    return
  }
  landingWait = key
  void commands
    .waitLandingNeeds({ repositoryPath, after })
    .then(result => {
      if (result.status === "ok" && result.data !== after && key === currentKey()) {
        askedView = null
        void askRepositoryView()
      } else {
        // no computation answers for these inputs (it failed, or took too long): the needs stay unknown, no step waits
        landingSettled.set(key, result.status === "ok" ? result.data : after)
        update(key, () => ({ landingNeedsComputing: false }))
      }
    })
    .finally(() => {
      if (landingWait === key) {
        landingWait = null
      }
    })
}

/**
 * `next`, its entries equal to those of `previous` kept as the same objects, so a row that reads one doesn't render
 * again; `previous` itself while every entry is
 */
function keptView(previous: PatronusView | null, next: PatronusView): PatronusView {
  if (previous === null) {
    return next
  }
  const keep = <T>(before: Readonly<Record<string, T>>, after: Record<string, T>): Readonly<Record<string, T>> => {
    const kept = Object.fromEntries(
      Object.entries(after).map(([name, entry]) => [name, name in before && JSON.stringify(before[name]) === JSON.stringify(entry) ? before[name]! : entry]),
    )
    const names = Object.keys(kept)
    return names.length === Object.keys(before).length && names.every(name => kept[name] === before[name]) ? before : kept
  }
  const branches = keep(previous.branches, next.branches)
  const stacks = keep(previous.stacks, next.stacks)
  const landedByRobot = keep(previous.landedByRobot, next.landedByRobot)
  return branches === previous.branches && stacks === previous.stacks && landedByRobot === previous.landedByRobot ? previous : { branches, stacks, landedByRobot }
}

/**
 * The repository view of the selected repository is of its robots as they are now (or the last question failed): what
 * the robots hold and land is known, not the lag of a change of them. True without Patronus.
 */
export function useRepositoryViewCurrent(): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => {
    const repository = state.repositories[key]
    return repository === undefined || Object.keys(repository.robots).length === 0 || repository.viewOf === repository.robots
  })
}

/**
 * Resolves once the repository view is of the robots, the remote facts and the table as the sync leaves them: a push
 * and a start decide on its holds after it. Nothing changed: at once
 */
export async function whenRepositoryViewCurrent(): Promise<void> {
  await whenSynced()
  await askRepositoryView()
}

/**
 * Keeps the repository view current while the main window shows the branches (`askRepositoryView`): on every change of
 * the robots and of the branches, asked only when its inputs changed. Returns the unsubscribe.
 */
export function watchRepositoryView(): () => void {
  let scheduled = false
  const schedule = () => {
    if (scheduled) {
      return
    }
    scheduled = true
    queueMicrotask(() => {
      scheduled = false
      void askRepositoryView()
    })
  }
  const unsubscribes = [patronusStore, branchSyncStore].map(store => store.subscribe(schedule))
  schedule()
  return () => unsubscribes.forEach(unsubscribe => unsubscribe())
}

/**
 * What the Safe Push of the branch needs of a branch outside what it lands, as Rust computes it in the background after
 * a sync (`landingNeeds` of the repository view): the refusals the landing gate of its start would give, empty when it
 * needs nothing; null while unknown (not computed yet), which is never "nothing"
 */
export function useLandingNeeds(branchName: string): readonly LandingRefusal[] | null {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => landingNeedsIn(state.repositories[key], branchName))
}

/** Non-hook variant of `useLandingNeeds`, for menus built on open and the next step */
export function getLandingNeeds(branchName: string): readonly LandingRefusal[] | null {
  return landingNeedsIn(currentRepository(), branchName)
}

function landingNeedsIn(repository: RepositoryPatronus | undefined, branchName: string): readonly LandingRefusal[] | null {
  const needs = repository?.view?.branches[branchName]?.landingNeeds
  return needs?.state === "known" ? needs.refusals : null
}

/**
 * Rust computes what the Safe Push of the branch needs now (`landingNeedsComputing`, its needs unknown): its step waits
 * for the answer, a second or two after the view (`PendingFact` `landingNeeds`)
 */
export function isLandingNeedsPending(branchName: string): boolean {
  const repository = currentRepository()
  return repository?.landingNeedsComputing === true && landingNeedsIn(repository, branchName) === null
}

/**
 * Why the branch offers no push (`PushHold`): a Safe Push runs on it or lands it with a branch built on it, or a merged
 * one landed it; null while it can be pushed
 */
export function usePushHold(branchName: string): PushHold | null {
  return useRepositoryFacts().holds.get(branchName) ?? null
}

/** Every push hold of the selected repository, branch → its hold; the same map while no hold changes */
export function usePushHolds(): ReadonlyMap<string, PushHold> {
  return useRepositoryFacts().holds
}

/** Non-hook variant of `usePushHold`, for menus built on open and the push itself */
export function getPushHold(branchName: string): PushHold | null {
  return currentFacts().holds.get(branchName) ?? null
}

/** The robots and the view of the selected repository; undefined before the first answer */
function currentRepository(): RepositoryPatronus | undefined {
  const key = currentKey()
  return key === null ? undefined : patronusStore.getState().repositories[key]
}

/** What an estimate depends on of a robot */
type EstimatedRobot = Pick<PatronusRobot, "pushMode" | "targetBranch" | "profile">

/**
 * How long a robot like this one usually runs (`estimateRobotDuration`), from the robots listed for the repository and
 * the history of finished ones; null without history. Renders again only when the estimate changes. No request.
 */
export function useRobotEstimate(robot: EstimatedRobot | undefined): RobotEstimate | null {
  const key = useRepositoryKey()
  const repositoryPath = useSelectedProjectPath() ?? ""
  const history = useStore(robotHistoryStore, state => state.items)
  const pushMode = robot?.pushMode
  const targetBranch = robot?.targetBranch
  const profileId = robot?.profile == null ? undefined : robot.profile.id
  const profileName = robot?.profile?.name ?? null
  return useStore(patronusStore, state => {
    const robots = state.repositories[key]?.robots
    if (robots === undefined || pushMode === undefined || targetBranch === undefined) {
      return null
    }
    const profile = profileId === undefined ? null : { id: profileId, name: profileName, skipsReview: null }
    return estimateIn(robots, history, repositoryPath, { pushMode, targetBranch, profile })
  })
}

/** Non-hook variant of `useRobotEstimate`, for the Dock */
export function getRobotEstimate(robot: EstimatedRobot): RobotEstimate | null {
  const key = currentKey()
  const robots = key === null ? undefined : patronusStore.getState().repositories[key]?.robots
  return robots === undefined ? null : estimateIn(robots, robotHistoryStore.getState().items, getSelectedProject()?.path ?? "", robot)
}

interface Estimates {
  readonly robots: Readonly<Record<string, PatronusRobot>>
  readonly history: readonly RobotHistoryEntry[]
  readonly repositoryPath: string
  /** By push mode, target and profile */
  readonly values: Map<string, RobotEstimate | null>
}

/** The estimates of the last robots and history: every slot asks for its own, they are computed once per change */
let lastEstimates: Estimates | null = null

function estimateIn(robots: Readonly<Record<string, PatronusRobot>>, history: readonly RobotHistoryEntry[], repositoryPath: string, robot: EstimatedRobot): RobotEstimate | null {
  let estimates = lastEstimates
  if (estimates === null || estimates.robots !== robots || estimates.history !== history || estimates.repositoryPath !== repositoryPath) {
    estimates = { robots, history, repositoryPath, values: new Map() }
    lastEstimates = estimates
  }
  const profile = robot.profile
  const cacheKey = `${robot.pushMode}\n${robot.targetBranch}\n${profile == null ? "?" : `${profile.id ?? ""}\n${profile.name ?? ""}`}`
  let estimate = estimates.values.get(cacheKey)
  if (estimate === undefined) {
    const samples = [...Object.values(robots).map(robotSample), ...historySamples(history, repositoryPath)]
    estimate = estimateRobotDuration(robot, samples, Date.now())
    estimates.values.set(cacheKey, estimate)
  }
  return estimate
}

/** The robot failed for missing code-owner approvals (its problems are read once, see `loadReviewProblems`) */
export function useRobotNeedsReview(robotId: string | undefined): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => robotId !== undefined && state.repositories[key]?.needsReview[robotId] === true)
}

/** By robot id, the failed robots of the branches that failed for missing code-owner approvals; re-renders only when they change */
/** The robots of no branch the app watches (`watchRobot`), by id, with what they test */
export function useWatchedRobots(): Readonly<Record<string, string>> {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.watched ?? EMPTY.watched)
}

export function useRobotsNeedingReview(): Readonly<Record<string, boolean>> {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.needsReview ?? EMPTY.needsReview)
}

/** The running robot will fail: a required check failed (see `RepositoryPatronus.failing`) */
export function useRobotFailing(robotId: string | undefined): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => robotId !== undefined && state.repositories[key]?.failing[robotId] === true)
}

/** By robot id, the running robots that will fail; re-renders only when they change */
export function useFailingRobots(): Readonly<Record<string, true>> {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.failing ?? EMPTY.failing)
}

/**
 * The details of a running robot said that it will fail (`PatronusRobotDetails.failing`): the open popover knows it
 * before the next poll
 */
export function markRobotFailing(robotId: string): void {
  const key = currentKey()
  if (key !== null) {
    markFailing(key, [robotId])
  }
}

function markFailing(key: string, robotIds: readonly string[]): void {
  const known = patronusStore.getState().repositories[key]?.failing ?? EMPTY.failing
  const added = robotIds.filter(id => known[id] !== true)
  if (added.length > 0) {
    update(key, previous => ({ failing: { ...previous.failing, ...Object.fromEntries(added.map(id => [id, true] as const)) } }))
  }
}

/** Non-hook variant of `useRobotNeedsReview`, for menus built on open */
export function isRobotNeedingReview(robotId: string): boolean {
  const key = currentKey()
  return key !== null && patronusStore.getState().repositories[key]?.needsReview[robotId] === true
}

/** A required check of the running robot failed (`robot_is_failing`): it will fail */
export function isRobotFailing(robotId: string): boolean {
  const key = currentKey()
  return key !== null && patronusStore.getState().repositories[key]?.failing[robotId] === true
}

/** The robots of the selected repository, by id; re-renders only when they change, not on every refresh */
export function useRepositoryRobots(): Readonly<Record<string, PatronusRobot>> | undefined {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.robots)
}

/** The GitHub repository the branches are reviewed in (see `RepositoryPatronus.githubRepository`) */
export function usePatronusGitHubRepository(): string | null {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.githubRepository ?? null)
}

/** Non-hook variant of `usePatronusGitHubRepository`, for menus built on open */
export function getPatronusGitHubRepository(): string | null {
  const key = currentKey()
  return key === null ? null : (patronusStore.getState().repositories[key]?.githubRepository ?? null)
}

/** Non-hook variant of `useBranchRobot`, for menus built on open */
export function getBranchRobot(branchName: string): PatronusRobot | undefined {
  const key = currentKey()
  if (key === null) {
    return undefined
  }
  const branch = branchSyncStore.getState().branches[branchName]
  return findBranchRobot(patronusStore.getState().repositories[key]?.robots, branchName, branch?.commits[0]?.hash, branch?.remoteStatus?.remoteTip ?? null)
}

// ---- updates ----

function update(key: string, patch: (entry: RepositoryPatronus) => Partial<RepositoryPatronus>): void {
  patronusStore.setState(state => {
    const entry = state.repositories[key] ?? EMPTY
    return { repositories: { ...state.repositories, [key]: { ...entry, ...patch(entry) } } }
  })
}

function sameRobot(a: PatronusRobot, b: PatronusRobot): boolean {
  return (
    a.status === b.status &&
    a.sourceRevision === b.sourceRevision &&
    a.finishTime === b.finishTime &&
    a.name === b.name &&
    a.branchName === b.branchName &&
    a.cancellationReason === b.cancellationReason &&
    a.pushMode === b.pushMode &&
    a.targetBranch === b.targetBranch &&
    a.startTime === b.startTime &&
    a.pullRequest?.number === b.pullRequest?.number &&
    a.profile?.id === b.profile?.id &&
    a.profile?.name === b.profile?.name
  )
}

/**
 * `previous` with `changed` (each replaces the robot of its id, the merge profile kept when it names none: a list never
 * does, the details or a start named it) and without `removed`. A robot that did not change keeps its object, so the
 * chips of other branches don't render again; nothing changed = the same record.
 */
function withRobots(previous: Readonly<Record<string, PatronusRobot>>, changed: readonly PatronusRobot[], removed: readonly string[]): Readonly<Record<string, PatronusRobot>> {
  const next = withoutKeys(previous, removed)
  let differs = removed.some(id => id in previous)
  for (const answered of changed) {
    const known = previous[answered.id]
    const robot = answered.profile == null && known?.profile != null ? { ...answered, profile: known.profile } : answered
    if (known !== undefined && sameRobot(known, robot)) {
      next[robot.id] = known
    } else {
      next[robot.id] = robot
      differs = true
    }
  }
  return differs ? next : previous
}

/**
 * Stores `robot`, which an action of the page answered (a start, a restart, a cancel, a status read), in the robots of
 * `key` before the session's patch has it, and tells a merge it shows
 */
function storeRobot(key: string, robot: PatronusRobot): void {
  const previous = patronusStore.getState().repositories[key]?.robots ?? EMPTY.robots
  const robots = withRobots(previous, [robot], [])
  if (robots === previous) {
    return
  }
  update(key, () => ({ robots }))
  recordFinishedRobots(key.slice(0, key.lastIndexOf("\n")), Object.values(robots))
  for (const merged of findMergedRobots(previous, robots)) {
    announceMerge(key, merged)
  }
}

// ---- merges ----

/** The merged robots already told about, once each */
const announcedMerges = new Set<string>()

/**
 * A robot merged a branch or the whole stack ("Whole air stack merged into master"): a toast with Fetch, so the header
 * learns that an update brings the branch in, and "Open in Patronus". While no window is focused, an OS notification
 * says it at once (unless Settings turned it off), and the toast waits for the focus.
 * A stacked branch landed its ancestors too, the whole stack every branch of it: their open pull requests are closed
 * as the repository view says it (`closeLandedPullRequests`, `landedByRobot`), and the toast says so: it waits for the
 * view of the merged robot (`whenRepositoryViewCurrent`), which the sync that runs and one `get_repository_view` give.
 */
function announceMerge(key: string, robot: PatronusRobot): void {
  if (announcedMerges.has(robot.id)) {
    return
  }
  announcedMerges.add(robot.id)
  if (robot.pullRequest != null) {
    // Patronus closed the pull request with the label `merged`
    void refreshPullRequests("push")
  }
  // which pull requests it landed: the view of the merged robot, once it is of the robots as they are now
  void whenRepositoryViewCurrent().then(() => tellMerge(key, robot))
}

/** The merge told (`announceMerge`), with the pull requests of what it landed closed, as the view says it now */
function tellMerge(key: string, robot: PatronusRobot): void {
  const [repositoryPath] = key.split("\n") as [string, string]
  const landed = patronusStore.getState().repositories[key]?.view?.landedByRobot[robot.id] ?? []
  // the branches and pull requests of the table, and the view the wait made current, are of the shown repository: a
  // merge of another one closes nothing there, and its toast says so
  const closed = currentKey() === key ? closeLandedPullRequests(robot, landed, key) : describeLandedLeftOpen(landed, repositoryPath)
  const description = closed === null ? formatPushMode(robot.pushMode) : `${formatPushMode(robot.pushMode)} · ${closed}`
  const title = formatMergedRobotTitle(robot, patronusStore.getState().repositories[key]?.watched[robot.id])
  recordRobot(title, description, robot, repositoryPath)
  // the toast's button picks it up (Update Project fetches first): the notification says what merged, as the toast
  const notification = { title, body: description }
  announce("patronusMerges", notification, () => showMergeToast(repositoryPath, robot, title, description))
}

/**
 * The baseline was fetched after `finishTime` (Unix seconds), by Branch Deck or elsewhere (`FETCH_HEAD`,
 * `lastFetchTime`): the header knows already what the update brings in
 */
function isFetchedSince(repositoryPath: string, finishTime: number | null): boolean {
  if (finishTime === null) {
    return false
  }
  const baseline = branchSyncStore.getState().baselineStatus
  const update = baseline?.repositoryPath === repositoryPath ? baseline.update : null
  const fetched = lastFetchTime(update, getRepositoryState(repositoryPath)?.lastFetchTime)
  return fetched !== null && fetched > finishTime * 1000
}

/**
 * The merge toast: "Update Project", which fetches the baseline and updates onto it (the click is the consent; a
 * conflict or local changes say so as Update Project does), then "Open in Patronus". A merge in a repository the main
 * window doesn't show is fetched only (Update Project updates the shown one), and once the baseline was fetched since
 * the merge in it, the toast opens the robot page alone (decided when it shows, also after the focus).
 */
function showMergeToast(repositoryPath: string, robot: PatronusRobot, title: string, description: string): void {
  const shown = getSelectedProject()?.path === repositoryPath
  if (!shown && isFetchedSince(repositoryPath, robot.finishTime)) {
    toast.add({ type: "success", title, description, timeout: 10_000, actionProps: { children: "Open in Patronus", onClick: () => openRobot(robot) } })
    return
  }
  const id = toast.add({
    type: "success",
    title,
    description,
    timeout: 10_000,
    actionProps: {
      children: shown ? "Update Project" : "Fetch",
      onClick: () => {
        toast.close(id)
        // the repository of the merge, whichever the main window shows at the click: a fetch only once it is another
        void (shown ? runUpdateProjectOf(repositoryPath) : runFetchBaseline(repositoryPath))
      },
    },
    // Fetch is the primary action, the robot page the second one
    data: { secondaryAction: { label: "Open in Patronus", onClick: () => openRobot(robot) } },
  })
}

// ---- the robots of the session ----

/** The facts of the session's view of the main window (`onSessionFacts`): the robots, when a patch has them */
export function applySessionFacts(repositoryPath: string, facts: FactsView, changes: FactsChanges | null): void {
  if (changes === null || changes.robots != null) {
    applyRobotsFacts(repositoryPath, facts.robots, changes?.robots ?? null)
  }
}

/**
 * The robots of the session of `repositoryPath` as its view has them now (`facts.robots`), with the patch that brought
 * them (`change`; null for a snapshot, which replaces them): the session lists and polls them by its rules
 * (`remote_facts::scheduler`), the page only stores them. A patch replaces the robots it names, the merge profile kept
 * when it names none (the details named it), and removes those it drops; a robot it doesn't name keeps its object, so
 * the chips of other branches don't render again. A new answer of Patronus asks for the facts of the robots it brought
 * (`loadRobotFacts`); the robots that merged since the robots before are told (`announceMerge`)
 */
export function applyRobotsFacts(repositoryPath: string, view: RobotsView, change: RobotsChange | null): void {
  const key = repositoryKey(repositoryPath, getEffectiveBranchPrefix())
  const entry = patronusStore.getState().repositories[key] ?? EMPTY
  const removed = change === null ? Object.keys(entry.robots).filter(id => !(id in view.robots)) : (change.removedRobots ?? [])
  const changed = Object.values(change === null ? view.robots : (change.robots ?? {}))
  const robots = withRobots(entry.robots, changed, removed)
  const head = view.head
  const availability = head.value?.availability ?? null
  // a failure after robots were listed keeps them, stale
  const listed = entry.listed || availability?.status === "available" || head.state.kind === "stale"
  // a snapshot of a session Patronus has not answered yet (`Loading`) is no answer: the robots of the repository are
  // stored from the first answer on, and their view asked from then
  const answered = head.value !== null && (change === null || change.head != null)
  if (robots === entry.robots && !answered) {
    return
  }
  if (answered && availability?.status === "available" && entry.viewFailure !== null) {
    // a view that failed is asked again after an answer of Patronus, also of the same robots
    askedView = null
  }
  update(key, previous => ({
    robots,
    availability,
    githubRepository: head.value?.githubRepository ?? null,
    fetchedAt: head.observedAt * 1000,
    listed,
    // a list that left the robots as they were (none) has its view already
    viewListed: previous.viewListed || (listed && !previous.listed && previous.viewOf === robots),
  }))
  if (robots !== entry.robots) {
    // what the estimates learn from, also after a restart
    recordFinishedRobots(repositoryPath, Object.values(robots))
    for (const robot of findMergedRobots(entry.robots, robots)) {
      announceMerge(key, robot)
    }
  }
  if (answered && head.value?.availability.status === "available") {
    loadRobotFacts(key)
  }
}

/**
 * Asks the session to list the robots again, the full window (the refresh button); `isRefreshing` until its patch is in
 */
export async function refreshPatronus(): Promise<void> {
  const key = currentKey()
  const repositoryPath = getSelectedProject()?.path
  if (key === null || repositoryPath === undefined) {
    return
  }
  update(key, () => ({ isRefreshing: true }))
  try {
    await refreshSessionFacts(repositoryPath, "robots", "manual")
  } finally {
    update(key, () => ({ isRefreshing: false }))
  }
}

/**
 * The facts of the robots an answer of Patronus (a list or a poll) brought: the problems of the failed ones, whether the
 * running ones will fail. Only after an answer: a sync end or a focus the list gate refused brings no new robot and asks
 * nothing (each `/problems` is a request per robot)
 */
function loadRobotFacts(key: string): void {
  void loadReviewProblems(key, true)
  void loadFailingRobots(key)
}

const problemsInFlight = new Set<string>()
/** The keys whose `loadFailingRobots` runs: one at a time per repository, another repository's runs beside it */
const failingInFlight = new Set<string>()

/**
 * Asks whether the running robots of the branches (and the watched ones) will fail, the ones not known failing yet, in
 * one request: a required check that failed dooms the robot long before it finishes, and the user can cancel it
 */
async function loadFailingRobots(key: string): Promise<void> {
  const failing = patronusStore.getState().repositories[key]?.failing ?? EMPTY.failing
  // a canceling robot is the user's decision already
  const robotIds = activeBranchRobots(key)
    .filter(robot => robot.status === "running" && failing[robot.id] !== true)
    .map(robot => robot.id)
  if (robotIds.length === 0 || failingInFlight.has(key)) {
    return
  }
  failingInFlight.add(key)
  try {
    const [repositoryPath] = key.split("\n") as [string, string]
    const result = await commands.getPatronusFailingRobots({ repositoryPath, robotIds })
    if (result.status === "ok") {
      markFailing(key, result.data)
    }
  } catch {
    // an extra: the robot shows as running
  } finally {
    failingInFlight.delete(key)
  }
}

/**
 * Reads the problems of the failed robots of the branches once (a finished robot does not change; the backend caches
 * them too), to name a robot that failed for missing approvals "Needs review"; the next step of its branch waits for
 * them (`isRobotProblemsPending`). Those Patronus didn't give are asked again only with `retryFailed` (after an answer
 * of a list or a poll): a change of the branches asks only the robots it matched anew.
 */
async function loadReviewProblems(key: string, retryFailed: boolean): Promise<void> {
  const [repositoryPath] = key.split("\n") as [string, string]
  const entry = patronusStore.getState().repositories[key]
  if (entry === undefined) {
    return
  }
  for (const robot of tableRobots(entry)) {
    if (
      robot === undefined ||
      robot.status !== "failure" ||
      robot.id in entry.needsReview ||
      problemsInFlight.has(robot.id) ||
      (!retryFailed && entry.problemsFailed[robot.id] !== undefined)
    ) {
      continue
    }
    problemsInFlight.add(robot.id)
    let answered = false
    let failure = "Patronus gave no answer"
    try {
      const result = await commands.getPatronusRobotProblems({ repositoryPath, robotId: robot.id })
      if (result.status === "ok") {
        answered = true
        const needsReview = result.data.some(problem => problem.subsystem === REVIEW_APPROVAL_SUBSYSTEM)
        update(key, previous => ({ needsReview: { ...previous.needsReview, [robot.id]: needsReview }, problemsFailed: withoutKey(previous.problemsFailed, robot.id) }))
      } else {
        failure = getErrorDetails(result.error)
      }
    } catch (error) {
      failure = getErrorDetails(error)
    } finally {
      problemsInFlight.delete(robot.id)
    }
    // why it failed stays unknown: the step of its branch says so, no restart as if it hadn't failed for approvals
    if (!answered) {
      update(key, previous => ({ problemsFailed: { ...previous.problemsFailed, [robot.id]: failure } }))
    }
  }
}

/** The robots of the branches and of the whole stacks of the table: those whose problems are asked (`loadReviewProblems`) */
function tableRobots(entry: RepositoryPatronus): Array<PatronusRobot | undefined> {
  const { branches, stackBranches } = branchSyncStore.getState()
  return [
    ...Object.values(branches).map(branch => findBranchRobot(entry.robots, branch.name, branch.commits[0]?.hash, branch.remoteStatus?.remoteTip ?? null)),
    // the robot of the whole stack fails for missing approvals in the pull request of the stack
    ...Object.values(stackBranches).map(stack => findStackRobot(entry.robots, stack.root, stack.tip, stack.remoteStatus?.remoteTip ?? null)),
  ]
}

/**
 * `robot` is the robot of a branch or of a whole stack of the table, so Patronus is asked why it failed: its restart
 * and its prompts wait for that answer on every surface (`getGatedRobotActions`); a watched robot of no branch is not
 */
export function isTableRobot(robot: PatronusRobot): boolean {
  const entry = currentRepository()
  return entry !== undefined && tableRobots(entry).some(it => it?.id === robot.id)
}

function withoutKey<T>(record: Readonly<Record<string, T>>, key: string): Readonly<Record<string, T>> {
  if (!(key in record)) {
    return record
  }
  return Object.fromEntries(Object.entries(record).filter(([id]) => id !== key))
}

/**
 * Why the failed `robot` failed is still to come (`loadReviewProblems`): for approvals ("Needs review": a restart fails
 * again) or not; the next step of its branch waits for it. Neither a robot that didn't fail nor one whose problems
 * Patronus didn't give
 */
export function isRobotProblemsPending(robot: PatronusRobot): boolean {
  const key = currentKey()
  const entry = key === null ? undefined : patronusStore.getState().repositories[key]
  return robot.status === "failure" && entry !== undefined && !(robot.id in entry.needsReview) && entry.problemsFailed[robot.id] === undefined
}

/** Why Patronus didn't give the problems of the failed `robot` (`problemsFailed`); null while they came or are to come */
export function getRobotProblemsFailure(robot: PatronusRobot): string | null {
  const key = currentKey()
  return robot.status !== "failure" || key === null ? null : (patronusStore.getState().repositories[key]?.problemsFailed[robot.id] ?? null)
}

/** Asks Patronus again why the failed robots of the branches failed, those it didn't give too (Check Approvals Again) */
export function askRobotProblemsAgain(): void {
  const key = currentKey()
  if (key !== null) {
    void loadReviewProblems(key, true)
  }
}

/**
 * Asks why the failed robots that a change of the branches matched failed (`loadReviewProblems`): a sync that gives a
 * branch a robot of an earlier answer, whose problems no answer asked. Returns the unsubscribe
 */
export function watchRobotProblems(): () => void {
  let scheduled = false
  return branchSyncStore.subscribe((state, previous) => {
    if (scheduled || (state.branches === previous.branches && state.stackBranches === previous.stackBranches)) {
      return
    }
    scheduled = true
    queueMicrotask(() => {
      scheduled = false
      const key = currentKey()
      if (key !== null) {
        void loadReviewProblems(key, false)
      }
    })
  })
}

/**
 * The running robots of the branches of the selected repository, of the whole stacks and the watched ones, with the
 * status the app knows
 */
function activeBranchRobots(key: string): PatronusRobot[] {
  const repository = patronusStore.getState().repositories[key]
  const robots = repository?.robots
  const active = new Map<string, PatronusRobot>()
  for (const id of Object.keys(repository?.watched ?? {})) {
    const robot = robots?.[id]
    if (robot !== undefined && isRobotActive(robot)) {
      active.set(id, robot)
    }
  }
  const { branches, stackBranches } = branchSyncStore.getState()
  for (const branch of Object.values(branches)) {
    const robot = findBranchRobot(robots, branch.name, branch.commits[0]?.hash, branch.remoteStatus?.remoteTip ?? null)
    if (robot !== undefined && isRobotActive(robot)) {
      active.set(robot.id, robot)
    }
  }
  for (const stack of Object.values(stackBranches)) {
    const robot = findStackRobot(robots, stack.root, stack.tip, stack.remoteStatus?.remoteTip ?? null)
    if (robot !== undefined && isRobotActive(robot)) {
      active.set(robot.id, robot)
    }
  }
  return [...active.values()]
}

// ---- actions ----

/** The selected repository, whose robots the page shows; throws without one. Rust resolves the branch prefix */
export function patronusRequest(): VcsRequest {
  const key = currentKey()
  if (key === null) {
    throw new Error("No repository is selected")
  }
  const [repositoryPath] = key.split("\n") as [string, string]
  return { repositoryPath }
}

/** The key the robots of `repositoryPath` are stored under (`storeKeyOf`); null for one whose robots were never read */
function robotsKeyOf(repositoryPath: string): string | null {
  return storeKeyOf(repositoryPath, currentKey(), Object.keys(patronusStore.getState().repositories))
}

/** Patronus answers for the selected repository (it is on Space): the start actions are offered */
export function usePatronusAvailable(): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => {
    const status = state.repositories[key]?.availability?.status
    return status !== undefined && status !== "notPatronus"
  })
}

/** What Patronus answered for the selected repository: whether it is on Space, logged in, failing; null before */
export function getPatronusAvailability(): PatronusAvailability | null {
  return currentRepository()?.availability ?? null
}

/** Non-hook variant of `usePatronusAvailable`, for menus built on open */
export function isPatronusAvailable(): boolean {
  const key = currentKey()
  const status = key === null ? undefined : patronusStore.getState().repositories[key]?.availability?.status
  return status !== undefined && status !== "notPatronus"
}

/**
 * The starts of Patronus are offered for the selected repository: it answers for it, or it hasn't answered yet (each
 * start disabled with "Waits for the answer of Patronus" meanwhile); not once it says the repository is not on Space
 */
export function usePatronusOffered(): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.availability?.status !== "notPatronus")
}

/** Non-hook variant of `usePatronusOffered`, for menus built on open */
export function isPatronusOffered(): boolean {
  const key = currentKey()
  return key === null || patronusStore.getState().repositories[key]?.availability?.status !== "notPatronus"
}

/**
 * Patronus answered a list of the robots of the selected repository once: a failure after it (an error, offline) leaves
 * the steps on the robots it listed, as a failed refresh of GitHub keeps the last listing
 */
export function hasPatronusListed(): boolean {
  return currentRepository()?.listed ?? false
}

/**
 * Why Patronus didn't give the view of the robots of the selected repository, while no earlier view stands for it:
 * what they hold, land and need is unknown, so no start is decided on "none"; null once a view answered
 */
export function getPatronusViewFailure(): string | null {
  const entry = currentRepository()
  return entry === undefined || entry.view !== null ? null : (entry.viewFailure ?? null)
}

/** `getPatronusViewFailure` of the selected repository, for a component */
export function usePatronusViewFailure(): string | null {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => {
    const entry = state.repositories[key]
    return entry === undefined || entry.view !== null ? null : (entry.viewFailure ?? null)
  })
}

/** `isPatronusPending` of the selected repository, for a component */
export function usePatronusPending(): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => isPendingEntry(state.repositories[key]))
}

/** Patronus answered whether the selected repository is on Space: before that, nothing says how its branches land */
export function isPatronusKnown(): boolean {
  const key = currentKey()
  return key !== null && patronusStore.getState().repositories[key]?.availability?.status !== undefined
}

/**
 * The next steps wait for Patronus (`PendingFact` `patronus`): it hasn't answered for the selected repository, or it
 * lists robots and the repository view of the robots it listed first isn't in, so what they hold and land is not known.
 * A failure (not logged in, an error) is an answer
 */
export function isPatronusPending(): boolean {
  const key = currentKey()
  if (key === null) {
    return false
  }
  return isPendingEntry(patronusStore.getState().repositories[key])
}

function isPendingEntry(entry: RepositoryPatronus | undefined): boolean {
  const status = entry?.availability?.status
  return status === undefined || (status === "available" && !entry!.viewListed)
}

/**
 * Watches a robot of no branch the user started (a Safe Push of a reviewed pull request), named by what it tests
 * ("#101 Rework the session store"): it is stored, the session polls it while it runs (the demand of the window), its
 * merge is told and the header lists it, for the life of the window
 */
export function watchRobot(robot: PatronusRobot, label: string): void {
  const key = currentKey()
  if (key === null) {
    return
  }
  storeRobot(key, robot)
  update(key, entry => ({ watched: { ...entry.watched, [robot.id]: label } }))
  watchRobotInDemand(robot.id)
}

/**
 * Adds or replaces a robot the user just started, restarted or canceled, in the repository of `key` (by default the
 * selected one: a start that ran on after a switch stores its robot where it started); the command gave it to the
 * session too, which polls it while it runs
 */
export function upsertPatronusRobot(robot: PatronusRobot, key: string | null = currentKey()): void {
  if (key !== null) {
    storeRobot(key, robot)
  }
}

/**
 * The merge profile an answer named for a robot (its details), for the estimate of its time: kept on the stored robot,
 * as a list never names it. Asks nothing.
 */
export function rememberRobotProfile(robot: PatronusRobot): void {
  const key = currentKey()
  const known = key === null ? undefined : patronusStore.getState().repositories[key]?.robots[robot.id]
  if (key !== null && known !== undefined && robot.profile != null && known.profile == null) {
    storeRobot(key, { ...known, profile: robot.profile })
  }
}

/**
 * Starts a Safe Push or a Dry Run of the pushed virtual branch (the caller pushes it first when the remote does not have
 * its tip), or with `stack` of the pushed stack branch of the whole stack of the root `branchName`, with the merge request
 * whose approvals its review check counts. Resolves to the robot, now
 * polled while it runs, with the profile it was started with; throws what Patronus said. The caller announces it
 * (`announceRobot`, with the profile name).
 */
export async function startPatronusRobot(
  branchName: string,
  mode: PatronusPushMode,
  profileId: string | null,
  mergeRequest: PatronusMergeRequest | null,
  profileName: string | null = null,
  stack = false,
  allow: LandingAllowances = {},
  repository: StartRepository | null = null,
): Promise<PatronusRobot> {
  const request = repository?.request ?? patronusRequest()
  const key = robotsKeyOf(request.repositoryPath)
  const result = await commands.startPatronusRobot({
    repositoryPath: request.repositoryPath,
    baselineBranch: repository?.baselineBranch ?? branchSyncStore.getState().baselineBranch ?? "",
    branchName,
    ...(stack && { stack: true }),
    mode,
    profileId,
    mergeRequest,
    allow,
  })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  // the form knows the name of the profile Patronus may not have written into the robot yet
  const robot =
    result.data.profile?.name == null && profileId !== null
      ? { ...result.data, profile: { id: profileId, name: profileName, skipsReview: result.data.profile?.skipsReview ?? null } }
      : result.data
  upsertPatronusRobot(robot, key)
  return robot
}

/** The repository a start runs in, as it was when the start was asked, and its baseline then */
export interface StartRepository {
  readonly request: VcsRequest
  readonly baselineBranch: string
}

/** The branch of the user a comment on its pull request starts, checked in Rust before the comment as a start is */
export type OwnPullRequestStart = PullRequestStartBranch

/** A start from a pull request the bot accepted: its robot, or null while Patronus lists none yet; the posted comment */
export interface CommentStart {
  readonly robot: PatronusRobot | null
  readonly commentUrl: string
  /**
   * The robot once Patronus lists it: `robot`, or for a start accepted without one what the look in the background
   * finds, null when it found none within `LATE_ROBOT_WAIT_MS`
   */
  readonly listed: Promise<PatronusRobot | null>
}

/**
 * The accepted comment starts of the user's branches whose robot Patronus lists not yet, by repository, pull request
 * and mode (`lateStartKey`), until the look in the background ends: a repeat joins it and posts no second comment
 */
const lateCommentStarts = new Map<string, CommentStart>()

function lateStartKey(key: string, pullRequestNumber: number, mode: PatronusPushMode): string {
  return `${key}\0${pullRequestNumber}\0${mode}`
}

/** After a minute without the robot of an accepted comment, it is looked for in the background this long */
export const LATE_ROBOT_WAIT_MS = 5 * 60_000
/** How often the robot of an accepted comment is looked for after the first minute */
export const LATE_ROBOT_INTERVAL_MS = 15_000

/**
 * Starts a Safe Push (`/patronus merge`) or a Dry Run (`/patronus dry-run`) of the pushed branch `start` (or whole
 * stack) from its GitHub pull request: Patronus links the robot to the pull request only from its comment, so the
 * approvals of the pull request count and Patronus closes it once merged. Rust checks the start before the comment, as
 * `start_patronus_robot` checks it (parked, landed, not built, a robot in its way, the landing gate). Posts the comment,
 * then waits up to a minute for the bot: 👎 throws its reason, no answer throws; after 👍 the start is made, and the new
 * robot of the pull request is listed, stored and returned (polled while it runs). A robot Patronus doesn't list within
 * the minute is null: the 👍 started it, and it is looked for in the background for `LATE_ROBOT_WAIT_MS` more; a
 * repeat meanwhile (the same pull request and mode) joins that start and posts no second comment.
 */
export async function startPatronusFromPullRequest(
  pullRequestNumber: number,
  mode: PatronusPushMode,
  start: OwnPullRequestStart,
  request: VcsRequest | null = null,
): Promise<CommentStart> {
  return askPatronusOnPullRequest(pullRequestNumber, mode, "ownBranch", null, Number.POSITIVE_INFINITY, request, start)
}

/** After the 👍 on a pull request of someone else, its robot is looked for this long: it may not be the user's */
export const REVIEWED_ROBOT_WAIT_MS = 10_000

/**
 * Starts a Safe Push of a pull request of someone else the user reviewed (`/patronus merge`, which deletes its branch
 * once merged), in `githubRepository` (`owner/name`). The 👍 of the bot is the start; the robot is returned if the
 * user's robots list it soon after (Patronus may count it as the author's), else null. 👎 throws its reason.
 */
export async function startPatronusFromReviewedPullRequest(pullRequestNumber: number, githubRepository: string | null): Promise<PatronusRobot | null> {
  return (await askPatronusOnPullRequest(pullRequestNumber, "rebaseAutosquash", "reviewedPullRequest", githubRepository, REVIEWED_ROBOT_WAIT_MS)).robot
}

/**
 * The robot a `/patronus` comment on pull request `pullRequestNumber` started in `mode`, as `bd safe-push` matches it:
 * of that pull request, a dry run for a dry run and a Safe Push for a Safe Push, started since shortly before the
 * comment, and none of `known` (the robots listed before the comment: a canceled Dry Run of the same pull request a
 * minute before is never the new robot); the newest of them
 */
export function findCommentRobot(
  robots: Readonly<Record<string, PatronusRobot>>,
  pullRequestNumber: number,
  mode: PatronusPushMode,
  postedAt: number,
  known: ReadonlySet<string>,
): PatronusRobot | undefined {
  let found: PatronusRobot | undefined
  for (const robot of Object.values(robots)) {
    const matches =
      robot.pullRequest?.number === pullRequestNumber &&
      (robot.pushMode === "dryRun") === (mode === "dryRun") &&
      robot.startTime * 1000 >= postedAt - COMMENT_ROBOT_SLACK_MS &&
      !known.has(robot.id)
    if (matches && (found === undefined || robot.startTime > found.startTime)) {
      found = robot
    }
  }
  return found
}

/**
 * Posts the command, waits up to a minute from the comment (the checks before it take none of it) for the reaction of
 * the bot (👎 throws its reason, no answer throws), then, within that minute and for up to `robotWait` after the 👍,
 * asks the session to list the robots (`refresh_facts`, `afterWrite`) for the one it started (`findCommentRobot`):
 * returned, polled by the session while it runs; null when none came, then looked for in the background with an
 * infinite `robotWait` (the user's own robot).
 */
async function askPatronusOnPullRequest(
  pullRequestNumber: number,
  mode: PatronusPushMode,
  subject: PatronusCommandSubject,
  githubRepository: string | null,
  robotWait: number,
  request: VcsRequest | null = null,
  start: OwnPullRequestStart | null = null,
): Promise<CommentStart> {
  // the repository the start was asked in: the answer of the bot applies there, also after a switch
  const { repositoryPath } = request ?? patronusRequest()
  const key = robotsKeyOf(repositoryPath)
  const late = lateStartKey(key ?? repositoryPath, pullRequestNumber, mode)
  const joined = start === null ? undefined : lateCommentStarts.get(late)
  if (joined !== undefined) {
    return joined
  }
  const known = new Set(Object.keys((key === null ? undefined : patronusStore.getState().repositories[key])?.robots ?? {}))
  const posted = await commands.postPatronusCommand({
    repositoryPath,
    pullRequestNumber,
    mode,
    subject,
    githubRepository,
    start,
  })
  if (posted.status === "error") {
    throw commandFailure(posted.error)
  }
  // the comment is posted once the command answers, after the checks of the start in Rust (the list, the landing gate):
  // the minute of the bot counts from here, not from before the checks
  const postedAt = Date.now()
  const replyParams = { repositoryPath, pullRequestNumber, commentId: posted.data.id, githubRepository }
  // the session lists the robots started since shortly before its last list, and once more after a list in flight
  const lookUp = async (): Promise<PatronusRobot | undefined> => {
    if (key === null) {
      return undefined
    }
    await refreshSessionFacts(repositoryPath, "robots", "afterWrite")
    return findCommentRobot(patronusStore.getState().repositories[key]?.robots ?? {}, pullRequestNumber, mode, postedAt, known)
  }
  let acceptedAt: number | null = null
  let rejectedOnce = false
  const deadline = () => Math.min(postedAt + COMMAND_REPLY_TIMEOUT_MS, acceptedAt === null ? Number.POSITIVE_INFINITY : acceptedAt + robotWait)
  while (Date.now() < deadline()) {
    await new Promise(resolve => setTimeout(resolve, COMMAND_REPLY_INTERVAL_MS))
    if (acceptedAt === null) {
      const reply = await commands.readPatronusCommandReply(replyParams)
      if (reply.status === "error") {
        throw commandFailure(reply.error)
      }
      if (reply.data.status === "rejected") {
        // the bot reacts first and comments right after: one more look for the reason
        if (reply.data.reason !== null || rejectedOnce) {
          throw new Error(reply.data.reason ?? "Patronus did not start the robot")
        }
        rejectedOnce = true
        continue
      }
      if (reply.data.status !== "accepted") {
        continue
      }
      acceptedAt = Date.now()
    }
    const robot = await lookUp()
    if (robot !== undefined) {
      return { robot, commentUrl: posted.data.url, listed: Promise.resolve(robot) }
    }
  }
  if (acceptedAt === null) {
    throw new Error(`Patronus did not answer the comment within a minute: see pull request #${pullRequestNumber}`)
  }
  if (start === null) {
    return { robot: null, commentUrl: posted.data.url, listed: Promise.resolve(null) }
  }
  const pending: CommentStart = { robot: null, commentUrl: posted.data.url, listed: lookUpLate(lookUp) }
  lateCommentStarts.set(late, pending)
  void pending.listed.then(() => {
    if (lateCommentStarts.get(late) === pending) {
      lateCommentStarts.delete(late)
    }
  })
  return pending
}

/**
 * Looks for the robot of an accepted comment every `LATE_ROBOT_INTERVAL_MS` for `LATE_ROBOT_WAIT_MS`, until listed;
 * resolves to it, null when none came
 */
async function lookUpLate(lookUp: () => Promise<PatronusRobot | undefined>): Promise<PatronusRobot | null> {
  const until = Date.now() + LATE_ROBOT_WAIT_MS
  while (Date.now() < until) {
    await new Promise(resolve => setTimeout(resolve, LATE_ROBOT_INTERVAL_MS))
    const robot = await lookUp()
    if (robot !== undefined) {
      return robot
    }
  }
  return null
}

/**
 * The branch Patronus pushes into for the baseline of the selected repository; null when the baseline is no branch of
 * origin. Throws what failed: a failed read is no "the baseline is none"
 */
export async function readPatronusTargetBranch(): Promise<string | null> {
  const { repositoryPath } = patronusRequest()
  const baselineBranch = branchSyncStore.getState().baselineBranch
  if (baselineBranch === null) {
    return null
  }
  const result = await commands.getPatronusTargetBranch({ repositoryPath, baselineBranch })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  return result.data
}

async function robotAction(command: typeof commands.restartPatronusRobot, robot: PatronusRobot, request: VcsRequest | null = null, replaced = false): Promise<PatronusRobot> {
  const { repositoryPath } = request ?? patronusRequest()
  const key = robotsKeyOf(repositoryPath)
  const result = await command({ repositoryPath, robotId: robot.id, replaced })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  upsertPatronusRobot(result.data, key)
  return result.data
}

/**
 * Starts the robot again on the newest commit of its branch, in the repository of `request` (by default the selected
 * one: a restart after a push restarts where the push went, also after a switch); resolves to the new robot, throws
 * what Patronus said
 */
export function restartPatronusRobot(robot: PatronusRobot, request: VcsRequest | null = null): Promise<PatronusRobot> {
  return robotAction(commands.restartPatronusRobot, robot, request)
}

/**
 * Asks Patronus to cancel the robot of the repository (by default the selected one); resolves to it canceling, throws
 * what Patronus said. `replaced`: the robot a start or a push replaces or restarts, which resolves to it as listed when
 * it finished meanwhile (nothing left to cancel) instead of throwing "not running"
 */
export function cancelPatronusRobot(robot: PatronusRobot, request: VcsRequest | null = null, replaced = false): Promise<PatronusRobot> {
  return robotAction(commands.cancelPatronusRobot, robot, request, replaced)
}

/** The current status of the robot (one row read), stored; null when Patronus can't answer */
export async function fetchPatronusRobotStatus(robot: PatronusRobot): Promise<PatronusRobotStatus | null> {
  const result = await commands.getPatronusRobotStatus({ ...patronusRequest(), robotId: robot.id })
  if (result.status === "error") {
    return null
  }
  if (result.data !== robot.status) {
    upsertPatronusRobot({ ...robot, status: result.data })
  }
  return result.data
}

/** Forgets the data (tests) */
export function resetPatronus(): void {
  problemsInFlight.clear()
  landingSettled.clear()
  lateCommentStarts.clear()
  failingInFlight.clear()
  resetOsNotification()
  announcedMerges.clear()
  askedView = null
  lastFacts = NO_FACTS
  lastFactsOf = { view: null, robots: null }
  patronusStore.setState({ repositories: {} })
}
