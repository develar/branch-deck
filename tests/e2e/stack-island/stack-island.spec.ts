// @spec spec/branch/stack-island.spec.md
import type { Locator, Page } from "@playwright/test"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchDetailsRow, getStackChip } from "../helpers/branch-helpers"
import { NO_REVIEW_NEEDED, availableRobots, mergeRunningRobot, mergeRunningRobots, robot, routeReviewRequirement } from "../helpers/patronus-helpers"
import { availablePullRequests, pullRequest } from "../helpers/pull-request-helpers"
import { pushOutcome, routeHeldPush, routePush, routePushBranches, wholeStackResultKey } from "../helpers/push-helpers"
import { DEFAULT_CHECKS_ONLY, closeRefusal, expectDisabledItems, expectPopoverAtSlot, openSafePushWithKey, popoverStart, safePushPopover } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { rewriteNextSyncEvents, runAndWaitForSync, syncBranches } from "../helpers/sync-helpers"
import { routeCommand, routeCommandError, waitForStorePersistence } from "../helpers/wait-helpers"

/** The island of the stack on `root` */
function island(page: Page, root: string): Locator {
  return page.locator(`[data-testid="stack-island"][data-stack-root="${root}"]`)
}

const START_OPTIONS = DEFAULT_CHECKS_ONLY

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
 * The starts of the island of air answered in the page: the default checks, no code-owner review, every start refused
 * by Patronus (recorded, no robot holds the next start); the island, the slot of its header and the popover
 */
async function routeIslandStarts(page: Page): Promise<{ starts: Record<string, unknown>[]; air: Locator; headerSlot: Locator; popover: Locator }> {
  await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
  await routeReviewRequirement(page, () => NO_REVIEW_NEEDED)
  const starts = await routeCommand(page, "start_patronus_robot", () => ({ status: "error", error: { message: "Patronus is restarting" } }))
  const air = island(page, "air")
  return { starts, air, headerSlot: air.getByTestId("stack-island-actions").getByTestId("next-step-slot"), popover: safePushPopover(page) }
}

/** The branch rows of a table or an island, in display order */
async function rowNames(scope: Locator): Promise<string[]> {
  return scope.getByTestId("branch-row").evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name") ?? ""))
}

/** The subjects of the commit lines below a branch row of an island */
async function commitLines(page: Page, name: string): Promise<string[]> {
  const lines = page.locator(`[data-testid="branch-commit-lines-row"][data-branch-of="${name}"] [data-testid=stack-commit-line]`)
  return lines.evaluateAll(rows => rows.map(row => row.querySelector("td span.truncate")?.textContent ?? ""))
}

test.describe("Stack island", () => {
  test("should show a big stack as an island below the table, with its commits on the lanes, oldest first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const air = island(page, "air")
    await expect(air).toBeVisible()
    // the table keeps the lone branch and the small stack; the island the seven branches of air
    expect(await rowNames(page.locator("table").first())).toEqual(["docs", "auth", "login"])
    expect(await rowNames(air)).toEqual(["air", "bazel", "side-fix", "side-test", "bridge", "runtime", "deploy"])
    await expect(air.getByTestId("stack-island-size")).toHaveText("Stack · 7 branches · 14 commits")
    // the header sizes the stack: its root row has no chip
    await expect(findBranchRow(page, "air").getByTestId("stack-chip")).toHaveCount(0)
    // every branch shows its commits, a line each, oldest first, the push arrow on a commit the remote lacks
    expect(await commitLines(page, "air")).toEqual(["Add the agent, part 1", "Add the agent, part 2"])
    expect(await commitLines(page, "deploy")).toEqual(["Deploy the runtime, part 1", "Deploy the runtime, part 2"])
    await expect(page.locator("[data-testid=branch-commit-lines-row][data-branch-of=air] [data-testid=commit-push-mark]")).toHaveCount(2)
    // the full list, also oldest first, instead of the lines
    await expandBranch(page, findBranchRow(page, "bazel"))
    await expect(page.locator("[data-testid=branch-commit-lines-row][data-branch-of=bazel]")).toHaveCount(0)
    await expect(getBranchDetailsRow(page, "bazel").locator("tr[data-row-id]").first()).toContainText("Build the deploy jar, part 1")
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
    // the robot of runtime holds the branches it lands: they are not to push
    await expect(states.getByRole("button")).toHaveText(["1 failed", "1 running", "3 to push"])
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
    await expect(page.getByTestId("stack-island-push-description")).toContainText("A Safe Push of runtime is running: it lands air with it")
    await page.mouse.move(0, 0)
    await expect(page.getByTestId("stack-island-push-description")).toBeHidden()
    await expect(air.getByTestId("stack-island-actions").getByTestId("next-step-slot")).toHaveCount(0)
    // side-fix is built on bazel, which runtime lands: its restart would land bazel a second time
    await expect(findBranchRow(page, "side-fix").getByTestId("next-step")).toBeDisabled()

    // collapsed, the island still tells it, and the count shows its branch
    await air.getByTestId("stack-island-root").click()
    await states.getByRole("button", { name: "1 failed" }).click()
    await expect(air).toHaveAttribute("data-state", "open")
    await expect(getBranchDetailsRow(page, "side-fix")).toBeVisible()
  })

  test("should push the whole stack from the header in one push, parents first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const { stackPushes, release } = await routeHeldPush(page, "bazel", 45)
    // the compact push of a row: the commits it sends, the action its name
    const push = island(page, "air").getByTestId("stack-island-push")
    await expect(push).toHaveAccessibleName("Push with Children (7 branches)")
    await push.hover()
    await expect(page.getByTestId("stack-island-push-description")).toContainText("Pushes 7 branches, parents first:")
    await push.click()
    // one push of the 7 branches: the header names it, and every row of it shows the one progress
    await expect(push).toHaveAccessibleName("Pushing 7 branches 45%")
    for (const name of ["air", "bazel", "side-test"]) {
      await expect(findBranchRow(page, name).getByTestId("push-button")).toHaveAccessibleName("Pushing 45%")
    }
    release()
    // each branch after the one it is built on
    await expect.poll(() => stackPushes.map(push => push.map(branch => branch.branchName))).toEqual([["air", "bazel", "side-fix", "bridge", "runtime", "deploy", "side-test"]])
    // the branches are on the remote, the stack branch of the whole stack is not (no pull request, no Safe Push put it
    // there): the header's push stays, disabled with the reason, which says so
    await expect(push).toBeDisabled()
    await expect(push).toHaveAccessibleName("Push with Children")
    await push.hover()
    await expect(page.getByTestId("stack-island-push-description")).toContainText(
      "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack",
    )
  })

  test("should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("stack_island")
    // its pull request or Safe Push put the stack branch on the remote; the sync rebuilt it with 2 commits that remote lacks
    const stackRemote = (unpushedCommits: string[]) => pushOutcome("air", { unpushedCommits, remoteTip: "stack-pushed" }).remoteStatus
    await rewriteNextSyncEvents(page, event => (event.type === "stackBranchUpdate" ? { ...event, data: { ...event.data, remote: stackRemote(["s1", "s2"]) } } : event))
    await syncAndWaitForBranches()
    const { stackPushes } = await routePushBranches(page)
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
    await expect.poll(() => stackPushes.length).toBe(1)
    expect(stackPushes[0]!.map(branch => (branch.wholeStack === true ? `stack:${branch.branchName}` : branch.branchName))).toEqual([
      "air",
      "bazel",
      "side-fix",
      "bridge",
      "runtime",
      "deploy",
      "side-test",
      "stack:air",
    ])
    expect(stackPushes[0]!.at(-1)).toMatchObject({ expectedRemote: "stack-pushed" })
    await expect(push).toHaveAccessibleName("Push with Children")
    await page.mouse.move(0, 0)

    // the branches stay on the remote, the stack branch is rebuilt again: the "⋯" pushes it alone
    await rewriteNextSyncEvents(page, event => {
      if (event.type === "remoteStatusUpdate") {
        return { ...event, data: pushOutcome(event.data.branchName).remoteStatus }
      }
      return event.type === "stackBranchUpdate" ? { ...event, data: { ...event.data, remote: stackRemote(["s3"]) } } : event
    })
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    await expect(push).toHaveAccessibleName("Push with Children (the whole stack branch)")
    await expect(push).toHaveText("1")
    await air.getByRole("button", { name: "More actions for the air stack" }).click()
    await page.getByRole("menuitem", { name: "Push with Children (the whole stack branch)" }).click()
    await expect.poll(() => stackPushes.length).toBe(2)
    expect(stackPushes[1]).toEqual([expect.objectContaining({ branchName: "air", wholeStack: true, expectedRemote: "stack-pushed" })])
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Pushed the whole stack branch of air" })).toBeVisible()
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
    // its pull request or a push put the stack branch on the remote; the sync rebuilt it with 2 commits that remote lacks
    const onRemote = () =>
      rewriteNextSyncEvents(page, event =>
        event.type === "stackBranchUpdate"
          ? { ...event, data: { ...event.data, remote: pushOutcome("air", { unpushedCommits: ["s1", "s2"], remoteTip: "stack-pushed" }).remoteStatus } }
          : event,
      )
    await onRemote()
    await syncAndWaitForBranches()
    const { stackPushes } = await routePushBranches(page)
    const air = island(page, "air")
    const push = air.getByTestId("stack-island-push")
    const description = page.getByTestId("stack-island-push-description")
    await expect(push).toHaveAttribute("aria-disabled", "true")
    await push.hover()
    await expect(description).toContainText("A Safe Push of side-test is running: it lands air with it")
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
    expect(stackPushes).toEqual([])

    // a commit of side-test added since is what is left: it goes with the stack branch, the landed branches stay
    await onRemote()
    await runAndWaitForSync(page, async () => {
      await repo.commit("(side-fix/side-test) Test the manifest, part 3", { "side-test/part3.txt": "Test the manifest 3\n" }, 1704200000)
      await page.getByTestId("sync-button").click()
    })
    await expect(findBranchRow(page, "side-test").getByTestId("landed-with")).toHaveText("Landed · 1 commit after it")
    await expect(push).toHaveAccessibleName("Push with Children (2 branches)")
    await push.click()
    await expect.poll(() => stackPushes.length).toBe(1)
    expect(stackPushes[0]!.map(branch => (branch.wholeStack === true ? `stack:${branch.branchName}` : branch.branchName))).toEqual(["side-test", "stack:air"])
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Pushed side-test" })).toBeVisible()
  })

  test("should land the whole stack in one Safe Push from the header, its popover at the header", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    const { stackPushes, release } = await routeHeldPush(page, wholeStackResultKey("air"), 45)
    const { starts, air, headerSlot, popover } = await routeIslandStarts(page)
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
    await expect(safePush).toHaveText("Pushing 45%")
    await expect(safePush).toHaveAttribute("aria-busy", "true")
    const push = air.getByTestId("stack-island-push")
    await expect(push).toBeDisabled()
    await expect(push).not.toHaveAttribute("aria-busy", "true")
    expect(starts).toEqual([])
    release()
    await expect.poll(() => starts.length).toBe(1)
    expect(starts[0]).toMatchObject({ branchName: "air", stack: true, mode: "rebaseAutosquash" })
    expect(stackPushes).toEqual([[expect.objectContaining({ branchName: "air", wholeStack: true, expectedRemote: null })]])
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
    await popoverStart(page).click()
    await expect.poll(() => starts.length).toBe(2)
    expect(starts[1]).toMatchObject({ branchName: "air", stack: true, mode: "rebaseAutosquash" })
    await expect(popover).toHaveCount(0)
    await closeRefusal(page)

    // S on the focused header: the same popover, Enter starts it
    await air.getByRole("button", { name: /^air Stack · 7 branches/ }).focus()
    await page.keyboard.press("s")
    await expectWholeStackPopoverAt(popover, headerSlot)
    await page.keyboard.press("Enter")
    await expect.poll(() => starts.length).toBe(3)
    expect(starts[2]).toMatchObject({ branchName: "air", stack: true, mode: "rebaseAutosquash" })
    await closeRefusal(page)

    // Dry Run Whole Stack: checks the whole stack, its popover at the header too
    await air.getByRole("button", { name: "More actions for the air stack" }).click()
    await page.getByRole("menuitem", { name: "Dry Run Whole Stack" }).click()
    await expect(popover).toHaveAttribute("aria-label", "Dry Run of the whole air stack")
    await expect(popover.getByTestId("safe-push-lands")).toHaveText("Checks 7 branches · 14 commits on master in one Dry Run: air, bazel, … side-fix and side-test.")
    await expectPopoverAtSlot(popover, headerSlot)
    await popoverStart(page).click()
    await expect.poll(() => starts.length).toBe(4)
    expect(starts[3]).toMatchObject({ branchName: "air", stack: true, mode: "dryRun" })
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
    await setupRepo("stack_side_conflict", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})
    await routeCommandError(page, "get_stack_independence", "The trial build failed")
    const { starts, air, headerSlot, popover } = await routeIslandStarts(page)
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
    await popoverStart(page).click()
    await expect.poll(() => starts.length).toBe(1)
    expect(starts[0]).toMatchObject({ branchName: "deploy", mode: "rebaseAutosquash" })
    expect(starts[0]!.stack).toBeUndefined()
    expect(pushes.map(push => push.branchName)).toEqual(["air", "bazel", "bridge", "runtime", "deploy"])
  })

  test("should push the stack up to a commit, the branches it is built on first, the rest left to push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const { stackPushes } = await routePushBranches(page)
    const line = page.locator("[data-testid=branch-commit-lines-row][data-branch-of=bazel] [data-testid=stack-commit-line]").first()
    await expect(line).toContainText("Build the deploy jar, part 1")
    await line.getByTestId("commit-push-mark").click()
    const popover = page.getByTestId("push-up-to")
    await expect(popover.getByTestId("push-up-to-plan")).toHaveText(
      "Pushes 2 branches, parents first:\nair · 2 commits · creates the remote branch\nbazel · 1 of 2 commits · creates the remote branch",
    )
    // its button marks what it sends: the commits up to here, the nodes of the branches
    await popover.getByTestId("push-up-to-button").hover()
    await expect(page.locator("[data-testid=stack-commit-line][data-sent]")).toHaveCount(3)
    await expect(findBranchRow(page, "bazel").getByTestId("lane-node")).toHaveAttribute("data-pushed", "true")
    await expect(findBranchRow(page, "bridge").getByTestId("lane-node")).not.toHaveAttribute("data-pushed", "true")

    await popover.getByTestId("push-up-to-button").click()
    // one push: air goes whole, bazel up to its first commit
    await expect.poll(() => stackPushes.map(push => push.map(branch => branch.branchName))).toEqual([["air", "bazel"]])
    const [air, bazel] = stackPushes[0]!
    expect(air!.tip ?? null).toBeNull()
    expect(bazel!.tip).toMatch(/^[0-9a-f]{40}$/)
  })

  test("should show no push arrow on the commits a merged Safe Push landed, only on a commit added after them", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the Safe Push of bazel lands air with it
    const running = robot("bazel", "bazel-robot")
    const repo = await setupRepo("stack_island", { patronus: availableRobots(running) })
    await syncAndWaitForBranches()
    const marks = (name: string) => page.locator(`[data-testid=branch-commit-lines-row][data-branch-of="${name}"] [data-testid=commit-push-mark]`)
    await expect(marks("air")).toHaveCount(2)
    await expect(marks("bazel")).toHaveCount(2)

    // merged: its commits and air's are in master, their lines muted and without the arrow
    await mergeRunningRobot(page, repo, running)
    await expect(findBranchRow(page, "air").getByTestId("landed-with")).toHaveText("Landed with bazel")
    for (const name of ["air", "bazel"]) {
      await expect(marks(name)).toHaveCount(0)
      await expect(page.locator(`[data-testid=branch-commit-lines-row][data-branch-of="${name}"] [data-testid=stack-commit-line][data-landed]`)).toHaveCount(2)
    }
    // bridge, built on bazel, still has its own to push
    await expect(marks("bridge")).toHaveCount(2)

    // a new commit of air: the arrow is on it alone, the only work left of air
    await runAndWaitForSync(page, async () => {
      await repo.commit("(air) Add the agent, part 3", { "air/part3.txt": "Add the agent 3\n" }, 1704200000)
      await page.getByTestId("sync-button").click()
    })
    await expect(findBranchRow(page, "air").getByTestId("landed-with")).toHaveText("Landed with bazel · 1 commit after it")
    await expect(marks("air")).toHaveCount(1)
    expect(await commitLines(page, "air")).toEqual(["Add the agent, part 1", "Add the agent, part 2", "Add the agent, part 3"])
    await expect(page.locator("[data-testid=branch-commit-lines-row][data-branch-of=air] [data-testid=stack-commit-line]").last()).toHaveAttribute("data-unpushed", "true")
    await expect(marks("bazel")).toHaveCount(0)
  })

  test("should push up to a commit from its menu in the full list", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})
    await expandBranch(page, findBranchRow(page, "bazel"))
    const first = getBranchDetailsRow(page, "bazel").locator("tr[data-row-id]").first()
    await expect(first).toContainText("Build the deploy jar, part 1")
    await openContextMenu(page, first)
    await clickContextMenuItem(page, "Push Up to Here")
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["air", "bazel"])
  })

  test("should offer Push Up to Here in the full list only for a commit added after the ones a merged Safe Push landed, without the landed branches", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the Safe Push of bazel lands air with it
    const running = robot("bazel", "bazel-robot")
    const repo = await setupRepo("stack_island", { patronus: availableRobots(running) })
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})
    await expandBranch(page, findBranchRow(page, "bazel"))
    const commitRows = getBranchDetailsRow(page, "bazel").locator("tr[data-row-id]")
    const first = commitRows.first()
    await expect(first).toContainText("Build the deploy jar, part 1")
    // before the merge its commit offers the push up to it
    await openContextMenu(page, first)
    await expect(page.getByRole("menuitem", { name: "Push Up to Here" })).toBeVisible()
    await page.keyboard.press("Escape")

    // merged: air's and bazel's commits are in master, then a new commit of bazel is the only work left of it
    await mergeRunningRobot(page, repo, running)
    await runAndWaitForSync(page, async () => {
      await repo.commit("(air/bazel) Build the deploy jar, part 3", { "bazel/part3.txt": "Build the deploy jar 3\n" }, 1704200000)
      await page.getByTestId("sync-button").click()
    })
    await expect(findBranchRow(page, "bazel").getByTestId("landed-with")).toHaveText("Landed · 1 commit after it")
    await expect(commitRows).toHaveCount(3)
    // a landed commit offers no push, as the island shows no arrow for it: its menu is the same menu without the item
    await openContextMenu(page, first)
    await expect(page.getByRole("menuitem", { name: "Copy Commit Hash" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Push Up to Here" })).toHaveCount(0)
    await page.keyboard.press("Escape")
    // the new commit pushes bazel, without air: all of air landed
    await openContextMenu(page, commitRows.last())
    await clickContextMenuItem(page, "Push Up to Here")
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["bazel"])
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
