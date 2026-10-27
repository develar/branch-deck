import { GitPullRequestCreateIcon } from "lucide-react"
import { useStore } from "zustand"
import { requestSafePush } from "@/hooks/useSafePush"
import { restackBranch } from "@/hooks/useStackActions"
import type { ApprovalNews } from "@/lib/approval-news"
import { describeApprovalNews } from "@/lib/approval-news"
import type { ActionState, LandingFix, NextStep, NextStepView, PatronusRobot, PullRequest, PullRequestView } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { notifyError } from "@/lib/errors"
import { closeEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import { announce } from "@/lib/os-notification"
import { formatPushMode, formatSafePushLoginCommand } from "@/lib/patronus"
import { isWindows } from "@/lib/platform"
import { openPullRequest } from "@/lib/pull-request"
import { baselineOnRemote } from "@/lib/stack"
import { showToast } from "@/lib/toast-message"
import { stackReadinessKey } from "@/lib/whole-stack"
import { useShowReviewReadiness } from "@/model/demand"
import { getRowPart, useRowPart } from "@/model/store"
import type { RobotAction, SessionOperations } from "@/presenters/actions"
import { getRobotActionItems, isRestart } from "@/presenters/actions"
import type { BranchNextStep, NextStepActions, NextStepBusy, StepContext } from "@/presenters/next-step"
import { formatPendingFact, formatProblemsFailure, presentNextStep } from "@/presenters/next-step"
import { CREATE_PULL_REQUEST_LABEL } from "@/presenters/pull-request-url"
import { getRequestReviewItem } from "@/presenters/review"
import { recordApproval } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, useBranchSyncEntry, useBranchSyncField } from "@/stores/branch-sync"
import { openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import type { Operation } from "@/stores/operations"
import { getObjectBusy, operationLabel, runOperation, useObjectOperation } from "@/stores/operations"
import { getRobot, getRobotProblemsFailure, isRobotProblemsPending, isTableRobot, patronusStore, refreshPatronus } from "@/stores/patronus"
import { getPullRequest, getStackPullRequest, retargetPullRequest } from "@/stores/pull-requests"
import { requestPushConfirm } from "@/stores/push-confirm"
import { getSelectedProject, isSelectedRepository, repositoryStore, useSelectedProjectPath } from "@/stores/repository"
import { getReadinessTarget } from "@/stores/review-readiness"
import type { LandRun } from "@/stores/safe-push"
import { getLandRun, useLandRun } from "@/stores/safe-push"

/**
 * Retargets the open pull request of the branch to the target of Patronus, as a pull request names it ("master"), as
 * every pull request of a virtual branch goes (`retargetPullRequest`: the engine runs it, Activity records it), and says
 * so: the step of row 5, the menu, and the Safe Push form of a branch whose pull request merges into a virtual branch
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
      retargetPullRequest(branchName).then(
        () => {
          const title = `Retargeted #${pullRequest.number} to ${base}`
          const description = `${branchName} · its approvals count for a Safe Push now`
          showToast({ title, description, action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } })
        },
        (error: unknown) => notifyError(failure, error),
      ),
    reason => showToast({ type: "info", title: failure, description: reason }),
  )
}

/** Runs the fix of what a Safe Push needs (`LandingFix`): the restack of the root of what it lands onto the branch it needs */
export function fixLanding(fix: LandingFix): void {
  if (fix.kind === "restack") {
    void restackBranch(fix.branch, fix.onto)
  }
}

/**
 * What the words and the items of the step of a subject read of the stores beside the step itself (`nextSteps` of the
 * view): each a kept reference or a primitive, so a surface renders again only when one of them changes. The parts
 * still the page's: what runs on the row (an operation, a start: s10), the restart of its robot and the Request
 * Review… of its pull request (their items, until the actions are Rust's)
 */
export interface StepInputs {
  /** The branch Patronus pushes into ("master") */
  readonly target: string
  /** The operation that works on the row */
  readonly operation: Operation | undefined
  /** The start of a Safe Push or a Dry Run of the row that runs, or the click that waits for its plan */
  readonly starting: LandRun | null
  /** The robot the step restarts; undefined while the step restarts none */
  readonly robot: PatronusRobot | undefined
  /** The actions Rust offers of that robot (`robot:<id>`), its restart among them */
  readonly robotActions: readonly ActionState[] | undefined
  /** The operations of the session, while the step restarts a robot (its item says a running restart) */
  readonly running: SessionOperations | null
  /** The open pull request of the subject, while the step asks for a review of it */
  readonly pullRequest: PullRequestView | undefined
  readonly repositoryPath: string | null
}

/** The robot the step of a subject restarts (rows 2 and 3, or a restart disabled while Patronus didn't tell why it failed) */
function restartedRobotOf(step: NextStep | undefined): string | null {
  const kind = step?.kind
  return kind?.kind === "restart" || kind?.kind === "unavailable" ? kind.robotId : null
}

/** The step of a subject asks for a review of its open pull request (row 9) */
function asksReview(step: NextStep | undefined): boolean {
  return step?.kind.kind === "requestReview"
}

/** `StepInputs` of the subject `key` (a branch, `stack:<root>`), re-rendering when one changes */
export function useStepInputs(key: string, step: NextStep | undefined): StepInputs {
  const robotId = restartedRobotOf(step)
  const review = asksReview(step)
  return {
    target: useBranchSyncField("baselineBranch", baseline => baselineOnRemote(baseline ?? "")),
    operation: useObjectOperation(key),
    starting: useLandRun(key),
    robot: useStore(patronusStore, () => (robotId === null ? undefined : getRobot(robotId))),
    robotActions: useRowPart(robotId === null ? "" : `robot:${robotId}`, "actions"),
    running: useBranchSyncField("operations", operations => (robotId === null ? null : operations)),
    pullRequest: useBranchSyncEntry("openPullRequests", key, pullRequest => (review ? pullRequest : undefined)),
    repositoryPath: useSelectedProjectPath() ?? null,
  }
}

/** `StepInputs` of the subject `key` now, for a menu built on open, a key, a toast, a start */
export function getStepInputs(key: string, step: NextStep | undefined): StepInputs {
  const robotId = restartedRobotOf(step)
  return {
    target: getReadinessTarget(),
    operation: getObjectBusy(key),
    starting: getLandRun(key),
    robot: robotId === null ? undefined : getRobot(robotId),
    robotActions: robotId === null ? undefined : getRowPart(`robot:${robotId}`, "actions"),
    running: robotId === null ? null : branchSyncStore.getState().operations,
    pullRequest: asksReview(step) ? branchSyncStore.getState().openPullRequests[key] : undefined,
    repositoryPath: getSelectedProject()?.path ?? null,
  }
}

/**
 * What runs on the row as the step says it: an operation of the registry, or the running start of a Safe Push of the
 * row, which is no reason against the start it runs itself (`forStart`), unless another process runs it ("bd: Safe Push
 * of auth")
 */
export function busyOf(inputs: StepInputs, forStart: boolean): NextStepBusy | null {
  const { operation } = inputs
  if (operation !== undefined) {
    return { id: operation.id, label: operationLabel(operation), ...(operation.resources === undefined ? null : { resources: operation.resources }) }
  }
  // a start another process runs holds every start of the row, the one asked too
  const starting = forStart && inputs.starting?.by == null ? null : inputs.starting
  return starting === null ? null : { id: "", label: starting.by?.label ?? `Starting the ${formatPushMode(starting.mode)}` }
}

/**
 * The actions the items of a step run (`NextStepActions`), of the subject `key` (a branch, `stack:<root>`: the one
 * `openSafePush`, `openPullRequest` and the rest act on): the orchestrators of the page, each reading the stores at the
 * click, and the items still built here of `inputs`
 */
export function stepActionsOf(key: string, inputs: StepInputs, items: Pick<NextStepActions, "openPullRequest" | "retarget" | "createPullRequest">): NextStepActions {
  const { robot, running, repositoryPath } = inputs
  return {
    ...items,
    openSafePush: () => void requestSafePush(key, { mode: "rebaseAutosquash", via: "click" }),
    openStackPullRequest: root => {
      const pullRequest = getStackPullRequest(root)
      if (pullRequest !== undefined) {
        openPullRequest(pullRequest)
      }
    },
    fixLanding,
    copyLoginCommand: repositoryPath === null ? null : () => void copyText(formatSafePushLoginCommand(repositoryPath, isWindows)),
    refreshPatronus: () => void refreshPatronus(),
    pickRemoteCommits: () => requestPushConfirm(key, "row"),
    restart: robot === undefined || running === null ? null : (getRobotActionItems(robot, { offered: inputs.robotActions, running }).find(isRestart) ?? null),
    requestReview: inputs.pullRequest === undefined ? null : getRequestReviewItem(key),
  }
}

/** The context of the step of the branch `name` over `inputs` (`StepContext`); `forStart` for a start */
function branchContextOf(name: string, inputs: StepInputs, forStart: boolean): StepContext {
  return {
    subject: name,
    target: inputs.target,
    startDescription: null,
    busy: busyOf(inputs, forStart),
    forStart,
    actions: stepActionsOf(name, inputs, {
      openPullRequest: () => {
        const pullRequest = getPullRequest(name)
        if (pullRequest !== undefined) {
          openPullRequest(pullRequest)
        }
      },
      retarget: () => void retargetToTarget(name),
      // the pull request that collects the approvals; nothing of a branch that landed is left to review (row 1)
      createPullRequest: { id: "create-pull-request", label: CREATE_PULL_REQUEST_LABEL, icon: GitPullRequestCreateIcon, onSelect: () => openCreatePullRequestDialog(name) },
    }),
  }
}

/**
 * The next step of the branch `name` now, as Rust decides it (`nextSteps` of the view) in the page's words
 * (`presentNextStep`), for a menu built on open, a key, a toast; `forStart` as a start reads it (`readSafePush`); null
 * before the view has it
 */
export function getBranchNextStep(name: string, options?: { readonly forStart: true }): BranchNextStep | null {
  const view = getRowPart(name, "landing")
  return view === undefined ? null : presentNextStep(view.step, branchContextOf(name, getStepInputs(name, view.step), options !== undefined))
}

/**
 * The next step of the branch, re-rendering when it changes: the step of the view (one kept reference), and what its
 * words read beside it (`useStepInputs`). The component shows the step: the session asks for the readiness of the
 * branch while it does (`useShowReviewReadiness`), never for a row a collapse hides; of the root of a stack that forks,
 * the readiness of its whole stack too (`stackReadinessKey`), whose Safe Push its menu and its popover offer
 */
export function useBranchNextStep(name: string): BranchNextStep | null {
  useShowReviewReadiness(name)
  const headsWholeStack = useBranchSyncEntry("stackBranches", name, stackBranch => (stackBranch?.members.length ?? 0) > 0)
  useShowReviewReadiness(headsWholeStack ? stackReadinessKey(name) : "")
  const view = useRowPart(name, "landing")
  const inputs = useStepInputs(name, view?.step)
  return view === undefined ? null : presentNextStep(view.step, branchContextOf(name, inputs, false))
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

/** The parts of the view the actions of a robot read: its actions, and the step of its branch that words a restart it holds back */
export interface RobotParts {
  readonly actions: readonly ActionState[] | undefined
  readonly landing: NextStepView | undefined
}

/** `RobotParts` of `robot` now */
function robotPartsOf(robot: PatronusRobot): RobotParts {
  return { actions: getRowPart(`robot:${robot.id}`, "actions"), landing: robot.branchName === null ? undefined : getRowPart(robot.branchName, "landing") }
}

/** `RobotParts` of `robot`, re-rendering when one changes; none without a robot */
export function useRobotParts(robot: PatronusRobot | null | undefined): RobotParts {
  return { actions: useRowPart(robot == null ? "" : `robot:${robot.id}`, "actions"), landing: useRowPart(robot?.branchName ?? "", "landing") }
}

/**
 * The actions of a robot as Rust offers them (`getRobotActionItems`) for every surface that shows them (the branch menu,
 * the robot details, the robots indicator, the hold of a push): a restart the approvals its branch misses hold back
 * says so in the words of the step of the branch. A component passes the parts and the operations of the session it
 * subscribed to (`useRobotParts`, `useSessionOperations`), so its list follows them.
 */
export function getRobotItems(robot: PatronusRobot, running: SessionOperations = branchSyncStore.getState().operations, parts: RobotParts = robotPartsOf(robot)): RobotAction[] {
  const step = robot.branchName === null || parts.landing === undefined ? null : getBranchNextStep(robot.branchName)
  return getRobotActionItems(robot, { stepReason: step?.safePushBlocker ?? null, offered: parts.actions, running })
}

/** The operations of the session of the repository the main window shows, re-rendering when they change */
export function useSessionOperations(): SessionOperations {
  return useBranchSyncField("operations", operations => operations)
}
