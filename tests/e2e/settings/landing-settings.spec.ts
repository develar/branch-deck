// @spec openspec/specs/repository/settings/spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availablePullRequests } from "../helpers/pull-request-helpers"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"
import { TEST_SERVER_URL } from "../helpers/test-server-url"

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
    // All repositories: the route is one of each repository, and the robots of Patronus a setting of the app
    await expect(window.getByTestId("landing-per-repository")).toContainText("The route into master, the GitHub repository and the companions are set for each repository")
    await expect(window.getByRole("switch", { name: "New Patronus API" })).toBeVisible()
    await window
      .getByTestId("landing-per-repository")
      .getByRole("button", { name: /^Open / })
      .click()
    await expect(window.getByTestId("settings-scope-repository")).toHaveAttribute("aria-selected", "true")
    const route = window.getByTestId("setting-landing")
    const group = route.getByRole("group", { name: "Land by" })
    await expect(group.getByRole("button")).toHaveText(["Safe Push", "Push", "Pull Request"])
    // unset: the default of the host of origin, which the line says
    await expect(group.getByRole("button", { name: "Pull Request" })).toHaveAttribute("aria-pressed", "true")
    await expect(route.getByTestId("setting-landing-source")).toHaveText("Pull Request by default: the repository is not on Space")
    // off Patronus the pull requests of a straight stack are a stack of GitHub (docs/design/github-stacks.md)
    await expect(route).toContainText("Its pull request lands each piece, merged on GitHub; a straight stack is a stack of GitHub")
    // the push size limit of every push stays; the robots are a setting of the app, on All repositories
    await expect(window.getByRole("heading", { name: "Push", exact: true })).toBeVisible()
    await expect(window.getByRole("switch", { name: "New Patronus API" })).toHaveCount(0)

    // Push: written to .git/config, the main window syncs and offers Push to master
    let syncs = await finishedSyncCount(page)
    await group.getByRole("button", { name: "Push", exact: true }).click()
    await expect(group.getByRole("button", { name: "Push", exact: true })).toHaveAttribute("aria-pressed", "true")
    await expect(route.getByTestId("setting-landing-source")).toHaveText(/^Only in repo-\d+Reset$/)
    await expect(route).toHaveAttribute("data-own", "true")
    await expect(route).toContainText("A push of its copy lands each piece, without a robot or the CI of a pull request; a straight stack is a stack of GitHub")
    expect(await repo.getLanding()).toBe("push")
    await waitForSyncComplete(page, syncs)
    await expect(findBranchRow(page, "JIRA-456").getByTestId("safe-push-button")).toHaveText("Push to master")

    // Safe Push
    await group.getByRole("button", { name: "Safe Push" }).click()
    await expect(route).toContainText("A robot of Patronus checks each piece and pushes it into master; every pull request goes into master")
    expect(await repo.getLanding()).toBe("patronus")

    // Reset: the value goes from .git/config, the default again, and the row has no push into master
    const reset = route.getByRole("button", { name: "Reset" })
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

  test("should refuse a route for every repository, which would break the repositories off Space", async ({ page, setupRepo }) => {
    const repo = await setupRepo("issue_links")
    const window = await openSettingsWindow(page, { page: "landing" })
    // the tab of every repository has no control of the route at all
    await expect(window.getByTestId("setting-landing")).toHaveCount(0)
    const refused = await page.request.post(`${TEST_SERVER_URL}/invoke/set_repository_config`, {
      headers: { "x-repository-id": repo.id },
      data: { repositoryPath: repo.path, change: { key: "landing", value: "pullRequest" }, scope: "global" },
    })
    expect((await refused.json()) as { status: string }).toMatchObject({ status: "error" })
  })
})
