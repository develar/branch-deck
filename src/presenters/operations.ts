// The page's words of an operation of the session (`RepositoryView.operations`): its busy state as its surfaces say it
// ("Unstacking oauth", "Waits for Unstacking oauth", whom it waits for while another holder has the repository lock),
// the rows it works on (`operationOnRow`), and of an operation of another process of the repository: as its record in
// the operations registry of the repository names it (`OperationHolder`, `operations::registry_file`), the holder of
// what an operation of the page needs, which made it `busy` (`ErrorDetails.busy`, `describeError`); and as the session
// shows it on its row (`RunningOperation` with `origin.otherProcess`, `operations::registry_watch`): "bd: Safe Push of
// auth".
import type { HolderTool, Intent, OperationHolder, OperationId, OperationKind, RewriteIntent, RunningOperation, Subject } from "@/lib/bindings"
import { formatHeldFor, presentOperationLockWait } from "@/presenters/lock-wait"
import { presentRewrite } from "@/presenters/rewrite"

/** What an operation of `kind` does, in the words of a sentence */
const KIND_WORDS: Record<OperationKind, string> = {
  safe_push: "Safe Push",
  dry_run: "Dry Run",
  push: "push",
  restart_robot: "restart of a robot",
  cancel_robot: "cancel of a robot",
  retarget: "retarget of a pull request",
  create_pull_request: "creation of the pull request",
  request_review: "review request",
  close_pull_request: "close of a pull request",
  close_landed_pull_requests: "close of the pull requests a robot landed",
  submit_review: "review of a pull request",
  set_issue_state: "change of the state of an issue",
  forget_start: "forget of a start",
  rewrite: "rewrite",
  fetch_baseline: "fetch of the baseline",
  update_project: "update of the project",
  dismiss_landed: "dismissal of the landed commits",
  fetch_review: "fetch of a pull request to review",
  pick: "pick of commits",
}

/** Who runs it: `bd`, or another Branch Deck */
function toolName(tool: HolderTool): string {
  return tool === "bd" ? "`bd`" : "Branch Deck"
}

/**
 * Who runs the operation, with the pid that tells two of them apart: "`bd` (pid 4242) has run the Safe Push of auth for
 * 40 s"; of a start whose answer was lost, that it holds until Patronus tells or the start is forgotten
 */
export function presentOperationHolder(holder: OperationHolder): string {
  const what = holder.subject === null ? KIND_WORDS[holder.kind] : `${KIND_WORDS[holder.kind]} of ${holder.subject}`
  if (holder.unknown === true) {
    return `Cannot tell whether the ${what} that ${toolName(holder.tool)} (pid ${holder.pid}) sent started: it holds until Patronus lists its robot, or until Forget This Start`
  }
  return `${toolName(holder.tool)} (pid ${holder.pid}) has run the ${what} for ${formatHeldFor(holder.held_for)}`
}

/** The row of a subject: the branch, the root of a whole stack */
function subjectName(subject: Subject): string {
  return subject.kind === "branch" ? subject.name : subject.root
}

/** What `intent` does as the row of a running operation says it ("Safe Push of auth", "Pushing auth"), and in a sentence ("the Safe Push of auth") */
function runningWords(intent: Intent): readonly [label: string, phrase: string] {
  switch (intent.kind) {
    case "land": {
      const what = `${intent.mode === "dryRun" ? "Dry Run" : "Safe Push"} of ${subjectName(intent.subject)}`
      return [what, `the ${what}`]
    }
    case "push":
      return [`Pushing ${subjectName(intent.subject)}`, `a push of ${subjectName(intent.subject)}`]
    case "restartRobot":
      return [`Restarting robot ${intent.robotId}`, `a restart of robot ${intent.robotId}`]
    case "cancelRobot":
      return [`Canceling robot ${intent.robotId}`, `a cancel of robot ${intent.robotId}`]
    case "retarget":
      return [`Retargeting #${intent.number}`, `a retarget of #${intent.number}`]
    case "createPullRequest":
      return [`Opening the pull request of ${subjectName(intent.subject)}`, `the creation of the pull request of ${subjectName(intent.subject)}`]
    case "requestReview":
      return [`Requesting a review of ${subjectName(intent.subject)}`, `a review request of ${subjectName(intent.subject)}`]
    case "closePullRequest":
      return [`Closing #${intent.number}`, `a close of #${intent.number}`]
    case "closeLandedPullRequests":
      return [`Closing the pull requests robot ${intent.robotId} landed`, `the close of the pull requests robot ${intent.robotId} landed`]
    case "submitReview":
      return [`Reviewing #${intent.number}`, `a review of #${intent.number}`]
    case "setIssueState":
      return [`Setting ${intent.key} to ${intent.state}`, `a change of the state of ${intent.key}`]
    case "forgetStart":
      return [`Forgetting start ${intent.op}`, `the forget of start ${intent.op}`]
    case "rewrite": {
      const { label, phrase } = presentRewrite(intent)
      return [label, phrase]
    }
    case "fetchBaseline":
      return ["Fetching the baseline", "a fetch of the baseline"]
    case "updateProject":
      return ["Updating the project", "an update of the project"]
    case "dismissLanded":
      return ["Dismissing the landed commits", "the dismissal of the landed commits"]
    case "fetchReview":
      return [`Fetching #${intent.number} to review`, `a fetch of #${intent.number} to review`]
    case "pick":
      return ["Picking commits", "a pick of commits"]
  }
}

/** The busy state of an operation of the session as a surface says it: "Unstacking oauth", "Updating the project" */
export function presentRunningLabel(operation: RunningOperation): string {
  return runningWords(operation.intent)[0]
}

/** The operations of the session of the repository the main window shows, by id (`RepositoryView.operations`) */
export type SessionOperations = Readonly<Record<OperationId, RunningOperation>>

/**
 * The busy state of `operation` as every surface that shows it says it: whom it waits for while another holder has the
 * repository lock ("Waiting for `bd pick` (pid 4242, for 12 s)"), what it waits behind in the queue of the session
 * ("Waits for Unstacking oauth"), else its own ("Unstacking tool-events"); `labelOf` the busy state of an operation, as
 * the window that started it named it
 */
export function presentOperationBusy(operation: RunningOperation, operations: SessionOperations, labelOf: (operation: RunningOperation) => string): string {
  const lockWait = presentOperationLockWait(operation)
  if (lockWait !== null) {
    return lockWait
  }
  if (operation.step.kind === "queued") {
    const behind = operations[operation.step.behind]
    return `Waits for ${behind === undefined ? "another operation" : labelOf(behind)}`
  }
  return labelOf(operation)
}

/** What finds the rows of an operation in the table of the page: the branches with the commits it names, the open pull request of a row */
export interface RowLookup {
  readonly branchesOf: (originalIds: readonly string[]) => readonly string[]
  readonly pullRequestOf: (name: string) => number | undefined
}

/**
 * The rows a rewrite works on, the one that shows it first: of a move the branches of its commits, then the branch it
 * goes to; of a squash and a reword the branches of their commits; of the others the branches they name (a fix of a
 * conflict and an undo name none)
 */
function rewriteRows(rewrite: RewriteIntent, lookup: RowLookup): readonly string[] {
  switch (rewrite.rewrite) {
    case "move":
      return [...lookup.branchesOf(rewrite.commitIds), ...(rewrite.targetBranch === null ? [] : [rewrite.targetBranch])]
    case "squash":
      return lookup.branchesOf(rewrite.commitIds)
    case "reword":
      return lookup.branchesOf(rewrite.rewords.map(reword => reword.commit))
    case "park":
      return rewrite.branches.map(line => line.branchName)
    case "unpark":
      return rewrite.branchNames
    case "squashFixups":
      return rewrite.scope.type === "branch" ? [rewrite.scope.value] : []
    case "group":
      // a branch of the plan by its path, `parent/leaf` with stacked branches: its leaf is the row
      return [...Object.keys(rewrite.plan.branches ?? {}).map(path => path.slice(path.lastIndexOf("/") + 1)), ...(rewrite.plan.unstack ?? [])]
    case "moveHunk":
      return lookup.branchesOf([rewrite.from, rewrite.to])
    case "applyConflictFix":
    case "undo":
      return []
    default:
      return [rewrite.branchName]
  }
}

/**
 * Whether `operation` works on the row `name` (its right-click menu, its "⋯", its drag and its push wait meanwhile): a
 * rewrite of the branches it names or whose commits it names, the retarget of the open pull request of the row; with
 * `first`, only on the row that shows its spinner (its "⋯", while no surface that started it shows it): the first of
 * those
 */
export function worksOnRow(operation: RunningOperation, name: string, lookup: RowLookup, first = false): boolean {
  const { intent } = operation
  if (intent.kind === "retarget") {
    return !first && intent.number === lookup.pullRequestOf(name)
  }
  if (intent.kind !== "rewrite") {
    return false
  }
  const rows = rewriteRows(intent, lookup)
  return first ? rows[0] === name : rows.includes(name)
}

/** The first operation of `operations` that works on the row `name` (`worksOnRow`); undefined while none does */
export function operationOnRow(operations: SessionOperations, name: string, lookup: RowLookup, first = false): RunningOperation | undefined {
  return Object.values(operations).find(operation => worksOnRow(operation, name, lookup, first))
}

/** An operation of another process as its row says it (`presentOtherProcess`) */
export interface OtherProcessRun {
  /** The label of its slot and the reason a menu item waits: "bd: Safe Push of auth" (a running surface adds "…"); "Start Unknown" of a start whose answer was lost */
  readonly label: string
  /** The hover and the refusal of a start it holds back: "`bd` (pid 4242) runs the Safe Push of auth, which holds what it needs until it ends" */
  readonly description: string
  /** A start whose answer was lost (`startUnknown`): it runs no more, and holds until Patronus tells or it is forgotten */
  readonly unknown: boolean
}

/**
 * An operation of another process of the repository that the session shows (an agent's `bd safe-push auth`), in the
 * words of its row; null for an operation of this app
 */
export function presentOtherProcess(operation: RunningOperation): OtherProcessRun | null {
  if (operation.origin.kind !== "otherProcess") {
    return null
  }
  const [label, phrase] = runningWords(operation.intent)
  if (operation.step.kind === "startUnknown") {
    return {
      label: "Start Unknown",
      description: `Cannot tell whether ${phrase} started: its answer was lost. It holds until Patronus lists its robot, or until Forget This Start; Patronus may still run it`,
      unknown: true,
    }
  }
  const tool = operation.origin.tool === "bd" ? "bd" : "Branch Deck"
  return {
    label: `${tool}: ${label}`,
    description: `${toolName(operation.origin.tool)} (pid ${operation.origin.pid}) runs ${phrase}, which holds what it needs until it ends`,
    unknown: false,
  }
}
