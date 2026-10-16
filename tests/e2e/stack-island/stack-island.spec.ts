// @spec spec/branch/stack-island.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchDetailsRow, getStackChip } from "../helpers/branch-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { availablePullRequests, pullRequest } from "../helpers/pull-request-helpers"
import { routePush } from "../helpers/push-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { syncBranches } from "../helpers/sync-helpers"
import { waitForStorePersistence } from "../helpers/wait-helpers"

/** The island of the stack on `root` */
function island(page: Page, root: string): Locator {
  return page.locator(`[data-testid="stack-island"][data-stack-root="${root}"]`)
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

    await waitForStorePersistence(page)
    await page.reload()
    await syncBranches(page, ["[data-testid=\"branch-row\"][data-branch-name=\"docs\"]"])
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
    await expect(states.getByRole("button")).toHaveText(["1 failed", "1 running", "7 to push"])
    await expect(air.getByTestId("stack-island-pull-requests")).toHaveText("2 pull requests open · 1 approved · 1 changes requested")
    // the robot of runtime lands the branches it is built on
    await expect(air.getByTestId("stack-island-landing")).toContainText("Safe Push of runtime lands 4 branches")

    // collapsed, the island still tells it, and the count shows its branch
    await air.getByTestId("stack-island-root").click()
    await states.getByRole("button", { name: "1 failed" }).click()
    await expect(air).toHaveAttribute("data-state", "open")
    await expect(getBranchDetailsRow(page, "side-fix")).toBeVisible()
  })

  test("should push the whole stack from the header, parents first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})
    await island(page, "air").getByRole("button", { name: "Push with Children (7 branches)" }).click()
    // each branch after the one it is built on
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["air", "bazel", "side-fix", "bridge", "runtime", "deploy", "side-test"])
    // everything is on the remote: no push left in the header
    await expect(island(page, "air").getByTestId("stack-island-push")).toHaveCount(0)
  })

  test("should push the stack up to a commit, the branches it is built on first, the rest left to push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})
    const line = page.locator("[data-testid=branch-commit-lines-row][data-branch-of=bazel] [data-testid=stack-commit-line]").first()
    await expect(line).toContainText("Build the deploy jar, part 1")
    await line.getByTestId("commit-push-mark").click()
    const popover = page.getByTestId("push-up-to")
    await expect(popover.getByTestId("push-up-to-plan")).toHaveText("Pushes 2 branches, parents first:\nair · 2 commits · creates the remote branch\nbazel · 1 of 2 commits · creates the remote branch")
    // its button marks what it sends: the commits up to here, the nodes of the branches
    await popover.getByTestId("push-up-to-button").hover()
    await expect(page.locator("[data-testid=stack-commit-line][data-sent]")).toHaveCount(3)
    await expect(findBranchRow(page, "bazel").getByTestId("lane-node")).toHaveAttribute("data-pushed", "true")
    await expect(findBranchRow(page, "bridge").getByTestId("lane-node")).not.toHaveAttribute("data-pushed", "true")

    await popover.getByTestId("push-up-to-button").click()
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["air", "bazel"])
    // air goes whole, bazel up to its first commit
    expect(pushes[0]!.tip ?? null).toBeNull()
    expect(pushes[1]!.tip).toMatch(/^[0-9a-f]{40}$/)
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
