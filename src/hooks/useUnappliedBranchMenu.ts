import { ArchiveRestoreIcon, CloudUploadIcon, RocketIcon, Trash2Icon } from "lucide-react"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { reapplyBranch } from "@/lib/git/reapply"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { pushUnappliedBranch } from "@/lib/push-run"
import { openInline } from "@/stores/inline-row-action"
import type { UnappliedBranch } from "@/stores/unapplied-branches"
import { useUnappliedBranch } from "@/stores/unapplied-branches"

/** Why an unapplied branch starts no Safe Push and no Dry Run */
export const UNAPPLIED_LANDS_NOTHING = "Not in master: Reapply it to land it"

/** The menu of the unapplied row `name` (its full name), from its record in the store, as every surface renders it */
export function useUnappliedBranchMenu(name: string): MenuItemGroups {
  return getUnappliedBranchMenu(name, useUnappliedBranch(name))
}

/**
 * The menu of an unapplied branch: Reapply, Push (Force Push when the remote has another history, disabled when it has
 * it as it is), Safe Push disabled with why, Copy, and Delete Unapplied Branch…
 */
export function getUnappliedBranchMenu(name: string, branch: UnappliedBranch | undefined): MenuItemGroups {
  const action = branch?.remoteStatus?.action.kind ?? "create"
  const push: MenuItem = {
    id: "push",
    label: action === "force" ? "Force Push Branch" : "Push Branch",
    icon: CloudUploadIcon,
    disabled: action === "upToDate",
    description: action === "upToDate" ? "The remote has it as it is" : `Keeps a copy on the remote as ${name}`,
    onSelect: () => void pushUnappliedBranch(name),
  }
  const safePush: MenuItem = { id: "safe-push", label: "Safe Push", icon: RocketIcon, disabled: true, description: UNAPPLIED_LANDS_NOTHING, onSelect: () => {} }
  const reapply: MenuItem = {
    id: "reapply",
    label: "Reapply",
    icon: ArchiveRestoreIcon,
    description: "Puts its commits back on top of your branch, their files too",
    onSelect: () => void reapplyBranch(name),
  }
  return [
    [reapply],
    [push, safePush],
    getBranchCopyMenuItems(name, true),
    [{ id: "delete-unapplied", label: "Delete Unapplied Branch…", icon: Trash2Icon, color: "destructive", onSelect: () => openInline("delete-unapplied", name) }],
  ]
}
