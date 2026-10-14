import type { ComponentType } from "react"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { CopyButton } from "@/components/shared/CopyButton"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { RowActions } from "@/components/shared/RowActions"
import { getBranchContextMenuItems } from "@/hooks/useBranchContextActions"
import type { MenuItemGroups } from "@/lib/menu"
import type { Branch } from "@/stores/branch-sync"
import { useBranch } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"
import { usePatronusAvailable } from "@/stores/patronus"
import { getFullBranchName } from "@/stores/repository"

export interface BranchActionsProps {
  branch: Branch
  setExpanded: (name: string, expanded: boolean) => void
}

/**
 * Actions column of an active branch row: copy the full branch name, then, at the end of the cell so they line up across
 * the rows, push (disabled for a blocked stacked branch), "Safe Push…" on Space, and "⋯" with the branch menu. A click
 * in the cell, on a disabled button too, doesn't toggle the row.
 */
export function BranchActions({ branch, setExpanded }: BranchActionsProps) {
  const patronusAvailable = usePatronusAvailable()
  const processing = useIsRowProcessing(branch.name)
  const canPush = !branch.hasError && branch.commitCount > 0
  return (
    <RowActions>
      <CopyButton text={() => getFullBranchName(branch.name)} tooltip="Copy full branch name" label={`Copy full name of ${branch.name}`} alwaysVisible className="mr-auto" />
      {canPush && <BranchPushButton branch={branch} />}
      {canPush && patronusAvailable && <SafePushButton branch={branch} />}
      <MoreActionsMenu name={branch.name} disabled={processing} menu={<BranchMenuItems name={branch.name} setExpanded={setExpanded} Items={DropdownMenuItems} />} />
    </RowActions>
  )
}

export interface BranchMenuItemsProps {
  name: string
  setExpanded: (name: string, expanded: boolean) => void
  /** The renderer: `ContextMenuItems` for the right-click, `DropdownMenuItems` for "⋯" */
  Items: ComponentType<{ groups: MenuItemGroups }>
}

/**
 * The branch menu (`getBranchContextMenuItems`), the same under the right-click and under "⋯". Rendered only while the
 * menu is open, so the items are computed lazily from the current branch state.
 */
export function BranchMenuItems({ name, setExpanded, Items }: BranchMenuItemsProps) {
  const branch = useBranch(name)
  if (branch === undefined) {
    return null
  }
  return <Items groups={getBranchContextMenuItems(branch, setExpanded)} />
}
