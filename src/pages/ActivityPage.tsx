import { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { ActivityLogActions } from "@/components/repository/ActivityLog"
import { ActivityPanelContent } from "@/components/repository/ActivityPanelContent"
import { KeyHint, PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { useActivityReads } from "@/hooks/useActivityReads"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { ACTIVITY_WINDOW, activityWindowTitle } from "@/lib/activity-window"
import type { ActivityWindowData, ActivityWindowFeedback, ActivityWindowRequest } from "@/lib/activity-window"
import { logError, notifyError } from "@/lib/errors"
import { markWindowContentReady } from "@/lib/sub-window"
import { useActivityOf, useActivityRead, useUnavailableJournals, withUndo } from "@/stores/activity"

/**
 * The Activity window (spec/repository/activity.spec.md): the log of the repository the main window shows, in one
 * island under a search and the chips of the kinds on the canvas, Clear Log in the footer. The window reads the log
 * itself (at each open, and when a journal changed); the main window tells it over the link of the window the shown
 * repository, its branches, the landed report and the Undo of its rewrites, and runs a branch name, Undo, Dismiss and
 * Clear Log. The window shows once the first read answered. Each open starts over with the chip it names and an empty
 * search.
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
  useActivityReads(state.gitHubRepository)
  const read = useActivityRead()
  const entries = withUndo(useActivityOf(repositoryPath), state.undo)
  const unavailable = useUnavailableJournals(repositoryPath)
  useEffect(() => {
    if (read) {
      markWindowContentReady()
    }
  }, [read])
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
      readyByContent
      data-testid="activity-window"
      footer={
        <PreviewDialogFooter
          className="border-t-0 px-1 py-0"
          start={
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button size="xs" variant="ghost" disabled={entries.length === 0} focusableWhenDisabled data-testid="activity-clear" onClick={() => ask({ clear: true })} />
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
      {/* nothing before the first read: the window is still hidden, and an empty log would be news it isn't */}
      {read && <ActivityPanelContent entries={entries} unavailable={unavailable} initialFilter={data.kind} openedAt={data.openedAt} actions={actions} layout="window" />}
    </WindowPage>
  )
}
