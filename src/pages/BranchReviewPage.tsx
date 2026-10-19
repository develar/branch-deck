import { Suspense, lazy, useEffect, useEffectEvent, useState } from "react"
import { Card } from "@/components/ui/card"
import { BranchReviewActions, BranchReviewBar, BranchReviewBody, BranchReviewGone } from "@/components/review/BranchReview"
import type { BranchReviewMoves } from "@/components/review/BranchReview"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { hasCommitKeys, keyedAction, useBranchReview } from "@/hooks/useBranchReview"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { BRANCH_REVIEW_WINDOW, branchGoneReason, branchReviewWindowKey } from "@/lib/branch-review"
import { notifyError } from "@/lib/errors"
import type { MenuItemGroups } from "@/lib/menu"
import { openPullRequestUrl } from "@/lib/pull-request"
import { repositoryName } from "@/lib/repository-config"
import { showToast } from "@/lib/toast-message"
import { formatWholeStack } from "@/lib/whole-stack"
import type { BranchReviewFeedback, BranchReviewItem, BranchReviewMenuItem, BranchReviewRequest, BranchReviewWindowData } from "@/types/sub-window"

const WindowCreatePullRequestDialog = lazy(() => import("@/components/review/WindowCreatePullRequestDialog").then(it => ({ default: it.WindowCreatePullRequestDialog })))

/**
 * The review window of a branch (`BranchReview` on the canvas of a window of its own): the stack bar, Unified/Split and
 * the actions on the branch of the part in the title bar, the commits and the owner chips above the files beside the
 * diffs in the island, the position and the keys in the footer. The window doesn't own the branches: the main window sends the heads a sync built and the
 * actions of each branch over the link of the window, and runs what it asks for.
 */
export function BranchReviewPage() {
  const data = useSubWindowData<BranchReviewWindowData>()
  if (data === null) {
    return null
  }
  return <BranchReviewWindow key={branchReviewWindowKey(data)} data={data} />
}

function BranchReviewWindow({ data }: { data: BranchReviewWindowData }) {
  // what the main window says of the chain: the heads a sync built, the actions of each branch
  const linked = useLinkedState(BRANCH_REVIEW_WINDOW, data.state)
  // the "Create pull request" dialog of a branch of the chain: in this window, its context from the main window; a move
  // to a commit waits until it closes
  const [dialog, setDialog] = useState<{ branchName: string; generation: number } | null>(null)
  const model = useBranchReview(
    data,
    data.focus,
    { known: linked.selected, tips: linked.tips },
    { openedAt: data.openedAt, focus: data.move ? data.focus : null, held: dialog !== null },
  )
  // the actions of each branch while the main window showed the repository and had it: they stay (those that need the
  // main window disabled) while it shows another one, and for a branch that left
  const [seenItems, setSeenItems] = useState(data.state.items)
  const [knownItems, setKnownItems] = useState(data.state.items)
  if (linked.selected && linked.items !== seenItems) {
    setSeenItems(linked.items)
    setKnownItems({ ...knownItems, ...linked.items })
  }
  useLinkedFeedback<BranchReviewFeedback>(BRANCH_REVIEW_WINDOW, feedback => {
    if (feedback.kind === "error") {
      notifyError(feedback.title, feedback.message)
      return
    }
    const url = feedback.pullRequestUrl
    showToast({ ...feedback.toast, ...(url !== null && { action: { children: "Open Pull Request", onClick: () => openPullRequestUrl(url) } }) })
  })
  // loaded on the first open, then kept mounted (the dialog animates and returns the focus when it closes)
  const [isDialogLoaded, setDialogLoaded] = useState(false)
  if (dialog !== null && !isDialogLoaded) {
    setDialogLoaded(true)
  }
  // the review of the whole stack asks for the actions, the menu and the dialog of the whole stack of its root
  const wholeStack = data.wholeStack === true
  const askDialog = (branchName: string, open: boolean) =>
    void askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, {
      repositoryPath: data.repositoryPath,
      branchName,
      ...(wholeStack && { wholeStack }),
      dialog: open ? "open" : "close",
    })
  const openDialog = (branchName: string) => {
    setDialog({ branchName, generation: (dialog?.generation ?? 0) + 1 })
    askDialog(branchName, true)
  }
  const closeDialog = () => {
    if (dialog !== null) {
      askDialog(dialog.branchName, false)
    }
    setDialog(null)
  }

  const { loaded } = model
  const target = loaded?.target ?? data.chain.at(-1) ?? ""
  // up to the whole stack and down to a branch of it: the main window opens the review as its menus would
  const moves: BranchReviewMoves = {
    wholeStack: linked.wholeStack,
    onWholeStack: () => void askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, { repositoryPath: data.repositoryPath, branchName: target, scope: "wholeStack" }),
    from: data.from ?? [],
    stackCommits: linked.stackCommits,
    commitMenus: linked.commitMenus,
    onCommitMenuItem: (branchName, commitId, commitMenuItemId) =>
      void askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, { repositoryPath: data.repositoryPath, branchName, commitMenuItemId, commitId }),
    onBranch: (branchName, commitKey) =>
      void askMainWindow<BranchReviewRequest>(
        BRANCH_REVIEW_WINDOW,
        commitKey === undefined
          ? { repositoryPath: data.repositoryPath, branchName, review: "branch" }
          : { repositoryPath: data.repositoryPath, branchName, review: "commit", commitKey },
      ),
  }
  const actions = useBranchReviewActions(
    data.repositoryPath,
    loaded?.target ?? "",
    loaded === null
      ? []
      : shownItems(knownItems[loaded.target] ?? [], {
          repositoryPath: data.repositoryPath,
          branchName: loaded.target,
          selected: linked.selected,
          gone: model.gone.includes(loaded.target),
        }),
    { openCreatePullRequest: openDialog, keysSuspended: dialog !== null, wholeStack },
  )
  return (
    <WindowPage
      data-testid="branch-review"
      body="fill"
      readyByContent
      title={<BranchReviewBar model={model} moves={moves} />}
      titleEnd={
        loaded !== null && (
          <>
            <DiffViewToggle />
            <div className="flex items-center gap-1" data-testid="branch-review-actions">
              <BranchReviewActions actions={actions.items} branchName={loaded.target} onRun={actions.run} />
              <BranchReviewMenu
                repositoryPath={data.repositoryPath}
                branchName={loaded.target}
                groups={linked.menus?.[loaded.target] ?? []}
                selected={linked.selected}
                gone={model.gone.includes(loaded.target)}
                wholeStack={wholeStack}
              />
            </div>
          </>
        )
      }
      footer={
        loaded !== null && (
          <PreviewDialogFooter
            className="border-t-0 px-1 py-0"
            data-testid="branch-review-footer"
            start={
              <span className="tabular-nums" data-testid="branch-review-position">
                {loaded.position}
              </span>
            }
          >
            <ReviewKeyHints commits={hasCommitKeys(model)} escape="Close" />
          </PreviewDialogFooter>
        )
      }
    >
      <BranchReviewGone model={model} />
      <Card className="min-h-0 flex-1 flex-row gap-0 overflow-hidden py-0">
        <div className="min-w-0 flex-1">
          <BranchReviewBody model={model} moves={moves} />
        </div>
      </Card>
      {/* outside the review: its keys never see what is typed into the dialog */}
      {isDialogLoaded && (
        <Suspense fallback={null}>
          <WindowCreatePullRequestDialog repositoryPath={data.repositoryPath} dialog={dialog} wholeStack={wholeStack} context={linked.createPullRequest} onClose={closeDialog} />
        </Suspense>
      )}
    </WindowPage>
  )
}

/**
 * "⋯" of the title bar: the whole menu of the branch's row, as the main window sends it; the main window runs the item
 * (it focuses itself and shows the row: a rename or an amend is a form of the row; it switches to this repository first
 * while it shows another one). Disabled once the branch left, as the actions it runs.
 */
function BranchReviewMenu({
  repositoryPath,
  branchName,
  groups,
  selected,
  gone,
  wholeStack,
}: {
  repositoryPath: string
  branchName: string
  groups: readonly (readonly BranchReviewMenuItem[])[]
  selected: boolean
  gone: boolean
  wholeStack: boolean
}) {
  if (groups.length === 0) {
    return null
  }
  const items: MenuItemGroups = groups.map(group =>
    group.map(item => ({
      ...item,
      ...(gone && { disabled: true, description: branchGoneReason(branchName) }),
      ...(!gone && !selected && item.disabled !== true && { description: switchesRepository(repositoryPath) }),
      onSelect: () => void askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, { repositoryPath, branchName, ...(wholeStack && { wholeStack }), menuItemId: item.id }),
    })),
  )
  return <MoreActionsMenu name={wholeStack ? formatWholeStack(branchName) : branchName} menu={<DropdownMenuItems groups={items} />} />
}

/**
 * The actions of the footer as the main window last sent them: those it runs are disabled with the reason once the
 * branch left; while it shows another repository they stay, and their hover says that the main window switches to this
 * one first (`showRepository`); a pull request opens from here anyway
 */
function shownItems(
  items: readonly BranchReviewItem[],
  { repositoryPath, branchName, selected, gone }: { repositoryPath: string; branchName: string; selected: boolean; gone: boolean },
): readonly BranchReviewItem[] {
  if (selected && !gone) {
    return items
  }
  return items.map(item => {
    if (item.url !== undefined || item.id === "open-pull-request" || item.id === "open-stack-pull-request") {
      return item
    }
    return gone ? { ...item, disabled: true, description: branchGoneReason(branchName) } : { ...item, description: switchesRepository(repositoryPath) }
  })
}

/**
 * The actions on the branch in the title bar, the items of its menu: O opens its pull request, Create Pull Request…
 * opens its dialog in this window, S asks the main window for its Safe Push, "Push" to push it, without a key
 * (P is the previous change). Their keys work in the whole window, not while the dialog is open. The window doesn't
 * own the branches: the main window runs what it asks for and says why it can't.
 */
function useBranchReviewActions(
  repositoryPath: string,
  branchName: string,
  items: readonly BranchReviewItem[],
  { openCreatePullRequest, keysSuspended, wholeStack }: { openCreatePullRequest: (branchName: string) => void; keysSuspended: boolean; wholeStack: boolean },
) {
  const ask = (itemId: "safe-push" | "push" | "safe-push-whole-stack" | "push-with-children") =>
    void askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, { repositoryPath, branchName, ...(wholeStack && { wholeStack }), itemId })
  const run = (item: BranchReviewItem) => {
    if (item.url !== undefined) {
      openPullRequestUrl(item.url)
    } else if (item.id === "create-pull-request" || item.id === "create-stack-pull-request") {
      openCreatePullRequest(branchName)
    } else if (item.id !== "open-pull-request" && item.id !== "open-stack-pull-request") {
      ask(item.id)
    }
  }

  const handleKey = useEffectEvent((event: KeyboardEvent) => {
    if (keysSuspended) {
      return
    }
    // the physical keys, as in the branch preview: the key an item names
    const item = keyedAction(items, event)
    if (item !== undefined) {
      event.preventDefault()
      if (item.disabled !== true) {
        run(item)
      }
    }
  })
  // the whole window: the focus may be on the page itself
  useEffect(() => {
    const listener = (event: KeyboardEvent) => handleKey(event)
    window.addEventListener("keydown", listener)
    return () => window.removeEventListener("keydown", listener)
  }, [])
  return { items, run }
}

/** The hover of an action of the window while the main window shows another repository: the consequence of the click */
function switchesRepository(repositoryPath: string): string {
  return `Switches the main window to ${repositoryName(repositoryPath)}`
}
