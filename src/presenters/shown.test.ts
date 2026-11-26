// @spec openspec/specs/branch/shown-reviews/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ShowReviewersState } from "@/hooks/useShowReviewers"
import type { PullRequest, ShownReview, ShownState } from "@/lib/bindings"
import {
  describeCancelShownReview,
  describeCloseShownReview,
  describeMarkedLanding,
  describeMarkShownReview,
  describeShowCandidates,
  formatShownCanceledTitle,
  formatShownMarkedTitle,
  formatShownRequestedTitle,
} from "@/presenters/shown"

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
    const closes = "#12 closes on GitHub with a comment and its branches are deleted; the change stays in master."
    expect(
      [
        review({ kind: "approved", by: ["alice"] }),
        review({ kind: "inReview", waiting: ["alice"] }),
        review({ kind: "changesRequested", by: ["alice"] }),
        review({ kind: "extending", robot: "r2" }),
        review({ kind: "draft", landing: true }),
      ].map(it => describeCloseShownReview(it, "auth").description),
    ).toEqual([
      `alice approved #12. ${closes}`,
      `alice has not approved #12 yet; closing now ends the review without that approval. ${closes}`,
      `alice requested changes in #12; closing now ends the review without a fix. ${closes}`,
      `A fix of auth is still landing and is not in #12 yet; closing now leaves it unreviewed. ${closes}`,
      `#12 is a draft that asked nobody yet; closing now ends it without a review. ${closes}`,
    ])
    expect(describeCloseShownReview(review({ kind: "draft", landing: true }), "auth").title).toBe("Close Review #12?")
  })

  it("say what a mark ahead of the landing opens and whom, its draft picked again, its cancel and the Safe Push of it", () => {
    const marked = review({ kind: "draft", landing: false })
    expect([describeMarkShownReview("auth", undefined), describeMarkShownReview("auth", marked)]).toEqual([
      {
        title: "Request review after landing of auth",
        description:
          "A draft pull request opens now and asks nobody. Once auth lands, by its Safe Push, a Safe Push that lands it with more, or a safe-merge from its pull request, it opens on what landed and asks these reviewers.",
      },
      { title: "Request review after landing on #12", description: "Once auth lands, #12 opens and asks the reviewers you pick here; nobody is asked before." },
    ])
    expect(formatShownMarkedTitle("auth", ["alice", "bob"])).toEqual({
      title: "Review after landing of auth requested",
      description: "It opens once auth lands and asks alice and bob.",
    })
    expect(describeCancelShownReview(marked, "auth")).toEqual({
      title: "Cancel Review After Landing #12?",
      description: "#12 closes on GitHub with a comment and its branches are deleted; once auth lands, nothing opens and nobody is asked.",
    })
    expect(formatShownCanceledTitle(12, "auth")).toBe("Canceled review after landing #12 of auth")
    expect(describeMarkedLanding(marked)).toBe("#12 opens once it merges, asking alice.")
  })

  it("toast a request as asked, of a draft as asked once its Safe Push merged", () => {
    expect(formatShownRequestedTitle(12, ["bob"], false)).toBe("Asked bob to review #12 after landing")
    expect(formatShownRequestedTitle(12, ["bob", "carol"], true)).toBe("#12 asks bob and carol once the Safe Push merged")
  })
})
