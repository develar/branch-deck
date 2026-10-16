import type { Locator, Page } from "@playwright/test"
import type { CreatedPullRequest, NewPullRequest, PullRequest, PullRequestAvailability, PullRequestsResult } from "../../../src/lib/bindings"
import { routeCommand } from "./wait-helpers"

/** The branch prefix of the E2E repositories */
const BRANCH_PREFIX = "user-name"

/** The GitHub repository that `get_pull_requests` names, and that the pull requests are on */
export const GITHUB_REPOSITORY = { owner: "JetBrains", name: "ultimate" }

/**
 * An open pull request of a virtual branch that waits for a review, without checks. The empty `headRefOid` makes the
 * test server answer with the local tip of the branch, so the pull request is up to date unless a test sets one.
 */
export function pullRequest(branchName: string, number: number, patch: Partial<PullRequest> = {}): PullRequest {
  return {
    number,
    title: `Pull request of ${branchName}`,
    url: `https://github.com/${GITHUB_REPOSITORY.owner}/${GITHUB_REPOSITORY.name}/pull/${number}`,
    state: "open",
    isDraft: false,
    reviewDecision: "reviewRequired",
    headRefName: `${BRANCH_PREFIX}/virtual/${branchName}`,
    headRefOid: "",
    checks: null,
    updatedAt: 1704139200,
    reviewers: [],
    ...patch,
  }
}

/** The answer of `get_pull_requests` for these pull requests, keyed by their branch */
export function availablePullRequests(...pullRequests: PullRequest[]): PullRequestsResult {
  return {
    availability: { status: "available", ...GITHUB_REPOSITORY },
    pullRequests: Object.fromEntries(pullRequests.map(it => [it.headRefName.slice(`${BRANCH_PREFIX}/virtual/`.length), it])),
  }
}

export function unavailablePullRequests(availability: PullRequestAvailability): PullRequestsResult {
  return { availability, pullRequests: {} }
}

/** The URLs the app opened in the browser (the opener plugin is mocked) */
export async function getOpenedUrls(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __OPENED_URLS__?: string[] }).__OPENED_URLS__ ?? [])
}

/** The user of the GitHub token in E2E: the dialog leaves the user out of the reviewers */
export const VIEWER_LOGIN = "user-name"

/**
 * Answers `get_new_pull_request` (the text the "Create pull request" dialog starts with) in the page: the test server
 * pushes nothing, so there are no remote branches to read the commits of
 */
export function routeNewPullRequest(page: Page, patch: Partial<NewPullRequest> = {}): Promise<Record<string, unknown>[]> {
  return routeCommand(
    page,
    "get_new_pull_request",
    () => ({ title: "Add auth", body: "- **Add login**", unpushed: 0, viewerLogin: VIEWER_LOGIN, ...patch }) satisfies NewPullRequest,
  )
}

/** The answer of `create_pull_request` for a pull request GitHub created, assigned to the user */
export function createdPullRequest(created: PullRequest, patch: Partial<CreatedPullRequest> = {}): CreatedPullRequest {
  return { pullRequest: created, created: true, assignees: [VIEWER_LOGIN], warnings: [], ...patch }
}

export function createPullRequestDialog(page: Page): Locator {
  return page.getByTestId("create-pull-request-dialog")
}
