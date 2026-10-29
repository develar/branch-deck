import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { OperationEvent, OperationId, ReviewRange, ReviewRequest } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { diffSubjectId } from "@/lib/commit-files"
import { logError } from "@/lib/errors"
import { failedEffect } from "@/lib/run-intent"
import { dropFilesOf } from "@/stores/commit-files"
import { dropFileDiffsOf } from "@/stores/file-diffs"

/*
 * The fetch of the pull request the review window shows (the intent `fetchReview` of the session of its repository,
 * `operations::review_fetch`): it starts when the window shows it and gives the range to review, `base..head`. One
 * fetch at a time per window: a newer one stops the one in flight (`cancel_operation`), and so does the close of the
 * window (the app, by its label). The session prunes the review refs of a pull request no review window shows.
 */

/** How far a fetch is, as git says it (`OperationEvent::Progress`) */
export interface ReviewFetchProgress {
  readonly percent: number
  readonly line: string
  readonly transferred: string | null
  readonly speed: string | null
}

export type ReviewFetch =
  | { readonly status: "fetching"; readonly progress: ReviewFetchProgress | null }
  | { readonly status: "fetched"; readonly range: ReviewRange }
  | { readonly status: "failed"; readonly message: string }

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
  return useStore(reviewFetchStore, state => (key === null ? undefined : state.fetches[key]))
}

function setFetch(key: string, fetch: ReviewFetch | null): void {
  reviewFetchStore.setState(state => {
    const { [key]: _previous, ...others } = state.fetches
    return { fetches: fetch === null ? others : { ...others, [key]: fetch } }
  })
}

/** The fetch whose answer counts: an older one was stopped, or the window closed */
let latestKey: string | null = null
/** The operation of the latest fetch, once its first step named it: what a newer fetch stops */
let latestOp: OperationId | null = null

/**
 * Fetches the pull request unless its range is fetched or being fetched (a retry after a failure fetches again): Rust
 * fetches nothing when both commits are local, else writes `refs/branch-deck/review/<n>/…`, which the session prunes
 * once no review window shows the pull request.
 */
export function startReviewFetch(repositoryPath: string, request: ReviewRequest): void {
  const key = reviewFetchKey(repositoryPath, request)
  const current = reviewFetchStore.getState().fetches[key]
  if (current !== undefined && current.status !== "failed" && latestKey === key) {
    return
  }
  // another request of the window: its fetch stops, the range goes
  if (latestKey !== null && latestKey !== key) {
    forget(latestKey, true)
  }
  latestKey = key
  latestOp = null
  setFetch(key, { status: "fetching", progress: null })
  const events = new Channel<OperationEvent>()
  events.onmessage = event => {
    if (latestKey !== key) {
      return
    }
    if (event.kind === "step") {
      latestOp = event.op
    } else if (event.kind === "progress") {
      const progress: ReviewFetchProgress = { percent: event.percent, line: event.line, transferred: event.transferred, speed: event.speed }
      setFetch(key, { status: "fetching", progress })
    }
  }
  const params = { repositoryPath, intent: { kind: "fetchReview", number: request.number } as const, fingerprint: "" }
  void invokeCommand(() => commands.execute(params, events)).then(result => {
    // stopped by a newer fetch or by the close: nothing to show
    if (latestKey !== key) {
      return
    }
    latestOp = null
    const failure = result.status === "error" ? result.error : failedEffect(result.data)?.error
    if (failure === undefined) {
      const range = result.status === "ok" && result.data.result.kind === "reviewRange" ? result.data.result : null
      setFetch(key, range === null ? null : { status: "fetched", range })
    }
    // stopped for a newer fetch: no failure of this one
    else if (failure.code === "canceled") {
      setFetch(key, null)
    } else {
      logError(`Cannot fetch pull request #${request.number}`, failure)
      setFetch(key, { status: "failed", message: failure.message })
    }
  })
}

function forget(key: string, cancel: boolean): void {
  const fetch = reviewFetchStore.getState().fetches[key]
  if (cancel && fetch?.status === "fetching" && latestOp !== null) {
    void commands.cancelOperation({ op: latestOp }).catch(() => {})
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
  dropFileDiffsOf(ids)
}
