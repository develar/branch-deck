import type {
  BranchNameSuggestionOutcome,
  IssueNavigationConfig,
  PatronusCommandReply,
  PatronusProfile,
  PatronusReviewRequirement,
  PatronusRobot,
  PatronusRobotProgress,
  RemoteStatusUpdate,
  ReviewRequestsResult,
  StackBranchUpdate,
} from "../../../src/lib/bindings"
import type { PatronusRobotsFixture } from "./patronus-helpers"
import type { GitHubFixture, GitHubLog } from "./pull-request-helpers"
import { TEST_SERVER_URL } from "./test-server-url"

/**
 * An answer of a fake a test holds back (`HeldAnswer` of `remote_host.rs`): GitHub's search of the user's pull
 * requests, the review requests, the list of the robots, the pre-check of the code-owner review, the answer of the
 * computation of the landing needs, why a robot failed (its problems, of every robot), the rules of a branch, the view
 * of the robots, the statuses of Settings (the GitHub and the YouTrack sign-in, the tools)
 */
export type HeldAnswer =
  | "pullRequests"
  | "reviewRequests"
  | "robots"
  | "readiness"
  | "landingNeeds"
  | "robotProblems"
  | "branchRules"
  | "repositoryView"
  | "gitHubAuth"
  | "youTrackAuth"
  | "cliTools"
import type { YouTrackFixture, YouTrackLog } from "./youtrack-helpers"

/** What reached the fake Patronus: the requests by kind, the robots whose problems and progress were asked, in order */
export type PatronusRequests = {
  list: number
  updates: number
  details: number
  problems: string[]
  progress: string[]
  startOptions: number
  /** The query of each list request, in order */
  listQueries: Record<string, string>[]
  /** The robots whose TeamCity checks were asked, in order */
  checks: string[]
}

/** A robot of the history the statistics count (`HistoryRobotFixture` of the test server): owner `1` is the user */
export type HistoryRobotFixture = {
  id: string
  owner: string
  status: "success" | "failure" | "canceled" | "running"
  startTime: number
  finishTime: number | null
  /** The virtual branch of the user's robot */
  branch?: string
  dryRun?: boolean
  sourceRobotId?: string
}

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
 * What the app asked of Patronus (the lists and polls of the session, the page's commands for the details, which the
 * caches of the service may answer), what reached the fake (`requests`), and the actions in order: the start requests,
 * the robots restarted, canceled and whose status was read
 */
export type PatronusLog = {
  listCalls: number
  pollCalls: number
  detailCalls: number
  /** By id, the robots the session's lists and polls answered last (a fixture's empty revision resolved by the fake) */
  robots: Record<string, PatronusRobot>
  requests: PatronusRequests
  starts: PatronusStartRequest[]
  restarts: string[]
  cancels: string[]
  statusReads: string[]
}

/** `POST /repositories/{id}/cli-tools` */
export interface CliToolsFixture {
  installed?: { tool: "teamcity" | "youtrack"; version: string; unmanaged?: boolean }[]
  brew?: boolean
  latest?: Partial<Record<"teamcity" | "youtrack", string>>
  files?: Record<string, string>
}

/**
 * What the next sync of the repository reports differently (`POST /repositories/{id}/sync-overrides`, which the run of
 * its session that starts next takes, `crates/test-server/src/sync_overrides.rs`): what the server can't compute from a
 * template, which has no remote of its own. The table of the session, its patches and the table Rust keeps for the
 * checks are what a sync of such a repository makes; the syncs after it are the server's again, unless `every`
 */
export interface SyncOverrides {
  /**
   * Branch -> the fields of its remote status that replace the computed ones, and `keepUnpushed`: only the newest that
   * many of the commits the sync found the remote lacking stay unpushed (their ids come with the sync); null: none, its
   * status stays as it was
   */
  remote?: Record<string, (Partial<RemoteStatusUpdate> & { keepUnpushed?: number }) | null>
  /** Stack root -> the fields of its stack branch that replace the reported ones (`remote` as a whole) */
  stacks?: Record<string, Partial<StackBranchUpdate>>
  /** The issue links the sync reports instead of the repository's (`.idea/vcs.xml`) */
  issueLinks?: IssueNavigationConfig
  /** Every sync from now on reports them, not only the next: a test of several starts, until it sets others */
  every?: boolean
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

  /** The next sync of the repository reports `overrides` (`SyncOverrides`; each sync, with `every`), replacing the ones no sync took yet */
  async setSyncOverrides(overrides: SyncOverrides): Promise<void> {
    await this.setFixture("sync-overrides", overrides)
  }

  /**
   * Sets `branchdeck.branchPrefix` in `.git/config` behind the app's back, as `git config` in a terminal does: no command
   * of the app knows of it until one reads the config again
   */
  async setBranchPrefixInTerminal(prefix: string): Promise<void> {
    await this.setFixture("branch-prefix", { prefix })
  }

  /** Leaves the repository as an interrupted rebase does (`.git/rebase-merge`), or takes it back: a sync is deferred */
  async setRebaseInProgress(inProgress: boolean): Promise<void> {
    await this.setFixture("rebase-in-progress", { inProgress })
  }

  /** The commit a ref of the repository is at, e.g. the copy of a branch a sync built (`user-name/virtual/<branch>`) */
  async revision(name: string): Promise<string> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/revision?name=${encodeURIComponent(name)}`)
    if (!response.ok) {
      throw new Error(`Failed to resolve ${name}: ${response.status}`)
    }
    return ((await response.json()) as { id: string }).id
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

  /**
   * What the fake YouTrack of the repository holds from now on (`YouTrackFixture`), read and written by the real service
   * and sign-in session (docs/design/e2e-testing.md#remote-services); a field left out changes nothing
   */
  async setYouTrack(fixture: YouTrackFixture): Promise<void> {
    await this.setFixture("youtrack", fixture)
  }

  /** The URL of the fake YouTrack, whether a sign-in is in progress, and what reached the fake */
  async getYouTrack(): Promise<YouTrackLog> {
    return this.getFixture<YouTrackLog>("youtrack")
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
  async setPatronus(result: PatronusRobotsFixture): Promise<void> {
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

  /** Adds the robots of the history the Safe Push statistics count to the list of the fake Patronus, after `setPatronus` */
  async setPatronusHistory(robots: HistoryRobotFixture[]): Promise<void> {
    await this.setFixture("patronus-history", { robots })
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

  /**
   * Sets the version a check for updates finds, downloaded and ready to install (the install fails on the test server,
   * which has no updater), or `null`: the app is up to date, as without the fixture
   */
  async setAvailableUpdate(availableVersion: string | null): Promise<void> {
    await this.setFixture("update", { availableVersion })
  }

  /**
   * The fake `teamcity` and `youtrack` CLIs and `brew` of the repository (`cli_tools_fixtures.rs`): the tools installed,
   * whether the machine has Homebrew, the newest release of each, and state files of the fakes (`teamcity.auth`,
   * `youtrack.auth`: their `auth status --json`; `teamcity.skill`: the bundled skill; `brew.fail`: brew's error)
   */
  async setCliTools(fixture: CliToolsFixture): Promise<void> {
    await this.setFixture("cli-tools", fixture)
  }

  /** What the fakes of the CLIs were asked, in order: `brew install jetbrains/utils/youtrack`, `teamcity --version` */
  async getCliToolsLog(): Promise<string[]> {
    return (await this.getFixture<{ log: string[] }>("cli-tools")).log
  }

  /**
   * Holds the answers `hold` of the fakes from now on, fails `fail` and releases `release`
   * (`/remote-gates`): a held request of the page reaches its fake and waits there, so a test sees the page before the
   * answer, then after it; a released failing one answers again. The deletion of the repository releases what is still
   * held
   */
  async holdRemoteAnswers({ hold = [], fail = [], release = [] }: { hold?: HeldAnswer[]; fail?: HeldAnswer[]; release?: HeldAnswer[] }): Promise<void> {
    await this.setFixture("remote-gates", { hold, fail, release })
  }

  /** The held answers a request of the page reached and waits at: the page asked, the answer is still to come */
  async getArrivedAnswers(): Promise<HeldAnswer[]> {
    return (await this.getFixture<{ arrived: HeldAnswer[] }>("remote-gates")).arrived
  }

  /**
   * Takes the repository lock of the repository (`held`) or releases it, as another process holds it (`bd pick
   * --follow` of an agent): its holder record names the test server, `test-server`, alive while it runs
   */
  async holdRepositoryLock(held: boolean): Promise<void> {
    await this.setFixture("repository-lock", { held })
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
