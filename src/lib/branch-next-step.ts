import { ClockIcon, FileTextIcon, GitPullRequestArrowIcon, MessageSquareWarningIcon, ShieldCheckIcon, UserPlusIcon } from "lucide-react"
import type { MissingReviewGroup, Person, ReviewReadiness } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import type { SafePushSlot } from "@/lib/patronus"
import { describeStart, formatStartLabel } from "@/lib/patronus"
import { joinNames } from "@/lib/text"

/**
 * The next step of a branch, the first row of the table of [Next Step](../../spec/branch/next-step.spec.md) that
 * matches: what the Safe Push slot of its row shows, and whether "Safe Push" can open everywhere else
 */
export type NextStepKind =
  /** The repository is not on Space: no slot */
  | "hidden"
  /** Row 1: syncing, failed, blocked, held, landed, a robot runs or merged (also one of another branch that landed it); the Status cell says it */
  | "none"
  /** Rows 2 and 3: "Restart Safe Push" of the failed or canceled robot */
  | "restart"
  /** A robot that failed for approvals, while nobody can tell yet whether they count: its pull request */
  | "pullRequest"
  /** Row 5: the open pull request merges into a virtual branch (a parent's, landed or not): "Retarget #7 to master" */
  | "retarget"
  /** Row 5: the open pull request merges into another base on purpose maybe (a release branch): a quiet step that opens it */
  | "otherBase"
  /** Row 6: Patronus merges no draft */
  | "draft"
  /** Row 7: the approvals are missing and only a pull request can collect them */
  | "createPullRequest"
  /** Row 8: a code owner of a missing group requested changes */
  | "changesRequested"
  /** Row 9: a missing group has nobody asked to review */
  | "requestReview"
  /** Row 10: every missing group has someone asked: the wait */
  | "awaiting"
  /** Row 11 */
  | "safePush"

/** `quiet` is a state as a label (a wait, a draft): muted, it opens its object; `destructive` the same in red */
export type NextStepTone = "default" | "quiet" | "destructive"

/** The readiness of the branch as the store holds it (`review-readiness`); `stale` while a new answer loads */
export interface ReadinessFact {
  readonly readiness: ReviewReadiness
  readonly stale: boolean
}

/** The pull request of the branch, as the step needs it */
export interface NextStepPullRequest {
  readonly number: number
  readonly state: "open" | "closed" | "merged"
  readonly isDraft: boolean
  /** The base it merges into ("master"); empty when unknown */
  readonly base: string
  /** It merges into the target of Patronus, or either is unknown (`isPullRequestIntoTarget`) */
  readonly intoTarget: boolean
  /** Merged with the commits the branch has (not new work under the same name since) */
  readonly mergedCurrent: boolean
  /**
   * Its base is a virtual branch of the user (`isVirtualBranchBase`): a parent's branch, of a parent in the table or of
   * one that landed or is gone. Every pull request of a virtual branch goes into the target, so the retarget is what
   * the branch needs, never a choice
   */
  readonly baseIsVirtualBranch: boolean
}

export interface NextStepActions {
  /** The start of the Safe Push (`requestSafePush`): at once, or its popover at the row's slot */
  readonly openSafePush: () => void
  readonly openPullRequest: () => void
  /** Retargets the pull request to `NextStepFacts.target` */
  readonly retarget: () => void
  /** "Create Pull Request…" of the menu, which pushes first; null when the branch can't get one (no GitHub repository) */
  readonly createPullRequest: MenuItem | null
  /**
   * "Request Review…" of the open pull request (`getRequestReviewItem`), which opens its popover: disabled with the
   * reason for a draft and while the remote lacks work of the branch; null without an open pull request
   */
  readonly requestReview: MenuItem | null
}

/** What decides the next step of a branch; built from the stores by `getBranchNextStepNow`, plain data for the tests */
export interface NextStepFacts {
  readonly branchName: string
  /** What the texts call the object of the Safe Push ("Safe Push of auth waits for Ilya Korennoy"): the branch's name */
  readonly subject: string
  /** Patronus answers for the repository: it is on Space */
  readonly patronusAvailable: boolean
  readonly syncing: boolean
  readonly hasError: boolean
  readonly commitCount: number
  /** An inline action of the row runs */
  readonly processing: boolean
  /** A stacked branch whose parent failed to build (`Blocked`) */
  readonly blocked: boolean
  /**
   * Why a start is not possible by the robots (`canStartRobot`: blocked, a robot on this commit, one that lands it or an
   * ancestor of it: a stacked branch lands with what it is built on)
   */
  readonly startBlocker: string | null
  /** The same for a dry run, which may run while a robot lands an ancestor */
  readonly dryRunStartBlocker: string | null
  /** A Safe Push holds the pushes of the branch (`PushHold`): it runs on it or lands it */
  readonly held: boolean
  /** The baseline that has every commit of the branch ("origin/master"); null otherwise */
  readonly landedIn: string | null
  /**
   * A merged Safe Push of a branch built on it or of the whole stack landed the branch, before the fetch says so
   * (`getLandedBy`): the reason, "Landed in master with oauth" (`formatLandedReason`); null otherwise
   */
  readonly landedWith: string | null
  /** The robot of the row (`getSafePushSlot`) */
  readonly robot: SafePushSlot
  /** The pull request the robot was started from */
  readonly robotPullRequest: number | null
  /** "Restart Safe Push" of the robot, which pushes first when the remote lacks work (`getRobotActions`); null when it can't restart */
  readonly restart: MenuItem | null
  /** GitHub lists the pull requests of the repository: without it the table never offers one it may have */
  readonly pullRequestsAvailable: boolean
  readonly pullRequest: NextStepPullRequest | null
  /** The branch Patronus pushes into, as a pull request names its base ("master") */
  readonly target: string
  /** The branches a Safe Push of the branch lands in one robot: the branch and those it is built on (1 for a lone one) */
  readonly landsBranches: number
  /**
   * What the hover of the start says it lands, instead of `describeStart` of the branch and what it is built on: of the
   * whole stack, its branches (`describeWholeStackLanding`); null for a branch
   */
  readonly startDescription: string | null
  /** null until asked */
  readonly readiness: ReadinessFact | null
  /**
   * The open pull request of the whole stack the branch is in (`{prefix}/stack/<root>`), whose approvals count for the
   * Safe Push of the whole stack only: row 7 of a branch without its own pull request points to it ("In #12"); null
   * otherwise
   */
  readonly stackPullRequest: StackPullRequestFact | null
  readonly actions: NextStepActions
}

/** The open pull request of the whole stack a branch is in (`NextStepFacts.stackPullRequest`) */
export interface StackPullRequestFact {
  readonly number: number
  /** "the whole air stack" */
  readonly subject: string
  readonly open: () => void
}

export interface BranchNextStep {
  readonly kind: NextStepKind
  /** The action of the slot; null for `none` and `hidden` */
  readonly item: MenuItem | null
  readonly tone: NextStepTone
  /** Why "Safe Push" can't open now, the same on every surface; null when it can */
  readonly safePushBlocker: string | null
  /** Why a Dry Run can't start: never the approvals */
  readonly dryRunBlocker: string | null
  /** The step depends on the readiness of the branch: the store asks Patronus for it */
  readonly needsReadiness: boolean
  /**
   * The `info` toast of S while "Safe Push" is disabled: a short title naming the branch ("Safe Push of auth waits for
   * Ilya Korennoy"), the reason as the description; null when it can open
   */
  readonly notice: { readonly title: string; readonly description: string } | null
  /** "Retarget #7 to master" of an open pull request into another base, for the branch menu; null otherwise */
  readonly retarget: MenuItem | null
  /**
   * Row 7 only: a Safe Push without review (a merge profile that skips the code-owner review) is offered beside the
   * pull request, with why it can't start now (the reasons of the branch, never the approvals); null elsewhere
   */
  readonly withoutReview: { readonly blocker: string | null } | null
}

/** "Ilya Korennoy", the login without a full name */
export function formatPerson(person: Person): string {
  return person.fullName ?? person.login ?? "a reviewer"
}

/** "1 more approval", "2 more approvals" */
export function formatMoreApprovals(needed: number): string {
  return needed === 1 ? "1 more approval" : `${needed} more approvals`
}

/** The people of the groups, each once, by login (else by name) */
function distinctPeople(people: readonly Person[]): Person[] {
  const seen = new Set<string>()
  return people.filter(person => {
    const key = (person.login ?? person.fullName ?? "").toLowerCase()
    if (seen.has(key)) {
      return false
    }
    seen.add(key)
    return true
  })
}

function groupNames(groups: readonly MissingReviewGroup[]): string {
  return joinNames(groups.map(group => group.name))
}

/** "Ilya Korennoy is requested", "Ilya Korennoy and Alice are requested" */
function formatRequested(people: readonly Person[]): string {
  return `${joinNames(people.map(formatPerson))} ${people.length === 1 ? "is" : "are"} requested`
}

/** The awaited people of the missing groups */
export function awaitedPeople(groups: readonly MissingReviewGroup[]): Person[] {
  return distinctPeople(groups.flatMap(group => group.requested))
}

/** "Awaiting Ilya Korennoy", "Awaiting 2 Reviews": the label of the wait */
export function formatAwaitingLabel(people: readonly Person[]): string {
  return people.length === 1 ? `Awaiting ${formatPerson(people[0]!)}` : `Awaiting ${people.length} Reviews`
}

/**
 * The lines of the hover of the wait, one per missing group: "Awaiting Ilya Korennoy · IJPL Code Platform", "1 more
 * approval · IJPL Code Platform" for a group that has approvals already or needs more than one
 */
export function describeAwaiting(groups: readonly MissingReviewGroup[]): string[] {
  return groups.map(group => {
    const needed = group.needed ?? 1
    const waiting = group.needed !== null && (needed > 1 || group.approvedBy.length > 0) ? formatMoreApprovals(needed) : formatAwaitingLabel(group.requested)
    return `${waiting} · ${group.name}`
  })
}

/** Why Safe Push waits for the approvals of the missing groups (rows 7 to 10), with the pull request where they count */
function formatApprovalBlocker(kind: NextStepKind, groups: readonly MissingReviewGroup[], pullRequest: number | null): string {
  const names = groupNames(groups)
  const on = pullRequest === null ? "" : ` on #${pullRequest}`
  switch (kind) {
    case "createPullRequest":
      return `Code owners must approve in a pull request: ${names}`
    case "changesRequested": {
      const people = distinctPeople(groups.flatMap(group => group.changesRequestedBy))
      return `${joinNames(people.map(formatPerson))} requested changes${on}`
    }
    case "awaiting":
      return `Waits for the approval of ${names}: ${formatRequested(awaitedPeople(groups))}${on}`
    default: {
      const asked = groups.some(group => group.requested.length > 0)
      return asked
        ? `Waits for the approval of ${names}: fewer people are requested${on} than it needs`
        : `Waits for the approval of ${names}: nobody of ${groups.length === 1 ? "it" : "them"} is requested${on}`
    }
  }
}

/**
 * A missing group asks fewer people than the approvals it still needs (none asked, or one asked of a group that needs two):
 * waiting for them can't satisfy it, Request Review… can
 */
export function isShortOfRequests(group: MissingReviewGroup): boolean {
  return group.requested.length < (group.needed ?? 1)
}

/** Nobody of a missing group can approve (`noEligibleReviewer`): Request Review… can't help */
function formatNoEligibleReviewer(groups: readonly MissingReviewGroup[]): string {
  return `Nobody of ${groupNames(groups)} can approve these changes`
}

function step(kind: NextStepKind, item: MenuItem | null, safePushBlocker: string | null, dryRunBlocker: string | null, extra: Partial<BranchNextStep> = {}): BranchNextStep {
  return { kind, item, tone: "default", safePushBlocker, dryRunBlocker, needsReadiness: false, notice: null, retarget: null, withoutReview: null, ...extra }
}

/** The notice of a blocker no row words itself: "Safe Push of auth can't start" with the reason */
function withNotice(facts: NextStepFacts, result: BranchNextStep): BranchNextStep {
  const reason = result.safePushBlocker
  if (reason === null) {
    return result.notice === null ? result : { ...result, notice: null }
  }
  return result.notice === null ? { ...result, notice: { title: `Safe Push of ${facts.subject} can't start`, description: reason } } : result
}

/** The notice of a row's own reason; a reason of the branch (a sync, a robot) outranks it */
function rowNotice(general: string | null, title: string, description: string): Partial<BranchNextStep> {
  return general === null ? { notice: { title, description } } : {}
}

/** Why no start is possible now, whatever the approvals: the first of the reasons; null when one is */
function startReason(facts: NextStepFacts, startBlocker: string | null): string | null {
  if (facts.hasError) {
    return "The branch failed to sync"
  }
  if (facts.commitCount === 0) {
    return "No commits to push"
  }
  if (facts.syncing) {
    return "Wait for the sync to finish"
  }
  if (startBlocker !== null) {
    return startBlocker
  }
  if (facts.landedIn !== null) {
    return `Landed in ${facts.landedIn}`
  }
  if (facts.landedWith !== null) {
    return facts.landedWith
  }
  if (isMergedByRobot(facts)) {
    return "Merged by Patronus"
  }
  if (facts.pullRequest?.state === "merged" && facts.pullRequest.mergedCurrent) {
    return `Pull request #${facts.pullRequest.number} is merged`
  }
  if (facts.processing) {
    return "Wait for the action on the branch to finish"
  }
  return null
}

function isMergedByRobot(facts: NextStepFacts): boolean {
  return facts.robot.kind === "finished" && facts.robot.chip.kind === "merged"
}

/** Row 1: the branch has nothing to start now, and its Status cell says why */
function isSettled(facts: NextStepFacts): boolean {
  return (
    !facts.patronusAvailable ||
    facts.syncing ||
    facts.hasError ||
    facts.commitCount === 0 ||
    facts.blocked ||
    facts.held ||
    facts.landedIn !== null ||
    facts.landedWith !== null ||
    facts.robot.kind === "running" ||
    facts.robot.kind === "none" ||
    isMergedByRobot(facts) ||
    (facts.pullRequest?.state === "merged" && facts.pullRequest.mergedCurrent)
  )
}

function safePushItem(facts: NextStepFacts, blocker: string | null): MenuItem {
  return {
    id: "safe-push",
    label: formatStartLabel("rebaseAutosquash", facts.landsBranches),
    icon: ShieldCheckIcon,
    kbds: ["s"],
    disabled: blocker !== null,
    description: blocker ?? facts.startDescription ?? describeStart("rebaseAutosquash", facts.subject, facts.landsBranches, facts.target),
    onSelect: facts.actions.openSafePush,
  }
}

/** A quiet step: the state as its label, opening the pull request it waits in (O on the row opens it too) */
function quietItem(facts: NextStepFacts, label: string, description: string, icon: MenuItem["icon"]): MenuItem {
  return { id: "open-pull-request", label, icon, kbds: ["o"], description, onSelect: facts.actions.openPullRequest }
}

/**
 * The next step of a branch: the first row of the table that matches (see `NextStepKind`). Pure: the facts carry the
 * actions, so every surface renders the same items.
 */
export function getBranchNextStep(facts: NextStepFacts): BranchNextStep {
  // a dry run pushes nothing into the target: approvals, a landing ancestor and a draft don't hold it back
  const dryRunBlocker = startReason(facts, facts.dryRunStartBlocker)
  const general = startReason(facts, facts.startBlocker)
  // the surfaces offer a start only where Patronus answers: the blockers stay those of the branch
  if (!facts.patronusAvailable && facts.robot.kind === "none") {
    return step("hidden", null, general, dryRunBlocker)
  }
  if (isSettled(facts)) {
    return withNotice(facts, step("none", null, general, dryRunBlocker))
  }
  const result = withNotice(facts, nextStep(facts, general, dryRunBlocker))
  // an action of the row waits for the inline action that runs; a quiet step only opens the pull request
  if (facts.processing && result.item !== null && result.tone === "default" && result.item.disabled !== true) {
    return { ...result, item: { ...result.item, disabled: true, description: "Wait for the action on the branch to finish" } }
  }
  return result
}

/** Rows 2 to 11, for a branch that can start; `general` is why a start can't whatever the approvals */
function nextStep(facts: NextStepFacts, general: string | null, dryRunBlocker: string | null): BranchNextStep {
  const robot = facts.robot
  const pullRequest = facts.pullRequestsAvailable ? facts.pullRequest : null
  const open = pullRequest?.state === "open" ? pullRequest : null
  const readiness = facts.pullRequestsAvailable ? (facts.readiness?.readiness ?? null) : null
  // Patronus judges the local commits: an unpushed branch is asked too, so its step is never a leftover answer
  const asks = facts.pullRequestsAvailable
  const approved = readiness?.kind === "approved" || readiness?.kind === "notRequired"

  if (robot.kind === "finished") {
    const chip = robot.chip.kind
    // row 2
    if (chip === "failed" || chip === "canceled") {
      return facts.restart === null ? step("safePush", safePushItem(facts, general), general, dryRunBlocker) : step("restart", facts.restart, general, dryRunBlocker)
    }
    if (chip === "needsReview") {
      // row 3: the restart keeps the pull request of the robot, it counts the approvals it has by now
      const linked = open !== null && open.intoTarget && facts.robotPullRequest === open.number
      if (linked && approved && facts.restart !== null) {
        return step("restart", facts.restart, general, dryRunBlocker, { needsReadiness: asks })
      }
      // nobody can tell yet whether the approvals count: a restart may fail again, the pull request collects them
      if (readiness === null || readiness.kind === "unknown" || readiness.kind === "unavailable") {
        const item = open === null ? facts.actions.createPullRequest : quietItem(facts, "Open Pull Request", `Open Pull Request #${open.number}`, GitPullRequestArrowIcon)
        return item === null
          ? step("safePush", safePushItem(facts, general), general, dryRunBlocker, { needsReadiness: asks })
          : step("pullRequest", item, general, dryRunBlocker, { needsReadiness: asks })
      }
    }
  }

  // row 5: Patronus merges a pull request into its base, so a start from it would not land in the target
  if (open !== null && !open.intoTarget) {
    const reason = `Approvals in #${open.number} don't count: its base is ${open.base}`
    const retarget: MenuItem = {
      id: "retarget-pull-request",
      label: `Retarget #${open.number} to ${facts.target}`,
      icon: GitPullRequestArrowIcon,
      description: `Changes the base of #${open.number} from ${open.base} to ${facts.target} on GitHub: Patronus merges a pull request into its base`,
      onSelect: facts.actions.retarget,
    }
    const notice = rowNotice(general, `Approvals in #${open.number} of ${facts.subject} don't count`, `Its base is ${open.base}, not ${facts.target}`)
    // a parent's branch: every pull request of a virtual branch goes into the target, with what it lands
    if (open.baseIsVirtualBranch) {
      return step("retarget", { ...retarget, description: `${reason}, a virtual branch. ${retarget.description}` }, general ?? reason, dryRunBlocker, { retarget, ...notice })
    }
    // another base may be meant (a release branch): the step opens the pull request, the retarget is in the menu
    const item = { ...quietItem(facts, "Open Pull Request", `${reason}. Open Pull Request #${open.number}`, GitPullRequestArrowIcon), id: "open-pull-request-other-base" }
    return step("otherBase", item, general ?? reason, dryRunBlocker, { tone: "quiet", retarget, ...notice })
  }
  // row 6
  if (open?.isDraft === true) {
    const reason = `#${open.number} is a draft: Patronus merges it once it is ready for review`
    const item = { ...quietItem(facts, "Draft", `${reason}. Open Pull Request #${open.number}`, FileTextIcon), id: "open-pull-request-draft" }
    return step("draft", item, general ?? reason, dryRunBlocker, {
      tone: "quiet",
      ...rowNotice(general, `#${open.number} of ${facts.subject} is a draft`, "Patronus merges it once it is ready for review"),
    })
  }

  if (readiness?.kind !== "missing") {
    // row 11; row 3 for a robot whose approvals arrived, but not in the pull request it was started from
    return step("safePush", safePushItem(facts, general), general, dryRunBlocker, { needsReadiness: asks })
  }

  const groups = readiness.groups
  // row 7: a start through the API counts no approval
  if (open === null) {
    const reason = formatApprovalBlocker("createPullRequest", groups, null)
    const stack = facts.stackPullRequest
    // the pull request of the whole stack collects the approvals of its own Safe Push: the quiet way to it
    if (stack !== null) {
      const counts = `Approvals in #${stack.number}, the pull request of ${stack.subject}, count for its Safe Push only`
      const item: MenuItem = {
        id: "open-stack-pull-request",
        label: `In #${stack.number}`,
        icon: GitPullRequestArrowIcon,
        description: `${reason}\n${counts}\nOpen Pull Request #${stack.number}`,
        onSelect: stack.open,
      }
      return step("createPullRequest", item, general ?? reason, dryRunBlocker, {
        tone: "quiet",
        needsReadiness: asks,
        withoutReview: { blocker: general },
        ...rowNotice(general, `Safe Push of ${facts.subject} needs a pull request`, `Code owners must approve in it: ${groupNames(groups)} · ${counts}`),
      })
    }
    const item = facts.actions.createPullRequest ?? {
      id: "create-pull-request",
      label: "Create Pull Request…",
      icon: GitPullRequestArrowIcon,
      disabled: true,
      description: "The GitHub repository of the branches is unknown",
      onSelect: () => {},
    }
    return step("createPullRequest", { ...item, description: item.disabled === true ? item.description : reason }, general ?? reason, dryRunBlocker, {
      needsReadiness: asks,
      withoutReview: { blocker: general },
      ...rowNotice(general, `Safe Push of ${facts.subject} needs a pull request`, `Code owners must approve in it: ${groupNames(groups)}`),
    })
  }
  // row 8: only the changes requested by a member of a group still short of approvals block, as Patronus decides
  if (groups.some(group => group.changesRequestedBy.length > 0)) {
    const reason = formatApprovalBlocker("changesRequested", groups, open.number)
    return step(
      "changesRequested",
      quietItem(facts, "Changes Requested", `${reason}. Open Pull Request #${open.number}`, MessageSquareWarningIcon),
      general ?? reason,
      dryRunBlocker,
      {
        tone: "destructive",
        needsReadiness: asks,
        ...rowNotice(general, `Safe Push of ${facts.subject} waits for changes`, `${reason}`),
      },
    )
  }
  // row 9
  if (readiness.reason === "noEligibleReviewer" || groups.some(isShortOfRequests)) {
    const short = groups.filter(isShortOfRequests)
    const reason = formatApprovalBlocker("requestReview", short.length === 0 ? groups : short, open.number)
    const requestReview = facts.actions.requestReview ?? { id: "request-review", label: "Request Review…", icon: UserPlusIcon, onSelect: () => {} }
    // nobody of a group can approve: no request helps
    const item: MenuItem =
      readiness.reason === "noEligibleReviewer"
        ? { ...requestReview, disabled: true, description: formatNoEligibleReviewer(groups) }
        : requestReview.disabled === true
          ? requestReview
          : { ...requestReview, description: `${reason}. ${requestReview.description ?? ""}`.trim() }
    const asked = short.length === 0 ? groups : short
    return step("requestReview", item, general ?? reason, dryRunBlocker, {
      needsReadiness: asks,
      ...rowNotice(general, `Safe Push of ${facts.subject} waits for a review request`, `${groupNames(asked)} · nobody is requested on #${open.number}`),
    })
  }
  // row 10
  const reason = formatApprovalBlocker("awaiting", groups, open.number)
  const hover = [...describeAwaiting(groups), `Open Pull Request #${open.number}`].join("\n")
  const people = awaitedPeople(groups)
  const waitsFor = people.length === 1 ? formatPerson(people[0]!) : `${people.length} reviews`
  return step("awaiting", quietItem(facts, formatAwaitingLabel(people), hover, ClockIcon), general ?? reason, dryRunBlocker, {
    tone: "quiet",
    needsReadiness: asks,
    ...rowNotice(general, `Safe Push of ${facts.subject} waits for ${waitsFor}`, `${groupNames(groups)} · requested on #${open.number}`),
  })
}
