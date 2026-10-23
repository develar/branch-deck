import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { CreatedPullRequest, PullRequest, PullRequestAvailability, RequestedReviewers } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { isLogin } from "@/lib/create-pull-request"
import { commandFailure } from "@/lib/errors"
import type { VcsRequest } from "@/lib/git/vcs-request"
import type { GitHubRepository } from "@/lib/pull-request"
import { gitHubRepositoryOf } from "@/lib/pull-request"
import type { RefreshReason } from "@/lib/refresh-gate"
import { createRefreshGates } from "@/lib/refresh-gate"
import { stableStringify } from "@/lib/stable-stringify"
import { formatReadinessSubject, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"

/** Calls closer together than this are skipped, except right after a push and on a manual refresh */
export const MIN_REFRESH_GAP_MS = 60_000
/** On window focus, pull requests older than this are refreshed */
export const FOCUS_REFRESH_AGE_MS = 120_000
/** GitHub may show the old head for a moment after a push: one more look after this */
export const PUSH_RECHECK_DELAY_MS = 5_000

/** The pull requests of one repository and branch prefix */
export interface RepositoryPullRequests {
  /** null until the first answer */
  readonly availability: PullRequestAvailability | null
  /** By virtual branch name */
  readonly pullRequests: Readonly<Record<string, PullRequest>>
  /**
   * The pull requests of the stack branches (`{prefix}/stack/<root>`, the whole stack), by the root of the stack; never
   * a branch's (spec/branch/whole-stack.spec.md)
   */
  readonly stackPullRequests: Readonly<Record<string, PullRequest>>
  /** `Date.now()` of the last answer, 0 = never */
  readonly fetchedAt: number
  readonly isRefreshing: boolean
}

export interface PullRequestsState {
  /** By `repositoryKey`; in memory only, GitHub is asked again after a restart */
  readonly repositories: Readonly<Record<string, RepositoryPullRequests>>
}

export const pullRequestsStore = createStore<PullRequestsState>(() => ({ repositories: {} }))

const EMPTY: RepositoryPullRequests = { availability: null, pullRequests: {}, stackPullRequests: {}, fetchedAt: 0, isRefreshing: false }

function repositoryKey(repositoryPath: string, branchPrefix: string): string {
  return `${repositoryPath}\n${branchPrefix}`
}

// ---- hooks ----

function useRepositoryKey(): string {
  return repositoryKey(useSelectedProjectPath() ?? "", useEffectiveBranchPrefix())
}

/** The pull requests of the selected repository */
export function useRepositoryPullRequests(): RepositoryPullRequests {
  const key = useRepositoryKey()
  return useStore(pullRequestsStore, state => state.repositories[key] ?? EMPTY)
}

const NO_PULL_REQUESTS: RepositoryPullRequests["pullRequests"] = {}

/** The pull requests of the selected repository by branch; re-renders only when they change, not on `isRefreshing` */
export function useRepositoryPullRequestMap(): RepositoryPullRequests["pullRequests"] {
  const key = useRepositoryKey()
  return useStore(pullRequestsStore, state => state.repositories[key]?.pullRequests ?? NO_PULL_REQUESTS)
}

/** Re-renders only when the pull request of the branch changes, not on every refresh */
export function usePullRequest(branchName: string): PullRequest | undefined {
  const key = useRepositoryKey()
  return useStore(pullRequestsStore, state => state.repositories[key]?.pullRequests[branchName])
}

/** Non-hook variant of `usePullRequest`, for menus built on open */
export function getPullRequest(branchName: string): PullRequest | undefined {
  const key = currentKey()
  return key === null ? undefined : pullRequestsStore.getState().repositories[key]?.pullRequests[branchName]
}

/** The pull request of the whole stack of `root` (its stack branch); re-renders only when it changes */
export function useStackPullRequest(root: string): PullRequest | undefined {
  const key = useRepositoryKey()
  return useStore(pullRequestsStore, state => state.repositories[key]?.stackPullRequests[root])
}

/** Non-hook variant of `useStackPullRequest`, for menus built on open */
export function getStackPullRequest(root: string): PullRequest | undefined {
  const key = currentKey()
  return key === null ? undefined : pullRequestsStore.getState().repositories[key]?.stackPullRequests[root]
}

/**
 * The GitHub repository of the selected repository while GitHub lists its pull requests, null otherwise (see
 * `gitHubRepositoryOf`); for menus and toasts built on demand, e.g. a link to create a pull request
 */
export function getGitHubRepository(): GitHubRepository | null {
  const key = currentKey()
  return key === null ? null : gitHubRepositoryOf(pullRequestsStore.getState().repositories[key]?.availability ?? null)
}

/** Whether GitHub answers for the selected repository, and why not; null until the first answer */
export function usePullRequestAvailability(): PullRequestAvailability | null {
  const key = useRepositoryKey()
  return useStore(pullRequestsStore, state => state.repositories[key]?.availability ?? null)
}

/** Non-hook variant of `usePullRequestAvailability`, for menus and the readiness store */
export function getPullRequestAvailability(): PullRequestAvailability | null {
  const key = currentKey()
  return key === null ? null : (pullRequestsStore.getState().repositories[key]?.availability ?? null)
}

// ---- refresh ----

/**
 * A sync asks at most once a minute, a focus when the pull requests are older than two minutes; a push and the refresh
 * button always ask, after the request in flight if any. GitHub is never polled.
 */
const gates = createRefreshGates({
  isDue: (reason, sinceStart, sinceAnswer) => {
    switch (reason) {
      case "push":
      case "manual":
        return true
      case "sync":
        return sinceStart >= MIN_REFRESH_GAP_MS
      case "focus":
        return sinceStart >= MIN_REFRESH_GAP_MS && sinceAnswer >= FOCUS_REFRESH_AGE_MS
      case "poll":
        return false
    }
  },
  followUpReasons: new Set<RefreshReason>(["push", "manual"]),
})

function currentKey(): string | null {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const branchPrefix = getEffectiveBranchPrefix()
  return repositoryPath === "" || branchPrefix === "" ? null : repositoryKey(repositoryPath, branchPrefix)
}

function update(key: string, patch: Partial<RepositoryPullRequests>): void {
  pullRequestsStore.setState(state => ({
    repositories: { ...state.repositories, [key]: { ...(state.repositories[key] ?? EMPTY), ...patch } },
  }))
}

function sameJson(a: unknown, b: unknown): boolean {
  return a === b || stableStringify(a) === stableStringify(b)
}

/**
 * The answered pull requests, each unchanged one the object shown before, and `previous` itself when none changed:
 * every row selects its pull request, and a refresh (every sync, focus and push) answers mostly the same ones
 */
export function mergePullRequests(previous: RepositoryPullRequests["pullRequests"], incoming: RepositoryPullRequests["pullRequests"]): RepositoryPullRequests["pullRequests"] {
  const next: Record<string, PullRequest> = {}
  let changed = Object.keys(previous).length !== Object.keys(incoming).length
  for (const [branchName, pullRequest] of Object.entries(incoming)) {
    const known = previous[branchName]
    if (known !== undefined && sameJson(known, pullRequest)) {
      next[branchName] = known
    } else {
      next[branchName] = pullRequest
      changed = true
    }
  }
  return changed ? next : previous
}

/**
 * Asks GitHub for the pull requests of the selected repository, or of `repository` (the one a push went to, also after
 * a switch), if the reason allows it now (see the policy above).
 * One request per repository at a time. Never fails: what went wrong is the availability. A repository that is not on
 * GitHub is asked again only by the refresh button.
 */
export function refreshPullRequests(reason: RefreshReason, repository?: VcsRequest): Promise<void> {
  const key = repository === undefined ? currentKey() : repositoryKey(repository.repositoryPath, repository.branchPrefix)
  if (key === null) {
    return Promise.resolve()
  }
  const entry = pullRequestsStore.getState().repositories[key] ?? EMPTY
  if (entry.availability?.status === "notGitHub" && reason !== "manual") {
    return Promise.resolve()
  }
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  return gates.refresh(key, reason, entry.fetchedAt, async () => {
    update(key, { isRefreshing: true })
    // a pull request the app wrote after this point (a retarget, a close, reviewers) is newer than the answer
    const since = ++writeSeq
    try {
      const result = await commands.getPullRequests({ repositoryPath, branchPrefix })
      if (result.status === "ok") {
        const previous = pullRequestsStore.getState().repositories[key] ?? EMPTY
        const pullRequests = keepWritten(writtenKey(key, false), since, previous.pullRequests, withCreated(key, result.data.pullRequests, Date.now()))
        const stackPullRequests = keepWritten(writtenKey(key, true), since, previous.stackPullRequests, withCreated(stackKey(key), result.data.stackPullRequests ?? {}, Date.now()))
        update(key, {
          availability: sameJson(previous.availability, result.data.availability) ? previous.availability : result.data.availability,
          pullRequests: mergePullRequests(previous.pullRequests, pullRequests),
          stackPullRequests: mergePullRequests(previous.stackPullRequests, stackPullRequests),
          fetchedAt: Date.now(),
        })
      } else {
        // keep what was shown: a failed refresh does not make the pull requests go away
        update(key, { availability: { status: "error", message: result.error.message }, fetchedAt: Date.now() })
      }
    } catch (error) {
      update(key, { availability: { status: "error", message: error instanceof Error ? error.message : String(error) }, fetchedAt: Date.now() })
    } finally {
      update(key, { isRefreshing: false })
    }
  })
}

let recheckTimer: ReturnType<typeof setTimeout> | undefined

/** Counts the refreshes started and the pull requests the app wrote, in order */
let writeSeq = 0
/** By `writtenKey`, then branch (or root): the `writeSeq` of the app's last write of the pull request */
const written = new Map<string, Map<string, number>>()

/** The key of the writes of the pull requests of the branches, or of the stack branches (`stack`), of a repository */
function writtenKey(key: string, stack: boolean): string {
  return stack ? stackKey(key) : key
}

/**
 * The answer of a refresh started at `since`, with each pull request the app wrote after that kept as the store has it:
 * the search answered before the write, so its retarget, close or reviewers would come back otherwise, and the next push
 * would retarget again
 */
function keepWritten(
  writes: string,
  since: number,
  current: Readonly<Record<string, PullRequest>>,
  incoming: Readonly<Record<string, PullRequest>>,
): Readonly<Record<string, PullRequest>> {
  let result = incoming
  for (const [branchName, at] of written.get(writes) ?? []) {
    const kept = current[branchName]
    if (at > since && kept !== undefined) {
      result = { ...result, [branchName]: kept }
    }
  }
  return result
}

/** The app wrote the pull request of `branchName` (or of the whole stack of a root) now */
function noteWritten(writes: string, branchName: string): void {
  const writesOf = written.get(writes) ?? new Map<string, number>()
  writesOf.set(branchName, ++writeSeq)
  written.set(writes, writesOf)
}

/**
 * Right after a push: the pull request of the branch shows the pushed commit. If GitHub still reports the old head,
 * asks once more a little later. Both ask in the repository of the push (`repository`), also after a switch.
 */
export async function refreshPullRequestsAfterPush(branchName: string, pushedTip: string | undefined, repository: VcsRequest): Promise<void> {
  await refreshPullRequestsAfterPushes([[branchName, pushedTip]], repository)
}

/**
 * `refreshPullRequestsAfterPush` of the branches of one push, `[branch, pushed tip]`: one refresh, and one more a
 * little later if GitHub still reports an old head of any of them
 */
export async function refreshPullRequestsAfterPushes(pushed: readonly (readonly [string, string | undefined])[], repository: VcsRequest): Promise<void> {
  await refreshPullRequests("push", repository)
  const pullRequests = pullRequestsStore.getState().repositories[repositoryKey(repository.repositoryPath, repository.branchPrefix)]?.pullRequests ?? {}
  const stale = pushed.some(([branchName, pushedTip]) => {
    const pullRequest = pullRequests[branchName]
    return pullRequest !== undefined && pushedTip !== undefined && pullRequest.headRefOid !== pushedTip
  })
  if (stale) {
    clearTimeout(recheckTimer)
    recheckTimer = setTimeout(() => void refreshPullRequests("push", repository), PUSH_RECHECK_DELAY_MS)
  }
}

/**
 * Changes the base of the pull request of a branch on GitHub, and in the store at once: the toast
 * of the push that follows reads it before the next refresh. Throws what GitHub said.
 */
export async function retargetPullRequest(branchName: string, base: string): Promise<void> {
  const key = currentKey()
  const pullRequest = getPullRequest(branchName)
  if (key === null || pullRequest === undefined) {
    return
  }
  const result = await commands.retargetPullRequest({ repositoryPath: key.split("\n")[0]!, pullRequestNumber: pullRequest.number, base })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  setPullRequest(key, branchName, { ...pullRequest, baseRefName: base })
  // the retarget changes what the next step of the branch reads: GitHub is asked after the refresh in flight, if any
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  void refreshPullRequests("push", { repositoryPath, branchPrefix })
}

/**
 * Closes the pull request of a branch on GitHub with `comment`, and in the store at once; of the repository of `key`
 * (`repositoryKey`), by default the selected one. Throws what GitHub said.
 */
export async function closePullRequest(branchName: string, comment: string, key: string | null = currentKey()): Promise<void> {
  const pullRequest = key === null ? undefined : pullRequestsStore.getState().repositories[key]?.pullRequests[branchName]
  if (key === null || pullRequest === undefined) {
    return
  }
  await closeOnGitHub(key, pullRequest, comment)
  setPullRequest(key, branchName, { ...pullRequest, state: "closed" })
}

/**
 * Closes the pull request of the whole stack of `root` on GitHub with `comment` (the user asked: one that no longer
 * matches the stack is never closed by itself), and in the store at once. Throws what GitHub said.
 */
export async function closeStackPullRequest(root: string, comment: string): Promise<void> {
  const key = currentKey()
  const pullRequest = getStackPullRequest(root)
  if (key === null || pullRequest === undefined) {
    return
  }
  await closeOnGitHub(key, pullRequest, comment)
  setStackPullRequest(key, root, { ...pullRequest, state: "closed" })
}

async function closeOnGitHub(key: string, pullRequest: PullRequest, comment: string): Promise<void> {
  const result = await commands.closePullRequest({ repositoryPath: key.split("\n")[0]!, pullRequestNumber: pullRequest.number, comment })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
}

/**
 * Asks `logins` to review the open pull request of a branch on GitHub (Request Review…), or of the whole stack of a
 * root for its readiness key (`stackReadinessKey`), and marks whom GitHub asked as requested in the store at once: the
 * review readiness reads the reviews of the pull request. A refresh follows. Throws what GitHub said when it refused the
 * request as a whole.
 */
export async function requestReviewers(branchName: string, logins: readonly string[], repository: VcsRequest): Promise<RequestedReviewers> {
  // the repository the request was asked in: its pull request, also once the main window shows another repository
  const key = repositoryKey(repository.repositoryPath, repository.branchPrefix)
  const root = stackRootOfReadinessKey(branchName)
  const pullRequestOf = () => {
    const entry = pullRequestsStore.getState().repositories[key]
    return root === null ? entry?.pullRequests[branchName] : entry?.stackPullRequests[root]
  }
  const pullRequest = pullRequestOf()
  if (pullRequest === undefined) {
    throw new Error(`${formatReadinessSubject(branchName)} has no pull request`)
  }
  const result = await commands.requestPullRequestReviewers({ repositoryPath: repository.repositoryPath, pullRequestNumber: pullRequest.number, reviewers: [...logins] })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  const requested = result.data.requested
  const current = pullRequestOf() ?? pullRequest
  const reviewers = current.reviewers.map(reviewer => (requested.some(login => isLogin(login, reviewer.login)) ? { ...reviewer, requested: true } : reviewer))
  for (const login of requested) {
    if (!reviewers.some(reviewer => isLogin(reviewer.login, login))) {
      reviewers.push({ login, state: "requested", requested: true })
    }
  }
  if (root === null) {
    setPullRequest(key, branchName, { ...current, reviewers })
  } else {
    setStackPullRequest(key, root, { ...current, reviewers })
  }
  void refreshPullRequests("push")
  return result.data
}

/** GitHub's search lists a new pull request only after a while: one the app created is kept this long meanwhile */
export const CREATED_KEEP_MS = 5 * 60_000

/** The key of the pull requests of the stack branches the app created (`created`), beside those of the branches */
function stackKey(key: string): string {
  return `${key}\nstack`
}

/** By repository key, then branch: the pull requests the app created, until the search lists them */
const created = new Map<string, Map<string, { pullRequest: PullRequest; until: number }>>()

/** The answered pull requests with the ones the app created that the search doesn't list yet */
function withCreated(key: string, incoming: Readonly<Record<string, PullRequest>>, now: number): Readonly<Record<string, PullRequest>> {
  const pending = created.get(key)
  if (pending === undefined) {
    return incoming
  }
  let result = incoming
  for (const [branchName, { pullRequest, until }] of pending) {
    if (incoming[branchName]?.number === pullRequest.number || now > until) {
      pending.delete(branchName)
    } else if (incoming[branchName] === undefined || incoming[branchName].state === "closed") {
      result = { ...result, [branchName]: pullRequest }
    }
  }
  return result
}

/** What the "Create pull request" dialog asks for */
export interface NewPullRequestRequest {
  /** The repository and branch prefix the dialog was opened in: the pull request goes there, whichever is selected now */
  repositoryPath: string
  branchPrefix: string
  /** The branch, or the root of the whole stack (`wholeStack`) */
  branchName: string
  /** The pull request of the stack branch of the whole stack whose root `branchName` is */
  wholeStack?: boolean
  head: string
  base: string
  title: string
  body: string
  reviewers: string[]
  assignSelf: boolean
  /** The branch was just pushed: the creation waits while GitHub doesn't have it (a mirror copies it a moment later) */
  waitForMirror: boolean
}

/**
 * Opens the pull request of a branch on GitHub (or finds the one it has), and puts it into the store at once:
 * the row's chip and the Safe Push popover read it before the next refresh, which follows. Throws what GitHub said.
 */
export async function createPullRequest(request: NewPullRequestRequest): Promise<CreatedPullRequest> {
  const key = repositoryKey(request.repositoryPath, request.branchPrefix)
  const { wholeStack = false, ...params } = request
  const result = await commands.createPullRequest(params)
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  const createdKey = wholeStack ? stackKey(key) : key
  const pending = created.get(createdKey) ?? new Map()
  pending.set(request.branchName, { pullRequest: result.data.pullRequest, until: Date.now() + CREATED_KEEP_MS })
  created.set(createdKey, pending)
  if (wholeStack) {
    setStackPullRequest(key, request.branchName, result.data.pullRequest)
  } else {
    setPullRequest(key, request.branchName, result.data.pullRequest)
  }
  void refreshPullRequests("push")
  return result.data
}

function setPullRequest(key: string, branchName: string, pullRequest: PullRequest): void {
  const pullRequests = pullRequestsStore.getState().repositories[key]?.pullRequests ?? {}
  noteWritten(writtenKey(key, false), branchName)
  update(key, { pullRequests: { ...pullRequests, [branchName]: pullRequest } })
}

function setStackPullRequest(key: string, root: string, pullRequest: PullRequest): void {
  const stackPullRequests = pullRequestsStore.getState().repositories[key]?.stackPullRequests ?? {}
  noteWritten(writtenKey(key, true), root)
  update(key, { stackPullRequests: { ...stackPullRequests, [root]: pullRequest } })
}

/** Forgets the gates and the data (tests) */
export function resetPullRequests(): void {
  gates.reset()
  created.clear()
  written.clear()
  clearTimeout(recheckTimer)
  pullRequestsStore.setState({ repositories: {} })
}
