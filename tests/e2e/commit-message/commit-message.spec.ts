import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchDetailsRow } from "../helpers/branch-helpers"
import { captureFloatingElementSnapshot } from "../helpers/aria-snapshot-helpers"

// the `issue_links` template gives this commit a body with a wrapped paragraph, a list, indented code and a trailer
const BRANCH = "JIRA-456"
const SUBJECT = "Add connection retry logic"

/** Waits until enter/exit animations are over (infinite ones such as spinners are ignored). */
async function waitForAnimationsFinished(page: Page): Promise<void> {
  await page.waitForFunction(() => document.getAnimations().every(animation => animation.playState !== "running" || animation.effect?.getComputedTiming().iterations === Infinity))
}

/** Expands the branch and hovers the full-message icon of the commit to open its popover. */
async function openCommitMessagePopover(page: Page): Promise<{ trigger: Locator, popover: Locator }> {
  await expandBranch(page, findBranchRow(page, BRANCH))
  const commitRow = getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: SUBJECT })
  const trigger = commitRow.getByLabel("Show full commit message")
  const popover = page.getByTestId("commit-message-popover")
  // hover only once the layout is final: a hover on a moving icon opens the popover with the pointer elsewhere,
  // and the next pointer event closes it again
  await waitForAnimationsFinished(page)
  await trigger.evaluate((element) => {
    element.scrollIntoView({ block: "center" })
  })
  await trigger.hover()
  await expect(popover).toBeVisible()
  return { trigger, popover }
}

test.describe("Commit message popover", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    // the unassigned commits card can render after the branches and push the commit list down
    await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
  })

  test("should reflow the commit body into paragraphs, lists and code", async ({ page }) => {
    const { popover } = await openCommitMessagePopover(page)

    await captureFloatingElementSnapshot(popover, "commit-message-popover")

    // hard-wrapped lines are joined into one paragraph
    const paragraphs = popover.locator("p")
    await expect(paragraphs).toHaveCount(3)
    const firstParagraph = await paragraphs.first().textContent()
    expect(firstParagraph).toBe("The pool gave up after the first failed connection attempt, so a short database restart surfaced as an error for every request that was in flight at that moment.")
    expect(firstParagraph).not.toContain("\n")

    // list items lose their marker, and a continuation line stays in its item
    const items = popover.locator("ul > li")
    await expect(items).toHaveCount(3)
    await expect(items.first()).toHaveText("wait with exponential backoff, starting at 100 ms and capped at 5 s, for up to 6 attempts")

    // indented code is kept verbatim without the indent
    await expect(popover.locator("pre")).toHaveText("retry: { attempts: 6, maxDelayMs: 5000 }")

    await expect(popover.getByText("Co-Authored-By: Jane Doe <jane@example.com>")).toBeVisible()
  })

  test("should stay open while the pointer moves into the popover", async ({ page }) => {
    const { trigger, popover } = await openCommitMessagePopover(page)
    // let the open animation (zoom) finish so the boxes are final
    await waitForAnimationsFinished(page)

    // glide from the icon to the middle of the popover, as a user would
    const from = (await trigger.boundingBox())!
    const to = (await popover.boundingBox())!
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2)
    await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 10 })
    await page.waitForTimeout(500)
    await expect(popover).toBeVisible()
  })

  test("should fit a short window and scroll the body", async ({ page }) => {
    await page.setViewportSize({ width: 1200, height: 360 })
    const { popover } = await openCommitMessagePopover(page)

    const metrics = await popover.evaluate(element => ({
      scrollHeight: element.scrollHeight,
      clientHeight: element.clientHeight,
      bottom: element.getBoundingClientRect().bottom,
      top: element.getBoundingClientRect().top,
      viewportHeight: window.innerHeight,
    }))
    expect(metrics.scrollHeight).toBeGreaterThan(metrics.clientHeight)
    expect(metrics.top).toBeGreaterThanOrEqual(0)
    expect(metrics.bottom).toBeLessThanOrEqual(metrics.viewportHeight)

    // the body scrolls to the trailer at the end
    await popover.evaluate((element) => {
      element.scrollTop = element.scrollHeight
    })
    await expect(popover.getByText("Co-Authored-By: Jane Doe <jane@example.com>")).toBeInViewport()
  })
})
