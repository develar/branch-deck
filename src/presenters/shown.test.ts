// @spec openspec/specs/branch/shown-reviews/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ShowReviewersState } from "@/hooks/useShowReviewers"
import type { PullRequest, ShownReview, ShownState } from "@/lib/bindings"
import { describeCloseShownReview, describeShowCandidates, formatShownRequestedTitle } from "@/presenters/shown"

function review(state: ShownState): ShownReview {
  return { pullRequest: { number: 12, title: "Add the token", isDraft: state.kind === "draft" } as PullRequest, reviewers: ["alice"], state }
}

describe("the words of a review after landing", () => {
  it("say whom the picker starts with, of which group nobody can be asked, and why it starts empty", () => {
    expect(
      (
        [
          { kind: "loading" },
          { kind: "loaded", value: { groups: ["Auth Team", "Design"], reviewers: ["AlexPl292"], withoutReviewer: ["Design"] } },
          { kind: "loaded", value: { groups: ["Auth Team"], reviewers: ["AlexPl292"], withoutReviewer: [] } },
          { kind: "loaded", value: { groups: [], reviewers: [], withoutReviewer: [] } },
          { kind: "failed", message: "No Space token for the CodeOwners service" },
        ] satisfies ShowReviewersState[]
      ).map(describeShowCandidates),
    ).toEqual([
      null,
      "One available code owner of Auth Team · nobody of Design can be asked.",
      "One available code owner of Auth Team.",
      "No OWNERSHIP file names an owner of the changes: add the reviewers by their GitHub login.",
      "Cannot read the code owners of the changes: No Space token for the CodeOwners service. Add the reviewers by their GitHub login.",
    ])
  })

  it("confirm a close by what the review waits for, closed anyway without the approvals or the fix", () => {
    const deletes = "Closes it on GitHub with a comment and deletes its branches; the change stays in master."
    expect(
      [
        review({ kind: "approved", by: ["alice"] }),
        review({ kind: "inReview", waiting: ["alice"] }),
        review({ kind: "changesRequested", by: ["alice"] }),
        review({ kind: "extending", robot: "r2" }),
        review({ kind: "draft" }),
      ].map(it => describeCloseShownReview(it, "auth").description),
    ).toEqual([
      `alice approved #12. ${deletes}`,
      `alice has not approved #12: it closes without that approval. ${deletes}`,
      `alice requested changes in #12: it closes without a fix. ${deletes}`,
      `A fix of auth lands, which #12 has not taken yet: it closes before its review. ${deletes}`,
      `#12 is a draft that asked nobody yet: it closes without a review. ${deletes}`,
    ])
    expect(describeCloseShownReview(review({ kind: "draft" }), "auth").title).toBe("Close review after landing #12")
  })

  it("toast a request as asked, of a draft as asked once its Safe Push merged", () => {
    expect(formatShownRequestedTitle(12, ["bob"], false)).toBe("Asked bob to review #12 after landing")
    expect(formatShownRequestedTitle(12, ["bob", "carol"], true)).toBe("#12 asks bob and carol once the Safe Push merged")
  })
})
