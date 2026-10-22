// The app's side of a scenario case (`crates/domain-policy/cases/README.md`): `given` as the facts the stores would
// hold (`NextStepFacts`, then `SafePushFacts` as `readSafePush` builds them), the profile chosen as `readSafePush`
// chooses it; the whole stack as `stackFactsOf` reads it. Only what the README marks as mapped; anything else fails
// with "not mapped yet"
import type { LandingRefusal, PatronusProfile, PatronusPushMode, PatronusRobot, PatronusStartOptions, PullRequest, ReviewReadiness } from "@/lib/bindings"
import type { BranchNextStep, NextStepFacts, NextStepPullRequest } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import type { ParkedState } from "@/lib/parked"
import { wholeStackParkedState } from "@/lib/parked"
import type { LandedBy, LandingAncestor } from "@/lib/patronus"
import {
  allowsPushMode,
  formatLandedReason,
  formatLandedWholeReason,
  formatLandingReason,
  formatLandingWait,
  formatPushMode,
  getBypassProfiles,
  getPullRequestStart,
  getSafePushSlot,
  getStartPush,
  isPullRequestIntoTarget,
  isStartBlocked,
} from "@/lib/patronus"
import { changesRequestedBy, waitingReviewers } from "@/lib/pull-request"
import type { DismissFact, ReviewFact, SafePushFacts } from "@/lib/safe-push-decision"
import { facts, group, ilya, missing, restart } from "@/lib/test-next-step-facts"
import { testRobot } from "@/lib/test-robot"
import { formatWholeStack, getStackBranchBlocker, stackReadinessKey } from "@/lib/whole-stack"
import type { RemoteStatus, StackBranch } from "@/stores/branch-sync"
import { testRemoteStatus } from "@/stores/test-branch"
import type { Given, GivenRobot } from "@/test-support/scenario-case"
import { robotsOf } from "@/test-support/scenario-case"

const TARGET = "master"
const VIRTUAL = "test-user/virtual/"
const GITHUB_REPOSITORY = "acme/monorepo"
/** The GitHub login of Ilya Korennoy, the member of IJPL Code Platform Patronus suggests */
const ILYA = "ikorennoy"
/** The commit of a conflict of the stack branch, as the sync names it */
const CONFLICT_COMMIT = "0caf9066d2a4c3b1e5f7a9b8c6d4e2f1a3b5c7d9"
/** The commit `bd push` pushed, which the remote branch has */
const PUSHED = "pushed"

/** The remote status of the case's branch as the sync left it; null where nothing reads it */
function remoteStatusOf(given: Given): RemoteStatus | null {
  switch (given.remote) {
    case "rebuiltOnNewerBaseline":
      // the same commits by content, rebuilt on a newer baseline: not a fast-forward of the remote branch
      return testRemoteStatus({ remoteTip: PUSHED, fastForward: false })
    case "behind":
      return testRemoteStatus({
        remoteTip: "theirs",
        commitsBehind: 1,
        droppedCommits: [{ id: "theirs", subject: "Someone else", author: "Other", authorTime: 0 }],
        fastForward: false,
      })
    default:
      return null
  }
}

/** The parked state of `name`: the branch `given.parked` names (`self` the case's branch, `builtOn` its parent), or a branch built on it */
function parkedStateOf(given: Given, name: string): ParkedState | null {
  const parked = given.parked === "self" ? given.branch : given.parked === "builtOn" ? ancestorsOf(given, given.branch).at(-1) : given.parked
  if (parked === undefined) {
    return null
  }
  if (parked === name) {
    return { note: "", with: null }
  }
  return ancestorsOf(given, name).includes(parked) ? { note: "", with: parked } : null
}

function notMapped(what: string): never {
  throw new Error(`given.${what} is not mapped yet`)
}

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

/** The robot `given` describes: on the tip of `branch` (`<branch>-tip`), an older commit (`0000`), another branch, the stack branch */
function robotOf(given: GivenRobot, branch: string, index: number): PatronusRobot {
  const status = given.state
  const finishTime = status === "running" ? null : 1704189600 + index
  const [where, other] = given.on.split(":") as [string, string | undefined]
  switch (where) {
    case "tip":
      return testRobot(`robot-${index}`, branch, { status, finishTime })
    case "older":
      return testRobot(`robot-${index}`, branch, { status, finishTime, sourceRevision: "0000" })
    case "remote":
      return testRobot(`robot-${index}`, branch, { status, finishTime, sourceRevision: PUSHED })
    case "stack":
      return testRobot(`robot-${index}`, null, { status, finishTime, sourceRevision: "stack-tip" })
    default:
      return testRobot(`robot-${index}`, other!, { status, finishTime })
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

function isListed(given: Given): boolean {
  return given.readiness !== "unlisted" && given.readiness !== "space"
}

/** The pull request #7 of `branch` as GitHub lists it: of `pullRequest.of` only, when it names a branch */
function pullRequestOf(given: Given, branch: string): PullRequest | undefined {
  const pullRequest = given.pullRequest
  if (pullRequest === undefined || !isListed(given) || (pullRequest.of !== undefined && pullRequest.of !== branch)) {
    return undefined
  }
  return {
    number: 7,
    title: "Add auth",
    url: "https://github.com/acme/monorepo/pull/7",
    state: pullRequest.state,
    isDraft: pullRequest.draft,
    reviewDecision: null,
    headRefName: `${VIRTUAL}${branch}`,
    baseRefName: pullRequest.base,
    headRefOid: "1111",
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

function nextStepPullRequest(pullRequest: PullRequest): NextStepPullRequest {
  const base = pullRequest.baseRefName ?? ""
  return {
    number: pullRequest.number,
    state: pullRequest.state,
    isDraft: pullRequest.isDraft,
    base,
    intoTarget: isPullRequestIntoTarget(pullRequest, TARGET),
    // GitHub merged it with the commits the branch has
    mergedCurrent: pullRequest.state === "merged",
    baseIsVirtualBranch: base.length > VIRTUAL.length && base.startsWith(VIRTUAL),
    waitingFor: waitingReviewers(pullRequest),
    changesRequestedBy: changesRequestedBy(pullRequest),
    checksFailing: false,
    changesRequested: false,
  }
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

/** The Safe Push of `on` (a branch, `child:<b>`, `stack`) merged: what it landed of `branch` (`getLandedBy`); null for none */
function landedByOf(given: Given, branch: string): LandedBy | null {
  const robots = robotsOf(given)
  for (const [index, robot] of robots.entries()) {
    if (robot.state !== "success") {
      continue
    }
    const landed = robotOf(robot, branch, index)
    if (robot.on === "stack") {
      return { branch: rootOf(given, branch), robot: landed, wholeStack: true, after: [] }
    }
    const of = robot.on.startsWith("child:") ? robot.on.slice("child:".length) : null
    if (of !== null && (of === branch || ancestorsOf(given, of).includes(branch))) {
      return { branch: of, robot: landed, after: [] }
    }
  }
  return null
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

/** What the landing gate refuses of `branch` after the sync, as the repository view holds it */
function landingNeedsOf(given: Given, branch: string): LandingRefusal[] {
  if (given.dangling !== undefined) {
    return [danglingRefusalOf(branch, given.dangling)]
  }
  return given.landingNeeds === true ? [landingRefusalOf(branch)] : []
}

/** The facts of the next step of `branch` (the case's own by default) */
export function nextStepFactsOf(given: Given, branch: string = given.branch): NextStepFacts {
  checkMapped(given)
  const pullRequest = pullRequestOf(given, branch)
  const available = given.notOnSpace !== true
  const ancestors = ancestorsOf(given, branch)
  const robots = robotsOf(given).map((robot, index) => ({ given: robot, robot: robotOf(robot, branch, index) }))
  const own = robots.find(({ given }) => given.on === "tip" || given.on === "older" || given.on === "remote")
  const remoteStatus = remoteStatusOf(given)
  const running = (on: string) => robots.find(({ given }) => given.state === "running" && given.on.startsWith(on))
  const commitCount = given.landed === "byContent" ? 1 : (branchesOf(given).get(branch)?.commits ?? 0)
  const hasError = given.conflicts === branch
  const blockedBy = given.conflicts !== undefined && ancestors.includes(given.conflicts) ? given.conflicts : null
  // a robot on the commit the branch is at, or on its remote branch only rebuilt since (`isStartBlocked`)
  const onTip = own !== undefined && isStartBlocked(own.robot, `${branch}-tip`, remoteStatus) ? own.robot : null
  const descendant = running("child:")
  const ancestor = running("ancestor:")
  const landingPart: LandingAncestor | null = ancestor === undefined ? null : { branch: ancestor.robot.branchName!, robot: ancestor.robot }
  const blockerOf = (dryRun: boolean): string | null => {
    if (blockedBy !== null) {
      return `Blocked by ${blockedBy}: fix it first, then sync`
    }
    if (onTip !== null) {
      return `A ${formatPushMode(onTip.pushMode)} of the last commit is running`
    }
    if (descendant !== undefined) {
      return formatLandingReason(branch, { branch: descendant.robot.branchName!, robot: descendant.robot })
    }
    return landingPart === null || dryRun ? null : formatLandingWait(landingPart)
  }
  const landedBy = landedByOf(given, branch)
  const ownFailed = own !== undefined && (own.given.state === "failure" || own.given.state === "canceled")
  const stackRoot = rootOf(given, branch)
  return facts({
    branchName: branch,
    subject: branch,
    patronusAvailable: available,
    syncing: given.sync === "running",
    hasError,
    commitCount,
    blocked: blockedBy !== null,
    startBlocker: blockerOf(false),
    dryRunStartBlocker: blockerOf(true),
    landingPart,
    held: onTip !== null || descendant !== undefined,
    parked: parkedStateOf(given, branch),
    landedIn: given.landed === "inBaseline" ? "origin/master" : null,
    landedWith: landedBy === null ? null : formatLandedReason(landedBy),
    robot: getSafePushSlot({
      robot: own?.robot,
      localTip: `${branch}-tip`,
      remote: remoteStatus,
      needsReview: own?.given.reason === "approvals",
      failing: false,
      canOffer: available && commitCount > 0 && !hasError,
    }),
    robotPullRequest: own?.given.fromPullRequest === true ? 7 : null,
    restart: ownFailed ? restart : null,
    pullRequestsAvailable: isListed(given),
    pullRequest: pullRequest === undefined ? null : nextStepPullRequest(pullRequest),
    target: TARGET,
    landsBranches: ancestors.length + 1,
    landingNeeds: landingNeedsOf(given, branch),
    readiness: ((readiness: ReviewReadiness | null) => (readiness === null ? null : { readiness, stale: false }))(readinessOf(given, pullRequest)),
    stackPullRequest: given.stackPullRequest === true && isForking(given, stackRoot) ? { number: 12, subject: formatWholeStack(stackRoot), open: () => {} } : null,
  })
}

/** The stack branch of the stack of `root` as the sync left it */
function stackBranchOf(given: Given, root: string, members: string[]): StackBranch {
  const conflicts = given.conflicts !== undefined && members.includes(given.conflicts)
  const blocked = given.stackBranch === "notBuilt"
  return {
    root,
    branchName: `test-user/stack/${root}`,
    members,
    commitCount: members.reduce((count, member) => count + (branchesOf(given).get(member)?.commits ?? 0), 0),
    state: conflicts ? "Conflict" : blocked ? "Blocked" : "Built",
    tip: conflicts || blocked ? null : "stack-tip",
    blockedBy: blocked ? { member: members.at(-1)!, reason: "Problem" } : null,
    conflict: conflicts ? { commit: CONFLICT_COMMIT, files: ["config.txt"] } : null,
    error: null,
    remoteStatus: null,
  }
}

/** The facts of the whole stack of a stack that forks, as `stackFactsOf` reads them; merged robots only */
function stackNextStepFactsOf(given: Given, root: string): NextStepFacts {
  checkMapped(given)
  const members = membersOf(given, root)
  const stack = stackBranchOf(given, root, members)
  const blocker = getStackBranchBlocker(root, stack)
  const landed = members.map(member => landedByOf(given, member))
  const rest = members.filter((_, index) => landed[index] === null)
  const pullRequest = given.stackPullRequest === true && isListed(given) ? stackPullRequestOf(root) : undefined
  return facts({
    branchName: stackReadinessKey(root),
    subject: formatWholeStack(root),
    commitCount: stack.commitCount,
    startBlocker: blocker,
    dryRunStartBlocker: blocker,
    parked: wholeStackParkedState(members, name => parkedStateOf(given, name)),
    landedWith: formatLandedWholeReason(landed, root),
    robot: { kind: "start" },
    pullRequestsAvailable: isListed(given),
    pullRequest: pullRequest === undefined ? null : { ...nextStepPullRequest(pullRequest), baseIsVirtualBranch: false },
    target: TARGET,
    landsBranches: rest.length,
    landingNeeds: null,
    readiness: ((readiness: ReviewReadiness | null) => (readiness === null ? null : { readiness, stale: false }))(readinessOf(given, pullRequest)),
  })
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
    headRefName: `test-user/stack/${root}`,
    baseRefName: TARGET,
    headRefOid: "stack-tip",
    checks: null,
    updatedAt: 0,
    reviewers: [],
    author: "me",
  }
}

/**
 * The step of the case: of its branch, or of the whole stack of its branch, the root (`useStackNextStep`): of the stack
 * branch while the stack forks, else of the last branch of the straight chain
 */
export function nextStepOf(given: Given): BranchNextStep {
  if (given.subject === "branch") {
    return getBranchNextStep(nextStepFactsOf(given))
  }
  const root = given.branch
  if (isForking(given, root)) {
    return getBranchNextStep(stackNextStepFactsOf(given, root))
  }
  return getBranchNextStep(nextStepFactsOf(given, membersOf(given, root).at(-1)!))
}

/** The review as `reviewFactOf` of `useSafePush` gives it */
function reviewFactOf(given: Given, mode: PatronusPushMode, options: PatronusStartOptions, readiness: ReviewReadiness | null): ReviewFact {
  if (mode !== "rebaseAutosquash" || !options.supported) {
    return { kind: "none" }
  }
  if (given.readiness === "space") {
    return { kind: "space", required: true }
  }
  if (given.readiness === "unlisted") {
    return { kind: "unlisted" }
  }
  return readiness === null ? { kind: "loading" } : { kind: "loaded", readiness }
}

/** Whether the push of the start dismisses the approvals of #7 (`readStartBasics`): a failed read of the rules is `false` */
function dismissOf(given: Given, pullRequest: PullRequest | undefined, needsPush: boolean): DismissFact | null {
  if (!needsPush || pullRequest?.reviewers.some(reviewer => reviewer.state === "approved") !== true) {
    return null
  }
  switch (given.rules) {
    case undefined:
    case "keep":
    case "unknown":
      return { pullRequest: pullRequest.number, dismisses: false }
    case "dismiss":
      return { pullRequest: pullRequest.number, dismisses: true }
    case "loading":
      return { pullRequest: pullRequest.number, dismisses: null }
  }
}

/** The facts of the start of the case's branch, with its step, the profile chosen as `readSafePush` chooses it */
export function safePushFactsOf(given: Given): { step: BranchNextStep; facts: SafePushFacts } {
  if (given.subject !== "branch") {
    notMapped("subject wholeStack of a decision")
  }
  const branch = given.branch
  const next = nextStepFactsOf(given)
  const step = getBranchNextStep(next)
  const mode: PatronusPushMode = given.mode === "dryRun" ? "dryRun" : "rebaseAutosquash"
  const options = startOptions(given)
  const pullRequest = pullRequestOf(given, branch)
  const fromPullRequest = getPullRequestStart(given.readiness === "space" ? null : GITHUB_REPOSITORY, pullRequest, mode, TARGET)
  const review = reviewFactOf(given, mode, options, next.readiness?.readiness ?? null)
  const profiles = options.profiles.filter(profile => allowsPushMode(profile, mode))
  const branchProfileId = profiles.some(profile => profile.id === options.defaultProfileId) ? options.defaultProfileId : null
  const bypassReview =
    given.flags.includes("withoutApprovals") && mode === "rebaseAutosquash" && fromPullRequest === null && !(review.kind === "loaded" && review.readiness.kind !== "missing")
  const bypass = bypassReview ? getBypassProfiles(options, mode) : null
  const offered = bypass === null ? profiles : [...bypass.skipping, ...bypass.other, ...bypass.emergency]
  const chosen = given.profile?.chosen
  const profileId = chosen !== undefined ? (chosen === "none" ? null : chosen) : bypass === null ? branchProfileId : bypass.defaultProfileId
  const profile = fromPullRequest === null ? (offered.find(it => it.id === profileId) ?? null) : null
  const pushActions = { none: "create", upToDate: "upToDate", unpushed: "push", behind: "force", rebuiltOnNewerBaseline: "upToDate" } as const
  const startPush = getStartPush(pushActions[given.remote], false)
  const replaces = robotsOf(given).some(robot => robot.on === "older" && robot.state === "running") ? "0000" : null
  return {
    step,
    facts: {
      branchName: branch,
      subject: branch,
      mode,
      via: given.via,
      step: { kind: step.kind, blocker: mode === "dryRun" ? step.dryRunBlocker : step.safePushBlocker },
      target: TARGET,
      options: { status: "loaded", options },
      profile,
      bypassReview,
      fromPullRequest,
      startPush,
      hasTip: true,
      replaces,
      pushHeld: null,
      review,
      dismiss: dismissOf(given, pullRequest, startPush.kind !== "none"),
      ancestorCount: ancestorsOf(given, branch).length,
      wholeStackPullRequest: next.stackPullRequest?.number ?? null,
      landingRefusals: next.landingNeeds ?? [],
    },
  }
}
