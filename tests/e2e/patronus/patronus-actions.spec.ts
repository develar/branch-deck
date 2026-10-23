// @spec spec/branch/patronus.spec.md
import type { Locator, Page } from "@playwright/test"
import type {
  LandingAllowances,
  LandingCheck,
  LandingRefusal,
  PatronusMergeRequest,
  PatronusProfile,
  PatronusReviewRequirement,
  PatronusRobot,
  PatronusRobotDetails,
  PullRequest,
  TargetCheck,
} from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { expectNothingToPush, findBranchRow, getLandingNode, getMoreButton, getPushButton, pushFromRowMenu, verifyBranchState } from "../helpers/branch-helpers"
import {
  availableRobots,
  missingApprovals,
  robot,
  robotUrl,
  routeReviewRequirement,
  setupRobots,
  setupRobotsWith,
  startRequest,
  setupApprovalsDismissedByPush,
} from "../helpers/patronus-helpers"
import {
  availablePullRequests,
  createPullRequestDialog,
  dismissingApprovals,
  getOpenedUrls,
  gitHubRequests,
  gitHubWrites,
  pullRequest,
  repoPath,
  reviewer,
  skipCommandReplyWait,
  unavailablePullRequests,
} from "../helpers/pull-request-helpers"
import type { PushResult } from "../helpers/push-helpers"
import { confirmForcePush, pushedToast, pushOutcome, routeHeldPush, routePush } from "../helpers/push-helpers"
import { addRecentProject, OTHER_REPOSITORY, switchRepository } from "../helpers/repository-switch-helpers"
import { hoverStatusChip, openBranchPreview, pushBranchFromRow } from "../helpers/review-status-helpers"
import { chooseProfile, expectPopoverWithoutReview, openSafePushWithKey, openSafePushWithProfile, popoverStart, safePushPopover, slotSafePush } from "../helpers/safe-push-helpers"
import { closeMenus, openSubmenu } from "../helpers/selection-helpers"
import { expectSettingsOpened } from "../helpers/settings-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { rewriteNextRemoteStatus, syncFromMenu } from "../helpers/sync-helpers"
import type { PatronusLog, TestRepositoryBuilder } from "../helpers/test-repository"
import { holdCommand, holdCommandCalls, holdWithProgress, recordCommand, routeCommand, routeCommandError } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches, none pushed
const RUNNING = robot("JIRA-456", "running-robot")
const FAILED = robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })
/** A Safe Push of JIRA-456 running on a commit the branch moved past */
const OLDER = robot("JIRA-456", "running-robot", { sourceRevision: "older-commit" })

/** The merge profiles Patronus offers on master (`patronusProfiles`) */
const PROFILES: PatronusProfile[] = [
  { id: "lsc", name: "Large Scale Change", description: "All the tests", allowedPushModes: [], skipsReview: null },
  { id: "dry-only", name: "Dry Only", description: null, allowedPushModes: ["dryRun"], skipsReview: null },
]

function chip(page: Page, branchName: string) {
  return findBranchRow(page, branchName).getByTestId("patronus-chip")
}

/** The toast whose text has `text` */
function toastWith(page: Page, text: string): Locator {
  return page.locator("[data-slot=toast]", { hasText: text })
}

/** "Safe Push" of the branch menu (the right-click): a click, which starts at once when there is nothing to say */
async function safePushFromMenu(page: Page, branchName: string) {
  await findBranchRow(page, branchName).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Safe Push S", exact: true }).click()
}

/**
 * The robots of the `issue_links` template, the pushes answered in the page, the default checks: a start of a refusal
 * test. `starts` are the params of the start commands the page sent, the fake's log says what reached Patronus
 */
async function setupStart(page: Page, setupRepo: Parameters<typeof setupRobots>[1], syncAndWaitForBranches: Parameters<typeof setupRobots>[2]) {
  const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
  const pushes = await routePush(page, {})
  const starts = await recordCommand(page, "start_patronus_robot")
  return { repo, pushes, starts }
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
 * Answers the pushes of JIRA-456 in the page, each noting what the fake Patronus had done when it came (its log): the
 * order of the push and of the cancel, restart or start around it
 */
async function routePushNotingPatronus(page: Page, repo: TestRepositoryBuilder): Promise<PatronusLog[]> {
  const logs: PatronusLog[] = []
  await routeCommand(page, "push_branch", async params => {
    logs.push(await repo.getPatronusLog())
    return pushOutcome(params.branchName as string)
  })
  return logs
}

/** The robots, the cancels and the restarts of the fake's log, and the source branches of its starts */
async function patronusActions(repo: TestRepositoryBuilder) {
  const log = await repo.getPatronusLog()
  return { cancels: log.cancels, restarts: log.restarts, starts: log.starts.map(start => start.sourceBranch) }
}

/** The issue_links repository with `robots`, the pushes answered in the page, the profiles; every start goes to the fake */
async function setupStarts(
  page: Page,
  setupRepo: (template: string, options: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  ...robots: PatronusRobot[]
): Promise<TestRepositoryBuilder> {
  const repo = await setupRobotsWith(page, setupRepo, syncAndWaitForBranches, { patronusProfiles: { profiles: PROFILES } }, ...robots)
  await routePush(page, {})
  return repo
}

/** What the fake did for a start of JIRA-456 that replaces its running robot `OLDER`: canceled it, started a new one */
const REPLACED = { cancels: [OLDER.id], restarts: [], starts: [startRequest("JIRA-456").sourceBranch] }

/** The next sync finds the commits of JIRA-456 on its remote branch, rebuilt since on a newer baseline: a start pushes nothing */
async function remoteHasCommitsOfJira(page: Page): Promise<void> {
  await rewriteNextRemoteStatus(page, status =>
    status.branchName === "JIRA-456" ? { ...status, remoteExists: true, remoteTip: "JIRA-456-pushed", unpushedCommits: [], myUnpushedCount: 0, fastForward: false } : status,
  )
}

/** A Safe Push of an older commit of JIRA-456 runs; the remote branch has its commits (`remoteHasCommitsOfJira`) */
async function setupOlderWithPushedCommits(
  page: Page,
  setupRepo: (template: string, options: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
): Promise<TestRepositoryBuilder> {
  await remoteHasCommitsOfJira(page)
  return setupRobots(page, setupRepo, syncAndWaitForBranches, OLDER)
}

/** JIRA-456 shown running, after Patronus got one start of it: `patch` of the default start request */
async function expectJiraStarted(page: Page, repo: TestRepositoryBuilder, patch: Parameters<typeof startRequest>[1] = {}): Promise<void> {
  await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
  expect((await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456", patch)])
}

/** The source branches of the starts that reached the fake Patronus, in order */
async function startedBranches(repo: TestRepositoryBuilder): Promise<string[]> {
  return (await repo.getPatronusLog()).starts.map(start => start.sourceBranch.replace("refs/heads/user-name/virtual/", ""))
}

/** `setupRepo` with Patronus answering that `FAILED` failed for missing code-owner approvals */
function withMissingApprovals(setupRepo: (template: string, options: SetupRepoOptions) => Promise<TestRepositoryBuilder>) {
  return (template: string, options: SetupRepoOptions) => setupRepo(template, { ...options, patronusDetails: missingApprovals(FAILED.id) })
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
 * The restart of the running robot of JIRA-456, which pushes first: Patronus canceled it before the push, and restarted
 * it after; the push answered in the page
 */
async function expectCancelPushRestart(repo: TestRepositoryBuilder, pushes: PatronusLog[]): Promise<void> {
  await expect.poll(() => patronusActions(repo)).toEqual({ cancels: [OLDER.id], restarts: [OLDER.id], starts: [] })
  expect(pushes.map(log => [log.cancels, log.restarts])).toEqual([[[OLDER.id], []]])
}

async function pushAndRestartFromMenu(page: Page) {
  await findBranchRow(page, "JIRA-456").click({ button: "right" })
  await page.getByRole("menuitem", { name: "Restart Safe Push", exact: true }).click()
}

/** "Safe Push" of the open branch menu disabled, `reason` on its hover; then its "Patronus ›" opened */
async function expectMenuSafePushRefused(page: Page, reason: string): Promise<void> {
  const safePush = page.getByRole("menuitem", { name: "Safe Push S", exact: true })
  await expect(safePush).toBeDisabled()
  await safePush.hover()
  await expect(page.getByRole("tooltip")).toHaveText(reason)
  await openSubmenu(page, "Patronus")
}

test.describe("Patronus actions", () => {
  test("should start a Safe Push from the branch menu, pushing the branch first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
    const { pushes, release } = await routeHeldPush(page, "JIRA-456", 45)
    const options = await holdCommandCalls(page, "get_patronus_start_options")
    const starts = await holdCommandCalls(page, "start_patronus_robot")

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
    // Patronus got one start of the pushed virtual branch with the default checks
    expect((await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456")])
    expect(starts.calls).toHaveLength(1)
    // it started without the popover
    await expect(safePushPopover(page)).toHaveCount(0)

    // the toast names the branch and the target, never the ref the robot runs on, and opens the robot
    const started = toastWith(page, "Safe Push started")
    await expect(started.locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master")
    // the push is a step of the start: one action, one toast
    await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    await started.getByRole("button", { name: "Open in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual([robotUrl("started-1")])

    // a start asked by mistake costs only the robot: the toast cancels it
    await started.getByRole("button", { name: "Cancel Safe Push" }).click()
    await expect.poll(async () => (await repo.getPatronusLog()).cancels).toEqual(["started-1"])
    await expect(started).toHaveCount(0)
    await expect(chip(page, "JIRA-456")).toHaveText("Canceling")
  })

  test("should refuse a Safe Push that conflicts with the newest target, nothing pushed or started, with Update Project", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { repo, pushes, starts } = await setupStart(page, setupRepo, syncAndWaitForBranches)
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
    expect({ pushes, starts, started: (await repo.getPatronusLog()).starts }).toEqual({ pushes: [], starts: [], started: [] })
    await expect(slotSafePush(page, "JIRA-456")).toHaveText("Safe Push")

    await refused.getByRole("button", { name: "Update Project" }).click()
    await expect(page.getByTestId("update-project-button")).toHaveAttribute("aria-busy", "true")
    update.release()
  })

  test("should refuse a Safe Push whose branch uses a name another branch adds, with the restack and Start Anyway", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { repo, pushes, starts } = await setupStart(page, setupRepo, syncAndWaitForBranches)
    // the robot of 2026-10-21: a commit of the branch imports a name that a commit of ABC-123 adds, outside what lands
    const refusal: LandingRefusal = {
      kind: "missingDeclaration",
      branch: "JIRA-456",
      identifier: "Stills",
      file: "src/lane/BuildLog.kt",
      line: 7,
      usedBy: { id: "a25430833a277000", subject: "Show each scenario", branch: "JIRA-456" },
      declaredBy: { id: "ecebf00037b41000", subject: "Name lossy stills", branch: "ABC-123" },
      fix: { kind: "restack", branch: "JIRA-456", onto: "ABC-123" },
    }
    const checks = await routeCommand(page, "check_landing_gate", params =>
      (params.allow as LandingAllowances | undefined)?.allowMissing === true ? ({ refusals: [] } satisfies LandingCheck) : ({ refusals: [refusal] } satisfies LandingCheck),
    )
    await routeCommandError(page, "restack_branch", "Stacked branches are off")
    const restacks = await recordCommand(page, "restack_branch")
    const reason =
      "“Show each scenario” of JIRA-456 uses Stills (src/lane/BuildLog.kt:7), which “Name lossy stills” of ABC-123 adds: git merges it, but the build of JIRA-456 fails. Stack JIRA-456 on ABC-123 to land it too."

    // the restack the refusal offers reaches the backend; nothing was pushed or started
    await safePushFromMenu(page, "JIRA-456")
    let refused = toastWith(page, "Cannot start the Safe Push")
    await expect(refused.locator("[data-slot=toast-description]")).toHaveText(reason)
    expect(checks).toEqual([expect.objectContaining({ branchName: "JIRA-456", allow: {} })])
    expect({ pushes, starts, started: (await repo.getPatronusLog()).starts }).toEqual({ pushes: [], starts: [], started: [] })
    await expect(slotSafePush(page, "JIRA-456")).toHaveText("Safe Push")
    await refused.getByRole("button", { name: "Stack JIRA-456 on ABC-123" }).click()
    await expect.poll(() => restacks).toEqual([expect.objectContaining({ branchName: "JIRA-456", parent: "ABC-123" })])
    const restackFailed = toastWith(page, "Cannot stack JIRA-456 on ABC-123")
    await expect(restackFailed).toBeVisible()
    // the newest toast lies on top of the stack: closed, the refusal is on top again
    await restackFailed.getByRole("button", { name: "Close toast" }).click()
    await expect(restackFailed).toHaveCount(0)

    // Start Anyway starts the same start with the override of what refused it, which the start command gets too
    await safePushFromMenu(page, "JIRA-456")
    refused = toastWith(page, "Cannot start the Safe Push")
    await refused.getByRole("button", { name: "Start Anyway" }).click()
    await expect(toastWith(page, "Safe Push started")).toBeVisible()
    expect(checks.at(-1)).toMatchObject({ branchName: "JIRA-456", allow: { allowDangling: false, allowMissing: true } })
    // the start runs the gate again with the override, then Patronus starts the branch
    expect(starts).toEqual([expect.objectContaining({ branchName: "JIRA-456", allow: { allowDangling: false, allowMissing: true } })])
    expect((await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456")])
  })

  test("should start a Safe Push with the profile chosen in the popover of S and of Safe Push With Profile…", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupStarts(page, setupRepo, syncAndWaitForBranches)

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

    // Patronus got one start of the virtual branch with the profile chosen
    await expectJiraStarted(page, repo, { mergeProfileId: "lsc" })
    await expect(toastWith(page, "Safe Push started").locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master · profile Large Scale Change")
  })

  test("should open the popover of a click for the profile of the last Safe Push, and start with the one chosen", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the last robot of ABC-123 ran with Large Scale Change: its next start runs with it unless another is chosen
    const lastProfile = { id: "lsc", name: "Large Scale Change", skipsReview: null }
    const repo = await setupStarts(page, setupRepo, syncAndWaitForBranches, { ...FAILED, profile: lastProfile })
    const options = await recordCommand(page, "get_patronus_start_options")

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
    // a new robot with the default checks, not a restart of the last one
    const log = await repo.getPatronusLog()
    expect({ starts: log.starts, restarts: log.restarts }).toEqual({ starts: [startRequest("ABC-123")], restarts: [] })
  })

  test("should start a branch whose remote branch has its commits under other hashes without a push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
    // the remote branch has the commits of JIRA-456 by content, at another commit: the sync rebuilt them on a newer baseline
    await pushBranchFromRow(page, "JIRA-456")
    const pushes = await routePush(page, {})

    // Patronus rebases the remote branch itself: a push would only replace it with copies of the same commits
    await slotSafePush(page, "JIRA-456").click()

    await expectJiraStarted(page, repo)
    expect(pushes).toEqual([])
  })

  test('should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible', async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupStarts(page, setupRepo, syncAndWaitForBranches, RUNNING)

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
    expect(await startedBranches(repo)).toEqual(["ABC-123"])

    // "⋯" has the menu of the right-click
    const featureRow = findBranchRow(page, "feature-auth")
    await featureRow.getByRole("button", { name: "More actions for feature-auth" }).click()
    const rowMenu = page.getByRole("menu", { name: "More actions for feature-auth" })
    await expect(rowMenu.getByRole("menuitem", { name: "Copy", exact: true })).toBeVisible()
    // the approvals go into the GitHub pull request: no Space merge request is created (deprecated)
    await expect(rowMenu.getByRole("menuitem", { name: "Create Space Merge Request…" })).toHaveCount(0)
    await rowMenu.getByRole("menuitem", { name: "Safe Push S", exact: true }).click()
    await expect(chip(page, "feature-auth")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect(await startedBranches(repo)).toEqual(["ABC-123", "feature-auth"])

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
    await setupStarts(page, setupRepo, syncAndWaitForBranches)
    const row = findBranchRow(page, "ABC-123")
    await verifyBranchState(row, "closed")
    await slotSafePush(page, "ABC-123").click()
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)
    // the button starts, it doesn't toggle the row
    await verifyBranchState(row, "closed")
  })

  test("should open the Safe Push popover from the branch preview with S, and start from its footer button", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupStarts(page, setupRepo, syncAndWaitForBranches, RUNNING)

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
    expect(await startedBranches(repo)).toEqual(["ABC-123", "feature-auth"])
  })

  test("should cancel the robot for a new Safe Push from the popover, and not restart it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the robot runs on an older commit, so a start runs on the branch as it is
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, OLDER)
    const pushes = await routePushNotingPatronus(page, repo)

    await safePushFromMenu(page, "JIRA-456")
    // the click says first that its start replaces the running robot
    const popover = safePushPopover(page)
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popover.getByTestId("safe-push-replaces")).toHaveText("Cancels the Safe Push of JIRA-456 running on older-co.")
    expect(await patronusActions(repo)).toEqual({ cancels: [], restarts: [], starts: [] })
    await popoverStart(page).click()

    // and asks nothing more: canceled before the push, started after it, no restart of the old one
    await expect.poll(() => patronusActions(repo)).toEqual(REPLACED)
    expect(pushes.map(log => [log.cancels, log.starts])).toEqual([[[OLDER.id], []]])
    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
  })

  test("should cancel the robot it replaces without a push when the remote branch has the commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOlderWithPushedCommits(page, setupRepo, syncAndWaitForBranches)
    const pushes = await routePushNotingPatronus(page, repo)

    await safePushFromMenu(page, "JIRA-456")
    await expect(safePushPopover(page).getByTestId("safe-push-replaces")).toHaveText("Cancels the Safe Push of JIRA-456 running on older-co.")
    await popoverStart(page).click()
    // no push: the start cancels the robot itself, and never restarts it
    await expect.poll(() => patronusActions(repo)).toEqual(REPLACED)
    expect(pushes).toEqual([])
  })

  test("should start the Safe Push in its repository when another repository opens during its checks", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { patronus: availableRobots() })
    await addRecentProject(page)
    // the remote branch has the commits of JIRA-456: the start pushes nothing
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === "JIRA-456" ? { ...status, remoteExists: true, remoteTip: "JIRA-456-pushed", unpushedCommits: [], myUnpushedCount: 0, fastForward: false } : status,
    )
    await syncAndWaitForBranches()
    await expect(page.getByTestId("next-step-slot").first()).toBeAttached()
    const releaseCheck = await holdCommand(page, "check_target_conflict")

    await safePushFromMenu(page, "JIRA-456")
    await expect(slotSafePush(page, "JIRA-456")).toHaveText("Checking…")
    // the user opens another repository while the start checks the target
    await switchRepository(page, OTHER_REPOSITORY)
    releaseCheck()

    // the start goes on in the repository it was asked in: Patronus got it, and the toast says it
    await expect(toastWith(page, "Safe Push started")).toBeVisible()
    await expect.poll(async () => (await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456")])
    await expect(toastWith(page, "Cannot start the Safe Push")).toHaveCount(0)
    // back in it, the branch is landing
    await switchRepository(page, repo.path)
    await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
  })

  test("should start the Safe Push that replaces a robot which finished meanwhile, without a cancel", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOlderWithPushedCommits(page, setupRepo, syncAndWaitForBranches)
    await safePushFromMenu(page, "JIRA-456")
    await expect(safePushPopover(page).getByTestId("safe-push-replaces")).toHaveText("Cancels the Safe Push of JIRA-456 running on older-co.")
    // the robot fails while the popover is open: the page still has it running, Patronus lists it finished
    await repo.setPatronus({ availability: { status: "available" }, robots: [{ ...OLDER, status: "failure", finishTime: 1704142800 }] })
    await popoverStart(page).click()
    // nothing is left to cancel: the start goes on, never "Cannot cancel the Safe Push: … not running"
    await expect(toastWith(page, "Safe Push started")).toBeVisible()
    expect(await patronusActions(repo)).toEqual({ ...REPLACED, cancels: [] })
  })

  test("should wait to start until the remote status of the branch is loaded", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the first sync brings no remote status of JIRA-456
    await rewriteNextRemoteStatus(page, status => (status.branchName === "JIRA-456" ? null : status))
    const repo = await setupStarts(page, setupRepo, syncAndWaitForBranches)

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
    await expectJiraStarted(page, repo)
  })

  test("should restart a failed robot and cancel a running one from the popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED)

    let popover = await hoverStatusChip(page, chip(page, "ABC-123"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Cancel Safe Push" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Restart Safe Push" }).click()
    await expect(chip(page, "ABC-123")).toHaveAccessibleName(/^Safe Push: Running, /)
    expect((await repo.getPatronusLog()).restarts).toEqual([FAILED.id])
    await page.keyboard.press("Escape")

    popover = await hoverStatusChip(page, chip(page, "JIRA-456"), "patronus-popover")
    await expect(popover.getByRole("button", { name: "Restart Safe Push" })).toHaveCount(0)
    await popover.getByRole("button", { name: "Cancel Safe Push" }).click()
    await expect(chip(page, "JIRA-456")).toHaveText("Canceling")
    expect((await repo.getPatronusLog()).cancels).toEqual([RUNNING.id])

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
    const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, older)
    const pushes = await routePushNotingPatronus(page, repo)

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
    expect(pushes).toEqual([])
    releasePush()

    // the restart takes the commit just pushed; it is the action, its toast the one toast
    await expect(page.locator("[data-slot=toast-title]", { hasText: "Safe Push restarted" })).toBeVisible()
    expect({ restartsAtPush: pushes.map(log => log.restarts), restarts: (await repo.getPatronusLog()).restarts }).toEqual({ restartsAtPush: [[]], restarts: [older.id] })
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
      const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
      const pushes = await routePush(page, {})
      // the merge request needs the approvals until a recheck finds them
      const checks = await routeReviewRequirement(page, params =>
        params.recheck === true ? { ...REVIEW_REQUIRED, required: false, groups: [], mergeRequest: MERGE_REQUEST } : { ...REVIEW_REQUIRED, mergeRequest: MERGE_REQUEST },
      )

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
      await openSubmenu(page, "Patronus")
      await page.getByRole("menuitem", { name: "Check Approvals Again" }).click()
      await expect.poll(() => checks.some(params => params.recheck === true)).toBe(true)
      expect((await repo.getPatronusLog()).starts).toEqual([])

      // nothing to say any more: the click starts at once, with the merge request whose approvals count
      await slotSafePush(page, "JIRA-456").click()
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      // the branch isn't on the remote yet: the start pushes it first, without a toast of its own
      expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
      await expect(pushedToast(page, "JIRA-456")).toHaveCount(0)
      expect((await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456", { spaceReview: { projectKey: "IJ", reviewKey: "IJ-CR-7" } })])
    })

    test("should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
      // only on Space: Patronus finds no merge request, its pre-check needs the code owners
      await repo.setReviewAnswers({ answers: { "JIRA-456": REVIEW_REQUIRED } })

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
      const repo = await setupStarts(page, setupRepo, syncAndWaitForBranches)
      await repo.setReviewAnswers({ answers: { "JIRA-456": REVIEW_REQUIRED, "ABC-123": REVIEW_REQUIRED } })

      // "Dry Run" of the menu: the code owners don't check it, nothing to say, it starts
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await openSubmenu(page, "Patronus")
      await page.getByRole("menuitem", { name: "Dry Run", exact: true }).click()
      await expect(toastWith(page, "Dry Run started").locator("[data-slot=toast-description]")).toHaveText("JIRA-456 → master")
      expect((await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456", { mode: "DRY_RUN" })])
      expect(await repo.getReviewChecks()).toEqual({ calls: [], checks: [] })
      await expect(safePushPopover(page)).toHaveCount(0)

      // "Dry Run" of the Safe Push popover: it closes, and the dry run starts as the menu's does
      const popover = await openSafePushWithKey(page, "ABC-123")
      await popover.getByTestId("safe-push-dry-run").click()
      await expect(popover).toHaveCount(0)
      await expect(toastWith(page, "Dry Run started").filter({ hasText: "ABC-123 → master" })).toBeVisible()
      expect((await repo.getPatronusLog()).starts).toEqual([startRequest("JIRA-456", { mode: "DRY_RUN" }), startRequest("ABC-123", { mode: "DRY_RUN" })])
    })

    test("should open the pull request a robot that failed for missing approvals was started from, from its details", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const pullRequestUrl = "https://github.com/acme/monorepo/pull/7"
      await setupStarts(page, withMissingApprovals(setupRepo), syncAndWaitForBranches, { ...FAILED, pullRequest: { number: 7, url: pullRequestUrl } })
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
      const repo = await setupStarts(page, withMissingApprovals(setupRepo), syncAndWaitForBranches, FAILED)
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
      expect(await patronusActions(repo)).toEqual({ cancels: [], restarts: [], starts: [startRequest("ABC-123").sourceBranch] })
    })

    test("should close the branch preview for the Safe Push of its Patronus section", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupStarts(page, withMissingApprovals(setupRepo), syncAndWaitForBranches, FAILED)
      await expect(chip(page, "ABC-123")).toHaveText("Needs review")

      const preview = await openBranchPreview(page, "ABC-123")
      await preview.getByTestId("branch-preview-patronus").getByRole("button", { name: "Safe Push", exact: true }).click()
      // the start is the row's: the modal preview closes first, its progress shows in the row
      await expect(preview).toHaveCount(0)
      await expect(toastWith(page, "Safe Push started").locator("[data-slot=toast-description]")).toHaveText("ABC-123 → master")
      expect(await startedBranches(repo)).toEqual(["ABC-123"])
    })
  })

  test.describe("branches reviewed in GitHub pull requests", () => {
    /** issue_links mirrored to the GitHub repository `acme/monorepo`, with these pull requests and the profiles */
    async function setupGitHub(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
      ...pullRequests: PullRequest[]
    ): Promise<TestRepositoryBuilder> {
      return setupGitHubWith(page, setupRepo, syncAndWaitForBranches, undefined, ...pullRequests)
    }

    /** `setupGitHub` with what the pre-check answers by branch; without, no review is needed (no `OWNERSHIP` files) */
    async function setupGitHubWith(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
      reviewAnswers: Record<string, PatronusReviewRequirement> | undefined,
      ...pullRequests: PullRequest[]
    ): Promise<TestRepositoryBuilder> {
      const repo = await setupRepo("issue_links", {
        reviewAnswers,
        patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
        patronusProfiles: { profiles: PROFILES },
        pullRequests: { ...availablePullRequests(...pullRequests), users: [{ login: "alice-gh", name: "Alice" }] },
      })
      await syncAndWaitForBranches()
      await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
      for (const it of pullRequests) {
        await expect(findBranchRow(page, it.headRefName.split("/").pop()!).getByTestId("pull-request-chip")).toBeVisible()
      }
      return repo
    }

    /** JIRA-456 on GitHub without a pull request, the code owners required: the pushes and a creation answered */
    async function setupRequiredReview(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
    ) {
      const repo = await setupGitHubWith(page, setupRepo, syncAndWaitForBranches, { "JIRA-456": REVIEW_REQUIRED })
      const pushes = await routePush(page, {})
      return { repo, pushes }
    }

    /** JIRA-456 with the open pull request #7 and no review needed (no `OWNERSHIP` files); its push is answered in the page */
    async function setupPullRequestStart(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
      patch: Partial<PullRequest> = {},
    ): Promise<TestRepositoryBuilder> {
      const repo = await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, patch))
      await routePush(page, {})
      return repo
    }

    /** The Safe Push of JIRA-456 a comment on #7 starts now, as Patronus lists it */
    function robotOfComment(): PatronusRobot {
      return robot("JIRA-456", "pr-robot", {
        sourceRevision: null,
        startTime: Math.floor(Date.now() / 1000),
        pullRequest: { number: 7, url: "https://github.com/acme/monorepo/pull/7" },
      })
    }

    /** The comments the app posted on #7 of the fake GitHub, by their text */
    function comments(repo: TestRepositoryBuilder): Promise<unknown[]> {
      return gitHubRequests(repo, "POST", "issues", 7, "comments")
    }

    /**
     * A click on the slot of JIRA-456 confirms first with the warning `testId` saying `text`, after one read of the rules
     * of master and before any comment; a warning, not a refusal: the start comments on #7
     */
    async function expectWarnedStart(page: Page, repo: TestRepositoryBuilder, rules: readonly unknown[], testId: string, text: string): Promise<void> {
      await slotSafePush(page, "JIRA-456").click()
      const popover = safePushPopover(page)
      await expect(popover).toHaveAttribute("data-decision", "confirm")
      await expect(popover.getByTestId(testId)).toHaveText(text)
      expect(rules).toEqual([expect.objectContaining({ branch: "master" })])
      expect(await comments(repo)).toEqual([])
      await popoverStart(page).click()
      await expect.poll(() => comments(repo)).toEqual([{ body: "/patronus merge --keep-source-branch" }])
    }

    /**
     * The comment posted, then the fake clock over the app's wait before it reads the bot's reply (`COMMAND_REPLY_INTERVAL_MS`,
     * 3 s, in src/stores/patronus.ts) until it read it, instead of waiting it out
     */
    async function skipReplyWait(page: Page, repo: TestRepositoryBuilder) {
      await expect.poll(async () => (await comments(repo)).length).toBe(1)
      // the wait starts once the app has the posted comment, so a step can come before it: step until the read
      await skipCommandReplyWait(page, repo)
    }

    test("should start a Safe Push from the pull request with a /patronus merge comment", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches)
      const started = robotOfComment()
      // the robot the comment started is in the list asked for after the 👍
      const lists = await routeCommand(page, "get_patronus_robots", params => (params.since === null ? availableRobots() : availableRobots(started)))

      // a comment runs the default checks of the target: no profile to choose
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await openSubmenu(page, "Patronus")
      const withProfile = page.getByRole("menuitem", { name: "Safe Push With Profile…" })
      await expect(withProfile).toBeDisabled()
      await withProfile.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "comment" })).toHaveText("A comment on #7 runs the default checks: it can't choose a profile")
      await closeMenus(page)

      const popover = await openSafePushWithKey(page, "JIRA-456")
      await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #7: Patronus reports there and closes it once merged.")
      await expect(popover.getByTestId("safe-push-review")).toHaveText("No code-owner review needed.")
      await expect(popover.getByTestId("safe-push-profile-field")).toHaveCount(0)
      await popoverStart(page).click()
      await skipReplyWait(page, repo)

      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      // the own branch stays: the sync archives it once merged
      expect(await comments(repo)).toEqual([{ body: "/patronus merge --keep-source-branch" }])
      // the reply read is the one of the comment posted
      const posted = (await repo.getGitHubLog()).requests.findIndex(it => it.method === "POST" && it.path === repoPath("issues", 7, "comments"))
      const reads = (await repo.getGitHubLog()).requests.slice(posted + 1).filter(it => it.method === "GET" && it.path.startsWith(repoPath("issues", "comments")))
      expect(reads.length).toBeGreaterThan(0)
      expect(lists.at(-1)?.since).not.toBeNull()
      // the comment started it: no start through the API
      expect((await repo.getPatronusLog()).starts).toEqual([])

      const robotPopover = await hoverStatusChip(page, chip(page, "JIRA-456"), "patronus-popover")
      await robotPopover.getByRole("button", { name: "Open Pull Request #7" }).click()
      expect(await getOpenedUrls(page)).toContain("https://github.com/acme/monorepo/pull/7")
    })

    test("should hold a start Patronus accepted until it lists the robot: a repeat posts no second comment", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // nothing to push: the start is the comment alone
      await remoteHasCommitsOfJira(page)
      const repo = await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7))
      // the 👍 comes, the fake Patronus lists the robot only later

      await openSafePushWithKey(page, "JIRA-456")
      await popoverStart(page).click()
      await skipReplyWait(page, repo)
      // the minute the app lists the robots after the 👍 runs out
      await page.clock.fastForward(60_000)
      const toast = toastWith(page, "Safe Push started")
      await expect(toast.locator("[data-slot=toast-description]")).toHaveText("Patronus accepted the comment on #7: the robot shows once Patronus lists it")
      await expect(slotSafePush(page, "JIRA-456")).toHaveText("Starting…")
      await expect(slotSafePush(page, "JIRA-456")).toHaveAttribute("aria-busy", "true")

      // a second start meanwhile opens no popover and posts nothing: the press that opened the popover above
      await findBranchRow(page, "JIRA-456").focus()
      await page.keyboard.press("s")
      await expect(safePushPopover(page)).toHaveCount(0)
      expect(await comments(repo)).toEqual([{ body: "/patronus merge --keep-source-branch" }])

      // the look in the background finds the robot, which takes the slot of the start ("Starting…" above)
      await repo.setPatronus(availableRobots(robotOfComment()))
      await page.clock.fastForward(15_000)
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
      await expect(slotSafePush(page, "JIRA-456")).toHaveCount(0)
      expect(await comments(repo)).toEqual([{ body: "/patronus merge --keep-source-branch" }])
    })

    test("should say why Patronus refused the comment", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches)
      // the bot answers 👎 and says why in a comment of its own
      await repo.setPatronusReplies({ 7: { status: "rejected", reason: "Patronus did not start the robot due to an error. A robot is already running for the branch." } })

      // nothing to say: the click comments at once
      await slotSafePush(page, "JIRA-456").click()
      await skipReplyWait(page, repo)
      const error = toastWith(page, "Cannot start the Safe Push")
      await expect(error).toContainText("A robot is already running for the branch.")
      await expect(chip(page, "JIRA-456")).toHaveCount(0)
    })

    test("should warn before a push that dismisses the approvals of the pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches, { reviewers: [reviewer("alice", "approved")] })
      // the rules of the repository dismiss stale reviews
      await repo.setPullRequests(dismissingApprovals(pullRequest("JIRA-456", 7, { reviewers: [reviewer("alice", "approved")] })))
      const rules = await recordCommand(page, "get_branch_rules")

      // the approvals of alice don't count for Patronus, the push dismisses them: the click says so first
      await expectWarnedStart(
        page,
        repo,
        rules,
        "safe-push-dismisses",
        "The push dismisses the approvals of #7: the rules of the repository dismiss stale reviews when new commits are pushed.",
      )
    })

    test("should wait for the rules before a push into an approved pull request: Enter must not dismiss its approvals", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupPullRequestStart(page, setupRepo, syncAndWaitForBranches, { reviewers: [reviewer("alice", "approved")] })
      const rules = await holdCommandCalls(page, "get_branch_rules")

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
      await expect.poll(() => comments(repo)).toEqual([{ body: "/patronus merge --keep-source-branch" }])
    })

    /**
     * #7 approved by alice, and every read of the rules of master fails with `reason`; the reads recorded. The push of
     * the start runs through the test server (`origin` is the repository itself), not answered in the page
     */
    async function setupUnreadableRules(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
      reason: string,
    ): Promise<{ repo: TestRepositoryBuilder; rules: readonly unknown[] }> {
      const repo = await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, { reviewers: [reviewer("alice", "approved")] }))
      await routeCommandError(page, "get_branch_rules", reason)
      // the later route sees each read first
      const rules = await recordCommand(page, "get_branch_rules")
      return { repo, rules }
    }

    test("should confirm a push into an approved pull request whose rules can't be read, never reading them as keeping the approvals", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      const { repo, rules } = await setupUnreadableRules(page, setupRepo, syncAndWaitForBranches, "GitHub answered 403: Resource not accessible by integration")

      // the click has something to say: the popover, not a start
      await expectWarnedStart(
        page,
        repo,
        rules,
        "safe-push-rules-unknown",
        "Cannot check whether the push keeps the approvals of #7: GitHub answered 403: Resource not accessible by integration. Patronus counts them when the robot starts.",
      )
    })

    test("should ask again for the rules a start couldn't read, and confirm with the key", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { repo, rules } = await setupUnreadableRules(page, setupRepo, syncAndWaitForBranches, "GitHub answered 500")

      const first = await openSafePushWithKey(page, "JIRA-456")
      await expect(first.getByTestId("safe-push-rules-unknown")).toContainText("Cannot check whether the push keeps the approvals of #7: GitHub answered 500.")
      await page.keyboard.press("Escape")
      await expect(first).toHaveCount(0)
      expect(rules).toHaveLength(1)

      // a failed read is not kept: the next start asks GitHub again, and Enter confirms the start
      const popover = await openSafePushWithKey(page, "JIRA-456")
      await expect(popover.getByTestId("safe-push-rules-unknown")).toBeVisible()
      await expect.poll(() => rules.length).toBe(2)
      await expect(popoverStart(page)).toBeFocused()
      await page.keyboard.press("Enter")
      await expect.poll(() => comments(repo)).toEqual([{ body: "/patronus merge --keep-source-branch" }])
    })

    test.describe("a push that dismisses the approvals Patronus counts", () => {
      /** #7 approved by the code owner Patronus counts, and the rules dismiss stale reviews; JIRA-456 has commits the remote lacks */
      async function setupApproved(page: Page, setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>, syncAndWaitForBranches: () => Promise<unknown>) {
        const approved = pullRequest("JIRA-456", 7, { reviewers: [reviewer("ikorennoy", "approved")] })
        const repo = await setupApprovalsDismissedByPush(setupRepo, syncAndWaitForBranches, "JIRA-456", approved)
        await expect(findBranchRow(page, "JIRA-456").getByTestId("pull-request-chip")).toBeVisible()
        const pushes = await routePush(page, {})
        return { repo, pushes }
      }

      test("should refuse the click with the push alone in its toast", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        const { repo, pushes } = await setupApproved(page, setupRepo, syncAndWaitForBranches)
        await slotSafePush(page, "JIRA-456").click()
        // the robot would start without the approvals and fail: no start, the push is the step before it
        const refused = toastWith(page, "Safe Push of JIRA-456 can't start")
        await expect(refused).toContainText("The push dismisses the approvals of #7: push first, then ask for the review again")
        await refused.getByRole("button", { name: "Push", exact: true }).click()
        await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
        expect(await comments(repo)).toEqual([])
      })

      test("should say in the popover why it can't start, and push alone from it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        const { repo, pushes } = await setupApproved(page, setupRepo, syncAndWaitForBranches)
        const popover = await openSafePushWithKey(page, "JIRA-456")
        const status = popover.getByTestId("safe-push-status")
        await expect(status).toHaveAttribute("data-kind", "refuse")
        await expect(status).toHaveText("The push dismisses the approvals of #7: push first, then ask for the review again")
        await expect(popoverStart(page)).toBeDisabled()
        await popover.getByTestId("safe-push-push-only").click()
        await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
        expect(await comments(repo)).toEqual([])
      })
    })

    test("should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, { isDraft: true, reviewers: [reviewer("alice", "approved")] }))
      await routePush(page, {})

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
      await expectMenuSafePushRefused(page, "#7 is a draft: Patronus merges it once it is ready for review")
      await expect(page.getByRole("menuitem", { name: "Safe Push With Profile…", exact: true })).toBeDisabled()
      // a dry run pushes nothing into the target: a draft doesn't hold it back, it comments on the pull request
      await page.getByRole("menuitem", { name: "Dry Run", exact: true }).click()
      await expect.poll(() => comments(repo)).toEqual([{ body: "/patronus dry-run" }])
    })

    test("should offer no Safe Push of a branch whose pull request merged", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupGitHub(page, setupRepo, syncAndWaitForBranches, pullRequest("JIRA-456", 7, { state: "merged" }))

      await expect(findBranchRow(page, "JIRA-456").getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await expectMenuSafePushRefused(page, "Pull request #7 is merged")
      for (const name of ["Safe Push With Profile…", "Dry Run"]) {
        await expect(page.getByRole("menuitem", { name, exact: true })).toBeDisabled()
      }
    })

    test("should offer to create the pull request the approvals count in, and no start Patronus would fail", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { repo, pushes } = await setupRequiredReview(page, setupRepo, syncAndWaitForBranches)

      // not pushed yet: Patronus judges its local commits; it needs the code owners, whose approvals count in a pull request
      // the dialog opens at once: its Create pushes the branch, a dialog closed without one pushes nothing
      const dialog = await openCreatePullRequestFromSlot(page)
      await expect(dialog).toBeVisible()
      await expect(dialog.getByTestId("create-pull-request-submit")).toHaveText(/^Create Pull Request/)
      // Patronus names the code owner without a GitHub login: the user finds the person
      const suggestions = dialog.getByTestId("create-pull-request-suggestions")
      await expect(suggestions.getByRole("button")).toHaveText(["Alice"])
      const searches = await recordCommand(page, "search_github_users")
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
      expect((await repo.getPatronusLog()).starts).toEqual([])
    })

    test("should open the Safe Push popover without review from the pull request dialog", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { repo } = await setupRequiredReview(page, setupRepo, syncAndWaitForBranches)

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
      await expectJiraStarted(page, repo, { mergeProfileId: "lsc" })
    })

    test.describe("the pull request the approvals count in", () => {
      const CODE_OWNERS: PatronusReviewRequirement = {
        ...REVIEW_REQUIRED,
        groups: [{ ...REVIEW_REQUIRED.groups[0]!, reviewers: [{ fullName: "Alice", username: "alice", githubLogin: "alice-gh", available: true, suggested: true }] }],
      }

      test("should ask the code owner Patronus names, and the row then awaits the review in the pull request", async ({ page, setupRepo, syncAndWaitForBranches }) => {
        const repo = await setupRepo("issue_links", {
          patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
          pullRequests: { ...availablePullRequests(), users: [{ login: "alice-gh", name: "Alice" }], nextNumber: 7 },
        })
        await repo.setReviewAnswers({ answers: { "JIRA-456": CODE_OWNERS } })
        await syncAndWaitForBranches()
        const pushes = await routePush(page, {})
        const creates = await recordCommand(page, "create_pull_request")

        const dialog = await openCreatePullRequestFromSlot(page)
        const slot = findBranchRow(page, "JIRA-456").getByTestId("next-step-slot")
        // one suggested reviewer of each group is chosen at once
        await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["alice-gh"])
        await dialog.getByTestId("create-pull-request-submit").click()
        await expect(dialog).toHaveCount(0)
        // pushed first: GitHub opens a pull request of a branch it has
        expect(pushes.map(push => push.branchName)).toEqual(["JIRA-456"])
        expect(creates).toEqual([expect.objectContaining({ branchName: "JIRA-456", reviewers: ["alice-gh"], assignSelf: true, waitForMirror: true })])
        expect(gitHubWrites(await repo.getGitHubLog())).toContainEqual(["POST", repoPath("pulls", 7, "requested_reviewers"), { reviewers: ["alice-gh"] }])
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
        await pushFromRowMenu(page, "JIRA-456")
        await pushedToast(page, "JIRA-456").getByRole("button", { name: "Create Pull Request…" }).click()
        const dialog = createPullRequestDialog(page)
        await expect(dialog.getByTestId("create-pull-request-unavailable")).toContainText("No GitHub token to see pull requests: GitHub's form opens the pull request instead.")
        await expect(dialog.getByTestId("create-pull-request-submit")).toHaveCount(0)
        await expect(dialog.getByTestId("create-pull-request-reviewers")).toHaveCount(0)
        await expect(dialog.getByTestId("create-pull-request-assign")).toHaveCount(0)
        // the text of its commits fills the form first (under load it comes after the dialog)
        await expect(dialog.getByTestId("create-pull-request-title")).toHaveValue("JIRA-456 Fix database connection pool timeout")
        await dialog.getByTestId("create-pull-request-on-github").click()
        await expect(dialog).toHaveCount(0)
        // the text of its commits, and no assignee: nobody is known without a token
        const form =
          /^https:\/\/github\.com\/acme\/monorepo\/compare\/master\.\.\.user-name\/virtual\/JIRA-456\?expand=1&title=JIRA-456\+Fix\+database\+connection\+pool\+timeout&body=[^&]+$/
        expect(await getOpenedUrls(page)).toEqual([expect.stringMatching(form)])
      })
    })
  })

  test.describe("pushing into a branch whose Safe Push runs", () => {
    let repo: TestRepositoryBuilder
    test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // the robot runs on an older commit: the branch changed since, Restart (which pushes first) tests it as it is
      repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, OLDER)
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
      const pushes = await routePushNotingPatronus(page, repo)
      const preview = await openBranchPreview(page, "JIRA-456")
      await preview.getByTestId("push-button").hover()
      const description = page.getByTestId("push-description")
      await expect(description).toContainText("A Safe Push of JIRA-456 is running: wait until it merges, or restart it on the current branch")
      const actions = description.getByTestId("push-hold-actions")
      await actions.getByRole("button", { name: "Open in Patronus" }).click()
      expect(await getOpenedUrls(page)).toEqual([OLDER.url])
      await actions.getByRole("button", { name: "Restart Safe Push" }).click()
      await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })).toBeVisible()
      await expectCancelPushRestart(repo, pushes)
    })

    test("should cancel the robot, push and restart it on the pushed commit, from Restart Safe Push", async ({ page }) => {
      const pushes = await routePushNotingPatronus(page, repo)

      await pushAndRestartFromMenu(page)
      // the restart is the action: one toast says it, and opens the restarted robot
      const toast = page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })
      await expect(toast).toBeVisible()
      await expectCancelPushRestart(repo, pushes)
      await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
      await toast.getByRole("button", { name: "Open in Patronus" }).click()
      expect(await getOpenedUrls(page)).toEqual([robotUrl("restarted-1")])
      await expect(chip(page, "JIRA-456")).toHaveAccessibleName(/^Safe Push: Running, /)
    })

    test("should restart a robot canceled for a push the remote refused once the toast pushes", async ({ page }) => {
      const pushes = await routePush(page, { "JIRA-456": REMOTE_MOVED })
      await pushAndRestartFromMenu(page)
      const toast = page.locator("[data-slot=toast]", { hasText: "JIRA-456 changed on the remote" })
      await expect(toast).toContainText("The Safe Push is canceled: the push restarts it.")
      expect(await patronusActions(repo)).toEqual({ cancels: [OLDER.id], restarts: [], starts: [] })

      await toast.getByRole("button", { name: "Force Push" }).click()
      await confirmForcePush(page, "Force Push (3)")

      // canceled now: the restart takes the pushed commit, in the toast of the push
      await expect(pushedToast(page, "JIRA-456").locator("[data-slot=toast-description]")).toHaveText(/ · Safe Push restarted$/)
      // canceled once: the push that went through restarts it
      expect(await patronusActions(repo)).toEqual({ cancels: [OLDER.id], restarts: [OLDER.id], starts: [] })
      expect(pushes.map(push => push.expectedRemote)).toEqual([null, "moved-tip"])
    })
  })

  test.describe("a push the page's checks did not see", () => {
    test("should push nothing into a branch whose Safe Push started after the page's list: Rust refuses it, and the toast says why", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      // the page loaded with no robot; one started since (the IDE, bd), which the page learns of at its next poll
      const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches)
      await repo.setPatronus(availableRobots(RUNNING))
      const row = findBranchRow(page, "JIRA-456")
      const notPushed = row.getByRole("cell", { name: "not pushed" })
      await expect(notPushed).toBeVisible()

      // the push goes to the backend, which reads the user's robots itself before it pushes
      await pushFromRowMenu(page, "JIRA-456")
      const toast = page.locator("[data-slot=toast]", { hasText: "Cannot push JIRA-456" })
      await expect(toast).toContainText(
        "Nothing was pushed: A Safe Push of JIRA-456 is running on user-name/virtual/JIRA-456: a push moves the branch it took its commit from, so it won't land and fails the Safe Push at the end. Wait until it merges, or restart it on the current branch",
      )
      // nothing reached the remote: the branch is still not pushed; the backend asked one full list for the push, after
      // the page's one at load
      await expect(notPushed).toBeVisible()
      expect((await repo.getPatronusLog()).requests.list).toBe(2)
    })
  })
})
