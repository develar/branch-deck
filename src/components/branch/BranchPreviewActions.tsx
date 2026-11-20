import { useContext } from "react"
import { GitPullRequestCreateIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchMenuItems } from "@/components/branch/BranchActions"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { getCreatePullRequestItem, openPullRequestItemOf } from "@/hooks/useBranchContextActions"
import { ClosePreviewContext } from "@/hooks/useClosePreview"
import { useUnclaimedOperation } from "@/hooks/useRowOperation"
import { useLandsByPush } from "@/hooks/useSafePush"
import type { MenuItemGroups } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { toPreviewMenu } from "@/lib/menu"
import { cancelIntent } from "@/model/intents"
import { useRowPart } from "@/model/store"
import { presentStopWaiting, waitingOf } from "@/presenters/lock-wait"
import { formatFirstPush } from "@/presenters/push-button"
import type { Branch } from "@/stores/branch-sync"
import { usePushSlot } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { operationBusyOf } from "@/stores/indicators"
import { usePullRequest } from "@/stores/pull-requests"

/**
 * The actions on a branch in the footer of its preview, as its menu offers them: O Open Pull Request #7, Create Pull
 * Request… of a branch on GitHub without a live one (also beside a closed one; disabled with the reason once it landed),
 * S Safe Push where Patronus is, Push to master where the repository lands by a push
 * (disabled with the reason of the next step), P Push for a branch with work, disabled with why while its first push
 * is a step of its landing or the answers that tell it are still to come (`PushSlot.first`), so it keeps its place; the
 * rest of the row's menu is under "⋯"
 * of the header (`BranchPreviewMenu`). The Safe Push popover opens at the row and "Create pull request" a dialog of
 * its own: the preview closes for them (`ClosePreviewContext`).
 */
export function BranchPreviewActions({ branch }: { branch: Branch }) {
  const pullRequest = usePullRequest(branch.name)
  // the menu's item, "Open Pull Request #7"
  const openItem = pullRequest === undefined ? null : openPullRequestItemOf(pullRequest)
  // or Push to master, where the repository lands by a push of the copy into the target
  const landsByPush = useLandsByPush()
  const closePreview = useContext(ClosePreviewContext)
  // the menu's item: "Create Pull Request…" of a branch without a live pull request, on GitHub, as Rust offers it; the
  // actions of the branch it subscribes to are its argument, so the item follows them
  const actions = useRowPart(branch.name, "actions")
  const createItem = getCreatePullRequestItem(branch, actions)
  // Safe Push where Rust offers it (by what git tells of the remotes, from the first view: until Patronus answers it
  // waits there, disabled with why), or Merge #7, where Rust offers the merge of its pull request on GitHub
  const offersSafePush = actions?.some(action => action.id === "safe-push") === true
  const landsByMerge = actions?.some(action => action.id === "merge-pull-request") === true
  // the push keeps its place where the branch has one (Rust's `PushSlot`: none when its sync failed or it has no
  // commits), disabled with why while its first push is a step of its landing or the answers that tell it are to come
  const slot = usePushSlot(branch.name)
  const kept = slot !== null && slot.none !== true
  const waits = formatFirstPush(slot?.first, branch.name)
  return (
    <>
      {/* the slot of the pull request, as the menu has it: Create, then Open */}
      {createItem !== null && (
        <Tooltip disabled={createItem.disabled !== true}>
          <TooltipTrigger
            render={
              <Button
                variant="ghost"
                size="xs"
                disabled={createItem.disabled}
                focusableWhenDisabled
                onClick={() => {
                  closePreview?.()
                  createItem.onSelect(FROM_MENU)
                }}
              />
            }
          >
            <GitPullRequestCreateIcon data-icon="inline-start" />
            {createItem.label}
          </TooltipTrigger>
          <TooltipContent>{createItem.description}</TooltipContent>
        </Tooltip>
      )}
      {openItem !== null && (
        <Button variant="ghost" size="xs" onClick={() => openItem.onSelect(FROM_MENU)}>
          <Kbd>O</Kbd>
          {openItem.label}
        </Button>
      )}
      {(offersSafePush || landsByPush || landsByMerge) && <SafePushButton branch={branch} hint={<Kbd>S</Kbd>} />}
      {kept && <BranchPushButton branch={branch.row} colored={false} hint={<Kbd>P</Kbd>} confirmTarget="preview" waits={waits} />}
    </>
  )
}

/**
 * "⋯" of the preview's header, before its close button: the whole menu of the row (`getBranchContextMenuItems`), so the
 * expanded form offers what the compact one does; the footer keeps the keyed actions
 */
export function BranchPreviewMenu({ name }: { name: string }) {
  // an action started from this menu shows its spinner here, as on the row's "⋯"
  const operation = useUnclaimedOperation(name)
  const busy = operation === undefined ? null : operationBusyOf(operation)
  return (
    <MoreActionsMenu
      name={name}
      busy={busy}
      stopWaiting={presentStopWaiting(waitingOf(operation), cancelIntent)}
      menu={<BranchMenuItems name={name} setExpanded={expandRow} Items={PreviewMenuItems} />}
    />
  )
}

/** The keys the preview handles itself (`BranchPreviewDialog`): only their hints are shown in its menu */
const PREVIEW_KEYS = new Set(["p", "o", "d", "s"])

/** The row of the branch expands to show the commits an item works on (Amend, Unapply), as the reveal of a move does */
function expandRow(name: string, expanded: boolean): void {
  if (expanded) {
    requestReveal({ branchName: name, commitIds: [] })
  }
}

/** The menu of the row in the preview (`toPreviewMenu`) */
function PreviewMenuItems({ groups }: { groups: MenuItemGroups }) {
  return <DropdownMenuItems groups={toPreviewMenu(groups, useContext(ClosePreviewContext), PREVIEW_KEYS)} />
}
