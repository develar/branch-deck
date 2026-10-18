import { useContext } from "react"
import { GitPullRequestCreateIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { BranchMenuItems } from "@/components/branch/BranchActions"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { getCreatePullRequestItem } from "@/hooks/useBranchContextActions"
import { ClosePreviewContext } from "@/hooks/useClosePreview"
import type { MenuItemGroups } from "@/lib/menu"
import { toPreviewMenu } from "@/lib/menu"
import { openPullRequest } from "@/lib/pull-request"
import type { Branch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { usePatronusAvailable } from "@/stores/patronus"
import { usePullRequest } from "@/stores/pull-requests"

/**
 * The actions on a branch in the footer of its preview, as its menu offers them: O Open Pull Request, Create Pull
 * Request… of a branch on GitHub without a live one (also beside a closed one), S Safe Push where Patronus is
 * (disabled with the reason of the next step), P Push for a branch with work; the rest of the row's menu is under "⋯"
 * of the header (`BranchPreviewMenu`). The Safe Push popover opens at the row and "Create pull request" a dialog of
 * its own: the preview closes for them (`ClosePreviewContext`).
 */
export function BranchPreviewActions({ branch }: { branch: Branch }) {
  const pullRequest = usePullRequest(branch.name)
  const patronusAvailable = usePatronusAvailable()
  const closePreview = useContext(ClosePreviewContext)
  // the menu's item: "Create Pull Request…" of a branch without a live pull request, on GitHub
  const createItem = getCreatePullRequestItem(branch)
  // as the push button of the row: only a branch that synced without an error has something to push
  const hasPush = !branch.hasError && branch.commitCount > 0
  return (
    <>
      {pullRequest !== undefined && (
        <Button variant="ghost" size="xs" onClick={() => openPullRequest(pullRequest)}>
          <Kbd>O</Kbd>
          Open Pull Request
        </Button>
      )}
      {createItem !== null && (
        <Button
          variant="ghost"
          size="xs"
          onClick={() => {
            closePreview?.()
            createItem.onSelect()
          }}
        >
          <GitPullRequestCreateIcon data-icon="inline-start" />
          {createItem.label}
        </Button>
      )}
      {patronusAvailable && <SafePushButton branch={branch} hint={<Kbd>S</Kbd>} />}
      {hasPush && <BranchPushButton branch={branch} colored={false} hint={<Kbd>P</Kbd>} confirmTarget="preview" />}
    </>
  )
}

/**
 * "⋯" of the preview's header, before its close button: the whole menu of the row (`getBranchContextMenuItems`), so the
 * expanded form offers what the compact one does; the footer keeps the keyed actions
 */
export function BranchPreviewMenu({ name }: { name: string }) {
  return <MoreActionsMenu name={name} menu={<BranchMenuItems name={name} setExpanded={expandRow} Items={PreviewMenuItems} />} />
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
