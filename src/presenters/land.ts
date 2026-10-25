// The page's side of the decision of a start Rust makes (`LandDecision` of `domain_policy::land::decide`: the view's
// `lands` for the default starts, the answer of `decide_land` for another one): what a click does and what the popover
// of a start says, in the page's words. It decides nothing: the next step's reason still comes first (the step is the
// page's until the next step moves to Rust), a key always opens the popover, and the only blocker the popover lifts is
// the one an allowance of the start names, a refusal of the landing gate (Start Anyway).
import type { Confirmation, FactWait, LandDecision, Reason, StartRoute } from "@/lib/bindings"
import { formatReason } from "@/presenters/reasons"

/** A line the popover of a start shows before it starts, one per confirmation of the decision, and S itself */
export type PromptLine =
  /** S, never a start by itself */
  | "key"
  /** It lands more than its row: the branches it is built on */
  | "stack"
  /** A profile other than the default checks */
  | "profile"
  /** It cancels the Safe Push running on an older commit */
  | "replaces"
  /** Patronus judges the approvals only at the start, or couldn't answer the review */
  | "review"
  /** The push dismisses approvals of the pull request (not the ones Patronus counts) */
  | "dismisses"
  /** GitHub didn't answer the rules of the target: whether the push keeps the approvals is not known */
  | "rulesUnknown"
  /** Without review: the profile skips the code-owner review */
  | "bypass"
  /** A piece of a stack whose pull request is open: those approvals count for the whole stack only */
  | "wholeStack"
  /** Its build needs a branch outside what it lands (the landing gate): the fix, or Start Anyway */
  | "landing"
  /** Its push replaces commits of the remote someone else pushed */
  | "force"

/** What a start does now, as the click and the popover present it */
export type LandPrompt =
  /** A fact it decides on has no answer yet: the start waits, its button busy with `what` */
  | { readonly kind: "wait"; readonly what: string }
  /**
   * It can't start: `reason` said; `offersPush`: the push alone helps; `opens`: the object the reason names, which the
   * toast opens (the pull request of a draft, the robot that holds it), or `review`: the popover, whose review line names
   * the approvals it misses and where they count
   */
  | { readonly kind: "refuse"; readonly reason: string; readonly offersPush: boolean; readonly opens?: "pullRequest" | "robot" | "review" }
  /** A Safe Push without review waits for its profile */
  | { readonly kind: "choose"; readonly reason: string }
  /** The popover first, with the lines `reasons` names */
  | { readonly kind: "confirm"; readonly reasons: readonly PromptLine[] }
  /** Nothing to say: it starts */
  | { readonly kind: "start" }

/** How a start was asked, and what of it the page still decides itself */
export interface LandAsk {
  /** What the texts call what starts: "auth", "the whole air stack" */
  readonly subject: string
  /** A click starts at once when there is nothing to say; a key (S) always opens the popover, which Enter confirms */
  readonly via: "click" | "key"
  /** The reason of the next step of the branch that holds the start back (the page's next step); null: none */
  readonly stepBlocker: string | null
  /** A Safe Push without review was asked and applies (the approvals missing or not known yet): it needs a profile that skips the review */
  readonly withoutReview: boolean
}

/** The refusals of the landing gate among the blockers: Start Anyway lifts them (`LandingAllowances`) */
export function landingRefusalsOf(decision: LandDecision): Extract<Reason, { code: "landingRefused" }>[] {
  return decision.blockers.filter((reason): reason is Extract<Reason, { code: "landingRefused" }> => reason.code === "landingRefused")
}

/** How the robot starts: through the API with a profile, or from a pull request; null when nothing starts it */
export function startRouteOf(decision: LandDecision): StartRoute | null {
  for (const effect of decision.effects) {
    if (effect.kind === "startRobot") {
      return effect.route
    }
  }
  return null
}

/** The words of what a start waits for */
function waitWords(wait: FactWait, subject: string): string {
  switch (wait.fact.kind) {
    case "remote":
      return `Checking ${subject}…`
    case "profiles":
      return "Asking Patronus for the merge profiles…"
    case "pullRequests":
    case "readiness":
      return "Checking the code-owner review…"
    case "rules":
      return `Checking whether the push keeps the approvals of #${wait.fact.number}…`
    case "landingNeeds":
      return `Checking what ${subject} needs to land…`
  }
}

/** The order the waits are said in: the subject itself first, then the profiles, the review, the rules, the build */
const WAIT_ORDER: readonly FactWait["fact"]["kind"][] = ["remote", "profiles", "pullRequests", "readiness", "rules", "landingNeeds"]

/** The popover line of a confirmation */
function lineOf(confirmation: Confirmation): PromptLine {
  switch (confirmation.kind) {
    case "landsMoreThanRow":
      return "stack"
    case "nonDefaultProfile":
      return "profile"
    case "cancelsRobot":
      return "replaces"
    case "approvalsJudgedAtStart":
    case "reviewUnknown":
      return "review"
    case "dismissesApprovals":
      return "dismisses"
    case "rulesUnknown":
      return "rulesUnknown"
    case "withoutApprovals":
      return "bypass"
    case "stackPullRequestApprovalsDontCount":
      return "wholeStack"
    case "forcePushDrops":
      return "force"
  }
}

/** The object a refusal names, which its toast opens; the approvals it misses are the popover's review line */
function opensOf(reason: Reason): "pullRequest" | "robot" | "review" | undefined {
  switch (reason.code) {
    case "draft":
      return "pullRequest"
    case "held":
    case "stackHeld":
      return "robot"
    case "approvalsMissing":
      return "review"
    default:
      return undefined
  }
}

/**
 * What a start of `ask` does by `decision` (null: Rust has none yet, before the Patronus view): the next step's reason
 * first; a fact whose read failed refuses with why; then the first blocker of the decision, but a refusal of the landing
 * gate, which the popover offers to start anyway; then a fact without an answer, a wait; a start without review still
 * without its profile, the choice; then the lines the popover says first, else the start
 */
export function presentLand(decision: LandDecision | null, ask: LandAsk): LandPrompt {
  if (ask.stepBlocker !== null) {
    return { kind: "refuse", reason: ask.stepBlocker, offersPush: false }
  }
  if (decision === null) {
    return { kind: "wait", what: `Reading ${ask.subject}…` }
  }
  const failed = decision.waits.find(wait => wait.failed !== null)
  if (failed?.failed != null) {
    return { kind: "refuse", reason: failed.failed.message, offersPush: false }
  }
  const blocker = decision.blockers.find(reason => reason.code !== "landingRefused")
  if (blocker !== undefined) {
    const opens = opensOf(blocker)
    return { kind: "refuse", reason: formatReason(blocker), offersPush: blocker.code === "dismissesApprovals", ...(opens !== undefined && { opens }) }
  }
  const wait = WAIT_ORDER.flatMap(kind => decision.waits.filter(it => it.fact.kind === kind))[0]
  if (wait !== undefined) {
    return { kind: "wait", what: waitWords(wait, ask.subject) }
  }
  const route = startRouteOf(decision)
  if (ask.withoutReview && route?.kind === "api" && route.profile === null) {
    return { kind: "choose", reason: "Choose the profile that skips the code-owner review" }
  }
  const reasons: PromptLine[] = ask.via === "key" ? ["key"] : []
  const lines = decision.confirmations.map(lineOf)
  for (const line of ask.withoutReview && !lines.includes("bypass") ? [...lines, "bypass" as const] : lines) {
    if (!reasons.includes(line)) {
      reasons.push(line)
    }
  }
  if (landingRefusalsOf(decision).length > 0) {
    reasons.push("landing")
  }
  return reasons.length === 0 ? { kind: "start" } : { kind: "confirm", reasons }
}
