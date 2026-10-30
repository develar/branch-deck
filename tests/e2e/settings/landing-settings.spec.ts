// @spec spec/repository/settings.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availablePullRequests } from "../helpers/pull-request-helpers"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"

// the `issue_links` template, off Space: unset, its route is the pull request; JIRA-456 has 3 commits

test.describe("Settings → Landing", () => {
  test("should land the repository by a push into master once chosen, the main window taking the step, and back to the default", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("issue_links", { pullRequests: availablePullRequests() })
    await syncAndWaitForBranches()
    const window = await openSettingsWindow(page, { page: "landing" })
    const route = window.getByTestId("repository-setting-landing")
    const group = route.getByRole("group", { name: "Land by" })
    await expect(group.getByRole("button")).toHaveText(["Safe Push", "Push", "Pull Request"])
    // unset: the default of the host of origin, which the line says
    await expect(group.getByRole("button", { name: "Pull Request" })).toHaveAttribute("aria-pressed", "true")
    await expect(route.getByTestId("repository-setting-source")).toHaveText("Pull Request by default: the repository is not on Space")
    // off Patronus the pull requests of a straight stack are a stack of GitHub (docs/design/github-stacks.md)
    await expect(route).toContainText("Its pull request lands each piece, merged on GitHub; a straight stack is a stack of GitHub")
    // no robot lands it: no switch of the robots; the push size limit of every push stays
    await expect(window.getByRole("heading", { name: "Push", exact: true })).toBeVisible()
    await expect(window.getByRole("switch", { name: "New Patronus API" })).toHaveCount(0)

    // Push: written to .git/config, the main window syncs and offers Push to master
    let syncs = await finishedSyncCount(page)
    await group.getByRole("button", { name: "Push", exact: true }).click()
    await expect(group.getByRole("button", { name: "Push", exact: true })).toHaveAttribute("aria-pressed", "true")
    await expect(route.getByTestId("repository-setting-source")).toHaveText("Set in this repository")
    await expect(route).toContainText("A push of its copy lands each piece, without a robot or the CI of a pull request; a straight stack is a stack of GitHub")
    expect(await repo.getLanding()).toBe("push")
    await waitForSyncComplete(page, syncs)
    await expect(findBranchRow(page, "JIRA-456").getByTestId("safe-push-button")).toHaveText("Push to master")

    // Safe Push: the robots of Patronus are read again
    await group.getByRole("button", { name: "Safe Push" }).click()
    await expect(window.getByRole("switch", { name: "New Patronus API" })).toBeVisible()
    await expect(route).toContainText("A robot of Patronus checks each piece and pushes it into master; every pull request goes into master")
    expect(await repo.getLanding()).toBe("patronus")

    // Use Default: the line goes from .git/config, the default again, and the row has no push into master
    const reset = route.getByRole("button", { name: "Use Default" })
    await reset.hover()
    await expect(window.getByRole("tooltip")).toHaveText("Removes it from .git/config: back to Pull Request, the default off Space")
    syncs = await finishedSyncCount(page)
    await reset.click()
    await expect(group.getByRole("button", { name: "Pull Request" })).toHaveAttribute("aria-pressed", "true")
    await expect(reset).toHaveCount(0)
    expect(await repo.getLanding()).toBeNull()
    await waitForSyncComplete(page, syncs)
    await expect(findBranchRow(page, "JIRA-456")).toBeVisible()
    await expect(findBranchRow(page, "JIRA-456").getByTestId("safe-push-button")).toHaveCount(0)
  })
})
