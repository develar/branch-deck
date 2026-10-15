// @spec spec/repository/outgoing.spec.md
import type { Locator, Page } from "@playwright/test"
import { test as baseTest, testWithIssueLinks as test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton, verifyBranchState } from "../helpers/branch-helpers"
import type { DroppedCommit } from "../helpers/push-helpers"
import { confirmForcePush, expectPushedToast, pushedToast, routePush } from "../helpers/push-helpers"

// the `issue_links` template: 7 branches, none pushed; JIRA-456 has 3 commits, ABC-123 4, the others 1
const BRANCH_COUNT = 7
const TOTAL = 12

function getIndicator(page: Page): Locator {
  return page.getByTestId("outgoing-indicator")
}

async function openOverview(page: Page): Promise<Locator> {
  await getIndicator(page).hover()
  const popover = page.getByTestId("outgoing-popover")
  await expect(popover).toBeVisible()
  return popover
}

function getOverviewRow(popover: Locator, name: string): Locator {
  return popover.locator(`[data-testid="outgoing-branch"][data-branch-name="${name}"]`)
}

test.describe("Outgoing indicator", () => {
  test("should show the commits to push with a bar segment per branch", async ({ page }) => {
    const indicator = getIndicator(page)
    await expect(indicator).toHaveAccessibleName(`${TOTAL} commits to push in ${BRANCH_COUNT} branches`)
    await expect(indicator.getByTestId("outgoing-total")).toHaveText(String(TOTAL))
    const segments = indicator.getByTestId("outgoing-bar").locator("[data-hue]")
    await expect(segments).toHaveCount(BRANCH_COUNT)
    // nothing is pushed yet: every push creates the remote branch, a safe push
    await expect(indicator.getByTestId("outgoing-bar").locator("[data-hue=primary]")).toHaveCount(BRANCH_COUNT)
  })

  test("should list the outgoing branches in the overview on hover", async ({ page }) => {
    const popover = await openOverview(page)
    await expect(popover.getByRole("heading")).toHaveText(`${TOTAL} commits to push in ${BRANCH_COUNT} branches`)
    const rows = popover.getByTestId("outgoing-branch")
    await expect(rows).toHaveCount(BRANCH_COUNT)
    // the order of the branch table
    const tableOrder = await page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name")))
    expect(await rows.evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name")))).toEqual(tableOrder)

    const row = getOverviewRow(popover, "JIRA-456")
    // all 3 are the user's: the count is not repeated in parentheses
    await expect(row).toContainText("3↑")
    await expect(row).not.toContainText("(3)↑")
    await expect(row).toContainText("new")
    await expect(row.getByRole("button", { name: "Create Remote" })).toBeEnabled()
    await expect(popover.getByTestId("outgoing-legend")).toHaveText("push")
  })

  test("should show and expand the row of a branch clicked in the overview", async ({ page }) => {
    const popover = await openOverview(page)
    await verifyBranchState(findBranchRow(page, "ABC-123"), "closed")

    await getOverviewRow(popover, "ABC-123").getByRole("button", { name: "ABC-123", exact: true }).click()

    await expect(popover).toBeHidden()
    await verifyBranchState(findBranchRow(page, "ABC-123"), "open")
    await expect(findBranchRow(page, "ABC-123")).toBeInViewport()
  })

  test("should push a branch from the overview and follow the push results", async ({ page }) => {
    await routePush(page, {
      // the remote had a commit the branch no longer has
      "ABC-123": { unpushedCommits: ["a1"], commitsBehind: 1 },
    })
    const popover = await openOverview(page)

    await getOverviewRow(popover, "JIRA-456").getByRole("button", { name: "Create Remote" }).click()
    await expectPushedToast(page, "JIRA-456", "3 commits · created the remote branch")
    // pushed: gone from the overview, the popover stays open
    await expect(getOverviewRow(popover, "JIRA-456")).toHaveCount(0)
    await expect(popover.getByRole("heading")).toHaveText(`${TOTAL - 3} commits to push in ${BRANCH_COUNT - 1} branches`)

    const row = getOverviewRow(popover, "ABC-123")
    await row.getByRole("button", { name: "Create Remote" }).click()
    await expect(row).toHaveAttribute("data-status", "force")
    await expect(row).toContainText("1↑ · 1↓")
    await expect(row).toContainText("force")
    await expect(row.getByRole("button", { name: "Force Push (1)" })).toBeEnabled()
    await expect(popover.getByTestId("outgoing-legend")).toHaveText("pushdrops remote commits")

    const indicator = getIndicator(page)
    await expect(indicator.getByTestId("outgoing-total")).toHaveText(String(TOTAL - 3 - 4 + 1))
    await expect(indicator.getByTestId("outgoing-bar").locator("[data-hue=warning]")).toHaveCount(1)
  })
})

test.describe("Push progress", () => {
  test("should show the percent on the push button and git's last line on hover while the push runs", async ({ page }) => {
    let release!: () => void
    await routePush(page, {
      "JIRA-456": {
        progress: [
          { percent: 15, transferred: null, speed: null, line: "Compressing objects: 100% (3/3), done." },
          { percent: 55, transferred: "1.23 MiB", speed: "2.40 MiB/s", line: "Writing objects:  50% (2/4), 1.23 MiB | 2.40 MiB/s" },
        ],
        release: new Promise(resolve => release = resolve),
      },
    })
    const row = findBranchRow(page, "JIRA-456")
    const button = getPushButton(row)
    // the row shows the count, the label is its name and leads its description
    await expect(button).toHaveText("3")
    await expect(button).toHaveAccessibleName("Create Remote (3)")

    await button.click()
    await expect(button).toHaveText("55%")
    await expect(button).toHaveAccessibleName("Pushing 55%")
    await expect(button).toHaveAttribute("aria-busy", "true")
    // the pointer stays on the button after the click: its popover is open
    const progress = page.getByTestId("push-progress")
    await expect(progress).toContainText("Writing objects:  50% (2/4), 1.23 MiB | 2.40 MiB/s")
    // git's line says the amount already: no second line repeats it
    await expect(progress).not.toContainText("1.23 MiB · 2.40 MiB/s")

    release()
    await expectPushedToast(page, "JIRA-456")
    await expect(button).not.toHaveAttribute("aria-busy")
    await expect(button).toHaveAccessibleName("Push")
  })
})

/** A commit someone else pushed to the remote branch, three hours before the fixed clock of the page */
function theirCommit(id: string, subject: string, author: string): DroppedCommit {
  return { id, subject, author, authorTime: Date.parse("2024-01-02T07:00:00Z") / 1000 }
}

test.describe("Force push", () => {
  test("should force push a branch the remote has commits for, leased on the remote head the row shows", async ({ page }) => {
    const requests = await routePush(page, {
      "ABC-123": [
        // someone else pushed a commit the branch lacks
        { unpushedCommits: ["a1"], commitsBehind: 1, remoteTip: "remote-head-1", droppedCommits: [theirCommit("c0ffee0123456789", "Fix login", "Ann")] },
        {},
      ],
    })
    const row = findBranchRow(page, "ABC-123")
    await getPushButton(row).click()
    const forcePush = row.getByRole("button", { name: "Force Push (1)" })
    await expect(forcePush).toBeEnabled()
    // the description says what the force push does, then lists what it drops
    await forcePush.hover()
    await expect(page.getByText("Force push replaces the remote branch. 1 commit to push. The remote has 1 commit your branch doesn't (by Ann).")).toBeVisible()
    const dropped = page.getByTestId("dropped-commits")
    await expect(dropped).toContainText("Fix login")
    await expect(dropped).toContainText("Ann · c0ffee01 · 3 hours ago")
    // only the hover: no confirm before the click
    await expect(page.getByTestId("push-confirm")).toHaveCount(0)

    // it drops Ann's commit: the click asks first, the Force Push in the description pushes
    await forcePush.click()
    await expect(forcePush).toHaveAttribute("aria-expanded", "true")
    expect(requests).toHaveLength(1)
    await confirmForcePush(page, "Force Push (1)")

    await expect.poll(() => requests.map(request => request.expectedRemote)).toEqual([null, "remote-head-1"])
    await expect(row.getByRole("button", { name: "Push", exact: true })).toBeDisabled()
  })

  test("should show what the remote has when it moved since the status, and force push over it from the toast", async ({ page }) => {
    const requests = await routePush(page, {
      "JIRA-456": [
        {
          outcome: "remoteMoved",
          unpushedCommits: ["j1", "j2", "j3"],
          commitsBehind: 3,
          remoteTip: "moved-head",
          droppedCommits: [theirCommit("aaaa000011112222", "Fix A", "Ann"), theirCommit("bbbb000011112222", "Fix B", "Bob"), theirCommit("cccc000011112222", "Fix C", "Ann")],
        },
        {},
      ],
    })
    const row = findBranchRow(page, "JIRA-456")
    await getPushButton(row).click()

    const toast = page.locator("[data-slot=toast]", { hasText: "JIRA-456 changed on the remote" })
    await expect(toast).toContainText("3 commits you don't have (by Ann and Bob). Nothing was pushed.")
    await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
    // the row shows the fetched remote: a force push that drops the 3 commits
    await expect(row.getByRole("button", { name: "Force Push (3)" })).toBeEnabled()
    await expect(row).toContainText("3↓")

    // the push drops the 3 commits: the toast opens the confirm of the row's push button
    await toast.getByRole("button", { name: "Force Push" }).click()
    await expect(toast).toHaveCount(0)
    expect(requests).toHaveLength(1)
    await confirmForcePush(page, "Force Push (3)")

    await expectPushedToast(page, "JIRA-456")
    expect(requests.map(request => request.expectedRemote)).toEqual([null, "moved-head"])
  })

  test("should push a rebased branch at once: its force push drops nothing", async ({ page }) => {
    const requests = await routePush(page, {
      // the remote has the commits as they were before the rebase onto a newer baseline
      "ABC-123": [{ unpushedCommits: ["a1"], fastForward: false }, {}],
    })
    const row = findBranchRow(page, "ABC-123")
    await getPushButton(row).click()
    const forcePush = row.getByRole("button", { name: "Force Push (1)" })
    await expect(forcePush).toBeEnabled()

    await forcePush.click()

    await expect.poll(() => requests).toHaveLength(2)
    await expect(page.getByTestId("push-confirm")).toHaveCount(0)
  })
})

test.describe("Push button", () => {
  test("should keep the colour of the push while its description is open", async ({ page }) => {
    const button = getPushButton(findBranchRow(page, "JIRA-456"))
    await expect(button).toHaveAccessibleName("Create Remote (3)")
    await button.hover()
    await expect(page.getByTestId("push-description")).toBeVisible()
    await expect(button).toHaveAttribute("aria-expanded", "true")

    // the ghost variant turns an open trigger to the foreground; a routine push keeps its hue in the icon and the tint
    const colours = () => button.evaluate((element) => {
      const probe = document.createElement("span")
      document.body.append(probe)
      const styleOf = (className: string) => {
        probe.className = className
        const style = getComputedStyle(probe)
        return { color: style.color, background: style.backgroundColor }
      }
      const result = {
        label: getComputedStyle(element).color,
        icon: getComputedStyle(element.querySelector("svg")!).color,
        background: getComputedStyle(element).backgroundColor,
        primary: styleOf("text-primary").color,
        foreground: styleOf("text-foreground").color,
        tint: styleOf("bg-primary/10").background,
      }
      probe.remove()
      return result
    })
    await expect.poll(async () => {
      const { label, icon, background, primary, foreground, tint } = await colours()
      return icon === primary && label === foreground && background === tint
    }).toBe(true)
  })

  test("should describe a disabled push on hover", async ({ page }) => {
    await routePush(page, {})
    const button = getPushButton(findBranchRow(page, "JIRA-456"))
    await button.click()
    await expectPushedToast(page, "JIRA-456")
    await expect(button).toBeDisabled()
    // the click opened the description: close it, then hover the disabled button
    await page.keyboard.press("Escape")
    const description = page.getByTestId("push-description")
    await expect(description).toHaveCount(0)

    await page.mouse.move(0, 0)
    await button.hover()

    await expect(description).toContainText("Everything is pushed.")
  })
})

baseTest.describe("Outgoing indicator without outgoing commits", () => {
  baseTest("should hide when every branch is pushed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    await routePush(page, {})
    const indicator = getIndicator(page)
    await expect(indicator).toHaveAccessibleName("1 commit to push in 1 branch")

    await getPushButton(findBranchRow(page, "test-branch")).click()

    await expectPushedToast(page, "test-branch")
    await expect(indicator).toHaveCount(0)
  })
})
