import { useEffectEvent, useLayoutEffect, useRef } from "react"
import type { ComponentType } from "react"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { NextStepSlot } from "@/components/branch/NextStepSlot"
import { CopyButton } from "@/components/shared/CopyButton"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { RowActions } from "@/components/shared/RowActions"
import { getBranchContextMenuItems } from "@/hooks/useBranchContextActions"
import type { MenuItemGroups } from "@/lib/menu"
import { getPushAction } from "@/lib/push-summary"
import type { Branch } from "@/stores/branch-sync"
import { useBranch, useIsBranchPushing, useStackPushCurrent } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"
import { usePushHold } from "@/stores/patronus"
import { getFullBranchName } from "@/stores/repository"

export interface BranchActionsProps {
  branch: Branch
  setExpanded: (name: string, expanded: boolean) => void
  /** The root of an island: the island header shows the push of its stack, the row only its own push */
  stackPushInHeader?: boolean
}

/**
 * Actions column of an active branch row: copy the full branch name (on the row hover or focus: every row has it), then,
 * at the end of the cell so they line up across the rows, the compact push ("↑ 3", disabled for a blocked stacked
 * branch; its place stays empty while the remote branch is up to date or a Safe Push holds the branch, `PushHold`, which
 * the status says), the next step on
 * Space (`NextStepSlot`: "Safe Push…", a restart, a wait for approvals, one width in every row) and "⋯" with the branch
 * menu. A click in the
 * cell, on a disabled button too, doesn't toggle the row.
 */
export function BranchActions({ branch, setExpanded, stackPushInHeader = false }: BranchActionsProps) {
  const processing = useIsRowProcessing(branch.name)
  const isPushing = useIsBranchPushing(branch.name)
  const startedStackPush = useStackPushCurrent(branch.name)
  // one indicator of a stack push: the island header of the stack it pushes, or the row it started from
  const stackPushCurrent = stackPushInHeader ? null : startedStackPush
  const hold = usePushHold(branch.name)
  const canPush = !branch.hasError && branch.commitCount > 0
  // a running push keeps its button, with its progress, until it finished; a blocked branch says why on its button; a
  // branch a Safe Push holds offers no push, its Status cell says why (the menu, with the reason)
  const idle = !isPushing && stackPushCurrent === null
  const upToDate = idle && ((getPushAction(branch) === "upToDate" && branch.status !== "Blocked") || hold !== null)
  return (
    <RowActions>
      <CopyButton
        text={() => getFullBranchName(branch.name)}
        tooltip="Copy full branch name"
        label={`Copy full name of ${branch.name}`}
        className="mr-auto group-focus-visible:opacity-100 group-has-focus-visible:opacity-100"
      />
      {canPush && <PushSlot branch={branch} upToDate={upToDate} stackPushInHeader={stackPushInHeader} />}
      <NextStepSlot branch={branch} />
      <MoreActionsMenu name={branch.name} disabled={processing} menu={<BranchMenuItems name={branch.name} setExpanded={setExpanded} Items={DropdownMenuItems} />} />
    </RowActions>
  )
}

/**
 * The compact push, or its empty place while the remote branch is up to date. The focus in the button or its
 * description (pushed with a click or P) goes to the row when the push is done and the button leaves: the row keeps
 * its keys, Space included.
 */
function PushSlot({ branch, upToDate, stackPushInHeader }: { branch: Branch; upToDate: boolean; stackPushInHeader: boolean }) {
  const placeholderRef = useRef<HTMLSpanElement>(null)
  const leftWithFocusRef = useRef(false)
  useLayoutEffect(() => {
    if (upToDate && leftWithFocusRef.current) {
      leftWithFocusRef.current = false
      placeholderRef.current?.closest<HTMLElement>("tr")?.focus({ preventScroll: true })
    }
  }, [upToDate])
  if (upToDate) {
    return <span ref={placeholderRef} aria-hidden className="w-16 shrink-0" data-testid="push-placeholder" />
  }
  return (
    <>
      <FocusLeaveWatch name={branch.name} onLeave={() => (leftWithFocusRef.current = true)} />
      <BranchPushButton branch={branch} confirmTarget="row" compact shortcut="P" stackPushInHeader={stackPushInHeader} />
    </>
  )
}

/**
 * Tells when the push button of the row of `name` leaves the page with the focus in it or in its description. Its
 * layout cleanup runs before the nodes of the button are removed, while the focus is still there; a removed element gets
 * no blur to tell it later.
 */
function FocusLeaveWatch({ name, onLeave }: { name: string; onLeave: () => void }) {
  const leave = useEffectEvent(onLeave)
  useLayoutEffect(
    () => () => {
      const active = document.activeElement
      const row = active?.closest<HTMLElement>("tr[data-testid=branch-row]")
      if (
        active != null &&
        ((row?.dataset.branchName === name && active.closest("[data-testid=push-button]") !== null) || active.closest("[data-testid=push-description]") !== null)
      ) {
        leave()
      }
    },
    [name],
  )
  return null
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
