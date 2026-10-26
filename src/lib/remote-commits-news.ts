import { openUrl } from "@tauri-apps/plugin-opener"
import type { News } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import type { EventToastAction } from "@/lib/event-toast"
import { showEventToast, toastKeys } from "@/lib/event-toast"
import { joinNames } from "@/lib/text"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { getPullRequest, pullRequestsStore } from "@/stores/pull-requests"
import { getRepositorySwitchReason, repositoryStore } from "@/stores/repository"

/** Someone else pushed to the remote branch of a pull request, which the session fetched (`remoteCommits`) */
export interface RemoteCommitsNews {
  readonly branchName: string
  /** The remote tip it fetched */
  readonly tip: string
  /** The commits of others the branch lacks */
  readonly count: number
  /** Whose, newest first */
  readonly authors: readonly string[]
}

/** The commits of others the session fetched (`NewsEvent::RemoteCommits`, told once per remote tip) */
export function remoteCommitsNewsOf(news: readonly News[]): RemoteCommitsNews[] {
  return news.flatMap(({ event }) => (event.kind === "remoteCommits" ? [{ branchName: event.branch, tip: event.tip, count: event.count, authors: event.authors }] : []))
}

/**
 * The words of the toast: "Lev Leontev pushed 1 commit to bazel-dev-target-launch-v3", "Your branch doesn't have it: a
 * push drops it."
 */
export function describeRemoteCommitsNews(news: RemoteCommitsNews): { title: string; description: string } {
  const them = news.count === 1 ? "it" : "them"
  return {
    title: `${joinNames(news.authors)} pushed ${formatCommitCount(news.count)} to ${news.branchName}`,
    description: `Your branch doesn't have ${them}: a push drops ${them}.`,
  }
}

/**
 * Tells the commits of others the session fetched, one toast per branch, which stays until the user acts. It is the toast
 * of the pushes of the branch (`toastKeys.push`): a push of it since takes its place. It opens the pull request, while
 * the main window shows the repository of the news; it closes once the branch is gone
 */
export function tellRemoteCommitsNews(repositoryPath: string, news: readonly News[]): void {
  for (const item of remoteCommitsNewsOf(news)) {
    const { title, description } = describeRemoteCommitsNews(item)
    showEventToast({
      key: toastKeys.push(repositoryPath, item.branchName),
      watch: [branchSyncStore, repositoryStore, pullRequestsStore],
      render: () => {
        if (getRepositorySwitchReason(repositoryPath) !== null) {
          return { type: "warning", title, description, timeout: 0 }
        }
        if (getBranch(item.branchName) === undefined) {
          return null
        }
        return { type: "warning", title, description, action: openPullRequestAction(item.branchName), timeout: 0 }
      },
    })
  }
}

function openPullRequestAction(branchName: string): EventToastAction | undefined {
  const url = getPullRequest(branchName)?.url
  return url === undefined ? undefined : { label: "Open Pull Request", onClick: () => void openUrl(url).catch((error: unknown) => notifyError("Cannot open GitHub", error)) }
}
