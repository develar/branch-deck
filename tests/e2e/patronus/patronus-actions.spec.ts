// @spec spec/branch/patronus.spec.md
import type { Page, Route } from "@playwright/test"
import type { PatronusMergeRequest, PatronusReviewRequirement, PatronusRobot, PatronusRobotDetails, PatronusStartOptions } from "../../../src/lib/bindings"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton } from "../helpers/branch-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import type { PushResult } from "../helpers/push-helpers"
import { pushOutcome, routePush } from "../helpers/push-helpers"
import { hoverStatusChip } from "../helpers/review-status-helpers"

// the `issue_links` template: 7 branches, none pushed
const RUNNING = robot("JIRA-456", "running-robot")
const FAILED = robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })

const START_OPTIONS: PatronusStartOptions = {
  targetBranch: "master",
  supported: true,
  profiles: [
    { id: "lsc", name: "Large Scale Change", description: "All the tests", allowedPushModes: [] },
    { id: "dry-only", name: "Dry Only", description: null, allowedPushModes: ["dryRun"] },
  ],
  defaultProfileId: null,
}

/** Answers a Patronus command in the page and records the parameters of each call */
async function routeCommand(page: Page, command: string, answer: (params: Record<string, unknown>) => unknown): Promise<Record<string, unknown>[]> {
  const calls: Record<string, unknown>[] = []
  await page.route(`**/invoke/${command}`, async (route: Route) => {
    const params = route.request().postDataJSON() as Record<string, unknown>
    calls.push(params)
    await route.fulfill({ json: answer(params) })
  })
  return calls
}

/** The issue_links repository with these robots; the unassigned commits card renders after the branches and would move the rows away from the pointer */
async function setupRobots(page: Page, setupRepo: (template: string, options: { patronus: unknown }) => Promise<unknown>, syncAndWaitForBranches: () => Promise<unknown>, ...robots: PatronusRobot[]) {
  await setupRepo("issue_links", { patronus: availableRobots(...robots) })
  await syncAndWaitForBranches()
  await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
}

function chip(page: Page, branchName: string) {
  return findBranchRow(page, branchName).getByTestId("patronus-chip")
}

/** The first push of JIRA-456 finds its remote branch moved (someone pushed a commit), the second one goes through */
const REMOTE_MOVED: PushResult[] = [
  {
    outcome: "remoteMoved",
    unpushedCommits: ["j1", "j2", "j3"],
    commitsBehind: 1,
    remoteTip: "moved-tip",
    droppedCommits: [{ id: "aaaa000011112222", subject: "Fix the pool", author: "Ann", authorTime: 1704178800 }],
  },
  {},
]

/** Pushes JIRA-456 into its running robot with `answer`; the remote moved, so the warning toast comes up */
async function pushIntoMovedRemote(page: Page, answer: "Push Anyway" | "Cancel, Push and Restart") {
  await getPushButton(findBranchRow(page, "JIRA-456")).click()
  await page.getByTestId("push-guard-dialog").getByRole("button", { name: answer }).click()
  const toast = page.locator("[data-slot=toast]", { hasText: "JIRA-456 changed on the remote" })
  await expect(toast).toBeVisible()
  return toast
}

test.describe("Patronus actions", () => {
  test("should start a Safe Push from the branch menu, pushing the branch first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await routePush(page, {})
    const options = await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const starts = await routeCommand(page, "start_patronus_robot", () => robot("JIRA-456", "started-robot", { sourceRevision: null }) satisfies PatronusRobot)

    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Safe Push…" }).click()
    const form = page.getByTestId("patronus-start-form")
    await expect(form.getByTestId("patronus-start-push-note")).toHaveText("JIRA-456 isn't on the remote yet: it is pushed first, then Patronus starts from the remote branch.")
    await form.getByTestId("patronus-profile").click()
    // a profile that allows only dry runs is not offered for a Safe Push
    await expect(page.getByRole("option", { name: "Dry Only" })).toHaveCount(0)
    await page.getByRole("option", { name: "Large Scale Change" }).click()
    await form.getByRole("button", { name: "Push and Start Safe Push" }).click()

    await expect(chip(page, "JIRA-456")).toHaveText("Running")
    await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
    expect(options[0]).toMatchObject({ baselineBranch: "origin/master", lastRobotId: null })
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash", profileId: "lsc" })])
  })

  test("should cancel the robot for a new Safe Push from the form, and not restart it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the robot runs on an older commit, so the form can start one on the branch as it is
    const older = robot("JIRA-456", "running-robot", { sourceRevision: "older-commit" })
    await setupRobots(page, setupRepo, syncAndWaitForBranches, older)
    const order: string[] = []
    const record = (call: string, answer: unknown) => () => {
      order.push(call)
      return answer
    }
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    await routeCommand(page, "get_patronus_robot_status", () => "running")
    await routeCommand(page, "cancel_patronus_robot", record("cancel", { ...older, status: "canceling" }))
    await routeCommand(page, "push_branch", record("push", pushOutcome("JIRA-456")))
    await routeCommand(page, "restart_patronus_robot", record("restart", robot("JIRA-456", "restarted-robot")))
    const starts = await routeCommand(page, "start_patronus_robot", record("start", robot("JIRA-456", "started-robot", { sourceRevision: null })))

    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Safe Push…" }).click()
    await page.getByTestId("patronus-start-form").getByRole("button", { name: "Push and Start Safe Push" }).click()

    // the form's start is the new robot: no restart of the old one
    const dialog = page.getByTestId("push-guard-dialog")
    await expect(dialog).toContainText("The new Safe Push takes the pushed commit.")
    await expect(dialog.getByRole("button", { name: "Cancel, Push and Restart" })).toHaveCount(0)
    await dialog.getByRole("button", { name: "Cancel and Push" }).click()

    await expect.poll(() => order).toEqual(["cancel", "push", "start"])
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash" })])
    await expect(chip(page, "JIRA-456")).toHaveText("Running")
  })

  test("should restart a failed robot and cancel a running one from the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED)
    const restarts = await routeCommand(page, "restart_patronus_robot", () => robot("ABC-123", "restarted-robot"))
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => ({ ...RUNNING, status: "canceling" }))

    let popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Cancel" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Restart" }).click()
    await expect(chip(page, "ABC-123")).toHaveText("Running")
    expect(restarts).toEqual([expect.objectContaining({ robotId: "failed-robot" })])
    await page.keyboard.press("Escape")

    popover = await hoverStatusChip(page, chip(page, "JIRA-456"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Restart" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Cancel" }).click()
    await expect(chip(page, "JIRA-456")).toHaveText("Canceling")
    expect(cancels).toEqual([expect.objectContaining({ robotId: "running-robot" })])
  })

  test("should show the profile, the checks and the problems in the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, FAILED)
    await routeCommand(page, "get_patronus_robot_details", () => ({
      robot: { ...FAILED, sourceRevision: null },
      profileId: "lsc",
      profileName: "Large Scale Change",
      checks: { failed: 1, running: 0, passed: 9, canceled: 0, failedNames: ["Fast Tests"] },
      problems: [{ title: "Tests failed", subsystem: "TEAMCITY" }],
    }) satisfies PatronusRobotDetails)

    const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByTestId("patronus-profile-name")).toHaveText("Large Scale Change")
    await expect(popover.getByTestId("patronus-checks")).toHaveText("1 failing (Fast Tests), 9 passed")
    await expect(popover.getByTestId("patronus-problems")).toHaveText("Tests failed")
  })

  test.describe("code-owner review", () => {
    const MERGE_REQUEST: PatronusMergeRequest = { projectKey: "IJ", reviewKey: "IJ-CR-7", url: "https://jetbrains.team/p/ij/reviews/7" }
    const REQUIRED: PatronusReviewRequirement = {
      required: true,
      mergeRequest: null,
      groups: [{ name: "Air Plugin Release Team", reviewers: [{ fullName: "Alice", username: "alice", available: true, suggested: true }] }],
      skipped: false,
    }

    test("should name the missing approvals, create the merge request and start with it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const checks = await routeCommand(page, "get_patronus_review_requirement", params => params.recheck === true ? { ...REQUIRED, required: false, groups: [], mergeRequest: MERGE_REQUEST } : REQUIRED)
      const creates = await routeCommand(page, "create_patronus_merge_request", () => MERGE_REQUEST)
      const starts = await routeCommand(page, "start_patronus_robot", () => robot("JIRA-456", "started-robot", { sourceRevision: null }))

      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Safe Push…" }).click()
      const review = page.getByTestId("patronus-review")
      await expect(review).toHaveAttribute("data-status", "required")
      await expect(review.getByTestId("patronus-review-groups")).toHaveText("Air Plugin Release Team (suggested: Alice)")

      await review.getByRole("button", { name: "Create Merge Request" }).click()
      // pushed first: Space opens a merge request of a pushed branch
      await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
      await expect(review).toHaveAttribute("data-status", "approved")
      await expect(review).toHaveText("Approved in IJ-CR-7.")
      expect(await getOpenedUrls(page)).toEqual([MERGE_REQUEST.url])
      expect(creates).toEqual([expect.objectContaining({ branchName: "JIRA-456", baselineBranch: "origin/master" })])
      expect(checks.map(params => params.recheck)).toEqual([false, true])

      await page.getByTestId("patronus-start-form").getByRole("button", { name: /Start Safe Push/ }).click()
      await expect(chip(page, "JIRA-456")).toHaveText("Running")
      expect(starts).toEqual([expect.objectContaining({ mergeRequest: MERGE_REQUEST })])
    })

    test("should not check the review of a dry run", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const checks = await routeCommand(page, "get_patronus_review_requirement", () => REQUIRED)
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Dry Run…" }).click()
      await expect(page.getByTestId("patronus-start-explanation")).toContainText("without pushing into master")
      await expect(page.getByTestId("patronus-review")).toHaveCount(0)
      expect(checks).toHaveLength(0)
    })

    test("should say \"Needs review\" for a robot that failed for missing approvals, and offer a new Safe Push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await routeCommand(page, "get_patronus_robot_problems", () => [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }])
      await setupRobots(page, setupRepo, syncAndWaitForBranches, FAILED)
      await expect(chip(page, "ABC-123")).toHaveText("Needs review")
      await expect(chip(page, "ABC-123")).toHaveAttribute("data-color", "warning")

      const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
      await expect(popover.getByTestId("patronus-needs-review")).toBeVisible()
      await expect(popover.getByRole("button", { name: "Restart" })).toHaveCount(0)
      await popover.getByRole("button", { name: "Safe Push…" }).click()
      await expect(page.getByTestId("patronus-start-form")).toBeVisible()
    })
  })

  test.describe("pushing into a branch whose Safe Push runs", () => {
    test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    })

    test("should ask first, and push only when told to", async ({ page }) => {
      const pushes = await routeCommand(page, "push_branch", params => pushOutcome(params.branchName as string))
      const statuses = await routeCommand(page, "get_patronus_robot_status", () => "running")
      const dialog = page.getByTestId("push-guard-dialog")

      await getPushButton(findBranchRow(page, "JIRA-456")).click()
      await expect(dialog).toContainText("Safe Push of JIRA-456 is running")
      await page.keyboard.press("Escape")
      await expect(dialog).toBeHidden()
      expect(pushes).toHaveLength(0)
      expect(statuses).toHaveLength(1)

      await getPushButton(findBranchRow(page, "JIRA-456")).click()
      await dialog.getByRole("button", { name: "Push Anyway" }).click()
      await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
      expect(pushes).toHaveLength(1)
    })

    test("should cancel the robot, push and restart it on the pushed commit", async ({ page }) => {
      const order: string[] = []
      await routeCommand(page, "get_patronus_robot_status", () => "running")
      await routeCommand(page, "cancel_patronus_robot", () => {
        order.push("cancel")
        return { ...RUNNING, status: "canceling" }
      })
      await routeCommand(page, "push_branch", (params) => {
        order.push("push")
        return pushOutcome(params.branchName as string)
      })
      await routeCommand(page, "restart_patronus_robot", () => {
        order.push("restart")
        return robot("JIRA-456", "restarted-robot", { name: "Restarted" })
      })

      await getPushButton(findBranchRow(page, "JIRA-456")).click()
      await page.getByTestId("push-guard-dialog").getByRole("button", { name: "Cancel, Push and Restart" }).click()
      await expect(page.getByText("Safe Push restarted")).toBeVisible()
      expect(order).toEqual(["cancel", "push", "restart"])
      await expect(chip(page, "JIRA-456")).toHaveText("Running")
    })

    test("should push at once when the robot finished meanwhile", async ({ page }) => {
      await routePush(page, {})
      await routeCommand(page, "get_patronus_robot_status", () => "failure")
      await getPushButton(findBranchRow(page, "JIRA-456")).click()
      await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
      await expect(page.getByTestId("push-guard-dialog")).toHaveCount(0)
      await expect(chip(page, "JIRA-456")).toHaveText("Failed")
    })

    test("should ask again before the push of the remote-moved toast", async ({ page }) => {
      const statuses = await routeCommand(page, "get_patronus_robot_status", () => "running")
      const pushes = await routePush(page, { "JIRA-456": REMOTE_MOVED })
      const toast = await pushIntoMovedRemote(page, "Push Anyway")
      await expect(toast).not.toContainText("canceled")

      await toast.getByRole("button", { name: "Force Push" }).click()

      // the robot still runs: the same question, and Back pushes nothing
      const dialog = page.getByTestId("push-guard-dialog")
      await expect(dialog).toContainText("Safe Push of JIRA-456 is running")
      await page.keyboard.press("Escape")
      await expect(dialog).toBeHidden()
      expect(pushes.map(push => push.expectedRemote)).toEqual([null])
      expect(statuses).toHaveLength(2)
    })

    test("should restart a robot canceled for a push the remote refused once the toast pushes", async ({ page }) => {
      const order: string[] = []
      let canceled = false
      await routeCommand(page, "get_patronus_robot_status", () => canceled ? "canceled" : "running")
      await routeCommand(page, "cancel_patronus_robot", () => {
        order.push("cancel")
        canceled = true
        return { ...RUNNING, status: "canceling" }
      })
      await routeCommand(page, "restart_patronus_robot", () => {
        order.push("restart")
        return robot("JIRA-456", "restarted-robot", { name: "Restarted" })
      })
      const pushes = await routePush(page, { "JIRA-456": REMOTE_MOVED })
      const toast = await pushIntoMovedRemote(page, "Cancel, Push and Restart")
      await expect(toast).toContainText("The Safe Push is canceled: the push restarts it.")
      expect(order).toEqual(["cancel"])

      await toast.getByRole("button", { name: "Force Push" }).click()

      // canceled now: no question, and the restart takes the pushed commit
      await expect(page.getByText("Safe Push restarted")).toBeVisible()
      expect(order).toEqual(["cancel", "restart"])
      expect(pushes.map(push => push.expectedRemote)).toEqual([null, "moved-tip"])
      await expect(page.getByTestId("push-guard-dialog")).toHaveCount(0)
    })
  })
})
