import { createContext, useContext } from "react"
import type { ReviewPlace } from "@/lib/review-place"

/** Shows a place of the review around the component, as a link of a badge does; null outside a review */
export const SelectReviewFileContext = createContext<((place: ReviewPlace) => void) | null>(null)

/**
 * The selection of a place of the review around the component: its changed file, by its last path or its old path, and
 * the line of the place in its diff; null outside a review. A filter that hides the file is dropped first
 * (`CommitReview`)
 */
export function useSelectReviewFile(): ((place: ReviewPlace) => void) | null {
  return useContext(SelectReviewFileContext)
}
