import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { PullRequest, PullRequestAvailability } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProject } from "@/stores/repository"

/** Calls closer together than this are skipped, except right after a push and on a manual refresh */
export const MIN_REFRESH_GAP_MS = 60_000
/** On window focus, pull requests older than this are refreshed */
export const FOCUS_REFRESH_AGE_MS = 120_000
/** GitHub may show the old head for a moment after a push: one more look after this */
export const PUSH_RECHECK_DELAY_MS = 5_000

/**
 * Why a refresh is asked for. A sync refreshes at most once a minute, a focus when the data is older than two minutes;
 * a push and the refresh button always ask (after the request in flight, if any).
 */
export type RefreshReason = "sync" | "focus" | "push" | "manual"

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

/** The pull requests of the selected repository */
export function useRepositoryPullRequests(): RepositoryPullRequests {
  const key = repositoryKey(useSelectedProject()?.path ?? "", useEffectiveBranchPrefix())
  return useStore(pullRequestsStore, state => state.repositories[key] ?? EMPTY)
}

export function usePullRequest(branchName: string): PullRequest | undefined {
  return useRepositoryPullRequests().pullRequests[branchName]
}

/** Non-hook variant of `usePullRequest`, for menus built on open */
export function getPullRequest(branchName: string): PullRequest | undefined {
  const key = currentKey()
  return key === null ? undefined : pullRequestsStore.getState().repositories[key]?.pullRequests[branchName]
}

// ---- refresh ----

interface Gate {
  /** `Date.now()` when the last request started */
  startedAt: number
  inFlight: Promise<void> | null
  /** A push or a manual refresh came while a request was in flight: ask again after it */
  followUp: boolean
}

const gates = new Map<string, Gate>()

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

function isDue(reason: RefreshReason, gate: Gate, fetchedAt: number, now: number): boolean {
  switch (reason) {
    case "push":
    case "manual":
      return true
    case "sync":
      return now - gate.startedAt >= MIN_REFRESH_GAP_MS
    case "focus":
      return now - gate.startedAt >= MIN_REFRESH_GAP_MS && now - fetchedAt >= FOCUS_REFRESH_AGE_MS
  }
}

/**
 * Asks `gh` for the pull requests of the selected repository, if the reason allows it now (see `RefreshReason`).
 * One request per repository at a time. Never fails: what went wrong is the availability.
 */
export function refreshPullRequests(reason: RefreshReason): Promise<void> {
  const key = currentKey()
  if (key === null) {
    return Promise.resolve()
  }
  let gate = gates.get(key)
  if (gate === undefined) {
    gate = { startedAt: Number.NEGATIVE_INFINITY, inFlight: null, followUp: false }
    gates.set(key, gate)
  }
  const now = Date.now()
  const entry = pullRequestsStore.getState().repositories[key] ?? EMPTY
  if (gate.inFlight !== null) {
    if (reason === "push" || reason === "manual") {
      gate.followUp = true
    }
    return gate.inFlight
  }
  if (!isDue(reason, gate, entry.fetchedAt, now)) {
    return Promise.resolve()
  }

  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  gate.startedAt = now
  const run = async (): Promise<void> => {
    update(key, { isRefreshing: true })
    try {
      do {
        gate.followUp = false
        const result = await commands.getPullRequests({ repositoryPath, branchPrefix })
        if (result.status === "ok") {
          update(key, { availability: result.data.availability, pullRequests: result.data.pullRequests, fetchedAt: Date.now() })
        }
        else {
          // keep what was shown: a failed refresh does not make the pull requests go away
          update(key, { availability: { status: "error", message: result.error.message }, fetchedAt: Date.now() })
        }
      } while (gate.followUp)
    }
    catch (error) {
      update(key, { availability: { status: "error", message: error instanceof Error ? error.message : String(error) }, fetchedAt: Date.now() })
    }
    finally {
      gate.inFlight = null
      update(key, { isRefreshing: false })
    }
  }
  gate.inFlight = run()
  return gate.inFlight
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

/** Forgets the gates and the data (tests) */
export function resetPullRequests(): void {
  gates.clear()
  clearTimeout(recheckTimer)
  pullRequestsStore.setState({ repositories: {} })
}
