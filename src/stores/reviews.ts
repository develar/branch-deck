import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { ApiError, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewsView, RepositoryView, TableChanges } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { refreshSessionFacts } from "@/model/store"
import { useSelectedProjectPath } from "@/stores/repository"
import { reviewRequestsStore } from "@/stores/review-requests"

/*
 * Reviews (spec/repository/reviews.spec.md): every open pull request of the selected repository the user takes part in
 * beside the requests by name (Your Turn's rows lead the view). The session reads the rest of Reviews while the view
 * shows (`Demand::reviews`, held by the view) and derives its sections (`domain_policy::reviews`); the store keeps
 * them as the view has them (`applyReviewsView`). The links the user adds are Rust's (`link_pull_request`), kept for
 * the GitHub repository; the page only asks.
 */

export interface ReviewsState {
  /** By repository path, as the view of its session has them */
  readonly repositories: Readonly<Record<string, ReviewsView>>
  /** The refresh button asked, and the answer is not in yet, by repository path */
  readonly refreshing: ReadonlySet<string>
}

export const reviewsStore = createStore<ReviewsState>(() => ({ repositories: {}, refreshing: new Set() }))

const EMPTY: ReviewsView = { head: { value: null, state: { kind: "unknown" }, seq: 0, observedAt: 0 }, sections: [], checkedAt: 0 }

/** Reviews of the session of `repositoryPath` as its view has it, when the patch (`changes`; null for a snapshot) carries it */
export function applyReviewsView(repositoryPath: string, view: RepositoryView, changes: TableChanges | null): void {
  if (changes !== null && changes.reviews == null) {
    return
  }
  reviewsStore.setState(state => ({ repositories: { ...state.repositories, [repositoryPath]: view.reviews } }))
}

/** Reviews of the selected repository; the same object while nothing changed */
export function useReviews(): ReviewsView {
  const repositoryPath = useSelectedProjectPath() ?? ""
  return useStore(reviewsStore, state => state.repositories[repositoryPath] ?? EMPTY)
}

export function useIsRefreshingReviews(): boolean {
  const repositoryPath = useSelectedProjectPath() ?? ""
  return useStore(reviewsStore, state => state.refreshing.has(repositoryPath))
}

/** The rows of Reviews in their order, the requests of Your Turn first, with how the user takes part in each of the rest */
export function reviewRows(repositoryPath: string): { readonly requests: readonly ReviewRequest[]; readonly involvements: Readonly<Record<number, ReviewInvolvement>> } {
  const requests = reviewRequestsStore.getState().repositories[repositoryPath]?.requests ?? []
  const sections = reviewsStore.getState().repositories[repositoryPath]?.sections ?? []
  const entries: ReviewEntry[] = sections.flatMap(section => section.entries)
  const involvements: Record<number, ReviewInvolvement> = {}
  for (const entry of entries) {
    involvements[entry.request.number] = entry.involvement
  }
  return { requests: [...requests, ...entries.map(entry => entry.request)], involvements }
}

/**
 * Asks the session of `repositoryPath` to read Reviews again (the refresh button): the review requests with the rest of
 * Reviews, refreshing until the answer is in; never fails, what went wrong is the state of the view
 */
export async function refreshReviews(repositoryPath: string): Promise<void> {
  const mark = (refreshing: boolean) =>
    reviewsStore.setState(state => {
      const next = new Set(state.refreshing)
      if (refreshing) {
        next.add(repositoryPath)
      } else {
        next.delete(repositoryPath)
      }
      return { refreshing: next }
    })
  mark(true)
  try {
    await refreshSessionFacts(repositoryPath, "reviewRequests", "manual")
  } finally {
    mark(false)
  }
}

/** What the user's link answered: the pull request to open, and whether Reviews keeps it; or why it can't */
export type LinkAnswer = { readonly kind: "linked"; readonly entry: ReviewEntry; readonly kept: boolean } | { readonly kind: "refused"; readonly error: ApiError }

/**
 * Adds the pull request `input` names (its link, `owner/name#123`, `#123`) to Reviews of `repositoryPath`: Rust asks
 * GitHub, keeps an open one for the repository and reads Reviews again; a refusal (another repository, no such pull
 * request) is the answer, said by the field, not a toast
 */
export async function linkPullRequest(repositoryPath: string, input: string): Promise<LinkAnswer> {
  const result = await commands.linkPullRequest({ repositoryPath, input })
  if (result.status === "error") {
    return { kind: "refused", error: result.error }
  }
  return { kind: "linked", entry: result.data.entry, kept: result.data.kept }
}

/** The pull request `number` the user added by its link leaves Reviews */
export async function unlinkPullRequest(repositoryPath: string, number: number): Promise<void> {
  const result = await commands.unlinkPullRequest({ repositoryPath, number })
  if (result.status === "error") {
    notifyError(`Cannot remove #${number} from Reviews`, result.error)
  }
}

/** The field of a link takes the focus once it shows (the palette's "Add Pull Request by Link…"): one ask, once */
let linkFieldFocus = false
const linkFieldListeners = new Set<() => void>()

/** Asks the field of a link of Reviews for the focus, now or once it mounts */
export function requestLinkFieldFocus(): void {
  linkFieldFocus = true
  for (const listener of linkFieldListeners) {
    listener()
  }
}

/** The field of a link calls `focus` when it is asked for the focus, also for an ask made before it mounted */
export function onLinkFieldFocus(focus: () => void): () => void {
  const listener = () => {
    if (linkFieldFocus) {
      linkFieldFocus = false
      focus()
    }
  }
  linkFieldListeners.add(listener)
  listener()
  return () => linkFieldListeners.delete(listener)
}
