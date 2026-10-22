import { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { ActivityLogActions } from "@/components/repository/ActivityLog"
import { ActivityPanelContent } from "@/components/repository/ActivityPanelContent"
import { KeyHint, PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { ACTIVITY_WINDOW, activityWindowTitle } from "@/lib/activity-window"
import type { ActivityWindowData, ActivityWindowFeedback, ActivityWindowRequest } from "@/lib/activity-window"
import { logError, notifyError } from "@/lib/errors"

/**
 * The Activity window (spec/repository/activity.spec.md): the log of the repository the main window shows, in one
 * island under a search and the chips of the kinds on the canvas, Clear Log in the footer. The main window owns the log
 * and sends it over the link of the window; a branch name, Undo, Dismiss and Clear Log run there. Each open starts over
 * with the chip it names and an empty search.
 */
export function ActivityPage() {
  const data = useSubWindowData<ActivityWindowData>()
  if (data === null) {
    return null
  }
  return <ActivityWindow key={data.openedAt} data={data} />
}

function ActivityWindow({ data }: { data: ActivityWindowData }) {
  // the window follows the repository the main window shows
  const state = useLinkedState(ACTIVITY_WINDOW, data.state, { anyRepository: true })
  const { repositoryPath } = state
  useEffect(() => {
    getCurrentWindow()
      .setTitle(activityWindowTitle(repositoryPath))
      .catch((error: unknown) => logError("Cannot set the title of Activity", error))
  }, [repositoryPath])
  useLinkedFeedback<ActivityWindowFeedback>(ACTIVITY_WINDOW, feedback => notifyError(feedback.title, feedback.message))

  const ask = (request: { reveal: string } | { undo: string } | { dismiss: true } | { clear: true }) =>
    void askMainWindow<ActivityWindowRequest>(ACTIVITY_WINDOW, { repositoryPath, ...request })
  const actions: ActivityLogActions = {
    branches: new Set(state.branches),
    undismissed: new Set(state.undismissed),
    links: state.links,
    onReveal: name => ask({ reveal: name }),
    onUndo: entry => ask({ undo: entry.id }),
    onDismiss: () => ask({ dismiss: true }),
  }

  return (
    <WindowPage
      title="Activity"
      body="fill"
      data-testid="activity-window"
      footer={
        <PreviewDialogFooter
          className="border-t-0 px-1 py-0"
          start={
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button size="xs" variant="ghost" disabled={state.entries.length === 0} focusableWhenDisabled data-testid="activity-clear" onClick={() => ask({ clear: true })} />
                }
              >
                Clear Log
              </TooltipTrigger>
              <TooltipContent>Forget the whole log of the repository, whatever the filter shows</TooltipContent>
            </Tooltip>
          }
        >
          <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
        </PreviewDialogFooter>
      }
    >
      <ActivityPanelContent entries={state.entries} initialFilter={data.kind} openedAt={data.openedAt} actions={actions} layout="window" />
    </WindowPage>
  )
}
