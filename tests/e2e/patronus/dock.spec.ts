// @spec spec/repository/robots-indicator.spec.md
import type { PatronusRobotsResult } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { getDockCalls } from "../helpers/header-helpers"
import { DAY_ROBOTS, availableRobots, robot, setupRobots, setupRobotsWith, willFail } from "../helpers/patronus-helpers"
import { emitTauriEvent, runAndWaitForSync } from "../helpers/sync-helpers"

const { running: RUNNING, failed: FAILED, merged: MERGED } = DAY_ROBOTS

test.describe("Dock", () => {
  // the user agent of the Desktop Chrome device is the one of Windows, which has no badge count
  test.use({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36" })

  test("should show the progress of the robot nearest to finishing, and a badge only for what needs the user", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED, MERGED)
    await expect.poll(() => getDockCalls(page)).toEqual([{ progressBar: { status: "normal", progress: 45 } }, { badgeCount: 1 }])

    // only the clock moves: the progress follows it, the badge is not set again
    await page.clock.fastForward("02:00")
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toEqual({ progressBar: { status: "normal", progress: 50 } })
    expect((await getDockCalls(page)).filter(call => "badgeCount" in call)).toHaveLength(1)
  })

  test("should show an indeterminate progress without history, and no badge for a running robot", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    await expect.poll(() => getDockCalls(page)).toEqual([{ progressBar: { status: "indeterminate", progress: 0 } }])
  })

  test("should count a running robot that will fail in the badge, and not show its progress", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobotsWith(page, setupRepo, syncAndWaitForBranches, { patronusDetails: willFail("running-robot") }, RUNNING)
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toEqual({ badgeCount: 1 })
    expect((await getDockCalls(page)).filter(call => "progressBar" in call).at(-1) ?? { progressBar: { status: "none" } }).toMatchObject({ progressBar: { status: "none" } })
  })

  test("should count the failed robot of the whole stack only while the sync built its stack branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const failed = robot("air", "whole", {
      name: "Safe Push of the whole air stack",
      branchName: null,
      stackRoot: "air",
      status: "failure",
      sourceRevision: "stack-tip",
      finishTime: 1704142800,
    })
    // the robot failed on the stack branch as the sync built it
    const repo = await setupRepo("stack_island", { patronus: availableRobots(failed), syncOverrides: { stacks: { air: { tip: "stack-tip" } } } })
    await syncAndWaitForBranches()
    const states = page.locator(`[data-testid="stack-island"][data-stack-root="air"]`).getByTestId("stack-island-states")
    // the fix of the whole stack, then the branches, each a step of the user
    await expect(states.getByRole("button")).toHaveText(["1 to fix", "7 your step"])
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toEqual({ badgeCount: 1 })

    // the stack no longer forks: its stack branch stays for its remote branch, not rebuilt, and the stack lands in pieces
    await repo.setSyncOverrides({
      stacks: { air: { state: "Stale", members: [], tip: "stack-tip", error: "Not rebuilt: air heads no stack that forks; the ref stays for its remote branch" } },
    })
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toEqual({ badgeCount: null })
    // the header keeps what the branches hold
    await expect(states.getByRole("button")).toHaveText(["7 your step"])
  })

  test("should not count the failed robot of a branch while the Safe Push of a branch built on it lands it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const loginFailed = robot("login", "login-failed", { status: "failure", finishTime: 1704139300 })
    const oauth = robot("oauth", "oauth-robot")
    const repo = await setupRepo("stacked_branches", { patronus: availableRobots(loginFailed, oauth) })
    await syncAndWaitForBranches()
    // oauth is built on login: its Safe Push lands login, whose old failure asks for nothing
    await expect(findBranchRow(page, "login").getByTestId("landing-span")).toHaveText("lands with oauth")
    await expect.poll(() => getDockCalls(page)).toEqual([{ progressBar: { status: "indeterminate", progress: 0 } }])

    // canceled, it lands nothing: the failure of login is the user's move again
    const polled = page.waitForResponse(response => response.url().endsWith("/invoke/poll_patronus_robots"))
    await repo.setPatronus(availableRobots(loginFailed, { ...oauth, status: "canceled", finishTime: 1704142800 }))
    await emitTauriEvent(page, "tauri://focus")
    expect(((await (await polled).json()) as PatronusRobotsResult).robots.find(it => it.id === oauth.id)?.status).toBe("canceled")
    // the badge comes only now, after the progress of oauth ended: none before it
    await expect.poll(() => getDockCalls(page)).toEqual([{ progressBar: { status: "indeterminate", progress: 0 } }, { progressBar: { status: "none" } }, { badgeCount: 1 }])
  })
})

test.describe("Taskbar", () => {
  test("should show the progress, without a badge on Windows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED, MERGED)
    await expect.poll(() => getDockCalls(page)).toEqual([{ progressBar: { status: "normal", progress: 45 } }])
    await expect(page.getByTestId("landing-indicator")).toBeVisible()
    expect(await getDockCalls(page)).toHaveLength(1)
  })
})
