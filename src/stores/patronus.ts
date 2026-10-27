import { useMemo } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type {
  BranchLanding,
  Fact,
  FactState,
  LandedPullRequest,
  LandingRefusal,
  PatronusAvailability,
  PatronusPart,
  PatronusRobot,
  PatronusStartOptions,
  PatronusView,
  RepositoryView,
  RobotOf,
  RobotRestart,
  RobotsChange,
  RobotsView,
  StackLanding,
  TableChanges,
} from "@/lib/bindings"
import { getErrorDetails, notifyError } from "@/lib/errors"
import type { VcsRequest } from "@/lib/git/vcs-request"
import { showMergeToast } from "@/lib/landing-toast"
import { announce, resetOsNotification } from "@/lib/os-notification"
import type { LandedBy, LandingAncestor, LandingDescendant, PushHold } from "@/lib/patronus"
import { formatLandedWholeReason, formatMergedRobotTitle, formatPushMode, isHoldingRobot, isLandedWhole, isRobotActive } from "@/lib/patronus"
import { withoutKeys } from "@/lib/record"
import type { RobotEstimate } from "@/lib/robot-eta"
import { estimateRobotDuration, robotSample } from "@/lib/robot-eta"
import type { RowRobots } from "@/lib/robot-overview"
import { NO_ROW_ROBOTS } from "@/lib/robot-overview"
import { failedEffects, runFailure, runIntent } from "@/lib/run-intent"
import { describeOrphanedStackPullRequest } from "@/lib/whole-stack"
import { watchRobotInDemand } from "@/model/demand"
import { refreshSessionFacts } from "@/model/store"
import { describeLandedCloses } from "@/presenters/news"
import { recordRobot } from "@/stores/activity"
import type { BranchSyncState } from "@/stores/branch-sync"
import { branchSyncStore, getStackAncestors, getStackBranch, structureIn, whenSynced } from "@/stores/branch-sync"
import { rememberLandedStackRoot } from "@/stores/landed-stacks"
import { getStackPullRequest, refreshPullRequests } from "@/stores/pull-requests"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"
import type { RobotHistoryEntry } from "@/stores/robot-history"
import { historySamples, recordFinishedRobots, robotHistoryStore } from "@/stores/robot-history"

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
   * What the robots land, landed and hold of the branch table of the last sync, with the robot of each row and the
   * landing needs of each branch, as the session derives it for `bd` too (the part `patronus` of its view); null while
   * none is derived (Patronus has not answered, or the derivation failed without an earlier view)
   */
  readonly view: PatronusView | null
  /**
   * How the view stands (`PatronusPart.state`): `loading` until the view of the first answer of Patronus is in, `stale`
   * while one of newer robots, a newer table or a push is derived (no row waits for it), `unavailable` when it failed
   * without an earlier view
   */
  readonly viewState: FactState
  /** How the landing needs of the view stand (`PatronusPart.landingNeeds`): `loading` while Rust computes them */
  readonly landingNeedsState: FactState
  /**
   * By robot id, the failed robots whose problems Patronus didn't give (`loadReviewProblems` failed), with why: why they
   * failed stays unknown, so the step of the branch offers no restart (`UnavailableFact`); asked again after the next
   * answer of a list or a poll, and by Check Approvals Again (`askRobotProblemsAgain`)
   */
  readonly problemsFailed: Readonly<Record<string, string>>
}

export interface PatronusState {
  /** By `repositoryKey`; in memory only, Patronus is asked again after a restart */
  readonly repositories: Readonly<Record<string, RepositoryPatronus>>
  /**
   * By `repositoryKey`, the merge profiles of the target the baseline pushes to (`facts.profiles` of the session), which
   * a start and its popover ask for (`askStartProfiles`), without the default of the last robot
   */
  readonly profiles: Readonly<Record<string, Fact<PatronusStartOptions>>>
}

export const patronusStore = createStore<PatronusState>(() => ({ repositories: {}, profiles: {} }))

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
  viewState: { kind: "loading" },
  landingNeedsState: { kind: "loading" },
  problemsFailed: {},
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

/**
 * The robot of the row of `branchName`, as the session matched it (`robotId` of the view, `domain_policy::robot_match`:
 * started on the branch, or of a temporary ref at its tip), with what the page knows of it now; re-renders only when it
 * changes
 */
export function useBranchRobot(branchName: string): PatronusRobot | undefined {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => branchRobotIn(state.repositories[key], branchName))
}

/**
 * The robot of the whole stack of `root`, on its stack branch `{prefix}/stack/<root>`, as the session matched it;
 * re-renders only when it changes
 */
export function useStackRobot(root: string): PatronusRobot | undefined {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => stackRobotIn(state.repositories[key], root))
}

/** The robot `id` of the selected repository as the session listed it; undefined when it lists none of that id */
export function getRobot(id: string): PatronusRobot | undefined {
  return currentRepository()?.robots[id]
}

/** Non-hook variant of `useStackRobot`, for menus built on open */
export function getStackRobot(root: string): PatronusRobot | undefined {
  return stackRobotIn(currentRepository(), root)
}

/** The robots of the rows of each view (`rowRobotsOf`), computed once per view */
const rowRobotsOfView = new WeakMap<PatronusView, RowRobots>()

/**
 * The robot of each row of the selected repository as the session matched it (`robotId` of the view), for the landing
 * overview; the same object while the view is
 */
export function useRowRobots(): RowRobots {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => rowRobotsOf(state.repositories[key]?.view ?? null))
}

function rowRobotsOf(view: PatronusView | null): RowRobots {
  if (view === null) {
    return NO_ROW_ROBOTS
  }
  let known = rowRobotsOfView.get(view)
  if (known === undefined) {
    const ids = <T extends { readonly robotId: string | null }>(entries: Readonly<Record<string, T>>) =>
      Object.fromEntries(Object.entries(entries).flatMap(([name, facts]) => (facts.robotId === null ? [] : [[name, facts.robotId] as const])))
    known = { branches: ids(view.branches), stacks: ids(view.stacks) }
    rowRobotsOfView.set(view, known)
  }
  return known
}

function branchRobotIn(repository: RepositoryPatronus | undefined, branchName: string): PatronusRobot | undefined {
  const id = repository?.view?.branches[branchName]?.robotId
  return id == null ? undefined : repository!.robots[id]
}

function stackRobotIn(repository: RepositoryPatronus | undefined, root: string): PatronusRobot | undefined {
  const id = repository?.view?.stacks[root]?.robotId
  return id == null ? undefined : repository!.robots[id]
}

/**
 * What the robots land, landed and hold of the branches, by branch, as the session's view says it (`view`, the rule
 * `bd` reads too), each fact with its robot object: the same objects while nothing of them changes, so a row that reads
 * one renders again only when it does. A fact whose robot no longer does what the view saw (a hold of a robot that
 * stopped running, a landing of a robot no longer merged) waits for the next view, which the change of the robots
 * derives.
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
    const hold = facts.landing.hold
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
    const stackHold = facts.landing.hold
    const stackRobot = stackHold?.code === "stackHeld" ? robotOf(stackHold.robotId) : undefined
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
  return structureIn(branchSyncStore.getState(), branchName).ancestors.filter(ancestor => !isLandedWhole(getLandedBy(ancestor))).length + 1
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

const NO_BRANCH_LANDING: BranchLanding = { hold: null, start: null, dryRun: null, replaces: null, push: null }
const NO_STACK_LANDING: StackLanding = {
  hold: null,
  start: null,
  dryRun: null,
  replaces: null,
  rest: { members: [], commitCount: 0, landed: [], by: [], byOwnRobot: false },
}

function branchLandingIn(repository: RepositoryPatronus | undefined, branchName: string): BranchLanding {
  return repository?.view?.branches[branchName]?.landing ?? NO_BRANCH_LANDING
}

function stackLandingIn(repository: RepositoryPatronus | undefined, root: string): StackLanding {
  return repository?.view?.stacks[root]?.landing ?? NO_STACK_LANDING
}

/**
 * What the rules decide of the branch over its Patronus facts (`BranchLanding`, `domain_policy::holds` and `land`): what
 * holds its push, why its Safe Push or Dry Run can't start, the robot a Safe Push start replaces, why the push an action
 * needs can't be made; nothing before the session derived a view
 */
export function useBranchLanding(branchName: string): BranchLanding {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => branchLandingIn(state.repositories[key], branchName))
}

/** Non-hook variant of `useBranchLanding`, for menus built on open, the next step and the push itself */
export function getBranchLanding(branchName: string): BranchLanding {
  return branchLandingIn(currentRepository(), branchName)
}

/**
 * What the rules decide of the whole stack of `root` (`StackLanding`): what holds the push of its stack branch (a
 * parked member, every branch landed, its own robot, a build the sync did not make), why its starts can't, the robot a
 * Safe Push start replaces
 */
export function useStackLanding(root: string): StackLanding {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => stackLandingIn(state.repositories[key], root))
}

/** Non-hook variant of `useStackLanding`, for menus built on open and the push itself */
export function getStackLanding(root: string): StackLanding {
  return stackLandingIn(currentRepository(), root)
}

/**
 * The restart a robot of the user offers (`RobotRestart`, `domain_policy::holds::restart_action`): a restart, one that
 * pushes first, and why it can't start now; null for none
 */
export function useRobotRestart(robotId: string | undefined): RobotRestart | null {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => (robotId === undefined ? null : (state.repositories[key]?.view?.restarts[robotId] ?? null)))
}

/**
 * The view of `repository` is of its robots, its table and its remote branches as they are now: no derivation of
 * newer ones runs (a failed one is an answer too). True before Patronus answered, and while it lists no robot
 */
function isViewCurrentIn(repository: RepositoryPatronus | undefined): boolean {
  if (repository === undefined || Object.keys(repository.robots).length === 0) {
    return true
  }
  const state = repository.viewState
  return state.kind === "fresh" || state.kind === "unavailable" || (state.kind === "stale" && state.reason != null)
}

/**
 * The view of the selected repository is of its robots as they are now (or its last derivation failed): what the
 * robots hold and land is known, not the lag of a change of them. True without Patronus.
 */
export function useRepositoryViewCurrent(): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => isViewCurrentIn(state.repositories[key]))
}

/**
 * Resolves once the view of the session is of the robots, the remote branches and the table as the sync leaves them
 * (the session derives it again after a sync, a change of the robots and a push): a push and a start decide on its
 * holds after it. Nothing changed: at once; another repository selected meanwhile: at once
 */
export async function whenRepositoryViewCurrent(): Promise<void> {
  await whenSynced()
  const key = currentKey()
  if (key === null) {
    return
  }
  const current = () => key !== currentKey() || isViewCurrentIn(patronusStore.getState().repositories[key])
  if (current()) {
    return
  }
  await new Promise<void>(resolve => {
    const unsubscribe = patronusStore.subscribe(() => {
      if (current()) {
        unsubscribe()
        resolve()
      }
    })
  })
}

/**
 * What the Safe Push of the branch needs of a branch outside what it lands, as the session computes it after a sync
 * (`landingNeeds` of its view): the refusals the landing gate of its start would give, empty when it needs nothing;
 * null while unknown (not computed yet, or the computation failed), which is never "nothing"
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
 * Rust computes what the Safe Push of the branch needs now (the landing needs of the view `loading`, its needs
 * unknown): its step waits for the answer, a second or two after the view (`PendingFact` `landingNeeds`); a failed
 * computation waits for nothing
 */
export function isLandingNeedsPending(branchName: string): boolean {
  const repository = currentRepository()
  return repository?.view != null && repository.landingNeedsState.kind === "loading" && landingNeedsIn(repository, branchName) === null
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
  return branchRobotIn(currentRepository(), branchName)
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
 * `key` before the session's patch has it; the session tells a merge it sees
 */
function storeRobot(key: string, robot: PatronusRobot): void {
  const previous = patronusStore.getState().repositories[key]?.robots ?? EMPTY.robots
  const robots = withRobots(previous, [robot], [])
  if (robots === previous) {
    return
  }
  update(key, () => ({ robots }))
  recordFinishedRobots(key.slice(0, key.lastIndexOf("\n")), Object.values(robots))
}

// ---- merges ----

/** The merged robots already told about, once each */
const announcedMerges = new Set<string>()

/**
 * A robot merged a branch or the whole stack ("Whole air stack merged into master"), as the news of the session says
 * (`tellRobotNews`), once: a toast with Update Project and "Open in Patronus", which follows the merge until the update
 * lands it (`showMergeToast`). While no window is focused, an OS notification
 * says it at once (unless Settings turned it off), and the toast waits for the focus.
 * A stacked branch landed its ancestors too, the whole stack every branch of it: the open pull requests its landing
 * closes come with the news (`closes`, `domain_policy::landed::pull_requests_to_close`), which the session tells once
 * the view of the robots it merged in stands, so the toast names what it landed (`landedByRobot` of that view) at once
 * and what it closes (`describeLandedCloses`); the session of the robot's repository closes them
 * (`CloseLandedPullRequests`, the comment of each Rust's), whichever repository the main window shows meanwhile.
 */
export function announceMerge(key: string, robot: PatronusRobot, closes: readonly LandedPullRequest[]): void {
  if (announcedMerges.has(robot.id)) {
    return
  }
  announcedMerges.add(robot.id)
  if (robot.pullRequest != null) {
    // Patronus closed the pull request with the label `merged`
    void refreshPullRequests("afterWrite")
  }
  const [repositoryPath] = key.split("\n") as [string, string]
  const landed = patronusStore.getState().repositories[key]?.view?.landedByRobot[robot.id] ?? []
  if (closes.length > 0) {
    void closeLanded(repositoryPath, robot.id)
  }
  // the stacks of the table are of the shown repository: only a merge there tells a pull request of a whole stack that no longer matches it
  const closed = [describeLandedCloses(robot, closes), currentKey() === key ? rememberLandedRoot(robot) : null].filter((part): part is string => part !== null)
  const description = closed.length === 0 ? formatPushMode(robot.pushMode) : `${formatPushMode(robot.pushMode)} · ${closed.join(" · ")}`
  const title = formatMergedRobotTitle(robot, patronusStore.getState().repositories[key]?.watched[robot.id])
  recordRobot(title, description, robot, repositoryPath)
  // the toast's button picks it up (Update Project fetches first): the notification says what merged, as the toast
  const notification = { title, body: description }
  const landedBranches = landed.map(branch => branch.branch)
  announce("patronusMerges", notification, () => showMergeToast(repositoryPath, robot, title, description, landedBranches))
}

/**
 * Closes the pull requests the merged robot `robotId` landed with it, in the session of `repositoryPath`, the robot's
 * repository (`CloseLandedPullRequests`: which, and the comment of each, are Rust's); each close that failed is an error
 * toast "Cannot close pull request #5". A session that lists the robot no longer closes nothing
 */
async function closeLanded(repositoryPath: string, robotId: string): Promise<void> {
  const run = await runIntent(repositoryPath, { kind: "closeLandedPullRequests", robotId })
  if (run.kind === "ran") {
    for (const { effect, error } of failedEffects(run.outcome)) {
      notifyError(effect.kind === "closePullRequest" ? `Cannot close pull request #${effect.number}` : "Cannot close the pull requests the Safe Push landed", error)
    }
  } else if (run.kind !== "noPlan") {
    notifyError("Cannot close the pull requests the Safe Push landed", runFailure(run, ""))
  }
}

/**
 * A merged robot of a branch landed the root of its stack: an open pull request of the whole stack no longer matches it
 * (`rememberLandedStackRoot`); the user decides on it, nothing closes it. What the toast says of it ("#12 no longer
 * matches the air stack: oauth landed"), null when there is none.
 */
function rememberLandedRoot(robot: PatronusRobot): string | null {
  const name = robot.branchName
  if (name === null || robot.pushMode === "dryRun") {
    return null
  }
  const root = getStackAncestors(name)[0] ?? name
  const pullRequest = getStackPullRequest(root)
  if (pullRequest?.state !== "open") {
    return null
  }
  rememberLandedStackRoot(root, name)
  return describeOrphanedStackPullRequest(root, pullRequest, getStackBranch(root), name)
}

// ---- the robots of the session ----

/**
 * The facts of the session's view of the main window (`onSessionFacts`): the robots and the Patronus view the session
 * derives of them, when a patch has them
 */
export function applySessionFacts(repositoryPath: string, view: RepositoryView, changes: TableChanges | null): void {
  if (changes === null || changes.facts?.robots != null) {
    applyRobotsFacts(repositoryPath, view.facts.robots, changes?.facts?.robots ?? null)
  }
  if (changes === null || changes.patronus != null) {
    applyPatronusPart(repositoryPath, view.patronus)
  }
  const key = repositoryKey(repositoryPath, getEffectiveBranchPrefix())
  if ((changes === null || changes.facts?.profiles != null) && patronusStore.getState().profiles[key] !== view.facts.profiles) {
    patronusStore.setState(state => ({ profiles: { ...state.profiles, [key]: view.facts.profiles } }))
  }
}

/** The merge profiles of the selected repository (`PatronusState.profiles`); null before its session's first view */
export function getStartProfiles(): Fact<PatronusStartOptions> | null {
  const key = currentKey()
  return key === null ? null : (patronusStore.getState().profiles[key] ?? null)
}

/** `getStartProfiles` for a component */
export function useStartProfiles(): Fact<PatronusStartOptions> | null {
  const key = repositoryKey(useSelectedProjectPath() ?? "", useEffectiveBranchPrefix())
  return useStore(patronusStore, state => state.profiles[key] ?? null)
}

/**
 * Asks the session for the merge profiles when it has none, the last read failed, is a minute old or of another
 * baseline (`domain_policy::refresh::profiles_due`); resolves once they are in the store, at once when they are current
 */
export async function askStartProfiles(): Promise<void> {
  const key = currentKey()
  if (key !== null) {
    await refreshSessionFacts(patronusRequest().repositoryPath, "profiles", "afterWrite")
  }
}

/**
 * The Patronus view of the session of `repositoryPath` (`patronus` of its view): what the robots land, landed and hold
 * of the table, the robot of each row and the landing needs, as Rust derives them, with how they stand; the page only
 * stores them. The patch keeps every branch and stack it doesn't name as the same object, so a row that reads one
 * doesn't render again. Nothing is stored for a session Patronus has not answered yet
 */
function applyPatronusPart(repositoryPath: string, part: PatronusPart): void {
  const key = repositoryKey(repositoryPath, getEffectiveBranchPrefix())
  const entry = patronusStore.getState().repositories[key]
  if (entry === undefined && part.state.kind === "loading") {
    return
  }
  const derived = part.state.kind !== "loading" && part.state.kind !== "unavailable"
  const view = derived ? part.view : null
  if (entry !== undefined && entry.view === view && entry.viewState === part.state && entry.landingNeedsState === part.landingNeeds) {
    return
  }
  update(key, () => ({ view, viewState: part.state, landingNeedsState: part.landingNeeds }))
}

/**
 * The robots of the session of `repositoryPath` as its view has them now (`facts.robots`), with the patch that brought
 * them (`change`; null for a snapshot, which replaces them): the session lists and polls them by its rules
 * (`remote_facts::scheduler`) and asks which running ones will fail and why the failed ones of the table failed; the
 * page only stores them. A patch replaces the robots it names, the merge profile kept when it names none, and removes
 * those it drops; a robot it doesn't name keeps its object, so the chips of other branches don't render again. The
 * failing ones are the session's: those a list's answer and the details of a running robot said will fail
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
  // stored from the first answer on, and their view comes from then
  const answered = head.value !== null && (change === null || change.head != null)
  const asked = change !== null && (change.failing != null || change.needsReview != null || change.problemsFailed != null)
  if (robots === entry.robots && !answered && !asked) {
    return
  }
  const failing = change === null || change.failing != null ? failingOf(view.failing, entry.failing) : entry.failing
  update(key, previous => ({
    robots,
    availability,
    githubRepository: head.value?.githubRepository ?? null,
    fetchedAt: head.observedAt * 1000,
    listed,
    failing,
    needsReview: change === null ? view.needsReview : (change.needsReview ?? previous.needsReview),
    problemsFailed: change === null ? view.problemsFailed : (change.problemsFailed ?? previous.problemsFailed),
  }))
  if (robots !== entry.robots) {
    // what the estimates learn from, also after a restart
    recordFinishedRobots(repositoryPath, Object.values(robots))
  }
}

/** The failing robots of the session by id, the object kept while they are the same */
function failingOf(session: readonly string[], kept: Readonly<Record<string, true>>): Readonly<Record<string, true>> {
  const same = session.length === Object.keys(kept).length && session.every(id => kept[id] === true)
  return same ? kept : Object.fromEntries(session.map(id => [id, true] as const))
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

/** The robots of the branches and of the whole stacks of the table: those whose problems the session asks */
function tableRobots(entry: RepositoryPatronus): Array<PatronusRobot | undefined> {
  const { branches, stackBranches } = branchSyncStore.getState()
  return [
    ...Object.keys(branches).map(name => branchRobotIn(entry, name)),
    // the robot of the whole stack fails for missing approvals in the pull request of the stack
    ...Object.keys(stackBranches).map(root => stackRobotIn(entry, root)),
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
  const repositoryPath = getSelectedProject()?.path
  if (repositoryPath !== undefined) {
    void refreshSessionFacts(repositoryPath, "robotProblems", "manual")
  }
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

/**
 * Patronus answered a list of the robots of the selected repository once: a failure after it (an error, offline) leaves
 * the steps on the robots it listed, as a failed refresh of GitHub keeps the last listing
 */
export function hasPatronusListed(): boolean {
  return currentRepository()?.listed ?? false
}

/**
 * Why Rust didn't derive the view of the robots of the selected repository, while no earlier view stands for it
 * (`unavailable`): what they hold, land and need is unknown, so no start is decided on "none"; null once a view is in
 */
export function getPatronusViewFailure(): string | null {
  return viewFailureIn(currentRepository())
}

/** `getPatronusViewFailure` of the selected repository, for a component */
export function usePatronusViewFailure(): string | null {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => viewFailureIn(state.repositories[key]))
}

function viewFailureIn(entry: RepositoryPatronus | undefined): string | null {
  const state = entry?.viewState
  return state?.kind === "unavailable" ? getErrorDetails(state.reason) : null
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
 * lists robots and the view the session derives of the robots it listed first isn't in, so what they hold and land is
 * not known. A failure (not logged in, an error) is an answer, and so is a view that failed
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
  return status === undefined || (status === "available" && entry!.viewState.kind === "loading")
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

/** Forgets the data (tests) */
export function resetPatronus(): void {
  resetOsNotification()
  announcedMerges.clear()
  lastFacts = NO_FACTS
  lastFactsOf = { view: null, robots: null }
  patronusStore.setState({ repositories: {}, profiles: {} })
}
