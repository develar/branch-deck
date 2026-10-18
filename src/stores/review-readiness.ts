import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { ApprovalNews } from "@/lib/approval-news"
import { approvalNewsKey, findApprovalNews } from "@/lib/approval-news"
import type { PatronusReviewRequirement, PullRequest, ReviewReadiness, ReviewReadinessBranch } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { rememberReviewGroups } from "@/lib/branch-review-window"
import { getPullRequestStart } from "@/lib/patronus"
import { waitingReviewers } from "@/lib/pull-request"
import { baselineOnRemote } from "@/lib/stack"
import { stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { branchSyncStore, getBranch, getBranchTableOrder, getStackBranch } from "@/stores/branch-sync"
import { currentKey, getPatronusGitHubRepository, isPatronusAvailable, patronusStore, repositoryKey } from "@/stores/patronus"
import { getPullRequest, getStackPullRequest, pullRequestsStore, refreshPullRequests } from "@/stores/pull-requests"
import { getFullStackBranchName, useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"

/** The most branches one `get_branch_review_readiness` call asks: Patronus checks them one after another */
export const READINESS_BATCH_SIZE = 8
/** An answer that Patronus didn't give (unavailable) is asked again after this, on the next change of the stores */
export const READINESS_RETRY_MS = 5 * 60_000

/** The readiness of a branch, the answer of Patronus for one input (its tip, the start and the reviews) */
export interface ReadinessEntry {
  /** The input it answers (`readinessInputOf`) */
  readonly key: string
  readonly readiness: ReviewReadiness
  /** What Patronus answered; null when it didn't */
  readonly requirement: PatronusReviewRequirement | null
  /** The branch moved on (another tip, a new review, Check Approvals Again): shown until the new answer, never acted on by the form */
  readonly stale: boolean
  /** `Date.now()` of the answer */
  readonly answeredAt: number
  /** The tip the answer is about */
  readonly tip: string
  /** The pull request whose approvals it counts; null for a start through the API */
  readonly pullRequest: number | null
  /** The reviewers GitHub waited for at the answer: the news of an approval while Patronus can't judge (`findApprovalNews`) */
  readonly waitingFor: readonly string[]
  /** The reviewers whose latest review approved, at the answer */
  readonly approvedBy: readonly string[]
}

export interface ReviewReadinessState {
  /** By repository key, then by branch; in memory, the backend keeps the answers on disk */
  readonly repositories: Readonly<Record<string, Readonly<Record<string, ReadinessEntry>>>>
}

export const reviewReadinessStore = createStore<ReviewReadinessState>(() => ({ repositories: {} }))

const NO_ENTRIES: Readonly<Record<string, ReadinessEntry>> = {}

// ---- reading ----

/** The readiness of the branch, a stale one included; undefined until asked */
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
 * its reviews changed and the scan hasn't marked it yet); null while one loads
 */
export function freshReadiness(entry: ReadinessEntry | undefined, branchName: string): ReadinessEntry | null {
  return entry === undefined || entry.stale || readinessInputOf(branchName)?.key !== entry.key ? null : entry
}

// ---- the input ----

/** The branch Patronus pushes into, as a pull request names its base ("master" of "origin/master") */
export function getReadinessTarget(): string {
  return baselineOnRemote(branchSyncStore.getState().baselineBranch ?? "")
}

/** An input of the command with the key of its answer */
interface ReadinessInput {
  readonly key: string
  readonly branch: ReviewReadinessBranch
}

/**
 * The input the readiness of a branch answers: its tip, and how the robot would start: from the open pull request into
 * the target (`getPullRequestStart`), whose reviewers and author Patronus evaluates, else through the API (no pull
 * request, one into another base), which counts no approval. Null without a tip. A key of a whole stack
 * (`stackReadinessKey`) is its stack branch with the pull request of the whole stack, while the sync built it.
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
  return { key: JSON.stringify([rest.tip, branch.start, reviewsOf(pullRequest, branch)]), branch }
}

function reviewsOf(pullRequest: PullRequest | undefined, branch: ReviewReadinessBranch): unknown {
  return branch.start.kind === "rest" || pullRequest === undefined ? null : [pullRequest.author, pullRequest.reviewers]
}

// ---- asking ----

/** The branches a surface needs the readiness of now (the Safe Push popover, the "Create pull request" dialog), with a count each */
const demanded = new Map<string, number>()
/**
 * The branches whose next step a surface shows now (`showReviewReadiness`), with a count each: a row on the screen (not
 * one a collapsed stack, side stack or island hides: it isn't rendered), the header of an island, the preview, the
 * review window. Asked only while their step depends on the readiness (`wantedOf`).
 */
const shown = new Map<string, number>()
/** The branches whose next answer asks Patronus again (`recheck`), by `recheckKey`: of each repository its own */
const rechecks = new Set<string>()

function recheckKey(repository: string, branchName: string): string {
  return `${repository}\n${branchName}`
}
/** Of the shown branches (`askedOrder`), those whose next step depends on the readiness (`needsReadiness`); set by the watch */
let wantedOf: () => readonly string[] = () => []
/** Told when the approvals of a branch arrived (`findApprovalNews`), once each; set by the watch of the main window */
let approvalListener: (news: ApprovalNews) => void = () => {}
/** The approvals told this session (`approvalNewsKey`) */
const toldApprovals = new Set<string>()
/** By `recheckKey`, the last answer Patronus gave (not unavailable): a failure between two answers hides no approval */
const lastAnswered = new Map<string, ReadinessEntry>()
let inFlight = false
let scanTimer: ReturnType<typeof setTimeout> | undefined

function update(key: string, patch: (entries: Readonly<Record<string, ReadinessEntry>>) => Readonly<Record<string, ReadinessEntry>>): void {
  reviewReadinessStore.setState(state => {
    const entries = state.repositories[key] ?? NO_ENTRIES
    const next = patch(entries)
    return next === entries ? state : { repositories: { ...state.repositories, [key]: next } }
  })
}

/** Whether the branch needs a new answer for `input`: none yet, another input, a recheck, or Patronus didn't answer a while ago */
function isDue(repository: string, entry: ReadinessEntry | undefined, input: ReadinessInput, now: number): boolean {
  if (entry === undefined || entry.key !== input.key || rechecks.has(recheckKey(repository, input.branch.branchName))) {
    return true
  }
  return entry.readiness.kind === "unavailable" && now - entry.answeredAt >= READINESS_RETRY_MS
}

/**
 * The branches the watch may ask, in table order: the shown ones, and those whose last answer waits for an approval in
 * their pull request while nothing shows them (a collapsed stack): a new review of it asks again, so its approval is
 * news. A hidden branch is asked once it shows; its answer is kept meanwhile, marked stale when it shows with another
 * input. The whole stack of a root (`stackReadinessKey`, the header of its island) comes right before its root.
 */
function askedOrder(key: string): string[] {
  const entries = reviewReadinessStore.getState().repositories[key] ?? NO_ENTRIES
  const asked = (name: string) => shown.has(name) || (entries[name]?.readiness.kind === "missing" && entries[name].pullRequest !== null)
  return getBranchTableOrder().flatMap(name => {
    const stack = stackReadinessKey(name)
    return [stack, name].filter(asked)
  })
}

/**
 * Asks for the next batch, if none is in flight: the rechecks first (one call with `recheck`), then the branches a
 * surface demands, then the shown ones whose step needs it, in table order (`askedOrder`), up to
 * `READINESS_BATCH_SIZE`. An entry whose input changed is marked stale at once, so no surface acts on it while its
 * answer loads. Repeats until nothing is due.
 */
function scan(): void {
  scanTimer = undefined
  const key = currentKey()
  if (key === null || !isPatronusAvailable()) {
    return
  }
  const entries = reviewReadinessStore.getState().repositories[key] ?? NO_ENTRIES
  const now = Date.now()
  const prefix = recheckKey(key, "")
  const recheckNames = [...rechecks].filter(it => it.startsWith(prefix)).map(it => it.slice(prefix.length))
  // a branch without a tip any more (gone, emptied) has nothing to recheck: it never holds the others back
  for (const name of recheckNames) {
    if (readinessInputOf(name) === null) {
      rechecks.delete(recheckKey(key, name))
    }
  }
  const rechecked = recheckNames.filter(name => rechecks.has(recheckKey(key, name)))
  const names = rechecked.length > 0 ? rechecked : [...new Set([...demanded.keys(), ...wantedOf()])]
  const due: ReadinessInput[] = []
  for (const name of names) {
    const input = readinessInputOf(name)
    if (input !== null && isDue(key, entries[name], input, now)) {
      due.push(input)
    }
  }
  // what changed is stale now, also beyond the batch and while another batch is in flight: no surface acts on it
  update(key, current => {
    let next = current
    for (const input of due) {
      const entry = current[input.branch.branchName]
      if (entry !== undefined && !entry.stale) {
        next = { ...next, [input.branch.branchName]: { ...entry, stale: true } }
      }
    }
    return next
  })
  if (inFlight) {
    return
  }
  const batch = due.slice(0, READINESS_BATCH_SIZE)
  if (batch.length === 0) {
    return
  }
  const recheck = rechecked.length > 0
  for (const input of batch) {
    rechecks.delete(recheckKey(key, input.branch.branchName))
  }
  inFlight = true
  void ask(key, batch, recheck).finally(() => {
    inFlight = false
    scheduleReadinessScan()
  })
}

/** One call for `batch`; an answer for an input the branch no longer has is kept stale, a failure is `unavailable` */
async function ask(key: string, batch: readonly ReadinessInput[], recheck: boolean): Promise<void> {
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  const params = { repositoryPath, branchPrefix, baselineBranch: branchSyncStore.getState().baselineBranch ?? "", branches: batch.map(input => input.branch), recheck }
  let answers: { input: ReadinessInput; readiness: ReviewReadiness; requirement: PatronusReviewRequirement | null }[]
  try {
    const result = await commands.getBranchReviewReadiness(params)
    if (result.status === "error") {
      throw new Error(result.error.message)
    }
    answers = batch.map(input => {
      const answer = result.data.find(it => it.branchName === input.branch.branchName && it.tip === input.branch.tip)
      return answer === undefined
        ? { input, readiness: { kind: "unavailable", reason: "error", message: "Patronus gave no answer" }, requirement: null }
        : { input, readiness: answer.readiness, requirement: answer.requirement }
    })
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    answers = batch.map(input => ({ input, readiness: { kind: "unavailable", reason: "error", message }, requirement: null }))
  }
  const now = Date.now()
  const news: ApprovalNews[] = []
  update(key, current => {
    const next = { ...current }
    for (const { input, readiness, requirement } of answers) {
      const name = input.branch.branchName
      const stale = currentKey() !== key || readinessInputOf(name)?.key !== input.key
      // a stale answer replaces no answer of the input the branch has now
      if (stale && current[name] !== undefined && current[name].key !== input.key && !current[name].stale) {
        continue
      }
      const pullRequest = input.branch.start.kind === "pullRequest" ? input.branch.start.number : null
      const reviewers = input.branch.reviewers ?? []
      const entry: ReadinessEntry = {
        key: input.key,
        readiness,
        requirement,
        stale,
        answeredAt: now,
        tip: input.branch.tip,
        pullRequest,
        waitingFor: waitingReviewers({ reviewers }),
        approvedBy: reviewers.filter(it => it.state === "approved").map(it => it.login),
      }
      const known = current[name]?.readiness.kind === "unavailable" ? lastAnswered.get(recheckKey(key, name)) : current[name]
      const approval = stale ? null : findApprovalNews(name, known, entry)
      if (readiness.kind !== "unavailable") {
        lastAnswered.set(recheckKey(key, name), entry)
      }
      if (approval !== null && !toldApprovals.has(approvalNewsKey(approval))) {
        toldApprovals.add(approvalNewsKey(approval))
        news.push(approval)
      }
      next[name] = entry
      // a review window opened from the row shows the groups too
      if (!stale && requirement !== null) {
        rememberReviewGroups(name, requirement)
      }
    }
    return next
  })
  for (const approval of news) {
    approvalListener(approval)
  }
}

/** Asks for what is due soon: store changes come in bursts (a sync applies its events in batches) */
export function scheduleReadinessScan(): void {
  if (scanTimer === undefined) {
    scanTimer = setTimeout(scan, 0)
  }
}

/**
 * The readiness of the branch is needed while `enabled` (the Safe Push popover, the "Create pull request" dialog): asked
 * before the branches of the table, and again when its input changes
 */
export function useDemandReviewReadiness(branchName: string, enabled: boolean): void {
  useEffect(() => (enabled ? demandReviewReadiness(branchName) : undefined), [branchName, enabled])
}

/** Asks for the readiness of the branch until the returned function is called: what the hook does, outside React */
export function demandReviewReadiness(branchName: string): () => void {
  return countIn(demanded, branchName)
}

/**
 * A surface shows the next step of the branch until the returned function is called (a row, an island header, the
 * preview, the review window): the watch asks for its readiness while its step depends on it
 */
export function showReviewReadiness(branchName: string): () => void {
  return countIn(shown, branchName)
}

/** `showReviewReadiness` while the component shows the step of `branchName`; nothing for an empty name */
export function useShowReviewReadiness(branchName: string): void {
  useEffect(() => (branchName === "" ? undefined : showReviewReadiness(branchName)), [branchName])
}

/** Counts `branchName` in `counts` and asks soon, until the returned function is called (once) */
function countIn(counts: Map<string, number>, branchName: string): () => void {
  counts.set(branchName, (counts.get(branchName) ?? 0) + 1)
  scheduleReadinessScan()
  let released = false
  return () => {
    if (released) {
      return
    }
    released = true
    const count = (counts.get(branchName) ?? 1) - 1
    if (count === 0) {
      counts.delete(branchName)
    } else {
      counts.set(branchName, count)
    }
  }
}

/**
 * Check Approvals Again: the pull requests from GitHub first (a new review changes the input), then Patronus asked again
 * instead of the kept answer. The answer of the branch is stale meanwhile.
 */
export async function recheckReviewReadiness(branchName: string): Promise<void> {
  const key = currentKey()
  if (key === null) {
    return
  }
  update(key, current => (current[branchName] === undefined || current[branchName].stale ? current : { ...current, [branchName]: { ...current[branchName], stale: true } }))
  await refreshPullRequests("manual")
  rechecks.add(recheckKey(key, branchName))
  scheduleReadinessScan()
}

/**
 * Keeps the readiness of the branches current while the main window shows them: on every change of the branches, the
 * pull requests and the robots, and when a surface shows another branch, the shown branches whose next step depends on
 * it (`wanted` of `askedOrder`, in table order) are asked for what changed; `onApproval` is told when the approvals of a
 * branch arrived (once each). Returns the unsubscribe.
 */
export function watchReviewReadiness(wanted: (order: readonly string[]) => readonly string[], onApproval: (news: ApprovalNews) => void = () => {}): () => void {
  wantedOf = () => {
    const key = currentKey()
    return key === null ? [] : wanted(askedOrder(key))
  }
  approvalListener = onApproval
  const unsubscribes = [branchSyncStore, pullRequestsStore, patronusStore].map(store => store.subscribe(() => scheduleReadinessScan()))
  scheduleReadinessScan()
  return () => {
    unsubscribes.forEach(unsubscribe => unsubscribe())
    wantedOf = () => []
    approvalListener = () => {}
  }
}

/** Whether a row waits for an approval (Awaiting): a focus then asks GitHub for the pull requests at once */
export function hasAwaitingRow(isAwaiting: (name: string) => boolean): boolean {
  return getBranchTableOrder().some(isAwaiting)
}

/** Forgets every answer and request (tests) */
export function resetReviewReadiness(): void {
  demanded.clear()
  shown.clear()
  rechecks.clear()
  inFlight = false
  clearTimeout(scanTimer)
  scanTimer = undefined
  wantedOf = () => []
  approvalListener = () => {}
  toldApprovals.clear()
  lastAnswered.clear()
  reviewReadinessStore.setState({ repositories: {} })
}
