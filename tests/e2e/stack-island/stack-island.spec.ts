// @spec spec/branch/stack-island.spec.md
import type { Locator, Page } from "@playwright/test"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCells, getBranchCommitRows, getBranchDetailsRow, getStackChip, verifyBranchState } from "../helpers/branch-helpers"
import { pressCopyKey, readClipboard } from "../helpers/clipboard-helpers"
import { columnLefts, expectColumnAligned } from "../helpers/column-helpers"
import { availableRobots, expectPatronusStarts, mergeRunningRobots, robot, startRequest, wholeStackStartRequest } from "../helpers/patronus-helpers"
import { availablePullRequests, pullRequest } from "../helpers/pull-request-helpers"
import { branchOf, expectPushedToRemote, holdRemotePushes, pushedBefore, remotePushes, stackBranchOf } from "../helpers/push-helpers"
import { closeRefusal, expectDisabledItems, expectPopoverAtSlot, openSafePushWithKey, popoverStart, safePushPopover } from "../helpers/safe-push-helpers"
import { openContextMenu } from "../helpers/selection-helpers"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { runAndWaitForAnswer, runAndWaitForSync, syncBranches } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { routeCommandError, waitForStorePersistence } from "../helpers/wait-helpers"

/** The island of the stack on `root` */
function island(page: Page, root: string): Locator {
  return page.locator(`[data-testid="stack-island"][data-stack-root="${root}"]`)
}

/**
 * The popover of the Safe Push of the whole air stack at `slot` (above or below it), the only one: Enter starts it (its
 * start enabled, the focus in it)
 */
async function expectWholeStackPopoverAt(popover: Locator, slot: Locator): Promise<void> {
  await expect(popover).toHaveCount(1)
  await expect(popover).toHaveAttribute("aria-label", "Safe Push of the whole air stack")
  await expect(popover.getByTestId("safe-push-start")).toBeEnabled()
  await expect.poll(() => popover.page().evaluate(() => document.activeElement?.closest("[data-testid=safe-push-popover]") != null)).toBe(true)
  await expectPopoverAtSlot(popover, slot)
}

/**
 * The starts of the island of air: the default checks, no code-owner review, every start refused by the fake Patronus
 * (logged, no robot holds the next start); the island, the slot of its header and the popover
 */
async function refuseIslandStarts(page: Page, repo: TestRepositoryBuilder): Promise<{ air: Locator; headerSlot: Locator; popover: Locator }> {
  await repo.refusePatronusStarts("Patronus is restarting")
  const air = island(page, "air")
  return { air, headerSlot: air.getByTestId("stack-island-actions").getByTestId("next-step-slot"), popover: safePushPopover(page) }
}

/** The branch rows of a table or an island, in display order */
async function rowNames(scope: Locator): Promise<string[]> {
  return scope.getByTestId("branch-row").evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name") ?? ""))
}

test.describe("Stack island", () => {
  test("should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const air = island(page, "air")
    await expect(air).toBeVisible()
    // the table keeps the lone branch and the small stack; the island the seven branches of air
    expect(await rowNames(page.locator("table").first())).toEqual(["docs", "auth", "login"])
    const names = await rowNames(air)
    expect(names).toEqual(["air", "bazel", "side-fix", "side-test", "bridge", "runtime", "deploy"])
    await expect(air.getByTestId("stack-island-size")).toHaveText("Stack · 7 branches · 14 commits")
    // the header sizes the stack: its root row has no chip
    await expect(findBranchRow(page, "air").getByTestId("stack-chip")).toHaveCount(0)
    // its rows are the rows of the table: collapsed, none shows a commit below it
    for (const name of names) {
      await verifyBranchState(findBranchRow(page, name), "closed")
    }

    // a click on the row shows its commits on its lane, oldest first, the push arrow on each the remote lacks
    const row = findBranchRow(page, "air")
    const commits = getBranchCommitRows(page, "air")
    await getBranchCells(row).commits.click()
    await verifyBranchState(row, "open")
    await expect(commits).toHaveCount(2)
    await expect(commits).toContainText(["Add the agent, part 1", "Add the agent, part 2"])
    await expect(commits.getByTestId("commit-push-mark")).toHaveCount(2)
    // the next click hides them; the folder button, → and ← and Enter on the focused row show and hide them alike
    await getBranchCells(row).commits.click()
    await verifyBranchState(row, "closed")
    await row.getByRole("button", { name: "Expand air" }).click()
    await expect(commits).toHaveCount(2)
    await row.getByRole("button", { name: "Collapse air" }).click()
    await verifyBranchState(row, "closed")
    await row.focus()
    await page.keyboard.press("ArrowRight")
    await expect(commits).toHaveCount(2)
    await page.keyboard.press("ArrowLeft")
    await verifyBranchState(row, "closed")
    await page.keyboard.press("Enter")
    await expect(commits).toHaveCount(2)
    await page.keyboard.press("Enter")
    await verifyBranchState(row, "closed")

    // the last branch of the main line: its commits oldest first too, its lane ending at the last of them
    await expandBranch(page, findBranchRow(page, "deploy"))
    const deploy = getBranchCommitRows(page, "deploy")
    await expect(deploy).toHaveCount(2)
    await expect(deploy).toContainText(["Deploy the runtime, part 1", "Deploy the runtime, part 2"])
    await expect(getBranchDetailsRow(page, "deploy").getByTestId("commit-rail").locator("xpath=..")).toHaveAttribute("data-lane-end", "")
  })

  test("should line up the columns of the table, the island and its header, whatever each card shows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the Safe Push of deploy lands the main line: those rows say "lands with deploy" after their status, the rows of the
    // table say nothing there, and the header of the island has no step while it runs
    await setupRepo("stack_island", { patronus: availableRobots(robot("deploy", "deploy-robot")) })
    await syncAndWaitForBranches()
    const air = island(page, "air")
    await expect(findBranchRow(page, "air").getByTestId("landing-span")).toHaveText("lands with deploy")
    await expect(findBranchRow(page, "docs").getByTestId("landing-span")).toHaveCount(0)
    await expect(air.getByTestId("stack-island-actions").getByTestId("next-step-slot")).toHaveCount(0)
    const rowTrack = (track: string) => page.getByTestId("branch-row").locator(`[data-track=${track}]`)
    // the rows of the table and of the island
    for (const track of ["commits", "state", "landed", "step", "more"]) {
      await expectColumnAligned(rowTrack(track))
    }
    // the header's actions stand in the tracks of the rows' actions: its "⋯" over theirs, though its slot is empty
    await expectColumnAligned(page.locator("[data-track=more]"))
    expect(await columnLefts(air.getByTestId("stack-island-actions").locator("[data-track=more]"))).toEqual(await columnLefts(rowTrack("more")))

    // ⌘C on a row of the island copies its full name, as in the table
    await findBranchRow(page, "bazel").focus()
    await pressCopyKey(page)
    await expect.poll(() => readClipboard(page)).toBe("user-name/virtual/bazel")
  })

  test("should collapse the island into its header, per repository", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const air = island(page, "air")
    await air.getByTestId("stack-island-root").click()
    await expect(air).toHaveAttribute("data-state", "closed")
    await expect(air.getByTestId("branch-row")).toHaveCount(0)
    // the header still says what the stack holds
    await expect(air.getByTestId("stack-island-size")).toHaveText("Stack · 7 branches · 14 commits")

    // the repository remembers the collapsed root
    await waitForStorePersistence(page, "recentProjects", projects => (projects as { collapsedStacks?: string[] }[] | undefined)?.[0]?.collapsedStacks?.includes("air") === true)
    await page.reload()
    await syncBranches(page, ['[data-testid="branch-row"][data-branch-name="docs"]'])
    await expect(island(page, "air")).toHaveAttribute("data-state", "closed")
    await island(page, "air").getByTestId("stack-island-root").click()
    await expect(findBranchRow(page, "deploy")).toBeVisible()
  })

  test("should step from the table into the island with the arrows, and collapse it with Alt+Left", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    await findBranchRow(page, "login").focus()
    await page.keyboard.press("ArrowDown")
    await expect(findBranchRow(page, "air")).toBeFocused()
    await page.keyboard.press("ArrowDown")
    await expect(findBranchRow(page, "bazel")).toBeFocused()
    await page.keyboard.press("ArrowUp")
    await page.keyboard.press("ArrowUp")
    await expect(findBranchRow(page, "login")).toBeFocused()

    // from any row of the island: its header takes the focus, as its rows hide
    await findBranchRow(page, "runtime").focus()
    await page.keyboard.press("Alt+ArrowLeft")
    await expect(island(page, "air")).toHaveAttribute("data-state", "closed")
    await expect(island(page, "air").locator("[data-slot=collapsible-trigger]")).toBeFocused()
  })

  test("should say what the stack holds in the header, each state a way to its first branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island", {
      patronus: availableRobots(
        robot("runtime", "running-robot", { startTime: 1704189600 - 18 * 60 }),
        robot("side-fix", "failed-robot", { status: "failure", finishTime: 1704142800 }),
      ),
      pullRequests: availablePullRequests(pullRequest("air", 812, { reviewDecision: "approved" }), pullRequest("bazel", 813, { reviewDecision: "changesRequested" })),
    })
    await syncAndWaitForBranches()
    const air = island(page, "air")
    const states = air.getByTestId("stack-island-states")
    // where the branches are on their way into master, as their nodes say it: the four branches the robot of runtime
    // lands, the two it holds and side-fix waiting: its robot failed, but runtime lands bazel, a part of it, so the
    // failure waits for that robot (its restart would land bazel a second time)
    await expect(states.getByRole("button")).toHaveText(["4 landing", "3 waiting"])
    await expect(states.getByRole("button", { name: "3 waiting" })).not.toHaveClass(/text-destructive/)
    await expect(air.getByTestId("stack-island-pull-requests")).toHaveText("2 pull requests open · 1 approved · 1 changes requested")
    // the robot of runtime lands the branches it is built on
    await expect(air.getByTestId("stack-island-landing")).toContainText("Safe Push of runtime lands 4 branches")
    // one ring for the one running robot, on its chip: the landing line says what it lands
    await expect(states.getByTestId("robot-ring")).toHaveCount(1)
    await expect(air.getByTestId("stack-island-landing").getByTestId("robot-ring")).toHaveCount(0)
    // a Safe Push runs in the stack: the header's push is disabled with the reason, no Safe Push, as a row of a running
    // branch
    const push = air.getByTestId("stack-island-push")
    await expect(push).toBeDisabled()
    await expect(push).toHaveAccessibleName("Push with Children")
    await push.hover()
    await expect(page.getByTestId("stack-island-push-description")).toContainText("A Safe Push of runtime is running: it lands 2 commits of air with it")
    await page.mouse.move(0, 0)
    await expect(page.getByTestId("stack-island-push-description")).toBeHidden()
    await expect(air.getByTestId("stack-island-actions").getByTestId("next-step-slot")).toHaveCount(0)
    // side-fix is built on bazel, which runtime lands: no restart, which would land bazel a second time; its Safe Push
    // waits for that robot
    await expect(findBranchRow(page, "side-fix").getByRole("button", { name: "Safe Push · 3" })).toBeDisabled()

    // collapsed, the island still tells it, and the count shows its branch
    await air.getByTestId("stack-island-root").click()
    await states.getByRole("button", { name: "3 waiting" }).click()
    await expect(air).toHaveAttribute("data-state", "open")
    await expect(getBranchDetailsRow(page, "side-fix")).toBeVisible()
  })

  test("should push the whole stack from the header in one push, parents first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const held = await holdRemotePushes(repo)
    // the compact push of a row: the commits it sends, the action its name
    const push = island(page, "air").getByTestId("stack-island-push")
    await expect(push).toHaveAccessibleName("Push with Children (7 branches)")
    await push.hover()
    const description = page.getByTestId("stack-island-push-description")
    await expect(description).toContainText("Pushes 7 branches, parents first:")
    // read, then pushed: the click, later than half a second after the hover opened, leaves it open with the progress
    await page.clock.fastForward(1000)
    await push.click()
    // one push of the 7 branches, waiting at the remote: the header names it, and every row of it shows the one progress
    await held.arrived()
    await expect(push).toHaveAccessibleName(/^Pushing 7 branches \d+%$/)
    await expect(description).toContainText(/Writing objects: +\d+% \(\d+\/\d+\)/)
    await expect(description).toHaveAttribute("data-open", "")
    for (const name of ["air", "bazel", "side-test"]) {
      await expect(findBranchRow(page, name).getByTestId("push-button")).toHaveAccessibleName(/^Pushing \d+%$/)
    }
    await held.release()
    await expect.poll(() => remotePushes(repo)).toEqual([["air", "bazel", "bridge", "deploy", "runtime", "side-fix", "side-test"]])
    // the branches are on the remote, the stack branch of the whole stack is not (no pull request, no Safe Push put it
    // there): the header's push stays, disabled with the reason, which says so
    await expect(push).toBeDisabled()
    await expect(push).toHaveAccessibleName("Push with Children")
    await push.hover()
    await expect(description).toContainText("The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack")
  })

  test("should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("stack_island")
    await syncAndWaitForBranches()
    // its pull request or Safe Push put the stack branch on the remote; the sync rebuilt it with 2 commits that remote lacks
    await pushedBefore(page, repo, [stackBranchOf("air")], { lacking: 2 })
    const air = island(page, "air")
    const push = air.getByTestId("stack-island-push")

    // the header's push: the 7 branches and the stack branch, one more branch, named in the hover; its commits counted once
    await expect(push).toHaveAccessibleName("Push with Children (8 branches)")
    await expect(push).toHaveText("14")
    await push.hover()
    const description = page.getByTestId("stack-island-push-description")
    await expect(description).toContainText("Pushes 8 branches, parents first:")
    await expect(description).toContainText("user-name/stack/air · the whole stack branch · 2 commits")
    await push.click()
    // one push of the 8, the stack branch over what its remote had
    await expect.poll(() => remotePushes(repo)).toEqual([["air", "bazel", "bridge", "deploy", "runtime", "side-fix", "side-test", "stack:air"]])
    await expectPushedToRemote(repo, [stackBranchOf("air")])
    await expect(push).toHaveAccessibleName("Push with Children")
    await page.mouse.move(0, 0)

    // the branches stay on the remote, the stack branch lacks its newest commit there again: the "⋯" pushes it alone
    await repo.pushInTerminal(branchOf(stackBranchOf("air")), { revision: `${branchOf(stackBranchOf("air"))}~1` })
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    await expect(push).toHaveAccessibleName("Push with Children (the whole stack branch)")
    await expect(push).toHaveText("1")
    await air.getByRole("button", { name: "More actions for the air stack" }).click()
    await page.getByRole("menuitem", { name: "Push with Children (the whole stack branch)" }).click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Pushed the whole stack branch of air" })).toBeVisible()
    expect((await remotePushes(repo)).at(-1)).toEqual(["stack:air"])
    await expectPushedToRemote(repo, [stackBranchOf("air")])
  })

  test("should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the Safe Pushes of deploy and side-test run: between them they land every branch of the stack
    const deploy = robot("deploy", "deploy-robot")
    const sideTest = robot("side-test", "side-test-robot")
    const repo = await setupRepo("stack_island", { patronus: availableRobots(deploy, sideTest), pullRequests: availablePullRequests() })
    await syncAndWaitForBranches()
    // its pull request or a push put the stack branch on the remote; the sync rebuilt it with 2 commits that remote lacks
    await pushedBefore(page, repo, [stackBranchOf("air")], { lacking: 2 })
    const air = island(page, "air")
    const push = air.getByTestId("stack-island-push")
    const description = page.getByTestId("stack-island-push-description")
    await expect(push).toHaveAttribute("aria-disabled", "true")
    await push.hover()
    await expect(description).toContainText("A Safe Push of side-test is running: it lands 2 commits of air with it")
    await page.mouse.move(0, 0)

    // both merged: nothing of the stack is left to push, its stack branch neither, though its remote lacks commits of it
    await mergeRunningRobots(page, repo, [deploy, sideTest])
    await expect(findBranchRow(page, "air").getByTestId("landed-with")).toHaveText("Landed with deploy")
    await expect(findBranchRow(page, "side-fix").getByTestId("landed-with")).toHaveText("Landed with side-test")
    const reason = "Landed in master with deploy and side-test"
    const nothingLeft = `${reason}: nothing of air and its children is left to push`
    await expect(push).toHaveAccessibleName("Push with Children")
    await expect(push).toHaveAttribute("aria-disabled", "true")
    await push.hover()
    await expect(description).toContainText(nothingLeft)
    await page.mouse.move(0, 0)
    // the "⋯" of the island: Push with Children and the pull request of the whole stack, each saying why
    await air.getByRole("button", { name: "More actions for the air stack" }).click()
    await expectDisabledItems(page, [
      ["Push with Children", nothingLeft],
      ["Create Pull Request of the Stack…", reason],
    ])
    await page.keyboard.press("Escape")
    // the root's menu says it too
    await openContextMenu(page, findBranchRow(page, "air"))
    await expectDisabledItems(page, [["Push with Children", nothingLeft]])
    await page.keyboard.press("Escape")
    expect(await remotePushes(repo)).toEqual([])

    // a commit of side-test added since is what is left: it goes with the stack branch, the landed branches stay
    await runAndWaitForSync(page, async () => {
      await repo.commit("(side-fix/side-test) Test the manifest, part 3", { "side-test/part3.txt": "Test the manifest 3\n" }, 1704200000)
      await page.getByTestId("sync-button").click()
    })
    await expect(findBranchRow(page, "side-test").getByTestId("landed-with")).toHaveText("Landed · 1 commit after it")
    await expect(push).toHaveAccessibleName("Push with Children (2 branches)")
    await push.click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Pushed side-test" })).toBeVisible()
    expect(await remotePushes(repo)).toEqual([["side-test", "stack:air"]])
  })

  test("should land the whole stack in one Safe Push from the header, its popover at the header", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stack_island", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    const held = await holdRemotePushes(repo)
    const { air, headerSlot, popover } = await refuseIslandStarts(page, repo)
    const safePushOfAir = wholeStackStartRequest("air")
    const lands = "Lands 7 branches · 14 commits into master in one Safe Push: air, bazel, … side-fix and side-test"

    // collapsed: the Safe Push of the whole stack, its stack branch, opens its popover at the header; the island stays
    // collapsed
    await air.getByTestId("stack-island-root").click()
    await expect(air).toHaveAttribute("data-state", "closed")
    const safePush = headerSlot.getByTestId("safe-push-button")
    await expect(safePush).toHaveText("Safe Push · 7")
    await safePush.hover()
    await expect(page.getByRole("tooltip")).toHaveText(lands)
    // it lands more than a row: the click opens the popover
    await safePush.click()
    await expectWholeStackPopoverAt(popover, headerSlot)
    await expect(popover.getByTestId("safe-push-lands")).toHaveText(`${lands}.`)
    // no branch of the stack is on the remote: none to stay as it is there
    await expect(popover.getByTestId("safe-push-stack-branch")).toHaveText("Pushes only its stack branch user-name/stack/air.")
    await expect(air).toHaveAttribute("data-state", "closed")
    // Enter: the stack branch alone is pushed, its progress in the button of the start, the one indicator: the header's
    // push waits without it; then the robot starts from it
    await page.keyboard.press("Enter")
    // as the compact push of a row: the spinner and the percent, the push in its name
    await held.arrived()
    await expect(safePush).toHaveText(/^\d+%$/)
    await expect(safePush).toHaveAccessibleName(/^Pushing \d+%$/)
    await expect(safePush).toHaveAttribute("aria-busy", "true")
    const push = air.getByTestId("stack-island-push")
    await expect(push).toBeDisabled()
    await expect(push).not.toHaveAttribute("aria-busy", "true")
    expect((await repo.getPatronusLog()).starts).toEqual([])
    await runAndWaitForAnswer(page, "start_patronus_robot", () => held.release())
    await expectPatronusStarts(repo, [safePushOfAir])
    expect(await remotePushes(repo)).toEqual([["stack:air"]])
    await expect(popover).toHaveCount(0)
    await closeRefusal(page)

    // expanded: "⋯" has the Safe Push and the Dry Run of the whole stack, and opens the same popover at the header
    await air.getByTestId("stack-island-root").click()
    await expect(air).toHaveAttribute("data-state", "open")
    await air.getByRole("button", { name: "More actions for the air stack" }).click()
    await expect(page.getByRole("menu", { name: "More actions for the air stack" }).getByRole("menuitem")).toHaveText([
      "Safe Push Whole Stack",
      "Dry Run Whole Stack",
      "Push with Children (7 branches)",
      "Review Whole Stack",
      "Collapse Stack",
    ])
    await page.getByRole("menuitem", { name: "Safe Push Whole Stack" }).click()
    await expectWholeStackPopoverAt(popover, headerSlot)
    await runAndWaitForAnswer(page, "start_patronus_robot", () => popoverStart(page).click())
    await expectPatronusStarts(repo, [safePushOfAir, safePushOfAir])
    await expect(popover).toHaveCount(0)
    await closeRefusal(page)

    // S on the focused header: the same popover, Enter starts it
    await air.getByRole("button", { name: /^air Stack · 7 branches/ }).focus()
    await page.keyboard.press("s")
    await expectWholeStackPopoverAt(popover, headerSlot)
    await runAndWaitForAnswer(page, "start_patronus_robot", () => page.keyboard.press("Enter"))
    await expectPatronusStarts(repo, [safePushOfAir, safePushOfAir, safePushOfAir])
    await closeRefusal(page)

    // Dry Run Whole Stack: checks the whole stack, its popover at the header too
    await air.getByRole("button", { name: "More actions for the air stack" }).click()
    await page.getByRole("menuitem", { name: "Dry Run Whole Stack" }).click()
    await expect(popover).toHaveAttribute("aria-label", "Dry Run of the whole air stack")
    await expect(popover.getByTestId("safe-push-lands")).toHaveText("Checks 7 branches · 14 commits on master in one Dry Run: air, bazel, … side-fix and side-test.")
    await expectPopoverAtSlot(popover, headerSlot)
    await runAndWaitForAnswer(page, "start_patronus_robot", () => popoverStart(page).click())
    await expectPatronusStarts(repo, [safePushOfAir, safePushOfAir, safePushOfAir, wholeStackStartRequest("air", { mode: "DRY_RUN" })])
    await expect(page.locator("[data-slot=toast]", { hasText: "Cannot start the Dry Run" })).toBeVisible()

    // the row of deploy keeps its own popover at its slot: the Safe Push of deploy and what it is built on
    await openSafePushWithKey(page, "deploy")
    await expect(popover).toHaveAttribute("aria-label", "Safe Push of deploy")
    await expectPopoverAtSlot(popover, findBranchRow(page, "deploy").getByTestId("next-step-slot"))
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)

    // Review Whole Stack: the review window of its stack branch, every branch of the stack
    await air.getByRole("button", { name: "More actions for the air stack" }).click()
    await page.getByRole("menuitem", { name: "Review Whole Stack" }).click()
    await expect.poll(async () => (await getOpenedSubWindows<BranchReviewWindowData>(page)).length).toBe(1)
    const [opened] = await getOpenedSubWindows<BranchReviewWindowData>(page)
    expect(opened).toMatchObject({ windowId: "branch-review", title: "Review the whole air stack", data: { chain: ["air"], wholeStack: true } })
  })

  test("should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("stack_side_conflict", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    await routeCommandError(page, "get_stack_independence", { code: "internal", message: "The trial build failed" })
    const { air, headerSlot, popover } = await refuseIslandStarts(page, repo)
    const conflict = /^The stack branch conflicts at [0-9a-f]{8} in config\.js: the stack lands in pieces$/

    // the button of the whole stack is there, disabled with the conflict of its stack branch
    const safePush = headerSlot.getByTestId("safe-push-button")
    await expect(safePush).toHaveText("Safe Push · 6")
    await expect(safePush).toHaveAttribute("aria-disabled", "true")
    await safePush.hover()
    await expect(page.getByRole("tooltip")).toHaveText(conflict)

    // the menu says the same of both starts, and offers the piece of the main line
    await air.getByRole("button", { name: "More actions for the air stack" }).click()
    const menu = page.getByRole("menu", { name: "More actions for the air stack" })
    await expect(menu.getByRole("menuitem", { name: "Safe Push Whole Stack" })).toHaveAttribute("aria-disabled", "true")
    await expect(menu.getByRole("menuitem", { name: "Dry Run Whole Stack" })).toHaveAttribute("aria-disabled", "true")
    // the button's description, which says the same, fades out first (and so does the one of "⋯")
    const conflictTooltip = page.getByRole("tooltip", { name: conflict })
    await expect(conflictTooltip).toHaveCount(0)
    await menu.getByRole("menuitem", { name: "Safe Push Whole Stack" }).hover()
    await expect(conflictTooltip).toHaveText(conflict)
    await menu.getByRole("menuitem", { name: "Safe Push deploy" }).click()
    await expect(popover).toHaveAttribute("aria-label", "Safe Push of deploy")
    await expect(popover.getByTestId("safe-push-lands")).toHaveText("Lands deploy with air, bazel, bridge and runtime, which it is built on: 5 branches · 5 commits into master.")
    await expectPopoverAtSlot(popover, headerSlot)
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)

    // S on the header says why it can't start, and lands it in pieces from the notice: the main line first
    await air.getByRole("button", { name: /^air Stack · 6 branches/ }).focus()
    await page.keyboard.press("s")
    const notice = page.locator("[data-slot=toast]").filter({ hasText: "Safe Push of the whole air stack can't start" })
    await expect(notice).toContainText("the stack lands in pieces")
    await notice.getByRole("button", { name: "Safe Push deploy" }).click()
    await expect(popover).toHaveAttribute("aria-label", "Safe Push of deploy")
    await expectPopoverAtSlot(popover, headerSlot)
    await runAndWaitForAnswer(page, "start_patronus_robot", () => popoverStart(page).click())
    // the piece: the virtual branch of deploy, not the stack branch
    await expectPatronusStarts(repo, [startRequest("deploy")])
    expect(await remotePushes(repo)).toEqual([["air", "bazel", "bridge", "deploy", "runtime"]])
  })

  test("should collapse a side stack into its root, from its chip and with Alt+Left before the whole stack", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const air = island(page, "air")
    // side-fix forks off bazel and side-test is built on it: its chip sizes the side stack
    const chip = getStackChip(findBranchRow(page, "side-fix"))
    await expect(chip).toContainText("2 branches")
    await chip.click()
    await expect(chip).toHaveAttribute("data-stack", "collapsed")
    expect(await rowNames(air)).toEqual(["air", "bazel", "side-fix", "bridge", "runtime", "deploy"])
    await chip.click()
    expect(await rowNames(air)).toEqual(["air", "bazel", "side-fix", "side-test", "bridge", "runtime", "deploy"])

    // as ← in a tree: the side stack first, its root takes the focus; then the whole stack
    await findBranchRow(page, "side-test").focus()
    await page.keyboard.press("Alt+ArrowLeft")
    await expect(findBranchRow(page, "side-fix")).toBeFocused()
    expect(await rowNames(air)).toEqual(["air", "bazel", "side-fix", "bridge", "runtime", "deploy"])
    await page.keyboard.press("Alt+ArrowRight")
    await expect.poll(() => rowNames(air)).toContain("side-test")
    await page.keyboard.press("Alt+ArrowLeft")
    await page.keyboard.press("Alt+ArrowLeft")
    await expect(air).toHaveAttribute("data-state", "closed")
  })
})
