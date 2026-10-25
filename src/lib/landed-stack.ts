import type { LandedPullRequest, PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { repositoryName } from "@/lib/repository-config"
import { joinNames } from "@/lib/text"
import { describeOrphanedStackPullRequest } from "@/lib/whole-stack"
import { getStackAncestors, getStackBranch } from "@/stores/branch-sync"
import { rememberLandedStackRoot } from "@/stores/landed-stacks"
import { closePullRequest, getStackPullRequest } from "@/stores/pull-requests"
import { getFullBranchName, getFullStackBranchName } from "@/stores/repository"

/**
 * Closes the pull requests the landing of a merged robot closes (`closes` of its news, which Rust decides:
 * `domain_policy::landed::pull_requests_to_close`), each with a comment that names the robot's branch or the whole
 * stack; a failure is an error toast. What the merge toast says about them:
 * "closes the pull requests of auth and login: they landed with oauth", "closes the pull requests of 6 branches: they
 * landed with the whole stack"; null when there are none. A Safe Push of a branch that lands the root of a stack whose
 * whole stack has an open pull request is remembered: that pull request no longer matches the stack
 * (`rememberLandedStackRoot`), and the toast says so too ("#12 no longer matches the air stack: oauth landed"); the user
 * decides on it, nothing closes it.
 */
export function closeLandedPullRequests(robot: PatronusRobot, closes: readonly LandedPullRequest[], repositoryKey: string): string | null {
  const parts = [closeLanded(robot, closes, repositoryKey), rememberLandedRoot(robot)].filter((part): part is string => part !== null)
  return parts.length === 0 ? null : parts.join(" · ")
}

/**
 * What the merge toast of a robot of a repository the main window no longer shows says of the pull requests its landing
 * closes: the pull requests of the table are of the shown repository, so nothing is closed ("the pull requests of the
 * branches it landed stay open: idea-1 is no longer the open repository"); null when it closes none
 */
export function describeLandedLeftOpen(closes: readonly LandedPullRequest[], repositoryPath: string): string | null {
  return closes.length > 0 ? `the pull requests of the branches it landed stay open: ${repositoryName(repositoryPath)} is no longer the open repository` : null
}

function closeLanded(robot: PatronusRobot, landed: readonly LandedPullRequest[], repositoryKey: string): string | null {
  const name = robot.branchName
  const root = robot.stackRoot ?? null
  if (landed.length === 0 || (name === null && root === null)) {
    return null
  }
  const by = name === null ? `the Safe Push of the whole ${root} stack (${getFullStackBranchName(root!)})` : `the Safe Push of ${getFullBranchName(name)}`
  const what = name === null ? "which holds this branch" : "which is built on this branch"
  const comment = `Landed in ${robot.targetBranch} with ${robot.pullRequest == null ? by : `#${robot.pullRequest.number}, ${by}`}, ${what}.`
  for (const { branch, number } of landed) {
    closePullRequest(branch, comment, repositoryKey).catch((error: unknown) => notifyError(`Cannot close pull request #${number}`, error))
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
  const root = getStackAncestors(name)[0] ?? name
  const pullRequest = getStackPullRequest(root)
  if (pullRequest?.state !== "open") {
    return null
  }
  rememberLandedStackRoot(root, name)
  return describeOrphanedStackPullRequest(root, pullRequest, getStackBranch(root), name)
}
