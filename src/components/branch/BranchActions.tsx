import { useEffectEvent, useLayoutEffect, useRef } from "react"
import type { ComponentType } from "react"
import { ActionTracks } from "@/components/branch/ActionTracks"
import { ColumnTrack } from "@/components/branch/BranchColumns"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { ChildrenPushButton } from "@/components/branch/ChildrenPushButton"
import { NextStepSlot } from "@/components/branch/NextStepSlot"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { useBranchMenu } from "@/hooks/useBranchContextActions"
import { useChildrenPush, useStackBranchPushAlone } from "@/hooks/useBranchPushes"
import { useRowOperation, useUnclaimedOperation } from "@/hooks/useRowOperation"
import type { MenuItemGroups } from "@/lib/menu"
import { cancelIntent } from "@/model/intents"
import { presentStopWaiting, waitingOf } from "@/presenters/lock-wait"
import { rowPushOf } from "@/presenters/push-button"
import type { BranchRowFacts } from "@/stores/branch-sync"
import { useIsBranchPushing, usePushSlot, useStackPush } from "@/stores/branch-sync"
import { operationBusyOf } from "@/stores/indicators"

export interface BranchActionsProps {
  /** What the row shows of the branch (`Branch.row`): the cell renders again only when that changes, not its status */
  branch: BranchRowFacts
  setExpanded: (name: string, expanded: boolean) => void
  /** The root of an island: the island header shows the push of its stack, the row only its own push */
  stackPushInHeader?: boolean
}

/**
 * The Actions cell of an active branch row, each kind in its track of the Actions column (`ColumnTrack`), so it lines up
 * across the rows of every card: the compact push ("↑ 3", disabled for a blocked stacked branch; none while the remote
 * branch is up to date, unless a branch stacked on it has work: it is Push with Children then, "↑ 4 with children"; while a
 * Safe Push holds the branch, which the status says, and while the remote lacks the branch and its landing pushes it,
 * or may: Patronus and GitHub haven't answered yet; Rust decides it, `PushSlot`), the next step on Space (`NextStepSlot`: "Safe Push", a restart, a wait for
 * approvals) and "⋯" with the branch menu. A click in the cell, on a disabled button too, doesn't toggle the row.
 */
export function BranchActions({ branch, setExpanded, stackPushInHeader = false }: BranchActionsProps) {
  const processing = useRowOperation(branch.name) !== undefined
  // an action of the row started from its menu, a key or a toast shows its spinner on "⋯"
  const unclaimed = useUnclaimedOperation(branch.name)
  const unclaimedBusy = unclaimed === undefined ? null : operationBusyOf(unclaimed)
  const isPushing = useIsBranchPushing(branch.name)
  const startedStackPush = useStackPush(branch.name)
  // one indicator of a stack push: the island header of the stack it pushes, or the row it started from; so is the push
  // of the stack branch of the whole stack the branch heads, unless the start of its Safe Push shows it in the slot
  const stackPush = stackPushInHeader ? null : startedStackPush
  const stackBranchPush = useStackBranchPushAlone(branch.name) && !stackPushInHeader
  // where the row shows its push and in what shape, as Rust decides it (`PushSlot`, `domain_policy::push_slot`): the root of
  // an island leaves Push with Children to its header; what runs lies over it, its progress kept until it finished
  const slot = usePushSlot(branch.name)
  const row = rowPushOf(slot, stackPushInHeader)
  const canPush = slot !== null && slot.none !== true
  // a running push keeps its button, with its progress, until it finished
  const idle = !isPushing && stackPush === null && !stackBranchPush
  const upToDate = idle && row.kind === "none"
  // the branch has nothing of its own to send, the branches stacked on it have: its push is Push with Children, also
  // while a push of the stack started from the row runs
  const pushesChildren = !upToDate && !isPushing && row.shape === "children" && (row.kind === "withChildren" || startedStackPush !== null)
  // the answer the push waits for to show (a first push, Push with Children), named on its track as a pending step names
  // its own; none while a push runs, which shows whatever the answers say
  const pushWaitsFor = idle ? row.waitsFor : null
  return (
    <ActionTracks>
      <ColumnTrack track="push" waitsFor={pushWaitsFor}>
        {canPush && <PushSlot branch={branch} upToDate={upToDate} pushesChildren={pushesChildren} stackPushInHeader={stackPushInHeader} />}
      </ColumnTrack>
      <ColumnTrack track="step">
        <NextStepSlot branch={branch} wholeStackInHeader={stackPushInHeader} />
      </ColumnTrack>
      <ColumnTrack track="more">
        <MoreActionsMenu
          name={branch.name}
          disabled={processing}
          busy={unclaimedBusy}
          stopWaiting={presentStopWaiting(waitingOf(unclaimed), cancelIntent)}
          menu={<BranchMenuItems name={branch.name} setExpanded={setExpanded} Items={DropdownMenuItems} />}
        />
      </ColumnTrack>
    </ActionTracks>
  )
}

/**
 * The compact push, or nothing while the row offers none (`upToDate`: the remote branch is up to date, a Safe Push
 * holds the branch, or its first push is a step of its landing); Push with Children of a branch that has nothing of its
 * own to send (`pushesChildren`). The focus in the button or its description (pushed with a click or P) goes to the
 * row when the push is done and the button leaves: the row keeps its keys, Space included.
 */
function PushSlot({ branch, upToDate, pushesChildren, stackPushInHeader }: { branch: BranchRowFacts; upToDate: boolean; pushesChildren: boolean; stackPushInHeader: boolean }) {
  const leftWithFocusRef = useRef(false)
  const name = branch.name
  useLayoutEffect(() => {
    if (upToDate && leftWithFocusRef.current) {
      leftWithFocusRef.current = false
      document.querySelector<HTMLElement>(`tr[data-testid=branch-row][data-branch-name="${CSS.escape(name)}"]`)?.focus({ preventScroll: true })
    }
  }, [upToDate, name])
  if (upToDate) {
    return null
  }
  return (
    <>
      <FocusLeaveWatch name={branch.name} onLeave={() => (leftWithFocusRef.current = true)} />
      {pushesChildren ? <RowChildrenPush name={name} /> : <BranchPushButton branch={branch} confirmTarget="row" compact shortcut="P" stackPushInHeader={stackPushInHeader} />}
    </>
  )
}

/** Push with Children in the place of the push of the row, named and described as the push of the row */
function RowChildrenPush({ name }: { name: string }) {
  const push = useChildrenPush(name)
  return <ChildrenPushButton root={name} push={push} held={false} testId="push-button" descriptionTestId="push-description" row />
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
  // an open menu follows every store it reads: the operations that start and end, the pull requests, the robots
  const groups = useBranchMenu(name, setExpanded)
  return groups === null ? null : <Items groups={groups} />
}
