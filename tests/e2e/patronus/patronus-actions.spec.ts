// @spec spec/branch/patronus.spec.md
import type { Locator, Page } from "@playwright/test"
import type {
  PatronusMergeRequest,
  PatronusReviewRequirement,
  PatronusRobot,
  PatronusRobotDetails,
  PatronusStartOptions,
  PullRequest,
  TargetCheck,
} from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { expectNothingToPush, findBranchRow, getLandingNode, getMoreButton, getPushButton, pushFromRowMenu, verifyBranchState } from "../helpers/branch-helpers"
import { APPROVED_BY_CODE_OWNER, availableRobots, robot, routeReviewRequirement, setupRobots } from "../helpers/patronus-helpers"
import {
  availablePullRequests,
  createdPullRequest,
  createPullRequestDialog,
  getOpenedUrls,
  pullRequest,
  reviewer,
  routeNewPullRequest,
  unavailablePullRequests,
} from "../helpers/pull-request-helpers"
import type { PushResult } from "../helpers/push-helpers"
import { confirmForcePush, pushedToast, pushOutcome, routeHeldPush, routePush } from "../helpers/push-helpers"
import { hoverStatusChip, openBranchPreview, pushBranchFromRow } from "../helpers/review-status-helpers"
import { chooseProfile, expectPopoverWithoutReview, openSafePushWithKey, openSafePushWithProfile, popoverStart, safePushPopover, slotSafePush } from "../helpers/safe-push-helpers"
import { expectSettingsOpened } from "../helpers/settings-helpers"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { rewriteNextRemoteStatus, syncFromMenu } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { holdCommand, holdWithProgress, routeCommand } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches, none pushed
const RUNNING = robot("JIRA-456", "running-robot")
const FAILED = robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })
/** A Safe Push of JIRA-456 running on a commit the branch moved past */
const OLDER = robot("JIRA-456", "running-robot", { sourceRevision: "older-commit" })
/** The robot a start of JIRA-456 starts */
const STARTED = robot("JIRA-456", "started-robot", { sourceRevision: null })

const START_OPTIONS: PatronusStartOptions = {
  targetBranch: "master",
  supported: true,
  profiles: [
    { id: "lsc", name: "Large Scale Change", description: "All the tests", allowedPushModes: [], skipsReview: null },
    { id: "dry-only", name: "Dry Only", description: null, allowedPushModes: ["dryRun"], skipsReview: null },
  ],
  emergencyProfiles: [],
  defaultProfileId: null,
}

function chip(page: Page, branchName: string) {
  return findBranchRow(page, branchName).getByTestId("patronus-chip")
}

/** The toast whose text has `text` */
function toastWith(page: Page, text: string): Locator {
  return page.locator("[data-slot=toast]", { hasText: text })
}

/**
 * Answers `command` in the page with `answer` only once `release` is called: what the slot shows while the start waits
 * for it. `calls` are the params of each call.
 */
async function routeHeld(page: Page, command: string, answer: unknown): Promise<{ calls: Record<string, unknown>[]; release: () => void }> {
  let release!: () => void
  const held = new Promise<void>(resolve => (release = resolve))
  const calls: Record<string, unknown>[] = []
  await page.route(`**/invoke/${command}`, async route => {
    calls.push(route.request().postDataJSON() as Record<string, unknown>)
    await held
    await route.fulfill({ json: answer })
  })
  return { calls, release }
}

/** "Safe Push" of the branch menu (the right-click): a click, which starts at once when there is nothing to say */
async function safePushFromMenu(page: Page, branchName: string) {
  await findBranchRow(page, branchName).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Safe Push S", exact: true }).click()
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

/**
 * Answers the commands of a start that replaces the running robot `older` of JIRA-456 (its cancel, the push, a restart
 * and the new start), and records their order
 */
async function recordReplacingStart(page: Page, older: PatronusRobot) {
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
  const starts = await routeCommand(page, "start_patronus_robot", record("start", STARTED))
  return { order, starts }
}

/** The issue_links repository with `robots`, the pushes answered, the profiles, and a start of any branch answered */
async function setupStarts(
  page: Page,
  setupRepo: (template: string, options: object) => Promise<unknown>,
  syncAndWaitForBranches: () => Promise<unknown>,
  ...robots: PatronusRobot[]
): Promise<Record<string, unknown>[]> {
  await setupRobots(page, setupRepo, syncAndWaitForBranches, ...robots)
  await routePush(page, {})
  await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
  return routeCommand(page, "start_patronus_robot", params => robot(params.branchName as string, `${params.branchName as string}-robot`, { sourceRevision: null }))
}

/** "Create Pull Request…" of the slot of JIRA-456, whose approvals count only in a pull request it doesn't have: its dialog */
async function openCreatePullRequestFromSlot(page: Page): Promise<Locator> {
  const slot = findBranchRow(page, "JIRA-456").getByTestId("next-step-slot")
  await expect(slot).toHaveAttribute("data-step", "createPullRequest")
  await slot.getByRole("button", { name: "Create Pull Request…" }).click()
  return createPullRequestDialog(page)
}

/** Restart Safe Push of the running robot of JIRA-456, from its menu (it pushes first): the one push into a branch whose Safe Push runs */
/**
 * The restart of the running robot of JIRA-456 answered in the page, its status read running: the commands it ran, in
 * order ("cancel", "push", "restart")
 */
async function routeCancelPushRestart(page: Page): Promise<string[]> {
  const order: string[] = []
  await routeCommand(page, "get_patronus_robot_status", () => "running")
  await routeCommand(page, "cancel_patronus_robot", () => {
    order.push("cancel")
    return { ...OLDER, status: "canceling" }
  })
  await routeCommand(page, "push_branch", params => {
    order.push("push")
    return pushOutcome(params.branchName as string)
  })
  await routeCommand(page, "restart_patronus_robot", () => {
    order.push("restart")
    return robot("JIRA-456", "restarted-robot", { name: "Restarted" })
  })
  return order
}

async function pushAndRestartFromMenu(page: Page) {
  await findBranchRow(page, "JIRA-456").click({ button: "right" })
  await page.getByRole("menuitem", { name: "Restart Safe Push", exact: true }).click()
}

test.describe("Patronus actions", () => {
  test("should start a Safe Push from the branch menu, pushing the branch first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    const { pushes, release } = await routeHeldPush(page, "JIRA-456", 45)
    const options = await routeHeld(page, "get_patronus_start_options", START_OPTIONS)
    const starts = await routeHeld(page, "start_patronus_robot", STARTED satisfies PatronusRobot)
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => ({ ...STARTED, status: "canceling" }))

    // a lone branch, the default checks, no review needed: nothing to say, the click starts
    const button = slotSafePush(page, "JIRA-456")
    const more = getMoreButton(findBranchRow(page, "JIRA-456"))
    await expect(button).toHaveText("Safe Push")
    const rest = { width: (await button.boundingBox())!.width, moreX: (await more.boundingBox())!.x }
    // a step of the start is no wider than the label at rest: nothing moves under the pointer
    const expectInPlace = async () => {
      expect((await button.boundingBox())!.width).toBe(rest.width)
      expect((await more.boundingBox())!.x).toBe(rest.moreX)
    }
    await safePushFromMenu(page, "JIRA-456")
    // the slot's button shows each step of the start: the facts it decides on, the push, the start
    await expect(button).toHaveText("Checking…")
    await expect(button).toHaveAttribute("aria-busy", "true")
    await expectInPlace()
    options.release()
    // its push as the compact push of a row shows one: the spinner and the percent, the push in its name and hover
    await expect(button).toHaveText("45%")
    await expect(button).toHaveAccessibleName("Pushing 45%")
    await expectInPlace()
    // the push button of the row doesn't repeat the push
    const rowPush = getPushButton(findBranchRow(page, "JIRA-456"))
    await expect(rowPush).toBeDisabled()
    await expect(rowPush.getByRole("status")).toHaveCount(0)
    release()
    await expect(button).toHaveText("Starting…")
    await expectInPlace()
    starts.release()

    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
    expect(options.calls[0]).toMatchObject({ baselineBranch: "origin/master", lastRobotId: null })
    expect(starts.calls).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash", profileId: null })])
    // it started without the popover
    await expect(safePushPopover(page)).toHaveCount(0)

    // the toast names the branch and the target, never the ref the robot runs on, and opens the robot
    const started = toastWith(page, "Safe Push started")
    await expect(started.locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master")
    // the push is a step of the start: one action, one toast
    await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    await started.getByRole("button", { name: "Open in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual(["https://patronus.labs.jb.gg/robot/started-robot"])

    // a start asked by mistake costs only the robot: the toast cancels it
    await started.getByRole("button", { name: "Cancel Safe Push" }).click()
    await expect.poll(() => cancels).toEqual([expect.objectContaining({ robotId: "started-robot" })])
    await expect(started).toHaveCount(0)
    await expect(chip(page, "JIRA-456")).toHaveText("Canceling")
  })

  test("should refuse a Safe Push that conflicts with the newest target, nothing pushed or started, with Update Project", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    const pushes = await routePush(page, {})
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const starts = await routeCommand(page, "start_patronus_robot", () => STARTED)
    // the target moved after the last sync with a change of the lines a commit of the branch changes
    const conflict: TargetCheck = {
      baseline: "origin/master",
      fetchError: null,
      outcome: {
        kind: "conflict",
        targetTip: "faebba53ecd77000",
        commit: { id: "e90c2bf606405000", author: "Test User", authorTime: 1, subject: "Change the palette popup" },
        files: ["src/PalettePopup.kt"],
        fileCount: 1,
        targetCommits: [{ id: "6fddca13ab01cd02", author: "Alice", authorTime: 2, subject: "IJAI-4233 Change the popup" }],
      },
    }
    const checks = await routeCommand(page, "check_target_conflict", () => conflict)
    const update = await holdWithProgress(page, "update_project")

    await safePushFromMenu(page, "JIRA-456")
    // the error of a failed start, with the way out: Patronus would fail the robot before any check
    const refused = toastWith(page, "Cannot start the Safe Push")
    await expect(refused.locator("[data-slot=toast-description]")).toHaveText(
      "Patronus can't rebase JIRA-456 onto origin/master: e90c2bf6 (Change the palette popup) conflicts in src/PalettePopup.kt, which origin/master changed after the base of JIRA-456: 6fddca13 (Alice: IJAI-4233 Change the popup). Update Project resolves it locally; then start again.",
    )
    expect(checks).toEqual([expect.objectContaining({ branchName: "JIRA-456", baselineBranch: "origin/master" })])
    expect({ pushes, starts }).toEqual({ pushes: [], starts: [] })
    await expect(slotSafePush(page, "JIRA-456")).toHaveText("Safe Push")

    await refused.getByRole("button", { name: "Update Project" }).click()
    await expect(page.getByTestId("update-project-button")).toHaveAttribute("aria-busy", "true")
    update.release()
  })

  test("should start a Safe Push with the profile chosen in the popover of S and of Safe Push With Profile…", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await routePush(page, {})
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const starts = await routeCommand(page, "start_patronus_robot", () => STARTED)

    // S always opens the popover, never a start by itself: what lands, the default checks
    let popover = await openSafePushWithKey(page, "JIRA-456")
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popover.getByTestId("safe-push-lands")).toHaveText(/^Lands JIRA-456 into master: \d+ commits?\.$/)
    await expect(popover.getByTestId("safe-push-profile")).toContainText("Default checks")
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)

    popover = await openSafePushWithProfile(page, "JIRA-456")
    await popover.getByTestId("safe-push-profile").click()
    // a profile that allows only dry runs is not offered for a Safe Push
    await expect(page.getByRole("option", { name: "Large Scale Change" })).toBeVisible()
    await expect(page.getByRole("option", { name: "Dry Only" })).toHaveCount(0)
    await page.keyboard.press("Escape")
    await chooseProfile(page, "Large Scale Change")
    await expect(popover.getByTestId("safe-push-profile-description")).toHaveText("All the tests")
    await popoverStart(page).click()
    // a start closes the popover
    await expect(popover).toHaveCount(0)

    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash", profileId: "lsc" })])
    await expect(toastWith(page, "Safe Push started").locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master · profile Large Scale Change")
  })

  test("should open the popover of a click for the profile of the last Safe Push, and start with the one chosen", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, FAILED)
    await routePush(page, {})
    // the last robot of ABC-123 ran with Large Scale Change: its next start runs with it unless another is chosen
    const options = await routeCommand(page, "get_patronus_start_options", () => ({ ...START_OPTIONS, defaultProfileId: "lsc" }))
    const restarts = await routeCommand(page, "restart_patronus_robot", () => robot("ABC-123", "restarted-robot"))
    const starts = await routeCommand(page, "start_patronus_robot", () => robot("ABC-123", "started-robot", { sourceRevision: null }))

    await findBranchRow(page, "ABC-123").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Safe Push S", exact: true }).click()
    // another profile than the default checks: the click says so first
    const popover = safePushPopover(page)
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popover.getByTestId("safe-push-profile")).toContainText("Large Scale Change")
    await expect(popover.getByTestId("safe-push-profile-source")).toHaveText("The profile of the last Safe Push of ABC-123.")
    expect(options[0]).toMatchObject({ lastRobotId: "failed-robot" })
    await chooseProfile(page, "Default checks")
    await expect(popover.getByTestId("safe-push-profile-source")).toHaveCount(0)
    await popoverStart(page).click()

    await expect(toastWith(page, "Safe Push started").locator("[data-slot=toast-description]")).toHaveText("ABC-123 → master")
    expect(starts).toEqual([expect.objectContaining({ branchName: "ABC-123", profileId: null })])
    expect(restarts).toEqual([])
  })

  test("should start a branch whose remote branch has its commits under other hashes without a push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    // the remote branch has the commits of JIRA-456 by content, at another commit: the sync rebuilt them on a newer baseline
    await pushBranchFromRow(page, "JIRA-456")
    const pushes = await routePush(page, {})
    const starts = await routeCommand(page, "start_patronus_robot", () => STARTED)
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)

    // Patronus rebases the remote branch itself: a push would only replace it with copies of the same commits
    await slotSafePush(page, "JIRA-456").click()

    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash" })])
    expect(pushes).toEqual([])
  })

  test('should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible', async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const starts = await setupStarts(page, setupRepo, syncAndWaitForBranches, RUNNING)

    // a robot runs on the commit JIRA-456 is at: the slot is the robot, the menu says why a start is not possible
    await expect(chip(page, "JIRA-456")).toHaveAttribute("data-kind", "running")
    await expect(findBranchRow(page, "JIRA-456").getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
    await expect(slotSafePush(page, "JIRA-456")).toHaveCount(0)

    const button = slotSafePush(page, "ABC-123")
    await expect(button).toHaveText("Safe Push")
    await button.hover()
    // the hover says what it lands, and S starts it from the focused row
    const description = page.getByRole("tooltip").filter({ hasText: "Lands ABC-123" })
    await expect(description).toHaveText("Lands ABC-123 into master once the checks passS")
    await expect(description.locator("kbd")).toHaveText("S")
    await button.click()
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(starts).toEqual([expect.objectContaining({ branchName: "ABC-123" })])

    // "⋯" has the menu of the right-click
    const featureRow = findBranchRow(page, "feature-auth")
    await featureRow.getByRole("button", { name: "More actions for feature-auth" }).click()
    const rowMenu = page.getByRole("menu", { name: "More actions for feature-auth" })
    await expect(rowMenu.getByRole("menuitem", { name: "Copy Branch Name" })).toBeVisible()
    // the approvals go into the GitHub pull request: no Space merge request is created (deprecated)
    await expect(rowMenu.getByRole("menuitem", { name: "Create Space Merge Request…" })).toHaveCount(0)
    await rowMenu.getByRole("menuitem", { name: "Safe Push S", exact: true }).click()
    await expect(chip(page, "feature-auth")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(starts.map(start => start.branchName)).toEqual(["ABC-123", "feature-auth"])

    // the menu of the branch with the running robot says why
    await findBranchRow(page, "JIRA-456").getByRole("button", { name: "More actions for JIRA-456" }).click()
    const menu = page.getByRole("menu", { name: "More actions for JIRA-456" })
    const menuItem = menu.getByRole("menuitem", { name: "Safe Push S", exact: true })
    await expect(menuItem).toBeDisabled()
    await menuItem.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "is running" })).toHaveText("A Safe Push of the last commit is running")
    // Escape closes the menu with its description
    await page.keyboard.press("Escape")
    await expect(page.getByRole("menu")).toHaveCount(0)
  })

  // app bug: `requestSafePush` reveals the row (`requestReveal`), and the reveal expands it
  test("should start a Safe Push from the row button without toggling the row", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await routePush(page, {})
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    await routeCommand(page, "start_patronus_robot", () => robot("ABC-123", "started-robot", { sourceRevision: null }))
    const row = findBranchRow(page, "ABC-123")
    await verifyBranchState(row, "closed")
    await slotSafePush(page, "ABC-123").click()
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)
    // the button starts, it doesn't toggle the row
    await verifyBranchState(row, "closed")
  })

  test("should open the Safe Push popover from the branch preview with S, and start from its footer button", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const starts = await setupStarts(page, setupRepo, syncAndWaitForBranches, RUNNING)

    // a robot runs on the commit JIRA-456 is at: the button is disabled, S says why and the preview stays
    let preview = await openBranchPreview(page, "JIRA-456")
    await expect(preview.getByTestId("safe-push-button")).toBeDisabled()
    await page.keyboard.press("s")
    const refused = toastWith(page, "Safe Push of JIRA-456 can't start")
    await expect(refused).toContainText("A Safe Push of the last commit is running")
    await expect(preview).toBeVisible()
    await expect(safePushPopover(page)).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)
    // the robot it names opens from it; the focus goes back to the table
    await refused.getByRole("button", { name: "Open in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual([RUNNING.url])
    await findBranchRow(page, "ABC-123").focus()

    preview = await openBranchPreview(page, "ABC-123")
    await expect(preview.getByTestId("safe-push-button").locator("kbd")).toHaveText("S")
    await page.keyboard.press("s")
    // the popover is the row's: the preview closes for it
    await expect(preview).toHaveCount(0)
    const popover = safePushPopover(page)
    await expect(popover.getByTestId("safe-push-lands")).toHaveText(/^Lands ABC-123 into master: \d+ commits?\.$/)
    // its start takes the focus: Enter starts
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(popover).toHaveCount(0)
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)

    // the footer button is a click: the preview closes, and nothing to say starts it at once
    preview = await openBranchPreview(page, "feature-auth")
    await preview.getByTestId("safe-push-button").click()
    await expect(preview).toHaveCount(0)
    await expect(chip(page, "feature-auth")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(starts.map(start => start.branchName)).toEqual(["ABC-123", "feature-auth"])
  })

  test("should cancel the robot for a new Safe Push from the popover, and not restart it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the robot runs on an older commit, so a start runs on the branch as it is
    await setupRobots(page, setupRepo, syncAndWaitForBranches, OLDER)
    const { order, starts } = await recordReplacingStart(page, OLDER)

    await safePushFromMenu(page, "JIRA-456")
    // the click says first that its start replaces the running robot
    const popover = safePushPopover(page)
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popover.getByTestId("safe-push-replaces")).toHaveText("Cancels the Safe Push of JIRA-456 running on older-co.")
    expect(order).toEqual([])
    await popoverStart(page).click()

    // and asks nothing more: no restart of the old one
    await expect.poll(() => order).toEqual(["cancel", "push", "start"])
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash" })])
    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
  })

  test("should cancel the robot it replaces without a push when the remote branch has the commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a Safe Push of an older commit runs; the remote branch has the commits of JIRA-456, rebuilt since on a newer baseline
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === "JIRA-456" ? { ...status, remoteExists: true, remoteTip: "JIRA-456-pushed", unpushedCommits: [], myUnpushedCount: 0, fastForward: false } : status,
    )
    await setupRobots(page, setupRepo, syncAndWaitForBranches, OLDER)
    const { order, starts } = await recordReplacingStart(page, OLDER)

    await safePushFromMenu(page, "JIRA-456")
    await expect(safePushPopover(page).getByTestId("safe-push-replaces")).toHaveText("Cancels the Safe Push of JIRA-456 running on older-co.")
    await popoverStart(page).click()
    // no push: the start cancels the robot itself, and never restarts it
    await expect.poll(() => order).toEqual(["cancel", "start"])
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "rebaseAutosquash" })])
  })

  test("should wait to start until the remote status of the branch is loaded", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the first sync brings no remote status of JIRA-456
    await rewriteNextRemoteStatus(page, status => (status.branchName === "JIRA-456" ? null : status))
    await setupRobots(page, setupRepo, syncAndWaitForBranches)
    await routePush(page, {})
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    const starts = await routeCommand(page, "start_patronus_robot", () => STARTED)

    const popover = await openSafePushWithKey(page, "JIRA-456")
    // nothing is claimed about the remote while it is unknown, and the start waits
    const status = popover.getByTestId("safe-push-status")
    await expect(status).toHaveAttribute("data-kind", "wait")
    await expect(status).toHaveText("Checking JIRA-456…")
    await expect(popoverStart(page)).toBeDisabled()
    // Enter on the popover starts nothing yet
    await page.keyboard.press("Enter")

    await syncFromMenu(page)
    await expect(status).toHaveCount(0)
    await expect(popoverStart(page)).toBeEnabled()
    // the start enabled later takes the focus: Enter starts it now, once
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(starts).toHaveLength(1)
  })

  test("should restart a failed robot and cancel a running one from the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED)
    const restarts = await routeCommand(page, "restart_patronus_robot", () => robot("ABC-123", "restarted-robot"))
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => ({ ...RUNNING, status: "canceling" }))

    let popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Cancel Safe Push" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Restart Safe Push" }).click()
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(restarts).toEqual([expect.objectContaining({ robotId: "failed-robot" })])
    await page.keyboard.press("Escape")

    popover = await hoverStatusChip(page, chip(page, "JIRA-456"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Restart Safe Push" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Cancel Safe Push" }).click()
    await expect(chip(page, "JIRA-456")).toHaveText("Canceling")
    expect(cancels).toEqual([expect.objectContaining({ robotId: "running-robot" })])

    // each toast names the branch and the target, and opens its robot
    const restarted = page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })
    await expect(restarted.locator("[data-slot=toast-description]")).toHaveText("ABC-123 → master")
    await expect(restarted.getByRole("button", { name: "Open in Patronus" })).toHaveCount(1)
    const canceling = page.locator("[data-slot=toast]", { hasText: "Safe Push is canceling" })
    await expect(canceling.locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master")
  })

  test("should push and restart a failed robot whose branch changed since, from the branch preview, landing from the click", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // failed on an older commit; the fix is not on the remote yet
    const older = robot("ABC-123", "failed-robot", { status: "failure", sourceRevision: "older-commit", finishTime: 1704142800 })
    await setupRobots(page, setupRepo, syncAndWaitForBranches, older)
    const order: string[] = []
    await routeCommand(page, "push_branch", params => {
      order.push("push")
      return pushOutcome(params.branchName as string)
    })
    await routeCommand(page, "restart_patronus_robot", params => {
      order.push(`restart ${params.robotId as string}`)
      return robot("ABC-123", "restarted-robot")
    })

    // the row offers a new Safe Push; the preview has the robot, with the restart that tests the branch as it is
    await expect(chip(page, "ABC-123")).toHaveCount(0)
    const preview = await openBranchPreview(page, "ABC-123")
    const details = preview.getByTestId("branch-preview-patronus")
    await expect(details.getByTestId("patronus-outdated")).toContainText("Restart Safe Push to test the current branch.")
    const node = getLandingNode(findBranchRow(page, "ABC-123"))
    await expect(node).toHaveAttribute("data-kind", /^(ready|yourStep)$/)
    const releasePush = await holdCommand(page, "push_branch")
    await details.getByRole("button", { name: "Restart Safe Push", exact: true }).click()
    // the restart is the move made: the branch lands from the click on, while its push runs
    await expect(node).toHaveAttribute("data-kind", "landing")
    expect(order).toEqual([])
    releasePush()

    // the restart takes the commit just pushed; it is the action, its toast the one toast
    await expect(page.locator("[data-slot=toast-title]", { hasText: "Safe Push restarted" })).toBeVisible()
    expect(order).toEqual(["push", "restart failed-robot"])
    await expect(pushedToast(page, "ABC-123")).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)
  })

  test("should show the profile, the checks and the problems in the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, FAILED)
    await routeCommand(
      page,
      "get_patronus_robot_details",
      () =>
        ({
          robot: { ...FAILED, sourceRevision: null },
          profileId: "lsc",
          profileName: "Large Scale Change",
          checks: { failed: 1, running: 0, passed: 9, canceled: 0, failedNames: ["Fast Tests"] },
          problems: [{ title: "Tests failed", subsystem: "TEAMCITY" }],
          failing: false,
          progress: null,
        }) satisfies PatronusRobotDetails,
    )

    const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByTestId("patronus-profile-name")).toHaveText("Large Scale Change")
    await expect(popover.getByTestId("patronus-checks")).toHaveText("1 failing (Fast Tests), 9 passed")
    await expect(popover.getByTestId("patronus-problems")).toHaveText("Tests failed")
  })

  /** One group must approve, without the files it reviews: Patronus doesn't say, no OWNERSHIP file names it */
  const REVIEW_REQUIRED: PatronusReviewRequirement = {
    required: true,
    mergeRequest: null,
    groups: [
      {
        name: "Air Plugin Release Team",
        reviewers: [{ fullName: "Alice", username: "alice", githubLogin: null, available: true, suggested: true }],
        paths: [],
        branchWide: false,
        pathsSource: "unknown",
      },
    ],
    skipped: false,
  }

  test.describe("code-owner review", () => {
    const MERGE_REQUEST: PatronusMergeRequest = { projectKey: "IJ", reviewKey: "IJ-CR-7", url: "https://jetbrains.team/p/ij/reviews/7" }

    test("should name the approvals a Safe Push needs on Space, and start with the merge request found open", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      const pushes = await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      // the merge request needs the approvals until a recheck finds them
      const checks = await routeReviewRequirement(page, params =>
        params.recheck === true ? { ...REVIEW_REQUIRED, required: false, groups: [], mergeRequest: MERGE_REQUEST } : { ...REVIEW_REQUIRED, mergeRequest: MERGE_REQUEST },
      )
      const starts = await routeCommand(page, "start_patronus_robot", () => STARTED)

      // on Space the robot decides on the approvals: the click says first what the merge request needs
      await slotSafePush(page, "JIRA-456").click()
      const popover = safePushPopover(page)
      await expect(popover).toHaveAttribute("data-decision", "confirm")
      await expect(popover.getByTestId("safe-push-review")).toHaveText("Needs review by the code owners: a Safe Push fails without these approvals, which count in IJ-CR-7.")
      // a line per group, with the reviewers Patronus suggests
      await expect(popover.getByTestId("safe-push-review-groups").getByRole("listitem")).toHaveText(["Air Plugin Release Team (suggested: Alice)"])
      // the merge request opens from the popover
      await popover.getByRole("button", { name: "Open IJ-CR-7" }).click()
      expect(await getOpenedUrls(page)).toContain(MERGE_REQUEST.url)
      await popover.getByRole("button", { name: "Cancel" }).click()
      await expect(popover).toHaveCount(0)

      // Check Approvals Again asks Patronus again: it finds them now
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Check Approvals Again" }).click()
      await expect.poll(() => checks.some(params => params.recheck === true)).toBe(true)
      expect(starts).toEqual([])

      // nothing to say any more: the click starts at once, with the merge request whose approvals count
      await slotSafePush(page, "JIRA-456").click()
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      // the branch isn't on the remote yet: the start pushes it first, without a toast of its own
      expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
      await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
      expect(starts).toEqual([expect.objectContaining({ mergeRequest: MERGE_REQUEST })])
    })

    test("should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeReviewRequirement(page, () => REVIEW_REQUIRED)

      await slotSafePush(page, "JIRA-456").click()
      const popover = safePushPopover(page)
      // the line names the system the approvals count in, and how to make it known; nothing is created
      await expect(popover.getByTestId("safe-push-review")).toHaveText(
        "Needs review by the code owners: a Safe Push fails without these approvals. They count in the GitHub pull request of the branch, and its GitHub repository is unknown.",
      )
      await expect(popover.getByRole("button", { name: /Merge Request/ })).toHaveCount(0)
      await popover.getByRole("button", { name: "Set GitHub Repository…" }).click()
      await expectSettingsOpened(page, { page: "repository", focus: "githubRepository" })
      // Review Changes opens the branch in its review window, the groups as its filters
      await popover.getByTestId("safe-push-review-changes").click()
      await expect.poll(async () => (await getOpenedSubWindows(page)).map(window => window.windowId)).toEqual(["settings", "branch-review"])
    })

    test("should not check the review of a dry run", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const checks = await routeReviewRequirement(page, () => REVIEW_REQUIRED)
      const starts = await routeCommand(page, "start_patronus_robot", params =>
        robot(params.branchName as string, `${params.branchName as string}-dry-run`, { sourceRevision: null, pushMode: "dryRun" }),
      )

      // "Dry Run" of the menu: the code owners don't check it, nothing to say, it starts
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Dry Run", exact: true }).click()
      await expect(toastWith(page, "Dry Run started").locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master")
      expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", mode: "dryRun", profileId: null })])
      expect(checks).toHaveLength(0)
      await expect(safePushPopover(page)).toHaveCount(0)

      // "Dry Run" of the Safe Push popover: it closes, and the dry run starts as the menu's does
      const popover = await openSafePushWithKey(page, "ABC-123")
      await popover.getByTestId("safe-push-dry-run").click()
      await expect(popover).toHaveCount(0)
      await expect(toastWith(page, "Dry Run started").filter({ hasText: "ABC-123 → master" })).toBeVisible()
      expect(starts.map(start => [start.branchName, start.mode])).toEqual([
        ["JIRA-456", "dryRun"],
        ["ABC-123", "dryRun"],
      ])
    })

    test("should open the pull request a robot that failed for missing approvals was started from, from its details", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await routeCommand(page, "get_patronus_robot_problems", () => [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }])
      const pullRequestUrl = "https://github.com/acme/monorepo/pull/7"
      await setupStarts(page, setupRepo, syncAndWaitForBranches, { ...FAILED, pullRequest: { number: 7, url: pullRequestUrl } })
      await expect(chip(page, "ABC-123")).toHaveText("Needs review")
      const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
      const text = popover.getByTestId("patronus-needs-review")
      await expect(text).toHaveText(
        "The code owners did not approve the changes in pull request #7. Restart once they approve: the restart counts the approvals the pull request has by then.",
      )
      await text.getByRole("button", { name: "#7" }).click()
      expect(await getOpenedUrls(page)).toEqual([pullRequestUrl])
    })

    test('should say "Needs review" for a robot that failed for missing approvals, and start a new Safe Push from it', async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await routeCommand(page, "get_patronus_robot_problems", () => [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }])
      const starts = await setupStarts(page, setupRepo, syncAndWaitForBranches, FAILED)
      await expect(chip(page, "ABC-123")).toHaveText("Needs review")
      await expect(chip(page, "ABC-123")).toHaveAttribute("data-color", "warning")

      const popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
      // the approvals count in the GitHub pull request: no Space merge request (deprecated)
      await expect(popover.getByTestId("patronus-needs-review")).toHaveText(
        "The code owners did not approve the changes. Get their approvals in the pull request of the branch, then start a new Safe Push from it: a restart has no pull request to count them in.",
      )
      await expect(popover.getByRole("button", { name: "Restart Safe Push" })).toHaveCount(0)
      await popover.getByRole("button", { name: "Safe Push", exact: true }).click()
      await expect(toastWith(page, "Safe Push started").locator("[data-slot=toast-description]")).toHaveText("ABC-123 → master")
      expect(starts).toEqual([expect.objectContaining({ branchName: "ABC-123", mode: "rebaseAutosquash" })])
    })

    test("should close the branch preview for the Safe Push of its Patronus section", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await routeCommand(page, "get_patronus_robot_problems", () => [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }])
      const starts = await setupStarts(page, setupRepo, syncAndWaitForBranches, FAILED)
      await expect(chip(page, "ABC-123")).toHaveText("Needs review")

      const preview = await openBranchPreview(page, "ABC-123")
      await preview.getByTestId("branch-preview-patronus").getByRole("button", { name: "Safe Push", exact: true }).click()
      // the start is the row's: the modal preview closes first, its progress shows in the row
      await expect(preview).toHaveCount(0)
      await expect(toastWith(page, "Safe Push started").locator("[data-slot=toast-description]")).toHaveText("ABC-123 → master")
      expect(starts).toEqual([expect.objectContaining({ branchName: "ABC-123" })])
    })
  })

  test.describe("branches reviewed in GitHub pull requests", () => {
    const NO_REVIEW: PatronusReviewRequirement = { required: false, mergeRequest: null, groups: [], skipped: false }

    /** issue_links mirrored to the GitHub repository `acme/monorepo`, with these pull requests */
    async function setupGitHub(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
      ...pullRequests: PullRequest[]
    ): Promise<TestRepositoryBuilder> {
      const repo = await setupRepo("issue_links", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests(...pullRequests) })
      await syncAndWaitForBranches()
      await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
      for (const it of pullRequests) {
        await expect(findBranchRow(page, it.headRefName.split("/").pop()!).getByTestId("pull-request-chip")).toBeVisible()
      }
      return repo
    }

    /** JIRA-456 on GitHub without a pull request, the code owners required: the pushes, a start and a creation answered */
    async function setupRequiredReview(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
    ) {
      await routeReviewRequirement(page, () => REVIEW_REQUIRED)
      await setupGitHub(page, setupRepo, syncAndWaitForBranches)
      const pushes = await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const starts = await routeCommand(page, "start_patronus_robot", () => STARTED)
      await routeNewPullRequest(page)
      return { pushes, starts }
    }

    /** JIRA-456 with the open pull request #7 and no review needed; its push is answered in the page */
    async function setupPullRequestStart(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
      patch: Partial<PullRequest> = {},
    ): Promise<void> {
      await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, patch))
      await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeReviewRequirement(page, () => NO_REVIEW)
    }

    /**
     * Runs the fake clock over the app's wait before it reads the bot's reply to the posted comment
     * (`COMMAND_REPLY_INTERVAL_MS`, 3 s, in src/stores/patronus.ts) until it read it, instead of waiting it out
     */
    async function skipReplyWait(page: Page, posts: unknown[], replies: unknown[]) {
      await expect.poll(() => posts.length).toBe(1)
      // the wait starts once the app has the posted comment, so a step can come before it: step until the read
      await expect
        .poll(async () => {
          await page.clock.fastForward(3000)
          return replies.length
        })
        .toBeGreaterThan(0)
    }

    test("should start a Safe Push from the pull request with a /patronus merge comment", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches)
      const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "4402154133", url: "https://github.com/acme/monorepo/pull/7#issuecomment-4402154133" }))
      const replies = await routeCommand(page, "read_patronus_command_reply", () => ({ status: "accepted" }))
      const started = robot("JIRA-456", "pr-robot", {
        sourceRevision: null,
        startTime: Math.floor(Date.now() / 1000),
        pullRequest: { number: 7, url: "https://github.com/acme/monorepo/pull/7" },
      })
      // the robot the comment started is in the list asked for after the 👍
      const lists = await routeCommand(page, "get_patronus_robots", params => (params.since === null ? availableRobots() : availableRobots(started)))
      const rest = await routeCommand(page, "start_patronus_robot", () => started)

      // a comment runs the default checks of the target: no profile to choose
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      const withProfile = page.getByRole("menuitem", { name: "Safe Push With Profile…" })
      await expect(withProfile).toBeDisabled()
      await withProfile.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "comment" })).toHaveText("A comment on #7 runs the default checks: it can't choose a profile")
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)

      const popover = await openSafePushWithKey(page, "JIRA-456")
      await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #7: Patronus reports there and closes it once merged.")
      await expect(popover.getByTestId("safe-push-review")).toHaveText("No code-owner review needed.")
      await expect(popover.getByTestId("safe-push-profile-field")).toHaveCount(0)
      await popoverStart(page).click()
      await skipReplyWait(page, posts, replies)

      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      expect(posts).toEqual([expect.objectContaining({ pullRequestNumber: 7, mode: "rebaseAutosquash" })])
      expect(replies).toEqual([expect.objectContaining({ pullRequestNumber: 7, commentId: "4402154133" })])
      expect(lists.at(-1)?.since).not.toBeNull()
      expect(rest).toHaveLength(0)

      const robotPopover = await hoverStatusChip(page, chip(page, "JIRA-456"), "patronus-popover")
      await robotPopover.getByRole("button", { name: "Open Pull Request #7" }).click()
      expect(await getOpenedUrls(page)).toContain("https://github.com/acme/monorepo/pull/7")
    })

    test("should say why Patronus refused the comment", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches)
      const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "1", url: "u" }))
      const replies = await routeCommand(page, "read_patronus_command_reply", () => ({
        status: "rejected",
        reason: "Patronus did not start the robot due to an error. A robot is already running for the branch.",
      }))

      // nothing to say: the click comments at once
      await slotSafePush(page, "JIRA-456").click()
      await skipReplyWait(page, posts, replies)
      const error = toastWith(page, "Cannot start the Safe Push")
      await expect(error).toContainText("A robot is already running for the branch.")
      await expect(chip(page, "JIRA-456")).toHaveCount(0)
    })

    test("should warn before a push that dismisses the approvals of the pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches, { reviewers: [reviewer("alice", "approved")] })
      const rules = await routeCommand(page, "get_branch_rules", () => ({ dismissStaleReviewsOnPush: true }))
      const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "1", url: "u" }))

      // the approvals of alice don't count for Patronus, the push dismisses them: the click says so first
      await slotSafePush(page, "JIRA-456").click()
      const popover = safePushPopover(page)
      await expect(popover).toHaveAttribute("data-decision", "confirm")
      await expect(popover.getByTestId("safe-push-dismisses")).toHaveText(
        "The push dismisses the approvals of #7: the rules of the repository dismiss stale reviews when new commits are pushed.",
      )
      expect(rules).toEqual([expect.objectContaining({ branch: "master" })])
      expect(posts).toEqual([])
      // a warning, not a refusal: the start comments
      await popoverStart(page).click()
      await expect.poll(() => posts).toEqual([expect.objectContaining({ pullRequestNumber: 7 })])
    })

    test("should wait for the rules before a push into an approved pull request: Enter must not dismiss its approvals", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches, { reviewers: [reviewer("alice", "approved")] })
      const rules = await routeHeld(page, "get_branch_rules", { dismissStaleReviewsOnPush: false })
      const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "1", url: "u" }))

      const popover = await openSafePushWithKey(page, "JIRA-456")
      const status = popover.getByTestId("safe-push-status")
      await expect(status).toHaveAttribute("data-kind", "wait")
      await expect(status).toHaveText("Checking whether the push keeps the approvals of #7…")
      await expect(popoverStart(page)).toBeDisabled()
      // Enter while the rules load pushes nothing
      await page.keyboard.press("Enter")
      rules.release()
      // the rules keep the approvals: the start, and no warning
      await expect(popoverStart(page)).toBeEnabled()
      await expect(status).toHaveCount(0)
      await expect(popover.getByTestId("safe-push-dismisses")).toHaveCount(0)
      await expect(popoverStart(page)).toBeFocused()
      await page.keyboard.press("Enter")
      // only the Enter after the rules started it
      await expect.poll(() => posts).toEqual([expect.objectContaining({ pullRequestNumber: 7 })])
    })

    test.describe("a push that dismisses the approvals Patronus counts", () => {
      /** #7 approved by the code owner Patronus counts, and the rules dismiss stale reviews; JIRA-456 has commits the remote lacks */
      async function setupApproved(page: Page, setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>, syncAndWaitForBranches: () => Promise<unknown>) {
        const approved = pullRequest("JIRA-456", 7, { reviewers: [reviewer("ikorennoy", "approved")] })
        const repo = await setupRepo("issue_links", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests(approved) })
        await repo.setReviewAnswers({ answers: { "JIRA-456": APPROVED_BY_CODE_OWNER } })
        await syncAndWaitForBranches()
        await expect(findBranchRow(page, "JIRA-456").getByTestId("pull-request-chip")).toBeVisible()
        await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
        await routeCommand(page, "get_branch_rules", () => ({ dismissStaleReviewsOnPush: true }))
        const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "1", url: "u" }))
        const pushes = await routePush(page, {})
        return { posts, pushes }
      }

      test("should refuse the click with the push alone in its toast", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        const { posts, pushes } = await setupApproved(page, setupRepo, syncAndWaitForBranches)
        await slotSafePush(page, "JIRA-456").click()
        // the robot would start without the approvals and fail: no start, the push is the step before it
        const refused = toastWith(page, "Safe Push of JIRA-456 can't start")
        await expect(refused).toContainText("The push dismisses the approvals of #7: push first, then ask for the review again")
        await refused.getByRole("button", { name: "Push", exact: true }).click()
        await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
        expect(posts).toEqual([])
      })

      test("should say in the popover why it can't start, and push alone from it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        const { posts, pushes } = await setupApproved(page, setupRepo, syncAndWaitForBranches)
        const popover = await openSafePushWithKey(page, "JIRA-456")
        const status = popover.getByTestId("safe-push-status")
        await expect(status).toHaveAttribute("data-kind", "refuse")
        await expect(status).toHaveText("The push dismisses the approvals of #7: push first, then ask for the review again")
        await expect(popoverStart(page)).toBeDisabled()
        await popover.getByTestId("safe-push-push-only").click()
        await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
        expect(posts).toEqual([])
      })
    })

    test("should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, { isDraft: true, reviewers: [reviewer("alice", "approved")] }))
      await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "1", url: "u" }))

      // the state as the label, quiet, opening the pull request it waits in
      const step = findBranchRow(page, "JIRA-456").getByTestId("next-step")
      await expect(findBranchRow(page, "JIRA-456").getByTestId("next-step-slot")).toHaveAttribute("data-step", "draft")
      await expect(step).toHaveText("Draft")
      await expect(step).toHaveAttribute("data-tone", "quiet")
      await step.hover()
      await expect(page.getByTestId("next-step-hover")).toHaveText("#7 is a draft: Patronus merges it once it is ready for review. Open Pull Request #7O")
      await step.click()
      expect(await getOpenedUrls(page)).toEqual([pullRequest("JIRA-456", 7).url])

      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      const menu = page.getByRole("menu")
      for (const name of ["Safe Push S", "Safe Push With Profile…"]) {
        await expect(menu.getByRole("menuitem", { name, exact: true })).toBeDisabled()
      }
      await menu.getByRole("menuitem", { name: "Safe Push S", exact: true }).hover()
      await expect(page.getByRole("tooltip")).toHaveText("#7 is a draft: Patronus merges it once it is ready for review")
      // a dry run pushes nothing into the target: a draft doesn't hold it back, it comments on the pull request
      await menu.getByRole("menuitem", { name: "Dry Run", exact: true }).click()
      await expect.poll(() => posts).toEqual([expect.objectContaining({ pullRequestNumber: 7, mode: "dryRun" })])
    })

    test("should offer no Safe Push of a branch whose pull request merged", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, { state: "merged" }))

      await expect(findBranchRow(page, "JIRA-456").getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      const menu = page.getByRole("menu")
      for (const name of ["Safe Push S", "Safe Push With Profile…", "Dry Run"]) {
        await expect(menu.getByRole("menuitem", { name, exact: true })).toBeDisabled()
      }
      await menu.getByRole("menuitem", { name: "Safe Push S", exact: true }).hover()
      await expect(page.getByRole("tooltip")).toHaveText("Pull request #7 is merged")
    })

    test("should offer to create the pull request the approvals count in, and no start Patronus would fail", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { pushes, starts } = await setupRequiredReview(page, setupRepo, syncAndWaitForBranches)

      // not pushed yet: Patronus judges its local commits; it needs the code owners, whose approvals count in a pull request
      // the dialog opens at once: its Create pushes the branch, a dialog closed without one pushes nothing
      const dialog = await openCreatePullRequestFromSlot(page)
      await expect(dialog).toBeVisible()
      await expect(dialog.getByTestId("create-pull-request-submit")).toHaveText(/^Create Pull Request/)
      // Patronus names the code owner without a GitHub login: the user finds the person
      const suggestions = dialog.getByTestId("create-pull-request-suggestions")
      await expect(suggestions.getByRole("button")).toHaveText(["Alice"])
      const searches = await routeCommand(page, "search_github_users", () => [{ login: "alice-gh", name: "Alice" }])
      await suggestions.getByRole("button", { name: "Alice" }).click()
      await expect(dialog.getByTestId("create-pull-request-reviewer-search")).toHaveValue("Alice")
      await page.getByRole("option", { name: /alice-gh/ }).click()
      expect(searches.at(-1)).toEqual(expect.objectContaining({ query: "Alice" }))
      await page.keyboard.press("Escape")
      await expect(dialog).toHaveCount(0)
      expect(await getOpenedUrls(page)).toEqual([])
      expect(pushes).toEqual([])

      // Patronus would fail the robot at once: the menu says why, S says it in a toast, nothing starts
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      const menuItem = page.getByRole("menu").getByRole("menuitem", { name: "Safe Push S", exact: true })
      await expect(menuItem).toBeDisabled()
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)
      await findBranchRow(page, "JIRA-456").focus()
      await page.keyboard.press("s")
      await expect(toastWith(page, "Safe Push of JIRA-456 needs a pull request")).toContainText("Code owners must approve in it: Air Plugin Release Team")
      await expect(safePushPopover(page)).toHaveCount(0)
      expect(starts).toEqual([])
    })

    test("should open the Safe Push popover without review from the pull request dialog", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { starts } = await setupRequiredReview(page, setupRepo, syncAndWaitForBranches)

      const dialog = await openCreatePullRequestFromSlot(page)
      await dialog.getByRole("button", { name: "Safe Push Without Review…" }).click()
      await expect(dialog).toHaveCount(0)
      // the popover of the row, without review: the profile has to be chosen
      const popover = await expectPopoverWithoutReview(page, "Air Plugin Release Team")
      const start = popoverStart(page)
      await expect(start).toHaveText("Safe Push Without Review")
      await expect(start).toBeDisabled()

      await chooseProfile(page, "Large Scale Change")
      await expect(popover.getByTestId("safe-push-profile-note")).toHaveText(
        "Patronus tells only when the robot starts whether Large Scale Change skips the code-owner review: if it doesn't, the Safe Push fails at once.",
      )
      await expect(start).toBeEnabled()
      await start.click()
      await expect(popover).toHaveCount(0)
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", profileId: "lsc" })])
    })

    test.describe("the pull request the approvals count in", () => {
      const CODE_OWNERS: PatronusReviewRequirement = {
        ...REVIEW_REQUIRED,
        groups: [{ ...REVIEW_REQUIRED.groups[0]!, reviewers: [{ fullName: "Alice", username: "alice", githubLogin: "alice-gh", available: true, suggested: true }] }],
      }

      test("should ask the code owner Patronus names, and the row then awaits the review in the pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        const repo = await setupRepo("issue_links", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests() })
        await repo.setReviewAnswers({ answers: { "JIRA-456": CODE_OWNERS } })
        await syncAndWaitForBranches()
        const pushes = await routePush(page, {})
        await routeNewPullRequest(page)
        await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
        const created = pullRequest("JIRA-456", 7, { reviewers: [reviewer("alice-gh", "requested")] })
        const creates = await routeCommand(page, "create_pull_request", () => createdPullRequest(created))

        const dialog = await openCreatePullRequestFromSlot(page)
        const slot = findBranchRow(page, "JIRA-456").getByTestId("next-step-slot")
        // one suggested reviewer of each group is chosen at once
        await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["alice-gh"])
        await dialog.getByTestId("create-pull-request-submit").click()
        await expect(dialog).toHaveCount(0)
        // pushed first: GitHub opens a pull request of a branch it has
        expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
        expect(creates).toEqual([expect.objectContaining({ branchName: "JIRA-456", reviewers: ["alice-gh"], assignSelf: true, waitForMirror: true })])
        // the approvals count in it: the row waits for the code owner asked there, whom its hover names
        await expect(slot).toHaveAttribute("data-step", "awaiting")
        await expect(slot.getByTestId("next-step")).toHaveText("Awaiting Review")
        await slot.getByTestId("next-step").hover()
        await expect(page.getByTestId("next-step-hover")).toContainText("Awaiting Alice")
      })

      test("should open GitHub's form without a token GitHub accepts", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        await setupRepo("issue_links", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: unavailablePullRequests({ status: "signedOut" }) })
        await syncAndWaitForBranches()
        await routePush(page, {})
        await routeNewPullRequest(page, { viewerLogin: null })
        await pushFromRowMenu(page, "JIRA-456")
        await pushedToast(page, "JIRA-456").getByRole("button", { name: "Create Pull Request…" }).click()
        const dialog = createPullRequestDialog(page)
        await expect(dialog.getByTestId("create-pull-request-unavailable")).toContainText("No GitHub token to see pull requests: GitHub's form opens the pull request instead.")
        await expect(dialog.getByTestId("create-pull-request-submit")).toHaveCount(0)
        await expect(dialog.getByTestId("create-pull-request-reviewers")).toHaveCount(0)
        await expect(dialog.getByTestId("create-pull-request-assign")).toHaveCount(0)
        await dialog.getByTestId("create-pull-request-on-github").click()
        await expect(dialog).toHaveCount(0)
        expect(await getOpenedUrls(page)).toEqual(["https://github.com/acme/monorepo/compare/master...user-name/virtual/JIRA-456?expand=1&title=Add+auth&body=-+**Add+login**"])
      })
    })
  })

  test.describe("pushing into a branch whose Safe Push runs", () => {
    test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // the robot runs on an older commit: the branch changed since, Restart (which pushes first) tests it as it is
      await setupRobots(page, setupRepo, syncAndWaitForBranches, OLDER)
    })

    test("should offer no push: it won't land, and the menu says why", async ({ page }) => {
      const pushes = await routeCommand(page, "push_branch", params => pushOutcome(params.branchName as string))
      const row = findBranchRow(page, "JIRA-456")
      // no push, as the Safe Push slot has no step; the Status cell shows the robot
      await expectNothingToPush(row)
      await expect(row.getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
      // P does nothing on the row
      await row.focus()
      await page.keyboard.press("p")

      await row.getByRole("button", { name: "More actions for JIRA-456" }).click()
      const push = page.getByRole("menu", { name: "More actions for JIRA-456" }).getByRole("menuitem", { name: "Push P" })
      await expect(push).toBeDisabled()
      await push.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "is running" })).toHaveText(
        "A Safe Push of JIRA-456 is running: wait until it merges, or restart it on the current branch",
      )
      await page.keyboard.press("Escape")
      expect(pushes).toHaveLength(0)
    })

    test("should open and restart the robot from the description of the preview's push, which says why it can't push", async ({ page }) => {
      const order = await routeCancelPushRestart(page)
      const preview = await openBranchPreview(page, "JIRA-456")
      await preview.getByTestId("push-button").hover()
      const description = page.getByTestId("push-description")
      await expect(description).toContainText("A Safe Push of JIRA-456 is running: wait until it merges, or restart it on the current branch")
      const actions = description.getByTestId("push-hold-actions")
      await actions.getByRole("button", { name: "Open in Patronus" }).click()
      expect(await getOpenedUrls(page)).toEqual([OLDER.url])
      await actions.getByRole("button", { name: "Restart Safe Push" }).click()
      await expect.poll(() => order).toEqual(["cancel", "push", "restart"])
      await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })).toBeVisible()
    })

    test("should cancel the robot, push and restart it on the pushed commit, from Restart Safe Push", async ({ page }) => {
      const order = await routeCancelPushRestart(page)

      await pushAndRestartFromMenu(page)
      // the restart is the action: one toast says it, and opens the restarted robot
      const toast = page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })
      await expect(toast).toBeVisible()
      await expect.poll(() => order).toEqual(["cancel", "push", "restart"])
      await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
      await toast.getByRole("button", { name: "Open in Patronus" }).click()
      expect(await getOpenedUrls(page)).toEqual([expect.stringContaining("restarted-robot")])
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    })

    test("should restart a robot canceled for a push the remote refused once the toast pushes", async ({ page }) => {
      const order: string[] = []
      let canceled = false
      await routeCommand(page, "get_patronus_robot_status", () => (canceled ? "canceled" : "running"))
      await routeCommand(page, "cancel_patronus_robot", () => {
        order.push("cancel")
        canceled = true
        return { ...OLDER, status: "canceling" }
      })
      await routeCommand(page, "restart_patronus_robot", () => {
        order.push("restart")
        return robot("JIRA-456", "restarted-robot", { name: "Restarted" })
      })
      const pushes = await routePush(page, { "JIRA-456": REMOTE_MOVED })
      await pushAndRestartFromMenu(page)
      const toast = page.locator("[data-slot=toast]", { hasText: "JIRA-456 changed on the remote" })
      await expect(toast).toContainText("The Safe Push is canceled: the push restarts it.")
      expect(order).toEqual(["cancel"])

      await toast.getByRole("button", { name: "Force Push" }).click()
      await confirmForcePush(page, "Force Push (3)")

      // canceled now: the restart takes the pushed commit, in the toast of the push
      await expect(pushedToast(page, "JIRA-456").locator("[data-slot=toast-description]")).toHaveText(/ · Safe Push restarted$/)
      expect(order).toEqual(["cancel", "restart"])
      expect(pushes.map(push => push.expectedRemote)).toEqual([null, "moved-tip"])
    })
  })
})
