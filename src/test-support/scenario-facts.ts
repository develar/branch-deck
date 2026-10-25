// The app's side of a scenario case (`crates/domain-policy/cases/README.md`): the stores seeded as the sync, the
// repository view, Patronus, GitHub and the review scan leave them (`seedScenario`), then the app's own readers answer:
// `getBranchNextStepOf` and `getStackNextStepNow` for the step, `readSafePush` for the decision. What the Rust core
// computes of the case's repository (the tips, the robots, the repository view, the review readiness) comes from
// `cases/facts/<id>.json`, which the Rust scenario test writes from the same `given`; the adapter maps the rest of
// `given` to store-shaped facts (the table of the sync, the landing report, the pull requests, the rules: the README
// names what it still models) and decides nothing of either side. A value the README doesn't map fails with "not
// mapped yet"
import { getBranchNextStepOf } from "@/hooks/useBranchNextStep"
import type { SafePushRead } from "@/hooks/useSafePush"
import { readSafePush } from "@/hooks/useSafePush"
import { getStackNextStepNow } from "@/hooks/useStackNextStep"
import { internalError } from "@/lib/api-error"
import type { BranchRules, Fact, PatronusProfile, PatronusReviewRequirement, PatronusRobot, PatronusStartOptions, PatronusView, PullRequest, ReviewReadiness } from "@/lib/bindings"
import type { BranchNextStep } from "@/lib/branch-next-step"
import { waitingReviewers } from "@/lib/pull-request"
import { robotsById } from "@/lib/test-robot"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { landedStore } from "@/stores/landed"
import { patronusStore } from "@/stores/patronus"
import { pullRequestsStore } from "@/stores/pull-requests"
import type { ReadinessEntry } from "@/stores/review-readiness"
import { readinessInputOf, reviewReadinessStore } from "@/stores/review-readiness"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"
import { testPatronusRepository } from "@/test-support/patronus-repository"
import type { Given, GivenRobot } from "@/test-support/scenario-case"
import { robotsOf } from "@/test-support/scenario-case"

/** The selected repository and its prefix (`scenario-repository-mock.ts`), the key of the stores of Patronus and GitHub */
const REPOSITORY_PATH = "/repo"
const PREFIX = "test-user"
const KEY = `${REPOSITORY_PATH}\n${PREFIX}`
const TARGET = "master"
const BASELINE = "origin/master"
const GITHUB_REPOSITORY = "acme/monorepo"
/** The commit of a conflict of the stack branch, as the sync names it */
const CONFLICT_COMMIT = "0caf9066d2a4c3b1e5f7a9b8c6d4e2f1a3b5c7d9"

/** The command that rewrites the facts files */
export const UPDATE_FACTS = "UPDATE_SCENARIO_FACTS=1 cargo nextest run -p bd-remote -E 'test(/scenario/)'"

/** An answer of the review scan as the facts hold it: what Rust answered for a key of the review-readiness store */
interface ReadinessFact {
  readonly readiness: ReviewReadiness
  readonly requirement: PatronusReviewRequirement | null
  readonly tip: string
  readonly pullRequest: number | null
}

/**
 * What the Rust core computes of the case's repository, written by the group tests of `crates/bd-remote/src/scenario_test.rs`
 * (`assert_facts_kept`) from the fixtures of the same `given`: the tips of the virtual branches, of
 * their remote branches and of the stack branches (`<branch>-tip`, `<branch>-remote`, `stack-<root>-tip`), the robots
 * as the list answers them, the repository view (`patronus_view` with the landing needs) and the review readiness of
 * the branch and of its whole stack, by the keys of the review-readiness store
 */
export interface ScenarioFacts {
  /** `given` the facts were computed from, as the case file has it */
  readonly given: unknown
  readonly tips: Readonly<Record<string, string>>
  readonly remoteTips: Readonly<Record<string, string>>
  readonly stackTips: Readonly<Record<string, string>>
  readonly robots: readonly PatronusRobot[]
  readonly view: PatronusView
  readonly readiness: Readonly<Record<string, ReadinessFact>>
}

const FACTS_DIRECTORY = "/crates/domain-policy/cases/facts"
const factsFiles = import.meta.glob<ScenarioFacts>("/crates/domain-policy/cases/facts/*.json", { eager: true, import: "default" })

/** The facts of the case `id`; a case without its file fails: the Rust test writes it */
export function factsOf(id: string): ScenarioFacts {
  const facts = factsFiles[`${FACTS_DIRECTORY}/${id}.json`]
  if (facts === undefined) {
    throw new Error(`no facts of ${id}: ${UPDATE_FACTS} writes them`)
  }
  return facts
}

function notMapped(what: string): never {
  throw new Error(`given.${what} is not mapped yet`)
}

/** The tip of `name` as the sync built it, as the facts name it */
function tipOf(facts: ScenarioFacts, name: string): string {
  return facts.tips[name] ?? `${name}-tip`
}

// ---- the branches of the commits ----

/** The branch of a commit and the parent it declares: `(auth/login) …` is login on auth */
function groupOf(subject: string): { name: string; parent: string | null } {
  const prefix = /^\(([^)]+)\)/.exec(subject)![1]!
  const slash = prefix.lastIndexOf("/")
  return slash === -1 ? { name: prefix, parent: null } : { name: prefix.slice(slash + 1), parent: prefix.slice(0, slash) }
}

/** The commits a dangling reference fixture makes (`commit_dangling_reference`, `commit_changed_declaration`) */
const DANGLING_COMMITS = {
  removed: ["(fake-agent) IJAI-3880 Move the control constants", "(lane-document) IJAI-3878 Use the control protocol"],
  changed: ["(journey) IJAI-4182 The journey support goes with its tests", "(suspend) IJAI-4185 The cleanups may suspend"],
} as const

/** The branches of the commits in the order of their first commit, each with its parent and its commit count */
function branchesOf(given: Given): Map<string, { parent: string | null; commits: number }> {
  const branches = new Map<string, { parent: string | null; commits: number }>()
  for (const subject of given.dangling === undefined ? given.commits : DANGLING_COMMITS[given.dangling]) {
    const { name, parent } = groupOf(subject)
    const known = branches.get(name)
    branches.set(name, { parent: known?.parent ?? parent, commits: (known?.commits ?? 0) + 1 })
  }
  // `commits: []`: the branch is in the table without commits (every commit of it moved away); bd knows no such branch
  if (branches.size === 0) {
    branches.set(given.branch, { parent: null, commits: 0 })
  }
  return branches
}

/** The branches `name` is built on, its root first */
function ancestorsOf(given: Given, name: string): string[] {
  const branches = branchesOf(given)
  const ancestors: string[] = []
  for (let parent = branches.get(name)?.parent ?? null; parent !== null && !ancestors.includes(parent); parent = branches.get(parent)?.parent ?? null) {
    ancestors.unshift(parent)
  }
  return ancestors
}

/** The branches of the stack of `root`, the root first, each after its parent */
function membersOf(given: Given, root: string): string[] {
  return [...branchesOf(given).keys()].filter(name => name === root || ancestorsOf(given, name).includes(root))
}

/** The stack of `root` forks: a branch of it has two children, so the sync builds its stack branch */
function isForking(given: Given, root: string): boolean {
  const members = membersOf(given, root)
  const branches = branchesOf(given)
  return members.some(member => members.filter(name => branches.get(name)?.parent === member).length > 1)
}

function rootOf(given: Given, name: string): string {
  return ancestorsOf(given, name)[0] ?? name
}

/** The branch `given.parked` names: the case's branch (`self`), its parent (`builtOn`), or a branch of the stack */
function parkedBranchOf(given: Given): string | undefined {
  return given.parked === "self" ? given.branch : given.parked === "builtOn" ? ancestorsOf(given, given.branch).at(-1) : given.parked
}

// ---- the table of the sync ----

/**
 * The remote status of `name` as the sync computed it after what `given.remote` did: `bd push` pushes the stack, so the
 * ancestors of the branch are on the remote with it; a commit added since (`unpushed`) and a commit of someone else
 * (`behind`) are the branch's own; a stack rebuilt on a newer baseline has its commits by content on the remote
 */
function remoteStatusOf(given: Given, facts: ScenarioFacts, name: string): RemoteStatus {
  const own = name === given.branch
  const inStack = own || ancestorsOf(given, given.branch).includes(name)
  const commits = branchesOf(given).get(name)?.commits ?? 0
  const tip = tipOf(facts, name)
  const remoteTip = facts.remoteTips[name] ?? null
  const unpushed = testRemoteStatus({ exists: false, commitsAhead: commits, myCommitsAhead: commits, unpushedCommits: [tip], remoteTip: null })
  if (!inStack) {
    return unpushed
  }
  switch (given.remote) {
    case "none":
      return unpushed
    case "upToDate":
      return testRemoteStatus({ remoteTip })
    case "unpushed":
      return own ? testRemoteStatus({ remoteTip, commitsAhead: 1, myCommitsAhead: 1, unpushedCommits: [tip] }) : testRemoteStatus({ remoteTip })
    case "behind":
      return own
        ? testRemoteStatus({
            remoteTip,
            commitsBehind: 1,
            droppedCommits: [{ id: remoteTip ?? "theirs", subject: "Someone else", author: "Other", authorTime: 0 }],
            fastForward: false,
          })
        : testRemoteStatus({ remoteTip })
    case "rebuiltOnNewerBaseline":
      // the same commits by content, rebuilt on a newer baseline: not a fast-forward of the remote branch
      return testRemoteStatus({ remoteTip, fastForward: false })
  }
}

/** The branch `name` as the sync left it: its tip, its count, failed (`conflicts`) or blocked by the branch that did */
function branchOf(given: Given, facts: ScenarioFacts, name: string, parent: string | null, commits: number): Branch {
  const failed = given.conflicts === name
  const blockedBy = given.conflicts !== undefined && ancestorsOf(given, name).includes(given.conflicts) ? given.conflicts : null
  const status = given.sync === "running" ? "Syncing" : failed ? "Error" : blockedBy !== null ? "Blocked" : "Unchanged"
  const base = testBranch(name, parent)
  return testBranch(name, parent, {
    commits:
      commits === 0
        ? []
        : [{ originalHash: `${name}-c${commits - 1}`, hash: tipOf(facts, name), strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }],
    // the baseline landed the commits rebuilt (`byContent`): one is left
    commitCount: given.landed === "byContent" && name === given.branch ? 1 : commits,
    status,
    hasError: failed,
    remoteStatus: remoteStatusOf(given, facts, name),
    stack: { ...base.stack!, blockedBy: blockedBy === null ? null : { parent: blockedBy, reason: "Error" } },
  })
}

/** The stack branch of the stack of `root` as the sync left it */
function stackBranchOf(given: Given, facts: ScenarioFacts, root: string, members: string[]): StackBranch {
  const conflicts = given.conflicts !== undefined && members.includes(given.conflicts)
  const blocked = given.stackBranch === "notBuilt"
  return {
    root,
    branchName: `${PREFIX}/stack/${root}`,
    members,
    commitCount: members.reduce((count, member) => count + (branchesOf(given).get(member)?.commits ?? 0), 0),
    state: conflicts ? "Conflict" : blocked ? "Blocked" : "Built",
    tip: conflicts || blocked ? null : (facts.stackTips[root] ?? null),
    blockedBy: blocked ? { member: members.at(-1)!, reason: "Problem" } : null,
    conflict: conflicts ? { commit: CONFLICT_COMMIT, files: ["config.txt"] } : null,
    error: null,
    remoteStatus: null,
  }
}

/** The table of the sync: the branches, the stack branches of the stacks that fork, the parked notes, the baseline */
function seedSync(given: Given, facts: ScenarioFacts): void {
  const structure = branchesOf(given)
  const branches = Object.fromEntries([...structure].map(([name, { parent, commits }]) => [name, branchOf(given, facts, name, parent, commits)]))
  const roots = [...structure.keys()].filter(name => structure.get(name)?.parent === null)
  const stackBranches = Object.fromEntries(roots.filter(root => isForking(given, root)).map(root => [root, stackBranchOf(given, facts, root, membersOf(given, root))]))
  const parked = parkedBranchOf(given)
  branchSyncStore.setState({
    branches,
    branchOrder: [...structure.keys()],
    stacksEnabled: [...structure.values()].some(({ parent }) => parent !== null),
    baselineBranch: BASELINE,
    baselineStatus: { repositoryPath: REPOSITORY_PATH, baselineBranch: BASELINE, update: null, checkError: null },
    stackBranches,
    parked: parked === undefined ? {} : { [parked]: "" },
    isSyncing: given.sync === "running",
    pushingBranches: {},
    stackPushes: {},
  })
}

// ---- the robots and the repository view ----

/** The id the fixtures give the robot at `index` of `given.robot` (`add_robots` of the Rust side): `older` for the one on an older commit */
function robotIdOf(index: number, robot: GivenRobot): string {
  return robot.on === "older" ? "older" : `robot-${index}`
}

/**
 * Patronus as the store holds it: the robots as the list answers them and the repository view as `patronus_view`
 * computes it of them (the facts), the GitHub repository, whether each robot failed for the approvals (`needsReview`,
 * what the poll of a failed robot's problems says, by the robot's id: answered for every robot)
 */
function seedPatronus(given: Given, facts: ScenarioFacts): void {
  const needsReview = robotsOf(given).map((robot, index) => [robotIdOf(index, robot), robot.reason === "approvals"] as const)
  patronusStore.setState({
    repositories: {
      [KEY]: {
        ...testPatronusRepository(robotsById(...facts.robots), facts.view),
        availability: given.notOnSpace === true ? { status: "notPatronus" } : { status: "available" },
        githubRepository: given.readiness === "space" ? null : GITHUB_REPOSITORY,
        needsReview: Object.fromEntries(needsReview),
      },
    },
  })
}

// ---- GitHub, the review, the landing ----

function isListed(given: Given): boolean {
  return given.readiness !== "unlisted" && given.readiness !== "space"
}

/** The pull request #7 of `branch` as GitHub lists it: of `pullRequest.of` only, when it names a branch */
function pullRequestOf(given: Given, facts: ScenarioFacts, branch: string): PullRequest | undefined {
  const pullRequest = given.pullRequest
  if (pullRequest === undefined || (pullRequest.of !== undefined && pullRequest.of !== branch)) {
    return undefined
  }
  return {
    number: 7,
    title: "Add auth",
    url: "https://github.com/acme/monorepo/pull/7",
    state: pullRequest.state,
    isDraft: pullRequest.draft,
    reviewDecision: null,
    headRefName: `${PREFIX}/virtual/${branch}`,
    baseRefName: pullRequest.base,
    // GitHub merged it with the commits the branch has
    headRefOid: pullRequest.state === "merged" ? tipOf(facts, branch) : "1111",
    checks: null,
    updatedAt: 0,
    reviewers: [
      ...pullRequest.approvers.map(login => ({ login, state: "approved" as const, requested: false })),
      ...pullRequest.changesRequested.map(login => ({ login, state: "changesRequested" as const, requested: false })),
      ...pullRequest.reviewers.map(login => ({ login, state: "requested" as const, requested: true })),
    ],
    author: "me",
  }
}

/** The open pull request #12 of the stack branch of `root` */
function stackPullRequestOf(facts: ScenarioFacts, root: string): PullRequest {
  return {
    number: 12,
    title: `The whole ${root} stack`,
    url: "https://github.com/acme/monorepo/pull/12",
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: `${PREFIX}/stack/${root}`,
    baseRefName: TARGET,
    headRefOid: facts.stackTips[root] ?? "stack-tip",
    checks: null,
    updatedAt: 0,
    reviewers: [],
    author: "me",
  }
}

/** GitHub as the store holds it: the pull requests of the branches and of the stack branches, or why none is listed */
function seedGitHub(given: Given, facts: ScenarioFacts): void {
  const listed = isListed(given)
  const branch = given.pullRequest?.of ?? given.branch
  const pullRequest = listed ? pullRequestOf(given, facts, branch) : undefined
  const root = rootOf(given, given.branch)
  pullRequestsStore.setState({
    repositories: {
      [KEY]: {
        availability:
          given.readiness === "space"
            ? { status: "notGitHub" }
            : given.readiness === "unlisted"
              ? { status: "error", message: "GitHub answered 500" }
              : { status: "available", owner: "acme", name: "monorepo" },
        pullRequests: pullRequest === undefined ? {} : { [branch]: pullRequest },
        stackPullRequests: given.stackPullRequest === true && listed ? { [root]: stackPullRequestOf(facts, root) } : {},
        fetchedAt: 1,
        // the error of an unlisted case is GitHub's first answer: no listing before it
        listed,
        isRefreshing: false,
      },
    },
  })
}

/**
 * The answers of the review scan as the facts hold them (Rust asked Patronus and GitHub for the branch, and for the
 * whole stack of its root while the sync built its stack branch), each under the key of the input the branch has now
 * (`readinessInputOf`, so a start reads them as fresh), with the reviewers waited for and approved as the scan derives
 * them of the input; none while the app hasn't asked (`notAsked`). Seeded after the branches and the pull requests,
 * which the input is computed from. An answer asked with another tip or start than the app computes now is refused:
 * the rule of the start lives in the app (`getPullRequestStart`), and Rust's facts follow it or the Vitest reddens
 */
export function seedReadiness(facts: ScenarioFacts): void {
  const entries: Record<string, ReadinessEntry> = {}
  for (const [key, answer] of Object.entries(facts.readiness)) {
    const input = readinessInputOf(key)
    const asked = { tip: answer.tip, pullRequest: answer.pullRequest }
    const asks = input === null ? null : { tip: input.branch.tip, pullRequest: input.branch.start.kind === "pullRequest" ? input.branch.start.number : null }
    if (input === null || asks === null || asks.tip !== asked.tip || asks.pullRequest !== asked.pullRequest) {
      throw new Error(
        `${key}: the facts asked another input (${JSON.stringify(asked)}) than the app asks now (${JSON.stringify(asks)}): the start rule changed on one side; ${UPDATE_FACTS} rewrites the facts`,
      )
    }
    const reviewers = input.branch.reviewers ?? []
    entries[key] = {
      key: input.key,
      readiness: answer.readiness,
      requirement: answer.requirement,
      stale: false,
      answeredAt: 0,
      tip: answer.tip,
      pullRequest: answer.pullRequest,
      waitingFor: waitingReviewers({ reviewers }),
      approvedBy: reviewers.filter(it => it.state === "approved").map(it => it.login),
    }
  }
  reviewReadinessStore.setState({ repositories: { [KEY]: entries } })
}

/** The landing report of the baseline check: `inBaseline` has every commit of the branch */
function seedLanded(given: Given): void {
  landedStore.setState({
    repositoryPath: REPOSITORY_PATH,
    landing: given.landed === "inBaseline" ? [{ name: given.branch, commits: [], remaining: 0, landedAt: 0 }] : [],
    landed: [],
    seenAt: null,
  })
}

/**
 * The rules of the repository as the session read them (`facts.rules` of the target): `keep` or `dismiss`, a failed
 * read for `unknown` (never "keeps the approvals": the start confirms `rulesUnknown`), `loading` has no answer yet
 */
function seedRules(given: Given): void {
  if (given.rules === "loading") {
    pullRequestsStore.setState({ rules: {} })
    return
  }
  const rules: Fact<BranchRules> =
    given.rules === "unknown"
      ? { value: null, state: { kind: "unavailable", reason: internalError("GitHub answered 500") }, seq: 1, observedAt: 1 }
      : { value: { dismissStaleReviewsOnPush: given.rules === "dismiss" }, state: { kind: "fresh" }, seq: 1, observedAt: 1 }
  pullRequestsStore.setState({ rules: { [KEY]: { [TARGET]: rules } } })
}

// ---- the answers ----

function checkMapped(given: Given): void {
  if (given.profile?.lastRobot !== undefined && given.robot === undefined) {
    throw new Error("given.profile.lastRobot needs given.robot")
  }
  if (given.profile?.skipsReview === true && given.profile.lastRobot === undefined) {
    throw new Error("given.profile.skipsReview needs given.profile.lastRobot: Patronus says it only of the last robot's profile")
  }
}

/** The profiles the fake Patronus answers (`robot-start-supported`), `lsc` as the last robot's options describe it */
function startOptions(given: Given): PatronusStartOptions {
  const profile = given.profile
  const lastRobot = profile?.lastRobot ?? null
  const lsc: PatronusProfile = {
    id: "lsc",
    name: "Large Scale Change",
    description: "All the tests",
    allowedPushModes: ["rebaseAutosquash", "dryRun"],
    skipsReview: lastRobot === "lsc" && profile?.skipsReview === true ? true : null,
  }
  return {
    targetBranch: TARGET,
    supported: given.target === "protected",
    profiles: [{ id: "default", name: "Default", description: null, allowedPushModes: [], skipsReview: null }, lsc],
    emergencyProfiles: [{ id: "emergency", name: "Emergency", description: null, allowedPushModes: [], skipsReview: true }],
    defaultProfileId: lastRobot,
  }
}

/** Every store as the situation of the case `id` leaves it, with its facts; the next read of the app answers for it */
export async function seedScenario(id: string, given: Given): Promise<void> {
  checkMapped(given)
  const facts = factsOf(id)
  seedSync(given, facts)
  seedPatronus(given, facts)
  seedGitHub(given, facts)
  seedReadiness(facts)
  seedLanded(given)
  seedRules(given)
}

/**
 * The step of the case as the row shows it: of its branch (`getBranchNextStepOf`), or of the whole stack of its root
 * (`getStackNextStepNow`: of the stack branch while the stack forks, else of the last branch of the straight chain)
 */
export function nextStepOf(given: Given): BranchNextStep {
  if (given.subject === "branch") {
    return getBranchNextStepOf(getBranch(given.branch) ?? notMapped(`branch ${given.branch}: no branch of the commits`))
  }
  const root = rootOf(given, given.branch)
  return (getStackNextStepNow(root) ?? notMapped(`subject wholeStack of ${root}: a stack of one branch has no whole stack`)).step
}

/**
 * The start of the case as a click reads it (`readSafePush`): its facts, its decision and what it would run, with the
 * merge profiles as the fake Patronus answers them and the profile the case chose
 */
export function safePushReadOf(given: Given): SafePushRead {
  if (given.subject !== "branch") {
    notMapped("subject wholeStack of a decision")
  }
  const chosen = given.profile?.chosen
  const read = readSafePush(
    given.branch,
    {
      mode: given.mode === "dryRun" ? "dryRun" : "rebaseAutosquash",
      via: given.via,
      profileId: chosen === undefined ? undefined : chosen === "none" ? null : chosen,
      bypassReview: given.flags.includes("withoutApprovals"),
    },
    { status: "loaded", options: startOptions(given) },
  )
  return read ?? notMapped(`branch ${given.branch}: no branch of the commits`)
}
