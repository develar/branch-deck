// The app's side of a scenario case (`crates/domain-policy/cases/README.md`): `given` seeded into the stores as the
// sync, the repository view, Patronus, GitHub and the review scan leave them (`seedScenario`), then the app's own
// readers answer: `getBranchNextStepOf` and `getStackNextStepNow` for the step, `readSafePush` for the decision. The
// adapter maps `given` to store-shaped facts and computes no rule of the app; a value the README doesn't map fails
// with "not mapped yet"
import { vi } from "vite-plus/test"
import { getBranchNextStepOf } from "@/hooks/useBranchNextStep"
import { readDismissesStaleReviews, resetDismissesStaleReviews } from "@/hooks/useDismissesStaleReviews"
import type { SafePushRead } from "@/hooks/useSafePush"
import { readSafePush } from "@/hooks/useSafePush"
import { getStackNextStepNow } from "@/hooks/useStackNextStep"
import type {
  BranchPatronus,
  LandedByView,
  LandingRefusal,
  PatronusProfile,
  PatronusRobot,
  PatronusStartOptions,
  PatronusView,
  PullRequest,
  ReviewReadiness,
  RobotOf,
  SpanView,
  StackPatronus,
} from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { BranchNextStep } from "@/lib/branch-next-step"
import { group, ilya, missing } from "@/lib/test-next-step-facts"
import { robotsById, testRobot } from "@/lib/test-robot"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { landedStore } from "@/stores/landed"
import { patronusStore } from "@/stores/patronus"
import { pullRequestsStore } from "@/stores/pull-requests"
import type { ReadinessEntry } from "@/stores/review-readiness"
import { readinessInputOf, reviewReadinessStore } from "@/stores/review-readiness"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"
import { branchFacts, testPatronusRepository } from "@/test-support/patronus-repository"
import type { Given, GivenRobot } from "@/test-support/scenario-case"
import { robotsOf } from "@/test-support/scenario-case"

/** The selected repository and its prefix (`scenario-repository-mock.ts`), the key of the stores of Patronus and GitHub */
const REPOSITORY_PATH = "/repo"
const PREFIX = "test-user"
const KEY = `${REPOSITORY_PATH}\n${PREFIX}`
const TARGET = "master"
const BASELINE = "origin/master"
const GITHUB_REPOSITORY = "acme/monorepo"
/** The GitHub login of Ilya Korennoy, the member of IJPL Code Platform Patronus suggests */
const ILYA = "ikorennoy"
/** The commit of a conflict of the stack branch, as the sync names it */
const CONFLICT_COMMIT = "0caf9066d2a4c3b1e5f7a9b8c6d4e2f1a3b5c7d9"
/** The stack branch's tip as the sync built it */
const STACK_TIP = "stack-tip"

function notMapped(what: string): never {
  throw new Error(`given.${what} is not mapped yet`)
}

/** The tip of `name` as the sync built it */
function tipOf(name: string): string {
  return `${name}-tip`
}

/** The commit `bd push` pushed of `name`, which the remote branch has */
function pushedOf(name: string): string {
  return `${name}-pushed`
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
function remoteStatusOf(given: Given, name: string): RemoteStatus {
  const own = name === given.branch
  const inStack = own || ancestorsOf(given, given.branch).includes(name)
  const commits = branchesOf(given).get(name)?.commits ?? 0
  const unpushed = testRemoteStatus({ exists: false, commitsAhead: commits, myCommitsAhead: commits, unpushedCommits: [tipOf(name)], remoteTip: null })
  if (!inStack) {
    return unpushed
  }
  switch (given.remote) {
    case "none":
      return unpushed
    case "upToDate":
      return testRemoteStatus({ remoteTip: tipOf(name) })
    case "unpushed":
      return own
        ? testRemoteStatus({ remoteTip: pushedOf(name), commitsAhead: 1, myCommitsAhead: 1, unpushedCommits: [tipOf(name)] })
        : testRemoteStatus({ remoteTip: tipOf(name) })
    case "behind":
      return own
        ? testRemoteStatus({
            remoteTip: "theirs",
            commitsBehind: 1,
            droppedCommits: [{ id: "theirs", subject: "Someone else", author: "Other", authorTime: 0 }],
            fastForward: false,
          })
        : testRemoteStatus({ remoteTip: tipOf(name) })
    case "rebuiltOnNewerBaseline":
      // the same commits by content, rebuilt on a newer baseline: not a fast-forward of the remote branch
      return testRemoteStatus({ remoteTip: pushedOf(name), fastForward: false })
  }
}

/** The branch `name` as the sync left it: its tip, its count, failed (`conflicts`) or blocked by the branch that did */
function branchOf(given: Given, name: string, parent: string | null, commits: number): Branch {
  const failed = given.conflicts === name
  const blockedBy = given.conflicts !== undefined && ancestorsOf(given, name).includes(given.conflicts) ? given.conflicts : null
  const status = given.sync === "running" ? "Syncing" : failed ? "Error" : blockedBy !== null ? "Blocked" : "Unchanged"
  const base = testBranch(name, parent)
  return testBranch(name, parent, {
    commits: commits === 0 ? [] : [{ originalHash: `${name}-original`, hash: tipOf(name), strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }],
    // the baseline landed the commits rebuilt (`byContent`): one is left
    commitCount: given.landed === "byContent" && name === given.branch ? 1 : commits,
    status,
    hasError: failed,
    remoteStatus: remoteStatusOf(given, name),
    stack: { ...base.stack!, blockedBy: blockedBy === null ? null : { parent: blockedBy, reason: "Error" } },
  })
}

/** The stack branch of the stack of `root` as the sync left it */
function stackBranchOf(given: Given, root: string, members: string[]): StackBranch {
  const conflicts = given.conflicts !== undefined && members.includes(given.conflicts)
  const blocked = given.stackBranch === "notBuilt"
  return {
    root,
    branchName: `${PREFIX}/stack/${root}`,
    members,
    commitCount: members.reduce((count, member) => count + (branchesOf(given).get(member)?.commits ?? 0), 0),
    state: conflicts ? "Conflict" : blocked ? "Blocked" : "Built",
    tip: conflicts || blocked ? null : STACK_TIP,
    blockedBy: blocked ? { member: members.at(-1)!, reason: "Problem" } : null,
    conflict: conflicts ? { commit: CONFLICT_COMMIT, files: ["config.txt"] } : null,
    error: null,
    remoteStatus: null,
  }
}

/** The table of the sync: the branches, the stack branches of the stacks that fork, the parked notes, the baseline */
function seedSync(given: Given): void {
  const structure = branchesOf(given)
  const branches = Object.fromEntries([...structure].map(([name, { parent, commits }]) => [name, branchOf(given, name, parent, commits)]))
  const roots = [...structure.keys()].filter(name => structure.get(name)?.parent === null)
  const stackBranches = Object.fromEntries(roots.filter(root => isForking(given, root)).map(root => [root, stackBranchOf(given, root, membersOf(given, root))]))
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

/** A robot of `given` with the branch it ran on (null: the stack branch of the root) */
interface ScenarioRobot {
  readonly given: GivenRobot
  readonly robot: PatronusRobot
  readonly subject: string | null
}

/** The robots `given` describes: on the tip of the branch, an older commit (`0000`), its remote branch, another branch, the stack branch */
function robotsIn(given: Given): ScenarioRobot[] {
  const branch = given.branch
  const profile = given.profile
  return robotsOf(given).map((robot, index) => {
    const status = robot.state
    const finishTime = status === "running" ? null : 1704189600 + index
    const [where, other] = robot.on.split(":") as [string, string | undefined]
    const own = where === "tip" || where === "older" || where === "remote"
    const common: Partial<PatronusRobot> = {
      status,
      finishTime,
      pullRequest: robot.fromPullRequest ? { number: 7, url: "https://github.com/acme/monorepo/pull/7" } : null,
      // with the new API the list names the profile of the robot; the options of its start name the one of the last robot
      profile: own && profile?.newApi === true && profile.lastRobot === "lsc" ? { id: "lsc", name: "Large Scale Change", skipsReview: profile.skipsReview } : null,
    }
    const id = `robot-${index}`
    switch (where) {
      case "tip":
        return { given: robot, robot: testRobot(id, branch, common), subject: branch }
      case "older":
        return { given: robot, robot: testRobot(id, branch, { ...common, sourceRevision: "0000" }), subject: branch }
      case "remote":
        return { given: robot, robot: testRobot(id, branch, { ...common, sourceRevision: pushedOf(branch) }), subject: branch }
      case "stack":
        return { given: robot, robot: testRobot(id, null, { ...common, sourceRevision: STACK_TIP, stackRoot: rootOf(given, branch) }), subject: null }
      default:
        return { given: robot, robot: testRobot(id, other!, common), subject: other! }
    }
  })
}

/** What the robot ran on, as the view names it: its branch, or the root of the whole stack */
function robotOf(given: Given, robot: ScenarioRobot): RobotOf {
  return robot.subject === null
    ? { branch: rootOf(given, given.branch), robotId: robot.robot.id, wholeStack: true }
    : { branch: robot.subject, robotId: robot.robot.id, wholeStack: false }
}

/** The branches the robot lands (its merge landed): its branch and those it is built on, of the whole stack every member */
function landsOf(given: Given, robot: ScenarioRobot): string[] {
  return robot.subject === null ? membersOf(given, rootOf(given, given.branch)) : [...ancestorsOf(given, robot.subject), robot.subject]
}

/**
 * The facts of `name` as the repository view computes them of the robots (`patronus_view`): a merged robot that landed
 * it and those it is built on (of an older commit of the branch itself, its tip is the commit after); the span of a
 * running robot of a branch built on it, or of its whole stack; the hold: landed whole, else its own running robot,
 * else the span; the nearest ancestor whose own robot runs, or that a robot of a side branch lands. `start`, `dryRun`
 * and `startBlocked` are what `bd` reads of the view; the page decides them itself (`canStartRobot`)
 */
function branchViewOf(given: Given, name: string, robots: readonly ScenarioRobot[]): BranchPatronus {
  const running = robots.filter(robot => robot.given.state === "running")
  let landedBy: LandedByView | null = null
  for (const robot of robots.filter(robot => robot.given.state === "success" && landsOf(given, robot).includes(name))) {
    const own = robot.subject === name
    const after = own && robot.given.on === "older" ? [tipOf(name)] : []
    if (landedBy === null || after.length < landedBy.after.length) {
      landedBy = { by: robotOf(given, robot), own, landed: [], after }
    }
  }
  const spanOf = (of: string): SpanView | null => {
    const robot = running.find(robot => robot.subject !== of && landsOf(given, robot).includes(of))
    return robot === undefined ? null : { by: robotOf(given, robot), carried: null }
  }
  const span = spanOf(name)
  const ownRunning = running.find(robot => robot.subject === name)
  const hold =
    landedBy !== null && landedBy.after.length === 0
      ? { kind: "landed" as const, by: landedBy.by, carried: null }
      : ownRunning !== undefined
        ? { kind: "own" as const, by: robotOf(given, ownRunning), carried: null }
        : span === null
          ? null
          : { kind: "span" as const, by: span.by, carried: null }
  const landingAncestor = ancestorsOf(given, name)
    .toReversed()
    .map(ancestor => {
      const ownRobot = running.find(robot => robot.subject === ancestor)
      if (ownRobot !== undefined) {
        return robotOf(given, ownRobot)
      }
      const ancestorSpan = spanOf(ancestor)
      return ancestorSpan !== null && ancestorSpan.by.branch !== name ? ancestorSpan.by : null
    })
    .find(landing => landing !== null)
  return branchFacts({
    landedBy,
    span,
    landingAncestor: landingAncestor ?? null,
    hold,
    landingNeeds: { state: "known", refusals: landingNeedsOf(given, name) },
  })
}

/** The facts of the whole stack of `root` in the view: the first member whose own robot runs, the running robot of the stack branch */
function stackViewOf(given: Given, root: string, robots: readonly ScenarioRobot[]): StackPatronus {
  const running = robots.filter(robot => robot.given.state === "running")
  const member = membersOf(given, root)
    .map(member => running.find(robot => robot.subject === member))
    .find(robot => robot !== undefined)
  const stackRobot = running.find(robot => robot.subject === null)
  return {
    landingMember: member === undefined ? null : robotOf(given, member),
    holdRobotId: stackRobot?.robot.id ?? null,
    startBlocked: false,
    start: null,
    dryRun: null,
  }
}

/** Patronus as the store holds it: the robots, what the view says of the branches, the GitHub repository, the failed-for-approvals robots */
function seedPatronus(given: Given): void {
  const robots = robotsIn(given)
  const names = [...branchesOf(given).keys()]
  const view: PatronusView = {
    branches: Object.fromEntries(names.map(name => [name, branchViewOf(given, name, robots)])),
    stacks: Object.fromEntries(names.filter(name => rootOf(given, name) === name && isForking(given, name)).map(root => [root, stackViewOf(given, root, robots)])),
    landedByRobot: {},
  }
  patronusStore.setState({
    repositories: {
      [KEY]: {
        ...testPatronusRepository(robotsById(...robots.map(robot => robot.robot)), view),
        availability: given.notOnSpace === true ? { status: "notPatronus" } : { status: "available" },
        githubRepository: given.readiness === "space" ? null : GITHUB_REPOSITORY,
        needsReview: Object.fromEntries(robots.filter(robot => robot.given.reason === "approvals").map(robot => [robot.robot.id, true])),
      },
    },
  })
}

// ---- GitHub, the review, the landing ----

function isListed(given: Given): boolean {
  return given.readiness !== "unlisted" && given.readiness !== "space"
}

/** The pull request #7 of `branch` as GitHub lists it: of `pullRequest.of` only, when it names a branch */
function pullRequestOf(given: Given, branch: string): PullRequest | undefined {
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
    headRefOid: pullRequest.state === "merged" ? tipOf(branch) : "1111",
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
function stackPullRequestOf(root: string): PullRequest {
  return {
    number: 12,
    title: `The whole ${root} stack`,
    url: "https://github.com/acme/monorepo/pull/12",
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: `${PREFIX}/stack/${root}`,
    baseRefName: TARGET,
    headRefOid: STACK_TIP,
    checks: null,
    updatedAt: 0,
    reviewers: [],
    author: "me",
  }
}

/** GitHub as the store holds it: the pull requests of the branches and of the stack branches, or why none is listed */
function seedGitHub(given: Given): void {
  const listed = isListed(given)
  const branch = given.pullRequest?.of ?? given.branch
  const pullRequest = listed ? pullRequestOf(given, branch) : undefined
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
        stackPullRequests: given.stackPullRequest === true && listed ? { [root]: stackPullRequestOf(root) } : {},
        fetchedAt: 1,
        isRefreshing: false,
      },
    },
  })
}

/** What Rust answers for the branch (`to_branch_readiness`), as the review-readiness store holds it; null: not asked yet */
function readinessOf(given: Given, pullRequest: PullRequest | undefined): ReviewReadiness | null {
  const asked = (login: string) => pullRequest?.reviewers.some(reviewer => reviewer.requested && reviewer.login === login) === true
  const changed = (login: string) => pullRequest?.reviewers.some(reviewer => reviewer.state === "changesRequested" && reviewer.login === login) === true
  switch (given.readiness) {
    case "notAsked":
      return null
    case "notRequired":
    case "unlisted":
      return { kind: "notRequired" }
    case "approved":
      return { kind: "approved", groups: [{ name: "IJPL Code Platform", approvedBy: [ilya] }] }
    case "missing": {
      const missingGroup = group({ requested: asked(ILYA) ? [ilya] : [], changesRequestedBy: changed(ILYA) ? [ilya] : [] })
      return missing([missingGroup], { notCounted: [], reason: pullRequest === undefined ? "noPullRequestContext" : "approvalsMissing" })
    }
    case "space":
      return missing([group({ requested: [] })], { notCounted: [], reason: "approvalsMissing" })
    case "unknown":
      return { kind: "unknown", reason: "approvalsNotEvaluated" }
    case "unavailable":
      return { kind: "unavailable", reason: "error", message: "Patronus answered 500" }
  }
}

/**
 * The answers of the review scan for the branch and for the whole stack of its root, each under the key of the input
 * the branch has now (`readinessInputOf`, so a start reads them as fresh); a repository only on Space carries the
 * requirement of its merge request. Seeded after the branches and the pull requests, which the key is computed from
 */
function seedReadiness(given: Given): void {
  const entries: Record<string, ReadinessEntry> = {}
  const add = (key: string, tip: string, pullRequest: PullRequest | undefined): void => {
    const readiness = readinessOf(given, pullRequest)
    const input = readinessInputOf(key)
    if (readiness === null || input === null) {
      return
    }
    entries[key] = {
      key: input.key,
      readiness,
      requirement: given.readiness === "space" ? { required: true, mergeRequest: null, groups: [], skipped: false } : null,
      stale: false,
      answeredAt: 0,
      tip,
      pullRequest: pullRequest?.number ?? null,
      waitingFor: [],
      approvedBy: [],
    }
  }
  const branch = given.branch
  add(branch, tipOf(branch), isListed(given) ? pullRequestOf(given, branch) : undefined)
  const root = rootOf(given, branch)
  if (isForking(given, root)) {
    add(stackReadinessKey(root), STACK_TIP, given.stackPullRequest === true && isListed(given) ? stackPullRequestOf(root) : undefined)
  }
  reviewReadinessStore.setState({ repositories: { [KEY]: entries } })
}

/** The refusal of the landing gate: `branch` uses `controlUrl`, which `codec`, outside what it lands, adds */
function landingRefusalOf(branch: string): LandingRefusal {
  return {
    kind: "missingDeclaration",
    branch,
    identifier: "controlUrl",
    file: "src/b/Use.kt",
    line: 3,
    usedBy: { id: "1111111111111111111111111111111111111111", subject: "Start with controlUrl", branch },
    declaredBy: { id: "2222222222222222222222222222222222222222", subject: "Add controlUrl", branch: "codec" },
    fix: { kind: "restack", branch, onto: "codec" },
  }
}

/** The refusal of the landing gate of a dangling reference: `removed` of fake-agent (`commit_dangling_reference`), `changed` of suspend (`commit_changed_declaration`) */
function danglingRefusalOf(branch: string, reference: "removed" | "changed"): LandingRefusal {
  const removed = reference === "removed"
  const fixedBy = removed
    ? { id: "3333333333333333333333333333333333333333", subject: "IJAI-3878 Use the control protocol", branch: "lane-document" }
    : { id: "3333333333333333333333333333333333333333", subject: "IJAI-4182 The journey support goes with its tests", branch: "journey" }
  return {
    kind: "danglingReference",
    branch,
    reference,
    identifier: removed ? "CONTROL_URL_ENV" : "cleanup",
    file: removed ? "src/acp/AcpHost.kt" : "src/contract/AcpJourneyLifecycleTest.kt",
    line: removed ? 7 : 8,
    removedBy: { id: "4444444444444444444444444444444444444444", subject: removed ? "IJAI-3880 Move the control constants" : "IJAI-4185 The cleanups may suspend", branch },
    fixedBy,
    fix: { kind: "moveCommit", commit: fixedBy, into: branch },
  }
}

/** What the landing gate refuses of `branch` after the sync, as the repository view holds it: of the case's branch only */
function landingNeedsOf(given: Given, branch: string): LandingRefusal[] {
  if (branch !== given.branch) {
    return []
  }
  if (given.dangling !== undefined) {
    return [danglingRefusalOf(branch, given.dangling)]
  }
  return given.landingNeeds === true ? [landingRefusalOf(branch)] : []
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
 * The rules of the repository as the app read them (`readDismissesStaleReviews` of the target): `get_branch_rules`
 * answers `keep` or `dismiss`, fails for `unknown` (the app keeps a failed read as `false`, D9), `loading` has no
 * answer yet
 */
async function seedRules(given: Given): Promise<void> {
  resetDismissesStaleReviews()
  if (given.rules === "loading") {
    return
  }
  const rules = vi.spyOn(commands, "getBranchRules")
  try {
    rules.mockImplementation(() =>
      Promise.resolve(
        given.rules === "unknown"
          ? { status: "error" as const, error: { message: "GitHub answered 500" } as never }
          : { status: "ok" as const, data: { dismissStaleReviewsOnPush: given.rules === "dismiss" } as never },
      ),
    )
    await readDismissesStaleReviews(REPOSITORY_PATH, TARGET)
  } finally {
    rules.mockRestore()
  }
}

// ---- the answers ----

function checkMapped(given: Given): void {
  if (given.subject === "wholeStack" && given.robot !== undefined && robotsOf(given).some(robot => robot.state !== "success")) {
    notMapped("robot of the whole stack other than merged")
  }
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

/** Every store as the case's situation leaves it; the next read of the app answers for it */
export async function seedScenario(given: Given): Promise<void> {
  checkMapped(given)
  seedSync(given)
  seedPatronus(given)
  seedGitHub(given)
  seedReadiness(given)
  seedLanded(given)
  await seedRules(given)
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
