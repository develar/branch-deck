import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"

/** Whether the Settings dialog of the window is open: "Settings…" of the app menu (⌘,) and of the Your Turn card */
export const settingsDialogStore = createStore<{ open: boolean }>(() => ({ open: false }))

export function useIsSettingsDialogOpen(): boolean {
  return useStore(settingsDialogStore, state => state.open)
}

export function setSettingsDialogOpen(open: boolean): void {
  settingsDialogStore.setState({ open })
}
