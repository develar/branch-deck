// @spec openspec/specs/repository/onboarding/spec.md
// The stores of the rows forget what they held of a row once the window takes the view of another session of its
// repository (`forgetRows`, a slot of a lease pool leased again): a row of the same name there is another branch
import { describe, expect, it } from "vite-plus/test"
import { createPullRequestDialogStore, openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import { pushConfirmStore, requestPushConfirm } from "@/stores/push-confirm"
import { requestReviewPopoverStore, setRequestReviewOpen } from "@/stores/request-review-popover"
import { openSafePushPopover, safePushStore, setLandAsking } from "@/stores/safe-push"
import { forgetRows } from "@/stores/view-rows"

describe("the stores of the rows of a view", () => {
  it("forget the confirm of a Force Push, whose accept pushes what the session before held", () => {
    let accepted = 0
    requestPushConfirm("auth", "row", {}, () => accepted++)
    expect(pushConfirmStore.getState().request?.branchName).toBe("auth")
    forgetRows()
    expect(pushConfirmStore.getState().request).toBeNull()
    expect(accepted).toBe(0)
  })

  it("close the Request Review… popover of a branch", () => {
    setRequestReviewOpen("auth", true)
    expect(requestReviewPopoverStore.getState().branchName).toBe("auth")
    forgetRows()
    expect(requestReviewPopoverStore.getState().branchName).toBeNull()
  })

  it("close the popover of a start and forget the clicks that wait for the plan of one", () => {
    openSafePushPopover({ branchName: "auth", mode: "rebaseAutosquash", placement: "row" })
    setLandAsking("auth", "rebaseAutosquash", "/repo")
    expect(safePushStore.getState().ask?.branchName).toBe("auth")
    expect(Object.keys(safePushStore.getState().asking)).toHaveLength(1)
    forgetRows()
    expect(safePushStore.getState()).toMatchObject({ ask: null, asking: {} })
  })

  it("close the Create Pull Request dialog of a branch", () => {
    openCreatePullRequestDialog("auth")
    expect(createPullRequestDialogStore.getState().branchName).toBe("auth")
    forgetRows()
    expect(createPullRequestDialogStore.getState().branchName).toBeNull()
  })
})
