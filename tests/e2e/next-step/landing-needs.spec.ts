// @spec spec/branch/next-step.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availableRobots, expectPatronusStarts, startRequest, wholeStackStartRequest } from "../helpers/patronus-helpers"
import { expectPushedToRemote, pushedBranches, remotePushes } from "../helpers/push-helpers"
import { openSafePushWithKey, popoverStart, slotSafePush } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu, openSubmenu } from "../helpers/selection-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { runAndWaitForAnswer, runAndWaitForSync } from "../helpers/sync-helpers"
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
  test("should check what a branch needs to land before it offers its Safe Push, S and the popover waiting for the answer", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("landing_needs", { patronus: availableRobots() })
    await repo.holdRemoteAnswers({ hold: ["landingNeeds"] })
    const rowsAt = Date.now()
    await syncAndWaitForBranches()
    // Rust computes the needs after the view: the page waits for them, the step too
    await expect.poll(() => repo.getArrivedAnswers()).toContain("landingNeeds")
    for (const name of ["lane-run", "stills-codec"]) {
      await expect(step(page, name)).toHaveAttribute("data-step", "pending")
    }
    await step(page, "lane-run").getByTestId("next-step").hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText("Checking what lane-run needs to land…")
    // S says it, no popover, no start whose gate the page could have told
    await findBranchRow(page, "lane-run").focus()
    await page.keyboard.press("s")
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push of lane-run waits for the check of what it needs" })).toBeVisible()
    expect((await repo.getPatronusLog()).starts).toEqual([])

    // the answer: lane-run needs stills-codec, stills-codec needs nothing
    await repo.holdRemoteAnswers({ release: ["landingNeeds"] })
    await expect(step(page, "lane-run")).toHaveAttribute("data-step", "needsBranch")
    await expect(step(page, "lane-run").getByTestId("next-step")).toHaveText("Needs stills-codec")
    await expect(slotSafePush(page, "stills-codec")).toHaveText("Safe Push")
    // how long the needs of the three branches of the template take, held time aside, is in the log of the test
    test.info().annotations.push({ type: "rows to needs (ms, held)", description: String(Date.now() - rowsAt) })
  })

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
    const starts = await recordCommand(page, "execute")

    await openSafePushWithKey(page, "lane-run")
    await expect(popoverStart(page)).toHaveText("Start Anyway")
    await runAndWaitForAnswer(page, "execute", () => popoverStart(page).click())
    // the start runs with the override, which its plan and the gate of the engine get; nothing else refused it
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
    expect(starts).toEqual([
      expect.objectContaining({
        intent: expect.objectContaining({ subject: { kind: "branch", name: "lane-run" }, allow: expect.objectContaining({ allowDangling: false, allowMissing: true }) }),
      }),
    ])
    // Patronus got the start of lane-run, past the gate that refused it
    await expectPatronusStarts(repo, [startRequest("lane-run")])
    // it pushed lane-run alone, before the start
    await expectPushedToRemote(repo, ["lane-run"])
    expect(await pushedBranches(repo)).toEqual(["lane-run"])
  })

  test("should refuse the Safe Push of a whole stack whose build needs a branch outside it, with the restack and Start Anyway", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // lane-run forks (ui-perf-nightly and lane-docs are built on it): its whole stack lands from its stack branch, and the
    // landing needs of the view are of branches, so the gate of the start checks the whole stack
    const repo = await setupRepo("landing_needs_fork", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    await expect(step(page, "lane-run").getByTestId("next-step")).toHaveText("Needs stills-codec")
    const starts = await recordCommand(page, "execute")

    await findBranchRow(page, "lane-run").click({ button: "right" })
    await openSubmenu(page, "Whole Stack")
    await clickContextMenuItem(page, "Safe Push Whole Stack")
    await expect(popoverStart(page)).toBeFocused()
    await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
    // refused before its first effect: the reason, the restack of the root of what lands, and Start Anyway
    const refused = page.locator("[data-slot=toast]", { hasText: "Cannot start the Safe Push of the whole lane-run stack" })
    await expect(refused.locator("[data-slot=toast-description]")).toContainText(
      "“Show each scenario” of lane-run uses Stills (lane/src/BuildLog.kt:3), which “Name lossy stills” of stills-codec adds: git merges it, but the build of the whole lane-run stack fails. Stack lane-run on stills-codec to land it too.",
    )
    await expect(refused.getByRole("button", { name: "Stack lane-run on stills-codec" })).toBeVisible()
    expect({ pushed: await pushedBranches(repo), started: (await repo.getPatronusLog()).starts }).toEqual({ pushed: [], started: [] })

    // Start Anyway: the same start with the override of what refused it, planned again, which the gate lets through
    await runAndWaitForAnswer(page, "execute", () => refused.getByRole("button", { name: "Start Anyway" }).click())
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
    expect(starts.at(-1)).toMatchObject({ intent: { subject: { kind: "wholeStack", root: "lane-run" }, allow: { allowMissing: true } } })
    await expectPatronusStarts(repo, [wholeStackStartRequest("lane-run")])
    expect(await remotePushes(repo)).toEqual([["stack:lane-run"]])
  })
})
