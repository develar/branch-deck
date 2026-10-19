import { useRef, useState } from "react"
import { CreatePullRequestDialogFrame, CreatePullRequestForm } from "@/components/branch/CreatePullRequestDialog"
import type { CreatePullRequestActions } from "@/components/branch/CreatePullRequestDialog"
import { askMainWindow } from "@/hooks/useMainWindowLink"
import { BRANCH_REVIEW_WINDOW } from "@/lib/branch-review"
import type { CreatePullRequestContext } from "@/lib/create-pull-request-context"
import type { BranchReviewRequest } from "@/types/sub-window"

/**
 * The "Create pull request" dialog of the window: the form of the main window's dialog, its context sent by the main
 * window while it is open (the last one kept while the main window shows another repository, whose Create it refuses),
 * the creation and "Safe Push Without Review…" asked of the main window. It opens once the context came.
 */
export function WindowCreatePullRequestDialog({
  repositoryPath,
  dialog,
  wholeStack,
  context,
  onClose,
}: {
  repositoryPath: string
  dialog: { branchName: string; generation: number } | null
  /** The pull request of the whole stack of the root `dialog` names (the review of the whole stack) */
  wholeStack: boolean
  context: CreatePullRequestContext | null
  onClose: () => void
}) {
  // the last dialog and its context stay while the closing animation runs
  const [shown, setShown] = useState<{ branchName: string; generation: number; context: CreatePullRequestContext } | null>(null)
  if (dialog !== null && context !== null && context.branchName === dialog.branchName && (shown?.generation !== dialog.generation || shown.context !== context)) {
    setShown({ ...dialog, context })
  }
  const [isCreating, setIsCreating] = useState(false)
  const titleRef = useRef<HTMLInputElement>(null)
  const ask = (
    branchName: string,
    request:
      | { dialog: "safePushWithoutReview" }
      | { dialog: "create"; input: Parameters<CreatePullRequestActions["create"]>[0] }
      | { dialog: "continueOnGitHub"; fields: Parameters<CreatePullRequestActions["continueOnGitHub"]>[0] },
  ) => askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, { repositoryPath, branchName, ...(wholeStack && { wholeStack }), ...request })
  const open = dialog !== null && shown?.generation === dialog.generation
  return (
    <CreatePullRequestDialogFrame open={open} isCreating={isCreating} titleRef={titleRef} onClose={onClose}>
      {shown !== null && (
        <CreatePullRequestForm
          key={shown.generation}
          context={shown.context}
          actions={{
            create: input => ask(shown.branchName, { dialog: "create", input }),
            continueOnGitHub: fields => ask(shown.branchName, { dialog: "continueOnGitHub", fields }),
            safePushWithoutReview: () => {
              onClose()
              void ask(shown.branchName, { dialog: "safePushWithoutReview" })
            },
            close: onClose,
          }}
          titleRef={titleRef}
          isCreating={isCreating}
          setIsCreating={setIsCreating}
        />
      )}
    </CreatePullRequestDialogFrame>
  )
}
