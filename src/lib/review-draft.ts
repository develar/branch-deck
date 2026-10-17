/*
 * The draft of a request for changes, per pull request (by its URL), in the storage of the browser that every window
 * of the app shares: the review window reused for another pull request, or closed, gives it back when the pull request
 * is reviewed again. A convenience of this machine only: the storage may be empty or refuse, then the draft is not kept.
 */

const PREFIX = "review-draft:"

export function readReviewDraft(url: string): string {
  try {
    return localStorage.getItem(PREFIX + url) ?? ""
  } catch {
    return ""
  }
}

/** Keeps the draft; an empty one (sent, or cleared) is forgotten */
export function writeReviewDraft(url: string, draft: string): void {
  try {
    if (draft.trim() === "") {
      localStorage.removeItem(PREFIX + url)
    } else {
      localStorage.setItem(PREFIX + url, draft)
    }
  } catch {
    // no storage: the draft lives as long as the form
  }
}
