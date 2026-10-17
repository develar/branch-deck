import { FileDiffIcon } from "lucide-react"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import type { MenuItem } from "@/lib/menu"
import { getFilesOfCommit } from "@/stores/commit-files"

/** Why a commit of a row can't be reviewed: a selection of several, a commit without files; null when it can */
export function commitReviewBlocker(commits: readonly CommitUnion[]): string | null {
  if (commits.length !== 1) {
    return "Review one commit at a time"
  }
  const files = getFilesOfCommit(getCommitHash(commits[0]!))
  return files !== undefined && files.files.length === 0 ? "The commit changes no files" : null
}

/**
 * "Review Changes" (D) of a commit row, the review key of the row (`usePreviewHotkeys`): the review in its window;
 * disabled with the reason for several commits or one without files
 */
export function getCommitReviewMenuItems(commits: readonly CommitUnion[], review: (commit: CommitUnion) => void): MenuItem[] {
  const blocker = commitReviewBlocker(commits)
  const commit = commits[0]!
  return [
    {
      id: "review-changes",
      label: "Review Changes",
      icon: FileDiffIcon,
      kbds: ["d"],
      disabled: blocker !== null,
      description: blocker ?? "The tree of the files of the commit beside all its diffs, in a window",
      onSelect: () => review(commit),
    },
  ]
}
