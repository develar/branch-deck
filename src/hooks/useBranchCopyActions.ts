import { CopyIcon } from "lucide-react"
import { copyText } from "@/lib/clipboard"
import type { MenuItem } from "@/lib/menu"
import { getFullBranchName } from "@/stores/repository"

/**
 * "Copy Full Branch Name" / "Copy Branch Name" context menu items.
 * `isFullPath`: `branchName` is already the full git branch name (archived branches), otherwise it is the virtual branch name.
 */
export function getBranchCopyMenuItems(branchName: string, isFullPath = false): MenuItem[] {
  return [
    {
      label: "Copy Full Branch Name",
      icon: CopyIcon,
      onSelect: () => void copyText(isFullPath ? branchName : getFullBranchName(branchName)),
    },
    {
      label: "Copy Branch Name",
      icon: CopyIcon,
      onSelect: () => void copyText(isFullPath ? branchName.slice(branchName.lastIndexOf("/") + 1) : branchName),
    },
  ]
}

const branchCopyActions = { getCopyMenuItems: getBranchCopyMenuItems }

export function useBranchCopyActions(): typeof branchCopyActions {
  return branchCopyActions
}
