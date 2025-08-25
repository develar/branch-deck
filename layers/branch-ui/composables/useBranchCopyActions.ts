export interface BranchCopyActionsReturn {
  getCopyMenuItems: (branchName: string, isFullPath?: boolean) => Array<{ label: string, icon: string, onSelect: () => void }>
}

export function useBranchCopyActions(): BranchCopyActionsReturn {
  const { copyToClipboard } = useCopyToClipboard()
  const { getFullBranchName } = useRepository()

  const getCopyMenuItems = (branchName: string, isFullPath = false) => [
    {
      label: "Copy Branch Name",
      icon: "i-lucide-copy",
      onSelect: () => {
        const nameToUse = isFullPath
          ? branchName.substring(branchName.lastIndexOf("/") + 1)
          : branchName
        // noinspection JSIgnoredPromiseFromCall
        copyToClipboard(nameToUse)
      },
    },
    {
      label: "Copy Full Branch Name",
      icon: "i-lucide-copy",
      onSelect: () => {
        const fullName = isFullPath ? branchName : getFullBranchName(branchName)
        // noinspection JSIgnoredPromiseFromCall
        copyToClipboard(fullName)
      },
    },
  ]

  return {
    getCopyMenuItems,
  }
}