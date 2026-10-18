import type { Page } from "@playwright/test"
import type {
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
import { emitTauriEvent } from "./sync-helpers"
import type { TestRepositoryBuilder } from "./test-repository"
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
    url: `https://patronus.labs.jb.gg/robot/${id}`,
    ...patch,
  }
}

/**
 * The running `robot` merged: Patronus answers it on the next focus (a poll), its commit the tip its branch has then.
 * Returns it as the poll answered it, and keeps that commit in every later answer, whatever the branch becomes
 */
export async function mergeRunningRobot(page: Page, repo: TestRepositoryBuilder, robot: PatronusRobot, finishTime = 1704142800): Promise<PatronusRobot> {
  const polled = page.waitForResponse(response => response.url().endsWith("/invoke/poll_patronus_robots"))
  await repo.setPatronus(availableRobots({ ...robot, status: "success", finishTime }))
  await emitTauriEvent(page, "tauri://focus")
  const merged = ((await (await polled).json()) as PatronusRobotsResult).robots.find(it => it.id === robot.id)!
  expect(merged.sourceRevision).toMatch(/^[0-9a-f]{40}$/)
  await repo.setPatronus(availableRobots(merged))
  return merged
}

/** The answer of `get_patronus_robots` for these robots */
export function availableRobots(...robots: PatronusRobot[]): PatronusRobotsResult {
  return { availability: { status: "available" }, robots }
}

export function unavailableRobots(availability: PatronusAvailability): PatronusRobotsResult {
  return { availability, robots: [] }
}

/**
 * The issue_links repository with these robots; the unassigned commits card renders after the branches and would move
 * the rows away from the pointer
 */
export async function setupRobots(
  page: Page,
  setupRepo: (template: string, options: { patronus: unknown }) => Promise<unknown>,
  syncAndWaitForBranches: () => Promise<unknown>,
  ...robots: PatronusRobot[]
): Promise<void> {
  await setupRepo("issue_links", { patronus: availableRobots(...robots) })
  await syncAndWaitForBranches()
  await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
  // the robots load after the sync, with the answer that shows the Safe Push slots (empty while a robot runs): a push
  // before they did would not know that one runs
  await expect(page.getByTestId("next-step-slot").first()).toBeAttached()
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
