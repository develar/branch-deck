/*
 * The fields of Settings that hold valid typing save it before the window closes (⌘W, the close button, Escape outside
 * the field): one listener of the window (`SettingsPage`) waits for every save a field registered here, since a second
 * listener would close the window as soon as the first one answered.
 */

const saves = new Set<() => Promise<void>>()

/** Registers the save of a field; returns its removal */
export function saveBeforeClose(save: () => Promise<void>): () => void {
  saves.add(save)
  return () => {
    saves.delete(save)
  }
}

/** Every registered save, for the close of the window */
export function saveAllBeforeClose(): Promise<void> {
  return Promise.all([...saves].map(save => save())).then(() => undefined)
}
