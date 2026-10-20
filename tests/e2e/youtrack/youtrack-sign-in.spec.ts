// @spec spec/repository/youtrack.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { routeCommand, routeCommandError } from "../helpers/wait-helpers"
import { SIGNED_IN, SIGNED_OUT, YOUTRACK_URL, linkIssuesToYouTrack, openYouTrackSettings } from "../helpers/youtrack-helpers"

test.describe("YouTrack sign-in", () => {
  test("should sign in in the browser to the YouTrack the issue links name, and sign out", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await linkIssuesToYouTrack(page)
    await syncAndWaitForBranches()
    const { settings, youtrack, auth, signIn, setStatus } = await openYouTrackSettings(page, SIGNED_OUT)
    await expect(auth).toHaveAttribute("data-status", "signedOut")
    // the YouTrack of the repository's issue links, and where it came from
    await expect(auth.getByLabel("YouTrack URL")).toHaveValue(YOUTRACK_URL)
    await expect(auth).toContainText("From .idea/vcs.xml of")

    await auth.getByRole("button", { name: "Sign In to YouTrack" }).click()
    const waiting = youtrack.getByTestId("youtrack-sign-in-waiting")
    await expect(waiting).toContainText("Waiting for the browser…")
    expect(signIn.starts).toEqual([{ url: YOUTRACK_URL }])
    const hubPage = `${YOUTRACK_URL}/hub/api/rest/oauth2/auth?client_id=localhost-auth`
    await expect.poll(() => getOpenedUrls(settings)).toEqual([hubPage])
    // the tab was closed: the same page again
    await waiting.getByRole("button", { name: "Open the Page Again" }).click()
    await expect.poll(() => getOpenedUrls(settings)).toEqual([hubPage, hubPage])

    // the browser came back: the backend kept the tokens and tells every window
    setStatus(SIGNED_IN)
    signIn.finish()
    await expect(settings.locator("[data-slot=toast]")).toContainText("Signed in to youtrack.example.com as Jane Doe")
    await expect(auth).toHaveAttribute("data-status", "signedIn")
    await expect(auth).toContainText("Signed in to youtrack.example.com as Jane Doe (jane)")
    await expect(auth.getByTestId("youtrack-auth-detail")).toContainText("Through the browser since")
    // the refresh token of the sign-in is listed in the profile of its Hub
    await auth.getByRole("button", { name: "Manage Sign-In" }).click()
    await expect.poll(async () => (await getOpenedUrls(settings)).at(-1)).toBe("https://hub.example.com/users/me")

    const signOuts = await routeCommand(settings, "sign_out_of_youtrack", () => {
      setStatus(SIGNED_OUT)
      return { status: "ok", data: null }
    })
    await auth.getByRole("button", { name: "Sign Out" }).click()
    await expect(settings.locator("[data-slot=toast]").filter({ hasText: "Signed out of YouTrack" })).toBeVisible()
    await expect(auth).toHaveAttribute("data-status", "signedOut")
    expect(signOuts).toHaveLength(1)
  })

  test("should cancel the browser sign-in, and paste a token instead", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const { settings, youtrack, auth, signIn, setStatus } = await openYouTrackSettings(page, SIGNED_OUT)
    // no issue link of the template points to a YouTrack: nothing is offered, the hint says what to enter
    await expect(auth.getByLabel("YouTrack URL")).toHaveValue("")
    await expect(auth.getByTestId("youtrack-url-hint")).toHaveText("The address you open YouTrack at, e.g. https://youtrack.example.com")
    await expect(auth.getByRole("button", { name: "Sign In to YouTrack" })).toBeDisabled()
    await auth.getByLabel("YouTrack URL").fill("https://youtrack.example.com/issue/ABC-1")

    await auth.getByRole("button", { name: "Sign In to YouTrack" }).click()
    await youtrack.getByTestId("youtrack-sign-in-waiting").getByRole("button", { name: "Cancel" }).click()
    await expect(auth.getByRole("button", { name: "Sign In to YouTrack" })).toBeEnabled()
    expect(signIn.cancels).toHaveLength(1)
    // a canceled sign-in is no error
    await expect(settings.locator("[data-slot=toast]")).toHaveCount(0)

    await auth.getByRole("button", { name: "Use a Token Instead" }).click()
    const form = auth.getByTestId("youtrack-token-form")
    await form.getByRole("button", { name: "Create Token…" }).click()
    await expect.poll(async () => (await getOpenedUrls(settings)).at(-1)).toBe("https://youtrack.example.com/issue/ABC-1/users/me?tab=account-security")
    // the backend cuts the URL to the YouTrack; a wrong token is refused before it is kept
    await routeCommandError(settings, "save_youtrack_token", "YouTrack rejected the token")
    await form.getByLabel("Token", { exact: true }).fill("perm-wrong")
    await form.getByRole("button", { name: "Save Token" }).click()
    await expect(settings.locator("[data-slot=toast]").filter({ hasText: "Cannot sign in to YouTrack" })).toBeVisible()
    await expect(form).toBeVisible()

    const saves = await routeCommand(settings, "save_youtrack_token", () => {
      setStatus({ ...SIGNED_IN, source: "pastedToken", manageUrl: `${YOUTRACK_URL}/users/me?tab=account-security` })
      return { status: "ok", data: SIGNED_IN.signedInAs }
    })
    await form.getByLabel("Token", { exact: true }).fill("perm-right")
    await form.getByRole("button", { name: "Save Token" }).click()
    await expect(auth).toHaveAttribute("data-status", "signedIn")
    expect(saves).toEqual([{ url: "https://youtrack.example.com/issue/ABC-1", token: "perm-right" }])
    await expect(auth.getByTestId("youtrack-auth-detail")).toContainText("With a pasted token")
    await auth.getByRole("button", { name: "Manage Tokens" }).click()
    await expect.poll(async () => (await getOpenedUrls(settings)).at(-1)).toBe(`${YOUTRACK_URL}/users/me?tab=account-security`)
  })

  test("should ask to sign in again once the sign-in expired", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const { settings, auth, setStatus } = await openYouTrackSettings(page, SIGNED_IN)
    await expect(auth).toHaveAttribute("data-status", "signedIn")

    // the Hub refused the refresh token in another window: the event asks again
    setStatus({ ...SIGNED_IN, problem: "expired" })
    await emitTauriEvent(settings, "youtrack-auth-changed")
    await expect(auth).toHaveAttribute("data-status", "expired")
    await expect(auth.getByTestId("youtrack-auth-problem")).toHaveText("Your YouTrack sign-in expired: sign in again")
    await expect(auth.getByLabel("YouTrack URL")).toHaveValue(YOUTRACK_URL)
  })
})
