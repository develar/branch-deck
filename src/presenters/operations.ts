// The page's words of an operation of another process (`OperationHolder`, as its record in the operations registry of
// the repository names it, `operations::registry_file`): who runs what an operation of the page needs, which made it
// `busy` (`ErrorDetails.busy`, `describeError`).
import type { OperationHolder, OperationKind } from "@/lib/bindings"
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
}

/** Who runs the operation, with the pid that tells two of them apart: "`bd` (pid 4242) has run the Safe Push of auth for 40 s" */
export function presentOperationHolder(holder: OperationHolder): string {
  const tool = holder.tool === "bd" ? "`bd`" : "Branch Deck"
  const what = holder.subject === null ? KIND_WORDS[holder.kind] : `${KIND_WORDS[holder.kind]} of ${holder.subject}`
  return `${tool} (pid ${holder.pid}) has run the ${what} for ${formatHeldFor(holder.held_for)}`
}
