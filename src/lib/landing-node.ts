import type { NextStepFacts } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { isFailingChip } from "@/lib/patronus"
import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

/**
 * Where a branch is on its way into master, the node of its row ([Landing](../../docs/design/landing.md)): filled while
 * the next step is the user's and on this row, hollow while it waits for others (people, the parent, the robot) or is done
 */
export type LandingNodeKind = "ready" | "yourStep" | "needsFix" | "landing" | "waiting" | "inMaster"

export interface LandingNodeLook {
  readonly color: UIColor
  readonly filled: boolean
  /** Its line in the legend of the stack graph (`BranchGraphHelpPopover`) */
  readonly legend: string
}

/** The look of each kind, in the order of the legend */
export const landingNodeLooks: Readonly<Record<LandingNodeKind, LandingNodeLook>> = {
  ready: { color: "success", filled: true, legend: "Ready to land: Safe Push" },
  yourStep: { color: "primary", filled: true, legend: "Your step first: a pull request, a review, a retarget" },
  needsFix: { color: "error", filled: true, legend: "Needs a fix: a conflict, a failed robot, changes requested" },
  landing: { color: "primary", filled: false, legend: "Landing now" },
  waiting: { color: "neutral", filled: false, legend: "Waits for others: reviewers, its parent, a robot" },
  inMaster: { color: "info", filled: false, legend: "In master" },
}

export interface LandingNode {
  readonly kind: LandingNodeKind
  /** The hover of the node: the state in the words the row uses elsewhere, a line each */
  readonly label: string
}

/** What decides the node beside the facts of the next step, which don't tell these apart */
export interface LandingNodeFacts {
  readonly next: NextStepFacts
  readonly status: Branch["status"]
  readonly statusText: string
  /** The parent of a `Blocked` branch (`blockingBranchOf`) */
  readonly blockedBy: string | null
  /** A running Safe Push of a branch built on it, or of the whole stack, lands the branch: "oauth", "the whole air stack" */
  readonly landingWith: string | null
  /** The robot of the branch is a Dry Run: it checks the branch, it lands nothing */
  readonly robotDryRun: boolean
  /** Patronus answered whether the repository is on Space */
  readonly patronusKnown: boolean
  /** The remote lacks work of the branch (`getPushAction`): the step a repository without Patronus and GitHub shows */
  readonly pushPending: boolean
}

const node = (kind: LandingNodeKind, label: string): LandingNode => ({ kind, label })

/** The approvals are not judged before the start: the start says so too */
const CHECKED_AT_START = "Patronus checks the approvals when the robot starts"

/**
 * The node of a branch: the first state that matches. A sync and an inline action of the row pass, so the node keeps
 * the state they started from; the next step (`getBranchNextStep`) decides the rest, as the slot of the row shows it.
 */
export function getLandingNode(facts: LandingNodeFacts): LandingNode {
  const next: NextStepFacts = { ...facts.next, syncing: false, processing: false }
  if (next.hasError || facts.status === "Error" || facts.status === "MergeConflict" || facts.status === "AnalyzingConflict") {
    return node("needsFix", facts.statusText || "The branch failed to sync")
  }
  const landed = landedReason(next)
  if (landed !== null) {
    return node("inMaster", landed)
  }
  if (next.commitCount === 0) {
    return node("waiting", "No commits")
  }
  if (facts.status === "Blocked") {
    return node("waiting", facts.blockedBy === null ? "Blocked by its parent branch" : `Blocked by ${facts.blockedBy}, which doesn't build`)
  }
  const robot = next.robot
  if (robot.kind === "running") {
    if (facts.robotDryRun) {
      return node("waiting", "Dry Run running: it checks the branch, it lands nothing")
    }
    return node("landing", isFailingChip(robot.chip) ? "Landing: Safe Push running, a required check failed" : "Landing: Safe Push running")
  }
  if (facts.landingWith !== null) {
    return node("landing", `Lands with ${facts.landingWith}: its Safe Push runs`)
  }
  if (next.held) {
    return node("waiting", "A running Safe Push holds the branch")
  }
  if (!next.patronusAvailable) {
    return facts.patronusKnown ? withoutPatronus(next, facts.pushPending) : node("waiting", "Waits for the answer of Patronus")
  }
  if (robot.kind === "finished" && robot.chip.kind === "failed") {
    return node("needsFix", `Safe Push failed on ${next.subject} as it is: fix it or restart it`)
  }

  // the reason of the step itself: a robot that lands an ancestor is said apart, for a start only
  const step = getBranchNextStep({ ...next, startBlocker: null })
  const reason = step.safePushBlocker ?? ""
  switch (step.kind) {
    case "safePush":
    case "restart":
      return ready(next)
    case "retarget":
    case "draft":
      return node("yourStep", reason)
    case "pullRequest":
      return node("yourStep", "Approvals count only in a pull request: Patronus checks them when the robot starts")
    case "requestReview":
      // nobody of a group can approve: the item says so, no request helps
      return node("yourStep", step.item?.disabled === true ? (step.item.description ?? reason) : reason)
    case "createPullRequest":
      // "In #12": its approvals count in the pull request of the whole stack, whose step is in the island header
      return step.tone === "quiet" ? node("waiting", step.item?.description ?? reason) : node("yourStep", reason)
    case "changesRequested":
      return node("needsFix", reason)
    case "otherBase":
    case "awaiting":
      return node("waiting", reason)
    case "none":
    case "hidden":
      return node("waiting", step.safePushBlocker ?? "Nothing to land")
  }
}

/** The branch is in master: the fetched baseline has it, or a merge says so before the update */
function landedReason(facts: NextStepFacts): string | null {
  if (facts.landedIn !== null) {
    return `Landed in ${facts.landedIn}`
  }
  if (facts.landedWith !== null) {
    return facts.landedWith
  }
  if (facts.robot.kind === "finished" && facts.robot.chip.kind === "merged") {
    return "Merged by Patronus"
  }
  if (facts.pullRequest?.state === "merged" && facts.pullRequest.mergedCurrent) {
    return `Pull request #${facts.pullRequest.number} is merged`
  }
  return null
}

/** A start can land it now, unless a robot that lands an ancestor runs (the branch lands with it later) */
function ready(facts: NextStepFacts): LandingNode {
  if (facts.startBlocker !== null) {
    return node("waiting", facts.startBlocker)
  }
  const readiness = facts.pullRequestsAvailable ? (facts.readiness?.readiness ?? null) : null
  // before the first answer of Patronus nobody knows whether it can land: never a green that turns blue
  if (facts.pullRequestsAvailable && readiness === null) {
    return node("waiting", "Waits for the answer of Patronus about the approvals")
  }
  const lands = facts.landsBranches > 1 ? `Ready to land: Safe Push lands ${facts.landsBranches} branches` : "Ready to land: Safe Push"
  const judged = readiness?.kind === "approved" || readiness?.kind === "notRequired"
  return node("ready", judged ? lands : `${lands}\n${CHECKED_AT_START}`)
}

/** A repository not on Space lands a branch by its pull request; without one GitHub lists, the push is the step it shows */
function withoutPatronus(facts: NextStepFacts, pushPending: boolean): LandingNode {
  if (!facts.pullRequestsAvailable) {
    return pushPending ? node("yourStep", "Not on the remote yet: push it") : node("waiting", "Pushed: it lands through its pull request")
  }
  const pullRequest = facts.pullRequest
  if (pullRequest?.state !== "open") {
    return node("yourStep", "Not in a pull request")
  }
  return pullRequest.isDraft ? node("yourStep", `#${pullRequest.number} is a draft`) : node("waiting", `Lands through #${pullRequest.number}`)
}
