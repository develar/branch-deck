// @spec spec/repository/landing-overview.spec.md
import type { PatronusRobotDetails } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { activityPanel } from "../helpers/activity-helpers"
import { findBranchRow, getPushButton, verifyBranchState } from "../helpers/branch-helpers"
import { expectColumnAligned } from "../helpers/column-helpers"
import {
  AWAITING_CODE_OWNER,
  DAY_ROBOTS,
  NO_REVIEW_NEEDED,
  ROBOTS_NOW,
  availableRobots,
  expectPatronusStarts,
  minutes,
  robot,
  setupRobots,
  startRequest,
} from "../helpers/patronus-helpers"
import { availablePullRequests, createPullRequestDialog, getOpenedUrls } from "../helpers/pull-request-helpers"
import { pushedToast, routePush } from "../helpers/push-helpers"
import { EARLIER_COMMIT, landingRow, openLandingOverview } from "../helpers/review-status-helpers"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"
import { routeCommand } from "../helpers/wait-helpers"

const NOW = ROBOTS_NOW
const { running: RUNNING, failed: FAILED, merged: MERGED } = DAY_ROBOTS
// a branch merged and archived since
const ARCHIVED = robot("billing", "archived-robot", { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - minutes(200), finishTime: NOW - minutes(160) })
// a week ago: not of the last day
const OLD = robot("feature-docs", "old-robot", { status: "canceled", startTime: NOW - minutes(3000), finishTime: NOW - minutes(2990) })

test.describe("Landing overview", () => {
  test("should count the user's moves, put the most urgent node in the header and group the work by where it is", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED, MERGED, ARCHIVED, OLD)
    const indicator = page.getByTestId("landing-indicator")
    // ABC-123 needs a fix (its Safe Push failed on it as it is); the four branches GitHub lists no pull request of
    // are a Safe Push nobody judged; JIRA-456 is landing, feature-db is in master
    await expect(indicator).toHaveAccessibleName("On the way to master: 5 your move · 1 landing · 1 in master")
    await expect(indicator.getByTestId("landing-moves")).toHaveText("5")
    await expect(indicator).toHaveAttribute("data-urgent", "needsFix")
    await expect(indicator.getByTestId("robot-ring")).toBeVisible()

    const popover = await openLandingOverview(page)
    await expect(popover.getByRole("heading", { level: 2 })).toHaveText("On the way to master: 5 your move · 1 landing · 1 in master")
    // the most urgent first: the failed robot with its actions
    const moves = popover.getByTestId("landing-your-move")
    await expect(moves.getByTestId("robots-row").first()).toHaveAttribute("data-branch-name", "ABC-123")
    await expect(moves.getByTestId("robots-row").first().getByTestId("robots-row-state")).toHaveText("Failed · 50 minutes ago")
    await expect(moves.getByTestId("landing-row")).toHaveCount(4)
    const running = popover.getByTestId("landing-running").getByTestId("robots-row")
    await expect(running).toHaveAttribute("data-branch-name", "JIRA-456")
    await expect(running.getByTestId("robots-row-state")).toHaveText("18m · ~22m")
    await expect(running.getByTestId("robot-progress")).toHaveAttribute("aria-valuenow", "45")
    await expect(landingRow(popover.getByTestId("landing-in-master"), "feature-db").getByTestId("landing-row-reason")).toHaveText("Merged by Patronus")
    // the reason of a row is the hover of its node: the state in full, what it names opens from there
    await landingRow(popover.getByTestId("landing-in-master"), "feature-db").getByTestId("landing-row-reason").hover()
    await expect(page.getByTestId("lane-node-details")).toHaveText("Merged by Patronus")
    // the last day in one line, without what the In master row says: a branch archived since; its history is Activity
    const finished = popover.getByTestId("landing-finished")
    await expect(finished).toContainText("billing merged in the last 24 hours")
    await expect(popover.getByTestId("robots-row").filter({ hasText: "billing" })).toHaveCount(0)
    // the title stays at the top, the line and the footer at the bottom, the sections scroll between them
    await expect(popover.getByTestId("landing-sections")).toHaveCSS("overflow-y", "auto")

    // a branch name shows its row
    await running.getByRole("button", { name: "JIRA-456", exact: true }).click()
    await expect(popover).toBeHidden()
    await verifyBranchState(findBranchRow(page, "JIRA-456"), "open")
    await page.mouse.move(0, 0)

    // Activity has the history of the robots: Show in Activity opens its side panel at its Robots
    await (await openLandingOverview(page)).getByTestId("landing-finished").getByRole("button", { name: "Show in Activity" }).click()
    await expect(popover).toBeHidden()
    await expect(
      activityPanel(page)
        .getByTestId("activity-filter")
        .getByRole("button", { name: /Robots/ }),
    ).toHaveAttribute("aria-pressed", "true")
  })

  test("should queue the robots nearest to finishing first, and fit the window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a Safe Push usually takes 40 minutes: ABC-123 has 10 left, JIRA-456 22; a Dry Run lands nothing, it comes last
    const history = [1, 2].map(n => robot(`merged-${n}`, `history-${n}`, { status: "success", startTime: NOW - minutes(60 * n + 40), finishTime: NOW - minutes(60 * n) }))
    await page.setViewportSize({ width: 1280, height: 420 })
    await setupRobots(
      page,
      setupRepo,
      syncAndWaitForBranches,
      RUNNING,
      robot("ABC-123", "abc-robot", { startTime: NOW - minutes(30) }),
      robot("feature-auth", "dry-robot", { pushMode: "dryRun", startTime: NOW - minutes(35) }),
      ...history,
    )
    const popover = await openLandingOverview(page)
    const running = popover.getByTestId("landing-running")
    await expect(running.getByRole("heading")).toHaveText("Landing · 2 · 1 dry run")
    await expect(running.getByTestId("robots-row-state")).toHaveText(["30m · ~10m", "18m · ~22m", /^Dry Run · 35m/])
    expect(await running.getByTestId("robots-row").evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name")))).toEqual(["ABC-123", "JIRA-456", "feature-auth"])

    // below its button, as a dropdown, never beside it, and above the bottom of the window: the line of the last day
    // stays, the sections scroll
    const box = await popover.boundingBox()
    const button = await page.getByTestId("landing-indicator").boundingBox()
    expect(box!.y).toBeGreaterThanOrEqual(button!.y + button!.height)
    expect(box!.y + box!.height).toBeLessThanOrEqual(420)
    await expect(popover.getByTestId("landing-finished")).toBeInViewport()
    const sections = popover.getByTestId("landing-sections")
    expect(await sections.evaluate(element => element.scrollHeight > element.clientHeight)).toBe(true)
    await running.getByTestId("robots-row").last().scrollIntoViewIfNeeded()
    await expect(running.getByTestId("robots-row").last()).toBeInViewport()
    await expect(popover.getByTestId("landing-finished")).toBeInViewport()
  })

  test("should cancel a running robot from the overview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    const popover = await openLandingOverview(page)
    await popover.getByRole("button", { name: "Cancel Safe Push of JIRA-456" }).click()
    // the toast comes with the answer of the cancel: the request reached Patronus by then
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push is canceling" })).toBeVisible()
    expect((await repo.getPatronusLog()).cancels).toEqual(["running-robot"])
    await expect(popover.getByTestId("landing-running").getByTestId("robots-row-state")).toHaveText("Canceling")
  })

  test("should restart a failed robot from the overview, and open the pull request it was started from", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const pullRequest = { number: 7, url: "https://github.com/acme/monorepo/pull/7" }
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, { ...FAILED, pullRequest })
    const popover = await openLandingOverview(page)
    const row = popover.getByTestId("landing-your-move").getByTestId("robots-row")
    // the actions of the branch menu and the robot details, in their order and under their names
    await expect(row.getByRole("button", { name: "Restart Safe Push of ABC-123" })).toBeVisible()
    expect(await row.locator("button[aria-label]").evaluateAll(buttons => buttons.map(it => it.getAttribute("aria-label")))).toEqual([
      "Restart Safe Push of ABC-123",
      "Copy Prompt for Agent of ABC-123",
      "Copy Prompt with CI Details of ABC-123",
      "Open Safe Push of ABC-123 in Patronus",
      "Open Pull Request #7 of ABC-123",
    ])
    await row.getByRole("button", { name: "Open Pull Request #7 of ABC-123" }).click()
    expect(await getOpenedUrls(page)).toEqual([pullRequest.url])
    // ↗ opens the robot; a Safe Push goes without its mode
    await expect(row.getByTestId("robots-row-mode")).toHaveCount(0)
    await row.getByRole("button", { name: "Open Safe Push of ABC-123 in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual([pullRequest.url, FAILED.url])
    await row.getByRole("button", { name: "Restart Safe Push of ABC-123" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })).toBeVisible()
    expect((await repo.getPatronusLog()).restarts).toEqual(["failed-robot"])
  })

  test("should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await routeCommand(page, "get_patronus_failing_robots", () => ["running-robot"])
    await routeCommand(
      page,
      "get_patronus_robot_details",
      () =>
        ({
          robot: RUNNING,
          profileId: null,
          profileName: null,
          checks: { failed: 1, failedNames: ["Bazel Community Build Aggregator"], running: 2, passed: 9, canceled: 0 },
          problems: [],
          failing: true,
          progress: { remainingSeconds: 1500, checkName: "IntelliJ Aggregator", checkPercent: 36, overtimeSeconds: null, queued: 0, probablyHanging: false },
        }) satisfies PatronusRobotDetails,
    )
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    const toast = page.locator("[data-slot=toast]", { hasText: "A required check of JIRA-456 failed" })
    await expect(toast).toContainText("Bazel Community Build Aggregator · the Safe Push will fail")
    const indicator = page.getByTestId("landing-indicator")
    await expect(indicator).toHaveAttribute("data-urgent", "needsFix")
    const popover = await openLandingOverview(page)
    // no use waiting for it: a move, not landing
    await expect(popover.getByTestId("landing-running")).toHaveCount(0)
    const running = popover.getByTestId("landing-your-move").getByTestId("robots-row")
    await expect(running).toHaveAttribute("data-branch-name", "JIRA-456")
    await expect(running.getByTestId("robots-row-state")).toHaveText("18m · Failing")
    await expect(running.getByTestId("robots-row-failed")).toHaveText("Failed: Bazel Community Build Aggregator")
    await expect(running.getByTestId("robot-progress")).toHaveAttribute("data-failing", "true")
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()

    // no use waiting for it: the toast cancels it
    await toast.getByRole("button", { name: "Cancel Safe Push" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push is canceling" })).toBeVisible()
    expect((await repo.getPatronusLog()).cancels).toEqual(["running-robot"])
  })

  test("should run the next step of a move from the overview at its row: a Safe Push, a pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a stack is one move: its root's row, the step of the whole stack
    const repo = await setupRepo("stacked_branches", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests() })
    await repo.setReviewAnswers({ answers: { docs: NO_REVIEW_NEEDED, "stack:auth": AWAITING_CODE_OWNER } })
    await syncAndWaitForBranches()
    await routePush(page, {})
    let popover = await openLandingOverview(page)
    await expect(landingRow(popover, "auth").getByText("4 branches · 4 commits")).toBeVisible()
    await expect(landingRow(popover, "login")).toHaveCount(0)
    // the steps of the rows end in one column, as wide as the widest: "Safe Push" of docs, "Create Pull Request…" of the stack
    await expectColumnAligned(popover.getByTestId("landing-row").getByTestId("next-step"), "right")

    // the Safe Push of docs has nothing to ask: it pushes and starts at once, the row shown
    const docs = landingRow(popover, "docs")
    await expect(docs).toHaveAttribute("data-kind", "ready")
    await runAndWaitForAnswer(page, "start_patronus_robot", () => docs.getByTestId("next-step").click())
    await expect(popover).toBeHidden()
    await expectPatronusStarts(repo, [startRequest("docs")])
    await expect(findBranchRow(page, "docs")).toBeInViewport()
    await page.mouse.move(0, 0)

    // the step of the whole auth stack: its pull request, whose approvals its Safe Push counts
    popover = await openLandingOverview(page)
    const step = landingRow(popover, "auth").getByTestId("next-step")
    // the step of the row of the whole stack, short in its column; its menus name it "Create Pull Request of the Stack…"
    await expect(step).toHaveText("Create Pull Request…")
    await step.click()
    await expect(createPullRequestDialog(page)).toBeVisible()
  })

  test("should wait for the Safe Push of the root instead of the failure of the whole straight stack, once it runs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the `stack_push_alone` template: perf-part-2 on IJPL-257337, a straight chain whose whole stack is its last branch
    const failed = robot("perf-part-2", "leaf-failed", { status: "failure", startTime: NOW - minutes(180), finishTime: NOW - minutes(120) })
    const repo = await setupRepo("stack_push_alone", { patronus: availableRobots(failed) })
    await syncAndWaitForBranches()
    let popover = await openLandingOverview(page)
    // the robot of the whole stack failed on it as it is: the user's move, its restart the step
    const moves = popover.getByTestId("landing-your-move")
    await expect(landingRow(moves, "IJPL-257337")).toBeVisible()
    await expect(landingRow(moves, "IJPL-257337").getByTestId("next-step")).toHaveText("Restart Safe Push")

    // the user started a Safe Push of the root outside the app: it lands a part of the stack
    await repo.setPatronus(availableRobots(failed, robot("IJPL-257337", "root-running", { startTime: NOW - minutes(8) })))
    await popover.getByRole("button", { name: "Refresh Patronus" }).click()
    await expect(page.getByTestId("landing-indicator")).toHaveAccessibleName("On the way to master: 1 landing · 1 waiting")
    // the sections changed under the pointer: open the overview again
    await page.mouse.move(0, 0)
    popover = await openLandingOverview(page)
    await expect(popover.getByTestId("landing-running").getByTestId("robots-row")).toHaveAttribute("data-branch-name", "IJPL-257337")
    const waiting = landingRow(popover.getByTestId("landing-waiting"), "IJPL-257337")
    await expect(waiting.getByTestId("landing-row-reason")).toHaveText("A Safe Push of IJPL-257337 is running: wait until it merges")
    await expect(popover.getByTestId("landing-your-move")).toHaveCount(0)
    // the step of the stack is no restart: two robots would land the same commits; its Safe Push says why it waits
    const step = waiting.getByTestId("next-step")
    await expect(step).toHaveText("Safe Push · 2")
    await expect(step).toHaveAttribute("aria-disabled", "true")
  })

  test("should be hidden while nothing is on the way to master and nothing ran in the last day", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    await routePush(page, {})
    const indicator = page.getByTestId("landing-indicator")
    // not on Space, no pull requests GitHub lists: the push is the step
    await expect(indicator).toHaveAccessibleName("On the way to master: 1 your move")
    await getPushButton(findBranchRow(page, "test-branch")).click()
    await expect(pushedToast(page, "test-branch")).toBeVisible()
    // pushed: it lands through its pull request, which nobody opened; the work still waits
    await expect(indicator).toHaveAccessibleName("On the way to master: 1 waiting")
  })
})
