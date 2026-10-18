import type { PatronusProfile, PatronusStartOptions, ReviewReadiness } from "@/lib/bindings"
import type { NextStepKind } from "@/lib/branch-next-step"
import type { PullRequestStart, StartPush } from "@/lib/patronus"
import type { SafePushMode } from "@/stores/safe-push"

/** The merge profiles of the target, asked once per start (Patronus caches nothing for the app) */
export type StartOptionsFact =
  | { readonly status: "loading" }
  | { readonly status: "loaded"; readonly options: PatronusStartOptions }
  | { readonly status: "error"; readonly message: string }

/**
 * The code-owner review of a Safe Push as Patronus answered it for the branch as it is (a stale answer is `loading`):
 * `none` for a dry run, which is never checked; `unlisted` while GitHub doesn't list the pull requests (the start isn't
 * known); `space` for a repository only on Space, whose start the robot decides
 */
export type ReviewFact =
  | { readonly kind: "none" }
  | { readonly kind: "unlisted" }
  | { readonly kind: "loading" }
  | { readonly kind: "loaded"; readonly readiness: ReviewReadiness }
  | { readonly kind: "space"; readonly required: boolean }

/** A push into an approved pull request: whether the rules of the repository dismiss its approvals; null while asked */
export interface DismissFact {
  readonly pullRequest: number
  readonly dismisses: boolean | null
}

/** Everything a start of a robot decides on, read when it is asked (`readSafePushFacts`) and while its popover is open */
export interface SafePushFacts {
  /** The branch, or the key of the whole stack of a root (`stackReadinessKey`) */
  readonly branchName: string
  /** What the texts call what starts: the branch's name, "the whole air stack" */
  readonly subject: string
  readonly mode: SafePushMode
  /** A click starts at once when there is nothing to say; a key (S) always opens the popover, which Enter confirms */
  readonly via: "click" | "key"
  /**
   * The next step of the branch with the fresh review only (`getBranchNextStep`): why every surface disables the start
   * (`safePushBlocker`, a dry run's `dryRunBlocker`), and its kind, to know the approvals among the reasons
   */
  readonly step: { readonly kind: NextStepKind; readonly blocker: string | null }
  /** The target branch ("master") */
  readonly target: string
  readonly options: StartOptionsFact
  /** The profile the start runs with: the chosen one, else the branch's own (its last robot's); null: the default checks */
  readonly profile: PatronusProfile | null
  /** A Safe Push without review: the profile has to skip the code-owner review, and the approvals don't hold it back */
  readonly bypassReview: boolean
  /** The open pull request into the target the start comments on (`/patronus merge`); null: through the API */
  readonly fromPullRequest: PullRequestStart | null
  /** What the start pushes first: nothing, the branch, the stack (`getStartPush`); `unknown` until the remote status loads */
  readonly startPush: StartPush
  /** The branch has a tip in the list */
  readonly hasTip: boolean
  /** A Safe Push of the branch runs on an older commit: the start cancels it (its source revision) */
  readonly replaces: string | null
  /** A push the start needs goes into a branch a Safe Push holds (a dry run): the reason, as the push button says it */
  readonly pushHeld: string | null
  readonly review: ReviewFact
  readonly dismiss: DismissFact | null
  /** The branches the branch is built on: its Safe Push lands them with it, in one robot */
  readonly ancestorCount: number
  /**
   * The open pull request of the whole stack the branch is in: its approvals count for the Safe Push of the whole stack
   * only, so a start of a piece of it says so first; null otherwise (and for the whole stack itself)
   */
  readonly wholeStackPullRequest: number | null
}

/** Why a start opens its popover instead of starting at once: each a line of the popover */
export type ConfirmReason =
  /** S, never a start by itself */
  | "key"
  /** It lands more than its row: the branches it is built on */
  | "stack"
  /** A profile other than the default checks: the branch's own (its last robot's), or one chosen */
  | "profile"
  /** It cancels the Safe Push running on an older commit */
  | "replaces"
  /** Nobody but Patronus can tell yet whether the approvals count, or a repository only on Space needs them */
  | "review"
  /** The push dismisses approvals of the pull request (not the ones Patronus counts) */
  | "dismisses"
  /** Without review: the profile skips the code-owner review */
  | "bypass"
  /** A piece of a stack whose pull request is open: the approvals of that pull request don't count for this start */
  | "wholeStack"

export type SafePushDecision =
  /** A fact it decides on loads: the start waits, its button busy with `what` */
  | { readonly kind: "wait"; readonly what: string }
  /** Patronus would refuse or fail it, or the branch can't: no start, `reason` said; `offersPush`: the push alone helps */
  | { readonly kind: "refuse"; readonly reason: string; readonly offersPush: boolean }
  /** A Safe Push without review waits for its profile */
  | { readonly kind: "choose"; readonly reason: string }
  /** The popover first, with the lines `reasons` names */
  | { readonly kind: "confirm"; readonly reasons: readonly ConfirmReason[] }
  /** Nothing to say: it starts */
  | { readonly kind: "start" }

/** The steps of the next step that wait for approvals: a profile that skips the review, or a start without it, needs none */
const APPROVAL_STEPS: ReadonlySet<NextStepKind> = new Set(["createPullRequest", "changesRequested", "requestReview", "awaiting"])

/** The approvals don't hold the start back: a start without review, or a profile known to skip it (`skipsReview`) */
function skipsApprovals(facts: SafePushFacts): boolean {
  return facts.mode === "rebaseAutosquash" && (facts.bypassReview || facts.profile?.skipsReview === true)
}

/**
 * What a start of a robot does now, the one decision of every surface that asks it (the slot, the menus, S, the preview,
 * the review window, the island header) and of its popover: first what holds it back (the next step of the branch, the
 * profiles, the remote, a hold, the review, the rules of the repository), each a wait while it loads; then whether it
 * has something to say before it starts (`confirm`): it lands more than its row, runs another profile than the default
 * checks, cancels a running Safe Push, Patronus can't judge the approvals yet, or it lands a piece of a stack whose pull
 * request is open (whose approvals count for the whole stack only). A key always confirms.
 */
export function getSafePushDecision(facts: SafePushFacts): SafePushDecision {
  const skips = skipsApprovals(facts)
  if (facts.step.blocker !== null && !(skips && APPROVAL_STEPS.has(facts.step.kind))) {
    return { kind: "refuse", reason: facts.step.blocker, offersPush: false }
  }
  if (!facts.hasTip) {
    return { kind: "wait", what: `Reading ${facts.subject}…` }
  }
  const { options } = facts
  if (options.status === "loading") {
    return { kind: "wait", what: "Asking Patronus for the merge profiles…" }
  }
  if (options.status === "error") {
    return { kind: "refuse", reason: options.message, offersPush: false }
  }
  if (!options.options.supported) {
    return { kind: "refuse", reason: `Patronus does not push into ${options.options.targetBranch}`, offersPush: false }
  }
  if (facts.startPush.kind === "unknown") {
    return { kind: "wait", what: `Checking ${facts.subject}…` }
  }
  const pullRequest = facts.fromPullRequest
  if (pullRequest?.blockedByDraft === true) {
    return { kind: "refuse", reason: `#${pullRequest.number} is a draft: Patronus merges it once it is ready for review`, offersPush: false }
  }
  if (facts.pushHeld !== null) {
    return { kind: "refuse", reason: facts.pushHeld, offersPush: false }
  }
  const { review } = facts
  if (review.kind === "loading") {
    return { kind: "wait", what: "Checking the code-owner review…" }
  }
  if (review.kind === "loaded" && review.readiness.kind === "missing" && !skips) {
    return { kind: "refuse", reason: "Patronus checks the approvals when the robot starts and fails without them", offersPush: false }
  }
  const { dismiss } = facts
  if (dismiss?.dismisses === null) {
    return { kind: "wait", what: `Checking whether the push keeps the approvals of #${dismiss.pullRequest}…` }
  }
  const dismisses = dismiss?.dismisses === true
  // the approvals Patronus counts would be gone with the push: the robot would start without them and fail
  if (dismisses && review.kind === "loaded" && review.readiness.kind === "approved") {
    return { kind: "refuse", reason: `The push dismisses the approvals of #${dismiss!.pullRequest}: push first, then ask for the review again`, offersPush: true }
  }
  if (facts.bypassReview && facts.profile === null) {
    return { kind: "choose", reason: "Choose the profile that skips the code-owner review" }
  }

  const reasons: ConfirmReason[] = []
  if (facts.via === "key") {
    reasons.push("key")
  }
  if (facts.ancestorCount > 0) {
    reasons.push("stack")
  }
  if (facts.profile !== null) {
    reasons.push("profile")
  }
  if (facts.replaces !== null) {
    reasons.push("replaces")
  }
  if (isReviewUnjudged(review)) {
    reasons.push("review")
  }
  if (dismisses) {
    reasons.push("dismisses")
  }
  if (facts.bypassReview) {
    reasons.push("bypass")
  }
  if (facts.wholeStackPullRequest !== null) {
    reasons.push("wholeStack")
  }
  return reasons.length === 0 ? { kind: "start" } : { kind: "confirm", reasons }
}

/** Patronus can't tell before the start whether the approvals count: the popover says so first */
function isReviewUnjudged(review: ReviewFact): boolean {
  switch (review.kind) {
    case "unlisted":
      return true
    case "space":
      return review.required
    case "loaded":
      return review.readiness.kind === "unknown" || review.readiness.kind === "unavailable"
    case "none":
    case "loading":
      return false
  }
}
