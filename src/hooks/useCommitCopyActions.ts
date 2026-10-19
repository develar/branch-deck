import { CopyIcon } from "lucide-react"
import type { CommitFiles } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import { getChangedPaths } from "@/lib/commit-files"
import { notifyError } from "@/lib/errors"
import type { MenuItem } from "@/lib/menu"
import { getFilesOfCommit, loadCommitFiles } from "@/stores/commit-files"

/**
 * "Copy Commit Hash(es)" / "Copy Commit Message(s)" / "Copy Changed Paths" context menu items; one line per commit,
 * and per path.
 */
export function getCommitCopyMenuItems(commits: readonly CommitUnion[]): MenuItem[] {
  return copyMenuItems(
    commits.map(getCommitHash).filter(Boolean),
    commits.map(commit => commit.message),
  )
}

/** The copies of a commit of a review window, by the id it shows (its copy in the virtual branch) */
export function getReviewCommitCopyMenuItems(commit: { readonly id: string; readonly message: string }): MenuItem[] {
  return copyMenuItems([commit.id], [commit.message])
}

function copyMenuItems(hashes: readonly string[], messages: readonly string[]): MenuItem[] {
  const isMultiple = hashes.length > 1
  return [
    {
      id: "copy-commit-hash",
      label: isMultiple ? `Copy ${hashes.length} Commit Hashes` : "Copy Commit Hash",
      icon: CopyIcon,
      onSelect: () => void copyText(hashes.join("\n")),
    },
    {
      id: "copy-commit-message",
      label: isMultiple ? `Copy ${hashes.length} Commit Messages` : "Copy Commit Message",
      icon: CopyIcon,
      onSelect: () => void copyText(messages.filter(Boolean).join("\n")),
    },
    {
      id: "copy-changed-paths",
      label: isMultiple ? `Copy Changed Paths of ${hashes.length} Commits` : "Copy Changed Paths",
      icon: CopyIcon,
      onSelect: () => void copyChangedPaths(hashes),
    },
  ]
}

/**
 * The paths the commits changed, each once. The list loaded the files of its commits, so the copy is synchronous, in
 * the click; only files still loading are waited for.
 */
async function copyChangedPaths(commitIds: readonly string[]): Promise<void> {
  const known = commitIds.map(getFilesOfCommit)
  if (known.some(files => files === undefined)) {
    await loadCommitFiles(commitIds)
  }
  const files = commitIds.map(getFilesOfCommit).filter((it): it is CommitFiles => it !== undefined)
  if (files.length < commitIds.length) {
    notifyError("Cannot copy the changed paths", "The files of the commits could not be read")
    return
  }
  await copyText(getChangedPaths(files).join("\n"))
}
