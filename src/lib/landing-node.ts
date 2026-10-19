import type { NextStepFacts } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { isFailingChip } from "@/lib/patronus"
import { joinNames } from "@/lib/text"
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
  ready: { color: "success", filled: true, legend: "Ready to land: Patronus counts the approvals, Safe Push lands it" },
  yourStep: { color: "primary", filled: true, legend: "Your step first: a pull request, a review, a retarget, a Safe Push whose approvals Patronus checks at the start" },
  needsFix: { color: "error", filled: true, legend: "Needs a fix: a conflict, a failed robot, changes requested" },
  landing: { color: "primary", filled: false, legend: "Landing now" },
  waiting: { color: "neutral", filled: false, legend: "Waits for others: reviewers, its parent, a robot" },
  inMaster: { color: "info", filled: false, legend: "In master" },
}

export interface LandingNode {
  readonly kind: LandingNodeKind
  /** The hover of the node: the state in the words the row uses elsewhere, a line each */
  readonly label: string
  /** It waits for the first answer of Patronus: the header counts no move of it yet */
  readonly pending?: true
  /** The label tells the state of the robot of the branch (of the whole stack): the hover shows the robot with its actions */
  readonly robot?: true
  /** The branch the label names, which the hover offers to show: the parent a blocked branch waits for */
  readonly shows?: string
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

/** A state of the robot of the branch itself, which the hover shows with its actions (`LandingNodeDetails`) */
const robotNode = (kind: LandingNodeKind, label: string): LandingNode => ({ kind, label, robot: true })

/** Nobody can tell yet where it is: Patronus hasn't answered */
const pendingNode = (label: string): LandingNode => ({ kind: "waiting", label, pending: true })

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
    return facts.blockedBy === null
      ? node("waiting", "Blocked by its parent branch")
      : { kind: "waiting", label: `Blocked by ${facts.blockedBy}, which doesn't build`, shows: facts.blockedBy }
  }
  const robot = next.robot
  if (robot.kind === "running") {
    if (facts.robotDryRun) {
      return robotNode("waiting", "Dry Run running: it checks the branch, it lands nothing")
    }
    return robotNode("landing", isFailingChip(robot.chip) ? "Landing: Safe Push running, a required check failed" : "Landing: Safe Push running")
  }
  if (facts.landingWith !== null) {
    return node("landing", `Lands with ${facts.landingWith}: its Safe Push runs`)
  }
  if (next.held) {
    return node("waiting", "A running Safe Push holds the branch")
  }
  if (!next.patronusAvailable) {
    return facts.patronusKnown ? withoutPatronus(next, facts.pushPending) : pendingNode("Waits for the answer of Patronus")
  }
  if (robot.kind === "finished" && robot.chip.kind === "failed") {
    // the hover shows the robot: its restart and the prompts for an agent are buttons there
    return robotNode("needsFix", `Safe Push failed on ${next.subject} as it is: fix it, or Restart Safe Push`)
  }

  // the reason of the step itself: a robot that lands an ancestor is said apart, for a start only
  const step = getBranchNextStep({ ...next, startBlocker: null })
  // a step that leaves the Safe Push enabled (a wait GitHub tells while Patronus can't judge) says it in its item
  const reason = step.safePushBlocker ?? step.item?.description ?? ""
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

/**
 * A start can land it now, unless a robot that lands an ancestor runs (the branch lands with it later). Ready only
 * when Patronus judged the approvals; otherwise the start is still the user's step, and Patronus checks them then: a
 * bot's approval or a GitHub decision is no judge of what Patronus counts.
 */
function ready(facts: NextStepFacts): LandingNode {
  if (facts.startBlocker !== null) {
    return node("waiting", facts.startBlocker)
  }
  const readiness = facts.pullRequestsAvailable ? (facts.readiness?.readiness ?? null) : null
  // before the first answer of Patronus nobody knows whether it can land: never a green that turns blue
  if (facts.pullRequestsAvailable && readiness === null) {
    return pendingNode("Waits for the answer of Patronus about the approvals")
  }
  const lands = facts.landsBranches > 1 ? `Safe Push lands ${facts.landsBranches} branches` : "Safe Push"
  return readiness?.kind === "approved" || readiness?.kind === "notRequired"
    ? node("ready", `Ready to land: ${lands}`)
    : node("yourStep", `${lands}: Patronus checks the approvals when the robot starts`)
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
  if (pullRequest.isDraft) {
    return node("yourStep", `#${pullRequest.number} is a draft`)
  }
  // it lands through the pull request: what it waits for is the author's to fix
  if (pullRequest.changesRequestedBy.length > 0) {
    return node("needsFix", `${joinNames(pullRequest.changesRequestedBy)} requested changes on #${pullRequest.number}`)
  }
  if (pullRequest.changesRequested) {
    return node("needsFix", `Changes requested on #${pullRequest.number}`)
  }
  if (pullRequest.checksFailing) {
    return node("needsFix", `Checks fail on #${pullRequest.number}`)
  }
  return pullRequest.waitingFor.length > 0
    ? node("waiting", `Waits for ${joinNames(pullRequest.waitingFor)} on #${pullRequest.number}`)
    : node("waiting", `Lands through #${pullRequest.number}`)
}
