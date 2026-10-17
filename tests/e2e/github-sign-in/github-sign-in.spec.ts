// @spec spec/repository/github-sign-in.spec.md
import type { Page } from "@playwright/test"
import type { GitHubAuthStatus } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { readClipboard } from "../helpers/clipboard-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest, unavailablePullRequests } from "../helpers/pull-request-helpers"
import { openOutgoingOverview } from "../helpers/review-status-helpers"
import { expectSettingsOpened, openSettingsWindow } from "../helpers/settings-helpers"
import { openLinkedSubWindow, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { routeCommand } from "../helpers/wait-helpers"

const SIGNED_OUT: GitHubAuthStatus = { signInAvailable: true, signedInAs: null, tokenSource: null, manageUrl: "https://github.com/settings/connections/applications/client-1" }
const SIGNED_IN: GitHubAuthStatus = { ...SIGNED_OUT, signedInAs: "octocat", tokenSource: "branchDeck" }

/**
 * Answers the sign-in commands in the page: the status is `status()`, the code `WDJB-MJHT`, and the wait ends with
 * `octocat` when the returned function is called (with "canceled" after a cancel)
 */
async function routeSignIn(page: Page, status: () => GitHubAuthStatus): Promise<{ finish: () => void; cancels: Record<string, unknown>[] }> {
  await routeCommand(page, "get_github_auth_status", () => ({ status: "ok", data: status() }))
  await routeCommand(page, "start_github_sign_in", () => ({ status: "ok", data: { userCode: "WDJB-MJHT", verificationUri: "https://github.com/login/device", expiresIn: 900 } }))
  let finish: (answer: unknown) => void = () => {}
  await page.route("**/invoke/wait_github_sign_in", async route => {
    const answer = await new Promise(resolve => {
      finish = resolve
    })
    await route.fulfill({ json: answer })
  })
  const cancels = await routeCommand(page, "cancel_github_sign_in", () => {
    finish({ status: "error", error: { message: "canceled" } })
    return { status: "ok", data: null }
  })
  return { finish: () => finish({ status: "ok", data: "octocat" }), cancels }
}

test.describe("GitHub sign-in", () => {
  test("should sign in from the hint of a repository without a token, and ask GitHub again", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { pullRequests: unavailablePullRequests({ status: "signedOut" }) })
    let status = SIGNED_OUT
    await routeSignIn(page, () => status)
    await syncAndWaitForBranches()

    const overview = await openOutgoingOverview(page)
    await expect(overview.getByTestId("pull-request-status-line")).toContainText("No GitHub token to see pull requests")
    await expect(overview.getByTestId("pull-request-status-line-action")).toHaveText("Sign In…")
    await overview.getByTestId("pull-request-status-line-action").click()
    // Settings opens at GitHub; the window asks the backend itself
    let signIn: Awaited<ReturnType<typeof routeSignIn>> | null = null
    const settings = await openLinkedSubWindow(page, await expectSettingsOpened(page, { page: "github" }), {
      beforeLoad: async window => {
        signIn = await routeSignIn(window, () => status)
      },
    })
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

    // GitHub answers: the backend kept the token and tells every window
    status = SIGNED_IN
    await repo.setPullRequests(availablePullRequests(pullRequest("JIRA-456", 760)))
    signIn!.finish()
    await expect(settings.getByText("Signed in to GitHub as @octocat")).toBeVisible()
    await expect(github.getByTestId("github-auth")).toHaveAttribute("data-status", "signedIn")
    await expect(github.getByTestId("github-auth")).toContainText("Signed in as @octocat")
    await expect(github.getByTestId("github-approval-note")).toContainText("request it in Manage on GitHub")
    await emitTauriEvent(page, "github-auth-changed")
    // the overview the hint was in closes: the chip of the row
    await page.keyboard.press("Escape")
    await expect(overview).toBeHidden()
    await expect(page.getByTestId("pull-request-chip")).toHaveCount(1)
  })

  test("should cancel the sign-in, and sign out", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    let status = SIGNED_OUT
    let signIn: Awaited<ReturnType<typeof routeSignIn>> | null = null
    await syncAndWaitForBranches()
    const settings = await openSettingsWindow(
      page,
      { page: "github" },
      {
        beforeLoad: async window => {
          signIn = await routeSignIn(window, () => status)
        },
      },
    )
    const github = settings.getByTestId("settings-page-github")

    await github.getByRole("button", { name: "Sign In to GitHub" }).click()
    await github.getByTestId("github-sign-in-code").getByRole("button", { name: "Cancel" }).click()
    await expect(github.getByRole("button", { name: "Sign In to GitHub" })).toBeVisible()
    expect(signIn!.cancels).toHaveLength(1)
    // a canceled sign-in is no error
    await expect(settings.locator("[data-slot=toast]")).toHaveCount(0)

    // signed in elsewhere meanwhile: the next open of the window asks again
    status = SIGNED_IN
    await routeCommand(settings, "sign_out_of_github", () => {
      status = SIGNED_OUT
      return { status: "ok", data: null }
    })
    await emitTauriEvent(page, "open_settings", { page: "github" })
    await reopenLinkedSubWindow(settings, await expectSettingsOpened(page, { page: "github" }, 2))
    await github.getByRole("button", { name: "Sign Out" }).click()
    await expect(settings.getByText("Signed out of GitHub")).toBeVisible()
    await expect(github.getByTestId("github-auth")).toHaveAttribute("data-status", "signedOut")
  })

  test("should ask an organization to approve the token of the sign-in that can't see the repository", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { pullRequests: unavailablePullRequests({ status: "noAccess", source: "branchDeck", message: "no repository acme/monorepo" }) })
    await routeCommand(page, "get_github_auth_status", () => ({ status: "ok", data: SIGNED_IN }))
    await syncAndWaitForBranches()

    const overview = await openOutgoingOverview(page)
    await expect(overview.getByTestId("pull-request-status-line")).toContainText(
      "GitHub: no repository acme/monorepo (the token from the Branch Deck sign-in). An organization that restricts OAuth apps shows its private repositories only after an admin approves Branch Deck: request it, or run gh auth login",
    )
    await expect(overview.getByTestId("pull-request-status-line-action")).toHaveText("Request Access…")
    await overview.getByTestId("pull-request-status-line-action").click()
    await expect.poll(() => getOpenedUrls(page)).toEqual(["https://github.com/settings/connections/applications/client-1"])
  })

  test("should send a build without the sign-in to GitHub Settings, which says what gives a token, and name the token of gh", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("issue_links", { pullRequests: unavailablePullRequests({ status: "signedOut" }) })
    await syncAndWaitForBranches()

    // the test server's build has no client id
    const overview = await openOutgoingOverview(page)
    await expect(overview.getByTestId("pull-request-status-line-action")).toHaveText("GitHub Settings…")
    await overview.getByTestId("pull-request-status-line-action").click()
    const settings = await openLinkedSubWindow(page, await expectSettingsOpened(page, { page: "github" }))
    const auth = settings.getByTestId("settings-page-github").getByTestId("github-auth")
    await expect(auth).toContainText("No GitHub token: run gh auth login, set GH_TOKEN, or store HTTPS credentials of github.com in git")
    await expect(auth.getByRole("button", { name: "Sign In to GitHub" })).toHaveCount(0)

    // gh signed in meanwhile: a sign-in or sign-out of any window asks again
    await routeCommand(settings, "get_github_auth_status", () => ({ status: "ok", data: { signInAvailable: true, signedInAs: null, tokenSource: "gh", manageUrl: null } }))
    await emitTauriEvent(settings, "github-auth-changed")
    await expect(auth).toContainText("Uses the token of gh")
  })
})
