import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

/** The views of the main window: the branch tables, the statistics of the landing into master */
export type MainView = "branches" | "landing"

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
