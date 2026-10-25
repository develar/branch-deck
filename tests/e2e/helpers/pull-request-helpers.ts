import { expect } from "@playwright/test"
import type { Locator, Page } from "@playwright/test"
import type { GitHubUser, PullRequest, PullRequestAvailability, Reviewer, ReviewerState } from "../../../src/lib/bindings"

/** The branch prefix of the E2E repositories */
const BRANCH_PREFIX = "user-name"

/** The GitHub repository of a test repository with pull requests (`branchdeck.githubRepository`), on its fake GitHub */
export const GITHUB_REPOSITORY = { owner: "acme", name: "monorepo" }

/**
 * A listing of the pull requests as the session's list answers it (`PullRequestsResult` of Rust, which no command
 * carries to the page any more): the availability, the pull requests by branch and by the root of a whole stack
 */
export interface PullRequestsFixture {
  availability: PullRequestAvailability
  pullRequests: Record<string, PullRequest>
  stackPullRequests?: Record<string, PullRequest>
}

/**
 * What the fake GitHub of a test repository holds (`setPullRequests`): the listing the session of the repository reads,
 * which the real service reads back through the real parser (the server refuses one it would not), and what only the
 * writes see: `repository` of an availability that names none, the `users` a review can be asked from and assigned
 * (the collaborators: GitHub refuses a review request of anyone else), whether the rules of every branch dismiss
 * approvals on a push, the number of the next pull request created, the branches a mirror has not copied yet
 */
export type GitHubFixture = PullRequestsFixture & {
  repository?: string
  users?: GitHubUser[]
  dismissStaleReviewsOnPush?: boolean
  nextNumber?: number
  missingBranches?: string[]
}

/** A request that reached the fake GitHub: method, path and JSON body (null for none) */
export type GitHubRequest = { method: string; path: string; body: unknown }

/** What the app asked of GitHub (`calls`: the session's lists of the pull requests) and every request that reached the fake, in order */
export type GitHubLog = { calls: number; requests: GitHubRequest[] }

/** The REST path of the fake's repository: `repoPath("pulls", 7)` is `/repos/acme/monorepo/pulls/7` */
export function repoPath(...segments: (string | number)[]): string {
  return ["", "repos", GITHUB_REPOSITORY.owner, GITHUB_REPOSITORY.name, ...segments].join("/")
}

/** The requests of `method` to the path `segments` of the fake's repository, a `*` segment any: their bodies, in order */
export async function gitHubRequests(repo: { getGitHubLog(): Promise<GitHubLog> }, method: string, ...segments: (string | number)[]): Promise<unknown[]> {
  const pattern = new RegExp(`^${repoPath(...segments).replaceAll("*", "[^/]+")}$`)
  return (await repo.getGitHubLog()).requests.filter(it => it.method === method && pattern.test(it.path)).map(it => it.body)
}

/**
 * Runs the fake clock over the app's wait before it reads the reply of the Patronus bot to its comment
 * (`COMMAND_REPLY_INTERVAL_MS`, 3 s) until it read it from the fake GitHub
 */
export async function skipCommandReplyWait(page: Page, repo: { getGitHubLog(): Promise<GitHubLog> }): Promise<void> {
  await expect
    .poll(async () => {
      await page.clock.fastForward(3000)
      return (await gitHubRequests(repo, "GET", "issues", "comments", "*")).length
    })
    .toBeGreaterThan(0)
}

/** The writes of a log (every request but a GET and a GraphQL query), as `[method, path, body]` */
export function gitHubWrites(log: GitHubLog): [string, string, unknown][] {
  return log.requests.filter(it => it.method !== "GET" && it.path !== "/graphql").map(it => [it.method, it.path, it.body])
}

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
    author: VIEWER_LOGIN,
    ...patch,
  }
}

/**
 * A reviewer of a pull request: `state` is the latest submitted review, `requested` an open review request (by default
 * only for one who hasn't reviewed); a re-requested approver is `reviewer("alice", "approved", true)`
 */
export function reviewer(login: string, state: ReviewerState, requested = state === "requested"): Reviewer {
  return { login, state, requested }
}

/** The GitHub of these pull requests, keyed by their branch as `get_pull_requests` answers them */
export function availablePullRequests(...pullRequests: PullRequest[]): GitHubFixture {
  return {
    availability: { status: "available", ...GITHUB_REPOSITORY },
    pullRequests: Object.fromEntries(pullRequests.map(it => [it.headRefName.slice(`${BRANCH_PREFIX}/virtual/`.length), it])),
  }
}

/** The GitHub of these pull requests whose rules dismiss the approvals of a pull request on a push of new commits */
export function dismissingApprovals(...pullRequests: PullRequest[]): GitHubFixture {
  return { ...availablePullRequests(...pullRequests), dismissStaleReviewsOnPush: true }
}

/** A GitHub that answers every request with the failure of `availability` (a repository for any but `notGitHub`) */
export function unavailablePullRequests(availability: PullRequestAvailability): GitHubFixture {
  const repository = availability.status === "notGitHub" ? {} : { repository: `${GITHUB_REPOSITORY.owner}/${GITHUB_REPOSITORY.name}` }
  return { availability, pullRequests: {}, ...repository }
}

/** The URLs the app opened in the browser (the opener plugin is mocked) */
export async function getOpenedUrls(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __OPENED_URLS__?: string[] }).__OPENED_URLS__ ?? [])
}

/** The user of the GitHub token in E2E: the dialog leaves the user out of the reviewers */
export const VIEWER_LOGIN = "user-name"

export function createPullRequestDialog(page: Page): Locator {
  return page.getByTestId("create-pull-request-dialog")
}
