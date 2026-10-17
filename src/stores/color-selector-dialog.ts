import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

/** Whether the Theme dialog of the window is open: "Theme…" of the app menu and of the main menu */
export const colorSelectorDialogStore = createStore<{ open: boolean }>(() => ({ open: false }))

export function useIsColorSelectorDialogOpen(): boolean {
  return useStore(colorSelectorDialogStore, state => state.open)
}

export function setColorSelectorDialogOpen(open: boolean): void {
  colorSelectorDialogStore.setState({ open })
}
