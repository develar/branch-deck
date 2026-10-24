import type { Page } from "@playwright/test"
import type {
  PullRequest,
  BranchReviewReadiness,
  PatronusAvailability,
  PatronusReviewer,
  PatronusReviewerGroup,
  PatronusReviewRequirement,
  PatronusRobot,
  PatronusRobotsResult,
  ReviewReadiness,
  ReviewReadinessBranch,
} from "../../../src/lib/bindings"
import { expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "./branch-helpers"
import { dismissingApprovals } from "./pull-request-helpers"
import { waitForStepFacts } from "./safe-push-helpers"
import type { SetupRepoOptions } from "./setup-options"
import { emitTauriEvent } from "./sync-helpers"
import type { PatronusStartRequest, SyncOverrides, TestRepositoryBuilder } from "./test-repository"
import { routeCommand } from "./wait-helpers"

/**
 * A running Safe Push of a virtual branch. The empty `sourceRevision` makes the test server answer with the local tip
 * of the branch, so the robot tests the branch as it is unless a test sets another commit.
 */
export function robot(branchName: string, id: string, patch: Partial<PatronusRobot> = {}): PatronusRobot {
  return {
    id,
    name: `Safe Push of ${branchName}`,
    status: "running",
    pushMode: "rebaseAutosquash",
    branchName,
    sourceRevision: "",
    targetBranch: "master",
    startTime: 1704139200,
    finishTime: null,
    cancellationReason: null,
    url: robotUrl(id),
    ...patch,
  }
}

/**
 * The running `robot` merged: Patronus answers it on the next focus (a poll), its commit the tip its branch has then.
 * Returns it as the poll answered it, and keeps that commit in every later answer, whatever the branch becomes
 */
export async function mergeRunningRobot(page: Page, repo: TestRepositoryBuilder, robot: PatronusRobot, finishTime = 1704142800): Promise<PatronusRobot> {
  const [merged] = await mergeRunningRobots(page, repo, [robot], finishTime)
  return merged!
}

/**
 * The running `robots` merged in turn, an hour apart from `finishTime`, answered by one poll (`mergeRunningRobot`):
 * returned as the poll answered them, their commits kept in every later answer. `others` stay in every answer as they
 * are (a robot that finished before). The page shows the robots running first (a `landing-span`): the focus polls only
 * the running robots the page knows, so a focus before the list of the robots answered polls nothing
 */
export async function mergeRunningRobots(
  page: Page,
  repo: TestRepositoryBuilder,
  robots: readonly PatronusRobot[],
  finishTime = 1704142800,
  others: readonly PatronusRobot[] = [],
): Promise<PatronusRobot[]> {
  const polled = page.waitForResponse(response => response.url().endsWith("/invoke/poll_patronus_robots"))
  await repo.setPatronus(availableRobots(...others, ...robots.map((robot, index) => ({ ...robot, status: "success" as const, finishTime: finishTime + index * 3600 }))))
  await emitTauriEvent(page, "tauri://focus")
  const answered = ((await (await polled).json()) as PatronusRobotsResult).robots
  const merged = robots.map(robot => answered.find(it => it.id === robot.id)!)
  for (const robot of merged) {
    expect(robot.sourceRevision).toMatch(/^[0-9a-f]{40}$/)
  }
  await repo.setPatronus(availableRobots(...others, ...merged))
  return merged
}

/**
 * What Patronus answers of a robot whose Fast Tests failed (`TestRepositoryBuilder.setPatronusDetails`): the check
 * (`Fast_Aggregator`, build 1095220100, attempt 1 of 2, one failed test), its error problem, and the failed Unit Tests
 * build of the attempt with the failed test `com.example.AuthTest.testLogin`
 */
export function failedFastTests(robotId: string) {
  const attemptId = `${robotId}-fast-1`
  return {
    checks: {
      [robotId]: [
        {
          id: "1",
          name: "Fast Tests",
          buildConfigurationId: "Fast_Aggregator",
          buildConfigurationName: "Fast Tests Aggregator",
          status: "FAILURE",
          attemptLimit: 2,
          attempts: [
            {
              id: attemptId,
              number: 0,
              buildId: "1095220100",
              buildUrl: "https://ci.example/build/1095220100",
              status: "FAILURE",
              failedTestsNumber: 1,
              failedBuildsNumber: 1,
              failedToStartBuildsNumber: 0,
            },
          ],
        },
      ],
    },
    problems: { [robotId]: [{ id: "p", uniqueId: null, title: "Fast Tests failed", detailsMarkdown: null, severity: "ERROR", subsystem: "TEAMCITY_CHECKS" }] },
    attemptBuilds: {
      [attemptId]: [
        {
          buildId: "1095220101",
          buildUrl: "https://ci.example/build/1095220101",
          buildConfigurationId: "UnitTests",
          buildConfigurationUrl: "https://ci.example/UnitTests",
          buildConfigurationName: "Unit Tests",
          fullProjectName: "Example",
          status: "FAILURE",
          isFailedToStart: false,
          isComposite: false,
          isFailedTestsOnlyFailure: true,
          problems: [],
          failedTests: [
            {
              name: "com.example.AuthTest.testLogin",
              url: null,
              isIgnoredFailure: false,
              ignoreReason: null,
              buildId: "1095220101",
              testId: "1",
              testRunId: "1",
              oldUiUrl: null,
              testHistoryUrl: null,
            },
          ],
        },
      ],
    },
  }
}

/**
 * What Patronus answers of a robot whose Fast Tests failed before any build started (`setPatronusDetails`): the CI
 * failed, not the branch, so the failure is `infrastructure` and its toast offers the restart
 */
export function buildsNeverStarted(robotId: string) {
  const attempt = { id: `${robotId}-fast-1`, number: 0, buildId: "1095220200", status: "FAILURE", failedTestsNumber: 0, failedBuildsNumber: 0, failedToStartBuildsNumber: 3 }
  return {
    checks: {
      [robotId]: [
        {
          id: "1",
          name: "Fast Tests",
          buildConfigurationId: "Fast_Aggregator",
          buildConfigurationName: "Fast Tests Aggregator",
          status: "FAILURE",
          attemptLimit: 1,
          attempts: [attempt],
        },
      ],
    },
  }
}

/**
 * The request of a start of `branchName` of the `issue_links` templates (prefix `user-name`) into master, as the fake
 * Patronus logs it (`getPatronusLog().starts`): a Safe Push with the default checks unless `patch` says otherwise
 */
export function startRequest(branchName: string, patch: Partial<PatronusStartRequest> = {}): PatronusStartRequest {
  return {
    type: "ROBOT_WEB_UI",
    repository: "repository",
    sourceBranch: `refs/heads/user-name/virtual/${branchName}`,
    targetBranch: "master",
    mode: "REBASE_AUTOSQUASH",
    sourceBranchDeletePolicy: "NEVER",
    mergeProfileId: null,
    ...patch,
  }
}

/** `startRequest` of the whole stack on `root`: its stack branch `user-name/stack/<root>` is the source */
export function wholeStackStartRequest(root: string, patch: Partial<PatronusStartRequest> = {}): PatronusStartRequest {
  return startRequest(root, { sourceBranch: `refs/heads/user-name/stack/${root}`, ...patch })
}

/** Waits until the fake Patronus logged exactly the start requests `requests`, in order */
export async function expectPatronusStarts(repo: TestRepositoryBuilder, requests: PatronusStartRequest[]): Promise<void> {
  await expect.poll(async () => (await repo.getPatronusLog()).starts).toEqual(requests)
}

/** The page of the robot `id` a fake Patronus started (`started-<n>`, `restarted-<n>`), as the client derives it */
export function robotUrl(id: string): string {
  return `https://patronus.labs.jb.gg/robot/${id}`
}

/** The warning Patronus registers on a running robot once a required check failed: the robot will fail */
export function willFail(robotId: string) {
  return {
    problems: {
      [robotId]: [
        {
          id: "w",
          uniqueId: "robot_is_failing",
          title: "This Safe Push will fail because required checks have already failed",
          detailsMarkdown: null,
          severity: "WARNING",
          subsystem: "TEAMCITY_CHECKS",
        },
      ],
    },
  }
}

/** The error problem of a robot that failed for missing code-owner approvals */
export function missingApprovals(robotId: string) {
  return {
    problems: {
      [robotId]: [{ id: "p", uniqueId: null, title: "Changes require review approval", detailsMarkdown: null, severity: "ERROR", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }],
    },
  }
}

/** The answer of `get_patronus_robots` for these robots */
export function availableRobots(...robots: PatronusRobot[]): PatronusRobotsResult {
  return { availability: { status: "available" }, robots }
}

export function unavailableRobots(availability: PatronusAvailability): PatronusRobotsResult {
  return { availability, robots: [] }
}

/**
 * `template` with the `running` robot, synced, and the robot shown running: the `landing-span` of `landed`, a branch it
 * lands with it. A merge of it (`mergeRunningRobot`) polls it then
 */
export async function setupRunningRobot(
  page: Page,
  setupRepo: (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  template: string,
  running: PatronusRobot,
  landed: string,
): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo(template, { patronus: availableRobots(running) })
  await syncAndWaitForBranches()
  await expect(findBranchRow(page, landed).getByTestId("landing-span")).toHaveText(`lands with ${running.branchName}`)
  return repo
}

/**
 * `setupRunningRobot` with `options`, the robot then kept on the commit it took: the tip its branch had when Patronus was
 * first asked, in every later answer, whatever the branch becomes (a restack after the start). Returns the repository
 * and the robot as it was answered
 */
export async function setupTakenRobot(
  page: Page,
  setupRepo: (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  template: string,
  running: PatronusRobot,
  landed: string,
  options: SetupRepoOptions = {},
): Promise<{ repo: TestRepositoryBuilder; robot: PatronusRobot }> {
  const listed = page.waitForResponse(response => response.url().endsWith("/invoke/get_patronus_robots"))
  const repo = await setupRunningRobot(page, (name, setup) => setupRepo(name, { ...options, ...setup }), syncAndWaitForBranches, template, running, landed)
  const taken = ((await (await listed).json()) as PatronusRobotsResult).robots.find(it => it.id === running.id)!
  expect(taken.sourceRevision).toMatch(/^[0-9a-f]{40}$/)
  await repo.setPatronus(availableRobots(taken))
  return { repo, robot: taken }
}

/** The issue_links repository with these robots */
export function setupRobots(
  page: Page,
  setupRepo: (template: string, options: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  ...robots: PatronusRobot[]
): Promise<TestRepositoryBuilder> {
  return setupRobotsWith(page, setupRepo, syncAndWaitForBranches, {}, ...robots)
}

/** `setupRobots` with more `options` of the repository (`patronusDetails`) */
export async function setupRobotsWith(
  page: Page,
  setupRepo: (template: string, options: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  options: SetupRepoOptions,
  ...robots: PatronusRobot[]
): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("issue_links", { ...options, patronus: availableRobots(...robots) })
  await syncAndWaitForBranches()
  // the robots load after the sync, with the answer that shows the Safe Push slots (empty while a robot runs): a push
  // before they did would not know that one runs
  await expect(page.getByTestId("next-step-slot").first()).toBeAttached()
  // and the steps once every fact they wait for is in (why a robot failed, what a branch needs to land): a step that
  // answers late changes the width of its column and moves the chips under the pointer of a hover
  await waitForStepFacts(page.getByTestId("next-step-slot"))
  await expect(page.locator('[data-testid="next-step-slot"][data-step="pending"]')).toHaveCount(0)
  return repo
}

/**
 * Answers `get_branch_review_readiness` in the page with the requirement `answer` gives for the call, for every branch
 * asked; the readiness is that of a Patronus that doesn't evaluate pull requests (missing while required, unless the
 * groups say they approved). For the readiness the app computes, set the answer on the test server instead
 * (`TestRepositoryBuilder.setReviewAnswers`). The params of each call.
 */
export function routeReviewRequirement(page: Page, answer: (params: Record<string, unknown>) => PatronusReviewRequirement): Promise<Record<string, unknown>[]> {
  return routeCommand(page, "get_branch_review_readiness", params =>
    (params.branches as ReviewReadinessBranch[]).map(branch => {
      const requirement = answer(params)
      return { branchName: branch.branchName, tip: branch.tip, requirement, readiness: readinessOf(requirement) } satisfies BranchReviewReadiness
    }),
  )
}

function readinessOf(requirement: PatronusReviewRequirement): ReviewReadiness {
  if (requirement.skipped) {
    return { kind: "unknown", reason: "skipped" }
  }
  if (!requirement.required) {
    return { kind: "notRequired" }
  }
  const groups = requirement.groups
    .filter(group => group.approved !== true)
    .map(group => ({ name: group.name, needed: null, approvedBy: [], requested: [], suggested: [], changesRequestedBy: [] }))
  return { kind: "missing", groups, notCounted: [], reason: "approvalsMissing" }
}

/** A member of a reviewer group, as Patronus names it */
export function codeOwner(fullName: string, login: string, patch: Partial<PatronusReviewer> = {}): PatronusReviewer {
  return { fullName, username: login, githubLogin: login, available: true, suggested: false, ...patch }
}

/** A group of an answer about a pull request: `approved` and the counts of an ownership rule that needs one approval */
export function evaluatedGroup(name: string, approved: boolean, reviewers: PatronusReviewer[], patch: Partial<PatronusReviewerGroup> = {}): PatronusReviewerGroup {
  return {
    name,
    reviewers,
    paths: [],
    branchWide: true,
    pathsSource: "patronus",
    requiredApprovals: 1,
    collectedApprovals: approved ? 1 : 0,
    approved,
    selfApprovalAllowed: false,
    ...patch,
  }
}

/** No approval needed (for `setReviewAnswers`): the readiness is `notRequired` for any start */
export const NO_REVIEW_NEEDED: PatronusReviewRequirement = { required: false, mergeRequest: null, groups: [], skipped: false }

/**
 * The answer of the screenshot of 2026-10-17 (for `setReviewAnswers`): IJPL Code Platform has to approve; Ilya Korennoy
 * is asked and hasn't. With the pull request's reviewers `reviewer("junie-jetbrains", "approved")` and
 * `reviewer("ikorennoy", "requested")`, the readiness waits for him and doesn't count the bot
 */
export const AWAITING_CODE_OWNER: PatronusReviewRequirement = {
  required: true,
  mergeRequest: null,
  groups: [evaluatedGroup("IJPL Code Platform", false, [codeOwner("Ilya Korennoy", "ikorennoy", { suggested: true, approvalState: "reviewing" })])],
  skipped: false,
}

/** The answer once Ilya Korennoy approved: the readiness is approved */
export const APPROVED_BY_CODE_OWNER: PatronusReviewRequirement = {
  required: false,
  mergeRequest: null,
  groups: [evaluatedGroup("IJPL Code Platform", true, [codeOwner("Ilya Korennoy", "ikorennoy", { suggested: true, approvalState: "approved" })])],
  skipped: false,
}

/**
 * The answer of a Patronus that doesn't evaluate pull requests: approvals required, the groups without states; with an
 * approval in the pull request the readiness is unknown, without one missing
 */
export const NOT_EVALUATED: PatronusReviewRequirement = {
  required: true,
  mergeRequest: null,
  groups: [{ name: "IJPL Code Platform", reviewers: [codeOwner("Ilya Korennoy", "ikorennoy", { suggested: true })], paths: [], branchWide: true, pathsSource: "patronus" }],
  skipped: false,
}

/** The page's clock: 2024-01-02 10:00Z, in seconds */
export const ROBOTS_NOW = 1704189600

/** `count` minutes in seconds */
export function minutes(count: number): number {
  return count * 60
}

/**
 * The robots of a day of `issue_links` for the header and the Dock: JIRA-456 runs for 18 minutes (a Safe Push usually
 * takes 40), the Safe Push of ABC-123 failed 50 minutes ago on it as it is, feature-db merged 80 minutes ago
 */
export const DAY_ROBOTS = {
  running: robot("JIRA-456", "running-robot", { startTime: ROBOTS_NOW - minutes(18) }),
  failed: robot("ABC-123", "failed-robot", { status: "failure", startTime: ROBOTS_NOW - minutes(80), finishTime: ROBOTS_NOW - minutes(50) }),
  merged: robot("feature-db", "merged-robot", { status: "success", startTime: ROBOTS_NOW - minutes(120), finishTime: ROBOTS_NOW - minutes(80) }),
}

/**
 * issue_links on Space, mirrored to `acme/monorepo`, synced with `approved`, the pull request of `branch` whose approval
 * by the code owner Patronus counts (`APPROVED_BY_CODE_OWNER`), in a repository whose rules dismiss the approvals of a
 * pull request on a push of new commits; the first sync reports what `syncOverrides` sets
 */
export async function setupApprovalsDismissedByPush(
  setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  branch: string,
  approved: PullRequest,
  syncOverrides?: SyncOverrides,
): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("issue_links", {
    patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
    pullRequests: dismissingApprovals(approved),
    syncOverrides,
  })
  await repo.setReviewAnswers({ answers: { [branch]: APPROVED_BY_CODE_OWNER } })
  await syncAndWaitForBranches()
  return repo
}
