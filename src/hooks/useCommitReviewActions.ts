import { AppWindowIcon, FileDiffIcon } from "lucide-react"
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
 * "Review Changes" (D) and "Open in Window" (W) of a commit row, the review keys of the row (`usePreviewHotkeys`):
 * the review in the commit preview, or in its window; disabled with the reason for several commits or none with files
 */
export function getCommitReviewMenuItems(commits: readonly CommitUnion[], { review, openInWindow }: { review: (commit: CommitUnion) => void, openInWindow: ((commit: CommitUnion) => void) | null }): MenuItem[] {
  const blocker = commitReviewBlocker(commits)
  const commit = commits[0]!
  const items: MenuItem[] = [{
    id: "review-changes",
    label: "Review Changes",
    icon: FileDiffIcon,
    kbds: ["d"],
    disabled: blocker !== null,
    description: blocker ?? "The tree of the files of the commit beside all its diffs",
    onSelect: () => review(commit),
  }]
  if (openInWindow !== null) {
    items.push({
      id: "open-in-window",
      label: "Open in Window",
      icon: AppWindowIcon,
      kbds: ["w"],
      disabled: blocker !== null,
      description: blocker ?? "The review of the commit in a window of its own",
      onSelect: () => openInWindow(commit),
    })
  }
  return items
}
