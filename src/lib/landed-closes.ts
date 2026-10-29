// The closes of the pull requests a landing landed all of, which the session of its repository plans (which pull
// requests, the comment of each: `CloseLandedPullRequests`, `domain_policy::landed::pull_requests_to_close`). It decides
// nothing.
import type { LandingKey, News } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { failedEffects, runFailure, runIntent } from "@/model/intents"

/**
 * Closes the pull requests `landing` landed all of in the session of `repositoryPath`, its repository; each close that
 * failed is an error toast "Cannot close pull request #5", a run that failed as a whole `failure`. A session that no
 * longer knows the landing closes nothing
 */
export async function closeLanded(repositoryPath: string, landing: LandingKey, failure: string): Promise<void> {
  const run = await runIntent(repositoryPath, { kind: "closeLandedPullRequests", landing })
  if (run.kind === "ran") {
    for (const { effect, error } of failedEffects(run.outcome)) {
      notifyError(effect.kind === "closePullRequest" ? `Cannot close pull request #${effect.number}` : failure, error)
    }
  } else if (run.kind !== "noPlan") {
    notifyError(failure, runFailure(run, ""))
  }
}

/**
 * The news of the session of `repositoryPath` the window has not told of the landings without a robot (a push into the
 * target, the fetched baseline, a merged pull request): the open pull requests each landed all of close at once, with
 * the comment Rust words; Activity says each close. The merges of robots close theirs with their toast (`announceMerge`)
 */
export function tellLandedNews(repositoryPath: string, news: readonly News[]): void {
  for (const { event } of news) {
    if (event.kind === "landed" && event.closes.length > 0) {
      void closeLanded(repositoryPath, { by: "told", id: event.landing }, "Cannot close the pull requests that landed")
    }
  }
}
