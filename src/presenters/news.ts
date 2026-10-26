// The page's words of the news of a session that Rust decides (`NewsEvent`): what the merge of a robot closes
// (`RobotMerged.closes`, `domain_policy::landed::pull_requests_to_close`). It decides nothing.
import type { LandedPullRequest, PatronusRobot } from "@/lib/bindings"
import { joinNames } from "@/lib/text"

/**
 * What the toast of the merge of `robot` says of the pull requests its landing closes: "closes the pull request of auth:
 * it landed with oauth", "closes the pull requests of auth and login: they landed with oauth", "closes the pull requests
 * of 6 branches: they landed with the whole stack"; null when it closes none
 */
export function describeLandedCloses(robot: PatronusRobot, closes: readonly LandedPullRequest[]): string | null {
  const name = robot.branchName
  if (closes.length === 0 || (name === null && robot.stackRoot == null)) {
    return null
  }
  const one = closes.length === 1
  const of = name !== null || closes.length <= 2 ? joinNames(closes.map(it => it.branch)) : `${closes.length} branches`
  return `closes the pull request${one ? "" : "s"} of ${of}: ${one ? "it" : "they"} landed with ${name ?? "the whole stack"}`
}
