import { useEffect, useEffectEvent, useState } from "react"
import { Card } from "@/components/ui/card"
import { BranchReviewActions, BranchReviewBar, BranchReviewBody, BranchReviewGone } from "@/components/review/BranchReview"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { hasCommitKeys, keyedAction, useBranchReview } from "@/hooks/useBranchReview"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { BRANCH_REVIEW_WINDOW, branchGoneReason, branchReviewWindowKey, requestAction } from "@/lib/branch-review"
import { notifyError } from "@/lib/errors"
import { openPullRequestUrl } from "@/lib/pull-request"
import { otherRepositoryReason } from "@/stores/review-requests"
import type { BranchReviewFeedback, BranchReviewItem, BranchReviewRequest, BranchReviewWindowData } from "@/types/sub-window"

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
  const model = useBranchReview(data, data.focus, { known: linked.selected, tips: linked.tips })
  // the actions of each branch while the main window showed the repository and had it: they stay (those that need the
  // main window disabled) while it shows another one, and for a branch that left
  const [seenItems, setSeenItems] = useState(data.state.items)
  const [knownItems, setKnownItems] = useState(data.state.items)
  if (linked.selected && linked.items !== seenItems) {
    setSeenItems(linked.items)
    setKnownItems({ ...knownItems, ...linked.items })
  }
  useLinkedFeedback<BranchReviewFeedback>(BRANCH_REVIEW_WINDOW, feedback => notifyError(feedback.title, feedback.message))

  const { loaded } = model
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
  )
  return (
    <WindowPage
      data-testid="branch-review"
      body="fill"
      title={<BranchReviewBar model={model} />}
      titleEnd={
        loaded !== null && (
          <>
            <DiffViewToggle />
            <div className="flex items-center gap-1" data-testid="branch-review-actions">
              <BranchReviewActions actions={actions.items} branchName={loaded.target} onRun={actions.run} />
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
          <BranchReviewBody model={model} />
        </div>
      </Card>
    </WindowPage>
  )
}

/**
 * The actions of the footer as the main window last sent them: those it runs are disabled with the reason while it shows
 * another repository or the branch left; a pull request opens from here anyway
 */
function shownItems(
  items: readonly BranchReviewItem[],
  { repositoryPath, branchName, selected, gone }: { repositoryPath: string; branchName: string; selected: boolean; gone: boolean },
): readonly BranchReviewItem[] {
  if (selected && !gone) {
    return items
  }
  return items.map(item => {
    if (item.url !== undefined || item.id === "open-pull-request") {
      return item
    }
    const reason = gone ? branchGoneReason(branchName) : otherRepositoryReason(repositoryPath, requestAction(item.id, branchName))
    return { ...item, disabled: true, description: reason }
  })
}

/**
 * The actions on the branch in the title bar, the items of its menu: O opens its pull request, Create Pull Request…
 * asks the main window to open its dialog, S to open its Safe Push form, "Push" to push it, without a key (P is the
 * previous change). Their keys work in the whole window. The window doesn't own the branches: the main window runs what
 * it asks for and says why it can't.
 */
function useBranchReviewActions(repositoryPath: string, branchName: string, items: readonly BranchReviewItem[]) {
  const ask = (itemId: BranchReviewRequest["itemId"]) => void askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, { repositoryPath, branchName, itemId })
  const run = (item: BranchReviewItem) => {
    if (item.url !== undefined) {
      openPullRequestUrl(item.url)
    } else if (item.id !== "open-pull-request") {
      ask(item.id)
    }
  }

  const handleKey = useEffectEvent((event: KeyboardEvent) => {
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
