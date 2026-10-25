import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
/**
 * The code-owner review readiness of the branches and whole stacks, as the session of the repository answers it
 * (`facts.readiness` of its view, by subject: a branch, or `stackReadinessKey` of a whole stack). The session asks
 * Patronus by what the window shows and needs (`src/model/demand.ts`): a surface that needs an answer now at once,
 * the shown rows whose step waits for it, then the hidden ones that wait for approvals; the store only applies its
 * answers (`applyReadinessFacts`) and tells it which shown subjects wait (`watchReviewReadiness`).
 */
import type { FactsChanges, FactsView, Fact, PatronusReviewRequirement, PullRequest, ReviewReadiness, ReviewReadinessBranch, SubjectReadiness } from "@/lib/bindings"
import { rememberReviewGroups } from "@/lib/branch-review-window"
import { getPullRequestStart } from "@/lib/patronus"
import { baselineOnRemote } from "@/lib/stack"
import { stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { setReadinessShown, shownReadinessKeys, onShownReadinessChange } from "@/model/demand"
import { refreshSessionFacts } from "@/model/store"
import { branchSyncStore, getBranch, getBranchTableOrder, getStackBranch } from "@/stores/branch-sync"
import { askRobotProblemsAgain, currentKey, getPatronusGitHubRepository, patronusStore, repositoryKey } from "@/stores/patronus"
import { getPullRequest, getStackPullRequest, pullRequestsStore, refreshPullRequests } from "@/stores/pull-requests"
import { getEffectiveBranchPrefix, getFullStackBranchName, useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"

/** The readiness of a branch, the answer of Patronus for one input (its tip, the start and the reviews) */
export interface ReadinessEntry {
  /** The input it answers (`readinessInputOf`); null for an answer of no input: Patronus can't be asked now */
  readonly key: string | null
  readonly readiness: ReviewReadiness
  /** What Patronus answered; null when it didn't */
  readonly requirement: PatronusReviewRequirement | null
  /** The branch moved on (another tip, a new review, Check Approvals Again): shown until the new answer, never acted on by the form */
  readonly stale: boolean
  /** The tip the answer is about; null with no input */
  readonly tip: string | null
  /** The pull request whose approvals it counts; null for a start through the API */
  readonly pullRequest: number | null
}

export interface ReviewReadinessState {
  /** By repository key, then by subject, as the session answered them */
  readonly repositories: Readonly<Record<string, Readonly<Record<string, ReadinessEntry>>>>
}

export const reviewReadinessStore = createStore<ReviewReadinessState>(() => ({ repositories: {} }))

const NO_ENTRIES: Readonly<Record<string, ReadinessEntry>> = {}

// ---- reading ----

/** The readiness of the branch, a stale one included; undefined until the session answered */
export function useReviewReadiness(branchName: string): ReadinessEntry | undefined {
  const key = repositoryKey(useSelectedProjectPath() ?? "", useEffectiveBranchPrefix())
  return useStore(reviewReadinessStore, state => state.repositories[key]?.[branchName])
}

/** Non-hook variant of `useReviewReadiness`, for menus and the next step of a branch */
export function getReviewReadiness(branchName: string): ReadinessEntry | undefined {
  const key = currentKey()
  return key === null ? undefined : (reviewReadinessStore.getState().repositories[key] ?? NO_ENTRIES)[branchName]
}

/**
 * The answer for the branch as it is now: neither stale nor of another input than the branch has (its tip, its start,
 * its reviews changed and the session's patch of it hasn't come yet); null while one loads
 */
export function freshReadiness(entry: ReadinessEntry | undefined, branchName: string): ReadinessEntry | null {
  if (entry === undefined || entry.stale) {
    return null
  }
  return entry.key === null || readinessInputOf(branchName)?.key === entry.key ? entry : null
}

// ---- the input ----

/** The branch Patronus pushes into, as a pull request names its base ("master" of "origin/master") */
export function getReadinessTarget(): string {
  return baselineOnRemote(branchSyncStore.getState().baselineBranch ?? "")
}

/** An input of the readiness with the key of its answer */
interface ReadinessInput {
  readonly key: string
  readonly branch: ReviewReadinessBranch
}

/**
 * The input the readiness of a branch answers, as the session asks it: its tip, and how the robot would start: from the
 * open pull request into the target (`getPullRequestStart`), whose reviewers and author Patronus evaluates, else through
 * the API (no pull request, one into another base), which counts no approval. Null without a tip. A key of a whole
 * stack (`stackReadinessKey`) is its stack branch with the pull request of the whole stack, while the sync built it.
 */
export function readinessInputOf(branchName: string): ReadinessInput | null {
  const root = stackRootOfReadinessKey(branchName)
  if (root !== null) {
    return stackReadinessInputOf(branchName, root)
  }
  const tip = getBranch(branchName)?.commits[0]?.hash
  if (tip === undefined || tip === "") {
    return null
  }
  return inputOf({ branchName, tip, start: { kind: "rest" } }, getPullRequest(branchName))
}

/**
 * The input of the whole stack of `root`: Patronus is asked about its stack branch (`source`), the answer kept under
 * the key; only a built stack branch is what its Safe Push lands, so another state has none
 */
function stackReadinessInputOf(key: string, root: string): ReadinessInput | null {
  const stack = getStackBranch(root)
  if (stack === undefined || stack.state !== "Built" || stack.tip === null || stack.members.length === 0) {
    return null
  }
  return inputOf({ branchName: key, tip: stack.tip, start: { kind: "rest" }, source: getFullStackBranchName(root) }, getStackPullRequest(root))
}

/** `branch` started from its open pull request into the target when it has one, with the key of the answer */
function inputOf(rest: ReviewReadinessBranch, pullRequest: PullRequest | undefined): ReadinessInput {
  const start = getPullRequestStart(getPatronusGitHubRepository(), pullRequest, "rebaseAutosquash", getReadinessTarget())
  const branch: ReviewReadinessBranch =
    start === null || pullRequest === undefined
      ? rest
      : { ...rest, start: { kind: "pullRequest", number: start.number }, reviewers: pullRequest.reviewers, author: pullRequest.author }
  return { key: inputKey(branch), branch }
}

/** The key of what an ask of the readiness answers: its tip, its start and, from a pull request, its reviews */
function inputKey(branch: ReviewReadinessBranch): string {
  const reviews = branch.start.kind === "rest" ? null : [branch.author ?? null, branch.reviewers ?? []]
  return JSON.stringify([branch.tip, branch.start, reviews])
}

// ---- the answers of the session ----

/**
 * The readiness of the session's view of `repositoryPath`, by subject: all of it with a snapshot (`change` null), the
 * subjects a patch names otherwise; an entry the patch doesn't name keeps its object. A fresh answer with the groups of
 * Patronus gives them to a review window opened from the row (`rememberReviewGroups`)
 */
export function applyReadinessFacts(repositoryPath: string, facts: FactsView, change: FactsChanges | null): void {
  const key = repositoryKey(repositoryPath, getEffectiveBranchPrefix())
  const named = change === null ? Object.keys(facts.readiness) : Object.keys(change.readiness ?? {})
  const removed = change === null ? null : (change.removedReadiness ?? [])
  if (named.length === 0 && removed !== null && removed.length === 0) {
    return
  }
  const changed = new Map(named.map(subject => [subject, entryOf(facts.readiness[subject])]))
  const gone = new Set([...(removed ?? []), ...[...changed].filter(([, entry]) => entry === undefined).map(([subject]) => subject)])
  reviewReadinessStore.setState(state => {
    const kept = removed === null ? [] : Object.entries(state.repositories[key] ?? NO_ENTRIES).filter(([subject]) => !gone.has(subject) && !changed.has(subject))
    const answered = [...changed].flatMap(([subject, entry]) => (entry === undefined ? [] : [[subject, entry] as const]))
    return { repositories: { ...state.repositories, [key]: Object.fromEntries([...kept, ...answered]) } }
  })
  for (const [subject, entry] of changed) {
    if (entry !== undefined && !entry.stale && entry.requirement !== null) {
      rememberReviewGroups(subject, entry.requirement)
    }
  }
}

/**
 * The entry of a fact: none while it loads; the answer, stale while it is asked again for other inputs; `unavailable`
 * with why when the session can't ask Patronus or its read failed (a failure after an answer shows no stale answer
 * the form would wait on)
 */
function entryOf(fact: Fact<SubjectReadiness> | undefined): ReadinessEntry | undefined {
  if (fact === undefined) {
    return undefined
  }
  const { state, value } = fact
  if (state.kind === "loading" || state.kind === "unknown") {
    return undefined
  }
  const input = value === null ? null : { key: inputKey(value.ask), tip: value.ask.tip, pullRequest: value.ask.start.kind === "pullRequest" ? value.ask.start.number : null }
  const failure = state.kind === "unavailable" ? state.reason : state.kind === "stale" ? (state.reason ?? null) : null
  if (failure !== null) {
    const readiness: ReviewReadiness = { kind: "unavailable", reason: failure.code === "not_logged_in" ? "notLoggedIn" : "error", message: failure.message }
    return { key: input?.key ?? null, readiness, requirement: null, stale: false, tip: input?.tip ?? null, pullRequest: input?.pullRequest ?? null }
  }
  if (value === null || input === null) {
    return undefined
  }
  return { ...input, readiness: value.readiness, requirement: value.requirement, stale: state.kind !== "fresh" }
}

// ---- asking ----

/**
 * The shown subjects in table order (`shownReadinessKeys`), the whole stack of a root right before it: the session asks
 * those whose next step depends on the readiness (`watchReviewReadiness`)
 */
function shownOrder(): string[] {
  const shown = shownReadinessKeys()
  return getBranchTableOrder().flatMap(name => [stackReadinessKey(name), name].filter(key => shown.has(key)))
}

/**
 * Tells the session of the main window which shown subjects (`showReviewReadiness`) wait for their readiness (`wanted`
 * of them, in table order) on every change of the branches, the pull requests, the robots and the answers, once per
 * burst of changes; the session asks those. Returns the unsubscribe.
 */
export function watchReviewReadiness(wanted: (order: readonly string[]) => readonly string[]): () => void {
  let queued = false
  const tell = () => {
    queued = false
    const keys = wanted(shownOrder())
    const stacks = keys.map(stackRootOfReadinessKey).filter(root => root !== null)
    setReadinessShown(
      keys.filter(key => stackRootOfReadinessKey(key) === null),
      stacks,
    )
  }
  const schedule = () => {
    if (!queued) {
      queued = true
      queueMicrotask(tell)
    }
  }
  const unsubscribes = [branchSyncStore, pullRequestsStore, patronusStore, reviewReadinessStore].map(store => store.subscribe(schedule))
  unsubscribes.push(onShownReadinessChange(schedule))
  schedule()
  return () => {
    unsubscribes.forEach(unsubscribe => unsubscribe())
    setReadinessShown([], [])
  }
}

/**
 * Check Approvals Again: why the failed robots failed where Patronus didn't give it (`askRobotProblemsAgain`), the pull
 * requests from GitHub first (a new review changes the input), then the session asks Patronus again instead of the kept
 * answer. The answer of the branch is stale from the press on. Resolves once the session's answer is in the store
 */
export async function recheckReviewReadiness(branchName: string): Promise<void> {
  const key = currentKey()
  const repositoryPath = key?.split("\n")[0]
  if (key === null || repositoryPath === undefined) {
    return
  }
  askRobotProblemsAgain()
  reviewReadinessStore.setState(state => {
    const entries = state.repositories[key] ?? NO_ENTRIES
    const entry = entries[branchName]
    return entry === undefined || entry.stale ? state : { repositories: { ...state.repositories, [key]: { ...entries, [branchName]: { ...entry, stale: true } } } }
  })
  await refreshPullRequests("manual")
  await refreshSessionFacts(repositoryPath, "readiness", "manual", branchName)
}

/** Whether a row waits for an approval (Awaiting): a focus then asks GitHub for the pull requests at once */
export function hasAwaitingRow(isAwaiting: (name: string) => boolean): boolean {
  return getBranchTableOrder().some(isAwaiting)
}

/** Forgets every answer (tests) */
export function resetReviewReadiness(): void {
  reviewReadinessStore.setState({ repositories: {} })
}
