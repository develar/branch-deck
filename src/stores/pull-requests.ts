import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { PullRequest, PullRequestAvailability } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { GitHubRepository } from "@/lib/pull-request"
import { gitHubRepositoryOf } from "@/lib/pull-request"
import type { RefreshReason } from "@/lib/refresh-gate"
import { createRefreshGates } from "@/lib/refresh-gate"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"
import { stableStringify } from "@/stores/settings/sync-utils"

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
  /** `Date.now()` of the last answer, 0 = never */
  readonly fetchedAt: number
  readonly isRefreshing: boolean
}

export interface PullRequestsState {
  /** By `repositoryKey`; in memory only, GitHub is asked again after a restart */
  readonly repositories: Readonly<Record<string, RepositoryPullRequests>>
}

export const pullRequestsStore = createStore<PullRequestsState>(() => ({ repositories: {} }))

const EMPTY: RepositoryPullRequests = { availability: null, pullRequests: {}, fetchedAt: 0, isRefreshing: false }

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

/**
 * The GitHub repository of the selected repository while GitHub lists its pull requests, null otherwise (see
 * `gitHubRepositoryOf`); for menus and toasts built on demand, e.g. a link to create a pull request
 */
export function getGitHubRepository(): GitHubRepository | null {
  const key = currentKey()
  return key === null ? null : gitHubRepositoryOf(pullRequestsStore.getState().repositories[key]?.availability ?? null)
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
export function mergePullRequests(
  previous: RepositoryPullRequests["pullRequests"],
  incoming: RepositoryPullRequests["pullRequests"],
): RepositoryPullRequests["pullRequests"] {
  const next: Record<string, PullRequest> = {}
  let changed = Object.keys(previous).length !== Object.keys(incoming).length
  for (const [branchName, pullRequest] of Object.entries(incoming)) {
    const known = previous[branchName]
    if (known !== undefined && sameJson(known, pullRequest)) {
      next[branchName] = known
    }
    else {
      next[branchName] = pullRequest
      changed = true
    }
  }
  return changed ? next : previous
}

/**
 * Asks GitHub for the pull requests of the selected repository, if the reason allows it now (see the policy above).
 * One request per repository at a time. Never fails: what went wrong is the availability. A repository that is not on
 * GitHub is asked again only by the refresh button.
 */
export function refreshPullRequests(reason: RefreshReason): Promise<void> {
  const key = currentKey()
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
    try {
      const result = await commands.getPullRequests({ repositoryPath, branchPrefix })
      if (result.status === "ok") {
        const previous = pullRequestsStore.getState().repositories[key] ?? EMPTY
        update(key, {
          availability: sameJson(previous.availability, result.data.availability) ? previous.availability : result.data.availability,
          pullRequests: mergePullRequests(previous.pullRequests, result.data.pullRequests),
          fetchedAt: Date.now(),
        })
      }
      else {
        // keep what was shown: a failed refresh does not make the pull requests go away
        update(key, { availability: { status: "error", message: result.error.message }, fetchedAt: Date.now() })
      }
    }
    catch (error) {
      update(key, { availability: { status: "error", message: error instanceof Error ? error.message : String(error) }, fetchedAt: Date.now() })
    }
    finally {
      update(key, { isRefreshing: false })
    }
  })
}

let recheckTimer: ReturnType<typeof setTimeout> | undefined

/**
 * Right after a push: the pull request of the branch shows the pushed commit. If GitHub still reports the old head,
 * asks once more a little later.
 */
export async function refreshPullRequestsAfterPush(branchName: string, pushedTip: string | undefined): Promise<void> {
  await refreshPullRequests("push")
  const pullRequest = getPullRequest(branchName)
  if (pullRequest !== undefined && pushedTip !== undefined && pullRequest.headRefOid !== pushedTip) {
    clearTimeout(recheckTimer)
    recheckTimer = setTimeout(() => void refreshPullRequests("push"), PUSH_RECHECK_DELAY_MS)
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
    throw new Error(result.error.message)
  }
  setPullRequest(key, branchName, { ...pullRequest, baseRefName: base })
}

/** Closes the pull request of a branch on GitHub with `comment`, and in the store at once. Throws what GitHub said. */
export async function closePullRequest(branchName: string, comment: string): Promise<void> {
  const key = currentKey()
  const pullRequest = getPullRequest(branchName)
  if (key === null || pullRequest === undefined) {
    return
  }
  const result = await commands.closePullRequest({ repositoryPath: key.split("\n")[0]!, pullRequestNumber: pullRequest.number, comment })
  if (result.status === "error") {
    throw new Error(result.error.message)
  }
  setPullRequest(key, branchName, { ...pullRequest, state: "closed" })
}

function setPullRequest(key: string, branchName: string, pullRequest: PullRequest): void {
  const pullRequests = pullRequestsStore.getState().repositories[key]?.pullRequests ?? {}
  update(key, { pullRequests: { ...pullRequests, [branchName]: pullRequest } })
}

/** Forgets the gates and the data (tests) */
export function resetPullRequests(): void {
  gates.reset()
  clearTimeout(recheckTimer)
  pullRequestsStore.setState({ repositories: {} })
}
