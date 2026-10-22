// @spec spec/branch/next-step.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availableRobots, expectPatronusStarts, startRequest } from "../helpers/patronus-helpers"
import { routePush } from "../helpers/push-helpers"
import { openSafePushWithKey, popoverStart, slotSafePush } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { runAndWaitForSync } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { recordCommand } from "../helpers/wait-helpers"

/** The step of the row's slot */
function step(page: Page, branchName: string) {
  return findBranchRow(page, branchName).getByTestId("next-step-slot")
}

/** `name@depth` of the rows of the three branches of `landing_needs` */
async function depths(page: Page): Promise<string[]> {
  const depth = async (name: string) => `${name}@${(await findBranchRow(page, name).getAttribute("data-depth")) ?? 0}`
  return Promise.all(["stills-codec", "lane-run", "ui-perf-nightly"].map(depth))
}

/**
 * The robot of 2026-10-22 (`landing_needs`): lane-run imports Stills, which stills-codec adds, a branch it is not built
 * on; ui-perf-nightly is built on lane-run. Synced, the landing needs computed in the background: both say what their
 * Safe Push needs before any press, stills-codec needs nothing
 */
async function setupNeeds(
  page: Page,
  setupRepo: (template: string, options: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("landing_needs", { patronus: availableRobots() })
  await syncAndWaitForBranches()
  for (const name of ["lane-run", "ui-perf-nightly"]) {
    await expect(step(page, name)).toHaveAttribute("data-step", "needsBranch")
    await expect(step(page, name).getByTestId("next-step")).toHaveText("Needs stills-codec")
  }
  await expect(slotSafePush(page, "stills-codec")).toHaveText("Safe Push")
  expect(await depths(page)).toEqual(["stills-codec@0", "lane-run@0", "ui-perf-nightly@1"])
  return repo
}

/** lane-run stands on stills-codec, and its Safe Push lands both: nothing is needed any more */
async function expectStacked(page: Page) {
  await expect(page.getByText("Stacked lane-run on stills-codec")).toBeVisible()
  await expect.poll(() => depths(page)).toEqual(["stills-codec@0", "lane-run@1", "ui-perf-nightly@2"])
  await expect(slotSafePush(page, "lane-run")).toHaveText("Safe Push · 2")
  await expect(slotSafePush(page, "ui-perf-nightly")).toHaveText("Safe Push · 3")
}

test.describe("Landing needs", () => {
  test("should say on the row which branch its Safe Push needs before any press, and stack it on that branch from the slot", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupNeeds(page, setupRepo, syncAndWaitForBranches)
    const button = step(page, "lane-run").getByTestId("next-step")
    await button.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText(
      "“Show each scenario” of lane-run uses Stills (lane/src/BuildLog.kt:3), which “Name lossy stills” of stills-codec adds: git merges it, but the build of lane-run fails. Stack lane-run on stills-codec to land it too.",
    )
    await runAndWaitForSync(page, () => button.click())
    await expectStacked(page)
  })

  test("should stack the branch on the branch its Safe Push needs from the menu of a branch built on it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupNeeds(page, setupRepo, syncAndWaitForBranches)
    await openContextMenu(page, findBranchRow(page, "ui-perf-nightly"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Stack lane-run on stills-codec"))
    await expectStacked(page)
  })

  test("should say in the Safe Push popover why the build fails before the start, and stack the branch from it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupNeeds(page, setupRepo, syncAndWaitForBranches)

    const popover = await openSafePushWithKey(page, "ui-perf-nightly")
    const needs = popover.getByTestId("safe-push-landing-needs")
    await expect(needs).toContainText(
      "“Show each scenario” of lane-run uses Stills (lane/src/BuildLog.kt:3), which “Name lossy stills” of stills-codec adds: git merges it, but the build of ui-perf-nightly fails. Stack lane-run on stills-codec to land it too.",
    )
    await expect(popoverStart(page)).toHaveText("Start Anyway")
    // the fix restacks the root of what lands; the popover reads the start again, which lands stills-codec too
    await runAndWaitForSync(page, () => needs.getByRole("button", { name: "Stack lane-run on stills-codec" }).click())
    await expectStacked(page)
    await expect(popover.getByTestId("safe-push-landing-needs")).toHaveCount(0)
    await expect(popoverStart(page)).toHaveText("Safe Push")
  })

  test("should start anyway from the Safe Push popover with the override of what refuses it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches)
    const starts = await recordCommand(page, "start_patronus_robot")
    const pushes = await routePush(page, {})

    await openSafePushWithKey(page, "lane-run")
    await expect(popoverStart(page)).toHaveText("Start Anyway")
    await popoverStart(page).click()
    // the gate of the start and the start command get the override; nothing else refused it
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
    expect(starts).toEqual([expect.objectContaining({ branchName: "lane-run", allow: { allowDangling: false, allowMissing: true } })])
    // Patronus got the start of lane-run, past the gate that refused it
    await expectPatronusStarts(repo, [startRequest("lane-run")])
    expect(pushes.map(push => push.branchName)).toEqual(["lane-run"])
  })
})
