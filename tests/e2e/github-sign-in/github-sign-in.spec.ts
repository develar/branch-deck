// @spec spec/repository/github-sign-in.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { readClipboard } from "../helpers/clipboard-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest, unavailablePullRequests, VIEWER_LOGIN } from "../helpers/pull-request-helpers"
import { openLandingOverview } from "../helpers/review-status-helpers"
import { expectSettingsOpened, openSettingsWindow } from "../helpers/settings-helpers"
import { openLinkedSubWindow, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

/** The OAuth app of the build in these tests: the test server's build has none until a test names it */
const CLIENT_ID = "client-1"

/** The commands of `page` whose answer has not come yet (`__ipcStats` of the page mock) */
function pendingCommands(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as { __ipcStats: { pending: number } }).__ipcStats.pending)
}

test.describe("GitHub sign-in", () => {
  test("should sign in from the hint of a repository without a token, and ask GitHub again", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // GitHub has a pull request of the branch, which the app sees only with a token
    const { pullRequests } = availablePullRequests(pullRequest("JIRA-456", 760))
    const repo = await setupRepo("issue_links", { pullRequests: { ...unavailablePullRequests({ status: "signedOut" }), pullRequests } })
    await repo.setGitHubSignIn({ clientId: CLIENT_ID })
    await syncAndWaitForBranches()

    const overview = await openLandingOverview(page)
    await expect(overview.getByTestId("pull-request-status-line")).toContainText("No GitHub token to see pull requests")
    await expect(overview.getByTestId("pull-request-status-line-action")).toHaveText("Sign In…")
    await overview.getByTestId("pull-request-status-line-action").click()
    // Settings opens at GitHub; the window asks the backend itself
    const settings = await openLinkedSubWindow(page, await expectSettingsOpened(page, { page: "github" }))
    const github = settings.getByTestId("settings-page-github")
    await expect(github.getByTestId("github-auth")).toHaveAttribute("data-status", "signedOut")
    await expect(github.getByTestId("github-approval-note")).toHaveText(
      "An organization that restricts OAuth apps shows its private repositories only after an admin approves Branch Deck; gh is often approved there already",
    )

    await github.getByRole("button", { name: "Sign In to GitHub" }).click()
    const code = github.getByTestId("github-sign-in-code")
    await expect(code.getByTestId("github-user-code")).toHaveText("WDJB-MJHT")
    await expect(code).toContainText(/Waiting for GitHub · expires in 1[45]:\d\d/)
    await code.getByRole("button", { name: "Copy Code and Open GitHub" }).click()
    await expect.poll(() => getOpenedUrls(settings)).toEqual(["https://github.com/login/device"])
    expect(await readClipboard(settings)).toBe("WDJB-MJHT")

    // the backend asks GitHub for the token until the user entered the code there
    await expect.poll(async () => (await repo.getGitHubSignIn()).polls).toBeGreaterThan(0)
    expect(await repo.getGitHubSignIn()).toMatchObject({ pending: true, codes: 1 })
    await repo.setGitHubSignIn({ authorize: true })
    // the backend kept the token of GitHub's user and tells every window
    await expect(settings.getByText(`Signed in to GitHub as @${VIEWER_LOGIN}`)).toBeVisible()
    await expect(github.getByTestId("github-auth")).toHaveAttribute("data-status", "signedIn")
    await expect(github.getByTestId("github-auth")).toContainText(`Signed in as @${VIEWER_LOGIN}`)
    await expect(github.getByTestId("github-approval-note")).toContainText("request it in Manage on GitHub")
    expect(await repo.getGitHubSignIn()).toMatchObject({ pending: false })
    // the overview the hint was in closes: the chip of the row
    await page.keyboard.press("Escape")
    await expect(overview).toBeHidden()
    await expect(page.getByTestId("pull-request-chip")).toHaveCount(1)
  })

  test("should cancel the sign-in, and sign out", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await repo.setGitHubSignIn({ clientId: CLIENT_ID, signedIn: false })
    await syncAndWaitForBranches()
    const settings = await openSettingsWindow(page, { page: "github" })
    const github = settings.getByTestId("settings-page-github")

    await github.getByRole("button", { name: "Sign In to GitHub" }).click()
    await expect(github.getByTestId("github-user-code")).toHaveText("WDJB-MJHT")
    expect(await repo.getGitHubSignIn()).toMatchObject({ pending: true, codes: 1 })
    await github.getByTestId("github-sign-in-code").getByRole("button", { name: "Cancel" }).click()
    await expect(github.getByRole("button", { name: "Sign In to GitHub" })).toBeVisible()
    await expect.poll(async () => (await repo.getGitHubSignIn()).pending).toBe(false)
    // the wait ended with "canceled", which is no error
    await expect.poll(() => pendingCommands(settings)).toBe(0)
    await expect(settings.locator("[data-slot=toast]")).toHaveCount(0)

    // signed in elsewhere meanwhile: the next open of the window asks again
    await repo.setGitHubSignIn({ signedIn: true })
    await emitTauriEvent(page, "open_settings", { page: "github" })
    await reopenLinkedSubWindow(settings, await expectSettingsOpened(page, { page: "github" }, 2))
    await expect(github.getByTestId("github-auth")).toContainText(`Signed in as @${VIEWER_LOGIN}`)
    await github.getByRole("button", { name: "Sign Out" }).click()
    await expect(settings.getByText("Signed out of GitHub")).toBeVisible()
    await expect(github.getByTestId("github-auth")).toHaveAttribute("data-status", "signedOut")
  })

  test("should ask an organization to approve the token of the sign-in that can't see the repository", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { pullRequests: unavailablePullRequests({ status: "noAccess", source: "branchDeck", message: "no repository acme/monorepo" }) })
    await repo.setGitHubSignIn({ clientId: CLIENT_ID })
    await syncAndWaitForBranches()

    const overview = await openLandingOverview(page)
    await expect(overview.getByTestId("pull-request-status-line")).toContainText(
      "GitHub: no repository acme/monorepo (the token from the Branch Deck sign-in). An organization that restricts OAuth apps shows its private repositories only after an admin approves Branch Deck: request it, or run gh auth login",
    )
    await expect(overview.getByTestId("pull-request-status-line-action")).toHaveText("Request Access…")
    await overview.getByTestId("pull-request-status-line-action").click()
    await expect.poll(() => getOpenedUrls(page)).toEqual([`https://github.com/settings/connections/applications/${CLIENT_ID}`])
  })

  test("should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("issue_links", { pullRequests: unavailablePullRequests({ status: "signedOut" }) })
    await syncAndWaitForBranches()

    // the test server's build has no client id
    const overview = await openLandingOverview(page)
    await expect(overview.getByTestId("pull-request-status-line-action")).toHaveText("GitHub Settings…")
    await overview.getByTestId("pull-request-status-line-action").click()
    const settings = await openLinkedSubWindow(page, await expectSettingsOpened(page, { page: "github" }))
    const auth = settings.getByTestId("settings-page-github").getByTestId("github-auth")
    await expect(auth).toContainText("No GitHub token: run gh auth login, set GH_TOKEN, or store HTTPS credentials of github.com in git")
    await expect(auth.getByRole("button", { name: "Sign In to GitHub" })).toHaveCount(0)

    // gh signed in meanwhile: a sign-in or sign-out of any window asks again
    await repo.setGitHubSignIn({ ghToken: "gho_from_gh" })
    await emitTauriEvent(settings, "github-auth-changed")
    await expect(auth).toContainText("Uses the token of gh")
  })
})
