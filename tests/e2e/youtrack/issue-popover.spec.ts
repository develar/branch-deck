// @spec spec/repository/youtrack.spec.md
import type { Page } from "@playwright/test"
import type { YouTrackAuthStatus, YouTrackIssue } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { expectSettingsOpened } from "../helpers/settings-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { routeCommand, routeCommandError } from "../helpers/wait-helpers"
import { SIGNED_IN, SIGNED_OUT, YOUTRACK_URL, linkIssuesToYouTrack, routeYouTrackStatus } from "../helpers/youtrack-helpers"

/** An open issue, or one resolved an hour before `resolvedBefore` (ms, the clock of the page) */
function issue(key: string, resolvedBefore: number | null = null): YouTrackIssue {
  const resolved = resolvedBefore !== null
  return {
    key,
    summary: "The settings page loses the timeout",
    url: `${YOUTRACK_URL}/issue/${key}`,
    resolved: resolvedBefore === null ? null : Math.floor(resolvedBefore / 1000) - 3600,
    state: { field: "State", value: resolved ? "Fixed" : "Open", resolved, hasFixed: true },
    fields: [
      { name: "Priority", value: "Major" },
      { name: "Assignee", value: "Jane Doe" },
    ],
  }
}

/** The repository of the issue_links template with its keys linked to the YouTrack, signed in as `status()` */
async function setUp(
  page: Page,
  setupRepo: (template: string) => Promise<unknown>,
  syncAndWaitForBranches: () => Promise<unknown>,
  status: () => YouTrackAuthStatus,
): Promise<{ lookups: Record<string, unknown>[] }> {
  await setupRepo("issue_links")
  await routeYouTrackStatus(page, status)
  const lookups = await routeCommand(page, "get_youtrack_issue", params => ({ status: "ok", data: { status: "found", issue: issue(String(params.key)) } }))
  await linkIssuesToYouTrack(page)
  await syncAndWaitForBranches()
  // the window asked for the status when it started, before the route: a sign-in of any window asks again
  await emitTauriEvent(page, "youtrack-auth-changed")
  return { lookups }
}

function keyLink(page: Page, key: string) {
  return page.locator(`[data-testid="branch-row"][data-branch-name="${key}"]`).getByRole("link", { name: key }).first()
}

test.describe("YouTrack issue popover", () => {
  test("should show the issue of a key on hover, mark it fixed, and undo it from the toast", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { lookups } = await setUp(page, setupRepo, syncAndWaitForBranches, () => SIGNED_IN)
    const states = await routeCommand(page, "set_youtrack_issue_state", params => ({
      status: "ok",
      data: { field: "State", previous: params.state === "Fixed" ? "Open" : "Fixed" },
    }))
    const link = keyLink(page, "JIRA-456")
    await expect(link).toHaveAttribute("data-issue-key", "JIRA-456")

    await link.hover()
    const popover = page.getByTestId("issue-popover")
    await expect(popover.getByTestId("issue-details")).toContainText("The settings page loses the timeout")
    await expect(popover.getByTestId("issue-state")).toHaveText("Open")
    await expect(popover.getByTestId("issue-fields")).toContainText("PriorityMajor")
    await expect(popover.getByTestId("issue-fields")).toContainText("AssigneeJane Doe")
    expect(lookups).toEqual([{ key: "JIRA-456" }])

    await popover.getByTestId("issue-action-mark-fixed").click()
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Marked JIRA-456 Fixed" })
    await expect(toast).toContainText("Was Open")
    expect(states).toEqual([{ key: "JIRA-456", state: "Fixed", expected: null }])

    // the key in the toast's title is a link of the YouTrack too: its popover lies in the toasts, above them
    await toast.getByRole("link", { name: "JIRA-456" }).hover()
    await expect(page.getByTestId("issue-popover")).toHaveAttribute("data-issue-key", "JIRA-456")
    await expect(page.getByTestId("issue-popover")).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(page.getByTestId("issue-popover")).toHaveCount(0)

    await toast.getByRole("button", { name: "Undo" }).click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "JIRA-456 is Open again" })).toBeVisible()
    expect(states).toEqual([
      { key: "JIRA-456", state: "Fixed", expected: null },
      { key: "JIRA-456", state: "Open", expected: "Fixed" },
    ])
  })

  test("should open on the keyboard focus, and mark the issue of a branch fixed from its menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setUp(page, setupRepo, syncAndWaitForBranches, () => SIGNED_IN)
    const link = keyLink(page, "ABC-123")
    await expect(link).toHaveAttribute("data-issue-key", "ABC-123")
    await link.focus()
    // a focus by a script is no visible focus: Tab from the link before it shows the ring
    await page.keyboard.press("Shift+Tab")
    await page.keyboard.press("Tab")
    await expect(link).toBeFocused()
    await expect(page.getByTestId("issue-popover")).toHaveAttribute("data-issue-key", "ABC-123")
    await page.keyboard.press("Escape")
    await expect(page.getByTestId("issue-popover")).toHaveCount(0)

    await routeCommandError(page, "set_youtrack_issue_state", "ABC-123 is already Duplicate")
    await page.locator('[data-testid="branch-row"][data-branch-name="ABC-123"]').click({ button: "right" })
    await page.getByRole("menuitem", { name: "Mark ABC-123 Fixed" }).click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Cannot mark ABC-123 fixed" })).toContainText("ABC-123 is already Duplicate")
  })

  test("should show a resolved issue without Mark Fixed, and send an expired sign-in to Settings", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    let status = SIGNED_IN
    await setUp(page, setupRepo, syncAndWaitForBranches, () => status)
    const now = await page.evaluate(() => Date.now())
    await routeCommand(page, "get_youtrack_issue", params => ({ status: "ok", data: { status: "found", issue: issue(String(params.key), now) } }))
    await keyLink(page, "JIRA-456").hover()
    const popover = page.getByTestId("issue-popover")
    await expect(popover.getByTestId("issue-state")).toHaveText("Fixed · resolved 1 hour ago")
    await expect(popover.getByTestId("issue-action-open-in-youtrack")).toBeVisible()
    await expect(popover.getByTestId("issue-action-mark-fixed")).toHaveCount(0)
    await page.mouse.move(0, 0)
    await expect(popover).toHaveCount(0)

    status = { ...SIGNED_IN, problem: "expired" }
    await routeCommand(page, "get_youtrack_issue", () => ({ status: "ok", data: { status: "signInProblem", problem: "expired" } }))
    await keyLink(page, "ABC-123").hover()
    await expect(popover.getByTestId("issue-sign-in")).toContainText("Your YouTrack sign-in expired: sign in again")
    await popover.getByRole("button", { name: "YouTrack Settings…" }).click()
    await expectSettingsOpened(page, { page: "youtrack" })
  })

  test("should leave the links of a signed-out user as they are", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    let status = SIGNED_OUT
    const { lookups } = await setUp(page, setupRepo, syncAndWaitForBranches, () => status)
    const link = keyLink(page, "JIRA-456")
    await expect(link).toHaveAttribute("href", `${YOUTRACK_URL}/issue/JIRA-456`)
    await expect(link).not.toHaveAttribute("data-issue-key")
    await link.hover()

    // signed in in Settings: the same hover shows the issue, the first one asked nothing
    status = SIGNED_IN
    await emitTauriEvent(page, "youtrack-auth-changed")
    await expect(link).toHaveAttribute("data-issue-key", "JIRA-456")
    await page.mouse.move(0, 0)
    await link.hover()
    await expect(page.getByTestId("issue-popover").getByTestId("issue-details")).toBeVisible()
    expect(lookups).toEqual([{ key: "JIRA-456" }])
  })

  test("should keep the branch preview open under the popover of its title", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setUp(page, setupRepo, syncAndWaitForBranches, () => SIGNED_IN)
    const states = await routeCommand(page, "set_youtrack_issue_state", () => ({ status: "ok", data: { field: "State", previous: "Open" } }))
    await page.locator('[data-testid="branch-row"][data-branch-name="JIRA-456"]').hover()
    await page.keyboard.press("Space")
    const preview = page.getByTestId("branch-preview")
    await expect(preview.getByTestId("branch-preview-body")).toBeFocused()

    await preview.getByRole("link", { name: "JIRA-456" }).first().hover()
    const popover = preview.getByTestId("issue-popover")
    await popover.getByTestId("issue-action-mark-fixed").click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Marked JIRA-456 Fixed" })).toBeVisible()
    expect(states).toHaveLength(1)
    await expect(preview).toBeVisible()
    // Escape closes the popover only, then the preview
    await popover.hover()
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)
    await expect(preview).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)
  })
})
