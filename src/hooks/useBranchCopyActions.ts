import { CopyIcon } from "lucide-react"
import { copyText } from "@/lib/clipboard"
import type { MenuItem } from "@/lib/menu"
import { getFullBranchName } from "@/stores/repository"

/**
 * "Copy Full Branch Name" (⌘C on the focused row of a branch table) / "Copy Branch Name" context menu items.
 * `isFullPath`: `branchName` is already the full git branch name (archived branches), otherwise it is the virtual branch name.
 */
export function getBranchCopyMenuItems(branchName: string, isFullPath = false): MenuItem[] {
  return [
    {
      id: "copy-full-branch-name",
      label: "Copy Full Branch Name",
      icon: CopyIcon,
      // ⌘C on the focused row
      kbds: ["mod", "c"],
      onSelect: () => void copyText(isFullPath ? branchName : getFullBranchName(branchName)),
    },
    {
      id: "copy-branch-name",
      label: "Copy Branch Name",
      icon: CopyIcon,
      onSelect: () => void copyText(isFullPath ? branchName.slice(branchName.lastIndexOf("/") + 1) : branchName),
    },
  ]
}
