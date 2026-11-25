// @spec openspec/specs/branch/landing-plan/spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availableRobots, expectPatronusStarts, startRequest } from "../helpers/patronus-helpers"
import { landingRow, openLandingOverview } from "../helpers/review-status-helpers"
import { openSafePushWithKey, popoverStart, safePushPopover } from "../helpers/safe-push-helpers"
import { runAndWaitForAnswer, runAndWaitForSync } from "../helpers/sync-helpers"
import { expectClosed } from "../helpers/wait-helpers"

function step(page: Page, branchName: string) {
  return findBranchRow(page, branchName).getByTestId("next-step-slot")
}

/** The branches and the commits of each unit of the Landing overview, by its row */
async function overviewUnits(page: Page): Promise<string[]> {
  const popover = await openLandingOverview(page)
  const rows = popover.getByTestId("landing-row")
  const units = await rows.evaluateAll(elements => elements.map(element => element.getAttribute("data-branch-name") ?? ""))
  await page.mouse.move(0, 0)
  await expectClosed(popover)
  return units.toSorted()
}

test.describe("Land Together", () => {
  test(
    "should land a branch with the branch it needs in one robot from the Safe Push popover, one unit in the Landing overview, and undo it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // landing_needs: lane-run (with ui-perf-nightly on it) uses Stills of stills-codec, which nothing lands
      await setupRepo("landing_needs", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await expect(step(page, "lane-run").getByTestId("next-step")).toHaveText("Needs stills-codec")
      expect(await overviewUnits(page)).toEqual(["lane-run", "stills-codec"])

      const popover = await openSafePushWithKey(page, "lane-run")
      const needs = popover.getByTestId("safe-push-landing-needs")
      const together = needs.getByRole("button", { name: "Land Together" })
      await expect(together).toBeFocused()
      await runAndWaitForSync(page, () => together.click())
      const toast = page.locator("[data-slot=toast]", { hasText: "Stacked lane-run on stills-codec" })
      await expect(toast).toBeVisible()
      // one unit: the stack of stills-codec lands all three in one robot, and nothing is needed any more
      await expect.poll(() => findBranchRow(page, "lane-run").getAttribute("data-depth")).toBe("1")
      await expect(safePushPopover(page).getByTestId("safe-push-landing-needs")).toHaveCount(0)
      await page.keyboard.press("Escape")
      expect(await overviewUnits(page)).toEqual(["stills-codec"])
      const overview = await openLandingOverview(page)
      await expect(landingRow(overview, "stills-codec")).toContainText("3 branches")
      await page.mouse.move(0, 0)
      await expectClosed(overview)

      // one undo puts the two units back, the need with them
      await runAndWaitForSync(page, () => toast.getByRole("button", { name: "Undo" }).click())
      await expect(step(page, "lane-run").getByTestId("next-step")).toHaveText("Needs stills-codec")
      expect(await overviewUnits(page)).toEqual(["lane-run", "stills-codec"])
    },
  )

  test("should group the unassigned commit a branch needs from the Safe Push popover", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("landing_needs", { patronus: availableRobots() })
    // lane-encode uses Encoder, which a commit of no branch adds
    await repo.commit("Add the encoder", { "bundle/src/Encoder.kt": "package bundle\n\nclass Encoder\n" }, 1704124800)
    await repo.commit("(lane-encode) Encode each scenario", { "encode/src/Encode.kt": "package encode\n\nimport bundle.Encoder\n\nfun encode(encoder: Encoder) = 1\n" }, 1704126600)
    await syncAndWaitForBranches()
    await expect(step(page, "lane-encode").getByTestId("next-step")).toHaveText(/^Group \w{8} into lane-encode$/)

    const popover = await openSafePushWithKey(page, "lane-encode")
    const group = popover.getByTestId("safe-push-landing-needs").getByRole("button", { name: /^Group \w{8} into lane-encode$/ })
    await expect(group).toBeFocused()
    await runAndWaitForSync(page, () => group.click())
    // the commit is the branch's now: nothing is needed
    await expect(page.locator("[data-slot=toast]", { hasText: "Moved 1 commit to lane-encode" })).toBeVisible()
    await expect(step(page, "lane-encode")).not.toHaveAttribute("data-step", "needsBranch")
  })

  test("should start the Safe Push of the branch built on both from the Safe Push popover of a cycle", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("landing_needs", { patronus: availableRobots() })
    // gauge uses Meter, which meter adds, and meter is built on gauge: the Safe Push of meter lands both
    await repo.commit("(gauge/meter) Add the meter", { "meter/src/Meter.kt": "package meter\n\nclass Meter\n" }, 1704124800)
    await repo.commit("(gauge) Gauge the lane", { "gauge/src/Gauge.kt": "package gauge\n\nimport meter.Meter\n\nfun gauge(meter: Meter) = 1\n" }, 1704126600)
    await syncAndWaitForBranches()
    await expect(step(page, "gauge").getByTestId("next-step")).toHaveText("Safe Push meter")

    const popover = await openSafePushWithKey(page, "gauge")
    const start = popover.getByTestId("safe-push-landing-needs").getByRole("button", { name: "Safe Push meter" })
    await expect(start).toBeFocused()
    // a click of the Safe Push of meter, which lands gauge with it: its popover says so, and its start lands both
    await start.click()
    await expect(safePushPopover(page)).toHaveAttribute("aria-label", "Safe Push of meter")
    await runAndWaitForAnswer(page, "execute", () => popoverStart(page).click())
    await expectPatronusStarts(repo, [startRequest("meter")])
  })
})
