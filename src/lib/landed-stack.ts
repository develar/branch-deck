import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { stackAncestors } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import type { Branch } from "@/stores/branch-sync"
import { getBranch } from "@/stores/branch-sync"
import { closePullRequest, getPullRequest } from "@/stores/pull-requests"
import { getFullBranchName } from "@/stores/repository"

/** The open pull request of an ancestor a merged robot landed with its branch */
export interface LandedPullRequest {
  branch: string
  number: number
}

/**
 * The open pull requests of the ancestors a robot that merged a stacked branch landed with it: the branch carries their
 * commits. Only while the robot ran on the branch as the table has it, and only a pull request whose head is the
 * ancestor's tip: then the merge has every commit of it. `branchOf` and `pullRequestOf` look them up.
 */
export function findLandedPullRequests(
  robot: PatronusRobot,
  branchOf: (name: string) => Branch | undefined,
  pullRequestOf: (name: string) => PullRequest | undefined,
): LandedPullRequest[] {
  const name = robot.branchName
  const branch = name === null ? undefined : branchOf(name)
  if (name === null || branch === undefined || robot.pushMode === "dryRun" || robot.sourceRevision === null || robot.sourceRevision !== branch.commits[0]?.hash) {
    return []
  }
  const landed: LandedPullRequest[] = []
  for (const ancestor of stackAncestors(name, branchOf)) {
    const pullRequest = pullRequestOf(ancestor.name)
    if (pullRequest?.state === "open" && pullRequest.headRefOid === ancestor.commits[0]?.hash) {
      landed.push({ branch: ancestor.name, number: pullRequest.number })
    }
  }
  return landed
}

/**
 * Closes the pull requests of the ancestors a merged robot landed (`findLandedPullRequests`), each with a comment that
 * names the robot's branch; a failure is an error toast. What the merge toast says about them: "closes the pull requests
 * of auth and login: they landed with oauth"; null when there are none.
 */
export function closeLandedPullRequests(robot: PatronusRobot): string | null {
  const landed = findLandedPullRequests(robot, getBranch, getPullRequest)
  if (landed.length === 0 || robot.branchName === null) {
    return null
  }
  const name = robot.branchName
  const by = robot.pullRequest == null ? `the Safe Push of ${getFullBranchName(name)}` : `#${robot.pullRequest.number}, the Safe Push of ${getFullBranchName(name)}`
  const comment = `Landed in ${robot.targetBranch} with ${by}, which is built on this branch.`
  for (const { branch, number } of landed) {
    closePullRequest(branch, comment).catch((error: unknown) => notifyError(`Cannot close pull request #${number}`, error))
  }
  const of = joinNames(landed.map(it => it.branch))
  return `closes the pull request${landed.length === 1 ? "" : "s"} of ${of}: ${landed.length === 1 ? "it" : "they"} landed with ${name}`
}
