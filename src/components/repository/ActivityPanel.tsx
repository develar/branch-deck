import { useEffect, useRef, useSyncExternalStore } from "react"
import type { FocusEvent, KeyboardEvent } from "react"
import { AppWindowIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { ActivityLogActions } from "@/components/repository/ActivityLog"
import { ActivityPanelContent } from "@/components/repository/ActivityPanelContent"
import { openActivityInWindow } from "@/hooks/useActivityPanel"
import type { ActivityWindowState } from "@/lib/activity-window"
import { buildActivityState, subscribeActivityState } from "@/lib/activity-window-link"
import { notifyError } from "@/lib/errors"
import { dismissLanded } from "@/lib/landed-run"
import { canUndoActivity, undoActivity, useActivityOf, useUnavailableJournals } from "@/stores/activity"
import { closeActivityPanel, useActivityPanelOpen } from "@/stores/activity-panel"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject } from "@/stores/repository"

/** What the panel shows besides the log, as JSON too: a change of a store that changes nothing of it renders nothing */
let shownState: { json: string; state: ActivityWindowState | null } | null = null

/** Builds the state of the selected repository; true when it changed */
function refreshShownState(): boolean {
  const repositoryPath = getSelectedProject()?.path
  const state = repositoryPath === undefined ? null : buildActivityState({ repositoryPath })
  const json = JSON.stringify(state)
  if (shownState?.json === json) {
    return false
  }
  shownState = { json, state }
  return true
}

function subscribeShownState(notify: () => void): () => void {
  refreshShownState()
  return subscribeActivityState(() => {
    if (refreshShownState()) {
      notify()
    }
  })
}

function getShownState(): ActivityWindowState | null {
  if (shownState === null) {
    refreshShownState()
  }
  return shownState!.state
}

/**
 * The Activity side panel of the main window (spec/repository/activity.spec.md): one island beside the branch tables,
 * its 40px header with "Activity", Open in Window and Close, then the search, the chips and the log of the selected
 * repository, as this window read it, with what only this window knows. A keyboard open puts the focus into the log;
 * Escape there, or Close, closes the panel and gives the focus back where it was before it came in.
 */
export function ActivityPanel() {
  const state = useSyncExternalStore(subscribeShownState, getShownState)
  const entries = useActivityOf(state?.repositoryPath)
  const unavailable = useUnavailableJournals(state?.repositoryPath)
  const { openedAt, kind, focus } = useActivityPanelOpen()
  const root = useRef<HTMLDivElement>(null)
  const list = useRef<HTMLDivElement>(null)
  // the element that had the focus before it came into the panel
  const focusBefore = useRef<HTMLElement | null>(null)

  useEffect(() => {
    if (focus) {
      list.current?.focus()
    }
  }, [openedAt, focus])

  if (state === null) {
    return null
  }
  const { repositoryPath } = state
  const actions: ActivityLogActions = {
    branches: new Set(state.branches),
    undismissed: new Set(state.undismissed),
    links: state.links,
    onReveal: name => requestReveal({ branchName: name, commitIds: [] }),
    onUndo: entry => {
      if (canUndoActivity(entry)) {
        undoActivity(entry)
      } else {
        notifyError(`Cannot undo ${entry.title}`, "its time ran out")
      }
    },
    onDismiss: () => void dismissLanded(repositoryPath),
  }

  const close = () => {
    const before = focusBefore.current
    if (root.current?.contains(document.activeElement) === true && before?.isConnected === true) {
      before.focus()
    }
    closeActivityPanel()
  }
  const onFocus = (event: FocusEvent<HTMLDivElement>) => {
    const from = event.relatedTarget
    if (from instanceof HTMLElement && !event.currentTarget.contains(from)) {
      focusBefore.current = from
    }
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    // a popover of an entry lives in a portal: its Escape is its own
    if (event.key !== "Escape" || event.defaultPrevented || !(event.target instanceof Node) || !event.currentTarget.contains(event.target)) {
      return
    }
    event.preventDefault()
    close()
  }

  return (
    <Card ref={root} role="complementary" aria-label="Activity" className="h-full min-h-0 gap-0 py-0" data-testid="activity-panel" onFocus={onFocus} onKeyDown={onKeyDown}>
      <div className="flex h-10 shrink-0 items-center gap-1 pr-2 pl-4">
        <h2 className="min-w-0 flex-1 truncate text-sm font-medium">Activity</h2>
        <Button size="xs" variant="ghost" data-testid="activity-panel-window" onClick={() => openActivityInWindow(kind)}>
          <AppWindowIcon data-icon="inline-start" />
          Open in Window
        </Button>
        <Tooltip>
          <TooltipTrigger render={<Button size="icon-xs" variant="ghost" aria-label="Close Activity" data-testid="activity-panel-close" onClick={close} />}>
            <XIcon />
          </TooltipTrigger>
          <TooltipContent>Close Activity</TooltipContent>
        </Tooltip>
      </div>
      <ActivityPanelContent
        key={openedAt}
        entries={entries}
        unavailable={unavailable}
        initialFilter={kind}
        openedAt={openedAt}
        actions={actions}
        layout="panel"
        listRef={list}
        slashFocus={false}
      />
    </Card>
  )
}
