import type {
  BranchNameSuggestionOutcome,
  Intent,
  IssueNavigationConfig,
  PatronusMergeRequest,
  PatronusProfile,
  PatronusReviewRequirement,
  PatronusRobot,
  PatronusRobotProgress,
  PlannedEffect,
  RemoteStatusUpdate,
  Resource,
  StackBranchUpdate,
} from "../../../src/lib/bindings"
import type { PatronusRobotsFixture } from "./patronus-helpers"
import type { GitHubFixture, GitHubLog } from "./pull-request-helpers"
import type { ReviewRequestsFixture } from "./review-request-helpers"
import { TEST_SERVER_URL } from "./test-server-url"

/** How the bot answers a `/patronus` comment (`PatronusCommandReply` of `domain_types::patronus`, which no command of the page answers) */
export type PatronusCommandReply = { readonly status: "pending" } | { readonly status: "accepted" } | { readonly status: "rejected"; readonly reason: string | null }

/**
 * An answer of a fake a test holds back (`HeldAnswer` of `remote_host.rs`): GitHub's search of the user's pull
 * requests, the review requests, the list of the robots, the start, the restart and the cancel of a robot, the
 * pre-check of the code-owner review, the answer of the
 * computation of the landing needs, why a robot failed (its problems, of every robot), the rules of a branch, the merge
 * profiles of the target, the view of the robots, the status of the sign-ins (the read of the sessions and of
 * Settings), the tools of Settings, a push to the remote of the repository (which has its pack then, and moves no ref until the release), the
 * creation of a pull request and a review request on GitHub, a sync run of the session (it runs until the release,
 * what waits for it is seen waiting), the configuration of the YouTrack that names its Hub (a sign-in in the browser
 * asks it first)
 */
export type HeldAnswer =
  | "pullRequests"
  | "reviewRequests"
  | "robots"
  | "start"
  | "restart"
  | "cancel"
  | "readiness"
  | "landingNeeds"
  | "robotProblems"
  | "branchRules"
  | "profiles"
  | "repositoryView"
  | "authStatus"
  | "cliTools"
  | "push"
  | "createPullRequest"
  | "requestReviewers"
  | "submitReview"
  | "sync"
  | "youTrackConfig"

/** A commit someone else pushed onto a branch of the remote: its author (`Name <email>`), time and files */
export interface RemoteCommit {
  subject: string
  author: string
  timestamp: number
  files: Record<string, string>
}
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
/**
 * A remote status as a test sets it: its counts, never the action of a push, which the server derives from them as the
 * app's sync does (`domain_policy::push::action`, `test_remote_status` of the server)
 */
export type TestRemoteStatus = Omit<RemoteStatusUpdate, "action">

export interface SyncOverrides {
  /**
   * Branch -> the fields of its remote status that replace the computed ones, and `keepUnpushed`: only the newest that
   * many of the commits the sync found the remote lacking stay unpushed (their ids come with the sync); null: none, its
   * status stays as it was
   */
  remote?: Record<string, (Partial<TestRemoteStatus> & { keepUnpushed?: number }) | null>
  /**
   * Stack root -> the fields of its stack branch that replace the reported ones (`remote` as a whole, its counts: the
   * server derives its action)
   */
  stacks?: Record<string, Partial<Omit<StackBranchUpdate, "remote"> & { remote: TestRemoteStatus | null }>>
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

  /**
   * Updates the checked out branch onto the fetched baseline behind the app's back, as `bd update` beside the app does:
   * the core of Update Project, called by no page, so only the next sync of the page learns what it landed
   */
  async updateOutsideTheApp(): Promise<void> {
    await this.setFixture("update-outside", {})
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
   * The branches of the remote of the repository (`.git/upstream.git`, its `origin`), by name without `refs/heads/`:
   * what the app's pushes left there (docs/design/e2e-testing.md#the-remote-of-a-repository)
   */
  async getRemoteBranches(): Promise<Record<string, string>> {
    return (await this.getFixture<{ branches: Record<string, string> }>("remote")).branches
  }

  /**
   * The pushes of the app the remote took, in order (its `post-receive` hook logs each): the branches each moved, by
   * name without `refs/heads/`, to their commits, null for a deleted one. A push in a terminal is not one, and a
   * refused push moved nothing
   */
  async getRemotePushes(): Promise<Record<string, string | null>[]> {
    return (await this.getFixture<{ pushes: Record<string, string | null>[] }>("remote")).pushes
  }

  /** Fetches the remote as `git fetch origin` in a terminal does: the remote-tracking refs move to what it has */
  async fetchInTerminal(): Promise<void> {
    await this.setFixture("remote/fetch", {})
  }

  /**
   * Pushes `revision` (a ref or a commit of the repository, `branch` by default) to `branch` of the remote behind the
   * app's back, as `git push --force origin <revision>:<branch>` in a terminal: the remote branch and the
   * remote-tracking ref move, so the next sync shows the branch pushed. With `earlierBuild`, a copy of the commits of
   * `revision` after `origin/master` instead, each committed a day earlier: the remote has the commits by content
   * under other ids, as after the sync rebuilt a pushed branch. Returns the commit pushed
   */
  async pushInTerminal(branch: string, { revision = branch, earlierBuild = false }: { revision?: string; earlierBuild?: boolean } = {}): Promise<string> {
    return this.postForCommit("remote/push", { branch, revision, earlierBuild })
  }

  /**
   * Writes `commit` onto `branch` of the remote, as someone else's push does: the remote-tracking ref stays where it
   * was, so the app finds the branch moved when it pushes (or fetches). Returns the new commit
   */
  async commitOnRemote(branch: string, commit: RemoteCommit): Promise<string> {
    return this.postForCommit("remote/commit", { branch, ...commit, files: Object.entries(commit.files) })
  }

  /**
   * Names the route of the repository into master (`branchdeck.landing` of its own config; null unsets it) behind the
   * app's back, as `git config --local` in a terminal does: the next sync reads it. With `gitHub` (`owner/name`),
   * `origin` pushes to that GitHub repository by its URL while every push still reaches the remote, so the gate of a
   * push into the target reads the facts of the fake GitHub (permission, rules)
   */
  async setLanding(setting: "patronus" | "push" | "pull-request" | null, { gitHub }: { gitHub?: string } = {}): Promise<void> {
    await this.setFixture("landing", { setting, ...(gitHub !== undefined && { gitHub }) })
  }

  /** `branchdeck.landing` of the repository's own config as git keeps it (`push`, `pull-request`), null unset */
  async getLanding(): Promise<string | null> {
    return (await this.getFixture<{ setting: string | null }>("landing")).setting
  }

  /** The remote refuses every push from now on with `message` (git shows it as `remote: <message>`), or takes them (`null`) */
  async refuseRemotePushes(message: string | null): Promise<void> {
    await this.setFixture("remote/refusal", { message })
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
   * Sets what GitHub answers the session of the review requests from now on (E2E never runs gh); `fetchError` fails
   * the review fetch (the intent `fetchReview`) with that message
   */
  async setReviewRequests(result: ReviewRequestsFixture, fetchError?: string): Promise<void> {
    await this.setFixture("review-requests", fetchError === undefined ? result : { ...result, fetchError })
  }

  /** How often the session listed the review requests */
  async getReviewRequestCalls(): Promise<number> {
    return (await this.getReviewRequestsFixture()).calls
  }

  /** How often the session read the rest of Reviews with the review requests (the Reviews view showed) */
  async getReviewsCalls(): Promise<number> {
    return (await this.getReviewRequestsFixture()).reviewsCalls
  }

  /**
   * The calls of the review commands: the requests (of them with Reviews), the fetches, and the `keep` of each prune of
   * the review refs (the session's, at each change of the review windows)
   */
  async getReviewRequestsFixture(): Promise<{ calls: number; reviewsCalls: number; fetchCalls: number; prunes: number[][] }> {
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
   * new API of Settings → Landing), by robot id, as of now
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
   * Sets what the fake Patronus answers the pre-check of the readiness reads of the session with, by branch (see
   * `patronus-helpers.ts`): a virtual branch by its name, the stack branch of a whole stack as `stack:<root>`, the
   * answer of the pre-check whose newest commit is that branch's tip; the real core judges it. A branch without an
   * answer: the owners of its files by the `OWNERSHIP` files of the target, as a Patronus that doesn't evaluate pull
   * requests. An answer Patronus could not give is refused. `mergeRequest`: the Space merge request found open for
   * every branch, whose approvals a repository only on Space counts, with what the pre-check answers asked with its
   * review (as `answers`; a branch without one answers as without the review)
   */
  async setReviewAnswers(fixture: {
    answers: Record<string, PatronusReviewRequirement>
    mergeRequest?: { request: PatronusMergeRequest; answers: Record<string, PatronusReviewRequirement> }
  }): Promise<void> {
    await this.setFixture("review-answers", fixture)
  }

  /** The readiness reads of the sessions: the subjects each asked and whether it was a recheck */
  async getReviewReadinessCalls(): Promise<{ branches: string[]; recheck: boolean }[]> {
    return (await this.getReviewChecks()).calls
  }

  /** The readiness reads of the sessions, and the pre-checks that reached the fake Patronus: the tip and the pull request of each */
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
   * The folder the native dialog of the app picks from now on (`browse_repository`, the Browse button): `path`, or
   * `null`, the user cancels; without the fixture it picks the repository's own path
   */
  async setDialogPick(path: string | null): Promise<void> {
    await this.setFixture("dialog-pick", { path })
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

  /**
   * Moves the clock of the backend's sessions of this repository by `ms` (`POST …/clock/advance`): the timers that
   * decide what its sessions ask and when (their polls, their retries, the sweep of a session no window shows) wake as
   * if that much time went by. The sessions of the other tests, the page's own timers (`page.clock`) and the 16 ms merge
   * of the patches do not move with it: a test waits for the patch the advance causes (its DOM state), never for time
   */
  async advanceBackendClock(ms: number): Promise<void> {
    await this.setFixture("clock/advance", { ms })
  }

  /**
   * The app quits: the sessions of the repository go with the subscriptions of their windows (`POST …/sessions/close`),
   * so a reload opens a new session whose facts are asked anew, as the next start of the app does
   */
  async closeSessions(): Promise<void> {
    await this.setFixture("sessions/close", {})
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
    await this.setFixture("locks/repository", { held })
  }

  /**
   * Takes the fetch lock of the repository (`held`) or releases it, as an agent's `bd update` that fetches the baseline
   * holds it: the fetch of an update or a Fetch of the page waits for it in the backend, then fetches itself
   */
  async holdFetchLock(held: boolean): Promise<void> {
    await this.setFixture("locks/fetch", { held })
  }

  /**
   * `bd` of another process runs `intent` in the repository (an agent's `bd safe-push`): its record in the operations
   * registry of the repository, naming a child of the test server that runs until the next call; the app's sessions show
   * it from the watch of the registry and refuse what needs what it holds (by default what a start or a push of its branch
   * holds). `unknown`: a start of it whose answer was lost, sent now on the clock of the sessions of the repository, which
   * no read of Patronus settles until the test moves that clock past it: through the API, or as the `/patronus` comment
   * `comment` on the pull request `number` at `url`. `null` ends the one before. Answers its operation and pid
   */
  async setForeignOperation(
    intent: Intent | null,
    resources?: Resource[],
    unknown = false,
    comment?: { readonly number: number; readonly url: string },
  ): Promise<{ op: string; pid: number } | null> {
    const response = await this.postFixture("foreign-operation", { intent, unknown, ...(resources !== undefined && { resources }), ...(comment !== undefined && { comment }) })
    return (await response.json()) as { op: string; pid: number } | null
  }

  /**
   * An agent's `bd` ran `intent` in a slot of the lease pool of the repository and it ended with each of `effects` done:
   * the journal of the repository has its entry, an agent's by the state of the pool, which the page reads at its next
   * read of Activity. Answers its operation and the worktree of its slot
   */
  async journalAgentOperation(intent: Intent, effects: PlannedEffect[]): Promise<{ op: string; worktree: string }> {
    return (await (await this.postFixture("agent-operation", { intent, effects })).json()) as { op: string; worktree: string }
  }

  /**
   * The disk of the journal of the repository is gone, as a removed one is: the settings remember its journal at a path
   * on no disk, until a window subscribes to the session again. Answers that path
   */
  async loseJournalDisk(): Promise<string> {
    return ((await (await this.postFixture("journal-gone", {})).json()) as { journal: string }).journal
  }

  /** `bd` did a mutation of the repository behind the app's back (an agent's `bd group`): it touched the stamp of the repository */
  async touchStamp(): Promise<void> {
    await this.setFixture("stamp", {})
  }

  private async setFixture(name: string, value: unknown): Promise<void> {
    await this.postFixture(name, value)
  }

  /** POSTs `value` to the fixture `name`, which answers the commit it wrote or pushed */
  private async postForCommit(name: string, value: unknown): Promise<string> {
    return ((await (await this.postFixture(name, value)).json()) as { commitId: string }).commitId
  }

  /** POSTs `value` to the fixture `name` of the repository, which must answer 2xx: the response, whose JSON the caller reads */
  private async postFixture(name: string, value: unknown): Promise<Response> {
    const response = await fetch(`${this.baseUrl}/repositories/${this.id}/${name}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(value),
    })
    if (!response.ok) {
      throw new Error(`Failed to set the ${name} fixture: ${response.status} ${await response.text()}`)
    }
    return response
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
