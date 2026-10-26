// @spec spec/branch/stacked-branches.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRows, getBranchDetailsRow } from "../helpers/branch-helpers"
import { mergeRunningRobot, robot, setupRunningRobot } from "../helpers/patronus-helpers"
import { remotePushes, virtualBranch } from "../helpers/push-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"

// the `stack_island` template: air → bazel → bridge → runtime → deploy with the side chain side-fix → side-test off
// bazel, none of them on the remote; the commits of a branch are the same wherever its row is, here in the island
test.describe("Push Up to Here", () => {
  test("should push the stack up to a commit, the branches it is built on first, the rest left to push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stack_island")
    await syncAndWaitForBranches()
    // the arrow is on the commits of the full list
    await expandBranch(page, findBranchRow(page, "air"))
    await expandBranch(page, findBranchRow(page, "bazel"))
    const first = getBranchCommitRows(page, "bazel").first()
    await expect(first).toContainText("Build the deploy jar, part 1")
    await first.getByTestId("commit-push-mark").click()
    const popover = page.getByTestId("push-up-to")
    await expect(popover.getByTestId("push-up-to-plan")).toHaveText(
      "Pushes 2 branches, parents first:\nair · 2 commits · creates the remote branch\nbazel · 1 of 2 commits · creates the remote branch",
    )
    // the click opened the popover, not the selection of the commit
    await expect(first).toHaveAttribute("data-selected", "false")
    // its button marks what it sends: the commits up to here, the nodes of the branches
    await popover.getByTestId("push-up-to-button").hover()
    await expect(getBranchCommitRows(page, "air").and(page.locator("[data-sent]"))).toHaveCount(2)
    await expect(getBranchCommitRows(page, "bazel").and(page.locator("[data-sent]"))).toHaveCount(1)
    await expect(first).toHaveAttribute("data-sent", "true")
    await expect(findBranchRow(page, "bazel").getByTestId("lane-node")).toHaveAttribute("data-pushed", "true")
    await expect(findBranchRow(page, "bridge").getByTestId("lane-node")).not.toHaveAttribute("data-pushed", "true")

    await popover.getByTestId("push-up-to-button").click()
    // one push: air goes whole, bazel up to its first commit
    await expect.poll(() => remotePushes(repo)).toEqual([["air", "bazel"]])
    const remote = await repo.getRemoteBranches()
    expect(remote[virtualBranch("air")]).toBe(await repo.revision(virtualBranch("air")))
    expect(remote[virtualBranch("bazel")]).toBe(await repo.revision(`${virtualBranch("bazel")}~1`))
  })

  test("should show no push arrow on the commits a merged Safe Push landed, only on a commit added after them", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the Safe Push of bazel lands air with it
    const running = robot("bazel", "bazel-robot")
    const repo = await setupRunningRobot(page, setupRepo, syncAndWaitForBranches, "stack_island", running, "air")
    for (const name of ["air", "bazel", "bridge"]) {
      await expandBranch(page, findBranchRow(page, name))
    }
    const marks = (name: string) => getBranchCommitRows(page, name).getByTestId("commit-push-mark")
    await expect(marks("air")).toHaveCount(2)
    await expect(marks("bazel")).toHaveCount(2)

    // merged: its commits and air's are in master, their rows without the arrow
    await mergeRunningRobot(page, repo, running)
    await expect(findBranchRow(page, "air").getByTestId("landed-with")).toHaveText("Landed with bazel")
    for (const name of ["air", "bazel"]) {
      await expect(getBranchCommitRows(page, name)).toHaveCount(2)
      await expect(marks(name)).toHaveCount(0)
    }
    // bridge, built on bazel, still has its own to push
    await expect(marks("bridge")).toHaveCount(2)

    // a new commit of air: the arrow is on it alone, the only work left of air
    await runAndWaitForSync(page, async () => {
      await repo.commit("(air) Add the agent, part 3", { "air/part3.txt": "Add the agent 3\n" }, 1704200000)
      await page.getByTestId("sync-button").click()
    })
    await expect(findBranchRow(page, "air").getByTestId("landed-with")).toHaveText("Landed with bazel · 1 commit after it")
    const airCommits = getBranchCommitRows(page, "air")
    await expect(airCommits).toHaveCount(3)
    await expect(airCommits).toContainText(["Add the agent, part 1", "Add the agent, part 2", "Add the agent, part 3"])
    await expect(marks("air")).toHaveCount(1)
    await expect(airCommits.last().getByTestId("commit-push-mark")).toHaveCount(1)
    await expect(marks("bazel")).toHaveCount(0)
  })

  test("should push up to a commit from its menu in the full list", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stack_island")
    await syncAndWaitForBranches()
    await expandBranch(page, findBranchRow(page, "bazel"))
    const first = getBranchDetailsRow(page, "bazel").locator("tr[data-row-id]").first()
    await expect(first).toContainText("Build the deploy jar, part 1")
    await openContextMenu(page, first)
    await clickContextMenuItem(page, "Push Up to Here")
    await expect.poll(() => remotePushes(repo)).toEqual([["air", "bazel"]])
  })

  test("should offer Push Up to Here in the full list only for a commit added after the ones a merged Safe Push landed, without the landed branches", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the Safe Push of bazel lands air with it
    const running = robot("bazel", "bazel-robot")
    const repo = await setupRunningRobot(page, setupRepo, syncAndWaitForBranches, "stack_island", running, "air")
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
    await expect.poll(() => remotePushes(repo)).toEqual([["bazel"]])
  })
})
