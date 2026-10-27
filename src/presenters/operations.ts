// The page's words of an operation of another process of the repository: as its record in the operations registry of
// the repository names it (`OperationHolder`, `operations::registry_file`), the holder of what an operation of the page
// needs, which made it `busy` (`ErrorDetails.busy`, `describeError`); and as the session shows it on its row
// (`RunningOperation` with `origin.otherProcess`, `operations::registry_watch`): "bd: Safe Push of auth".
import type { HolderTool, Intent, OperationHolder, OperationKind, RunningOperation, Subject } from "@/lib/bindings"
import { formatHeldFor } from "@/presenters/lock-wait"

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
  }
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
