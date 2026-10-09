import { CopyIcon } from "lucide-react"
import { notifyError } from "@/lib/errors"
import type { MenuItem } from "@/lib/menu"
import { getFullBranchName } from "@/stores/repository"

function copy(text: string): void {
  navigator.clipboard.writeText(text).catch((error: unknown) => {
    notifyError("Copy Failed", error)
  })
}

/**
 * "Copy Full Branch Name" / "Copy Branch Name" context menu items.
 * `isFullPath`: `branchName` is already the full git branch name (archived branches), otherwise it is the virtual branch name.
 */
export function getBranchCopyMenuItems(branchName: string, isFullPath = false): MenuItem[] {
  return [
    {
      label: "Copy Full Branch Name",
      icon: CopyIcon,
      onSelect: () => copy(isFullPath ? branchName : getFullBranchName(branchName)),
    },
    {
      label: "Copy Branch Name",
      icon: CopyIcon,
      onSelect: () => copy(isFullPath ? branchName.slice(branchName.lastIndexOf("/") + 1) : branchName),
    },
  ]
}

const branchCopyActions = { getCopyMenuItems: getBranchCopyMenuItems }

export function useBranchCopyActions(): typeof branchCopyActions {
  return branchCopyActions
}
