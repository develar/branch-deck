import { CopyIcon } from "lucide-react"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import type { MenuItem } from "@/lib/menu"

function copy(text: string): void {
  navigator.clipboard.writeText(text).catch((error: unknown) => {
    notifyError("Copy Failed", error)
  })
}

/** "Copy Commit Hash(es)" / "Copy Commit Message(s)" context menu items; one line per commit. */
export function getCommitCopyMenuItems(commits: readonly CommitUnion[]): MenuItem[] {
  const isMultiple = commits.length > 1
  return [
    {
      label: isMultiple ? `Copy ${commits.length} Commit Hashes` : "Copy Commit Hash",
      icon: CopyIcon,
      onSelect: () => copy(commits.map(getCommitHash).filter(Boolean).join("\n")),
    },
    {
      label: isMultiple ? `Copy ${commits.length} Commit Messages` : "Copy Commit Message",
      icon: CopyIcon,
      onSelect: () => copy(commits.map(commit => commit.message).filter(Boolean).join("\n")),
    },
  ]
}

const commitCopyActions = { getCopyMenuItems: getCommitCopyMenuItems }

export function useCommitCopyActions(): typeof commitCopyActions {
  return commitCopyActions
}
