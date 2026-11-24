// @spec openspec/specs/repository/backend-protocol/spec.md
import { expect, test } from "../fixtures/test-fixtures"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"
import { openToast } from "../helpers/toast-helpers"

test.describe("Restart to Update", () => {
  test("a binary of the app replaced while its window is open is one toast of the main window, whose Restart to Update restarts the app", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()
    const title = "A newer Branch Deck is installed"
    await expect(openToast(page, title)).toHaveCount(0)

    await repo.replaceTheBinaryWhileRunning()
    const toast = openToast(page, title)
    await expect(toast).toBeVisible()
    // a second bridge of the new build asks again: one toast still tells it
    await repo.replaceTheBinaryWhileRunning()
    await expect(openToast(page, title)).toHaveCount(1)

    await runAndWaitForAnswer(page, "restart_app", () => toast.getByRole("button", { name: "Restart to Update" }).click())
  })
})
