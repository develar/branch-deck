// @spec openspec/specs/commit/quiet-changes/spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { MenuItem } from "@/lib/menu"
import { REVIEW_KEYS, reviewKeyAction } from "@/lib/review-keys"
import { getReviewRequestActions } from "@/lib/review-request-actions"
import { ACTION_LOOKS } from "@/presenters/actions"
import type { ActionLook } from "@/presenters/actions"
import { reviewRequest, reviewRequestActionContext } from "@/test-support/review-request"

vi.mock("@/lib/clipboard", () => ({ copyText: async () => {} }))

/** The letter an action names as its one key, in the case of the table of the review; none for a chord or no key */
function letterOf(kbds: readonly string[] | undefined): string[] {
  return kbds?.length === 1 && /^[a-z]$/.test(kbds[0]!) ? [kbds[0]!.toUpperCase()] : []
}

describe("the keys of a review", () => {
  it("answers the action of each of its keys by the physical key, and none for another key", () => {
    const keys = Object.values(REVIEW_KEYS)
    expect(keys.every(key => /^[A-Z]$/.test(key))).toBe(true)
    expect(new Set(keys).size).toBe(keys.length)
    expect(["KeyJ", "KeyK", "KeyN", "KeyP", "KeyF", "KeyH"].map(reviewKeyAction)).toEqual(["nextSection", "previousSection", "nextChange", "previousChange", "sidebar", "notes"])
    // the key of an action of the window, the letter a layout types, a key with no letter
    expect(["KeyO", "h", "H", "Space", ""].map(reviewKeyAction)).toEqual([undefined, undefined, undefined, undefined, undefined])
  })

  it("takes no letter that an action of a window of the review names as its key: O stays the pull request's", () => {
    // the actions of a branch, as its row and the title bar of its review name them; the title bar shows the push
    // without its key, since P is the previous change of the review
    const ofBranch = (Object.entries(ACTION_LOOKS) as [string, ActionLook][]).flatMap(([id, look]) => (id === "push" ? [] : letterOf(look.kbds)))
    // the actions of a pull request in its review window
    const offered = (["review", "approve", "request-changes", "open-pull-request", "safe-push-pull-request"] as const).map(id => ({
      id,
      availability: { kind: "enabled" as const },
    }))
    const request = getReviewRequestActions(reviewRequest(7), reviewRequestActionContext(offered), "window")
    const ofRequest = [request.review, request.approve, request.safePush, request.requestChanges, request.openPullRequest].flatMap((item: MenuItem | null) => letterOf(item?.kbds))
    expect(ofBranch.toSorted()).toEqual(["D", "M", "O", "S", "S", "S"])
    expect(ofRequest.toSorted()).toEqual(["A", "D", "M", "O", "R"])
    // the details of a row of the tree and of a commit (I) are a key of the window too
    const taken = new Set([...ofBranch, ...ofRequest, "I"])
    expect(Object.values(REVIEW_KEYS).filter(key => taken.has(key))).toEqual([])
  })
})
