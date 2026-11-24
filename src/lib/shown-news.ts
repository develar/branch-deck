// The reviews after landing (show of ship/show/ask) the session of a repository tells to act on: a draft whose Safe Push
// merged opens, one a fix of its branch landed after it opened takes the fix, one every reviewer it asked approved closes
// (`domain_policy::shown::shown_news`). Rust decides which and plans each (`operations::shown`), and tells a run that
// failed again at a later answer; the page only asks it.
import type { Intent, News } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { failedEffects, runFailure, runIntent } from "@/model/intents"

/** Runs `intent` in the session of `repositoryPath`; each effect that failed, or the run as a whole, is a toast `failure` */
async function runShown(repositoryPath: string, intent: Intent, failure: string): Promise<void> {
  const run = await runIntent(repositoryPath, intent)
  if (run.kind === "ran") {
    for (const { error } of failedEffects(run.outcome)) {
      notifyError(failure, error)
    }
  } else if (run.kind !== "noPlan") {
    notifyError(failure, runFailure(run, ""))
  }
}

/**
 * The news of the session of `repositoryPath` about its reviews after landing: the review of a branch whose Safe Push
 * merged opens (its head on what landed, ready for review, its reviewers asked), one whose fix landed takes it on its
 * head and asks its reviewers again, one every reviewer approved closes with its branches; Activity says each
 */
export function tellShownNews(repositoryPath: string, news: readonly News[]): void {
  for (const { event } of news) {
    if (event.kind === "shownReviewLanded") {
      void runShown(repositoryPath, { kind: "openShownReview", branch: event.branch }, `Cannot open the review after landing of ${event.branch}`)
    } else if (event.kind === "shownReviewFixLanded") {
      void runShown(repositoryPath, { kind: "extendShownReview", branch: event.branch }, `Cannot add the fix to the review after landing of ${event.branch}`)
    } else if (event.kind === "shownReviewApproved") {
      void runShown(repositoryPath, { kind: "closeShownReview", branch: event.branch, allowUnapproved: false }, `Cannot close the review after landing of ${event.branch}`)
    }
  }
}
