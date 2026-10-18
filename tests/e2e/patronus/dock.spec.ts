// @spec spec/repository/robots-indicator.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { getDockCalls } from "../helpers/header-helpers"
import { DAY_ROBOTS, setupRobots } from "../helpers/patronus-helpers"
import { routeCommand } from "../helpers/wait-helpers"

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
    await routeCommand(page, "get_patronus_failing_robots", () => ["running-robot"])
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toEqual({ badgeCount: 1 })
    expect((await getDockCalls(page)).filter(call => "progressBar" in call).at(-1) ?? { progressBar: { status: "none" } }).toMatchObject({ progressBar: { status: "none" } })
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
