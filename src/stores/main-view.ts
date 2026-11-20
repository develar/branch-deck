import { flushSync } from "react-dom"
import { invoke } from "@tauri-apps/api/core"
import type { LucideIcon } from "lucide-react"
import { ChartColumnIcon, GitPullRequestIcon, ListTreeIcon } from "lucide-react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { logError } from "@/lib/errors"
import { hasViewTabs } from "@/lib/title-bar"
import { windowLabel } from "@/lib/window-label"

/** The views of the main window: the branch tables, the pull requests the user takes part in, the statistics of the landing into master */
export type MainView = "branches" | "reviews" | "landing"

/** A view as the surfaces that switch to it name it (the command palette) */
export interface MainViewEntry {
  readonly view: MainView
  readonly label: string
  readonly icon: LucideIcon
  /** Its key in the main window (the accelerator of its menu item) */
  readonly kbds?: readonly string[]
}

/** The views the window has, in their order: a view joins the surfaces that switch to it by its entry here */
export const MAIN_VIEWS: readonly MainViewEntry[] = [
  { view: "branches", label: "Branches", icon: ListTreeIcon, kbds: ["mod", "1"] },
  { view: "reviews", label: "Reviews", icon: GitPullRequestIcon, kbds: ["mod", "2"] },
  { view: "landing", label: "Landing", icon: ChartColumnIcon, kbds: ["mod", "3"] },
]

interface MainViewState {
  view: MainView
}

/**
 * The view the main window shows. A state of the window's session, never persisted: every start shows the branches.
 * Only an action of the user changes it (a switcher, a key, a menu item, a reveal of a branch), never a robot, a sync
 * or an error. In the IDE each view is a tab with a page of its own, and the main view always shows the branches.
 */
export const mainViewStore = createStore<MainViewState>(() => ({ view: "branches" }))

export function useMainView(): MainView {
  return useStore(mainViewStore, state => state.view)
}

/**
 * The view of the app file of the IDE whose tab shows each view of the main window
 * (`docs/design/ide-embedding/protocol/examples/branch-deck.yaml`): Branches is the main view
 */
const TAB_VIEWS: Readonly<Record<MainView, string>> = { branches: "main", reviews: "reviews", landing: "landing" }

/** The call of `brd mcp` that shows the tab of a view (`SHOW_VIEW_CALL` of `crates/mcp-host/src/server.rs`) */
const SHOW_VIEW_CALL = "plugin:air|show_view"

/**
 * The view of the main window that the tab of this page shows, where the host shows each view in a tab of its own
 * (`hasViewTabs`, the IDE); null in the desktop app and in a page of the IDE that is no such tab (a review, Activity)
 */
export const tabView: MainView | null = hasViewTabs ? (MAIN_VIEWS.find(entry => TAB_VIEWS[entry.view] === windowLabel)?.view ?? null) : null

/** An event for the page of the tab a switch shows, as `brd mcp` tells it the page of that tab once the IDE shows it */
export interface TabEvent {
  readonly event: string
  readonly payload?: unknown
}

/** `view` is in another tab of the host than this page: a switch to it asks the host for that tab */
export function isInAnotherTab(view: MainView): boolean {
  return hasViewTabs && tabView !== view
}

/** Asks `brd mcp` to show the tab of `view`, and to tell its page `event` */
function showTab(view: MainView, event: TabEvent | undefined): void {
  const args = { view: TAB_VIEWS[view], ...(event !== undefined && { event: event.event, payload: event.payload ?? null }) }
  invoke(SHOW_VIEW_CALL, args).catch((error: unknown) => logError(`Cannot show the tab ${TAB_VIEWS[view]}`, error))
}

/**
 * Shows `view` in the main window. In the IDE, whose tool window shows each view in a tab of its own, a view of another
 * tab asks the IDE for that tab, and its page hears `event` (in this window the caller does what the event asks itself)
 */
export function showMainView(view: MainView, event?: TabEvent): void {
  if (isInAnotherTab(view)) {
    showTab(view, event)
    return
  }
  if (mainViewStore.getState().view !== view) {
    mainViewStore.setState({ view })
  }
}

/**
 * Shows `view` and lets React commit it at once, so what the caller focuses or scrolls into view next is in a shown
 * view: a hidden view (`display: none`) takes no focus. Only from an event, never from a render or an effect. A view of
 * another tab of the IDE asks for that tab, as `showMainView` does
 */
export function showMainViewNow(view: MainView): void {
  if (isInAnotherTab(view)) {
    showTab(view, undefined)
    return
  }
  if (mainViewStore.getState().view !== view) {
    flushSync(() => showMainView(view))
  }
}
