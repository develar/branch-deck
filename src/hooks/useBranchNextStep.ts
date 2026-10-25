import { ClipboardCopyIcon, RefreshCwIcon } from "lucide-react"
import { useStore } from "zustand"
import { getCreatePullRequestItem } from "@/hooks/useBranchContextActions"
import { requestSafePush } from "@/hooks/useSafePush"
import { restackBranch } from "@/hooks/useStackActions"
import type { ApprovalNews } from "@/lib/approval-news"
import { describeApprovalNews } from "@/lib/approval-news"
import type { LandingFix, PatronusRobot, PullRequest } from "@/lib/bindings"
import type { BranchNextStep, NextStepBusy, NextStepFacts, NextStepPullRequest, StackPullRequestFact, UnavailableFact } from "@/lib/branch-next-step"
import { formatPendingFact, getBranchNextStep, landedReasonOf } from "@/lib/branch-next-step"
import { copyText } from "@/lib/clipboard"
import { notifyError } from "@/lib/errors"
import { closeEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import type { MenuItem } from "@/lib/menu"
import { announce } from "@/lib/os-notification"
import { formatPatronusAvailabilityHint, formatPushMode, formatSafePushLoginCommand, getSafePushSlot, isLandedByOwnRobot } from "@/lib/patronus"
import type { RobotAction } from "@/lib/patronus-actions"
import { getRobotActions } from "@/lib/patronus-actions"
import { isWindows } from "@/lib/platform"
import { changesRequestedBy, formatAvailabilityHint, openPullRequest, waitingReviewers } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { stackProblemOf } from "@/lib/stack"
import { showToast } from "@/lib/toast-message"
import { formatWholeStack, stackReadinessKey } from "@/lib/whole-stack"
import { useShowReviewReadiness } from "@/model/demand"
import { formatReasonOrNull } from "@/presenters/reasons"
import { recordApproval, recordRetarget } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getLandedState, getOpenPullRequest, getParkedState, getStackBranchOf, useBranchSync } from "@/stores/branch-sync"
import type { Operations } from "@/stores/operations"
import { getObjectBusy, operationLabel, operationsStore, runOperation } from "@/stores/operations"
import {
  getBranchLanding,
  getBranchRobot,
  getLandedBy,
  getLandingAncestor,
  getLandingBranchCount,
  getLandingNeeds,
  getPatronusAvailability,
  getPatronusViewFailure,
  getPushHold,
  getRobotProblemsFailure,
  hasPatronusListed,
  isLandingNeedsPending,
  isPatronusAvailable,
  isPatronusPending,
  isRobotFailing,
  isRobotNeedingReview,
  isRobotProblemsPending,
  isTableRobot,
  patronusStore,
  refreshPatronus,
} from "@/stores/patronus"
import {
  getPullRequest,
  getPullRequestAvailability,
  getPullRequestListing,
  getStackPullRequest,
  isPullRequestsLoading,
  pullRequestsStore,
  retargetPullRequest,
} from "@/stores/pull-requests"
import { getSelectedProject, isSelectedRepository, repositoryStore } from "@/stores/repository"
import { openRequestReview } from "@/stores/request-review-popover"
import { freshReadiness, getReadinessTarget, getReviewReadiness, reviewReadinessStore } from "@/stores/review-readiness"
import { getSafePushStarting, safePushStore } from "@/stores/safe-push"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

/** The pull request of the branch as the next step needs it */
function pullRequestFactOf(branch: Branch): NextStepPullRequest | null {
  const pullRequest = getPullRequest(branch.name)
  if (pullRequest === undefined) {
    return null
  }
  const base = getOpenPullRequest(branch.name)?.base
  return {
    number: pullRequest.number,
    state: pullRequest.state,
    isDraft: pullRequest.isDraft,
    base: pullRequest.baseRefName ?? "",
    // where an open one goes, as Rust reads it (`pull_request::base_of`)
    intoTarget: base === undefined || base.kind === "target",
    baseIsVirtualBranch: base?.kind === "virtual",
    waitingFor: waitingReviewers(pullRequest),
    changesRequestedBy: changesRequestedBy(pullRequest),
    checksFailing: pullRequest.checks?.state === "failing",
    changesRequested: pullRequest.reviewDecision === "changesRequested",
  }
}

/**
 * Retargets the open pull request of the branch to the target of Patronus, as a pull request names it ("master"), as
 * every pull request of a virtual branch goes, and says so (Activity too): the step of row 5, the menu, and the Safe
 * Push form of a branch whose pull request merges into a virtual branch
 */
export async function retargetToTarget(branchName: string): Promise<void> {
  const pullRequest = getPullRequest(branchName)
  if (getBranch(branchName) === undefined || pullRequest === undefined) {
    return
  }
  const base = getReadinessTarget()
  const failure = `Cannot retarget pull request #${pullRequest.number}`
  // an operation of the pull request: a second click joins it, the slot and the menu say it runs
  await runOperation(
    {
      id: "retarget-pull-request",
      subject: `pr:${pullRequest.number}`,
      objects: [branchName],
      kind: "remote",
      label: `Retargeting #${pullRequest.number} to ${base}`,
      // the base of the pull request: a review request of it runs beside
      resources: [`pr:${pullRequest.number}:base`],
    },
    () =>
      retargetPullRequest(branchName, base).then(
        () => {
          const title = `Retargeted #${pullRequest.number} to ${base}`
          const description = `${branchName} · its approvals count for a Safe Push now`
          recordRetarget(title, description, pullRequest.url)
          showToast({ title, description, action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } })
        },
        (error: unknown) => notifyError(failure, error),
      ),
    reason => showToast({ type: "info", title: failure, description: reason }),
  )
}

/**
 * How a start reads the next step (`readSafePush`): the review only as Patronus answered it for the branch as it is
 * (a stale answer is not asked yet: the start waits for the new one), and its own running start is no reason against it
 */
export interface StartStepOptions {
  readonly forStart: true
}

/** The open pull request of the whole stack that holds the branch (`getStackBranchOf`); null without one */
function stackPullRequestFactOf(name: string): StackPullRequestFact | null {
  const stack = getStackBranchOf(name)
  const pullRequest = stack === undefined ? undefined : getStackPullRequest(stack.root)
  if (stack === undefined || pullRequest?.state !== "open") {
    return null
  }
  return { number: pullRequest.number, subject: formatWholeStack(stack.root), open: () => openPullRequest(getStackPullRequest(stack.root) ?? pullRequest) }
}

/**
 * The operation on the row for the next step (`NextStepFacts.busy`): an operation of the registry, or the running start
 * of a Safe Push of the row, which is no reason against the start it runs itself (`options`)
 */
export function busyFact(key: string, options: StartStepOptions | null): NextStepBusy | null {
  const operation = getObjectBusy(key)
  if (operation !== undefined) {
    return { id: operation.id, label: operationLabel(operation), ...(operation.resources === undefined ? null : { resources: operation.resources }) }
  }
  const starting = options === null ? getSafePushStarting(key) : null
  return starting == null ? null : { id: "", label: `Starting the ${formatPushMode(starting.mode)}` }
}

/**
 * The first fact of the remote the step of a branch (of a whole stack: `robot` of its stack branch, no `branchName`:
 * the gate of its start checks what it needs) waits for (`NextStepFacts.pending`): Patronus, the pull requests, what its
 * Safe Push needs, why its robot failed; null while each is known
 */
export function pendingFactOf(robot: PatronusRobot | undefined, branchName: string | null): NextStepFacts["pending"] {
  if (isPatronusPending()) {
    return "patronus"
  }
  if (isPullRequestsLoading()) {
    return "pullRequests"
  }
  if (branchName !== null && isLandingNeedsPending(branchName)) {
    return "landingNeeds"
  }
  return robot !== undefined && isRobotProblemsPending(robot) ? "robotProblems" : null
}

/**
 * The first fact the step decides on whose read failed (`NextStepFacts.unavailable`): Patronus answered with a failure
 * (the Safe Push CLI not logged in: Copy Command of its login; an error: Refresh), or didn't give why the robot failed
 */
export function unavailableFactOf(robot: PatronusRobot | undefined): UnavailableFact | null {
  const availability = getPatronusAvailability()
  // an error after a list keeps the robots it listed, as a failed refresh of GitHub keeps the last listing; a token
  // Patronus refuses is gone
  if (availability?.status === "notLoggedIn" || (availability?.status === "error" && !hasPatronusListed())) {
    const reason = formatPatronusAvailabilityHint(availability) ?? "Patronus doesn't answer"
    const repositoryPath = getSelectedProject()?.path
    const action: MenuItem =
      availability.status === "notLoggedIn" && repositoryPath !== undefined
        ? {
            id: "copy-safe-push-login",
            label: "Copy Login Command",
            icon: ClipboardCopyIcon,
            description: `${reason}: the command goes to the clipboard, for a terminal`,
            onSelect: () => void copyText(formatSafePushLoginCommand(repositoryPath, isWindows)),
          }
        : refreshPatronusItem(reason)
    return { fact: "patronus", reason, action }
  }
  // no view stands for what the robots hold, land and need: no start on "none"
  const viewFailure = getPatronusViewFailure()
  if (viewFailure !== null) {
    const reason = formatViewFailure(viewFailure)
    return { fact: "patronus", reason, action: refreshPatronusItem(reason) }
  }
  const failure = robot === undefined ? null : getRobotProblemsFailure(robot)
  return failure === null ? null : { fact: "robotProblems", reason: formatProblemsFailure(failure), action: null }
}

function refreshPatronusItem(reason: string): MenuItem {
  return { id: "refresh-patronus", label: "Refresh Patronus", icon: RefreshCwIcon, description: reason, onSelect: () => void refreshPatronus() }
}

/** Why no start is decided while Patronus didn't give the view of its robots, and no earlier one stands for it */
export function formatViewFailure(failure: string): string {
  return `Patronus didn't tell what its robots hold and land (${failure})`
}

/**
 * Why the restart and the prompts of the failed `robot` wait: why it failed is still to come, or Patronus didn't give
 * it; null for a robot that didn't fail, whose problems came, or that is of no branch of the table
 */
export function robotProblemsReason(robot: PatronusRobot, branch: Branch | null): string | null {
  if (robot.status !== "failure" || (branch === null && !isTableRobot(robot))) {
    return null
  }
  if (isRobotProblemsPending(robot)) {
    return formatPendingFact("robotProblems")
  }
  const failure = getRobotProblemsFailure(robot)
  return failure === null ? null : formatProblemsFailure(failure)
}

/** Why a restart of a failed robot is not offered while Patronus didn't give why it failed */
function formatProblemsFailure(failure: string): string {
  return `Patronus didn't tell why the Safe Push failed (${failure}): Check Approvals Again asks again`
}

/** What decides the next step of the branch now, from the stores; the node of its row reads them too (`useLandingNode`) */
export function getNextStepFacts(branch: Branch, options: StartStepOptions | null = null): NextStepFacts {
  const name = branch.name
  const available = isPatronusAvailable()
  const robot = getBranchRobot(name)
  const needsReview = robot !== undefined && isRobotNeedingReview(robot.id)
  const failing = robot !== undefined && isRobotFailing(robot.id)
  const localTip = branch.commits[0]?.hash
  const landedBy = getLandedBy(name)
  const slot = getSafePushSlot({
    robot,
    localTip,
    remote: branch.remoteStatus,
    needsReview,
    failing,
    canOffer: available && !branch.hasError && branch.commitCount > 0,
    landed: isLandedByOwnRobot(landedBy, robot),
  })
  const landingAncestor = getLandingAncestor(name)
  // why its starts can't, as Rust decides it of the view (`domain_policy::land::start_refusal`)
  const landing = getBranchLanding(name)
  const target = getReadinessTarget()
  const pullRequest = getPullRequest(name)
  const restart =
    robot === undefined ? null : (getRobotActions(robot, branch, needsReview, failing).find(it => it.id === "restart-robot" || it.id === "push-and-restart-robot") ?? null)
  const readiness = options === null ? getReviewReadiness(name) : (freshReadiness(getReviewReadiness(name), name) ?? undefined)
  // what landed it, as Rust decides it of the landing report, the robots and the pull requests (`landedStates`)
  const landed = getLandedState(name) ?? null
  return {
    branchName: name,
    subject: name,
    patronusAvailable: available,
    syncing: branch.status === "Syncing",
    hasError: branch.hasError,
    errorReason: stackProblemOf(branch)?.reason ?? null,
    commitCount: branch.commitCount,
    busy: busyFact(name, options),
    blocked: branch.status === "Blocked",
    startBlocker: formatReasonOrNull(landing.start),
    dryRunStartBlocker: formatReasonOrNull(landing.dryRun),
    landingPart: landingAncestor,
    held: getPushHold(name) !== null,
    parked: getParkedState(name),
    landed,
    robot: slot,
    robotPullRequest: robot?.pullRequest?.number ?? null,
    restart,
    // a refresh that failed for a while keeps the last listing: its pull requests are known as of then
    pullRequestsAvailable: getPullRequestListing() === "listed",
    pullRequestsUnlisted: getPullRequestListing() === "unlisted" ? formatAvailabilityHint(getPullRequestAvailability()) : null,
    pullRequest: pullRequestFactOf(branch),
    target,
    // the ancestors a merged Safe Push landed are in the target: the robot lands the others
    landsBranches: getLandingBranchCount(name),
    startDescription: null,
    landingNeeds: getLandingNeeds(name),
    readiness: readiness === undefined ? null : { readiness: readiness.readiness, stale: readiness.stale },
    pending: pendingFactOf(robot, name),
    unavailable: unavailableFactOf(robot),
    forStart: options !== null,
    stackPullRequest: stackPullRequestFactOf(name),
    actions: {
      openSafePush: () => void requestSafePush(name, { mode: "rebaseAutosquash", via: "click" }),
      openPullRequest: () => {
        const current = getPullRequest(name) ?? pullRequest
        if (current !== undefined) {
          openPullRequest(current)
        }
      },
      retarget: () => void retargetToTarget(name),
      // nothing of a branch that landed is left to review: the step decides it, so the item reads its facts here
      createPullRequest: getCreatePullRequestItem(branch, landedReasonOf({ landed, robot: slot })),
      requestReview: getRequestReviewItem(getOpenPullRequest(name), () => openRequestReview(name)),
      fixLanding,
    },
  }
}

/** Runs the fix of what a Safe Push needs (`LandingFix`): the restack of the root of what it lands onto the branch it needs */
export function fixLanding(fix: LandingFix): void {
  if (fix.kind === "restack") {
    void restackBranch(fix.branch, fix.onto)
  }
}

/**
 * The next step of the branch now (`getBranchNextStep`), for menus built on open, keys, toasts and the rows; null without
 * the branch. Computed once per change of the stores it reads (`nextStepSources`): six subscriptions of a row, the readiness
 * scan and the focus ask it, they share the answer.
 */
export function getBranchNextStepNow(name: string): BranchNextStep | null {
  const sources = nextStepSources()
  const shown = shownSteps.get(name)
  if (shown !== undefined && shown.sources.every((source, index) => source === sources[index])) {
    return shown.step
  }
  const branch = getBranch(name)
  const step = branch === undefined ? null : getBranchNextStepOf(branch)
  const key = stepKey(step, name)
  // the same object while the step stays: a row renders again only when it changes
  const kept = shown?.key === key ? shown.step : step
  shownSteps.set(name, { sources, key, step: kept })
  return kept
}

/** The states of every store the next step reads: a change of any is a new step (of a branch, of a whole stack) */
export function nextStepSources(): readonly unknown[] {
  return [
    branchSyncStore.getState(),
    patronusStore.getState(),
    pullRequestsStore.getState(),
    reviewReadinessStore.getState(),
    operationsStore.getState(),
    safePushStore.getState(),
    repositorySettingsStore.getState(),
  ]
}

/** The next step of `branch` as it is given (a menu of the branch it shows), the rest from the stores; `options` for a start */
export function getBranchNextStepOf(branch: Branch, options: StartStepOptions | null = null): BranchNextStep {
  return getBranchNextStep(getNextStepFacts(branch, options))
}

/** What a surface renders of a step: one object while it stays, so a row renders again only when its step changes */
function stepKey(step: BranchNextStep | null, name: string): string {
  if (step === null) {
    return ""
  }
  const item = step.item
  const pullRequest = getPullRequest(name)
  return JSON.stringify([
    step.kind,
    step.tone,
    step.safePushBlocker,
    step.dryRunBlocker,
    step.needsReadiness,
    step.landed,
    item?.id,
    item?.label,
    item?.disabled,
    item?.description,
    getBranchRobot(name)?.id,
    pullRequest?.number,
    pullRequest?.url,
  ])
}

/** By branch: the step shown, its key, and the store states it was computed for */
const shownSteps = new Map<string, { sources: readonly unknown[]; key: string; step: BranchNextStep | null }>()

/**
 * The next step of the branch, re-rendering when it changes: it depends on the branches, the robots, the pull
 * requests (what landed it is of the branch store), the readiness and the operations on the row, so each of their stores is watched.
 * The component shows the step: the readiness store asks for the readiness of the branch while it does
 * (`useShowReviewReadiness`), never for a row a collapse hides; of the root of a stack that forks, the readiness of its
 * whole stack too (`stackReadinessKey`), whose Safe Push its menu and its popover offer, with or without an island.
 */
export function useBranchNextStep(name: string): BranchNextStep | null {
  useShowReviewReadiness(name)
  const headsWholeStack = useBranchSync(state => (state.stackBranches[name]?.members.length ?? 0) > 0)
  useShowReviewReadiness(headsWholeStack ? stackReadinessKey(name) : "")
  return useNextStepStores(() => getBranchNextStepNow(name))
}

/** Renders again with `select` when a store the next step reads changes (of a branch, of a whole stack, of a node) */
export function useNextStepStores<T>(select: () => T): T {
  useStore(branchSyncStore, select)
  useStore(patronusStore, select)
  useStore(pullRequestsStore, select)
  useStore(reviewReadinessStore, select)
  useStore(safePushStore, select)
  return useStore(operationsStore, select)
}

/** Whether a row waits for an approval (Awaiting): a focus asks GitHub for the pull requests at once then */
export function isAwaitingApproval(name: string): boolean {
  return getBranchNextStepNow(name)?.kind === "awaiting"
}

/**
 * Tells that the approvals of a branch arrived (the news of the session, `approvalNewsOf`): a toast
 * "Ilya Korennoy approved auth" / "#1078 · IJPL Code Platform" with "Safe Push" first (the start of the row, the row
 * revealed; decided again at the click: a reason that came since is an `info` toast) and "Open Pull Request"; in the
 * background the OS notification of "Approvals of my pull requests" and the toast on the next focus; an Activity entry
 */
export function tellApprovalNews(news: ApprovalNews): void {
  announceApproval(news, getPullRequest(news.branchName), {
    label: "Safe Push",
    onClick: () => void requestSafePush(news.branchName, { mode: "rebaseAutosquash", via: "click" }),
  })
}

/**
 * The toast, the OS notification and the Activity entry of approvals that arrived in `pullRequest` (`describeApprovalNews`),
 * the start of what they count for first, then "Open Pull Request": of a branch, of the whole stack (`tellStackApprovalNews`)
 */
export function announceApproval(news: ApprovalNews, pullRequest: PullRequest | undefined, start: { readonly label: string; readonly onClick: () => void }): void {
  const repositoryPath = getSelectedProject()?.path
  if (repositoryPath === undefined || pullRequest === undefined) {
    return
  }
  const { title, description } = describeApprovalNews(news)
  recordApproval(repositoryPath, title, description, pullRequest.url)
  // the toast of the approvals of the subject: its start takes its place (`robotToastKeys`), from the toast or any other
  // surface; the start only while the main window shows the repository, whose branch it starts
  const key = toastKeys.approval(repositoryPath, news.branchName)
  const openPullRequestAction = { label: "Open Pull Request", onClick: () => openPullRequest(pullRequest) }
  announce("approvals", { title, body: description }, () =>
    showEventToast({
      key,
      watch: [repositoryStore],
      render: () =>
        isSelectedRepository(repositoryPath)
          ? {
              type: "success",
              title,
              description,
              action: {
                label: start.label,
                onClick: () => {
                  closeEventToasts(key)
                  start.onClick()
                },
              },
              secondaryAction: openPullRequestAction,
            }
          : { type: "success", title, description, action: openPullRequestAction },
    }),
  )
}

/** The actions of a failed robot that read why it failed: a restart, the prompts for an agent */
const UNKNOWN_FAILURE_ACTIONS: ReadonlySet<string> = new Set(["restart-robot", "push-and-restart-robot", "copy-robot-prompt", "copy-robot-full-prompt"])

/** The next steps that wait for approvals: a restart of a robot that failed for them would fail again */
const APPROVAL_STEPS: ReadonlySet<BranchNextStep["kind"]> = new Set(["createPullRequest", "changesRequested", "requestReview", "awaiting"])

/**
 * The actions of a robot (`getRobotActions`) for every surface that shows them (the branch menu, the robot details, the
 * landing overview): its restart is disabled with the reason of the next step while the approvals that count are still
 * missing (a restart of a robot that failed for them fails again)
 */
export function getGatedRobotActions(robot: PatronusRobot, branch: Branch | null, needsReview?: boolean, failing?: boolean, operations?: Operations): RobotAction[] {
  const actions = getRobotActions(robot, branch, needsReview, failing, operations)
  // why the robot of a branch or of a whole stack failed is still to come, or Patronus didn't give it
  // (`loadReviewProblems` reads them for the robots of the table): no restart and no prompt as if it hadn't failed for
  // the approvals
  const problems = robotProblemsReason(robot, branch)
  if (problems !== null) {
    return actions.map(item => (UNKNOWN_FAILURE_ACTIONS.has(item.id) ? { ...item, disabled: true, description: problems } : item))
  }
  if (branch === null) {
    return actions
  }
  const step = getBranchNextStepOf(branch)
  const reason = APPROVAL_STEPS.has(step.kind) ? step.safePushBlocker : null
  return reason === null
    ? actions
    : actions.map(item => (item.id === "restart-robot" || item.id === "push-and-restart-robot" ? { ...item, disabled: true, description: reason } : item))
}
