// @spec openspec/specs/branch/landing-plan/spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { findBranchRow, getRowWait } from "../helpers/branch-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { landingRow, openLandingOverview } from "../helpers/review-status-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { expectClosed } from "../helpers/wait-helpers"

// The `landing_needs` template: lane-run (with ui-perf-nightly on it) uses Stills of stills-codec, whose Safe Push runs
const STILLS = robot("stills-codec", "stills-robot")

/**
 * Beside the template: lane-encode uses Bundle, which lane-bundle adds, and nothing lands lane-bundle: Land Together is
 * offered for the two
 */
async function addOpenNeed(repo: TestRepositoryBuilder): Promise<void> {
  await repo.commit("(lane-bundle) Bundle the scenarios", { "bundle/src/Bundle.kt": "package bundle\n\nclass Bundle\n" }, 1704124800)
  await repo.commit("(lane-encode) Encode each scenario", { "encode/src/Encode.kt": "package encode\n\nimport bundle.Bundle\n\nfun encode(bundle: Bundle) = 1\n" }, 1704126600)
}

function plan(page: Page): Locator {
  return page.getByTestId("landing-plan")
}

function unitCard(page: Page, name: string): Locator {
  return plan(page).locator(`[data-testid="plan-unit"][data-unit="${name}"]`)
}

function edgeChip(page: Page, from: string, needed: string): Locator {
  return plan(page).locator(`[data-testid="plan-edge"][data-from="${from}"][data-needed="${needed}"]`)
}

/** The waves of the plan, each its title and its units in their order */
async function waves(page: Page): Promise<string[]> {
  const titles = await plan(page).getByTestId("plan-wave").locator("h2").allTextContents()
  const grid = await plan(page)
    .getByTestId("plan-unit")
    .evaluateAll(elements => elements.map(element => ({ name: element.getAttribute("data-unit") ?? "", column: element.style.gridColumnStart })))
  return titles.map((title, index) => {
    const names = grid.filter(unit => unit.column === String(index + 1)).map(unit => unit.name)
    return `${title}: ${names.join(", ")}`
  })
}

async function showPlan(page: Page): Promise<void> {
  const popover = await openLandingOverview(page)
  await popover.getByRole("button", { name: "Show Plan" }).click()
  await expectClosed(popover)
  await expect(plan(page)).toBeVisible()
}

test.describe("Landing plan", () => {
  test(
    "should show the waves of the plan: the running robot now, what is ready next, what waits after what, each edge with its evidence on hover, and Land Together framed with its price, undone in one step",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("landing_needs", { patronus: availableRobots(STILLS) })
      await addOpenNeed(repo)
      await syncAndWaitForBranches()
      await expect(getRowWait(findBranchRow(page, "lane-run"))).toHaveText("Lands after stills-codec")

      // Show Plan of the landing overview: Landing at its plan
      await showPlan(page)
      await expect(page.getByTestId("main-view-landing")).toHaveAttribute("aria-pressed", "true")
      await expect(page.getByTestId("landing-section-plan")).toHaveAttribute("aria-pressed", "true")
      await expect(plan(page)).not.toHaveAttribute("data-stale")
      expect(await waves(page)).toEqual(["Landing now: stills-codec", "Next: lane-bundle", "After stills-codec: lane-run", "After lane-bundle: lane-encode"])
      await expect(unitCard(page, "stills-codec").getByTestId("plan-unit-robot")).toBeVisible()
      await expect(unitCard(page, "lane-run")).toContainText("2 branches · 2 commits")

      // the edge of the wait and its evidence; the one nothing lands
      const waits = edgeChip(page, "lane-run", "stills-codec")
      await expect(waits).toHaveAttribute("data-state", "waits")
      await expect(waits).toHaveText("Stills")
      await waits.hover()
      await expect(page.getByTestId("plan-edge-hover")).toContainText("Uses Stills of stills-codec in BuildLog.kt: stills-codec is landing in its Safe Push")
      await expect(page.getByTestId("plan-edge-hover")).toContainText("lane/src/BuildLog.kt")
      await expect(edgeChip(page, "lane-encode", "lane-bundle")).toHaveAttribute("data-state", "open")

      // Land Together, framed around the pair with its price; one unit after the restack, and the plan follows
      const proposal = plan(page).locator('[data-testid="plan-proposal"][data-from="lane-encode"]')
      await expect(proposal).toHaveAttribute("data-onto", "lane-bundle")
      await expect(proposal.getByTestId("plan-price")).toHaveText("1 robot instead of 2 · checks unknown · a failure holds both")
      await expect(proposal.getByRole("button", { name: "Land lane-bundle First" })).toBeVisible()
      await runAndWaitForSync(page, () => proposal.getByRole("button", { name: "Land Together" }).click())
      const toast = page.locator("[data-slot=toast]", { hasText: "Stacked lane-encode on lane-bundle" })
      await expect(toast).toBeVisible()
      await expect(unitCard(page, "lane-encode")).toHaveCount(0)
      await expect(unitCard(page, "lane-bundle")).toContainText("2 branches · 2 commits")
      await expect(plan(page).getByTestId("plan-proposal")).toHaveCount(0)
      expect(await waves(page)).toEqual(["Landing now: stills-codec", "Next: lane-bundle", "After stills-codec: lane-run"])

      // one undo puts the two units and the offer back (the toast shows the row it undid; the plan follows it too)
      await runAndWaitForSync(page, () => toast.getByRole("button", { name: "Undo" }).click())
      await page.getByTestId("main-view-landing").click()
      await expect(proposal).toBeVisible()
      expect(await waves(page)).toEqual(["Landing now: stills-codec", "Next: lane-bundle", "After stills-codec: lane-run", "After lane-bundle: lane-encode"])
    },
  )

  test("should open the plan at the edge of a start that lands after another from its Status cell, and show the row of a unit from its card", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("landing_needs", { patronus: availableRobots(STILLS) })
    await syncAndWaitForBranches()
    // the Status cell of the wait opens its edge in the plan
    await getRowWait(findBranchRow(page, "lane-run")).click()
    await expect(plan(page)).toBeVisible()
    await expect(edgeChip(page, "lane-run", "stills-codec")).toHaveAttribute("data-shown", "true")
    await expect(unitCard(page, "lane-run")).toHaveAttribute("data-shown", "true")

    // a click on the card of a unit shows its row in the branches
    await unitCard(page, "lane-run").click()
    await expect(findBranchRow(page, "lane-run")).toBeInViewport()
    await expect(findBranchRow(page, "lane-run")).toHaveAttribute("data-state", "open")
    await expect(plan(page)).toBeHidden()

    // back in Landing the highlight went with the move away
    await page.getByTestId("main-view-landing").click()
    await expect(edgeChip(page, "lane-run", "stills-codec")).not.toHaveAttribute("data-shown")
  })

  test("should switch between Plan and Statistics, and open each from the landing overview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("landing_needs", { patronus: availableRobots(STILLS) })
    await syncAndWaitForBranches()
    await page.getByTestId("main-view-landing").click()
    // the plan first
    await expect(plan(page)).toBeVisible()
    await page.getByTestId("landing-section-statistics").click()
    await expect(page.getByTestId("landing-view")).toHaveAttribute("data-section", "statistics")
    await expect(plan(page)).toHaveCount(0)
    await page.getByTestId("landing-section-plan").click()
    await expect(plan(page)).toBeVisible()

    await page.getByTestId("main-view-branches").click()
    const popover = await openLandingOverview(page)
    await popover.getByRole("button", { name: "Show Statistics" }).click()
    await expectClosed(popover)
    await expect(page.getByTestId("landing-view")).toHaveAttribute("data-section", "statistics")
    await page.getByTestId("main-view-branches").click()
    await showPlan(page)
    await expect(page.getByTestId("landing-view")).toHaveAttribute("data-section", "plan")
  })
  test("should land a stack that forks together with the branch it needs, the Safe Push of the whole stack its next step", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // landing_needs_fork: lane-run forks (lane-docs, ui-perf-nightly) and uses Stills of stills-codec, which nothing lands
    await setupRepo("landing_needs_fork", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    await page.getByTestId("main-view-landing").click()
    const proposal = plan(page).locator('[data-testid="plan-proposal"][data-from="lane-run"]')
    await expect(proposal).toHaveAttribute("data-onto", "stills-codec")
    await runAndWaitForSync(page, () => proposal.getByRole("button", { name: "Land Together · Safe Push Whole Stack" }).click())
    await expect(page.locator("[data-slot=toast]", { hasText: "Stacked lane-run on stills-codec" })).toBeVisible()
    // one unit of the four, whose next step is the Safe Push of its whole stack: built by the sync of the restack, it
    // lands all four in one robot (a stack branch that is not built holds it: `StackNotBuilt`)
    await page.getByTestId("main-view-landing").click()
    await expect(unitCard(page, "stills-codec")).toContainText("4 branches · 4 commits")
    await expect(unitCard(page, "lane-run")).toHaveCount(0)
    const overview = await openLandingOverview(page)
    const row = landingRow(overview, "stills-codec")
    await expect(row.getByTestId("landing-row-reason")).toHaveText("Safe Push lands 4 branches: Patronus checks the approvals when the robot starts")
    await expect(row.getByTestId("next-step")).toHaveText("Safe Push · 4")
    await expect(row.getByTestId("next-step")).toBeEnabled()
  })
})
