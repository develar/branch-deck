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
import { useChildrenPush, useLandingPushes, useStackBranchPushAlone, useUnpushedChildren } from "@/hooks/useBranchPushes"
import { useRowOperation, useUnclaimedOperation } from "@/hooks/useRowOperation"
import type { MenuItemGroups } from "@/lib/menu"
import { cancelIntent } from "@/model/intents"
import { presentStopWaiting, waitingOf } from "@/presenters/lock-wait"
import { firstPushPending, isFirstPushInLanding } from "@/presenters/push-button"
import type { Branch } from "@/stores/branch-sync"
import { useIsBranchPushing, useStackPush } from "@/stores/branch-sync"
import { operationBusyOf } from "@/stores/indicators"
import { usePatronusPending, usePushHold } from "@/stores/patronus"
import { usePullRequestsLoading } from "@/stores/pull-requests"

export interface BranchActionsProps {
  branch: Branch
  setExpanded: (name: string, expanded: boolean) => void
  /** The root of an island: the island header shows the push of its stack, the row only its own push */
  stackPushInHeader?: boolean
}

/**
 * The Actions cell of an active branch row, each kind in its track of the Actions column (`ColumnTrack`), so it lines up
 * across the rows of every card: the compact push ("↑ 3", disabled for a blocked stacked branch; none while the remote
 * branch is up to date, unless a branch stacked on it has work: it is Push with Children then, "↑ 4 with children"; while a
 * Safe Push holds the branch, `PushHold`, which the status says, and while the remote lacks the branch and its landing
 * pushes it, `isFirstPushInLanding`, or may: Patronus and GitHub haven't answered yet), the next step on Space (`NextStepSlot`: "Safe Push", a restart, a wait for
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
  const hold = usePushHold(branch.name)
  const canPush = !branch.hasError && branch.commitCount > 0
  // a running push keeps its button, with its progress, until it finished; a blocked branch says why on its button; a
  // branch a Safe Push holds offers no push, its Status cell says why (the menu, with the reason)
  const idle = !isPushing && stackPush === null && !stackBranchPush
  // the branches stacked on it have work for the remote: the push of a branch with nothing of its own to send is Push
  // with Children (the island header offers it for an island root), the description of any other offers it
  const unpushedChildren = useUnpushedChildren(branch.name)
  const childrenToPush = unpushedChildren !== null && !stackPushInHeader
  // the first push is a step of Safe Push and Create Pull Request: only the menu offers it; unless Push with Children
  // takes the stack branch of the whole stack along, whose open pull request it updates
  const landingPushes = useLandingPushes(branch.name) && (unpushedChildren?.stackBranch ?? null) === null
  const firstPush = isFirstPushInLanding(branch, { landingPushes })
  // until Patronus and GitHub answer whether the landing pushes it, a first push shows no button either: one shown now
  // would leave at their answer and move the columns under the pointer (the settled layout keeps the track where it shows)
  const landingPending = firstPushPending(usePatronusPending(), usePullRequestsLoading())
  const pushAction = branch.remoteStatus?.action.kind
  const firstPushWaits = !landingPushes && landingPending !== null && pushAction === "create"
  // Push with Children in the place of the push waits for the answers of Patronus and GitHub as a first push does: a
  // robot holds the children, a merged one landed them, an open pull request of the whole stack takes its stack branch
  // along, so a button shown before them could leave or come at them; the settled layout keeps the track meanwhile
  const offersChildren = childrenToPush && landingPending === null
  const upToDate = idle && ((pushAction === "upToDate" && branch.status !== "Blocked" && !offersChildren) || hold !== null || firstPush || firstPushWaits)
  // the branch has nothing of its own to send, the branches stacked on it have: its push is Push with Children, as the
  // header of an island, also while that push runs
  const pushesChildren =
    !upToDate && !stackPushInHeader && pushAction === "upToDate" && branch.status !== "Blocked" && hold === null && !isPushing && (offersChildren || startedStackPush !== null)
  return (
    <ActionTracks>
      <ColumnTrack track="push">{canPush && <PushSlot branch={branch} upToDate={upToDate} pushesChildren={pushesChildren} stackPushInHeader={stackPushInHeader} />}</ColumnTrack>
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
function PushSlot({ branch, upToDate, pushesChildren, stackPushInHeader }: { branch: Branch; upToDate: boolean; pushesChildren: boolean; stackPushInHeader: boolean }) {
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
