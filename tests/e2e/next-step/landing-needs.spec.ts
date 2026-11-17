// @spec openspec/specs/branch/next-step/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect, expectErrorLog } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availableRobots, expectPatronusStarts, startRequest, wholeStackStartRequest } from "../helpers/patronus-helpers"
import { expectPushedToRemote, pushedBranches, remotePushes } from "../helpers/push-helpers"
import { addRecentProject, switchRepository } from "../helpers/repository-switch-helpers"
import { openSafePushWithKey, popoverStart, safePushPopover, slotSafePush, startAnyway } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu, openSubmenu } from "../helpers/selection-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { runAndWaitForAnswer, runAndWaitForSync, waitForSettledSession } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { holdCommand, recordCommand } from "../helpers/wait-helpers"

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
 * Safe Push needs before any press, stills-codec needs nothing. `other`: a second repository in the recent projects
 */
async function setupNeeds(
  page: Page,
  setupRepo: (template: string, options: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  other?: string,
): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("landing_needs", { patronus: availableRobots() })
  if (other !== undefined) {
    await addRecentProject(page, other)
  }
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

/**
 * A commit of stills-codec in a terminal, synced: its tip moves on the same baseline, and the session computes the needs
 * of the new tips while the test holds them. The steps keep the needs before meanwhile, the page marks them stale; the
 * stale readiness of stills-codec (its new tip) answered, so its Safe Push is enabled
 */
async function holdNeedsOfNewTip(page: Page, repo: TestRepositoryBuilder): Promise<void> {
  await repo.holdRemoteAnswers({ hold: ["landingNeeds"] })
  await repo.commit("(stills-codec) Name lossless stills", { "bundle/src/Lossless.kt": "package bundle\n\nclass Lossless\n" }, 1704124800)
  await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
  await repo.arrival("landingNeeds")
  await expect(page.getByTestId("sync-button")).toHaveAttribute("data-landing-needs", "stale")
  await expect(slotSafePush(page, "stills-codec")).toHaveText("Safe Push")
  await expect(slotSafePush(page, "stills-codec")).toBeEnabled()
}

/**
 * Start Anyway of lane-run past Stills from its popover goes out once newer needs are in: a commit of lane-run uses the
 * Encoder of a new branch. The refusal it didn't show refuses it, the one it showed stays lifted: the toast asks again
 */
async function refusedByNewerNeeds(page: Page, repo: TestRepositoryBuilder): Promise<Locator> {
  expectErrorLog(page, /^Cannot start the Safe Push of lane-run: “Encode each scenario” of lane-run uses Encoder /)
  await openSafePushWithKey(page, "lane-run")
  await expect(popoverStart(page)).toHaveText("Start Anyway")
  const release = await holdCommand(page, "execute")
  await popoverStart(page).click()
  await repo.commit("(encoder-kit) Add the encoder", { "bundle/src/Encoder.kt": "package bundle\n\nclass Encoder\n" }, 1704124800)
  await repo.commit("(lane-run) Encode each scenario", { "lane/src/Encode.kt": "package lane\n\nimport bundle.Encoder\n\nfun encode(encoder: Encoder) = 1\n" }, 1704126600)
  await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
  await waitForSettledSession(page, repo)
  await runAndWaitForAnswer(page, "execute", async () => release())
  const refused = page.locator("[data-slot=toast]", { hasText: "Cannot start the Safe Push of lane-run" })
  await expect(refused.locator("[data-slot=toast-description]")).toContainText("uses Encoder")
  expect((await repo.getPatronusLog()).starts).toEqual([])
  return refused
}

/**
 * Another repository opens while a press of the start of `branchName` waits for the fresh needs: the press starts
 * nothing and says why at once. The needs come meanwhile; back in the repository, its start is offered again, nothing
 * of it ran in either
 */
async function expectNothingStartedAfterSwitch(page: Page, repo: TestRepositoryBuilder, other: string, branchName: string, executes: readonly unknown[]): Promise<void> {
  await switchRepository(page, other)
  const switched = page.locator("[data-slot=toast]", { hasText: "is no longer the open repository" })
  await expect(switched).toContainText(`Cannot start the Safe Push of ${branchName}`)
  await repo.holdRemoteAnswers({ release: ["landingNeeds"] })
  await switchRepository(page, repo.path)
  await expect(page.getByTestId("sync-button")).not.toHaveAttribute("data-landing-needs")
  await expect(step(page, branchName)).not.toHaveAttribute("data-waits-for")
  expect({ executes: executes.length, started: (await repo.getPatronusLog()).starts }).toEqual({ executes: 0, started: [] })
}

test.describe("Landing needs", () => {
  test("should check what a branch needs to land before it offers its Safe Push, S and the popover waiting for the answer", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("landing_needs", { patronus: availableRobots() })
    await repo.holdRemoteAnswers({ hold: ["landingNeeds"] })
    const rowsAt = Date.now()
    await syncAndWaitForBranches()
    // Rust computes the needs after the view: the page waits for them, the step too
    await repo.arrival("landingNeeds")
    for (const name of ["lane-run", "stills-codec"]) {
      await expect(step(page, name)).toHaveAttribute("data-step", "pending")
    }
    // the lazy tooltip of the row describes nothing before it is reached, and adds no node; its words once it is
    const button = step(page, "lane-run").getByTestId("next-step")
    await expect(button).not.toHaveAttribute("aria-describedby")
    await button.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText("Checking what lane-run needs to land…")
    await expect(button).toHaveAccessibleDescription("Checking what lane-run needs to land…")
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

  test("should keep what a branch needs while the needs of a new tip are computed, its fix waiting for them and running once on the fresh ones", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches)
    const moves = page.getByTestId("landing-moves")
    await expect(moves).toBeVisible()
    const counted = (await moves.textContent()) ?? ""
    // a commit of stills-codec: the needs of the tips before stand, no step waits or changes, the page marks the needs
    // stale (no word says it)
    await holdNeedsOfNewTip(page, repo)
    for (const name of ["lane-run", "ui-perf-nightly"]) {
      await expect(step(page, name)).toHaveAttribute("data-step", "needsBranch")
      await expect(step(page, name)).not.toHaveAttribute("data-waits-for")
      await expect(step(page, name).getByTestId("next-step")).toHaveText("Needs stills-codec")
    }
    await expect(slotSafePush(page, "stills-codec")).toHaveText("Safe Push")
    // the header counts the moves of that answer, which it says is checked again
    const indicator = page.getByTestId("landing-indicator")
    await expect(indicator).toHaveAttribute("aria-busy", "true")
    await expect(indicator).toHaveAttribute("data-landing-needs", "stale")
    await expect(moves).toHaveText(counted)

    // the fix goes to the engine with the refusal it fixes, which waits there for the fresh needs: the row is busy with
    // it, and nothing is rewritten meanwhile
    const executes = await recordCommand(page, "execute")
    const fix = step(page, "lane-run").getByTestId("next-step")
    await fix.click()
    await expect.poll(() => executes.length).toBe(1)
    await expect(fix).toBeDisabled()
    expect(executes[0]).toMatchObject({
      intent: { kind: "rewrite", rewrite: "restack", branchName: "lane-run", parent: "stills-codec", fixes: { kind: "missingDeclaration", identifier: "Stills" } },
    })
    expect(await depths(page)).toEqual(["stills-codec@0", "lane-run@0", "ui-perf-nightly@1"])
    // the fresh needs still name stills-codec: the fix runs on them, once
    await runAndWaitForSync(page, () => repo.holdRemoteAnswers({ release: ["landingNeeds"] }))
    await expectStacked(page)
    expect(executes).toHaveLength(1)
    await expect(page.getByTestId("sync-button")).not.toHaveAttribute("data-landing-needs")
    await expect(indicator).not.toHaveAttribute("aria-busy")
  })

  test("should open the Safe Push popover instead of the fix pressed while the needs of a new tip are computed, once the fresh needs no longer ask for it", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches)
    // a commit of stills-codec: lane-run keeps "Needs stills-codec" while the needs of the new tips are computed
    await holdNeedsOfNewTip(page, repo)
    await expect(step(page, "lane-run").getByTestId("next-step")).toHaveText("Needs stills-codec")

    const executes = await recordCommand(page, "execute")
    await step(page, "lane-run").getByTestId("next-step").click()
    await expect.poll(() => executes.length).toBe(1)
    // meanwhile the commit lane-run needs moves to a branch of its own in a terminal (a later commit that dropped the use
    // would not do: the need is the edge of the commit that uses Stills to the one that adds it); the history is
    // Initial, Name lossy stills, Show each scenario, Measure the UI nightly, Name lossless stills
    await repo.reword(await repo.revision("HEAD~3"), "(stills-core) Name lossy stills")
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    // the fresh needs ask for another fix: the engine refuses the one pressed, which rewrote nothing, and the popover
    // opens with the needs as they are
    await runAndWaitForAnswer(page, "execute", () => repo.holdRemoteAnswers({ release: ["landingNeeds"] }))
    await expect(safePushPopover(page)).toBeVisible()
    await expect(safePushPopover(page).getByTestId("safe-push-landing-needs")).toContainText("Stack lane-run on stills-core")
    await expect(popoverStart(page)).toHaveText("Start Anyway")
    expect(executes).toHaveLength(1)
    expect(await depths(page)).toEqual(["stills-codec@0", "lane-run@0", "ui-perf-nightly@1"])
    await expect(page.locator("[data-slot=toast]", { hasText: /^Stacked lane-run/ })).toHaveCount(0)
  })

  test("should ask again when newer needs refuse a Start Anyway, and start anyway from its toast past the refusals of both", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches)
    const refused = await refusedByNewerNeeds(page, repo)

    const starts = await recordCommand(page, "execute")
    await runAndWaitForAnswer(page, "execute", () => refused.getByRole("button", { name: "Start Anyway" }).click())
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
    expect(starts.at(-1)).toMatchObject({ intent: { allow: { allowMissing: true }, anyway: [{ identifier: "Stills" }, { identifier: "Encoder" }] } })
    await expectPatronusStarts(repo, [startRequest("lane-run")])
  })

  test("should wait in the popover of S for the fresh needs of a new tip, and start on them", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches)
    await holdNeedsOfNewTip(page, repo)
    // S opens the popover, whose start waits for the fresh needs
    await findBranchRow(page, "stills-codec").focus()
    await page.keyboard.press("s")
    await expect(safePushPopover(page).getByTestId("safe-push-status")).toHaveText("Checking what stills-codec needs to land…")
    await expect(popoverStart(page)).toBeDisabled()
    expect((await repo.getPatronusLog()).starts).toEqual([])

    // they came, nothing needed: the start is enabled and starts
    await repo.holdRemoteAnswers({ release: ["landingNeeds"] })
    await expect(popoverStart(page)).toHaveText("Safe Push")
    await runAndWaitForAnswer(page, "execute", () => popoverStart(page).click())
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
    await expectPatronusStarts(repo, [startRequest("stills-codec")])
  })

  test("should wait in a click of the start of the slot for the fresh needs of a new tip, its button checking, and start on them", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches)
    await holdNeedsOfNewTip(page, repo)
    await slotSafePush(page, "stills-codec").click()
    await expect(slotSafePush(page, "stills-codec")).toHaveText("Checking…")
    expect((await repo.getPatronusLog()).starts).toEqual([])
    await runAndWaitForAnswer(page, "execute", () => repo.holdRemoteAnswers({ release: ["landingNeeds"] }))
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
    await expectPatronusStarts(repo, [startRequest("stills-codec")])
  })

  test("should start nothing when another repository opens while a click of the slot waits for the fresh needs of a new tip, and say why", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
    otherRepository,
  }) => {
    expectErrorLog(page, /^Cannot start the Safe Push of stills-codec: .+ is no longer the open repository$/)
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches, otherRepository)
    await holdNeedsOfNewTip(page, repo)
    const executes = await recordCommand(page, "execute")
    await runAndWaitForAnswer(page, "prepare", () => slotSafePush(page, "stills-codec").click())
    await expect(slotSafePush(page, "stills-codec")).toHaveText("Checking…")
    await expectNothingStartedAfterSwitch(page, repo, otherRepository, "stills-codec", executes)
    await expect(slotSafePush(page, "stills-codec")).toHaveText("Safe Push")
  })

  test("should start nothing when another repository opens and the first one shows again while a click of the slot waits for its plan, and say why", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
    otherRepository,
  }) => {
    expectErrorLog(page, /^Cannot start the Safe Push of stills-codec: Another repository opened while the start in .+ waited$/)
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches, otherRepository)
    const executes = await recordCommand(page, "execute")
    // the plan of the click is held: the window shows another repository and the first one again meanwhile
    const asked = page.waitForRequest("**/invoke/prepare")
    const release = await holdCommand(page, "prepare")
    await slotSafePush(page, "stills-codec").click()
    await asked
    await switchRepository(page, otherRepository)
    await switchRepository(page, repo.path)
    await expect(slotSafePush(page, "stills-codec")).toHaveText("Checking…")
    await runAndWaitForAnswer(page, "prepare", async () => release())
    // the click waited on the table of the repository before the switch: it ends, and says why
    await expect(page.locator("[data-slot=toast]", { hasText: "Cannot start the Safe Push of stills-codec" })).toContainText("Another repository opened while the start in")
    await expect(slotSafePush(page, "stills-codec")).toHaveText("Safe Push")
    expect({ executes: executes.length, started: (await repo.getPatronusLog()).starts }).toEqual({ executes: 0, started: [] })
  })

  test("should start nothing when another repository opens while a Start Anyway of a toast waits for the fresh needs of a new tip, and say why", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
    otherRepository,
  }) => {
    expectErrorLog(page, /^Cannot start the Safe Push of lane-run: .+ is no longer the open repository$/)
    const repo = await setupNeeds(page, setupRepo, syncAndWaitForBranches, otherRepository)
    const refused = await refusedByNewerNeeds(page, repo)
    await holdNeedsOfNewTip(page, repo)
    const executes = await recordCommand(page, "execute")
    // Start Anyway of the toast is planned again as a click is: it waits for the fresh needs
    await runAndWaitForAnswer(page, "prepare", () => refused.getByRole("button", { name: "Start Anyway" }).click())
    await expectNothingStartedAfterSwitch(page, repo, otherRepository, "lane-run", executes)
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
    await openSafePushWithKey(page, "lane-run")
    // the start runs with the override, which its plan and the gate of the engine get; nothing else refused it
    await startAnyway(page, "lane-run", { allowDangling: false, allowMissing: true })
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
    expectErrorLog(page, /^Cannot start the Safe Push of the whole lane-run stack: /)
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
