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
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import { ClosePreviewContext } from "@/hooks/useClosePreview"
import { isFirstPushInLanding, useLandingPushes, useUnpushedChildren } from "@/hooks/usePushButton"
import type { MenuItemGroups } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { toPreviewMenu } from "@/lib/menu"
import { openPullRequest } from "@/lib/pull-request"
import type { Branch } from "@/stores/branch-sync"
import { useIsBranchPushing } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { branchSubject, operationLabel, stopWaitingItem, useUnclaimedOperation } from "@/stores/operations"
import { usePatronusOffered, usePushHold } from "@/stores/patronus"
import { usePullRequest } from "@/stores/pull-requests"

/**
 * The actions on a branch in the footer of its preview, as its menu offers them: O Open Pull Request, Create Pull
 * Request… of a branch on GitHub without a live one (also beside a closed one; disabled with the reason once it landed),
 * S Safe Push where Patronus is
 * (disabled with the reason of the next step), P Push for a branch with work, unless its first push is a step of its
 * landing (`isFirstPushInLanding`); the rest of the row's menu is under "⋯"
 * of the header (`BranchPreviewMenu`). The Safe Push popover opens at the row and "Create pull request" a dialog of
 * its own: the preview closes for them (`ClosePreviewContext`).
 */
export function BranchPreviewActions({ branch }: { branch: Branch }) {
  const pullRequest = usePullRequest(branch.name)
  // until Patronus answers, its Safe Push waits there, disabled with why
  const patronusOffered = usePatronusOffered()
  const closePreview = useContext(ClosePreviewContext)
  // the menu's item: "Create Pull Request…" of a branch without a live pull request, on GitHub; disabled with the reason
  // of the next step once nothing of it is left to land
  const landed = useBranchNextStep(branch.name)?.landed ?? null
  const createItem = getCreatePullRequestItem(branch, landed)
  // as the push button of the row: only a branch that synced without an error has something to push
  // the first push of a branch is a step of Safe Push and Create Pull Request: "⋯" offers it; unless Push with Children
  // takes the stack branch of the whole stack along, whose open pull request it updates
  const stackBranchAlong = (useUnpushedChildren(branch.name)?.stackBranch ?? null) !== null
  const landingPushes = useLandingPushes(branch.name) && !stackBranchAlong
  const isPushing = useIsBranchPushing(branch.name)
  const held = usePushHold(branch.name) !== null
  const hasPush = !branch.hasError && branch.commitCount > 0 && (isPushing || !isFirstPushInLanding(branch, { landingPushes, held }))
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
      {patronusOffered && <SafePushButton branch={branch} hint={<Kbd>S</Kbd>} />}
      {hasPush && <BranchPushButton branch={branch} colored={false} hint={<Kbd>P</Kbd>} confirmTarget="preview" />}
    </>
  )
}

/**
 * "⋯" of the preview's header, before its close button: the whole menu of the row (`getBranchContextMenuItems`), so the
 * expanded form offers what the compact one does; the footer keeps the keyed actions
 */
export function BranchPreviewMenu({ name }: { name: string }) {
  // an action started from this menu shows its spinner here, as on the row's "⋯"
  const operation = useUnclaimedOperation(branchSubject(name))
  const busy = operation === undefined ? null : operationLabel(operation)
  return (
    <MoreActionsMenu name={name} busy={busy} stopWaiting={stopWaitingItem(operation)} menu={<BranchMenuItems name={name} setExpanded={expandRow} Items={PreviewMenuItems} />} />
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
