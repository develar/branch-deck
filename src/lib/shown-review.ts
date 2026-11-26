// The reviews after landing (show of ship/show/ask) the user acts on from the row of a branch: whom Safe Push and
// Request Review… asks by default (`get_show_reviewers`, the code owners of what the branch changes), its mark ahead of
// time (Request Review After Landing…, `Intent::MarkShownReview`), more reviewers asked of an open one (Request Review…,
// `Intent::OpenShownReview` with them) and its close (Close Review…, Cancel Review After Landing…,
// `Intent::CloseShownReview`). Rust plans and runs each (`operations::shown`); the page only asks and toasts.
import type { Intent, ShowReviewers } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { commandFailure, notifyError } from "@/lib/errors"
import { failedEffects, runFailure, runIntent } from "@/model/intents"

/**
 * Whom the review after landing of `branchName` in the repository of `repositoryPath` asks by default: one available
 * code owner of each owner group of what it changes, never the user
 */
export async function readShowReviewers(repositoryPath: string, branchName: string): Promise<ShowReviewers> {
  const answer = await commands.getShowReviewers({ repositoryPath, branchName })
  if (answer.status === "error") {
    throw commandFailure(answer.error)
  }
  return answer.data
}

/**
 * Runs `intent`, an open, an extend or a close of a review after landing, in the session of `repositoryPath`; each
 * effect that failed, or the run as a whole, is a toast `failure`. Resolves to whether every effect went
 */
export async function runShownIntent(repositoryPath: string, intent: Intent, failure: string): Promise<boolean> {
  const run = await runIntent(repositoryPath, intent)
  if (run.kind === "ran") {
    const failed = failedEffects(run.outcome)
    for (const { error } of failed) {
      notifyError(failure, error)
    }
    return failed.length === 0
  }
  if (run.kind !== "noPlan") {
    notifyError(failure, runFailure(run, ""))
  }
  return false
}

/**
 * Request Review After Landing… of `branch`: its review after landing marked ahead of time with `reviewers` (a draft now,
 * which opens once anything lands the branch), of its draft those recorded again, of a branch that landed the review
 * opened now
 */
export function markShownReview(repositoryPath: string, branch: string, reviewers: readonly string[]): Promise<boolean> {
  return runShownIntent(repositoryPath, { kind: "markShownReview", branch, reviewers: [...reviewers] }, `Cannot request the review after landing of ${branch}`)
}

/** Request Review… of the open review after landing of `branch`: `reviewers` are asked and recorded in its body */
export function requestShownReview(repositoryPath: string, branch: string, reviewers: readonly string[]): Promise<boolean> {
  return runShownIntent(repositoryPath, { kind: "openShownReview", branch, reviewers: [...reviewers] }, `Cannot request the review after landing of ${branch}`)
}

/**
 * Close Review… of the review after landing of `branch`, which the user confirmed: closed with its comment and its
 * branches deleted, approved or not (as `brd close-review --allow-unapproved`)
 */
export function closeShownReview(repositoryPath: string, branch: string): Promise<boolean> {
  return runShownIntent(repositoryPath, { kind: "closeShownReview", branch, allowUnapproved: true }, `Cannot close the review after landing of ${branch}`)
}
