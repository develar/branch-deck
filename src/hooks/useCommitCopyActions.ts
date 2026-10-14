import { CopyIcon } from "lucide-react"
import type { CommitFiles } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { copyText } from "@/lib/clipboard"
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
  const isMultiple = commits.length > 1
  return [
    {
      label: isMultiple ? `Copy ${commits.length} Commit Hashes` : "Copy Commit Hash",
      icon: CopyIcon,
      onSelect: () => void copyText(commits.map(getCommitHash).filter(Boolean).join("\n")),
    },
    {
      label: isMultiple ? `Copy ${commits.length} Commit Messages` : "Copy Commit Message",
      icon: CopyIcon,
      onSelect: () => void copyText(commits.map(commit => commit.message).filter(Boolean).join("\n")),
    },
    {
      label: isMultiple ? `Copy Changed Paths of ${commits.length} Commits` : "Copy Changed Paths",
      icon: CopyIcon,
      onSelect: () => void copyChangedPaths(commits.map(getCommitHash).filter(Boolean)),
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

const commitCopyActions = { getCopyMenuItems: getCommitCopyMenuItems }

export function useCommitCopyActions(): typeof commitCopyActions {
  return commitCopyActions
}
