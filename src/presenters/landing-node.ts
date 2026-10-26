// The page's words and look of the node of a row as Rust decides it (`LandingNode` of the view's `nextSteps`,
// `domain_policy::landing_node`; [Landing](../../docs/design/landing.md)): its kind, the hover in the words the row uses
// elsewhere, the robot its hover shows, the branch it offers to show. It decides nothing: a running move of the row (a
// start, a review request: `FoundMove`; without Patronus its push) is the page's local state until the operations move
// into Rust (s10), and shows in place of a node that says so (`moveShows`; the push in place of `pushIt`).
import type { LandingNode as NodeView, LandingNodeKind, NodeLabel } from "@/lib/bindings"
import { formatParkedLabel } from "@/lib/parked"
import { formatLandingSubject } from "@/lib/patronus"
import { formatAvailabilityHint } from "@/lib/pull-request"
import type { FoundMove } from "@/lib/running-move"
import { joinNames } from "@/lib/text"
import type { UIColor } from "@/lib/ui-types"
import type { StepContext } from "@/presenters/next-step"
import { formatPendingFact, formatProblemsFailure, formatStepFailure, formatUnlisted, presentNextStep } from "@/presenters/next-step"
import { formatLanded, formatLandedWhole, formatReason } from "@/presenters/reasons"

export interface LandingNodeLook {
  readonly color: UIColor
  readonly filled: boolean
  /** Its line in the legend of the stack graph (`BranchGraphHelpPopover`) */
  readonly legend: string
}

/** The look of each kind, in the order of the legend */
export const landingNodeLooks: Readonly<Record<LandingNodeKind, LandingNodeLook>> = {
  ready: { color: "success", filled: true, legend: "Ready to land: Patronus counts the approvals, Safe Push lands it" },
  yourStep: {
    color: "primary",
    filled: true,
    legend: "Your step first: a pull request, a review, a retarget, a branch its build needs, a Safe Push whose approvals Patronus checks at the start",
  },
  needsFix: { color: "error", filled: true, legend: "Needs a fix: a conflict, a failed robot, changes requested" },
  landing: { color: "primary", filled: false, legend: "Landing now" },
  waiting: { color: "neutral", filled: false, legend: "Waits for others: reviewers, its parent, a robot" },
  parked: { color: "neutral", filled: false, legend: "Parked: kept local, never pushed" },
  inMaster: { color: "info", filled: false, legend: "In master" },
}

/** The node of a row in the page's words */
export interface LandingNode {
  readonly kind: LandingNodeKind
  /** The hover of the node: the state in the words the row uses elsewhere, a line each */
  readonly label: string
  /** It waits for the first answer of a fact: the header counts no move of it yet */
  readonly pending?: true
  /** The label tells the state of the robot of the branch (of the whole stack): the hover shows the robot with its actions */
  readonly robot?: true
  /** The branch the label names, which the hover offers to show: the parent a blocked branch waits for */
  readonly shows?: string
}

/** What the words of a node need beside the node: those of its step, the status text of the row, the move that runs */
export interface NodeContext {
  readonly step: StepContext
  /** The status text of the row ("merge conflict"): why the sync failed where Rust names no problem */
  readonly statusText: string
  /** A running move of the row, or of a row whose Safe Push lands it (`findRunningMove`): the move is made */
  readonly move: FoundMove | null
  /** A push of the row runs (`isBranchPushing`): without Patronus the push made, its node that of the pushed branch */
  readonly pushing: boolean
}

/** The node of a running move: Landing for a robot that lands the row, else Waiting, what others answer next */
function moveNode({ move, with: by }: FoundMove): LandingNode {
  if (by !== null) {
    return { kind: "landing", label: `Lands with ${by}: its Safe Push starts` }
  }
  return { kind: move.lands ? "landing" : "waiting", label: move.label }
}

/** "Safe Push", "Safe Push lands 3 branches" */
function formatLands(lands: number): string {
  return lands > 1 ? `Safe Push lands ${lands} branches` : "Safe Push"
}

/** The words of the node's label; `kind` for the step's own (the hover of "In #12" and of Request Review… say more) */
function labelWords(label: NodeLabel, context: NodeContext): string {
  const subject = context.step.subject
  switch (label.kind) {
    case "parked": {
      const parked = formatParkedLabel({ note: label.parked.note, with: label.parked.with })
      return label.conflict ? `${parked}\nIt doesn't build: fix it before you unpark it` : parked
    }
    case "syncFailed":
      return label.problem ?? (context.statusText || "The branch failed to sync")
    case "landed":
      return formatLandedWhole(label.landed) ?? formatLanded(label.landed)
    case "noCommits":
      return "No commits"
    case "blocked":
      return label.by === null ? "Blocked by its parent branch" : `Blocked by ${label.by}, which doesn't build`
    case "robotRunning":
      if (label.dryRun) {
        return "Dry Run running: it checks the branch, it lands nothing"
      }
      return label.failing ? "Landing: Safe Push running, a required check failed" : "Landing: Safe Push running"
    case "landsWith":
      return `Lands with ${formatLandingSubject(label.by)}: its Safe Push runs`
    case "waits":
      return formatPendingFact(label.fact)
    case "unavailable":
      return formatStepFailure(label.failure)
    case "held":
      return "A running Safe Push holds the branch"
    case "robotFailed":
      // the hover shows the robot: its restart and the prompts for an agent are buttons there
      return label.problems === null
        ? `Safe Push failed on ${subject} as it is: fix it, or Restart Safe Push`
        : `Safe Push failed on ${subject} as it is · ${formatProblemsFailure(label.problems)}`
    case "startWaits":
      return formatReason(label.reason)
    case "ready":
      return `Ready to land: ${formatLands(label.lands)}`
    case "atStart": {
      const unlisted = formatAvailabilityHint(label.unlisted)
      const lands = formatLands(label.lands)
      return unlisted === null ? `${lands}: Patronus checks the approvals when the robot starts` : `${lands} · ${formatUnlisted(unlisted)}`
    }
    case "step":
      return stepWords(label.step, context)
    case "pushIt":
      return "Not on the remote yet: push it"
    case "pushed":
      return "Pushed: it lands through its pull request"
    case "noPullRequest":
      return "Not in a pull request"
    case "draft":
      return `#${label.number} is a draft`
    case "changesRequestedBy":
      return `${joinNames(label.by)} requested changes on #${label.number}`
    case "changesRequested":
      return `Changes requested on #${label.number}`
    case "checksFail":
      return `Checks fail on #${label.number}`
    case "waitsFor":
      return `Waits for ${joinNames(label.reviewers)} on #${label.number}`
    case "landsThrough":
      return `Lands through #${label.number}`
  }
}

/** The reason of the step of a node, as its slot says it: a step that leaves the Safe Push enabled says it in its item */
function stepWords(step: Extract<NodeLabel, { kind: "step" }>["step"], context: NodeContext): string {
  const shown = presentNextStep(step, { ...context.step, busy: null, forStart: false })
  const reason = shown.safePushBlocker ?? shown.item?.description ?? ""
  switch (shown.kind) {
    case "pullRequest":
      return "Approvals count only in a pull request: Patronus checks them when the robot starts"
    case "requestReview":
      // nobody of a group can approve: the item says so, no request helps
      return shown.item?.disabled === true ? (shown.item.description ?? reason) : reason
    case "createPullRequest":
      // "In #12": its approvals count in the pull request of the whole stack, whose step is in the island header
      return shown.tone === "quiet" ? (shown.item?.description ?? reason) : reason
    case "none":
    case "hidden":
      return shown.safePushBlocker ?? "Nothing to land"
    case "pending":
      // nobody knows yet whether it can land: never a node that a later answer turns into another
      return shown.item?.description ?? formatPendingFact("readiness")
    default:
      return reason
  }
}

/** The node `node` of a row in the page's words, a running move of the row in place of a node that says so */
export function presentLandingNode(node: NodeView, context: NodeContext): LandingNode {
  if (context.move !== null && node.moveShows) {
    return moveNode(context.move)
  }
  // a node that asks for the push leaves the user's moves at the click of the push, not when the remote has it
  if (context.pushing && node.label.kind === "pushIt") {
    return { kind: "waiting", label: labelWords({ kind: "pushed" }, context) }
  }
  return {
    kind: node.kind,
    label: labelWords(node.label, context),
    ...(node.pending && { pending: true }),
    ...(node.robot && { robot: true }),
    ...(node.shows !== null && { shows: node.shows }),
  }
}
