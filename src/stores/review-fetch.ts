import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { Channel } from "@tauri-apps/api/core"
import type { ReviewRange, ReviewRequest, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { diffSubjectId } from "@/lib/commit-files"
import { logError } from "@/lib/errors"
import { dropFilesOf } from "@/stores/commit-files"
import { dropPatchesOf } from "@/stores/commit-patch"
import { noteReviewRefsWritten } from "@/stores/review-requests"

/*
 * The fetch of the pull request the review dialog shows (`fetch_review_head`): it starts when the dialog opens and
 * gives the range to review, `base..head`. One fetch at a time: a newer one, or the close, kills the one in flight.
 */

export type ReviewFetch
  = | { readonly status: "fetching", readonly progress: TransferProgress | null }
    | { readonly status: "fetched", readonly range: ReviewRange }
    | { readonly status: "failed", readonly message: string }

export interface ReviewFetchState {
  /** By `reviewFetchKey`: the repository, the number and the commits GitHub named */
  readonly fetches: Readonly<Record<string, ReviewFetch>>
}

export const reviewFetchStore = createStore<ReviewFetchState>(() => ({ fetches: {} }))

/** A pushed or rebased pull request is another fetch */
export function reviewFetchKey(repositoryPath: string, request: Pick<ReviewRequest, "number" | "headRefOid" | "baseRefOid">): string {
  return `${repositoryPath}\n${request.number}\n${request.headRefOid}\n${request.baseRefOid}`
}

export function useReviewFetch(key: string | null): ReviewFetch | undefined {
  return useStore(reviewFetchStore, state => key === null ? undefined : state.fetches[key])
}

function setFetch(key: string, fetch: ReviewFetch | null): void {
  reviewFetchStore.setState((state) => {
    const { [key]: _previous, ...others } = state.fetches
    return { fetches: fetch === null ? others : { ...others, [key]: fetch } }
  })
}

/** The fetch whose answer counts: an older one was killed by the backend or by the close */
let latestKey: string | null = null

/**
 * Fetches the pull request unless its range is fetched or being fetched (a retry after a failure fetches again).
 * `fetch_review_head` costs nothing when both commits are local; otherwise it writes `refs/branch-deck/review/<n>/…`,
 * which the next prune deletes.
 */
export function startReviewFetch(repositoryPath: string, request: ReviewRequest): void {
  const key = reviewFetchKey(repositoryPath, request)
  const current = reviewFetchStore.getState().fetches[key]
  if (current !== undefined && current.status !== "failed" && latestKey === key) {
    return
  }
  // another request of the dialog: the backend kills its fetch, the range goes
  if (latestKey !== null && latestKey !== key) {
    forget(latestKey, false)
  }
  latestKey = key
  setFetch(key, { status: "fetching", progress: null })
  const progress = new Channel<TransferProgress>()
  progress.onmessage = (message) => {
    if (latestKey === key) {
      setFetch(key, { status: "fetching", progress: message })
    }
  }
  const params = {
    repositoryPath,
    number: request.number,
    headRefName: request.headRefName,
    headRefOid: request.headRefOid,
    baseRefName: request.baseRefName,
    baseRefOid: request.baseRefOid,
    isCrossRepository: request.isCrossRepository,
  }
  void invokeCommand(() => commands.fetchReviewHead(params, progress)).then((result) => {
    // killed by a newer fetch or by the close: nothing to show
    if (latestKey !== key) {
      return
    }
    if (result.status === "ok") {
      if (result.data.source !== "local") {
        noteReviewRefsWritten(repositoryPath)
      }
      setFetch(key, { status: "fetched", range: result.data })
    }
    else {
      logError(`Cannot fetch pull request #${request.number}`, result.error)
      setFetch(key, { status: "failed", message: result.error.message })
    }
  })
}

/**
 * The dialog closed: the fetch in flight is killed, and the range is forgotten with its files and patches (a reopen
 * asks `fetch_review_head` again, which finds the commits local while gc keeps them)
 */
export function endReviewFetch(): void {
  const key = latestKey
  latestKey = null
  if (key !== null) {
    forget(key, true)
  }
}

function forget(key: string, cancel: boolean): void {
  const fetch = reviewFetchStore.getState().fetches[key]
  if (cancel && fetch?.status === "fetching") {
    void commands.cancelReviewFetch().catch(() => {})
  }
  if (fetch?.status === "fetched") {
    const { base, head } = fetch.range
    dropRangeOf([base, ...fetch.range.commits.map(commit => commit.id)], head)
  }
  setFetch(key, null)
}

/** Drops the ranges `<base>..head` of the review (all changes, and since the user's review) */
function dropRangeOf(bases: readonly string[], head: string): void {
  const ids = bases.map(base => diffSubjectId({ kind: "range", base, head }))
  dropFilesOf(ids)
  dropPatchesOf(ids)
}

/** Forgets every fetch (tests) */
export function resetReviewFetches(): void {
  latestKey = null
  reviewFetchStore.setState({ fetches: {} })
}
