import { describe, expect, it } from "vite-plus/test"
import type { ActionState, PullRequestView, ReviewCandidate } from "@/lib/bindings"
import type { SelectContext } from "@/lib/menu"
import { describeCodeOwnersFact, formatRefused, formatRequestedToast, getRequestReviewItem } from "@/presenters/review"
import { disabled, enabled } from "@/test-support/actions"

const ilya: ReviewCandidate = { login: "ikorennoy", fullName: "Ilya Korennoy", groups: ["IJPL Code Platform"], preselected: true }

/** The open pull request #1078 as Rust reads it (`PullRequestView`): its candidates are `review::candidates` */
function pullRequest(patch: Partial<PullRequestView> = {}): PullRequestView {
  return { number: 1078, base: { kind: "target" }, draft: false, candidates: [], ...patch }
}

describe("what Request Review… says of the code owners", () => {
  it("says while Patronus checks them, when its check failed or doesn't tell, never nothing as if none were needed", () => {
    expect(describeCodeOwnersFact(null, true)).toBe("Asking Patronus which code owners must approve…")
    expect(describeCodeOwnersFact(null, false)).toBeNull()
    expect(describeCodeOwnersFact({ kind: "unavailable", reason: "error", message: "503" }, true)).toBe(
      "Patronus didn't answer which code owners must approve (503): ask the reviewers yourself.",
    )
    expect(describeCodeOwnersFact({ kind: "unknown", reason: "approvalsNotEvaluated" }, true)).toBe(
      "Patronus doesn't tell yet which code owners must approve: ask the reviewers yourself.",
    )
    expect(describeCodeOwnersFact({ kind: "notRequired" }, true)).toBeNull()
  })
})

describe("the Request Review… item", () => {
  const words = { subject: "auth", operations: {}, named: {} }

  /** The item of the open pull request `view` of `subject`, the request as Rust offers it, its popover opened by `open` */
  function itemOf(view: PullRequestView | undefined, request?: ActionState, open: () => void = () => {}, subject = "auth") {
    return getRequestReviewItem({ pullRequest: view, actions: request === undefined ? [] : [request], open }, { ...words, subject })
  }

  it("asks the chosen code owners on the open pull request, its press opens the popover", () => {
    const candidates = [ilya, { ...ilya, login: "bob", fullName: "Bob", preselected: false }]
    let opened = 0
    const item = itemOf(pullRequest({ candidates }), enabled("request-review")[0]!, () => opened++)
    expect(item).toMatchObject({ id: "request-review", label: "Request Review…", description: "Ask Ilya Korennoy to review #1078" })
    expect(item?.disabled).toBeUndefined()
    item?.onSelect({} as SelectContext)
    expect(opened).toBe(1)
    expect(itemOf(pullRequest(), enabled("request-review")[0]!)).toMatchObject({ description: "Ask for a review of #1078" })
  })

  it("is disabled with the reason Rust gives for a draft: work the remote lacks is pushed by the request", () => {
    const item = itemOf(pullRequest({ draft: true, candidates: [ilya] }), disabled("request-review", { code: "draft", number: 1078 }))
    expect(item).toMatchObject({ disabled: true, description: "#1078 is a draft: mark it ready for review on GitHub first" })
  })

  it("is in its place without an open pull request, disabled with what creates one, and not there while Rust doesn't offer it", () => {
    expect(itemOf(undefined, disabled("request-review", { code: "noOpenPullRequest", unlisted: null }))).toMatchObject({
      id: "request-review",
      disabled: true,
      description: "auth has no open pull request: Create Pull Request… asks for a review as it opens one",
    })
    expect(itemOf(undefined, disabled("request-review", { code: "noOpenPullRequest", unlisted: null }), () => {}, "stack:air")).toMatchObject({
      description: "Whole air stack has no open pull request: Create Pull Request of the Stack… asks for a review as it opens one",
    })
    expect(itemOf(undefined, disabled("request-review", { code: "noOpenPullRequest", unlisted: { status: "signedOut" } }))).toMatchObject({
      description: "No GitHub token to see pull requests",
    })
    expect(itemOf(pullRequest())).toBeNull()
  })
})

describe("the toast of a request", () => {
  const nameOf = (login: string) => (login === "ikorennoy" ? "Ilya Korennoy" : login)

  it("names whom GitHub asked, and whom it refused as a warning", () => {
    expect(formatRequestedToast({ number: 1078 }, { requested: ["ikorennoy"], refused: [] }, nameOf)).toEqual({
      title: "Requested a review from Ilya Korennoy on #1078",
    })
    const refused = { requested: ["ikorennoy"], refused: [{ login: "me", reason: "Review cannot be requested from pull request author." }] }
    expect(formatRequestedToast({ number: 1078 }, refused, nameOf)).toEqual({
      type: "warning",
      title: "Requested a review from Ilya Korennoy on #1078",
      description: "GitHub refused me: Review cannot be requested from pull request author.",
    })
  })

  it("is none when GitHub asked nobody: the error says why", () => {
    const result = { requested: [], refused: [{ login: "ikorennoy", reason: "Reviews may only be requested from collaborators." }] }
    expect(formatRequestedToast({ number: 1078 }, result, nameOf)).toBeNull()
    expect(formatRefused(result, nameOf)).toBe("Ilya Korennoy: Reviews may only be requested from collaborators.")
  })
})
