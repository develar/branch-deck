import { flushSync } from "react-dom"
import type { LucideIcon } from "lucide-react"
import { ChartColumnIcon, ListTreeIcon } from "lucide-react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

/** The views of the main window: the branch tables, the statistics of the landing into master */
export type MainView = "branches" | "landing"

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
  { view: "landing", label: "Landing", icon: ChartColumnIcon, kbds: ["mod", "2"] },
]

interface MainViewState {
  view: MainView
}

/**
 * The view the main window shows. A state of the window's session, never persisted: every start shows the branches.
 * Only an action of the user changes it (a switcher, a key, a menu item, a reveal of a branch), never a robot, a sync
 * or an error.
 */
export const mainViewStore = createStore<MainViewState>(() => ({ view: "branches" }))

export function useMainView(): MainView {
  return useStore(mainViewStore, state => state.view)
}

/** Shows `view` in the main window */
export function showMainView(view: MainView): void {
  if (mainViewStore.getState().view !== view) {
    mainViewStore.setState({ view })
  }
}

/**
 * Shows `view` and lets React commit it at once, so what the caller focuses or scrolls into view next is in a shown
 * view: a hidden view (`display: none`) takes no focus. Only from an event, never from a render or an effect
 */
export function showMainViewNow(view: MainView): void {
  if (mainViewStore.getState().view !== view) {
    flushSync(() => showMainView(view))
  }
}
