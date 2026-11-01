import { FileDiffIcon } from "lucide-react"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import type { MenuItem } from "@/lib/menu"
import { getFilesOfCommit } from "@/stores/commit-files"

/**
 * Why the commits the user selected in a row can't be reviewed, a selection of the page and no action of a subject Rust
 * offers: a single commit without files; null when it can. A selection of several is reviewed together
 * (`getCommitReviewMenuItems`)
 */
export function unreviewableSelection(commits: readonly CommitUnion[]): string | null {
  if (commits.length !== 1) {
    return null
  }
  const files = getFilesOfCommit(getCommitHash(commits[0]!))
  return files !== undefined && files.files.length === 0 ? "The commit changes no files" : null
}

/**
 * "Review Changes" (D) of a commit row, the review key of the row (`usePreviewHotkeys`): the review in its window, of
 * the commit or of the commits selected with it; disabled with the reason for one without files
 */
export function getCommitReviewMenuItems(commits: readonly CommitUnion[], review: (commits: readonly CommitUnion[]) => void): MenuItem[] {
  const blocker = unreviewableSelection(commits)
  return [
    {
      id: "review-changes",
      label: "Review Changes",
      icon: FileDiffIcon,
      kbds: ["d"],
      disabled: blocker !== null,
      description: blocker ?? (commits.length === 1 ? "The tree of the files of the commit beside all its diffs, in a window" : `The ${commits.length} commits, in a window`),
      onSelect: () => review(commits),
    },
  ]
}
