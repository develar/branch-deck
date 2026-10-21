import { CloudUploadIcon, RocketIcon, Trash2Icon } from "lucide-react"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { pushUnappliedBranch } from "@/lib/git/unapplied-push"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { getPushAction } from "@/lib/push-summary"
import { openInline } from "@/stores/inline-row-action"
import type { UnappliedBranch } from "@/stores/unapplied-branches"
import { useUnappliedBranch } from "@/stores/unapplied-branches"

/** Why an unapplied branch starts no Safe Push and no Dry Run */
export const UNAPPLIED_LANDS_NOTHING = "Not in master: an unapplied branch lands nothing"

/** The menu of the unapplied row `name` (its full name), from its record in the store, as every surface renders it */
export function useUnappliedBranchMenu(name: string): MenuItemGroups {
  return getUnappliedBranchMenu(name, useUnappliedBranch(name))
}

/**
 * The menu of an unapplied branch: Push (Force Push when the remote has another history, disabled when it has it as it
 * is), Safe Push disabled with why, Copy, and Delete Unapplied Branch…
 */
export function getUnappliedBranchMenu(name: string, branch: UnappliedBranch | undefined): MenuItemGroups {
  const action = getPushAction({ remoteStatus: branch?.remoteStatus ?? null }) ?? "create"
  const push: MenuItem = {
    id: "push",
    label: action === "force" ? "Force Push" : "Push",
    icon: CloudUploadIcon,
    disabled: action === "upToDate",
    description: action === "upToDate" ? "The remote has it as it is" : `Keeps a copy on the remote as ${name}`,
    onSelect: () => void pushUnappliedBranch(name),
  }
  const safePush: MenuItem = { id: "safe-push", label: "Safe Push", icon: RocketIcon, disabled: true, description: UNAPPLIED_LANDS_NOTHING, onSelect: () => {} }
  return [
    [push, safePush],
    getBranchCopyMenuItems(name, true),
    [{ id: "delete-unapplied", label: "Delete Unapplied Branch…", icon: Trash2Icon, color: "destructive", onSelect: () => openInline("delete-unapplied", name) }],
  ]
}
