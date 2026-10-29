// @spec spec/repository/youtrack.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { bringUnderStillPointer, findBranchRow } from "../helpers/branch-helpers"
import { expectSettingsOpened } from "../helpers/settings-helpers"
import { emitTauriEvent, waitForTauriListener } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { closeToast, holdToast } from "../helpers/toast-helpers"
import type { YouTrackFixture } from "../helpers/youtrack-helpers"
import { issueKeyLink, linkIssuesToYouTrack, openIssue, openYouTrackSettings, resolvedIssue, youTrackUrlOf } from "../helpers/youtrack-helpers"

/** The repository of the issue_links template with its keys linked to its fake YouTrack, which holds `youtrack` */
async function setUp(
  page: Page,
  setupRepo: (template: string, options: { youtrack: YouTrackFixture }) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  youtrack: YouTrackFixture,
): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("issue_links", { youtrack })
  await linkIssuesToYouTrack(repo, await youTrackUrlOf(repo))
  await syncAndWaitForBranches()
  return repo
}

const OPEN_ISSUES: YouTrackFixture = { issues: [openIssue("JIRA-456"), openIssue("ABC-123")], signedIn: "signIn" }

test.describe("YouTrack issue popover", () => {
  test("should show the issue of a key on hover, mark it fixed, and undo it from the toast", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setUp(page, setupRepo, syncAndWaitForBranches, OPEN_ISSUES)
    const link = issueKeyLink(page, "JIRA-456")
    await expect(link).toHaveAttribute("data-issue-key", "JIRA-456")

    await link.hover()
    const popover = page.getByTestId("issue-popover")
    await expect(popover.getByTestId("issue-details")).toContainText("The settings page loses the timeout")
    await expect(popover.getByTestId("issue-state")).toHaveText("Open")
    await expect(popover.getByTestId("issue-fields")).toContainText("PriorityMajor")
    await expect(popover.getByTestId("issue-fields")).toContainText("AssigneeJane Doe")
    expect((await repo.getYouTrack()).lookups).toEqual(["JIRA-456"])

    await popover.getByTestId("issue-action-mark-fixed").click()
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Marked JIRA-456 Fixed" })
    await expect(toast).toContainText("Was Open")
    expect((await repo.getYouTrack()).commands).toEqual([{ query: "State Fixed", issues: ["JIRA-456"] }])

    // the key in the toast's title is a link of the YouTrack too: its popover lies in the toasts, above them
    await toast.getByRole("link", { name: "JIRA-456" }).hover()
    await expect(page.getByTestId("issue-popover")).toHaveAttribute("data-issue-key", "JIRA-456")
    await expect(page.getByTestId("issue-popover")).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(page.getByTestId("issue-popover")).toHaveCount(0)

    // the toast of the issue: its Undo says in place that the issue is open again
    const held = await holdToast(toast)
    await held.getByRole("button", { name: "Undo" }).click()
    await expect(held.locator("[data-slot=toast-title]")).toHaveText("JIRA-456 is Open again")
    await expect(held.getByRole("button", { name: "Undo" })).toHaveCount(0)
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    expect((await repo.getYouTrack()).commands).toEqual([
      { query: "State Fixed", issues: ["JIRA-456"] },
      { query: "State Open", issues: ["JIRA-456"] },
    ])
  })

  test("should close as the view of its link hides", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setUp(page, setupRepo, syncAndWaitForBranches, OPEN_ISSUES)
    const link = issueKeyLink(page, "JIRA-456")
    const popover = page.getByTestId("issue-popover")
    await link.hover()
    await expect(popover).toHaveAttribute("data-issue-key", "JIRA-456")
    // ⌘3 with the pointer at rest: the link hides under it, the popover would stay over Landing
    await waitForTauriListener(page, "show_landing")
    await emitTauriEvent(page, "show_landing")
    await expect(page.getByTestId("landing-view")).toBeVisible()
    await expect(popover).toHaveCount(0)
    // ⌘1: the link shows it again on hover
    await emitTauriEvent(page, "show_branches")
    await page.mouse.move(640, 700)
    await link.hover()
    await expect(popover).toHaveAttribute("data-issue-key", "JIRA-456")
  })

  test(
    "should show no issue of a link that came under a still pointer, and show it once the pointer comes over it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setUp(page, setupRepo, syncAndWaitForBranches, OPEN_ISSUES)
      const link = issueKeyLink(page, "ABC-123")
      const popover = page.getByTestId("issue-popover")
      await expect(link).toHaveAttribute("data-issue-key", "ABC-123")
      // JIRA-456 above it expanded and collapsed: the link comes under the pointer, and the delay of the hover runs out
      await bringUnderStillPointer(page, link, findBranchRow(page, "JIRA-456"))
      await page.clock.fastForward(1000)
      await expect(popover).toHaveCount(0)
      // the pointer comes over it
      await page.mouse.move(640, 700)
      await link.hover()
      await expect(popover).toHaveAttribute("data-issue-key", "ABC-123")
    },
  )

  test("should open on the keyboard focus, and mark the issue of a branch fixed from its menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setUp(page, setupRepo, syncAndWaitForBranches, { issues: [openIssue("JIRA-456"), resolvedIssue("ABC-123", "Duplicate")], signedIn: "signIn" })
    const link = issueKeyLink(page, "ABC-123")
    await expect(link).toHaveAttribute("data-issue-key", "ABC-123")
    await link.focus()
    // a focus by a script is no visible focus: Tab from the link before it shows the ring
    await page.keyboard.press("Shift+Tab")
    await page.keyboard.press("Tab")
    await expect(link).toBeFocused()
    await expect(page.getByTestId("issue-popover")).toHaveAttribute("data-issue-key", "ABC-123")
    await page.keyboard.press("Escape")
    await expect(page.getByTestId("issue-popover")).toHaveCount(0)

    // the menu does not know the issue is resolved: the backend refuses to turn a Duplicate into Fixed
    await page.locator('[data-testid="branch-row"][data-branch-name="ABC-123"]').click({ button: "right" })
    await page.getByRole("menuitem", { name: "Mark ABC-123 Fixed" }).click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Cannot mark ABC-123 fixed" })).toContainText("ABC-123 is already Duplicate")
    expect((await repo.getYouTrack()).commands).toEqual([])
  })

  test("should show a resolved issue without Mark Fixed, and send an expired sign-in to Settings", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setUp(page, setupRepo, syncAndWaitForBranches, { issues: [resolvedIssue("JIRA-456", "Fixed"), openIssue("ABC-123")], signedIn: "signIn" })
    await issueKeyLink(page, "JIRA-456").hover()
    const popover = page.getByTestId("issue-popover")
    await expect(popover.getByTestId("issue-state")).toHaveText("Fixed · resolved 1 hour ago")
    await expect(popover.getByTestId("issue-action-open-in-youtrack")).toBeVisible()
    await expect(popover.getByTestId("issue-action-mark-fixed")).toHaveCount(0)
    await page.mouse.move(0, 0)
    await expect(popover).toHaveCount(0)

    // the Hub ended the sign-in: the access token expired and the refresh token is no longer known
    await repo.setYouTrack({ signedIn: "expired" })
    await issueKeyLink(page, "ABC-123").hover()
    await expect(popover.getByTestId("issue-sign-in")).toContainText("Your YouTrack sign-in expired: sign in again")
    await popover.getByRole("button", { name: "YouTrack Settings…" }).click()
    await expectSettingsOpened(page, { page: "youtrack" })
  })

  test("should leave the links of a signed-out user as they are", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the user created the token perm-right in Account Security of the YouTrack
    const repo = await setUp(page, setupRepo, syncAndWaitForBranches, { issues: [openIssue("JIRA-456")], tokens: ["perm-right"] })
    const url = await youTrackUrlOf(repo)
    const link = issueKeyLink(page, "JIRA-456")
    await expect(link).toHaveAttribute("href", `${url}/issue/JIRA-456`)
    await expect(link).not.toHaveAttribute("data-issue-key")
    await link.hover()

    // signed in in Settings: the session of the main window reads the sign-ins again, and the same hover shows the
    // issue; the first one asked nothing
    const { auth } = await openYouTrackSettings(page)
    await auth.getByRole("button", { name: "Use a Token Instead" }).click()
    const form = auth.getByTestId("youtrack-token-form")
    await form.getByLabel("Token", { exact: true }).fill("perm-right")
    await form.getByRole("button", { name: "Save Token" }).click()
    await expect(auth).toHaveAttribute("data-status", "signedIn")
    await expect(link).toHaveAttribute("data-issue-key", "JIRA-456")
    await page.mouse.move(0, 0)
    await link.hover()
    await expect(page.getByTestId("issue-popover").getByTestId("issue-details")).toBeVisible()
    expect((await repo.getYouTrack()).lookups).toEqual(["JIRA-456"])
  })

  test("should keep the branch preview open under the popover of its title", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setUp(page, setupRepo, syncAndWaitForBranches, OPEN_ISSUES)
    await page.locator('[data-testid="branch-row"][data-branch-name="JIRA-456"]').hover()
    await page.keyboard.press("Space")
    const preview = page.getByTestId("branch-preview")
    await expect(preview.getByTestId("branch-preview-body")).toBeFocused()

    await preview.getByRole("link", { name: "JIRA-456" }).first().hover()
    const popover = preview.getByTestId("issue-popover")
    // one move takes the pointer from the link onto Mark Fixed: the popover stays past the delay of a leave
    await popover.getByTestId("issue-action-mark-fixed").click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Marked JIRA-456 Fixed" })).toBeVisible()
    expect((await repo.getYouTrack()).commands).toHaveLength(1)
    await page.clock.fastForward(1000)
    await expect(preview).toBeVisible()
    // Escape closes the popover only, then the preview
    await popover.hover()
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)
    await expect(preview).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)
  })

  test("should say a failed read of the sign-in with Retry, never signed out for the session, and show the issues once a read answers", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the start can't read the sign-ins: an error that says so, Retry in it
    const repo = await setupRepo("issue_links", { youtrack: OPEN_ISSUES, remoteAnswers: { fail: ["authStatus"] } })
    await linkIssuesToYouTrack(repo, await youTrackUrlOf(repo))
    await syncAndWaitForBranches()
    const link = issueKeyLink(page, "JIRA-456")
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Cannot read the GitHub and YouTrack sign-ins" })
    await expect(toast).toContainText("the fixture fails this read")
    // without a status the link is a plain link, and the menu has no Mark Fixed
    await expect(link).toBeVisible()
    await expect(link).not.toHaveAttribute("data-issue-key")
    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Review Changes" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Mark JIRA-456 Fixed" })).toHaveCount(0)
    await page.keyboard.press("Escape")

    // Retry has the session read them again
    await repo.holdRemoteAnswers({ release: ["authStatus"] })
    await toast.getByRole("button", { name: "Retry" }).click()
    await expect(link).toHaveAttribute("data-issue-key", "JIRA-456")
    await link.hover()
    await expect(page.getByTestId("issue-popover").getByTestId("issue-state")).toHaveText("Open")
    await page.mouse.move(0, 0)
    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Mark JIRA-456 Fixed" })).toBeVisible()
    await page.keyboard.press("Escape")
    // the error stays until the user closes it
    await closeToast(toast)

    // a later read of the session fails (a focus two minutes after the last answer): said again, and no sign-out
    await repo.holdRemoteAnswers({ fail: ["authStatus"] })
    await repo.advanceBackendClock(121_000)
    await emitTauriEvent(page, "tauri://focus")
    await expect(toast).toContainText("the fixture fails this read")
    await expect(link).toHaveAttribute("data-issue-key", "JIRA-456")
    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Mark JIRA-456 Fixed" })).toBeVisible()
  })
})
