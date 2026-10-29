import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { BranchRules, Fact, FactsChanges, FactsView, PullRequest, PullRequestAvailability } from "@/lib/bindings"
import type { BranchRulesAnswer } from "@/lib/branch-rules"
import { isRulesUnknown, rulesAnswerOf } from "@/lib/branch-rules"
import { commandFailure } from "@/lib/errors"
import type { GitHubRepository } from "@/lib/pull-request"
import { gitHubRepositoryOf } from "@/lib/pull-request"
import { runFailure, runIntent } from "@/model/intents"
import { refreshSessionFacts } from "@/model/store"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"

/**
 * The pull requests of one repository and branch prefix, as the session of the repository lists them and the app's
 * writes leave them (`facts.pullRequests` of its view, `repository_model::pull_request_facts`): the session asks GitHub
 * by its rules (a committed table at most once a minute, a focus after two minutes, a push and a write of the app
 * always, the refresh button; `remote_facts::scheduler`) and orders the listings against the writes, so a listing that
 * started before a retarget never rolls it back; the page only stores them
 */
export interface RepositoryPullRequests {
  /** What the last answer said; null until the first */
  readonly availability: PullRequestAvailability | null
  /** A listing stands: GitHub listed them, or failed for a while after a listing, whose pull requests stay */
  readonly listed: boolean
  /** By virtual branch name */
  readonly pullRequests: Readonly<Record<string, PullRequest>>
  /**
   * The pull requests of the stack branches (`{prefix}/stack/<root>`, the whole stack), by the root of the stack; never
   * a branch's (spec/branch/whole-stack.spec.md)
   */
  readonly stackPullRequests: Readonly<Record<string, PullRequest>>
  /** `Date.now()` of the last answer, 0 = never */
  readonly fetchedAt: number
  /** The refresh button's listing runs */
  readonly isRefreshing: boolean
}

export interface PullRequestsState {
  /** By `repositoryKey`, of the session of the window */
  readonly repositories: Readonly<Record<string, RepositoryPullRequests>>
  /** By `repositoryKey`, then by base: the rules of the bases of the approved pull requests (`facts.rules`) */
  readonly rules: Readonly<Record<string, Readonly<Record<string, Fact<BranchRules>>>>>
}

export const pullRequestsStore = createStore<PullRequestsState>(() => ({ repositories: {}, rules: {} }))

const EMPTY: RepositoryPullRequests = { availability: null, listed: false, pullRequests: {}, stackPullRequests: {}, fetchedAt: 0, isRefreshing: false }

/**
 * Whether the pull requests of a repository are known: `loading` before GitHub's first answer (after a start or a
 * switch); `listed` while GitHub lists them, and through a refresh that failed for a while after a listing (offline, an
 * error, the rate limit: the pull requests are the last listing's); `unlisted` while GitHub can't list them (no token,
 * a rejected one, SSO, no access, not on GitHub, a failure before any listing). Neither of the others is "no pull
 * request"
 */
export type PullRequestListing = "loading" | "listed" | "unlisted"

function listingOf(entry: RepositoryPullRequests | undefined): PullRequestListing {
  if ((entry?.availability ?? null) === null) {
    return "loading"
  }
  return entry?.listed === true ? "listed" : "unlisted"
}

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

/**
 * GitHub hasn't answered yet whether it lists the pull requests of the selected repository (after a start or a switch):
 * no pull request of a branch is known, which is never "no pull request". A failed answer is an availability, not this
 */
export function isPullRequestsLoading(): boolean {
  return currentKey() !== null && getPullRequestAvailability() === null
}

/** `isPullRequestsLoading` of the selected repository, for a component */
export function usePullRequestsLoading(): boolean {
  const key = useRepositoryKey()
  return useStore(pullRequestsStore, state => (state.repositories[key]?.availability ?? null) === null)
}

/** Whether the pull requests of the selected repository are known (`PullRequestListing`); `loading` without one */
export function getPullRequestListing(): PullRequestListing {
  const key = currentKey()
  return key === null ? "loading" : listingOf(pullRequestsStore.getState().repositories[key])
}

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

/** The path of a repository key */
function pathOf(key: string): string {
  return key.split("\n")[0]!
}

/**
 * The pull requests and the rules of the bases of the session of `repositoryPath` as its view has them now
 * (`facts.pullRequests`, `facts.rules`), with the patch that brought them (`change`; null for a snapshot, which
 * replaces them). A patch replaces the pull requests it names, keeps the object of each it doesn't, so a row that reads
 * one doesn't render again
 */
export function applyPullRequestsFacts(repositoryPath: string, facts: FactsView, change: FactsChanges | null): void {
  const key = repositoryKey(repositoryPath, getEffectiveBranchPrefix())
  if (change === null || change.pullRequests != null) {
    const { head, pullRequests, stackPullRequests } = facts.pullRequests
    const entry = pullRequestsStore.getState().repositories[key]
    const listed = head.state.kind === "stale" || (head.state.kind === "fresh" && head.value?.status === "available")
    if (entry === undefined && head.value === null) {
      // a session GitHub has not answered yet: no pull request is known, as without an entry
    } else if (
      entry?.availability !== head.value ||
      entry.listed !== listed ||
      entry.pullRequests !== pullRequests ||
      entry.stackPullRequests !== stackPullRequests ||
      entry.fetchedAt !== head.observedAt * 1000
    ) {
      update(key, { availability: head.value, listed, pullRequests, stackPullRequests, fetchedAt: head.observedAt * 1000 })
    }
  }
  if ((change === null || change.rules != null) && pullRequestsStore.getState().rules[key] !== facts.rules) {
    pullRequestsStore.setState(state => ({ rules: { ...state.rules, [key]: facts.rules } }))
  }
}

/**
 * Asks the session of the selected repository to list the pull requests again: the refresh button (`manual`, which the
 * status line shows running) and the app's own reasons that follow a write (`afterWrite`: the merge of a robot whose
 * pull request Patronus closed). Resolves once the listing's patch is in the store; never fails
 */
export async function refreshPullRequests(reason: "manual" | "afterWrite" = "manual"): Promise<void> {
  const key = currentKey()
  if (key === null) {
    return
  }
  const manual = reason === "manual"
  if (manual) {
    update(key, { isRefreshing: true })
  }
  try {
    await refreshSessionFacts(pathOf(key), "pullRequests", reason)
  } finally {
    if (manual) {
      update(key, { isRefreshing: false })
    }
  }
}

/**
 * Retargets the open pull request of a branch into the target on GitHub (`Retarget`, the engine of the session): a
 * second retarget of it joins the first, a review request of it runs beside; resolves once the session has the new base
 * (its patch is in the store), so the toast of the push that follows reads it, and Activity has it. Throws why not
 */
export async function retargetPullRequest(branchName: string): Promise<void> {
  const key = currentKey()
  const pullRequest = getPullRequest(branchName)
  if (key === null || pullRequest === undefined) {
    return
  }
  const run = await runIntent(pathOf(key), { kind: "retarget", number: pullRequest.number })
  const failure = runFailure(run, `The repository lists no open pull request #${pullRequest.number}`)
  if (failure !== null) {
    throw typeof failure === "string" ? new Error(failure) : commandFailure(failure)
  }
}

// ---- the rules of the bases ----

/**
 * The rules of `branch` (a base of the approved pull requests, which the session reads once each after a listing) of
 * the selected repository; null while not read, for a decision read outside React
 */
export function peekBranchRules(branch: string): BranchRulesAnswer | null {
  const key = currentKey()
  return key === null ? null : rulesAnswerOf(pullRequestsStore.getState().rules[key]?.[branch])
}

/**
 * The rules of `branch` while `enabled`: the Request Review form warns before a push takes the approvals away. Null
 * while not read; `keep` when not `enabled`. A form that opens on rules GitHub didn't give asks them again
 */
export function useBranchRules(branch: string | null, enabled: boolean): BranchRulesAnswer | null {
  const key = useRepositoryKey()
  const fact = useStore(pullRequestsStore, state => (branch === null ? undefined : state.rules[key]?.[branch]))
  const answer = rulesAnswerOf(fact)
  const unknown = enabled && answer !== null && isRulesUnknown(answer)
  useEffect(() => {
    if (unknown) {
      void askBranchRulesAgain()
    }
  }, [unknown])
  return enabled ? answer : "keep"
}

/**
 * Asks the session to read again the rules GitHub didn't give (a start reads its decision on the new answer, not the old
 * reason); resolves once their reads applied, at once when none failed
 */
export async function askBranchRulesAgain(): Promise<void> {
  const key = currentKey()
  if (key !== null) {
    await refreshSessionFacts(pathOf(key), "branchRules", "afterWrite")
  }
}

/** Forgets the pull requests and the rules (tests) */
export function resetPullRequests(): void {
  pullRequestsStore.setState({ repositories: {}, rules: {} })
}
