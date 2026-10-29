import { useContext } from "react"
import { cn } from "cn"
import { GitPullRequestCreateIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchMenuItems } from "@/components/branch/BranchActions"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { getCreatePullRequestItem } from "@/hooks/useBranchContextActions"
import { useLandingPushes, useUnpushedChildren } from "@/hooks/useBranchPushes"
import { ClosePreviewContext } from "@/hooks/useClosePreview"
import { useUnclaimedOperation } from "@/hooks/useRowOperation"
import { useLandsByPush } from "@/hooks/useSafePush"
import type { MenuItemGroups } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { toPreviewMenu } from "@/lib/menu"
import { openPullRequest } from "@/lib/pull-request"
import { cancelIntent } from "@/model/intents"
import { useRowPart } from "@/model/store"
import { presentStopWaiting, waitingOf } from "@/presenters/lock-wait"
import { firstPushPending, firstPushWait } from "@/presenters/push-button"
import type { Branch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { operationBusyOf } from "@/stores/indicators"
import { usePatronusAvailable, usePatronusOffered, usePatronusPending, usePushHold } from "@/stores/patronus"
import { usePullRequest, usePullRequestsLoading } from "@/stores/pull-requests"

/**
 * The actions on a branch in the footer of its preview, as its menu offers them: O Open Pull Request, Create Pull
 * Request… of a branch on GitHub without a live one (also beside a closed one; disabled with the reason once it landed),
 * S Safe Push where Patronus is, Push to master where the repository lands by a push
 * (disabled with the reason of the next step), P Push for a branch with work, disabled with why while its first push
 * is a step of its landing or the answers that tell it are still to come (`firstPushWait`), so it keeps its place; the
 * rest of the row's menu is under "⋯"
 * of the header (`BranchPreviewMenu`). The Safe Push popover opens at the row and "Create pull request" a dialog of
 * its own: the preview closes for them (`ClosePreviewContext`).
 */
export function BranchPreviewActions({ branch }: { branch: Branch }) {
  const pullRequest = usePullRequest(branch.name)
  // until Patronus answers, its Safe Push waits there, disabled with why
  const patronusOffered = usePatronusOffered()
  // or Push to master, where the repository lands by a push of the copy into the target
  const landsByPush = useLandsByPush()
  const closePreview = useContext(ClosePreviewContext)
  // the menu's item: "Create Pull Request…" of a branch without a live pull request, on GitHub, as Rust offers it; the
  // actions of the branch it subscribes to are its argument, so the item follows them
  const actions = useRowPart(branch.name, "actions")
  const createItem = getCreatePullRequestItem(branch, actions)
  // or Merge #7, where Rust offers the merge of its pull request on GitHub
  const landsByMerge = actions?.some(action => action.id === "merge-pull-request") === true
  // as the push button of the row: only a branch that synced without an error has something to push
  // the first push of a branch is a step of Safe Push and Create Pull Request: "⋯" offers it; unless Push with Children
  // takes the stack branch of the whole stack along, whose open pull request it updates
  const stackBranchAlong = (useUnpushedChildren(branch.name)?.stackBranch ?? null) !== null
  const landingPushes = useLandingPushes(branch.name) && !stackBranchAlong
  const held = usePushHold(branch.name) !== null
  const hasPush = !branch.hasError && branch.commitCount > 0
  // a first push the landing makes keeps its place, disabled with why: the answers that decide it move no button
  const waits = firstPushWait(branch, { landingPushes, patronus: usePatronusAvailable(), held, pending: firstPushPending(usePatronusPending(), usePullRequestsLoading()) })
  return (
    <>
      {pullRequest !== undefined && (
        <Button variant="ghost" size="xs" onClick={() => openPullRequest(pullRequest)}>
          <Kbd>O</Kbd>
          Open Pull Request
        </Button>
      )}
      {createItem !== null && (
        <Tooltip disabled={createItem.disabled !== true}>
          <TooltipTrigger
            render={
              <Button
                variant="ghost"
                size="xs"
                disabled={createItem.disabled}
                focusableWhenDisabled
                className={cn(createItem.disabled === true && "opacity-50")}
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
      {(patronusOffered || landsByPush || landsByMerge) && <SafePushButton branch={branch} hint={<Kbd>S</Kbd>} />}
      {hasPush && <BranchPushButton branch={branch.row} colored={false} hint={<Kbd>P</Kbd>} confirmTarget="preview" waits={waits} />}
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
