import type { ReviewCommit } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { openSubWindow } from "@/lib/window-management"
import { getDiffRepositoryPath } from "@/stores/repository"

/** The label of the review window of a commit: one window, reused for the next commit */
export const COMMIT_REVIEW_WINDOW = "commit-review"

/** `__INIT_DATA__` of the review window of a commit (`openCommitReviewWindow`) */
export interface CommitReviewWindowData {
  /** The repository whose diffs the window reads (`getDiffRepositoryPath`) */
  readonly repositoryPath: string
  /** The list the commit was reviewed from, as it was: ⌥↑/⌥↓ step through it. Commits don't change, so it stays true. */
  readonly commits: readonly ReviewCommit[]
  /** The name of that list, in the title bar: the branch, or "Unassigned"; null for a list without one */
  readonly listLabel: string | null
  readonly index: number
  /** The file to show (Enter on a file of the preview's tree); null for the first one, or where the review is when it
   * shows the same commit already */
  readonly path: string | null
  /** A new value on every open: the window starts over */
  readonly openedAt: number
}

/** "Review a1b2c3d Sort the start-up report", the title of the review window of a commit */
export function commitReviewTitle(commit: Pick<ReviewCommit, "id" | "subject">): string {
  return `Review ${formatShortHash(commit.id)} ${commit.subject}`
}

/**
 * Reviews `commits[index]` of a list (named `listLabel`) in the review window of a commit, at `path` (the first file
 * when null). From any window: the missing commits of a conflict are a list in a sub-window. Resolves to whether the
 * window opened; a failure is told.
 */
export async function openCommitReviewWindow(commits: readonly ReviewCommit[], listLabel: string | null, index: number, path: string | null): Promise<boolean> {
  const repositoryPath = getDiffRepositoryPath()
  const commit = commits[index]
  if (repositoryPath === undefined || commit === undefined) {
    return false
  }
  const data: CommitReviewWindowData = { repositoryPath, commits, listLabel, index, path, openedAt: Date.now() }
  return openSubWindow({ windowId: COMMIT_REVIEW_WINDOW, url: "/commit-review", title: commitReviewTitle(commit), data }).then(
    () => true,
    (error: unknown) => {
      notifyError(`Cannot open the review of ${formatShortHash(commit.id)}`, error)
      return false
    },
  )
}
