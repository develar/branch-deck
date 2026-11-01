import { createContext, useContext } from "react"

/**
 * The block of a story a mark of a section of the diffs showed last (spec/branch/review-tour.spec.md, "Anchors"): the
 * story that tells it opens it when a fold holds it and outlines it for a moment; `serial` tells a click again from
 * the one before
 */
export interface StoryHighlight {
  readonly commit: string
  readonly block: number
  readonly serial: number
}

/** The block of a story to outline; null outside a review and before a mark is clicked */
export const StoryHighlightContext = createContext<StoryHighlight | null>(null)

/** The block of a story a mark showed last; null outside a review */
export function useStoryHighlight(): StoryHighlight | null {
  return useContext(StoryHighlightContext)
}
