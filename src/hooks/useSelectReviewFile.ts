import { createContext, useContext } from "react"
import type { ReviewPlace } from "@/lib/review-place"
import type { StoryMark } from "@/presenters/review-story"

/** What a link of the review plan shows in the review around it */
export interface ReviewNavigator {
  /** A place of a changed file, by its last path or its old path: the file selected, its line shown, its name selected */
  readonly show: (place: ReviewPlace) => void
  /** A place outside the change at the head of the review: its file opens over the review, at the line */
  readonly showOutside: (place: ReviewPlace) => void
  /** A step of the story (`ReviewStep::id`): its opening at the top of the diffs, its row selected */
  readonly showStep: (step: string) => void
  /** The block of a story that names a section: where it is told (an opening, the overview), outlined for a moment */
  readonly showStory: (mark: StoryMark) => void
}

/**
 * A link of a document beside the review it names (the Overview tab of a pull request), followed once the review is
 * read: the call of `ReviewNavigator` it stands for
 */
export type ReviewLink =
  | { readonly kind: "show"; readonly place: ReviewPlace }
  | { readonly kind: "showOutside"; readonly place: ReviewPlace }
  | { readonly kind: "showStep"; readonly step: string }
  | { readonly kind: "showStory"; readonly mark: StoryMark }

/** The navigator that keeps each link as a `ReviewLink` for `follow` */
export function linkNavigator(follow: (link: ReviewLink) => void): ReviewNavigator {
  return {
    show: place => follow({ kind: "show", place }),
    showOutside: place => follow({ kind: "showOutside", place }),
    showStep: step => follow({ kind: "showStep", step }),
    showStory: mark => follow({ kind: "showStory", mark }),
  }
}

/** Shows the place of `link` in the review of `navigator` */
export function followLink(navigator: ReviewNavigator, link: ReviewLink): void {
  switch (link.kind) {
    case "show":
      navigator.show(link.place)
      break
    case "showOutside":
      navigator.showOutside(link.place)
      break
    case "showStep":
      navigator.showStep(link.step)
      break
    case "showStory":
      navigator.showStory(link.mark)
      break
  }
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
