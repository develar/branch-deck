// @spec spec/branch/branch-filter.spec.md
import type { Locator, Page } from "@playwright/test"
import type { PatronusRobot, PatronusStartOptions } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { expectPushedToast, routePush } from "../helpers/push-helpers"
import { routeCommand } from "../helpers/wait-helpers"

// the `branch_filter` template: the stack core → api → web with cli forked off core, and seven branches on their own
const ROW_ORDER = ["deps", "lint", "search", "IJPL-102", "IJPL-101", "ci", "docs", "core", "cli", "api", "web"]

const START_OPTIONS: PatronusStartOptions = {
  targetBranch: "master",
  supported: true,
  profiles: [{ id: "lsc", name: "Large Scale Change", description: null, allowedPushModes: [] }],
  defaultProfileId: null,
}

/** The shown branch rows, `~` after a dimmed one */
async function shownRows(page: Page): Promise<string[]> {
  return page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => `${row.getAttribute("data-branch-name")}${row.hasAttribute("data-dimmed") ? "~" : ""}`))
}

function searchInput(page: Page): Locator {
  return page.getByRole("textbox", { name: "Filter branches" })
}

function chip(page: Page, name: "all" | "toPush" | "running" | "attention"): Locator {
  return page.getByRole("group", { name: "Branch state" }).locator(`[data-chip=${name}]`)
}

/** The branches the header counts as having work for the remote: "… commits to push in 11 branches", To Push's rule */
async function outgoingBranchCount(page: Page): Promise<number> {
  const label = await page.getByTestId("outgoing-indicator").getAttribute("aria-label")
  return Number(/in (\d+) branch/.exec(label ?? "")?.[1])
}

/** The `branch_filter` repository with these robots, synced; the unassigned commits card renders after the branches */
async function setup(page: Page, setupRepo: (template: string, options?: { patronus: unknown }) => Promise<unknown>, syncAndWaitForBranches: () => Promise<void>, ...robots: PatronusRobot[]) {
  await setupRepo("branch_filter", robots.length === 0 ? undefined : { patronus: availableRobots(...robots) })
  await syncAndWaitForBranches()
  await expect(page.getByTestId("branch-row")).toHaveCount(ROW_ORDER.length)
  for (const it of robots) {
    await expect(findBranchRow(page, it.branchName!).getByTestId("patronus-chip")).toBeVisible()
  }
}

/** Focuses a row from the keyboard: a click on the table header gives the table the focus, ↓ the rows from the first */
async function focusRow(page: Page, name: string) {
  await page.getByRole("columnheader", { name: "Branch Name" }).click()
  for (let i = 0; i <= ROW_ORDER.indexOf(name); i++) {
    await page.keyboard.press("ArrowDown")
  }
  await expect(findBranchRow(page, name)).toBeFocused()
}

test.describe("Branch filter", () => {
  test("should show the matches of a search and, dimmed, the branches they stack on", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches)
    expect(await shownRows(page)).toEqual(ROW_ORDER)

    // `/` from anywhere
    await page.mouse.move(0, 0)
    await page.keyboard.press("/")
    await expect(searchInput(page)).toBeFocused()
    // the name of api, the subject of docs ("Document the API"); core stays as the branch api is built on
    await page.keyboard.type("API")
    await expect.poll(() => shownRows(page)).toEqual(["docs", "core~", "api"])
    // the graph stays connected: core and api on one lane
    await expect(findBranchRow(page, "core")).toHaveAttribute("data-lane", "0")
    await expect(findBranchRow(page, "api")).toHaveAttribute("data-lane", "0")
    // a dimmed row is still a row: it expands
    await findBranchRow(page, "core").getByText("core", { exact: true }).click()
    await verifyBranchState(findBranchRow(page, "core"), "open")

    // the issue key of a prefix branch, every word somewhere
    await searchInput(page).fill("ijpl-200 fuzzy")
    await expect.poll(() => shownRows(page)).toEqual(["search"])

    // Escape clears the text, then leaves the search for the table
    await searchInput(page).press("Escape")
    await expect(searchInput(page)).toHaveValue("")
    await expect.poll(() => shownRows(page)).toEqual(ROW_ORDER)
    await searchInput(page).press("Escape")
    await expect(page.locator("table").filter({ has: page.getByTestId("branch-row") }).first()).toBeFocused()
    // ↓ from the search goes to the first row
    await page.keyboard.press("/")
    await page.keyboard.press("ArrowDown")
    await expect(findBranchRow(page, "deps")).toBeFocused()
  })

  test("should filter by a state chip, with the counts of all branches", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, robot("web", "running-robot"), robot("ci", "failed-robot", { status: "failure", finishTime: 1704142800 }))
    await expect(chip(page, "all")).toHaveText("All11")
    await expect(chip(page, "toPush")).toHaveText(`To Push${await outgoingBranchCount(page)}`)
    await expect(chip(page, "running")).toHaveText("Running1")
    await expect(chip(page, "attention")).toHaveText("Attention1")

    await chip(page, "running").click()
    await expect(chip(page, "running")).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => shownRows(page)).toEqual(["core~", "api~", "web"])
    // a chip combines with the text
    await searchInput(page).fill("deps")
    await expect(page.getByTestId("branch-filter-empty")).toHaveText("No branches with a running robot match “deps”Clear Filter")
    await searchInput(page).fill("")

    await chip(page, "attention").click()
    await expect.poll(() => shownRows(page)).toEqual(["ci"])
    // the chosen chip again: all branches
    await chip(page, "attention").click()
    await expect.poll(() => shownRows(page)).toEqual(ROW_ORDER)
  })

  test("should say when nothing matches and clear the filter", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches)
    await searchInput(page).fill("zzz")
    const empty = page.getByTestId("branch-filter-empty")
    await expect(empty).toContainText("No branches match “zzz”")
    await expect(page.getByTestId("branch-row")).toHaveCount(0)
    await empty.getByRole("button", { name: "Clear Filter" }).click()
    await expect(searchInput(page)).toHaveValue("")
    await expect.poll(() => shownRows(page)).toEqual(ROW_ORDER)
  })

  test("should keep a pushed branch under To Push until the filter changes", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches)
    const pushes = await routePush(page, {})
    const toPush = await outgoingBranchCount(page)
    await chip(page, "toPush").click()
    await focusRow(page, "docs")

    await page.keyboard.press("p")
    await expectPushedToast(page, "docs")
    expect(pushes.map(push => push.branchName)).toEqual(["docs"])
    await expect(chip(page, "toPush")).toHaveText(`To Push${toPush - 1}`)
    // where the user is looking: the row stays, and the focus with it
    await expect(findBranchRow(page, "docs")).toBeVisible()
    await expect(findBranchRow(page, "docs")).toBeFocused()

    await chip(page, "toPush").click()
    await chip(page, "toPush").click()
    await expect(findBranchRow(page, "docs")).toHaveCount(0)
  })

  test("should walk the branch rows with the arrows and preview the focused one with Space", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches)
    const deps = findBranchRow(page, "deps")
    await deps.getByText("deps", { exact: true }).click()
    await verifyBranchState(deps, "open")
    // over the commits of deps, down to the next branch row; the pointer stays on deps
    await page.keyboard.press("ArrowDown")
    await page.keyboard.press("ArrowDown")
    await expect(findBranchRow(page, "search")).toBeFocused()

    await page.keyboard.press("Space")
    const preview = page.getByTestId("branch-preview")
    await expect(preview.getByRole("heading")).toHaveText("search")
    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)

    // → expands, ← collapses, ← again goes to the branch the row is built on
    await focusRow(page, "api")
    await page.keyboard.press("ArrowRight")
    await verifyBranchState(findBranchRow(page, "api"), "open")
    await page.keyboard.press("ArrowLeft")
    await verifyBranchState(findBranchRow(page, "api"), "closed")
    await page.keyboard.press("ArrowLeft")
    await expect(findBranchRow(page, "core")).toBeFocused()
    // Enter toggles the focused row
    await page.keyboard.press("Enter")
    await verifyBranchState(findBranchRow(page, "core"), "open")
  })

  test("should select only the matches with Cmd+A and keep a hidden selected branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches)
    await searchInput(page).fill("api")
    await expect.poll(() => shownRows(page)).toEqual(["docs", "core~", "api"])
    await findBranchRow(page, "api").click({ modifiers: ["ControlOrMeta"] })
    await page.keyboard.press("ControlOrMeta+a")
    const selection = page.getByTestId("branch-selection-actions")
    // not core: it is shown only because api is built on it
    await expect(selection).toContainText("2 branches selected")
    await expect(findBranchRow(page, "core")).toHaveAttribute("data-selected", "false")

    await searchInput(page).fill("deps")
    await expect(selection).toContainText("2 branches selected · 2 hidden")
  })

  test("should show every branch again for a reveal of a hidden one", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches)
    await searchInput(page).fill("docs")
    await expect.poll(() => shownRows(page)).toEqual(["docs"])

    await page.getByTestId("outgoing-indicator").hover()
    const popover = page.getByTestId("outgoing-popover")
    await popover.locator("[data-testid=outgoing-branch][data-branch-name=lint]").getByRole("button", { name: "lint", exact: true }).click()
    await expect(searchInput(page)).toHaveValue("")
    await verifyBranchState(findBranchRow(page, "lint"), "open")
  })

  test("should open the Safe Push form of the focused row with S and start it with Enter", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, robot("web", "running-robot"))
    await routePush(page, {})
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const starts = await routeCommand(page, "start_patronus_robot", () => robot("lint", "started-robot", { sourceRevision: null }) satisfies PatronusRobot)

    // a robot runs on the commit web is at: S opens nothing
    await focusRow(page, "web")
    await page.keyboard.press("s")
    const form = page.getByTestId("patronus-start-form")
    await expect(form).toHaveCount(0)

    await focusRow(page, "lint")
    await page.keyboard.press("s")
    const start = form.getByRole("button", { name: "Push and Start Safe Push" })
    await expect(start).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
    expect(starts).toEqual([expect.objectContaining({ branchName: "lint", mode: "rebaseAutosquash" })])
  })

  test("should have no filter bar in a table of a few branches", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    await expect(page.getByTestId("branch-row")).toHaveCount(5)
    await expect(page.getByTestId("branch-filter")).toHaveCount(0)
  })
})
