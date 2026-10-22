// @spec spec/repository/youtrack.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { waitForApiResponse } from "../helpers/wait-helpers"
import { completeYouTrackSignIn, issueKeyLink, linkIssuesToYouTrack, openIssue, openYouTrackSettings, youTrackHost, youTrackUrlOf } from "../helpers/youtrack-helpers"

test.describe("YouTrack sign-in", () => {
  test("should sign in in the browser to the YouTrack the issue links name, and sign out", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    const url = await youTrackUrlOf(repo)
    await linkIssuesToYouTrack(page, url)
    await syncAndWaitForBranches()
    const { settings, youtrack, auth } = await openYouTrackSettings(page)
    await expect(auth).toHaveAttribute("data-status", "signedOut")
    // the YouTrack of the repository's issue links, and where it came from
    await expect(auth.getByLabel("YouTrack URL")).toHaveValue(url)
    await expect(auth).toContainText("From .idea/vcs.xml of")

    await auth.getByRole("button", { name: "Sign In to YouTrack" }).click()
    const waiting = youtrack.getByTestId("youtrack-sign-in-waiting")
    await expect(waiting).toContainText("Waiting for the browser…")
    // the page of the YouTrack's Hub for the public client, back to the app's listener on the loopback address
    await expect.poll(() => getOpenedUrls(settings)).toHaveLength(1)
    const [hubPage] = await getOpenedUrls(settings)
    expect(hubPage?.startsWith(`${url}/hub/api/rest/oauth2/auth?`)).toBe(true)
    const query = new URL(hubPage!).searchParams
    expect([query.get("client_id"), query.get("access_type"), query.get("code_challenge_method")]).toEqual(["localhost-auth", "offline", "S256"])
    expect(query.get("redirect_uri")).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/)
    // the tab was closed: the same page again
    await waiting.getByRole("button", { name: "Open the Page Again" }).click()
    await expect.poll(() => getOpenedUrls(settings)).toEqual([hubPage, hubPage])

    // the browser came back: the backend kept the tokens and tells every window
    await completeYouTrackSignIn(settings)
    const host = youTrackHost(url)
    await expect(settings.locator("[data-slot=toast]")).toContainText(`Signed in to ${host} as Jane Doe`)
    await expect(auth).toHaveAttribute("data-status", "signedIn")
    await expect(auth).toContainText(`Signed in to ${host} as Jane Doe (jane)`)
    await expect(auth.getByTestId("youtrack-auth-detail")).toContainText("Through the browser since")
    // the main window got the sign-in too: its issue links open the popover
    await expect(issueKeyLink(page, "JIRA-456")).toHaveAttribute("data-issue-key", "JIRA-456")
    // the Hub is built into the YouTrack: the refresh token of the sign-in is listed in its Account Security
    await auth.getByRole("button", { name: "Manage Sign-In" }).click()
    await expect.poll(async () => (await getOpenedUrls(settings)).at(-1)).toBe(`${url}/users/me?tab=account-security`)

    await auth.getByRole("button", { name: "Sign Out" }).click()
    await expect(settings.locator("[data-slot=toast]").filter({ hasText: "Signed out of YouTrack" })).toBeVisible()
    await expect(auth).toHaveAttribute("data-status", "signedOut")
    await expect(issueKeyLink(page, "JIRA-456")).not.toHaveAttribute("data-issue-key")
  })

  test("should cancel the browser sign-in, and paste a token instead", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the user created the token perm-right in Account Security of the YouTrack
    const repo = await setupRepo("issue_links", { youtrack: { tokens: ["perm-right"] } })
    await syncAndWaitForBranches()
    const url = await youTrackUrlOf(repo)
    const { settings, youtrack, auth } = await openYouTrackSettings(page)
    // no issue link of the template points to a YouTrack: nothing is offered, the hint says what to enter
    await expect(auth.getByLabel("YouTrack URL")).toHaveValue("")
    await expect(auth.getByTestId("youtrack-url-hint")).toHaveText("The address you open YouTrack at, e.g. https://youtrack.example.com")
    await expect(auth.getByRole("button", { name: "Sign In to YouTrack" })).toBeDisabled()
    await auth.getByLabel("YouTrack URL").fill(`${url}/issue/ABC-1`)

    await auth.getByRole("button", { name: "Sign In to YouTrack" }).click()
    const waiting = youtrack.getByTestId("youtrack-sign-in-waiting")
    await expect(waiting).toContainText("Waiting for the browser…")
    await expect.poll(async () => (await repo.getYouTrack()).pending).toBe(true)
    const waitEnded = waitForApiResponse(settings, "/invoke/wait_youtrack_sign_in")
    await waiting.getByRole("button", { name: "Cancel" }).click()
    await expect(auth.getByRole("button", { name: "Sign In to YouTrack" })).toBeEnabled()
    // the backend ended the wait with "canceled", which is no error
    await waitEnded
    expect((await repo.getYouTrack()).pending).toBe(false)
    await expect(settings.locator("[data-slot=toast]")).toHaveCount(0)

    await auth.getByRole("button", { name: "Use a Token Instead" }).click()
    const form = auth.getByTestId("youtrack-token-form")
    await form.getByRole("button", { name: "Create Token…" }).click()
    await expect.poll(async () => (await getOpenedUrls(settings)).at(-1)).toBe(`${url}/issue/ABC-1/users/me?tab=account-security`)
    // the backend cuts the URL to the YouTrack; a token YouTrack does not know is refused before it is kept
    await form.getByLabel("Token", { exact: true }).fill("perm-wrong")
    await form.getByRole("button", { name: "Save Token" }).click()
    await expect(settings.locator("[data-slot=toast]").filter({ hasText: "Cannot sign in to YouTrack" })).toContainText("YouTrack rejected the token")
    await expect(form).toBeVisible()

    await form.getByLabel("Token", { exact: true }).fill("perm-right")
    await form.getByRole("button", { name: "Save Token" }).click()
    await expect(auth).toHaveAttribute("data-status", "signedIn")
    expect((await repo.getYouTrack()).meTokens).toEqual(["perm-wrong", "perm-right"])
    await expect(auth.getByTestId("youtrack-auth-detail")).toContainText("With a pasted token")
    await auth.getByRole("button", { name: "Manage Tokens" }).click()
    await expect.poll(async () => (await getOpenedUrls(settings)).at(-1)).toBe(`${url}/users/me?tab=account-security`)
  })

  test("should ask to sign in again once the sign-in expired", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the access token expired, and the Hub no longer knows the refresh token: the first lookup finds it out
    const repo = await setupRepo("issue_links", { youtrack: { issues: [openIssue("JIRA-456")], signedIn: "expired" } })
    const url = await youTrackUrlOf(repo)
    await linkIssuesToYouTrack(page, url)
    await syncAndWaitForBranches()
    await issueKeyLink(page, "JIRA-456").hover()
    await expect(page.getByTestId("issue-popover").getByTestId("issue-sign-in")).toContainText("Your YouTrack sign-in expired: sign in again")

    const { auth } = await openYouTrackSettings(page)
    await expect(auth).toHaveAttribute("data-status", "expired")
    await expect(auth.getByTestId("youtrack-auth-problem")).toHaveText("Your YouTrack sign-in expired: sign in again")
    await expect(auth.getByLabel("YouTrack URL")).toHaveValue(url)
  })
})
