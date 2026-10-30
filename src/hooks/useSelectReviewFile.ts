import { createContext, useContext } from "react"
import type { ReviewPlace } from "@/lib/review-place"

/** What a link of the review plan shows in the review around it */
export interface ReviewNavigator {
  /** A place of a changed file, by its last path or its old path: the file selected, its line shown, its name selected */
  readonly show: (place: ReviewPlace) => void
  /** A place outside the change at the head of the review: its file opens over the review, at the line */
  readonly showOutside: (place: ReviewPlace) => void
}

/** The places of the review around the component, as a link of a badge shows them; null outside a review */
export const SelectReviewFileContext = createContext<ReviewNavigator | null>(null)

/**
 * The places of the review around the component; null outside a review. A filter that hides the file of a place is
 * dropped first (`CommitReview`)
 */
export function useSelectReviewFile(): ReviewNavigator | null {
  return useContext(SelectReviewFileContext)
}
