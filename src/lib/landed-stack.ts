import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { LandedSource } from "@/lib/patronus"
import { findLandedBranches } from "@/lib/patronus"
import { stackAncestors } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { describeOrphanedStackPullRequest } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { getBranch, getStackBranch } from "@/stores/branch-sync"
import { rememberLandedStackRoot } from "@/stores/landed-stacks"
import { closePullRequest, getPullRequest, getStackPullRequest } from "@/stores/pull-requests"
import { getFullBranchName, getFullStackBranchName } from "@/stores/repository"

/** The open pull request of a branch a merged robot landed with the branch it ran on, or with the whole stack */
export interface LandedPullRequest {
  branch: string
  number: number
}

/** What the landed pull requests are found in: the branches of the table, their pull requests, the stack branches */
export interface LandingFacts extends LandedSource {
  readonly pullRequestOf: (name: string) => PullRequest | undefined
}

/**
 * The pull request shows nothing the landed branch lacks, by content: its head is the branch's tip, or it is the remote
 * branch the remote status compared, which has no commit the branch lacks (older versions of commits amended or restacked
 * here are no such commits: their newer versions landed)
 */
function holdsNothingMore(pullRequest: PullRequest, branch: Branch): boolean {
  const remote = branch.remoteStatus
  return pullRequest.headRefOid === branch.commits[0]?.hash || (remote !== null && remote.remoteTip === pullRequest.headRefOid && remote.commitsBehind === 0)
}

/**
 * The open pull requests of the branches a merged robot landed besides its own (`findLandedBranches`, the branches the
 * rows say it landed): of the ancestors of a stacked branch (the branch carries their commits), or of every branch of
 * the stack for the robot of the whole stack (its stack branch carries them all); only while the robot ran on the branch
 * or the stack branch as the table has it, and only a pull request that holds nothing the branch lacks
 * (`holdsNothingMore`): then the merge has every commit of it. The pull request the robot started from is Patronus' to
 * close.
 */
export function findLandedPullRequests(robot: PatronusRobot, facts: LandingFacts): LandedPullRequest[] {
  return findLandedBranches(robot, facts).flatMap(branch => {
    const pullRequest = facts.pullRequestOf(branch.name)
    const own = pullRequest !== undefined && pullRequest.number === robot.pullRequest?.number
    return pullRequest?.state === "open" && !own && holdsNothingMore(pullRequest, branch) ? [{ branch: branch.name, number: pullRequest.number }] : []
  })
}

/**
 * Closes the pull requests of the branches a merged robot landed (`findLandedPullRequests`), each with a comment that
 * names the robot's branch or the whole stack; a failure is an error toast. What the merge toast says about them:
 * "closes the pull requests of auth and login: they landed with oauth", "closes the pull requests of 6 branches: they
 * landed with the whole stack"; null when there are none. A Safe Push of a branch that lands the root of a stack whose
 * whole stack has an open pull request is remembered: that pull request no longer matches the stack
 * (`rememberLandedStackRoot`), and the toast says so too ("#12 no longer matches the air stack: oauth landed"); the user
 * decides on it, nothing closes it.
 */
export function closeLandedPullRequests(robot: PatronusRobot): string | null {
  const parts = [closeLanded(robot), rememberLandedRoot(robot)].filter((part): part is string => part !== null)
  return parts.length === 0 ? null : parts.join(" · ")
}

function closeLanded(robot: PatronusRobot): string | null {
  const landed = findLandedPullRequests(robot, { branchOf: getBranch, pullRequestOf: getPullRequest, stackOf: getStackBranch })
  const name = robot.branchName
  const root = robot.stackRoot ?? null
  if (landed.length === 0 || (name === null && root === null)) {
    return null
  }
  const by = name === null ? `the Safe Push of the whole ${root} stack (${getFullStackBranchName(root!)})` : `the Safe Push of ${getFullBranchName(name)}`
  const what = name === null ? "which holds this branch" : "which is built on this branch"
  const comment = `Landed in ${robot.targetBranch} with ${robot.pullRequest == null ? by : `#${robot.pullRequest.number}, ${by}`}, ${what}.`
  for (const { branch, number } of landed) {
    closePullRequest(branch, comment).catch((error: unknown) => notifyError(`Cannot close pull request #${number}`, error))
  }
  const one = landed.length === 1
  const of = name !== null || landed.length <= 2 ? joinNames(landed.map(it => it.branch)) : `${landed.length} branches`
  return `closes the pull request${one ? "" : "s"} of ${of}: ${one ? "it" : "they"} landed with ${name ?? "the whole stack"}`
}

/**
 * A merged robot of a branch landed the root of its stack: an open pull request of the whole stack no longer matches it.
 * What the toast says of it, null when there is none.
 */
function rememberLandedRoot(robot: PatronusRobot): string | null {
  const name = robot.branchName
  if (name === null || robot.pushMode === "dryRun") {
    return null
  }
  const root = stackAncestors(name, getBranch)[0]?.name ?? name
  const pullRequest = getStackPullRequest(root)
  if (pullRequest?.state !== "open") {
    return null
  }
  rememberLandedStackRoot(root, name)
  return describeOrphanedStackPullRequest(root, pullRequest, getStackBranch(root), name)
}
