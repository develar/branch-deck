import type { ReviewCommit } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { createWindowLink } from "@/lib/sub-window-link"
import { getDiffRepositoryPath } from "@/stores/repository"

/** The label of the review window of a commit: one window, reused for the next commit */
export const COMMIT_REVIEW_WINDOW = "commit-review"

/** `__INIT_DATA__` of the review window of a commit (`openCommitReviewWindow`) */
export interface CommitReviewWindowData {
  /** The repository whose diffs the window reads (`getDiffRepositoryPath`) */
  readonly repositoryPath: string
  /** The list the commit was reviewed from, as it was: ⌥↑/⌥↓ step through it. Commits don't change, so it stays true. */
  readonly commits: readonly ReviewCommit[]
  readonly index: number
  /** The file on screen in the dialog it was moved from */
  readonly path: string | null
  /** A new value on every open: the window starts over */
  readonly openedAt: number
}

/** The open window: it has no state of the main window, only where it is open (the reviews of commits go there) */
const link = createWindowLink<{ repositoryPath: string }>({ label: COMMIT_REVIEW_WINDOW })

/** "Review a1b2c3d Sort the start-up report", the title of the review window of a commit */
export function commitReviewTitle(commit: Pick<ReviewCommit, "id" | "subject">): string {
  return `Review ${formatShortHash(commit.id)} ${commit.subject}`
}

/** The review window is open on a commit of `repositoryPath`: a review of a commit row opens there */
export function isCommitReviewWindowOpenFor(repositoryPath: string | undefined): boolean {
  return repositoryPath !== undefined && link.isOpenFor(repositoryPath)
}

/** Reviews `commits[index]` of a list in the review window of a commit, at `path` */
export async function openCommitReviewWindow(commits: readonly ReviewCommit[], index: number, path: string | null): Promise<void> {
  const repositoryPath = getDiffRepositoryPath()
  const commit = commits[index]
  if (repositoryPath === undefined || commit === undefined) {
    return
  }
  const data: CommitReviewWindowData = { repositoryPath, commits, index, path, openedAt: Date.now() }
  try {
    await link.open({ repositoryPath }, { url: "/commit-review", title: commitReviewTitle(commit), data })
  } catch (error) {
    notifyError(`Cannot open the review of ${formatShortHash(commit.id)}`, error)
  }
}
