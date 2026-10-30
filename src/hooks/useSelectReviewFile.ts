import { createContext, useContext } from "react"

/** Selects a changed file of the review around the component, as a link of a badge does; null outside a review */
export const SelectReviewFileContext = createContext<((path: string) => void) | null>(null)

/**
 * The selection of a changed file of the review around the component, by its last path or its old path; null outside a
 * review. A filter that hides the file is dropped first (`CommitReview`)
 */
export function useSelectReviewFile(): ((path: string) => void) | null {
  return useContext(SelectReviewFileContext)
}
