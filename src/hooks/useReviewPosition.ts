import { useCallback, useState } from "react"
import { rangePosition } from "@/lib/range-review"
import type { RangeReviewItem } from "@/lib/range-review"
import type { ReviewPosition } from "@/lib/review-position"
import { formatReviewPosition, shownPath } from "@/lib/review-position"

/** Where a review is, as its footer says it, and the file a review read again starts at */
export interface ReviewPositionState {
  /** The footer, of the scope or the commit of `items` shown: "All Changes · Overview", "All Changes · Step 2 · File 3 / 9", "Commit 2 / 5 · File 1 / 4" */
  readonly footer: (items: readonly RangeReviewItem[], selected: string) => string
  /** The last file the review showed: a review read again (a sync, another commit) starts at it */
  readonly lastPath: string | null
  readonly setLastPath: (path: string | null) => void
  /** What the review reports as its top changes (`CommitReview.onPositionChange`); keeps its identity */
  readonly showPosition: (position: ReviewPosition) => void
}

/**
 * The position of a review in one place for every window of a review (the review of a branch, of commits, of a pull
 * request): what the review reports at its top (the overview, the step and the file), worded with the scope or the
 * commit that is shown (`formatReviewPosition`), and the last file; `onPath` hears each file shown (`shownPath`), none
 * while the overview is at the top or the plan is not `settled` yet (its overview will head the review)
 */
export function useReviewPosition(initialPath: string | null, onPath?: (path: string) => void, settled = true): ReviewPositionState {
  const [position, setPosition] = useState<ReviewPosition | null>(null)
  const [lastPath, setLastPath] = useState(initialPath)
  const showPosition = useCallback(
    (next: ReviewPosition) => {
      setPosition(next)
      // the overview at the top shows no file: a review read again starts at the overview, Approve counts nothing; nor
      // does the first file while the plan whose overview will head it is read
      const path = settled ? shownPath(next) : null
      if (path !== null) {
        setLastPath(path)
        onPath?.(path)
      }
    },
    [onPath, settled],
  )
  const footer = (items: readonly RangeReviewItem[], selected: string) => formatReviewPosition({ file: position, ...rangePosition(items, selected) })
  return { footer, lastPath, setLastPath, showPosition }
}
