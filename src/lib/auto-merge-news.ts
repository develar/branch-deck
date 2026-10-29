// The auto-merge of a pull request of the user (spec/branch/landing-routes.spec.md#merge), one toast of it: its enable
// (the start's), Cancel Auto-Merge, and the news of an auto-merge GitHub completed (`NewsEvent::AutoMerged`), "Merged #7
// into master" with Update Project, which no update ran by itself. It decides nothing.
import type { News } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showEventToast, toastKeys } from "@/lib/event-toast"
import { formatReadinessSubject } from "@/lib/whole-stack"
import { failedEffects, runFailure, runIntent } from "@/model/intents"
import { runUpdateProjectOf } from "@/stores/update-project"

/** The pull requests GitHub merged by their auto-merge, which the session told: their number, subject and target */
export function autoMergedNewsOf(news: readonly News[]): { readonly number: number; readonly subject: string; readonly into: string }[] {
  return news.flatMap(({ event }) => (event.kind === "autoMerged" ? [{ number: event.number, subject: event.subject, into: event.into }] : []))
}

/** "Merged #7 into master", "auth landed with it: Update Project brings master onto it", Update Project */
export function tellAutoMergedNews(repositoryPath: string, news: readonly News[]): void {
  for (const merged of autoMergedNewsOf(news)) {
    showEventToast({
      key: toastKeys.autoMerge(repositoryPath, merged.number),
      render: () => ({
        type: "success",
        title: `Merged #${merged.number} into ${merged.into}`,
        description: `GitHub merged ${formatReadinessSubject(merged.subject)} once its checks passed: Update Project drops what landed from the branch`,
        action: { label: "Update Project", onClick: () => void runUpdateProjectOf(repositoryPath) },
      }),
    })
  }
}

/**
 * Cancel Auto-Merge of the open pull request `number` of `subject` (`Intent::DisableAutoMerge`): GitHub merges it no
 * more by itself; the toast of its auto-merge says it in place, a failure "Cannot cancel the auto-merge of #7"
 */
export async function cancelAutoMerge(repositoryPath: string, number: number, subject: string): Promise<void> {
  const run = await runIntent(repositoryPath, { kind: "disableAutoMerge", number })
  const failed = run.kind === "ran" ? failedEffects(run.outcome)[0]?.error : run.kind === "noPlan" ? null : runFailure(run, "")
  if (failed != null) {
    notifyError(`Cannot cancel the auto-merge of #${number}`, failed)
    return
  }
  showEventToast({
    key: toastKeys.autoMerge(repositoryPath, number),
    render: () => ({ type: "info", title: `Canceled the auto-merge of #${number}`, description: `GitHub merges ${formatReadinessSubject(subject)} no more by itself` }),
  })
}
