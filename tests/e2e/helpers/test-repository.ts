import type {
  BranchNameSuggestionOutcome,
  PatronusCommandReply,
  PatronusProfile,
  PatronusReviewRequirement,
  PatronusRobotProgress,
  PatronusRobotsResult,
  ReviewRequestsResult,
} from "../../../src/lib/bindings"
import type { GitHubFixture, GitHubLog } from "./pull-request-helpers"
import { TEST_SERVER_URL } from "./test-server-url"

/** What reached the fake Patronus: the requests by kind, the robots whose problems and progress were asked, in order */
export type PatronusRequests = { list: number; updates: number; details: number; problems: string[]; progress: string[]; startOptions: number }

/** A start request as the fake Patronus logs it (`BranchRobotStartRequest` of the patronus crate); see `startRequest` */
export type PatronusStartRequest = {
  type: "ROBOT_WEB_UI"
  repository: string
  sourceBranch: string
  targetBranch: string
  mode: "REBASE_AUTOSQUASH" | "DRY_RUN"
  sourceBranchDeletePolicy: "NEVER"
  mergeProfileId: string | null
  spaceReview?: { projectKey: string; reviewKey: string }
}

/**
 * What the page asked of Patronus (its commands, which the caches of the service may answer), what reached the fake
 * (`requests`), and the actions in order: the start requests, the robots restarted, canceled and whose status was read
 */
export type PatronusLog = {
  listCalls: number
  pollCalls: number
  detailCalls: number
  requests: PatronusRequests
  starts: PatronusStartRequest[]
  restarts: string[]
  cancels: string[]
  statusReads: string[]
}

export class TestRepositoryBuilder {
  private baseUrl = TEST_SERVER_URL
  public id: string = ""
  public path: string = ""
  private template: string | null = null
  private prepopulateStore: boolean = true

  useTemplate(templateName: string): this {
    this.template = templateName
    return this
  }

  withPrepopulateStore(prepopulate: boolean): this {
    this.prepopulateStore = prepopulate
    return this
  }

  async init(): Promise<void> {
    if (!this.template) {
      throw new Error("Template is required. Use useTemplate() before init()")
    }

    // Create a new test repository
    const response = await fetch(`${this.baseUrl}/repositories`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        template: this.template,
        prepopulate_store: this.prepopulateStore,
      }),
    })

    if (!response.ok) {
      throw new Error(`Failed to create repository: ${response.statusText}`)
    }

    const data = await response.json()
    this.id = data.id
    this.path = data.path
  }

  /** Rewrites the message of a commit behind the app's back (as `git rebase -i` would); returns the new commit id. */
  async reword(commitId: string, message: string): Promise<string> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/reword`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ commitId, message }),
    })

    if (!response.ok) {
      throw new Error(`Failed to reword commit ${commitId}: ${response.statusText}`)
    }

    const data = await response.json()
    return data.newCommitId
  }

  /**
   * Commits files onto HEAD behind the app's back, as a commit in a terminal would (a new commit of a branch), authored
   * at `timestamp` (Unix seconds); returns the new commit id
   */
  async commit(message: string, files: Record<string, string>, timestamp: number): Promise<string> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/commit`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ message, files: Object.entries(files), timestamp }),
    })
    if (!response.ok) {
      throw new Error(`Failed to commit ${message}: ${response.statusText}`)
    }
    const data = await response.json()
    return data.commitId
  }

  /**
   * Sets what the fake GitHub of the repository holds from now on (`GitHubFixture`), read and written by the real
   * service (docs/design/e2e-testing.md#remote-services); an empty `headRefOid` is the tip of the head branch when
   * GitHub answers
   */
  async setPullRequests(fixture: GitHubFixture): Promise<void> {
    await this.setFixture("pull-requests", fixture)
  }

  /**
   * The app's GitHub sign-in from now on, run by the real session against the device flow of the fake GitHub; a field
   * left out changes nothing. `clientId`: the OAuth app of the build (none by default, as a build without it);
   * `authorize`: the user entered the codes asked for so far on GitHub, so the waiting sign-in gets the token of
   * `user-name`; `signedIn`: the token file holds that sign-in, or none (signed in or out elsewhere); `ghToken`: `gh` is
   * signed in with this token
   */
  async setGitHubSignIn(fixture: { clientId?: string; authorize?: boolean; signedIn?: boolean; ghToken?: string }): Promise<void> {
    await this.setFixture("github-sign-in", fixture)
  }

  /** The sign-in as the backend holds it (`pending`: one waits for its code) and the codes and polls that reached GitHub */
  async getGitHubSignIn(): Promise<{ pending: boolean; codes: number; polls: number }> {
    return this.getFixture("github-sign-in")
  }

  /** How often the app asked for pull requests */
  async getPullRequestCalls(): Promise<number> {
    return (await this.getGitHubLog()).calls
  }

  /** What the page asked of GitHub and every request that reached the fake GitHub, in order */
  async getGitHubLog(): Promise<GitHubLog> {
    return this.getFixture<GitHubLog>("pull-requests")
  }

  /**
   * Sets what `get_review_requests` answers from now on (E2E never runs gh); `fetchError` fails `fetch_review_head`
   * with that message
   */
  async setReviewRequests(result: ReviewRequestsResult, fetchError?: string): Promise<void> {
    await this.setFixture("review-requests", fetchError === undefined ? result : { ...result, fetchError })
  }

  /** How often the app asked for the review requests */
  async getReviewRequestCalls(): Promise<number> {
    return (await this.getReviewRequestsFixture()).calls
  }

  /** The calls of the review commands: the requests, the fetches, and the `keep` of each prune of the review refs */
  async getReviewRequestsFixture(): Promise<{ calls: number; fetchCalls: number; prunes: number[][] }> {
    return this.getFixture("review-requests")
  }

  /** Sets what `suggest_branch_names` answers from now on and how long it takes (E2E never runs claude) */
  async setBranchNameSuggestions(fixture: { outcome?: BranchNameSuggestionOutcome; delayMs?: number }): Promise<void> {
    await this.setFixture("branch-name-suggestions", fixture)
  }

  /** How often the app asked for branch names */
  async getBranchNameSuggestionCalls(): Promise<number> {
    return (await this.getFixture<{ calls: number }>("branch-name-suggestions")).calls
  }

  /**
   * Sets what the fake Patronus of the repository answers from now on: the robots of the list, of a poll (those whose
   * status changed) and of the details, read by the real service (docs/design/e2e-testing.md#remote-services). The
   * `url` of a robot is the one the client derives from its id; an empty `sourceRevision` is the tip of the robot's
   * branch when Patronus answers, `~N` its Nth first parent
   */
  async setPatronus(result: PatronusRobotsResult): Promise<void> {
    await this.setFixture("patronus", result)
  }

  /**
   * Sets the estimate of the CI that Patronus folds for running robots (`robots/progress`, read by the details with the
   * new API of Settings → Patronus), by robot id, as of now
   */
  async setPatronusProgress(progress: Record<string, PatronusRobotProgress>): Promise<void> {
    await this.setFixture("patronus-progress", progress)
  }

  /**
   * Sets the merge profiles the fake Patronus offers for a start (`robot-start-supported`), the regular and the
   * emergency ones apart, as `PatronusStartOptions` names them; none until set. The default profile is the one of the
   * branch's last robot (its `profile`), as the real service takes it
   */
  async setPatronusProfiles(fixture: { profiles?: PatronusProfile[]; emergencyProfiles?: PatronusProfile[] }): Promise<void> {
    await this.setFixture("patronus-profiles", fixture)
  }

  /**
   * Makes the fake Patronus refuse every start from now on with `description` (HTTP 400, the message the app shows), or
   * start again with `null`; a refused start is logged in `getPatronusLog().starts` as any other and lists no robot
   */
  async refusePatronusStarts(description: string | null): Promise<void> {
    await this.setFixture("patronus-start-refusal", { description })
  }

  /**
   * Sets the TeamCity checks and the problems the fake Patronus answers for robots, by robot id, and the failed builds of
   * check attempts, by attempt id, in its wire form (`teamCityChecks`, `problems`, `builds`); none without them
   */
  async setPatronusDetails(fixture: { checks?: Record<string, unknown[]>; problems?: Record<string, unknown[]>; attemptBuilds?: Record<string, unknown[]> }): Promise<void> {
    await this.setFixture("patronus-details", fixture)
  }

  /** What the page asked of Patronus and what the fake Patronus got and did (`PatronusLog`) */
  async getPatronusLog(): Promise<PatronusLog> {
    return this.getFixture("patronus")
  }

  /**
   * Sets what the fake Patronus answers the pre-check of `get_branch_review_readiness` with, by branch (see
   * `patronus-helpers.ts`): a virtual branch by its name, the stack branch of a whole stack as `stack:<root>`, the
   * answer of the pre-check whose newest commit is that branch's tip; the real core judges it. A branch without an
   * answer: the owners of its files by the `OWNERSHIP` files of the target, as a Patronus that doesn't evaluate pull
   * requests. An answer Patronus could not give is refused
   */
  async setReviewAnswers(fixture: { answers: Record<string, PatronusReviewRequirement> }): Promise<void> {
    await this.setFixture("review-answers", fixture)
  }

  /** The page's `get_branch_review_readiness` calls: the branches each asked and whether it was a recheck */
  async getReviewReadinessCalls(): Promise<{ branches: string[]; recheck: boolean }[]> {
    return (await this.getReviewChecks()).calls
  }

  /** The page's readiness calls, and the pre-checks that reached the fake Patronus: the tip and the pull request of each */
  async getReviewChecks(): Promise<{ calls: { branches: string[]; recheck: boolean }[]; checks: { tip: string; pullRequest: { repository: string; number: number } | null }[] }> {
    return this.getFixture("review-answers")
  }

  /** How the Patronus bot answers a `/patronus` comment of a pull request, by its number; accepted for one not named */
  async setPatronusReplies(replies: Record<number, PatronusCommandReply>): Promise<void> {
    await this.setFixture("patronus-replies", { replies })
  }

  /**
   * The fake GitHub answers `method path` (without the query) with `status` and `body` from now on, before its state:
   * a failure no state of it gives, e.g. GitHub refusing a review
   */
  async answerGitHub(method: string, path: string, status: number, body: unknown): Promise<void> {
    await this.setFixture("github-answer", { method, path, status, body })
  }

  private async setFixture(name: string, value: unknown): Promise<void> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/${name}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(value),
    })
    if (!response.ok) {
      throw new Error(`Failed to set the ${name} fixture: ${response.status} ${await response.text()}`)
    }
  }

  private async getFixture<T>(name: string): Promise<T> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/${name}`)
    return (await response.json()) as T
  }

  async cleanup(): Promise<void> {
    if (!this.id) {
      return
    }

    try {
      // Delete the repository from the test server
      const response = await fetch(`${this.baseUrl}/repositories/${this.id}`, {
        method: "DELETE",
      })

      if (!response.ok) {
        console.warn(`Failed to cleanup repository ${this.id}: ${response.statusText}`)
      }
    } catch (error) {
      // Log but don't throw - cleanup errors shouldn't fail tests
      console.warn(`Failed to cleanup repository ${this.id}:`, error)
    }
  }
}
