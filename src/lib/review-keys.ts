/*
 * The keys of a review (`CommitReview`): each a letter without a modifier, the physical key. The review takes its key
 * before the window around it reads the event, and only while the diffs or the tree of the files have the focus. So a
 * letter of this table must be the key of no action of a window that holds a review (O the pull request, S the Safe
 * Push, A, R, M and D of a pull request): such a key would run the action or the review by where the focus is
 * (`review-keys.test.ts`).
 */
export const REVIEW_KEYS = {
  nextSection: "J",
  previousSection: "K",
  nextChange: "N",
  previousChange: "P",
  sidebar: "F",
  /** The old lines of the notes in view (Inline shows a quiet run once): H, the hidden lines */
  notes: "H",
} as const

export type ReviewKeyAction = keyof typeof REVIEW_KEYS

const ACTION_BY_CODE: ReadonlyMap<string, ReviewKeyAction> = new Map((Object.keys(REVIEW_KEYS) as ReviewKeyAction[]).map(action => [`Key${REVIEW_KEYS[action]}`, action]))

/** What the review does on the physical key `code` (`KeyboardEvent.code`); undefined for a key that is not its own */
export function reviewKeyAction(code: string): ReviewKeyAction | undefined {
  return ACTION_BY_CODE.get(code)
}
